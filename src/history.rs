use crate::{Annotation, SubGroups};

#[derive(Default)]
pub struct History {
    actions: Vec<Annotation>,
    end: usize,
}

impl History {
    pub fn iter(&self) -> impl Iterator<Item = &'_ SubGroups> {
        self.actions.iter().take(self.end).map(|(_, x)| x)
    }

    pub fn push(&mut self, a: Annotation) {
        self.actions.truncate(self.end);
        self.actions.push(a);
        self.end = self.actions.len();
    }

    pub fn redo(&mut self) -> Option<&Annotation> {
        let item = self.actions.get(self.end)?;
        self.end += 1;
        Some(item)
    }

    pub fn undo(&mut self) -> Option<&Annotation> {
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
        let item = ("Foo".into(), vec![(0, NonZeroU16::MIN)]);
        history.push(item.clone());
        assert_eq!(history.undo(), Some(&item));
        assert_eq!(history.undo(), None);
        assert_eq!(history.redo(), Some(&item));
    }

    #[test]
    fn push_after_undo() {
        let mut history = History::default();
        let item = ("Foo".into(), vec![(0, NonZeroU16::MIN)]);
        let item2 = ("Foo2".into(), vec![(10, NonZeroU16::MIN)]);
        history.push(item.clone());
        assert_eq!(history.undo(), Some(&item));
        assert_eq!(history.undo(), None);
        history.push(item2);
        assert_eq!(None, history.redo());
    }
}
