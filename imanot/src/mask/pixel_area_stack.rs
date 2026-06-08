use std::sync::Arc;

use crate::PixelArea;

#[derive(Clone, PartialEq, Eq, Default)]
pub struct PixelAreaStack {
    areas: Arc<Vec<Option<PixelArea>>>,
}
impl From<Vec<PixelArea>> for PixelAreaStack {
    fn from(areas: Vec<PixelArea>) -> Self {
        Self {
            areas: Arc::new(areas.into_iter().map(Some).collect()),
        }
    }
}

impl PixelAreaStack {
    pub fn get(&self, i: usize) -> Option<&PixelArea> {
        self.areas.get(i)?.as_ref()
    }
    pub fn from_iter(areas: impl IntoIterator<Item = (usize, PixelArea)>) -> Self {
        let mut all = vec![];
        for (i, area) in areas {
            if let Some(x) = all.get_mut(i) {
                *x = Some(area);
            } else {
                all.resize(i, None);
                all.push(Some(area))
            }
        }
        Self {
            areas: Arc::new(all),
        }
    }

    pub fn max_layer(&self) -> usize {
        self.areas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.areas.iter().all(Option::is_none)
    }

    pub fn iter(&self) -> PixelAreaStackIter<'_> {
        PixelAreaStackIter {
            inner: self.areas.iter().enumerate(),
        }
    }

    pub(super) fn make_mut_if_exists(&mut self, index: usize) -> Option<&mut PixelArea> {
        self.areas.get(index)?;
        let inner = Arc::make_mut(&mut self.areas);

        inner.get_mut(index).unwrap().as_mut()
    }
    pub(super) fn make_mut(&mut self, index: usize) -> &mut Option<PixelArea> {
        let inner = Arc::make_mut(&mut self.areas);
        if inner.len() <= index {
            inner.resize(index + 1, None);
        }

        inner.get_mut(index).unwrap()
    }
    pub(super) fn to_option_vec(&self) -> Vec<Option<PixelArea>> {
        Vec::clone(&self.areas)
    }
    pub(super) fn from_option_vec(areas: Vec<Option<PixelArea>>) -> Self {
        Self {
            areas: Arc::new(areas),
        }
    }
}

pub struct PixelAreaStackIter<'a> {
    inner: std::iter::Enumerate<std::slice::Iter<'a, Option<PixelArea>>>,
}

impl<'a> Iterator for PixelAreaStackIter<'a> {
    type Item = (usize, &'a PixelArea);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = self.inner.next()?;
            if let Some(x) = next.1 {
                return Some((next.0, x));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::*;
    const NON_ZERO_10: NonZeroU32 = NonZeroU32::new(10).unwrap();
    #[test]
    fn allow_unordered() {
        let stack = PixelAreaStack::from_iter([
            (
                10,
                PixelArea::single_range_total_black(0, 0, NON_ZERO_10, NON_ZERO_10),
            ),
            (
                1,
                PixelArea::single_range_total_black(0, 0, NON_ZERO_10, NON_ZERO_10),
            ),
        ]);
        assert!(stack.get(1).is_some());
        assert!(stack.get(10).is_some());
    }
}
