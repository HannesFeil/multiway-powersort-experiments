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
        input::DataType::UniformU32 => {
            perform_experiment::<u32, data::UniformData<u32>>(algorithm, runs, size, &mut rng)
        }
        input::DataType::PermutationU32 => {
            perform_experiment::<u32, data::PermutationData<u32>>(algorithm, runs, size, &mut rng)
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
        #[arg(long, default_value_t = DataType::PermutationU32)]
        pub data: DataType,
        /// Seed for the rng
        #[arg(long)]
        pub seed: Option<u64>,
    }

    #[derive(Debug, Clone, Copy, clap::ValueEnum)]
    pub enum PowersortNodePowerMethod {
        Trivial,
        DivisionLoop,
        BitwiseLoop,
        MostSignificantSetBit,
    }

    impl std::fmt::Display for PowersortNodePowerMethod {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(clap::ValueEnum::to_possible_value(self).unwrap().get_name())
        }
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
        Powersort {
            /// Which node power calculation method to use
            #[arg(short, long, default_value_t = PowersortNodePowerMethod::MostSignificantSetBit)]
            node_power_method: PowersortNodePowerMethod,
            /// Whether to use a power indexed stack
            #[arg(short, long)]
            power_indexed_stack: bool,
        },
        /// Powersort
        MultiwayPowersort {
            /// Which node power calculation method to use
            #[arg(short, long, default_value_t = PowersortNodePowerMethod::MostSignificantSetBit)]
            node_power_method: PowersortNodePowerMethod,
            /// Which k to use
            #[arg(short, long, default_value_t = 2)]
            k: usize,
        },
    }

    macro_rules! with_match_type {
        (
            type $t:ident = match ($value:expr) {
                $(
                    $pattern:pat => $t_value:ty
                ),*
                $(; else => $else_expr:expr)?
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
                $(_ => $else_expr)?
            }
        };
    }

    macro_rules! with_match_const {
        (
            const $t:ident: $t_type:ty = match ($value:expr) {
                $(
                    $pattern:pat => $t_value:expr
                ),*
                $(; else => $else_expr:expr)?
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
                $(_ => $else_expr)?
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
                Algorithm::Powersort { node_power_method, power_indexed_stack } => {
                    with_match_type! {
                        type M = match (node_power_method) {
                            PowersortNodePowerMethod::Trivial => crate::algorithms::powersort::node_power::Trivial,
                            PowersortNodePowerMethod::DivisionLoop => crate::algorithms::powersort::node_power::DivisionLoop,
                            PowersortNodePowerMethod::BitwiseLoop => crate::algorithms::powersort::node_power::BitwiseLoop,
                            PowersortNodePowerMethod::MostSignificantSetBit => crate::algorithms::powersort::node_power::MostSignificantSetBit,
                        }

                        {
                            with_match_const! {
                                const USE_POWER_INDEXED_STACK: bool = match (power_indexed_stack) {
                                    true => true,
                                    false => false,
                                }

                                {
                                    type $t = crate::algorithms::powersort::PowerSort<
                                        M,
                                        crate::algorithms::powersort::DefaultInsertionSort,
                                        crate::algorithms::powersort::DefaultMergingMethod,
                                        crate::algorithms::powersort::DefaultBufGuardFactory,
                                        { crate::algorithms::powersort::DEFAULT_MIN_RUN_LENGTH },
                                        { crate::algorithms::powersort::DEFAULT_ONLY_INCREASING_RUNS },
                                        USE_POWER_INDEXED_STACK,
                                    >;

                                    $code
                                }
                            }
                        }
                    }
                }
                Algorithm::MultiwayPowersort { node_power_method, k } => {
                    with_match_type! {
                        type NodePowerMethod = match (node_power_method) {
                            PowersortNodePowerMethod::Trivial => crate::algorithms::powersort::node_power::Trivial,
                            PowersortNodePowerMethod::DivisionLoop => crate::algorithms::powersort::node_power::DivisionLoop,
                            PowersortNodePowerMethod::BitwiseLoop => crate::algorithms::powersort::node_power::BitwiseLoop,
                            PowersortNodePowerMethod::MostSignificantSetBit => crate::algorithms::powersort::node_power::MostSignificantSetBit,
                        }

                        {
                            with_match_const! {
                                const MERGE_K_RUNS: usize = match (k) {
                                    2 => 2,
                                    4 => 4,
                                    8 => 8,
                                    16 => 16;
                                    else => panic!("Unsupported k"),
                                }

                                {
                                    type $t = crate::algorithms::powersort::MultiwayPowerSort<
                                        NodePowerMethod,
                                        crate::algorithms::powersort::DefaultInsertionSort,
                                        crate::algorithms::powersort::DefaultMultiMergingMethod,
                                        crate::algorithms::powersort::DefaultBufGuardFactory,
                                        MERGE_K_RUNS,
                                        { crate::algorithms::powersort::DEFAULT_MIN_RUN_LENGTH },
                                        { crate::algorithms::powersort::DEFAULT_ONLY_INCREASING_RUNS },
                                    >;

                                    $code
                                }
                            }
                        }
                    }
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
        UniformU32,
        PermutationU32,
    }

    impl std::fmt::Display for DataType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(clap::ValueEnum::to_possible_value(self).unwrap().get_name())
        }
    }
}
