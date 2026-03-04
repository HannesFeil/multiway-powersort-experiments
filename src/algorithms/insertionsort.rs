//! Multiple insertion sort implementations

/// The default `BINARY` parameter for `InsertionSort`
pub const DEFAULT_BINARY: bool = false;

/// The insertion [`super::Sort`]
///
/// - `BINARY` indicates whether to use binary search for the insertion.
pub struct InsertionSort<const BINARY: bool = DEFAULT_BINARY>;

impl<const BINARY: bool> super::Sort for InsertionSort<BINARY> {
    const IS_STABLE: bool = true;

    const BASE_NAME: &str = "insertionsort";

    fn parameters() -> impl Iterator<Item = (&'static str, String)> {
        vec![("binary", BINARY.to_string())].into_iter()
    }

    fn sort<T: Ord>(slice: &mut [T]) {
        <Self as super::PostfixSort>::sort_with_sorted_prefix(slice, 1);
    }
}

impl<const BINARY: bool> super::PostfixSort for InsertionSort<BINARY> {
    fn sort_with_sorted_prefix<T: Ord>(slice: &mut [T], split_point: usize) {
        if slice.len() < 2 {
            return;
        }

        if BINARY {
            Self::binary_insertion_sort_with_partition(slice, split_point);
        } else {
            Self::insertion_sort_with_partition(slice, split_point);
        }
    }
}

impl<const BINARY: bool> InsertionSort<BINARY> {
    /// Sorts slice using insertion sort, assuming that `slice[0..partition]` is already in order
    fn insertion_sort_with_partition<T: Ord>(slice: &mut [T], partition_point: usize) {
        assert!(
            (0..=slice.len()).contains(&partition_point),
            "Partition point needs to be in bounds"
        );
        debug_assert!(slice[..partition_point].is_sorted());

        for i in partition_point..slice.len() {
            for j in (0..i).rev() {
                if slice[j + 1] < slice[j] {
                    // NOTE: Swapping here seems to have no strong performance implications as
                    // opposed to 'rotating', especially since the general case has so few elements
                    slice.swap(j + 1, j);
                } else {
                    break;
                }
            }
        }
    }

    /// Sorts slice using binary insertion sort, assuming that `slice[0..partition]` is already in
    /// order.
    fn binary_insertion_sort_with_partition<T: Ord>(slice: &mut [T], partition_point: usize) {
        assert!(
            (0..=slice.len()).contains(&partition_point),
            "Partition point needs to be in bounds"
        );
        debug_assert!(slice[..partition_point].is_sorted());

        for i in partition_point..slice.len() {
            let j = slice[..i].partition_point(|x| x <= &slice[i]);

            slice[j..=i].rotate_right(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: usize = 100;
    const TEST_SIZE: usize = 100;

    #[test]
    pub fn empty() {
        crate::test::test_empty::<InsertionSort>();
        crate::test::test_empty::<InsertionSort<true>>();
    }

    #[test]
    pub fn random() {
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, InsertionSort>();
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, InsertionSort<true>>();
    }

    #[test]
    pub fn random_stable() {
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, InsertionSort>();
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, InsertionSort<true>>();
    }
}
