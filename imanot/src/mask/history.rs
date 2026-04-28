//! History is a stack of actions that can be aplied to Vec<SubGroups>.
//! There is no undo on Vec<SubGroups>, but the original Vec<SubGroup> can be converted multiple times to get the Aggregated result.
//! This way, a we don't need to implement undo, which would require additional infos in HistoryAction

use std::ops::RangeInclusive;

use imask::{ImageDimension, SortedRanges};
use itertools::Itertools;
use range_set_blaze::SortedDisjointMap;

use crate::{Meta, PixelArea};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HistoryActionAdd {
    pub pixel_area: PixelArea,
    pub layer: Option<usize>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HistoryActionClear {
    pub ranges: SortedRanges<u64, u64>,
    pub layer: Option<usize>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HistoryAction {
    Add(HistoryActionAdd),
    Reset,
    Clear(HistoryActionClear),
}

impl HistoryAction {
    pub fn layer(&self) -> Option<usize> {
        match self {
            HistoryAction::Add(x) => x.layer,
            HistoryAction::Reset => None,
            HistoryAction::Clear(x) => x.layer,
        }
    }
    pub fn apply(&self, mut rest: Vec<Option<PixelArea>>) -> Vec<Option<PixelArea>> {
        match self {
            HistoryAction::Add(add) => match add.layer {
                None => {
                    rest.push(Some(add.pixel_area.clone()));
                    rest
                }
                Some(idx) => {
                    while rest.len() <= idx {
                        rest.push(None);
                    }
                    rest[idx] = match rest[idx].take() {
                        Some(existing) => {
                            let new_iter = add.pixel_area.pixels.iter::<RangeInclusive<u64>>();
                            let new_iter = range_set_blaze::CheckSortedDisjointMap::new(
                                // Meta will soon be removed, this is a workaround
                                new_iter.map(|(r, m)| (r, *m)).coalesce(|a, b| {
                                    if *a.0.end() == b.0.start() - 1 {
                                        Ok((*a.0.start()..=*b.0.end(), a.1))
                                    } else {
                                        Err((a, b))
                                    }
                                }),
                            );
                            existing.map_inplace(|existing_iter| {
                                let r = existing_iter.union(new_iter);
                                // Temporary fix until Meta-Removeal: Merge Meta to avoid adjacent
                                r.coalesce(|a, b| {
                                    if *a.0.end() == b.0.start() - 1 {
                                        Ok((*a.0.start()..=*b.0.end(), a.1))
                                    } else {
                                        Err((a, b))
                                    }
                                })
                            })
                        }
                        None => Some(add.pixel_area.clone()),
                    };
                    rest
                }
            },
            HistoryAction::Reset => {
                rest.clear();
                rest
            }
            HistoryAction::Clear(clear) => match clear.layer {
                None => rest
                    .into_iter()
                    .map(|opt_area| {
                        opt_area.and_then(|area| {
                            let width = area.pixels.width();
                            area.map_inplace(|x| {
                                x.map_and_set_difference(
                                    clear.ranges.iter_global_with::<RangeInclusive<u64>>(width),
                                )
                            })
                        })
                    })
                    .collect(),
                Some(idx) => {
                    if let Some(opt_area) = rest.get_mut(idx) {
                        *opt_area = opt_area.take().and_then(|area| {
                            let width = area.pixels.width();
                            area.map_inplace(|x| {
                                x.map_and_set_difference(
                                    clear.ranges.iter_global_with::<RangeInclusive<u64>>(width),
                                )
                            })
                        });
                    }
                    rest
                }
            },
        }
    }
}

impl range_set_blaze::ValueRef for Meta {
    type Target = Meta;

    fn into_value(self) -> Self::Target {
        self
    }
}

pub struct History {
    actions: Vec<HistoryAction>,
    end: usize,
    not_dirty_pos: Option<usize>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            actions: Default::default(),
            end: Default::default(),
            not_dirty_pos: Some(0),
        }
    }
}

impl History {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &'_ HistoryAction> {
        self.actions.iter().take(self.end)
    }

    pub(crate) fn random_seed(&self) -> u16 {
        self.end as u16
    }

    pub fn is_dirty(&self) -> bool {
        self.not_dirty_pos != Some(self.end)
    }

    pub fn mark_not_dirty(&mut self) {
        self.not_dirty_pos = Some(self.end);
    }

    pub fn push(&mut self, new_action: HistoryAction) {
        let last_action = self.end.checked_sub(1).and_then(|i| self.actions.get(i));
        if matches!(
            (&new_action, last_action),
            (HistoryAction::Reset, Some(HistoryAction::Reset))
        ) {
            return;
        }

        match &mut self.not_dirty_pos {
            Some(pos) if *pos > self.end => {
                self.not_dirty_pos = None;
            }
            _ => (),
        }

        self.actions.truncate(self.end);
        self.actions.push(new_action);
        self.end = self.actions.len();
    }

    pub fn redo(&mut self) -> Option<&HistoryAction> {
        let item = self.actions.get(self.end)?;
        self.end += 1;
        Some(item)
    }

    pub fn undo(&mut self) -> Option<&HistoryAction> {
        let item = self.actions.get(self.end.checked_sub(1)?)?;
        self.end -= 1;

        Some(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU32;

    const ONE: NonZeroU32 = NonZeroU32::MIN;
    const TEN: NonZeroU32 = NonZeroU32::new(10).unwrap();

    #[test]
    fn undo_empty_returns_none() {
        let mut history = History::default();
        assert_eq!(None, history.undo());
    }

    #[test]
    fn insert_undo_and_redo() {
        let mut history = History::default();
        let item = HistoryAction::Add(HistoryActionAdd {
            pixel_area: PixelArea::single_range_total_black(0, 0, ONE, TEN),
            layer: None,
        });
        history.push(item.clone());
        assert_eq!(history.undo(), Some(&item));
        assert_eq!(history.undo(), None);
        assert_eq!(history.redo(), Some(&item));
    }

    #[test]
    fn push_after_undo() {
        let mut history = History::default();
        let item = HistoryAction::Add(HistoryActionAdd {
            pixel_area: PixelArea::single_range_total_black(0, 0, ONE, TEN),
            layer: None,
        });
        let item2 = HistoryAction::Add(HistoryActionAdd {
            pixel_area: PixelArea::single_range_total_black(10, 0, ONE, TEN),
            layer: None,
        });
        history.push(item.clone());
        assert_eq!(history.undo(), Some(&item));
        assert_eq!(history.undo(), None);
        history.push(item2);
        assert_eq!(None, history.redo());
    }
}
