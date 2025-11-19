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

pub trait MergingMethod {
    fn merge<T: Ord, B: BufGuard<T>>(
        slice: &mut [T],
        split_point: usize,
        buffer: &mut [std::mem::MaybeUninit<T>],
    );
}

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

        // SAFETY: We know slice and buffer are distinct and just checked then length of buffer.
        // Since T and MaybeUninit<T> are guaranteed to have the same size, alignment and abi
        // this copy is sound.
        unsafe {
            std::ptr::copy_nonoverlapping(
                slice.as_ptr(),
                buffer.as_mut_ptr() as *mut T,
                slice.len(),
            );
        }

        let mut output = slice.as_mut_ptr();
        let (left, right) = buffer.split_at(split_point);
        let std::ops::Range {
            start: mut left_start,
            end: left_end,
        } = left.as_ptr_range();
        let std::ops::Range {
            start: mut right_start,
            end: right_end,
        } = right.as_ptr_range();

        // SAFETY: All pointers are kept in bounds of their respective range and each element from
        // buffer ends up being copied to slice exactly once
        unsafe {
            while left_start != left_end && right_start != right_end {
                if (*left_start).assume_init_ref() < (*right_start).assume_init_ref() {
                    output.copy_from_nonoverlapping(left_start as *const T, 1);
                    left_start = left_start.add(1);
                } else {
                    output.copy_from_nonoverlapping(right_start as *const T, 1);
                    right_start = right_start.add(1);
                }

                output = output.add(1);
            }
            while left_start < left_end {
                output.copy_from_nonoverlapping(left_start as *const T, 1);
                left_start = left_start.add(1);
                output = output.add(1);
            }
            while right_start < right_end {
                output.copy_from_nonoverlapping(right_start as *const T, 1);
                right_start = right_start.add(1);
                output = output.add(1);
            }
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
