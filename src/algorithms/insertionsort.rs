//! Multiple insertion sort implementations

// TODO: consider working with pointers/unsafe? of course there are a lot of in bounds checks here

/// The default `BINARY` parameter for `InsertionSort`
pub const DEFAULT_BINARY: bool = false;

/// The insertion [`super::Sort`]
pub struct InsertionSort<const BINARY: bool = DEFAULT_BINARY>;

impl<const BINARY: bool> super::Sort for InsertionSort<BINARY> {
    const IS_STABLE: bool = true;

    fn sort<T: Ord>(slice: &mut [T]) {
        if slice.len() < 2 {
            return;
        }

        if BINARY {
            insertion_sort_with_partition(slice, 1);
        } else {
            binary_insertion_sort_with_partition(slice, 1);
        }
    }
}

/// Sort slice using insertion sort, assuming that `slice[0..partition]` is already in order
fn insertion_sort_with_partition<T: Ord>(slice: &mut [T], partition_point: usize) {
    assert!(
        (0..slice.len()).contains(&partition_point),
        "Partition point needs to be in bounds"
    );

    for i in partition_point..slice.len() {
        for j in (0..i).rev() {
            if slice[j + 1] < slice[j] {
                // TODO: swapping is easiest, otherwise I'd have to work with unsafe I think
                slice.swap(j + 1, j);
            } else {
                break;
            }
        }
    }
}

/// Sort slice using binary insertion sort, assuming that `slice[0..partition]` is already in order
fn binary_insertion_sort_with_partition<T: Ord>(slice: &mut [T], partition_point: usize) {
    assert!(
        (0..slice.len()).contains(&partition_point),
        "Partition point needs to be in bounds"
    );

    for i in partition_point..slice.len() {
        let mut j = slice[0..i]
            .binary_search(&slice[i])
            .unwrap_or_else(|index| index);
        // Necessary for stability, TODO: is this correct w.r. c++ impl?
        while j < i && slice[j] == slice[i] {
            j += 1;
        }

        for p in (j..i).rev() {
            slice.swap(p, p + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: usize = 100;
    const TEST_SIZE: usize = 1000;

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
