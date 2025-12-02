//! The quicksort implementation

/// The default [`super::RandomFactory`] to use
type DefaultRngFactory = super::DefaultRngFactory;

/// The default insertion sort to use
type DefaultInsertionSort = super::insertionsort::InsertionSort;

/// The default `INSERTION_THRESHOLD` to use
pub const DEFAULT_INSERTION_THRESHOLD: usize = 24;

/// The default `NINTHER_THRESHOLD` to use
pub const DEFAULT_NINTHER_THRESHOLD: usize = 128;

/// The default `CHECK_SORTED` to use
pub const DEFAULT_CHECK_SORTED: bool = false;

/// The quicksort [`super::Sort`]
pub struct QuickSort<
    R: super::RandomFactory = DefaultRngFactory,
    I: super::Sort = DefaultInsertionSort,
    const INSERTION_THRESHOLD: usize = DEFAULT_INSERTION_THRESHOLD,
    const NINTHER_THRESHOLD: usize = DEFAULT_NINTHER_THRESHOLD,
    const CHECK_SORTED: bool = DEFAULT_CHECK_SORTED,
>(std::marker::PhantomData<R>, std::marker::PhantomData<I>);

impl<
    R: super::RandomFactory,
    I: super::Sort,
    const INSERTION_THRESHOLD: usize,
    const NINTHER_THRESHOLD: usize,
    const CHECK_SORTED: bool,
> super::Sort for QuickSort<R, I, INSERTION_THRESHOLD, NINTHER_THRESHOLD, CHECK_SORTED>
{
    const IS_STABLE: bool = false && I::IS_STABLE;

    fn sort<T: Ord>(slice: &mut [T]) {
        let mut rng = R::produce();

        quicksort::<T, R::Rng, I, INSERTION_THRESHOLD, NINTHER_THRESHOLD, CHECK_SORTED>(
            slice, &mut rng,
        );
    }
}

/// Quicksort the given slice
fn quicksort<
    T: Ord,
    R: rand::Rng,
    I: super::Sort,
    const INSERTION_THRESHOLD: usize,
    const NINTHER_THRESHOLD: usize,
    const CHECK_SORTED: bool,
>(
    slice: &mut [T],
    rng: &mut R,
) {
    debug_assert!(
        INSERTION_THRESHOLD >= 3,
        "We don't want to deal with slices smaller than that."
    );

    // Use insertion sort for small slices
    if slice.len() <= INSERTION_THRESHOLD {
        I::sort(slice);
        return;
    }

    // Check if we're already done and abort
    if CHECK_SORTED && slice.is_sorted() {
        return;
    }

    /// Call [`move_median_to_first()`] with random indices
    fn move_random_median_to_first<T: Ord, R: rand::Rng>(slice: &mut [T], rng: &mut R) {
        move_median_to_first(
            slice,
            rng.random_range(0..slice.len()),
            rng.random_range(0..slice.len()),
            rng.random_range(0..slice.len()),
        );
    }

    // Increase the likelihood of having a good pivot
    move_random_median_to_first(slice, rng);
    if slice.len() >= NINTHER_THRESHOLD {
        move_random_median_to_first(&mut slice[1..], rng);
        move_random_median_to_first(&mut slice[2..], rng);
        move_median_to_first(slice, 0, 1, 2);
    }

    // Classic quicksort partition with pivot at index 0
    let mut i = 0;
    let mut j = slice.len();
    loop {
        i += 1;
        j -= 1;
        while i < slice.len() && slice[i] < slice[0] {
            i += 1;
        }
        while slice[j] > slice[0] {
            j -= 1;
        }
        if j > i {
            slice.swap(i, j);
        } else {
            break;
        }
    }
    i -= 1;

    // Swap the pivot into place
    slice.swap(0, i);

    // Recurse into both partitions
    quicksort::<T, R, I, INSERTION_THRESHOLD, NINTHER_THRESHOLD, CHECK_SORTED>(
        &mut slice[..i],
        rng,
    );
    // This panics, other than the i = 0 case, which is why we need to check for it
    if i < slice.len() {
        quicksort::<T, R, I, INSERTION_THRESHOLD, NINTHER_THRESHOLD, CHECK_SORTED>(
            &mut slice[i + 1..],
            rng,
        );
    }
}

// TODO: is this right?
/// Swap the median of the three indices with the first element of the slice
fn move_median_to_first<T: Ord>(slice: &mut [T], index1: usize, index2: usize, index3: usize) {
    let indices = &mut [index1, index2, index3];
    indices.sort_by_key(|i| &slice[*i]);
    slice.swap(0, indices[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNS: usize = 100;
    const TEST_SIZE: usize = 100_000;

    type QuickSortChecked = QuickSort<
        DefaultRngFactory,
        DefaultInsertionSort,
        DEFAULT_INSERTION_THRESHOLD,
        DEFAULT_NINTHER_THRESHOLD,
        true,
    >;

    #[test]
    fn empty() {
        crate::test::test_empty::<QuickSort>();
        crate::test::test_empty::<QuickSortChecked>();
    }

    #[test]
    fn random() {
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, QuickSort>();
        crate::test::test_random_sorted::<RUNS, TEST_SIZE, QuickSortChecked>();
    }

    #[test]
    #[should_panic] // TODO: should we implement stable quicksort?
    fn random_stable() {
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, QuickSort>();
        crate::test::test_random_stable_sorted::<RUNS, TEST_SIZE, QuickSortChecked>();
    }
}
