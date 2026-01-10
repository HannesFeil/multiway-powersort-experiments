use clap::Parser as _;
use rand::SeedableRng;

mod algorithms;
mod data;

#[cfg(test)]
mod test;

// General TODO: check for overflows and stuff?

/// Program entry point
fn main() {
    let input::Args {
        algorithm,
        runs,
        size,
        data,
        seed,
    } = input::Args::parse();

    println!(
        "Running measurements for the following algorithm:\n{algorithm:?} (stable: {stable})",
        stable = algorithm.is_stable(),
    );
    println!("Runs: {runs}, Slice size: {size}, Data type: {data}");

    // Create rng
    // FIXME: this is probably bad, but i have to look into rng anyway
    let mut rng = match seed {
        Some(partial_seed) => rand::rngs::StdRng::seed_from_u64(partial_seed),
        None => {
            println!("No seed provided, generating one using system rng");
            rand::rngs::StdRng::from_os_rng()
        }
    };

    let (samples, stats) = match data {
        input::DataType::UniformU64 => {
            perform_experiment::<u64, data::UniformData<u64>>(algorithm, runs, size, &mut rng)
        }
        input::DataType::PermutationU64 => {
            perform_experiment::<u64, data::PermutationData<u64>>(algorithm, runs, size, &mut rng)
        }
    };

    println!("Stats: {stats:?}");
}

/// Perform a time sampling experiment on the given sorting algorithm
///
/// - runs: The number of samples to measure
/// - size: The size of the slices to sort
/// - rng: The rng used for sampling the data
fn perform_experiment<T: Ord + std::fmt::Debug, D: data::Data<T>>(
    algorithm: input::Algorithm,
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) -> (Vec<std::time::Duration>, rolling_stats::Stats<f64>) {
    let sorter = algorithm.sorter();
    let mut samples = Vec::with_capacity(runs);

    let mut stats: rolling_stats::Stats<f64> = rolling_stats::Stats::new();

    let bar = indicatif::ProgressBar::new(runs as u64);

    for run in 0..=runs {
        let mut data = D::initialize(size, rng);

        let now = std::time::Instant::now();
        sorter(std::hint::black_box(&mut data));
        let elapsed = now.elapsed();

        debug_assert!(
            data.is_sorted(),
            "{data:?} is not sorted after algorithm run"
        );

        // NOTE: Skip first sample (behavior taken from original codebase)
        if run != 0 {
            samples.push(elapsed);
            // TODO: is this cast fine?
            stats.update(elapsed.as_millis() as f64);

            bar.inc(1);
        }
    }

    (samples, stats)
}

/// Command line input handling
mod input {
    use crate::algorithms::Sort;

    /// Command line arguments
    #[derive(clap::Parser)]
    #[command(
        author,
        version,
        about,
        subcommand_value_name = "sort",
        subcommand_help_heading = "Sorts",
        disable_help_subcommand = true
    )]
    pub struct Args {
        /// The sorting algorithm to run
        #[command(subcommand)]
        pub algorithm: Algorithm,
        /// The number of runs to do
        #[arg(long, default_value_t = 1_000)]
        pub runs: usize,
        /// The size of the slices to sort
        #[arg(long, default_value_t = 1_000_000)]
        pub size: usize,
        /// The data type to use for sorting
        #[arg(long, default_value_t = DataType::PermutationU64)]
        pub data: DataType,
        /// Seed for the rng
        #[arg(long)]
        pub seed: Option<u64>,
    }

    #[derive(Debug, clap::Subcommand)]
    pub enum Algorithm {
        /// The default sort in [`std`]
        Std {
            /// Whether to use the unstable version
            #[arg(short, long)]
            unstable: bool,
        },
        /// Insertionsort
        Insertionsort {
            /// Whether to use the binary version
            #[arg(short, long)]
            binary: bool,
        },
        /// Quicksort
        Quicksort {
            /// Abort on already sorted slice
            #[arg(short, long)]
            check_sorted: bool,
        },
        /// Peeksort
        Peeksort {
            /// Whether to also peek for and reverse decreasing runs
            #[arg(short, long)]
            find_decreasing: bool,
        },
        /// Mergesort
        Mergesort {
            /// Whether to use bottom up merging instead of top down
            #[arg(short, long)]
            bottom_up: bool,
            /// Abort on already sorted slice
            #[arg(short, long)]
            check_sorted: bool,
        },
        /// Timsort
        Timsort {
            /// Whether to use [`crate::algorithms::merging::CopyBoth`], turning this into Trotsort
            #[arg(short, long)]
            simple_merging: bool,
        },
        /// Powersort
        Powersort {},
    }

    macro_rules! with_match_type {
        (
            type $t:ident = match ($value:expr) {
                $(
                    $pattern:pat => $t_value:ty
                ),*
                $(,)?
            }

            $code:block
        ) => {
            match $value {
                $(
                    $pattern => {
                        type $t = $t_value;

                        $code
                    }
                ),*
            }
        };
    }

    macro_rules! with_match_const {
        (
            const $t:ident: $t_type:ty = match ($value:expr) {
                $(
                    $pattern:pat => $t_value:expr
                ),*
                $(,)?
            }

            $code:block
        ) => {
            match $value {
                $(
                    $pattern => {
                        const $t: $t_type = $t_value;

                        $code
                    }
                ),*
            }
        };
    }

    macro_rules! with_type {
        ($alg:expr => $t:ident, $code:block) => {
            match $alg {
                Algorithm::Std { unstable } => with_match_const! {
                    const STABLE: bool = match (unstable) {
                        true => false,
                        false => true,
                    }

                    {
                        type $t = crate::algorithms::StdSort::<STABLE>;

                        $code
                    }
                },
                Algorithm::Insertionsort { binary } => with_match_const! {
                    const BINARY: bool = match (binary) {
                        true => true,
                        false => false,
                    }

                    {
                        type $t = crate::algorithms::insertionsort::InsertionSort::<BINARY>;

                        $code
                    }
                },
                Algorithm::Quicksort { check_sorted } => with_match_const! {
                    const CHECK_SORTED: bool = match (check_sorted) {
                        true => true,
                        false => false,
                    }

                    {
                        type $t = crate::algorithms::quicksort::QuickSort::<
                            crate::algorithms::quicksort::DefaultRngFactory,
                            crate::algorithms::quicksort::DefaultInsertionSort,
                            { crate::algorithms::quicksort::DEFAULT_INSERTION_THRESHOLD },
                            { crate::algorithms::quicksort::DEFAULT_NINTHER_THRESHOLD },
                            CHECK_SORTED,
                        >;

                        $code
                    }
                },
                Algorithm::Peeksort { find_decreasing } => with_match_const! {
                    const ONLY_INCREASING: bool = match (find_decreasing) {
                        true => true,
                        false => false,
                    }

                    {
                        type $t = crate::algorithms::peeksort::PeekSort::<
                            crate::algorithms::peeksort::DefaultInsertionSort,
                            crate::algorithms::peeksort::DefaultMergingMethod,
                            crate::algorithms::peeksort::DefaultBufGuardFactory,
                            { crate::algorithms::peeksort::DEFAULT_INSERTION_THRESHOLD },
                            ONLY_INCREASING,
                        >;

                        $code
                    }
                },
                Algorithm::Mergesort {
                    bottom_up,
                    check_sorted,
                } => with_match_const! {
                    const CHECK_SORTED: bool = match (check_sorted) {
                        true => true,
                        false => false,
                    }

                    {
                        with_match_type! {
                            type $t = match (bottom_up) {
                                true => crate::algorithms::mergesort::BottomUpMergeSort::<
                                    crate::algorithms::mergesort::DefaultInsertionSort,
                                    crate::algorithms::mergesort::DefaultMergingMethod,
                                    crate::algorithms::mergesort::DefaultBufGuardFactory,
                                    { crate::algorithms::mergesort::DEFAULT_INSERTION_THRESHOLD },
                                    CHECK_SORTED,
                                >,
                                false => crate::algorithms::mergesort::TopDownMergeSort::<
                                    crate::algorithms::mergesort::DefaultInsertionSort,
                                    crate::algorithms::mergesort::DefaultMergingMethod,
                                    crate::algorithms::mergesort::DefaultBufGuardFactory,
                                    { crate::algorithms::mergesort::DEFAULT_INSERTION_THRESHOLD },
                                    CHECK_SORTED,
                                >,
                            }

                            $code
                        }
                    }
                },
                Algorithm::Timsort { simple_merging } => with_match_type! {
                    type $t = match (simple_merging) {
                        false => crate::algorithms::timsort::TimSort<
                            crate::algorithms::timsort::DefaultInsertionSort,
                            crate::algorithms::timsort::DefaultMergingMethod,
                            crate::algorithms::timsort::DefaultBufGuardFactory,
                            { crate::algorithms::timsort::DEFAULT_MIN_MERGE },
                        >,
                        true => crate::algorithms::timsort::TimSort<
                            crate::algorithms::timsort::DefaultInsertionSort,
                            crate::algorithms::merging::CopyBoth,
                            crate::algorithms::timsort::DefaultBufGuardFactory,
                            { crate::algorithms::timsort::DEFAULT_MIN_MERGE },
                        >,
                    }

                    $code
                },
                Algorithm::Powersort {} => {
                    type $t = crate::algorithms::powersort::PowerSort;

                    $code
                }
            }
        };
    }

    impl Algorithm {
        /// Returns if this is a stable sort
        pub fn is_stable(&self) -> bool {
            with_type! { self => S, { S::IS_STABLE } }
        }

        /// Returns the sorting function
        pub fn sorter<T: Ord>(&self) -> fn(&mut [T]) {
            with_type! { self => S, { S::sort } }
        }
    }

    /// Available data types for sorting
    #[derive(Clone, Copy, clap::ValueEnum)]
    pub enum DataType {
        UniformU64,
        PermutationU64,
    }

    impl std::fmt::Display for DataType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(match self {
                DataType::UniformU64 => "uniform-u64",
                DataType::PermutationU64 => "permutation-u64",
            })
        }
    }
}
