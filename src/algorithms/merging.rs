/// Copied from [`std::slice::sort::stable::BufGuard<T>`]
pub trait BufGuard<T> {
    /// Creates new buffer that holds at least `capacity` memory.
    fn with_capacity(capacity: usize) -> Self;
    /// Returns mutable access to uninitialized memory owned by the buffer.
    fn as_uninit_slice_mut(&mut self) -> &mut [std::mem::MaybeUninit<T>];
}

impl<T> BufGuard<T> for Vec<T> {
    fn with_capacity(capacity: usize) -> Self {
        Vec::with_capacity(capacity)
    }

    fn as_uninit_slice_mut(&mut self) -> &mut [std::mem::MaybeUninit<T>] {
        self.spare_capacity_mut()
    }
}

/// Specifies ways to merge two adjacent runs in a slice, given a buffer
pub trait MergingMethod {
    /// Merge the two sorted runs `0..split_point` and `split_point..slice.len()`, potentially
    /// using `buffer`.
    fn merge<T: Ord, B: BufGuard<T>>(
        slice: &mut [T],
        split_point: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
    );
}

/// A [`MergingMethod`] implementation via a simple merging procedure
///
/// The `buffer` given in [`Self::merge`] has to have at least the same
/// size as the `slice`.
#[derive(Debug, Clone, Copy)]
pub struct CopyBoth;

impl MergingMethod for CopyBoth {
    fn merge<T: Ord, B: BufGuard<T>>(
        slice: &mut [T],
        split_point: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
    ) {
        assert!(
            buffer.len() >= slice.len(),
            "Buffer needs to have at least the size of slice"
        );
        assert!(
            (0..slice.len()).contains(&split_point),
            "Split points needs to be in bounds"
        );

        let mut output = buffer.as_mut_ptr();
        let (left, right) = slice.split_at(split_point);
        let std::ops::Range {
            start: mut left_start,
            end: left_end,
        } = left.as_ptr_range();
        let std::ops::Range {
            start: mut right_start,
            end: right_end,
        } = right.as_ptr_range();

        // NOTE: We copy after the merging as opposed to before, to prevent inconsistent
        // state which could occur when panicking on merging into slice

        // SAFETY: All pointers from slice are kept in bounds of their respective range.
        // Since it is assumed that slice.len() <= buffer.len() and in total slice.len()
        // elements are written into buffer one by one, these accesses are guaranteed to be
        // in bounds as well. The writing is valid since MaybeUninit<T> has the same layout,
        // size and ABI as as T and elements in [T] are guaranteed to be laid out sequentially
        // in memory (see https://doc.rust-lang.org/reference/type-layout.html#slice-layout)).
        //
        // Additionally each element is written into buffer exactly once,
        // so that buffer ends up as a permutation of slice.
        unsafe {
            // Repeatedly copy the smaller element of both runs into the buffer
            while left_start != left_end && right_start != right_end {
                if *left_start < *right_start {
                    output
                        .copy_from_nonoverlapping(left_start as *const std::mem::MaybeUninit<T>, 1);
                    left_start = left_start.add(1);
                } else {
                    output.copy_from_nonoverlapping(
                        right_start as *const std::mem::MaybeUninit<T>,
                        1,
                    );
                    right_start = right_start.add(1);
                }

                output = output.add(1);
            }

            // Copy the rest of the remaining run into the buffer
            while left_start < left_end {
                output.copy_from_nonoverlapping(left_start as *const std::mem::MaybeUninit<T>, 1);
                left_start = left_start.add(1);
                output = output.add(1);
            }
            while right_start < right_end {
                output.copy_from_nonoverlapping(right_start as *const std::mem::MaybeUninit<T>, 1);
                right_start = right_start.add(1);
                output = output.add(1);
            }
        }

        // SAFETY: Since buffer now contains a permutation of slice, we can safely copy it over to
        // slice, again regarding the same layout invariant for T and MaybeUninit<T>. (see above)
        unsafe {
            std::ptr::copy_nonoverlapping(
                buffer.as_ptr() as *mut T,
                slice.as_mut_ptr(),
                slice.len(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correct_merge() {
        let mut elements: [u32; 10] = [1, 4, 5, 8, 9, 0, 2, 3, 6, 7];
        let mut buffer = <Vec<_> as BufGuard<_>>::with_capacity(elements.len());
        CopyBoth::merge::<_, Vec<_>>(&mut elements, 5, buffer.as_uninit_slice_mut());
        assert!(elements.is_sorted());
    }
}
