//! The mergesort implementations

use crate::algorithms::merging::BufGuard as _;

/// The default insertion sort to use
pub type DefaultInsertionSort = super::insertionsort::InsertionSort;

/// The default [`super::merging::MergingMethod`] to use
pub type DefaultMergingMethod = super::merging::two_way::CopyBoth;

/// The default BufGuardFactory to use
pub type DefaultBufGuardFactory = super::DefaultBufGuardFactory;

/// The default `INSERTION_THRESHOLD` to use
pub const DEFAULT_INSERTION_THRESHOLD: usize = 24;

/// The default `CHECK_SORTED` to use
pub const DEFAULT_CHECK_SORTED: bool = true;

/// The Top-Down Mergesort [`super::Sort`]
pub struct TopDownMergeSort<
    I: super::Sort = DefaultInsertionSort,
    M: super::merging::two_way::MergingMethod = DefaultMergingMethod,
    B: super::BufGuardFactory = DefaultBufGuardFactory,
    const INSERTION_THRESHOLD: usize = DEFAULT_INSERTION_THRESHOLD,
    const CHECK_SORTED: bool = DEFAULT_CHECK_SORTED,
>(
    std::marker::PhantomData<I>,
    std::marker::PhantomData<M>,
    std::marker::PhantomData<B>,
);

impl<
    I: super::Sort,
    M: super::merging::two_way::MergingMethod,
    B: super::BufGuardFactory,
    const INSERTION_THRESHOLD: usize,
    const CHECK_SORTED: bool,
> super::Sort for TopDownMergeSort<I, M, B, INSERTION_THRESHOLD, CHECK_SORTED>
{
    const IS_STABLE: bool = I::IS_STABLE && M::IS_STABLE;

    fn sort<T: Ord>(slice: &mut [T]) {
        if slice.len() < 2 {
            return;
        }

        // Conservatively initiate a buffer big enough to merge the complete array
        let mut buffer = <B::Guard<T>>::with_capacity(M::required_capacity(slice.len()));

        // Delegate to helper function
        Self::top_down_mergesort(slice, buffer.as_uninit_slice_mut());
    }
}

impl<
    I: super::Sort,
    M: super::merging::two_way::MergingMethod,
    B: super::BufGuardFactory,
    const INSERTION_THRESHOLD: usize,
    const CHECK_SORTED: bool,
> TopDownMergeSort<I, M, B, INSERTION_THRESHOLD, CHECK_SORTED>
{
    /// The actual bottom-up mergesort implementation, sorts `slice`
    fn top_down_mergesort<T: Ord>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.len() <= INSERTION_THRESHOLD {
            I::sort(slice);
        } else {
            let middle = slice.len() / 2;

            let (left, right) = slice.split_at_mut(middle);
            Self::top_down_mergesort(left, buffer);
            Self::top_down_mergesort(right, buffer);

            if !CHECK_SORTED || slice[middle] < slice[middle - 1] {
                M::merge(slice, middle, buffer);
            }
        }
    }
}

/// The Bottom-Up Mergesort [`super::Sort`]
pub struct BottomUpMergeSort<
    I: super::Sort = DefaultInsertionSort,
    M: super::merging::two_way::MergingMethod = DefaultMergingMethod,
    B: super::BufGuardFactory = DefaultBufGuardFactory,
    const INSERTION_THRESHOLD: usize = DEFAULT_INSERTION_THRESHOLD,
    const CHECK_SORTED: bool = DEFAULT_CHECK_SORTED,
>(
    std::marker::PhantomData<I>,
    std::marker::PhantomData<M>,
    std::marker::PhantomData<B>,
);

impl<
    I: super::Sort,
    M: super::merging::two_way::MergingMethod,
    B: super::BufGuardFactory,
    const INSERTION_THRESHOLD: usize,
    const CHECK_SORTED: bool,
> super::Sort for BottomUpMergeSort<I, M, B, INSERTION_THRESHOLD, CHECK_SORTED>
{
    const IS_STABLE: bool = I::IS_STABLE && M::IS_STABLE;

    fn sort<T: Ord>(slice: &mut [T]) {
        if slice.len() < 2 {
            return;
        }

        // Conservatively initiate a buffer big enough to merge the complete array
        let mut buffer = <B::Guard<T>>::with_capacity(M::required_capacity(slice.len()));

        // Delegate to helper function
        Self::bottom_up_mergesort(slice, buffer.as_uninit_slice_mut());
    }
}

impl<
    I: super::Sort,
    M: super::merging::two_way::MergingMethod,
    B: super::BufGuardFactory,
    const INSERTION_THRESHOLD: usize,
    const CHECK_SORTED: bool,
> BottomUpMergeSort<I, M, B, INSERTION_THRESHOLD, CHECK_SORTED>
{
    /// The actual bottom-up mergesort implementation, sorts `slice`
    fn bottom_up_mergesort<T: Ord>(slice: &mut [T], buffer: &mut [std::mem::MaybeUninit<T>]) {
        if INSERTION_THRESHOLD > 1 {
            for chunk in slice.chunks_mut(INSERTION_THRESHOLD) {
                I::sort(chunk);
            }

            let mut merge_size = INSERTION_THRESHOLD;
            while merge_size < slice.len() {
                let mut start = 0;

                while start < slice.len() - merge_size {
                    if !CHECK_SORTED || slice[start + merge_size] < slice[start + merge_size - 1] {
                        let end = std::cmp::min(start + 2 * merge_size, slice.len());
                        M::merge(&mut slice[start..end], merge_size, buffer);
                    }

                    start += 2 * merge_size;
                }

                merge_size *= 2;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    const RUNS: usize = 100;
    const TEST_SIZE: usize = 100_000;

    mod bottom_up {
        use super::super::*;
        use super::*;

        type BottomUpMergesortUnchecked = BottomUpMergeSort<
            DefaultInsertionSort,
            DefaultMergingMethod,
            DefaultBufGuardFactory,
            DEFAULT_INSERTION_THRESHOLD,
            false,
        >;

        #[test]
        fn empty() {
            crate::test::test_empty::<BottomUpMergeSort>();
            crate::test::test_empty::<BottomUpMergesortUnchecked>();
        }

        #[test]
        fn random() {
            crate::test::test_random_sorted::<RUNS, TEST_SIZE, BottomUpMergeSort>();
            crate::test::test_random_sorted::<RUNS, TEST_SIZE, BottomUpMergesortUnchecked>();
        }

        #[test]
        fn random_stable() {
            crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, BottomUpMergeSort>();
            crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, BottomUpMergesortUnchecked>();
        }
    }

    mod top_down {
        use super::super::*;
        use super::*;

        type TopDownMergesortUnchecked = TopDownMergeSort<
            DefaultInsertionSort,
            DefaultMergingMethod,
            DefaultBufGuardFactory,
            DEFAULT_INSERTION_THRESHOLD,
            false,
        >;

        #[test]
        fn empty() {
            crate::test::test_empty::<TopDownMergeSort>();
            crate::test::test_empty::<TopDownMergesortUnchecked>();
        }

        #[test]
        fn random() {
            crate::test::test_random_sorted::<RUNS, TEST_SIZE, TopDownMergeSort>();
            crate::test::test_random_sorted::<RUNS, TEST_SIZE, TopDownMergesortUnchecked>();
        }

        #[test]
        fn random_stable() {
            crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, TopDownMergeSort>();
            crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, TopDownMergesortUnchecked>();
        }
    }
}
