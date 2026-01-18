//! The peeksort implementation

use crate::algorithms::merging::BufGuard as _;

/// The default insertion sort to use
pub type DefaultInsertionSort = super::insertionsort::InsertionSort;

/// The default [`super::merging::MergingMethod`] to use
pub type DefaultMergingMethod = super::merging::two_way::CopyBoth;

/// The default BufGuardFactory to use
pub type DefaultBufGuardFactory = super::DefaultBufGuardFactory;

/// The default `INSERTION_THRESHOLD` to use
pub const DEFAULT_INSERTION_THRESHOLD: usize = 24;

/// The default `ONLY_INCREASING_RUNS` to use
pub const DEFAULT_ONLY_INCREASING_RUNS: bool = true;

/// The peeksort [`super::Sort`]
pub struct PeekSort<
    I: super::Sort = DefaultInsertionSort,
    M: super::merging::two_way::MergingMethod = DefaultMergingMethod,
    B: super::BufGuardFactory = DefaultBufGuardFactory,
    const INSERTION_THRESHOLD: usize = DEFAULT_INSERTION_THRESHOLD,
    const ONLY_INCREASING_RUNS: bool = DEFAULT_ONLY_INCREASING_RUNS,
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
    const ONLY_INCREASING_RUNS: bool,
> super::PostfixSort for PeekSort<I, M, B, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>
{
    const IS_STABLE: bool = I::IS_STABLE && M::IS_STABLE;

    const BASE_NAME: &str = "peeksort";

    fn parameters() -> impl Iterator<Item = (&'static str, String)> {
        vec![
            ("i-sort", super::display_inline::<I>()),
            ("merging", M::display()),
            ("i-threshold", INSERTION_THRESHOLD.to_string()),
            ("only-increasing", ONLY_INCREASING_RUNS.to_string()),
        ]
        .into_iter()
    }

    fn sort<T: Ord>(slice: &mut [T], split_point: usize) {
        if slice.len() < 2 {
            return;
        }

        // Conservatively initiate a buffer big enough to merge the complete array
        let mut buffer = <B::Guard<T>>::with_capacity(M::required_capacity(slice.len()));

        // Delegate to helper function
        Self::peeksort(
            slice,
            buffer.as_uninit_slice_mut(),
            split_point,
            slice.len() - 1,
        );
    }
}

impl<
    I: super::Sort,
    M: super::merging::two_way::MergingMethod,
    B: super::BufGuardFactory,
    const INSERTION_THRESHOLD: usize,
    const ONLY_INCREASING_RUNS: bool,
> PeekSort<I, M, B, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>
{
    /// The actual peek sort implementation
    ///
    /// Sorts `slice` under the assumption, that `slice[..left_run_end]` and
    /// `slice[right_run_begin..]` are already sorted.
    fn peeksort<T: Ord>(
        slice: &mut [T],
        buffer: &mut [std::mem::MaybeUninit<T>],
        left_run_end: usize,
        right_run_begin: usize,
    ) {
        // Assert invariant in debug build
        debug_assert!(slice[..left_run_end].is_sorted() && slice[right_run_begin..].is_sorted());

        if left_run_end == slice.len() || right_run_begin == 0 {
            return;
        }

        // Use insertion sort for small slices
        if slice.len() < INSERTION_THRESHOLD {
            I::sort(slice);
            return;
        }

        let middle = slice.len() / 2;

        if middle <= left_run_end {
            Self::peeksort(
                &mut slice[left_run_end..],
                buffer,
                1,
                right_run_begin - left_run_end,
            );
            M::merge(slice, left_run_end, buffer);
        } else if middle >= right_run_begin {
            Self::peeksort(
                &mut slice[..right_run_begin],
                buffer,
                left_run_end,
                right_run_begin - 1,
            );
            M::merge(slice, right_run_begin, buffer);
        } else {
            let (i, j);

            #[allow(
                clippy::collapsible_else_if,
                reason = "Clearer distinction between compile time and runtime checks"
            )]
            if ONLY_INCREASING_RUNS {
                i = left_run_end
                    + crate::algorithms::merging::util::weakly_increasing_suffix_index(
                        &slice[left_run_end..middle],
                    );
                j = middle - 1
                    + crate::algorithms::merging::util::weakly_increasing_prefix_index(
                        &slice[middle - 1..right_run_begin],
                    );
            } else {
                if slice[middle - 1] <= slice[middle] {
                    i = left_run_end
                        + crate::algorithms::merging::util::weakly_increasing_suffix_index(
                            &slice[left_run_end..middle],
                        );
                    j = middle - 1
                        + crate::algorithms::merging::util::weakly_increasing_prefix_index(
                            &slice[middle - 1..right_run_begin],
                        );
                } else {
                    i = left_run_end
                        + crate::algorithms::merging::util::strictly_decreasing_suffix_index(
                            &slice[left_run_end..middle],
                        );
                    j = middle - 1
                        + crate::algorithms::merging::util::strictly_decreasing_prefix_index(
                            &slice[middle - 1..right_run_begin],
                        );
                    slice[i..j].reverse();
                }
            }

            // NOTE: is the j comparison necessary?
            if i == 0 && j == slice.len() {
                return;
            }

            if middle - i < j - middle {
                Self::peeksort(&mut slice[..i], buffer, left_run_end, i - 1);
                Self::peeksort(&mut slice[i..], buffer, j - i, right_run_begin - i);
                M::merge(slice, i, buffer);
            } else {
                Self::peeksort(&mut slice[..j], buffer, left_run_end, i);
                Self::peeksort(&mut slice[j..], buffer, 1, right_run_begin - j);
                M::merge(slice, j, buffer);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: usize = crate::test::DEFAULT_RUNS;
    const TEST_SIZE: usize = crate::test::DEFAULT_TEST_SIZE;

    /// Default peeksort but allowing decreasing runs
    type PeekSortDecreasing = PeekSort<
        DefaultInsertionSort,
        DefaultMergingMethod,
        DefaultBufGuardFactory,
        DEFAULT_INSERTION_THRESHOLD,
        false,
    >;

    #[test]
    fn empty() {
        crate::test::test_empty::<PeekSort>();
        crate::test::test_empty::<PeekSortDecreasing>();
    }

    #[test]
    fn random() {
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PeekSort>();
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, PeekSortDecreasing>();
    }

    #[test]
    fn random_stable() {
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PeekSort>();
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, PeekSortDecreasing>();
    }
}
