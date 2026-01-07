use clap::Parser as _;
use rand::SeedableRng as _;

mod algorithms;
mod data;

#[cfg(test)]
mod test;

// General TODO: check for overflows and stuff?

/// Program entry point
fn main() {
    let input::Args {
        algorithms: input::Algorithms(algorithms),
        runs,
        size,
        data,
        seed,
    } = input::Args::parse();

    println!(
        "Running measurements for the following algorithms:\n{algs}",
        algs = algorithms
            .iter()
            .map(|a| format!("{a}: stable = {stable}", stable = a.is_stable()))
            .collect::<Vec<_>>()
            .join(",\n")
    );
    println!("Runs: {runs}, Slice size: {size}, Data type: {data}");

    // Create rng
    // FIXME: this is probably bad, but i have to look into rng anyway
    let mut rng = match seed {
        Some(partial_seed) => {
            let mut seed = [0; 32];
            seed[16..].copy_from_slice(&partial_seed.to_le_bytes());
            rand::rngs::StdRng::from_seed(seed)
        }
        None => {
            println!("No seed provided, generating one using system rng");
            rand::rngs::StdRng::from_os_rng()
        }
    };

    for algorithm in algorithms {
        println!("Running experiment with sort: {algorithm}");

        let (samples, stats) = match data {
            input::DataType::UniformU32 => {
                perform_experiment::<u32, data::UniformData<u32>>(algorithm, runs, size, &mut rng)
            }
            input::DataType::PermutationU32 => {
                perform_experiment::<u32, data::PermutationData<u32>>(
                    algorithm, runs, size, &mut rng,
                )
            }
        };

        println!("Stats: {stats:?}");
    }
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
    use crate::algorithms::*;
    use clap::ValueEnum as _;

    /// Command line arguments
    #[derive(clap::Parser)]
    pub struct Args {
        /// The sorting algorithms to run, seperated by colons for example: 'std,quicksort'
        #[arg()]
        pub algorithms: Algorithms,
        /// The number of runs to do
        #[arg()]
        pub runs: usize,
        /// The size of the slices to sort
        #[arg()]
        pub size: usize,
        /// The data type to use for sorting
        #[arg()]
        pub data: DataType,
        /// Seed for the rng
        #[arg(long)]
        pub seed: Option<u128>,
    }

    /// Define the sorting algorithms available to the user
    macro_rules! algorithms {
        (
            $(
                $(
                    #[$attr:meta]
                )*
                $name:ident : $sort:ty
            ),*
            $(,)?
        ) => {
            /// The different sorting algorithms
            #[derive(Debug, Clone, Copy, clap::ValueEnum, Hash, PartialEq, Eq)]
            pub enum Algorithm {
                $(
                    $(
                        #[$attr]
                    )*
                    $name,
                )*
            }

            // Delegate each variant to the corresponding Sort type
            impl Algorithm {
                /// The sort function
                pub fn sorter<T: Ord>(self) -> fn(&mut [T]) {
                    match self {
                        $(
                            Self::$name => <$sort as crate::algorithms::Sort>::sort,
                        )*
                    }
                }

                /// Return whether the sort is stable
                pub fn is_stable(self) -> bool {
                    match self {
                        $(
                            Self::$name => <$sort as crate::algorithms::Sort>::IS_STABLE,
                        )*
                    }
                }
            }
        };
    }

    type TrotSort = timsort::TimSort<
        timsort::DefaultInsertionSort,
        merging::CopyBoth,
        timsort::DefaultBufGuardFactory,
        { timsort::DEFAULT_MIN_MERGE },
    >;

    algorithms! {
        /// The algorithm used by the rust std library
        Std: StdSort,
        /// The unstable algorithm used by the rust std library
        StdUnstable: StdSort<false>,
        /// Insertion sort
        Insertion: insertionsort::InsertionSort,
        /// Binary Insertion sort
        BinaryInsertion: insertionsort::InsertionSort<true>,
        /// Quicksort
        Quicksort: quicksort::QuickSort,
        /// Peeksort
        Peeksort: peeksort::PeekSort,
        /// Bottom-up mergesort
        BottomUpMergesort: bottom_up_mergesort::BottomUpMergesort,
        /// Top-down mergesort
        TopDownMergesort: top_down_mergesort::TopDownMergesort,
        /// Timsort
        Timsort: timsort::TimSort,
        /// Trotsort
        Trotsort: TrotSort,
    }

    impl std::fmt::Display for Algorithm {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.to_possible_value().unwrap().get_name())
        }
    }

    /// Set of sorting [`Algorithms`](crate::algorithms::Algorithm) to run
    #[derive(Debug, Clone)]
    pub struct Algorithms(pub std::collections::HashSet<Algorithm>);

    impl clap::builder::ValueParserFactory for Algorithms {
        type Parser = AlgorithmsParser;

        fn value_parser() -> Self::Parser {
            AlgorithmsParser(clap::builder::EnumValueParser::new())
        }
    }

    /// [`Parser`](clap::builder::TypedValueParser) for [`Algorithms`]
    #[derive(Clone)]
    pub struct AlgorithmsParser(clap::builder::EnumValueParser<Algorithm>);

    impl clap::builder::TypedValueParser for AlgorithmsParser {
        type Value = Algorithms;

        fn parse_ref(
            &self,
            cmd: &clap::Command,
            arg: Option<&clap::Arg>,
            value: &std::ffi::OsStr,
        ) -> Result<Self::Value, clap::Error> {
            let mut algorithms = std::collections::HashSet::new();

            if value.is_empty() {
                return self.0.parse_ref(cmd, arg, value).map(|_| unreachable!());
            }

            for mut value in value.to_string_lossy().split(',') {
                value = value.trim();

                if value == "*" {
                    self.0
                        .possible_values()
                        .unwrap()
                        .try_for_each(|possible_value| {
                            self.0
                                .parse_ref(cmd, arg, possible_value.get_name().as_ref())
                                .map(|algorithm| {
                                    algorithms.insert(algorithm);
                                })
                        })?;
                } else {
                    algorithms.insert(self.0.parse_ref(cmd, arg, value.as_ref())?);
                }
            }

            Ok(Algorithms(algorithms))
        }

        fn possible_values(
            &self,
        ) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
            Some(Box::new(self.0.possible_values().unwrap().chain(
                std::iter::once(
                    clap::builder::PossibleValue::new("*").help("All sorting algorithms"),
                ),
            )))
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
            f.write_str(match self {
                DataType::UniformU32 => "Uniform u32",
                DataType::PermutationU32 => "Permutation u32",
            })
        }
    }
}
