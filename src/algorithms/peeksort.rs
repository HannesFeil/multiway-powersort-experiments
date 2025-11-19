pub mod merging_method {
    pub const COPY_BOTH: usize = 0;
}

fn peek_sort<
    T: Ord,
    const INSERTION_THRESHOLD: usize,
    const ONLY_INCREASING_RUNS: bool,
    const MERING_METHOD: usize,
>(
    slice: &mut [T],
    left_run_end: usize,
    right_run_begin: usize,
) {
}

pub fn peek_sort_with_vec() {}
