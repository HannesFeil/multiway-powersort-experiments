//! The peeksort implementation

/// The actual peek sort implementation
///
/// Sorts `slice` under the assumption, that `slice[..left_run_end]` and
/// `slice[right_run_begin..]` are already sorted.
fn peeksort_helper<
    T: Ord,
    M: super::merging::MergingMethod,
    const INSERTION_THRESHOLD: usize,
    const ONLY_INCREASING_RUNS: bool,
>(
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
        crate::algorithms::insertionsort::insertion_sort(slice);
        return;
    }

    let middle = slice.len() / 2;

    if middle <= left_run_end {
        peeksort_helper::<T, M, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>(
            &mut slice[left_run_end..],
            buffer,
            1,
            right_run_begin - left_run_end,
        );
        M::merge(slice, left_run_end, buffer);
    } else if middle >= right_run_begin {
        peeksort_helper::<T, M, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>(
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
                + crate::algorithms::merging::weakly_increasing_suffix_index(
                    &mut slice[left_run_end..middle],
                );
            j = middle - 1
                + crate::algorithms::merging::weakly_increasing_prefix_index(
                    &mut slice[middle - 1..right_run_begin],
                );
        } else {
            if slice[middle - 1] <= slice[middle] {
                i = left_run_end
                    + crate::algorithms::merging::weakly_increasing_suffix_index(
                        &mut slice[left_run_end..middle],
                    );
                j = middle - 1
                    + crate::algorithms::merging::weakly_increasing_prefix_index(
                        &mut slice[middle - 1..right_run_begin],
                    );
            } else {
                i = left_run_end
                    + crate::algorithms::merging::strictly_decreasing_suffix_index(
                        &mut slice[left_run_end..middle],
                    );
                j = middle - 1
                    + crate::algorithms::merging::strictly_decreasing_prefix_index(
                        &mut slice[middle - 1..right_run_begin],
                    );
                slice[i..j].reverse();
            }
        }

        // NOTE: is the j comparison necessary?
        if i == 0 && j == slice.len() {
            return;
        }

        if middle - i < j - middle {
            peeksort_helper::<T, M, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>(
                &mut slice[..i],
                buffer,
                left_run_end,
                i - 1,
            );
            peeksort_helper::<T, M, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>(
                &mut slice[i..],
                buffer,
                j - i,
                right_run_begin - i,
            );
            M::merge(slice, i, buffer);
        } else {
            peeksort_helper::<T, M, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>(
                &mut slice[..j],
                buffer,
                left_run_end,
                i,
            );
            peeksort_helper::<T, M, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>(
                &mut slice[j..],
                buffer,
                1,
                right_run_begin - j,
            );
            M::merge(slice, j, buffer);
        }
    }
}

/// Sort the slice using Peeksort, initializing a buffer once which is then used for merging
pub fn peeksort<
    T: Ord,
    M: super::merging::MergingMethod,
    B: super::merging::BufGuard<T>,
    const INSERTION_THRESHOLD: usize,
    const ONLY_INCREASING_RUNS: bool,
>(
    slice: &mut [T],
) {
    if slice.len() < 2 {
        return;
    }

    // Conservatively initiate a buffer big enough to merge the complete array
    let mut buffer = B::with_capacity(M::required_capacity(slice.len()));

    // Delegate to helper function
    peeksort_helper::<T, M, INSERTION_THRESHOLD, ONLY_INCREASING_RUNS>(
        slice,
        buffer.as_uninit_slice_mut(),
        1,
        slice.len() - 1,
    );
}

/// Peeksort the given slice, with the following default const parameters
///
/// - `M = CopyBoth`
/// - `B = Vec<T>`
/// - `INSERTION_THRESHOLD = 24`
/// - `ONLY_INCREASING_RUNS = true`
pub fn default_peeksort<T: Ord>(slice: &mut [T]) {
    peeksort::<T, super::merging::CopyBoth, Vec<T>, 24, true>(slice);
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: usize = 100;
    const TEST_SIZE: usize = 100_000;

    #[test]
    fn empty() {
        crate::test::test_empty(default_peeksort);
    }

    #[test]
    fn random() {
        crate::test::test_random_sorted::<RUNS, TEST_SIZE>(default_peeksort);
    }

    #[test]
    fn random_stable() {
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE>(default_peeksort);
    }
}
