use rand::Rng;

/// Quicksort the given slice
fn quicksort<
    T: Ord,
    R: Rng,
    const INSERTION_THRESHOLD: usize,
    const NINTHER_THRESHOLD: usize,
    const CHECK_SORTED: bool,
>(
    slice: &mut [T],
    rng: &mut R,
) {
    debug_assert!(INSERTION_THRESHOLD >= 3);

    // Use insertion sort for small slices
    if slice.len() <= INSERTION_THRESHOLD {
        crate::algorithms::insertionsort::insertion_sort(slice);
        return;
    }

    // Check if we're already done and abort
    if CHECK_SORTED && slice.is_sorted() {
        return;
    }

    /// Call [`move_median_to_first()`] with random indices
    fn move_random_median_to_first<T: Ord, R: Rng>(slice: &mut [T], rng: &mut R) {
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
    quicksort::<_, _, INSERTION_THRESHOLD, INSERTION_THRESHOLD, CHECK_SORTED>(&mut slice[..i], rng);
    // This panics, other than the i = 0 case, which is why we need to check for it
    if i < slice.len() {
        quicksort::<_, _, INSERTION_THRESHOLD, INSERTION_THRESHOLD, CHECK_SORTED>(
            &mut slice[i + 1..],
            rng,
        );
    }
}

// TODO: is this right?, should this be made stable?
/// Swap the median of the three indices with the first element of the slice
fn move_median_to_first<T: Ord>(slice: &mut [T], index1: usize, index2: usize, index3: usize) {
    let indices = &mut [index1, index2, index3];
    indices.sort_by_key(|i| &slice[*i]);
    slice.swap(0, indices[1]);
}

/// Quicksort the given slice using the default [`rand::rng()`]
pub fn default_rng_quicksort<
    T: Ord,
    const INSERTION_THRESHOLD: usize,
    const NINTHER_THRESHOLD: usize,
    const CHECK_SORTED: bool,
>(
    slice: &mut [T],
) {
    let mut rng = rand::rng();
    quicksort::<_, _, INSERTION_THRESHOLD, NINTHER_THRESHOLD, CHECK_SORTED>(slice, &mut rng);
}

/// Quicksort the given slice, with the following default const parameters
///
/// - `INSERTION_THRESHOLD = 24`
/// - `NINTHER_THRESHOLD = 128`
/// - `CHECK_SORTED = false`
pub fn default_quicksort<T: Ord>(slice: &mut [T]) {
    default_rng_quicksort::<_, 24, 128, false>(slice);
}
