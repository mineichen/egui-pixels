//! History is a stack of actions that can be aplied to Vec<SubGroups>.
//! There is no undo on Vec<SubGroups>, but the original Vec<SubGroup> can be converted multiple times to get the Aggregated result.
//! This way, a we don't need to implement undo, which would require additional infos in HistoryAction

use crate::{SubGroup, Annotation};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum HistoryAction {
    Add(Annotation),
    Reset,
    Clear(Vec<SubGroup>),
}

impl HistoryAction {
    pub fn apply(&self, mut rest: Vec<Annotation>) -> Vec<Annotation> {
        match self {
            HistoryAction::Add(s) => rest.push(s.clone()),
            HistoryAction::Reset => rest.clear(),
            HistoryAction::Clear(s) => {
                rest.retain_mut(|sub| {
                    crate::remove_overlaps(sub, s.iter().copied());
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

    pub(crate) fn random_seed(&self) -> u16 {
        self.end as u16
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

    use crate::SubGroup;

    use super::*;

    #[test]
    fn undo_empty_returns_none() {
        let mut history = History::default();
        assert_eq!(None, history.undo());
    }

    #[test]
    fn insert_undo_and_redo() {
        let mut history = History::default();
        let item = HistoryAction::Add(Annotation::with_black_color(vec![SubGroup::new_total(
            0,
            NonZeroU16::MIN,
        )]));
        history.push(item.clone());
        assert_eq!(history.undo(), Some(&item));
        assert_eq!(history.undo(), None);
        assert_eq!(history.redo(), Some(&item));
    }

    #[test]
    fn push_after_undo() {
        let mut history = History::default();
        let item = HistoryAction::Add(Annotation::with_black_color(vec![SubGroup::new_total(
            0,
            NonZeroU16::MIN,
        )]));
        let item2 = HistoryAction::Add(Annotation::with_black_color(vec![SubGroup::new_total(
            10,
            NonZeroU16::MIN,
        )]));
        history.push(item.clone());
        assert_eq!(history.undo(), Some(&item));
        assert_eq!(history.undo(), None);
        history.push(item2);
        assert_eq!(None, history.redo());
    }
}
