//! History is a stack of actions that can be aplied to Vec<SubGroups>.
//! There is no undo on Vec<SubGroups>, but the original Vec<SubGroup> can be converted multiple times to get the Aggregated result.
//! This way, a we don't need to implement undo, which would require additional infos in HistoryAction
use imask::{ImaskSet, SortedRanges};

use crate::PixelArea;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HistoryActionAdd {
    pub pixel_area: SortedRanges<u32, u32>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HistoryActionClear {
    pub ranges: SortedRanges<u64, u64>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HistoryActionKind {
    Add(HistoryActionAdd),
    Reset,
    Clear(HistoryActionClear),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HistoryAction {
    pub kind: HistoryActionKind,
    pub layer: Option<usize>,
    pub tracked: bool,
}

impl HistoryAction {
    pub fn layer(&self) -> Option<usize> {
        self.layer
    }
    pub fn apply(&self, mut rest: Vec<Option<PixelArea>>) -> Vec<Option<PixelArea>> {
        match &self.kind {
            HistoryActionKind::Add(add) => match self.layer {
                None => {
                    let color = crate::random_color_from_seed(rest.len() as u16);
                    let pixel_area = PixelArea::from_ranges(add.pixel_area.clone(), color);
                    rest.push(Some(pixel_area));
                    rest
                }
                Some(idx) => {
                    while rest.len() <= idx {
                        rest.push(None);
                    }
                    rest[idx] = match rest[idx].take() {
                        Some(existing) => {
                            let new_spans = add.pixel_area.spans::<u64>();
                            existing
                                .map_span_inplace(|existing_spans| existing_spans.union(new_spans))
                        }
                        None => {
                            let color = crate::random_color_from_seed(rest.len() as u16);
                            let pixel_area = PixelArea::from_ranges(add.pixel_area.clone(), color);
                            Some(pixel_area)
                        }
                    };
                    rest
                }
            },
            HistoryActionKind::Reset => match self.layer {
                None => {
                    rest.clear();
                    rest
                }
                Some(idx) => {
                    if let Some(opt) = rest.get_mut(idx) {
                        *opt = None;
                    }
                    rest
                }
            },
            HistoryActionKind::Clear(clear) => match self.layer {
                None => rest
                    .into_iter()
                    .map(|opt_area| {
                        opt_area.and_then(|area| {
                            area.map_span_inplace(|existing_spans| {
                                let clear_spans = clear.ranges.spans();
                                existing_spans.subtract(clear_spans)
                            })
                        })
                    })
                    .collect(),
                Some(idx) => {
                    if let Some(opt_area) = rest.get_mut(idx) {
                        *opt_area = opt_area.take().and_then(|area| {
                            area.map_span_inplace(|existing_spans| {
                                existing_spans.subtract(clear.ranges.spans::<u64>())
                            })
                        });
                    }
                    rest
                }
            },
        }
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
        if let Some(i) = last_action
            && i.kind == HistoryActionKind::Reset
            && i.kind == new_action.kind
            && i.layer == new_action.layer
        {
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
        let tracked_idx = (self.end..self.actions.len()).find(|&i| self.actions[i].tracked)?;
        let mut new_end = tracked_idx + 1;
        while new_end < self.actions.len() && !self.actions[new_end].tracked {
            new_end += 1;
        }
        self.end = new_end;
        Some(&self.actions[tracked_idx])
    }
    pub fn undo(&mut self) -> Option<&HistoryAction> {
        let tracked_idx = (0..self.end).rev().find(|&i| self.actions[i].tracked)?;
        let action = &self.actions[tracked_idx];
        self.end = tracked_idx;
        Some(action)
    }
}

#[cfg(test)]
mod tests {
    use imask::Span;

    use super::*;

    fn tracked_add(x: u32) -> HistoryAction {
        HistoryAction {
            kind: HistoryActionKind::Add(HistoryActionAdd {
                pixel_area: Span::new(x..x + 1, 0).into(),
            }),
            layer: None,
            tracked: true,
        }
    }

    fn untracked_add(x: u32) -> HistoryAction {
        HistoryAction {
            kind: HistoryActionKind::Add(HistoryActionAdd {
                pixel_area: Span::new(x..x + 1, 0).into(),
            }),
            layer: None,
            tracked: false,
        }
    }

    #[test]
    fn undo_empty_returns_none() {
        let mut history = History::default();
        assert_eq!(None, history.undo());
    }

    #[test]
    fn insert_undo_and_redo() {
        let mut history = History::default();
        let item = tracked_add(0);
        history.push(item.clone());
        assert_eq!(history.undo(), Some(&item));
        assert_eq!(history.undo(), None);
        assert_eq!(history.redo(), Some(&item));
    }

    #[test]
    fn push_after_undo() {
        let mut history = History::default();
        let item = tracked_add(0);
        let item2 = tracked_add(10);
        history.push(item.clone());
        assert_eq!(history.undo(), Some(&item));
        assert_eq!(history.undo(), None);
        history.push(item2);
        assert_eq!(None, history.redo());
    }

    #[test]
    fn undo_redo_group_tracked_and_untracked() {
        let mut history = History::default();
        let tracked = tracked_add(0);
        let untracked = untracked_add(10);

        history.push(tracked.clone());
        history.push(untracked.clone());

        assert_eq!(history.end, 2);

        history.undo();
        assert_eq!(history.end, 0);

        history.redo();
        assert_eq!(history.end, 2);
    }

    #[test]
    fn undo_redo_multiple_groups() {
        let mut history = History::default();
        let a = tracked_add(0);
        let b = untracked_add(10);
        let c = tracked_add(20);
        let d = untracked_add(30);

        history.push(a.clone());
        history.push(b.clone());
        history.push(c.clone());
        history.push(d.clone());
        assert_eq!(history.end, 4);

        history.undo();
        assert_eq!(history.end, 2);

        history.undo();
        assert_eq!(history.end, 0);

        history.redo();
        assert_eq!(history.end, 2);

        history.redo();
        assert_eq!(history.end, 4);
    }

    #[test]
    fn redo_stops_at_next_tracked() {
        let mut history = History::default();
        let a = tracked_add(0);
        let b = untracked_add(10);

        history.push(a.clone());
        history.push(b.clone());

        history.undo();
        assert_eq!(history.end, 0);

        history.redo();
        assert_eq!(history.end, 2);

        history.undo();
        assert_eq!(history.end, 0);
    }

    #[test]
    fn only_untracked_actions_cannot_undo() {
        let mut history = History::default();
        history.push(untracked_add(0));
        history.push(untracked_add(10));

        assert_eq!(history.end, 2);

        assert_eq!(None, history.undo());
        assert_eq!(history.end, 2);
    }

    #[test]
    fn consecutive_resets_different_layers_are_not_deduped() {
        let mut history = History::default();
        history.push(HistoryAction {
            kind: HistoryActionKind::Reset,
            layer: Some(0),
            tracked: false,
        });
        history.push(HistoryAction {
            kind: HistoryActionKind::Reset,
            layer: Some(1),
            tracked: false,
        });

        assert_eq!(history.actions.len(), 2);
    }
}
