//! The Peeksort implementation

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

/// The Peeksort [`super::Sort`].
///
/// - `I` is the insertion sort used for small slices.
/// - `M` is the [`super::merging::two_way::MergingMethod`] used to merge the runs.
/// - `B` is the [`super::BufGuardFactory`] used to create the buffer for merging.
/// - `INSERTION_THRESHOLD` determines the maximum length for sub slices sorted with insertion sort.
/// - `ONLY_INCREASING_RUNS` indicates whether only increasing existing runs are used.
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
> super::Sort for PeekSort<I, M, B, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>
{
    const IS_STABLE: bool = I::IS_STABLE && M::IS_STABLE;

    const BASE_NAME: &str = "peeksort";

    fn parameters() -> impl Iterator<Item = (&'static str, String)> {
        vec![
            ("i-sort", crate::cli::display_inline::<I>()),
            ("merging", M::display()),
            ("i-threshold", INSERTION_THRESHOLD.to_string()),
            ("only-increasing", ONLY_INCREASING_RUNS.to_string()),
        ]
        .into_iter()
    }

    fn sort<T: Ord>(slice: &mut [T]) {
        <Self as super::PostfixSort>::sort_with_sorted_prefix(slice, 1);
    }
}

impl<
    I: super::Sort,
    M: super::merging::two_way::MergingMethod,
    B: super::BufGuardFactory,
    const INSERTION_THRESHOLD: usize,
    const ONLY_INCREASING_RUNS: bool,
> super::PostfixSort for PeekSort<I, M, B, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>
{
    fn sort_with_sorted_prefix<T: Ord>(slice: &mut [T], split_point: usize) {
        if slice.len() < 2 {
            return;
        }

        // Conservatively initiate a buffer big enough to merge the complete array
        let mut buffer = <B::Guard<T>>::with_capacity(M::required_capacity(slice.len()));

        // Delegate to helper function
        Self::peeksort(
            slice,
            split_point,
            slice.len() - 1,
            buffer.as_uninit_slice_mut(),
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
    /// The actual peek sort implementation.
    ///
    /// Sorts `slice` under the assumption, that `slice[..left_run_end]` and
    /// `slice[right_run_begin..]` are already sorted.
    fn peeksort<T: Ord>(
        slice: &mut [T],
        left_run_end: usize,
        right_run_begin: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
    ) {
        // Assert invariant in debug build
        debug_assert!(slice[..left_run_end].is_sorted() && slice[right_run_begin..].is_sorted());

        // Assert minimum and maximum run lengths
        assert!((1..=slice.len()).contains(&left_run_end));
        assert!((0..slice.len()).contains(&right_run_begin));

        // Slice is already sorted, nothing to do
        if left_run_end > right_run_begin {
            return;
        }

        // Use insertion sort for small slices
        if slice.len() < INSERTION_THRESHOLD {
            I::sort(slice);
            return;
        }

        let middle = slice.len() / 2;

        if middle <= left_run_end {
            // left run extends further than middle => sort rest and merge
            Self::peeksort(
                &mut slice[left_run_end..],
                1,                              // Left run always at least one element long
                right_run_begin - left_run_end, // Shift index since we cut right_run_begin elements
                buffer,
            );
            M::merge(slice, left_run_end, buffer);
        } else if middle >= right_run_begin {
            // right run extends further than middle => sort beginning and merge
            Self::peeksort(
                &mut slice[..right_run_begin],
                left_run_end,        // Left run at the beginning persists
                right_run_begin - 1, // Right run is always at least one element long
                buffer,
            );
            M::merge(slice, right_run_begin, buffer);
        } else {
            // Find the longest run containing `middle - 1`
            let (middle_run_start, middle_run_end);

            #[allow(
                clippy::collapsible_else_if,
                reason = "Clearer distinction between compile time and runtime checks"
            )]
            if ONLY_INCREASING_RUNS {
                middle_run_start = left_run_end
                    + crate::algorithms::merging::util::weakly_increasing_suffix_index(
                        &slice[left_run_end..middle],
                    );
                middle_run_end = middle - 1
                    + crate::algorithms::merging::util::weakly_increasing_prefix_index(
                        &slice[middle - 1..right_run_begin],
                    );
            } else {
                if slice[middle - 1] <= slice[middle] {
                    middle_run_start = left_run_end
                        + crate::algorithms::merging::util::weakly_increasing_suffix_index(
                            &slice[left_run_end..middle],
                        );
                    middle_run_end = middle - 1
                        + crate::algorithms::merging::util::weakly_increasing_prefix_index(
                            &slice[middle - 1..right_run_begin],
                        );
                } else {
                    middle_run_start = left_run_end
                        + crate::algorithms::merging::util::strictly_decreasing_suffix_index(
                            &slice[left_run_end..middle],
                        );
                    middle_run_end = middle - 1
                        + crate::algorithms::merging::util::strictly_decreasing_prefix_index(
                            &slice[middle - 1..right_run_begin],
                        );
                    slice[middle_run_start..middle_run_end].reverse();
                }
            }

            // Recurse mostly halfway, eating up the run in the middle with one half
            if middle - middle_run_start < middle_run_end - middle {
                // Middle run extends mostly into the right half
                Self::peeksort(
                    &mut slice[..middle_run_start],
                    left_run_end,         // Left run stays the same
                    middle_run_start - 1, // Right run with at least length 1
                    buffer,
                );
                Self::peeksort(
                    &mut slice[middle_run_start..],
                    middle_run_end - middle_run_start, // Middle run becomes left run
                    right_run_begin - middle_run_start, // End run stays the same
                    buffer,
                );
                M::merge(slice, middle_run_start, buffer);
            } else {
                // Middle run extends mostly into the left half
                Self::peeksort(
                    &mut slice[..middle_run_end],
                    left_run_end,     // Left run stays the same
                    middle_run_start, // Middle run becomes right run
                    buffer,
                );
                Self::peeksort(
                    &mut slice[middle_run_end..],
                    1,                                // Left run always has at least one element
                    right_run_begin - middle_run_end, // Right run stays the same
                    buffer,
                );
                M::merge(slice, middle_run_end, buffer);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: usize = crate::test::DEFAULT_RUNS;
    const TEST_SIZE: usize = crate::test::DEFAULT_TEST_SIZE;

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
