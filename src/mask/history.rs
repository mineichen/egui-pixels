use std::num::{NonZeroU16, NonZeroU32};

use crate::SubGroups;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HistoryAction {
    Add(String, SubGroups),
    Reset,
    // Box, image_width
    Clear([[usize; 2]; 2], NonZeroU32),
}

impl HistoryAction {
    pub fn apply(&self, mut rest: Vec<Vec<(u32, NonZeroU16)>>) -> Vec<Vec<(u32, NonZeroU16)>> {
        match self {
            HistoryAction::Add(_, s) => rest.push(s.clone()),
            HistoryAction::Reset => rest.clear(),
            HistoryAction::Clear([[x_top, y_top], [x_bottom, y_bottom]], image_width) => {
                let x_left = *x_top as u32;
                let x_right = *x_bottom as u32;
                let x_width = NonZeroU16::try_from((x_right - x_left + 1) as u16).unwrap();

                rest.retain_mut(|mut sub| {
                    let y_range = *y_top as u32..=*y_bottom as u32;
                    super::MaskImage::remove_overlaps(
                        &mut sub,
                        y_range.map(|y| (y * image_width.get() + x_left, x_width)),
                    );

                    !sub.is_empty()
                });
            }
        };
        rest
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
    pub fn iter(&self) -> impl Iterator<Item = &'_ HistoryAction> {
        self.actions.iter().take(self.end)
    }
    pub fn is_dirty(&self) -> bool {
        self.not_dirty_pos != Some(self.end)
    }

    pub fn mark_not_dirty(&mut self) {
        self.not_dirty_pos = Some(self.end);
    }

    pub fn push(&mut self, a: HistoryAction) {
        if matches!(
            (
                &a,
                self.end.checked_sub(1).and_then(|i| self.actions.get(i))
            ),
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
        self.actions.push(a);
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
    use std::num::NonZeroU16;

    use super::*;

    #[test]
    fn undo_empty_returns_none() {
        let mut history = History::default();
        assert_eq!(None, history.undo());
    }

    #[test]
    fn insert_undo_and_redo() {
        let mut history = History::default();
        let item = HistoryAction::Add("Foo".into(), vec![(0, NonZeroU16::MIN)]);
        history.push(item.clone());
        assert_eq!(history.undo(), Some(&item));
        assert_eq!(history.undo(), None);
        assert_eq!(history.redo(), Some(&item));
    }

    #[test]
    fn push_after_undo() {
        let mut history = History::default();
        let item = HistoryAction::Add("Foo".into(), vec![(0, NonZeroU16::MIN)]);
        let item2 = HistoryAction::Add("Foo2".into(), vec![(10, NonZeroU16::MIN)]);
        history.push(item.clone());
        assert_eq!(history.undo(), Some(&item));
        assert_eq!(history.undo(), None);
        history.push(item2);
        assert_eq!(None, history.redo());
    }
}
