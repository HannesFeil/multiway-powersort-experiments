//! Defines methods to merge two adjacent runs in a slice, see [`MergingMethod`].

/// Specifies ways to merge two adjacent runs in a slice, given a buffer.
pub trait MergingMethod {
    /// Whether the merging method is stable.
    const IS_STABLE: bool;

    /// Returns the string representation of this merging method.
    fn display() -> String;

    /// Merges the two sorted runs `slice[0..run_length]` and `slice[run_length..slice.len()]`,
    /// potentially using `buffer`.
    ///
    /// `buffer.len()` should be greater or equal to `Self::required_capacity(slice.len())`.
    fn merge<T: Ord>(slice: &mut [T], run_length: usize, buffer: &mut [std::mem::MaybeUninit<T>]);

    /// The required capacity of the buffer, needed for merging slices with length less than
    /// or equal to `size`.
    fn required_capacity(size: usize) -> usize {
        size
    }
}

/// A [`MergingMethod`] that copies all elements into `buffer` and does a simple merge back.
#[derive(Debug, Clone, Copy)]
pub struct CopyBoth;

impl MergingMethod for CopyBoth {
    const IS_STABLE: bool = true;

    fn display() -> String {
        "copy-both".to_string()
    }

    fn merge<T: Ord>(slice: &mut [T], run_length: usize, buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.is_empty() {
            return;
        }

        #[cfg(feature = "counters")]
        #[expect(
            clippy::as_conversions,
            reason = "slice.len() will realistically stay way below u64::MAX, so this is lossless"
        )]
        {
            crate::GLOBAL_COUNTERS
                .merge_slice
                .increase(slice.len() as u64);
            crate::GLOBAL_COUNTERS
                .merge_buffer
                .increase(slice.len() as u64);
        }

        assert!(
            buffer.len() >= slice.len(),
            "Buffer needs to have at least the size of slice"
        );
        assert!(
            (0..slice.len()).contains(&run_length),
            "run_lengths needs to be less than or equal to slice.len()"
        );

        let buffer = &mut buffer[..slice.len()];

        // SAFETY: We copy each element into buffer and back exactly once, such that slice ends up
        // permuted. Since we have exclusive access to slice and buffer, the constructed pointer
        // ranges are safe to read from and write to.
        unsafe {
            // Copy entire slice into buffer
            std::ptr::copy_nonoverlapping(slice.as_ptr(), buffer.as_mut_ptr().cast(), slice.len());

            // Construct the runs.
            // These are safe to assume init since we just copied over the elements.
            let ptr_range = buffer.as_mut_ptr_range();
            let runs = [
                super::Run(ptr_range.start..ptr_range.start.add(run_length)).assume_init(),
                super::Run(ptr_range.start.add(run_length)..ptr_range.end).assume_init(),
            ];

            // Construct the `output` run
            let output = super::Run(slice.as_mut_ptr_range());

            // All runs and output are valid by construction.
            // This makes sure each element in `buffer` gets copied back, even if a comparison
            // panics.
            let mut guard = super::MergingDropGuard::new(runs, output);

            // Destructure bindings for easier access, these are only references and
            // guard is still responsible for cleaning up.
            let &mut [ref mut left, ref mut right] = &mut guard.runs;
            let output = &mut guard.output;

            // Repeatedly copy the smaller element of both runs into the slice
            while !left.is_empty() && !right.is_empty() {
                if *left.start() <= *right.start() {
                    left.copy_nonoverlapping_prefix_to(output, 1);
                } else {
                    right.copy_nonoverlapping_prefix_to(output, 1);
                }
            }

            // Copy the rest of the remaining runs into the slice
            if !left.is_empty() {
                left.copy_nonoverlapping_prefix_to(output, left.len());
            }
            if !right.is_empty() {
                right.copy_nonoverlapping_prefix_to(output, right.len());
            }

            debug_assert!(guard.is_empty());

            // We are done at this point, so disarm the guard
            guard.disarm();
        }
    }
}

/// A [`MergingMethod`] that utilizes a galloping strategy taken from Timsort.
#[derive(Debug, Clone, Copy)]
pub struct Galloping<const MIN_GALLOP: usize = 7>;

impl<const MIN_GALLOP: usize> MergingMethod for Galloping<MIN_GALLOP> {
    const IS_STABLE: bool = true;

    fn display() -> String {
        format!("galloping (MIN_GALLOP = {MIN_GALLOP})")
    }

    fn merge<T: Ord>(slice: &mut [T], run_length: usize, buffer: &mut [std::mem::MaybeUninit<T>]) {
        if slice.len() < 2 || run_length == 0 {
            return;
        }

        // Gallop right to exclude elements from the left run that are smaller than all from the
        // right run.
        let start = Self::gallop::<T, false>(&slice[run_length], &slice[..run_length], 0);
        if start == run_length {
            return;
        }

        // Gallop left to exclude elements from the right run that are larger than all from the
        // left run.
        let end = Self::gallop::<T, true>(
            &slice[run_length - 1],
            &slice[run_length..],
            slice.len() - run_length - 1,
        ) + run_length;
        if end == run_length {
            return;
        }

        let mut min_gallop = MIN_GALLOP;

        // Merge depending on the smaller run
        if run_length - start <= end - run_length {
            Self::merge_low(
                &mut slice[start..end],
                run_length - start,
                buffer,
                &mut min_gallop,
            );
        } else {
            Self::merge_high(
                &mut slice[start..end],
                run_length - start,
                buffer,
                &mut min_gallop,
            );
        }
    }
}

impl<const MIN_GALLOP: usize> Galloping<MIN_GALLOP> {
    /// Returns the index `i` such that after inserting `key` between index at `i`, `slice` is
    /// still sorted. Assumes `slice` is sorted.
    ///
    /// The starting point `hint` indicates from where to start galloping.
    ///
    /// `BEFORE_EQUAL` determines if `i` is chosen before equal elements and otherwise after them.
    fn gallop<T: Ord, const BEFORE_EQUAL: bool>(key: &T, slice: &[T], hint: usize) -> usize {
        debug_assert!(slice.is_sorted());
        assert!((0..slice.len()).contains(&hint));

        let mut previous_offset = 0;
        let mut offset = 1;

        // Determine comparison functions depending on galloping direction
        type Comparator<T> = fn(&T, &T) -> bool;
        let (should_insert_past, should_not_insert_past): (Comparator<T>, Comparator<T>) =
            if BEFORE_EQUAL {
                (T::gt, T::le)
            } else {
                (T::ge, T::lt)
            };

        // Check if we're searching `slice[..hint]` or `slice[hint..]`
        if should_insert_past(key, &slice[hint]) {
            // Use quadratic search to find the containing interval
            let max_offset = slice.len() - hint;
            while offset < max_offset && should_insert_past(key, &slice[hint + offset]) {
                previous_offset = offset;
                offset = (offset << 1) + 1;
            }
            // Invariants:
            // - insert after hint + previous_offset
            // - insert before or at hint + offset

            offset = std::cmp::min(offset, max_offset);

            // Since we searched `slice[hint..]` we have to adjust the indices
            previous_offset += hint + 1; // + 1 since we insert after hint + previous_offset
            offset += hint;
        } else {
            // Use quadratic search to find the containing interval
            let max_offset = hint + 1;
            while offset < max_offset && should_not_insert_past(key, &slice[hint - offset]) {
                previous_offset = offset;
                offset = (offset << 1) + 1;
            }
            // Invariants:
            // - insert before or at hint - previous_offset
            // - insert after hint - offset

            offset = std::cmp::min(offset, max_offset);

            // Since we searched `slice[..hint]` backwards, we reverse the offsets
            let tmp = previous_offset;
            previous_offset = hint + 1 - offset; // + 1 since we insert after hint - offset
            offset = hint - tmp; // No + 1 since we know we don't insert after hint - previous_offset
        }
        assert!(previous_offset <= offset && offset <= slice.len());

        // Perform binary search in the found interval
        let result = slice[previous_offset..offset].partition_point(|x| should_insert_past(key, x))
            + previous_offset;

        debug_assert_eq!(
            result,
            slice.partition_point(|x| should_insert_past(key, x)),
        );

        result
    }

    /// Sort the given `slice` assuming `slice[..run_length]` and `slice[run_length..]` are
    /// already sorted and `run_length < slice.len() - run_length` and `slice[0] > slice[run_length]`.
    fn merge_low<T: Ord>(
        slice: &mut [T],
        run_length: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
        min_gallop: &mut usize,
    ) {
        assert!(
            buffer.len() >= run_length,
            "We need at least run_length buffer size"
        );
        assert!(
            (1..slice.len()).contains(&run_length),
            "Split point has to be within slice bounds"
        );

        #[cfg(feature = "counters")]
        #[expect(
            clippy::as_conversions,
            reason = "slice.len() will realistically stay way below u64::MAX, so this is lossless"
        )]
        {
            crate::GLOBAL_COUNTERS
                .merge_slice
                .increase(slice.len() as u64);
            crate::GLOBAL_COUNTERS
                .merge_buffer
                .increase(run_length as u64);
        }

        // Set buffer size
        let buffer = &mut buffer[..run_length];

        // SAFETY: all runs are valid by construction and we keep invariants about neither run
        // being empty before copying from them.
        unsafe {
            // Copy `slice[..run_length]` into temporary buffer
            std::ptr::copy_nonoverlapping(
                slice.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                run_length,
            );

            // Construct runs
            let slice_ptrs = slice.as_mut_ptr_range();
            let runs = [
                // Left run in buffer (we just initialized it)
                super::Run(buffer.as_mut_ptr_range()).assume_init(),
                // Right run at the end of slice
                super::Run(slice_ptrs.start.add(run_length)..slice_ptrs.end),
            ];

            // The output run
            // NOTE: Since `output` and `right` overlap, make sure to use the right copying method
            let output = super::Run(slice_ptrs);

            // This guard makes sure all elements get written back into `output` on panic
            let mut guard = super::MergingDropGuard::new(runs, output);

            // References for easier access, guard still owns the runs
            let &mut [ref mut left, ref mut right] = &mut guard.runs;
            let output = &mut guard.output;

            // Use a closure to allow early break out of block
            (move || {
                // Copy the first element from `right` into `output` since it's the smallest
                right.copy_nonoverlapping_prefix_to(output, 1);

                // Right side only had one element, only need to copy the left side
                if right.is_empty() {
                    left.copy_nonoverlapping_prefix_to(output, left.len());

                    return;
                }

                // Left side only has one element, copy the rest of the right side and then the one
                // element from the left side
                if left.len() == 1 {
                    right.copy_prefix_to(output, right.len());
                    left.copy_nonoverlapping_prefix_to(output, 1);

                    return;
                }

                // Continuously copy elements until `left.len() == 1` or `right.is_empty()`
                'outer: loop {
                    let mut count1 = 0;
                    let mut count2 = 0;

                    // Merge one by one until threshold for bulk merging is reached
                    while (count1 | count2) < *min_gallop {
                        assert!(left.len() > 1);
                        assert!(!right.is_empty());

                        if *right.start() < *left.start() {
                            // Advance the right side
                            right.copy_nonoverlapping_prefix_to(output, 1);
                            count2 += 1;
                            count1 = 0;

                            if right.is_empty() {
                                break 'outer;
                            }
                        } else {
                            // Advance the left side
                            left.copy_nonoverlapping_prefix_to(output, 1);
                            count1 += 1;
                            count2 = 0;

                            if left.len() == 1 {
                                break 'outer;
                            }
                        }
                    }

                    // Gallop and merge multiple until it's no longer worth it
                    loop {
                        assert!(left.len() > 1);
                        assert!(!right.is_empty());

                        // Gallop right to find how many left elements are smaller than right
                        count1 = Self::gallop::<T, false>(&*right.start(), left.as_slice(), 0);
                        if count1 != 0 {
                            // Copy the elements
                            left.copy_nonoverlapping_prefix_to(output, count1);

                            if left.len() <= 1 {
                                break 'outer;
                            }
                        }

                        // Right element must be lowest at this point and we know right is not empty
                        right.copy_nonoverlapping_prefix_to(output, 1);

                        if right.is_empty() {
                            break 'outer;
                        }

                        // Gallop left to find how many right elements are smaller than right
                        count2 = Self::gallop::<T, true>(&*left.start(), right.as_slice(), 0);
                        if count2 != 0 {
                            // Copy the elements
                            right.copy_prefix_to(output, count2);

                            if right.is_empty() {
                                break 'outer;
                            }
                        }

                        // Left element must be lowest at this point and we know left is not empty
                        left.copy_nonoverlapping_prefix_to(output, 1);

                        if left.len() == 1 {
                            break 'outer;
                        }

                        // Lower threshold for starting bulk merging
                        *min_gallop = min_gallop.saturating_sub(1);

                        if count1 < MIN_GALLOP && count2 < MIN_GALLOP {
                            break;
                        }
                    }

                    // Increase threshold for starting bulk merging
                    *min_gallop += 2;
                }

                // Loop end is reach so either `left.len() == 1` or `right.is_empty()`
                if left.len() == 1 {
                    assert!(!right.is_empty());
                    // Copy the remaining elements from right
                    right.copy_prefix_to(output, right.len());
                    // Copy the last element from left
                    left.copy_nonoverlapping_prefix_to(output, 1);
                } else {
                    assert!(!left.is_empty());
                    assert!(right.is_empty());
                    // Right is empty so just copy over left
                    left.copy_nonoverlapping_prefix_to(output, left.len());
                }
            })();

            // Guard should be empty at this point
            debug_assert!(guard.is_empty());

            // We are done merging so disarm the guard
            guard.disarm();
        }
    }

    /// Sort the given `slice` assuming `slice[..run_length]` and `slice[run_length..]` are
    /// already sorted and `slice.len() - run_length <= run_length` and `slice[run_length - 1] > slice[slice.len() - 1]`.
    fn merge_high<T: Ord>(
        slice: &mut [T],
        run_length: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
        min_gallop: &mut usize,
    ) {
        assert!(
            buffer.len() >= slice.len() - run_length,
            "We need at least slice.len() - run_length buffer size"
        );
        assert!(
            (1..slice.len()).contains(&run_length),
            "Split point has to be within slice bounds"
        );

        #[cfg(feature = "counters")]
        #[expect(
            clippy::as_conversions,
            reason = "slice.len() will realistically stay way below u64::MAX, so this is lossless"
        )]
        {
            crate::GLOBAL_COUNTERS
                .merge_slice
                .increase(slice.len() as u64);
            crate::GLOBAL_COUNTERS
                .merge_buffer
                .increase((slice.len() - run_length) as u64);
        }

        // Set buffer size
        let buffer = &mut buffer[..slice.len() - run_length];

        // SAFETY: all runs are valid by construction and we keep invariants about neither run
        // being empty before copying from them.
        unsafe {
            // Copy suffix into temporary buffer
            std::ptr::copy_nonoverlapping(
                slice.as_mut_ptr().add(run_length),
                buffer.as_mut_ptr().cast(),
                slice.len() - run_length,
            );

            // Construct runs
            let slice_ptrs = slice.as_mut_ptr_range();
            let runs = [
                // Left run at the start of the slice
                super::Run(slice_ptrs.start..slice_ptrs.start.add(run_length)),
                // Right run in buffer (we just initialized it)
                super::Run(buffer.as_mut_ptr_range()).assume_init(),
            ];
            // Output run
            // NOTE: This run overlaps with right run so be careful when copying elements
            let output = super::Run(slice_ptrs);

            // This guard makes sure all elements get written back into `output` on panic
            let mut guard = super::MergingDropGuard::new(runs, output);

            // References for easier access, guard still owns the runs
            let &mut [ref mut left, ref mut right] = &mut guard.runs;
            let output = &mut guard.output;

            // NOTE: We are merging into slice backwards
            // Use a closure to allow early break out of block
            (|| {
                // Copy the first element from `left` into `output` since it's the smallest
                left.copy_nonoverlapping_suffix_to(output, 1);

                // Left side only had one element, only need to copy the right side
                if left.is_empty() {
                    right.copy_nonoverlapping_suffix_to(output, right.len());

                    return;
                }

                // right side only has one element, copy the rest of the left side and then the one
                // element from the right side
                if right.len() == 1 {
                    left.copy_suffix_to(output, left.len());
                    right.copy_nonoverlapping_suffix_to(output, 1);

                    return;
                }

                // Loop until `right.len() == 1` or `left.is_empty()`
                'outer: loop {
                    let mut count1 = 0;
                    let mut count2 = 0;

                    // Merge one by one until threshold for bulk merging is reached
                    while (count1 | count2) < *min_gallop {
                        assert!(right.len() > 1);
                        assert!(!left.is_empty());

                        if *right.end().sub(1) < *left.end().sub(1) {
                            // Advance the left side
                            left.copy_nonoverlapping_suffix_to(output, 1);
                            count1 += 1;
                            count2 = 0;

                            if left.is_empty() {
                                break 'outer;
                            }
                        } else {
                            // Advance the right side
                            right.copy_nonoverlapping_suffix_to(output, 1);
                            count1 = 0;
                            count2 += 1;

                            if right.len() == 1 {
                                break 'outer;
                            }
                        }
                    }

                    // Gallop and merge multiple until it's no longer worth it
                    loop {
                        assert!(right.len() > 1);
                        assert!(!left.is_empty());

                        // Gallop right to find how many left elements are larger than right
                        let left_len = left.len();
                        count1 = left.len()
                            - Self::gallop::<T, false>(
                                &*right.end().sub(1),
                                left.as_slice(),
                                left_len - 1,
                            );
                        if count1 != 0 {
                            // Copy the elements
                            left.copy_suffix_to(output, count1);

                            if left.is_empty() {
                                break 'outer;
                            }
                        }

                        // Right now has the largest element and we know it's not empty
                        right.copy_nonoverlapping_suffix_to(output, 1);

                        if right.len() == 1 {
                            break 'outer;
                        }

                        // Gallop left to find how many right elements are larger than left
                        let right_len = right.len();
                        count2 = right.len()
                            - Self::gallop::<T, true>(
                                &*left.end().sub(1),
                                right.as_slice(),
                                right_len - 1,
                            );
                        if count2 != 0 {
                            // Copy the elements
                            right.copy_nonoverlapping_suffix_to(output, count2);

                            if right.len() <= 1 {
                                break 'outer;
                            }
                        }

                        // Left now has the largest element and we know it's not empty
                        left.copy_nonoverlapping_suffix_to(output, 1);

                        if left.is_empty() {
                            break 'outer;
                        }

                        // Lower threshold for starting bulk merging
                        *min_gallop = min_gallop.saturating_sub(1);

                        if count1 < MIN_GALLOP && count2 < MIN_GALLOP {
                            break;
                        }
                    }

                    // Increase threshold for starting bulk merging
                    *min_gallop += 2;
                }

                // Loop end is reach so either `right.len() == 1` or `left.is_empty()`
                if right.len() == 1 {
                    assert!(!left.is_empty());
                    // Copy the remaining elements from left
                    left.copy_suffix_to(output, left.len());
                    // Copy the last element from right
                    right.copy_nonoverlapping_suffix_to(output, 1);
                } else {
                    assert!(!right.is_empty());
                    assert!(left.is_empty());
                    // Left is empty so just copy over right
                    right.copy_nonoverlapping_suffix_to(output, right.len());
                }
            })();

            // Guard should be empty at this point
            debug_assert!(guard.is_empty());

            // We are done merging so disarm the guard
            guard.disarm();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_methods {
        (
            $method:ty
        ) => {
            #[test]
            fn test_empty_merges() {
                crate::test::merging::test_empty_merge::<$method, 2>();
            }

            #[test]
            fn test_correct_merges() {
                crate::test::merging::test_correct_merge::<$method, 2>();
            }

            #[test]
            fn test_correct_stable_merges() {
                crate::test::merging::test_correct_stable_merge::<$method, 2>();
            }

            #[test]
            fn test_soundness_merges() {
                crate::test::merging::test_soundness_merge::<$method, 2>();
            }
        };
    }

    mod copy_both {
        test_methods!(super::CopyBoth);
    }

    mod galloping {
        test_methods!(super::Galloping);
    }
}
