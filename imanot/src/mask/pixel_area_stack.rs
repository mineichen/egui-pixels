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

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (usize, &'_ PixelArea)> {
        PixelAreaStackIter {
            index: 0,
            end_index: self.areas.len(),
            inner: self.areas.iter().map(|x| x.as_ref()),
        }
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

impl IntoIterator for PixelAreaStack {
    type Item = (usize, PixelArea);

    type IntoIter = PixelAreaStackIter<std::vec::IntoIter<Option<PixelArea>>>;

    fn into_iter(mut self) -> Self::IntoIter {
        let extracted = std::sync::Arc::make_mut(&mut self.areas);

        PixelAreaStackIter {
            index: 0,
            end_index: extracted.len(),
            inner: std::mem::take(extracted).into_iter(),
        }
    }
}

pub struct PixelAreaStackIter<T> {
    index: usize,
    end_index: usize,
    inner: T,
}

impl<T: Iterator<Item = Option<TItem>>, TItem> Iterator for PixelAreaStackIter<T> {
    type Item = (usize, TItem);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = self.inner.next()?;
            let cur_index = self.index;
            self.index += 1;
            if let Some(x) = next {
                return Some((cur_index, x));
            }
        }
    }
}

impl<T: DoubleEndedIterator<Item = Option<TItem>>, TItem> DoubleEndedIterator
    for PixelAreaStackIter<T>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            let next = self.inner.next_back()?;
            self.end_index -= 1;

            if let Some(x) = next {
                return Some((self.end_index, x));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use imask::{SortedRanges, Span};

    use super::*;
    const NON_ZERO_10: NonZeroU32 = NonZeroU32::new(10).unwrap();
    #[test]
    fn allow_unordered() {
        let stack = PixelAreaStack::from_iter([
            (10, PixelArea::single_range_total_black(0, 0, NON_ZERO_10)),
            (1, PixelArea::single_range_total_black(0, 0, NON_ZERO_10)),
        ]);
        assert!(stack.get(1).is_some());
        assert!(stack.get(10).is_some());
    }

    #[test]
    fn test_end_index() {
        let ranges = SortedRanges::from(Span::new(0..10, 0));
        let example = PixelArea::from_ranges(ranges, [0, 0, 0, 255]);
        let x =
            PixelAreaStack::from_iter([(1, example.clone()), (3, example.clone()), (5, example)]);

        let iter = x.iter().map(|(i, _)| i);
        assert_eq!(vec![1, 3, 5], iter.collect::<Vec<_>>());
    }
}
