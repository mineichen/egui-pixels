use std::{iter::FusedIterator, ops::RangeInclusive};

use range_set_blaze::Integer;

pub struct MergeSortedOverlapping<I, T> {
    iter: I,
    acc: Option<RangeInclusive<T>>,
}

impl<I, T> MergeSortedOverlapping<I, T> {
    pub fn new(iter: impl IntoIterator<IntoIter = I>) -> Self {
        Self {
            iter: iter.into_iter(),
            acc: None,
        }
    }
}

impl<I, T: Integer> Iterator for MergeSortedOverlapping<I, T>
where
    I: Iterator<Item = RangeInclusive<T>>,
{
    type Item = RangeInclusive<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut iter = (&mut self.iter).inspect(|x| {
            let (start, end) = (x.start(), x.end());
            debug_assert!(
                start < end,
                "Got a range with start > end ({start:?} > {end:?})"
            )
        });
        let mut last = self.acc.take().or_else(|| iter.next())?;
        loop {
            match iter.next() {
                None => return Some(last),
                Some(next) => {
                    let (last_start, next_start) = (*last.start(), *next.start());
                    let (last_end, next_end) = (*last.end(), *next.end());
                    if last_start > next_start {
                        panic!(
                            "MergeSortedOverlapping: input not sorted by start. Got range {next_start:?} after range starting at {last_start:?}"
                        );
                    }
                    if next_start > last_end.add_one() {
                        self.acc = Some(next);
                        return Some(last);
                    }
                    last = last_start..=last_end.max(next_end);
                }
            }
        }
    }
}

impl<I, T: Integer> FusedIterator for MergeSortedOverlapping<I, T> where
    I: FusedIterator<Item = RangeInclusive<T>>
{
}

impl<I, T: Integer> range_set_blaze::SortedStarts<T> for MergeSortedOverlapping<I, T> where
    I: FusedIterator<Item = RangeInclusive<T>>
{
}
impl<I, T: Integer> range_set_blaze::SortedDisjoint<T> for MergeSortedOverlapping<I, T> where
    I: FusedIterator<Item = RangeInclusive<T>>
{
}

#[cfg(test)]
mod tests {
    use range_set_blaze::CheckSortedDisjoint;

    use super::*;

    #[test]
    fn empty() {
        let result =
            MergeSortedOverlapping::new([] as [RangeInclusive<u64>; 0]).collect::<Vec<_>>();
        assert_eq!(result, vec![]);
    }

    #[test]
    #[should_panic(expected = "start > end (10 > 9)")]
    fn range_with_end_bigger_start_after_initial() {
        MergeSortedOverlapping::new([0..=2, 10..=9]).next();
    }

    #[test]
    #[should_panic(expected = "start > end (10 > 9)")]
    fn range_with_end_bigger_start() {
        MergeSortedOverlapping::new([10..=9]).next();
    }

    #[test]
    fn last_range_has_not_the_highest_end() {
        let result = MergeSortedOverlapping::new([0..=10, 1..=8]).collect::<Vec<_>>();
        assert_eq!(result, vec![0..=10]);
    }

    #[test]
    #[should_panic(expected = "input not sorted by start")]
    fn out_of_order_panics() {
        MergeSortedOverlapping::new([5..=7, 1..=3]).for_each(|_| {});
    }

    // Allowed as this still causes a valid output
    // In contrast, `out_of_order_with_sooner_start_then_accumulator_start` cannot know if a smaller range was released already without tracking more variables
    #[test]
    fn out_of_order_after_merge_is_accepted() {
        assert_eq!(
            MergeSortedOverlapping::new([1..=5, 3..=7, 2..=103]).collect::<Vec<_>>(),
            vec![1..=103]
        );
    }

    #[test]
    fn out_of_order_with_same_start_then_accumulator_start() {
        assert_eq!(
            vec![1..=21],
            MergeSortedOverlapping::new([1..=5, 4..=20, 1..=21]).collect::<Vec<_>>()
        );
    }

    #[test]
    #[should_panic(expected = "input not sorted by start")]
    fn out_of_order_with_sooner_start_then_accumulator_start() {
        MergeSortedOverlapping::new([1..=5, 3..=7, 0..=103]).next();
    }

    #[test]
    fn two_disjoint() {
        let result = MergeSortedOverlapping::new([(1..=3), (5..=7)]).collect::<Vec<_>>();
        assert_eq!(result, vec![1..=3, 5..=7]);
    }

    #[test]
    fn two_overlapping() {
        let result = MergeSortedOverlapping::new([1..=5, 3..=7]).collect::<Vec<_>>();
        assert_eq!(result, vec![(1..=7)]);
    }

    #[test]
    fn two_touching() {
        let result = MergeSortedOverlapping::new([1..=3, 4..=7]).collect::<Vec<_>>();
        assert_eq!(result, vec![(1..=7)]);
    }

    #[test]
    fn two_touching_adjacent() {
        let result = MergeSortedOverlapping::new([1..=3, 3..=7]).collect::<Vec<_>>();
        assert_eq!(result, vec![1..=7]);
    }

    #[test]
    fn second_contained_in_first() {
        let result = MergeSortedOverlapping::new([1..=10, 3..=5]).collect::<Vec<_>>();
        assert_eq!(result, vec![(1..=10)]);
    }

    #[test]
    fn three_merge_all() {
        let result = MergeSortedOverlapping::new([1..=3, 2..=5, 4..=7]).collect::<Vec<_>>();
        assert_eq!(result, vec![1..=7]);
    }

    #[test]
    fn three_partial_merge() {
        let result = MergeSortedOverlapping::new([1..=3, 5..=7, 6..=9]).collect::<Vec<_>>();
        assert_eq!(result, vec![1..=3, 5..=9]);
    }

    #[test]
    fn many_interleaved() {
        let result = MergeSortedOverlapping::new([1..=3, 2..=4, 3..=5]).collect::<Vec<_>>();
        assert_eq!(result, vec![1..=5]);
    }

    #[test]
    fn same_start_different_end() {
        let result = MergeSortedOverlapping::new([1..=3, 1..=5, 1..=7]).collect::<Vec<_>>();
        assert_eq!(result, vec![1..=7]);
    }

    #[test]
    fn same_range_multiple_times() {
        let result = MergeSortedOverlapping::new([1..=3, 1..=3, 1..=3]).collect::<Vec<_>>();
        assert_eq!(result, vec![1..=3]);
    }

    #[test]
    fn range_set_blaze_contract() {
        let merged: Vec<_> = MergeSortedOverlapping::new([0..=1, 1..=4]).collect();
        for w in merged.windows(2) {
            assert!(
                w[0].end() < w[1].start(),
                "ranges must be disjoint: {:?} and {:?}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn fused_iterator_returns_none_after_exhaustion() {
        let mut iter = MergeSortedOverlapping::new([1..=3]);
        assert_eq!(iter.next(), Some(1..=3));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn reproduce_user_crash_case() {
        use range_set_blaze::CheckSortedDisjoint;

        let source_ranges = vec![
            2365505_u64..=2365559_u64,
            2365651_u64..=2365701_u64,
            2366806_u64..=2367960_u64,
            2367961_u64..=2368095_u64,
            2368662_u64..=2369039_u64,
        ];

        let merged = MergeSortedOverlapping::new(source_ranges.into_iter());
        CheckSortedDisjoint::new(merged).for_each(|_| {});
    }

    #[test]
    fn same_start_smaller_end_after_larger() {
        let result = MergeSortedOverlapping::new([1..=10, 1..=5, 1..=3]).collect::<Vec<_>>();
        assert_eq!(result, vec![1..=10]);
    }

    #[test]
    #[should_panic(expected = "input not sorted by start")]
    fn same_start_varied_ends_interleaved_with_others_panics() {
        MergeSortedOverlapping::new([1..=5, 1..=10, 20..=30, 1..=15]).for_each(|_| {});
    }

    #[test]
    fn adjacent_ranges_are_merged_for_check_sorted_disjoint() {
        let merge = MergeSortedOverlapping::new([1..=5, 6..=10, 20..=30]);
        let result = CheckSortedDisjoint::new(merge).collect::<Vec<_>>();
        assert_eq!(result, vec![1..=10, 20..=30]);
    }

    #[test]
    fn with_check_sorted_disjoint_overlapping_same_start() {
        let merged = MergeSortedOverlapping::new([1..=10, 1..=5, 1..=15, 1..=12]);
        let result = CheckSortedDisjoint::new(merged).collect::<Vec<_>>();
        assert_eq!(result, vec![1..=15]);
    }
}
