//! The top down mergesort implementation

use crate::algorithms::merging::BufGuard as _;

/// The default insertion sort to use
pub type DefaultInsertionSort = super::insertionsort::InsertionSort;

/// The default [`super::merging::MergingMethod`] to use
pub type DefaultMergingMethod = super::merging::CopyBoth;

/// The default BufGuardFactory to use
pub type DefaultBufGuardFactory = super::DefaultBufGuardFactory;

/// The default `INSERTION_THRESHOLD` to use
pub const DEFAULT_INSERTION_THRESHOLD: usize = 24;

/// The default `CHECK_SORTED` to use
pub const DEFAULT_CHECK_SORTED: bool = true;

/// The Top-Down Mergesort [`super::Sort`]
pub struct TopDownMergeSort<
    I: super::Sort = DefaultInsertionSort,
    M: super::merging::MergingMethod = DefaultMergingMethod,
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
    M: super::merging::MergingMethod,
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
    M: super::merging::MergingMethod,
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

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: usize = 100;
    const TEST_SIZE: usize = 100_000;

    /// Default peeksort but allowing decreasing runs
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
