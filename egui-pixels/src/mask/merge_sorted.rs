use std::{iter::FusedIterator, ops::RangeInclusive};

pub struct MergeSortedOverlapping<I> {
    iter: I,
    acc: Option<RangeInclusive<u64>>,
    last_start: Option<u64>,
}

impl<I> MergeSortedOverlapping<I> {
    pub fn new(iter: impl IntoIterator<IntoIter = I>) -> Self {
        Self {
            iter: iter.into_iter(),
            acc: None,
            last_start: None,
        }
    }
}

impl<I> Iterator for MergeSortedOverlapping<I>
where
    I: Iterator<Item = RangeInclusive<u64>>,
{
    type Item = RangeInclusive<u64>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.acc.take() {
                None => {
                    let next = self.iter.next();
                    if let Some(range) = &next {
                        self.last_start = Some(*range.start());
                    }
                    self.acc = next;
                    if self.acc.is_none() {
                        return None;
                    }
                }
                Some(acc) => {
                    let acc_end = *acc.end();
                    match self.iter.next() {
                        None => return Some(acc),
                        Some(next) => {
                            let next_start = *next.start();
                            let next_end = *next.end();
                            if let Some(last_start) = self.last_start {
                                if next_start < last_start {
                                    panic!(
                                        "MergeSortedOverlapping: input not sorted by start. \
                                         Got range {:?} after range starting at {}",
                                        next, last_start
                                    );
                                }
                            }
                            self.last_start = Some(next_start);
                            if next_start > acc_end + 1 {
                                self.acc = Some(next);
                                return Some(acc);
                            }
                            if next_end > acc_end {
                                self.acc = Some(*acc.start()..=next_end);
                            } else {
                                self.acc = Some(acc);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl<I> FusedIterator for MergeSortedOverlapping<I> where
    I: FusedIterator<Item = RangeInclusive<u64>>
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let result = MergeSortedOverlapping::new([]).collect::<Vec<_>>();
        assert_eq!(result, vec![]);
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

    #[test]
    #[should_panic(expected = "input not sorted by start")]
    fn out_of_order_after_merge_panics() {
        MergeSortedOverlapping::new([1..=5, 3..=7, 2..=103]).for_each(|_| {});
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
    fn from_test_case() {
        let result = MergeSortedOverlapping::new([0..=1, 1..=4]).collect::<Vec<_>>();
        assert_eq!(result, vec![0..=4]);
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

    fn verify_disjoint(merged: &[RangeInclusive<u64>]) {
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
    fn verify_all_cases_produce_disjoint_ranges() {
        let test_cases: Vec<Vec<RangeInclusive<u64>>> = vec![
            vec![],
            vec![1..=3],
            vec![1..=3, 5..=7],
            vec![1..=5, 3..=7],
            vec![1..=3, 4..=7],
            vec![1..=3, 3..=7],
            vec![1..=10, 3..=5],
            vec![1..=3, 2..=5, 4..=7],
            vec![1..=3, 5..=7, 6..=9],
            vec![1..=3, 2..=4, 3..=5],
            vec![1..=3, 1..=5, 1..=7],
            vec![1..=3, 1..=3, 1..=3],
            vec![0..=1, 1..=4],
        ];

        for case in test_cases {
            let merged: Vec<_> = MergeSortedOverlapping::new(case.into_iter()).collect();
            verify_disjoint(&merged);
        }
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
        let _ = MergeSortedOverlapping::new([1..=5, 1..=10, 20..=30, 1..=15]).collect::<Vec<_>>();
    }

    #[test]
    #[should_panic(expected = "input not sorted by start")]
    fn same_start_with_out_of_order_after_return() {
        let _ = MergeSortedOverlapping::new([1..=5, 10..=20, 1..=15]).collect::<Vec<_>>();
    }

    #[test]
    fn debug_check_what_check_sorted_disjoint_expects() {
        use range_set_blaze::CheckSortedDisjoint;

        let source_ranges = vec![1_u64..=5_u64, 7_u64..=10_u64];

        let checked: CheckSortedDisjoint<u64, _> =
            CheckSortedDisjoint::new(source_ranges.into_iter());
        let result: Vec<_> = checked.map(|r| (*r.start(), *r.end())).collect();
        assert_eq!(result, vec![(1, 5), (7, 10)]);
    }

    #[test]
    #[should_panic(expected = "ranges must be disjoint")]
    fn debug_adjacent_ranges_directly_panics() {
        use range_set_blaze::CheckSortedDisjoint;

        let source_ranges = vec![1_u64..=5_u64, 6_u64..=10_u64];

        let checked: CheckSortedDisjoint<u64, _> =
            CheckSortedDisjoint::new(source_ranges.into_iter());
        let _: Vec<_> = checked.collect();
    }

    #[test]
    fn adjacent_ranges_are_merged_for_check_sorted_disjoint() {
        use range_set_blaze::CheckSortedDisjoint;

        let source_ranges = vec![1_u64..=5_u64, 6_u64..=10_u64, 20_u64..=30_u64];

        let merged = MergeSortedOverlapping::new(source_ranges.into_iter());
        let checked: CheckSortedDisjoint<u64, _> = CheckSortedDisjoint::new(merged);
        let result: Vec<_> = checked.map(|r| (*r.start(), *r.end())).collect();
        assert_eq!(result, vec![(1, 10), (20, 30)]);
    }

    #[test]
    fn with_check_sorted_disjoint_overlapping_same_start() {
        use range_set_blaze::CheckSortedDisjoint;

        let source_ranges = vec![
            1_u64..=10_u64,
            1_u64..=5_u64,
            1_u64..=15_u64,
            1_u64..=12_u64,
        ];

        let merged = MergeSortedOverlapping::new(source_ranges.into_iter());
        let checked: CheckSortedDisjoint<u64, _> = CheckSortedDisjoint::new(merged);
        let result: Vec<_> = checked.map(|r| (*r.start(), *r.end())).collect();
        assert_eq!(result, vec![(1, 15)]);
    }
}
