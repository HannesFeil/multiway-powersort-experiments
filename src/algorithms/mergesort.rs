//! The mergesort implementations.

use crate::algorithms::merging::BufGuard as _;

/// The default insertion sort to use.
pub type DefaultInsertionSort = super::insertionsort::InsertionSort;

/// The default [`super::merging::MergingMethod`] to use.
pub type DefaultMergingMethod = super::merging::two_way::CopyBoth;

/// The default [`super::BufGuardFactory`] to use.
pub type DefaultBufGuardFactory = super::DefaultBufGuardFactory;

/// The default `BOTTOM_UP` to use.
pub const DEFAULT_BOTTOM_UP: bool = false;

/// The default `INSERTION_THRESHOLD` to use.
pub const DEFAULT_INSERTION_THRESHOLD: usize = 24;

/// The default `CHECK_SORTED` to use.
pub const DEFAULT_CHECK_SORTED: bool = true;

/// Mergesort [`super::Sort`].
///
/// - `I` is the insertion sort, used to sort small sub slices.
/// - `M` is the merging method, used to merge two runs.
/// - `B` is the [`super::BufGuardFactory`] used to create the merging buffer.
/// - `BOTTOM_UP` indicates whether bottom-up mergesort is used as opposed to top-down mergesort.
/// - `INSERTION_THRESHOLD` determines the maximum length of sub slices which are sorted by `I`.
/// - `CHECK_SORTED` enables a check for pre-sortedness before merging two runs.
pub struct MergeSort<
    I: super::Sort = DefaultInsertionSort,
    M: super::merging::MergingMethod = DefaultMergingMethod,
    B: super::BufGuardFactory = DefaultBufGuardFactory,
    const BOTTOM_UP: bool = DEFAULT_BOTTOM_UP,
    const INSERTION_THRESHOLD: usize = DEFAULT_INSERTION_THRESHOLD,
    const CHECK_SORTED: bool = DEFAULT_CHECK_SORTED,
>(
    std::marker::PhantomData<I>,
    std::marker::PhantomData<M>,
    std::marker::PhantomData<B>,
);

impl<
    I: super::Sort,
    M: super::merging::MergingMethod,
    B: super::BufGuardFactory,
    const BOTTOM_UP: bool,
    const INSERTION_THRESHOLD: usize,
    const CHECK_SORTED: bool,
> super::Sort for MergeSort<I, M, B, BOTTOM_UP, INSERTION_THRESHOLD, CHECK_SORTED>
{
    const IS_STABLE: bool = I::IS_STABLE && M::IS_STABLE;

    const BASE_NAME: &str = "mergesort";

    fn parameters() -> impl Iterator<Item = (&'static str, String)> {
        vec![
            ("bottom-up", BOTTOM_UP.to_string()),
            ("i-sort", crate::cli::display_inline::<I>()),
            ("merging", M::display()),
            ("i-threshold", INSERTION_THRESHOLD.to_string()),
            ("check_sorted", CHECK_SORTED.to_string()),
        ]
        .into_iter()
    }

    fn sort<T: Ord>(slice: &mut [T]) {
        if slice.len() < 2 {
            return;
        }

        // Conservatively initiate a buffer big enough to merge the complete array
        let mut buffer = <B::Guard<T>>::with_capacity(M::required_capacity(slice.len()));

        // Delegate to helper function
        if BOTTOM_UP {
            Self::bottom_up_mergesort(slice, buffer.as_uninit_slice_mut());
        } else {
            Self::top_down_mergesort(slice, buffer.as_uninit_slice_mut());
        }
    }
}

impl<
    I: super::Sort,
    M: super::merging::MergingMethod,
    B: super::BufGuardFactory,
    const BOTTOM_UP: bool,
    const INSERTION_THRESHOLD: usize,
    const CHECK_SORTED: bool,
> MergeSort<I, M, B, BOTTOM_UP, INSERTION_THRESHOLD, CHECK_SORTED>
{
    /// The actual top-down mergesort implementation, sorts `slice`
    fn top_down_mergesort<T: Ord>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.len() <= INSERTION_THRESHOLD {
            I::sort(slice);
        } else {
            let middle = slice.len() / 2;

            let (left, right) = slice.split_at_mut(middle);
            Self::top_down_mergesort(left, buffer);
            Self::top_down_mergesort(right, buffer);

            if CHECK_SORTED {
                if left.last().unwrap() > right.first().unwrap() {
                    M::merge(slice, middle, buffer);
                }
            } else {
                M::merge(slice, middle, buffer);
            }
        }
    }

    /// The actual bottom-up mergesort implementation, sorts `slice`
    fn bottom_up_mergesort<T: Ord>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        assert!(
            INSERTION_THRESHOLD >= 1,
            "Insertion threshold has to be greater than or equal to 1"
        );

        // Sort each chunk of insertion threshold
        for chunk in slice.chunks_mut(INSERTION_THRESHOLD) {
            I::sort(chunk);
        }

        let mut merge_size = INSERTION_THRESHOLD;

        // Iterate through merge tree levels from the bottom up
        while merge_size < slice.len() {
            // Merge all runs of length `merge_size`
            for start in (0..slice.len() - merge_size).step_by(merge_size * 2) {
                let end = std::cmp::min(start + 2 * merge_size, slice.len());

                if CHECK_SORTED {
                    if slice[start + merge_size] < slice[start + merge_size - 1] {
                        M::merge(&mut slice[start..end], merge_size, buffer);
                    }
                } else {
                    M::merge(&mut slice[start..end], merge_size, buffer);
                }
            }

            merge_size *= 2;
        }
    }
}

#[cfg(test)]
mod tests {
    const RUNS: usize = crate::test::DEFAULT_RUNS;
    const TEST_SIZE: usize = crate::test::DEFAULT_TEST_SIZE;

    mod bottom_up {
        use super::super::*;
        use super::*;

        type BottomUpMergeSort = MergeSort<
            DefaultInsertionSort,
            DefaultMergingMethod,
            DefaultBufGuardFactory,
            false,
            DEFAULT_INSERTION_THRESHOLD,
            DEFAULT_CHECK_SORTED,
        >;
        type BottomUpMergeSortUnchecked = MergeSort<
            DefaultInsertionSort,
            DefaultMergingMethod,
            DefaultBufGuardFactory,
            false,
            DEFAULT_INSERTION_THRESHOLD,
            false,
        >;

        #[test]
        fn empty() {
            crate::test::test_empty::<BottomUpMergeSort>();
            crate::test::test_empty::<BottomUpMergeSortUnchecked>();
        }

        #[test]
        fn random() {
            crate::test::test_random_sorted::<RUNS, TEST_SIZE, BottomUpMergeSort>();
            crate::test::test_random_sorted::<RUNS, TEST_SIZE, BottomUpMergeSortUnchecked>();
        }

        #[test]
        fn random_stable() {
            crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, BottomUpMergeSort>();
            crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, BottomUpMergeSortUnchecked>();
        }
    }

    mod top_down {
        use super::super::*;
        use super::*;

        type MergesortUnchecked = MergeSort<
            DefaultInsertionSort,
            DefaultMergingMethod,
            DefaultBufGuardFactory,
            DEFAULT_BOTTOM_UP,
            DEFAULT_INSERTION_THRESHOLD,
            false,
        >;

        #[test]
        fn empty() {
            crate::test::test_empty::<MergeSort>();
            crate::test::test_empty::<MergesortUnchecked>();
        }

        #[test]
        fn random() {
            crate::test::test_random_sorted::<RUNS, TEST_SIZE, MergeSort>();
            crate::test::test_random_sorted::<RUNS, TEST_SIZE, MergesortUnchecked>();
        }

        #[test]
        fn random_stable() {
            crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, MergeSort>();
            crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, MergesortUnchecked>();
        }
    }
}
