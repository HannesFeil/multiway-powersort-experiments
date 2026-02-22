//! Command line input handling

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
    #[arg()]
    pub algorithm: Algorithm,
    /// The data type to use for sorting
    #[arg()]
    pub data: DataType,
    /// The algorithm variant, use `-v=-1` to print available options
    #[arg(short, long, default_value_t = 0)]
    pub variant: isize,
    /// The number of runs to do
    #[arg(short, long, default_value_t = 1_000)]
    pub runs: usize,
    /// The size of the slices to sort
    #[arg(short, long, default_value_t = 1_000_000)]
    pub size: usize,
    /// Seed for the rng
    #[arg(long)]
    pub seed: Option<u64>,
    /// The output file to write the samples to
    pub output: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Algorithm {
    /// The default sort in [`std`]
    Std,
    /// Insertionsort
    Insertionsort,
    /// Quicksort
    Quicksort,
    /// Peeksort
    Peeksort,
    /// Mergesort
    Mergesort,
    /// Timsort
    Timsort,
    /// Powersort
    Powersort,
    /// Powersort
    MultiwayPowersort,
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(clap::ValueEnum::to_possible_value(self).unwrap().get_name())
    }
}
macro_rules! declare_variants {
    (
        $name:ident {
            $(
                $top_algorithm:pat => [
                    $(
                        $variant:ty
                    ),*
                    $(,)?
                ]
            ),*
            $(,)?
        }
    ) => {
        pub struct $name;

        impl $name {
            pub fn variants(algorithm: Algorithm) -> impl Iterator<Item = String> {
                let mut variants = Vec::new();
                declare_variants! { @match_algorithm
                    algorithm => Variant
                    ($(
                        $top_algorithm => [
                            $($variant),*
                        ]
                    ),*)
                    {
                        variants.push(display::<Variant>())
                    }
                }
                variants.into_iter()
            }

            pub fn sorter<T: Ord>(algorithm: Algorithm, variant: usize) -> Option<fn(&mut [T])> {
                let mut index = 0;

                declare_variants! { @match_algorithm
                    algorithm => Variant
                    ($(
                        $top_algorithm => [
                            $($variant),*
                        ]
                    ),*)
                    {
                        if variant == index {
                            return Some(<Variant as Sort>::sort);
                        } else {
                            index += 1;
                        }
                    }
                }

                None
            }

            pub fn is_stable(algorithm: Algorithm, variant: usize) -> Option<bool> {
                let mut index = 0;

                declare_variants! { @match_algorithm
                    algorithm => Variant
                    ($(
                        $top_algorithm => [
                            $($variant),*
                        ]
                    ),*)
                    {
                        if variant == index {
                            return Some(<Variant as Sort>::IS_STABLE);
                        } else {
                            index += 1;
                        }
                    }
                }

                None
            }
        }
    };
    (@match_algorithm
        $alg:expr => $variant_name:ident
        ($(
            $top_algorithm:pat => [
                $($variant:ty),*
            ]
        ),*)
        $code:block
    ) => {
        match $alg {
            $(
                $top_algorithm => {
                    $(
                        {
                            type $variant_name = $variant;

                            $code
                        }
                    )*
                }
            )*
        }
    };
}

use crate::algorithms::*;

// TODO: fill variants
declare_variants! {
    AlgorithmVariants {
        Algorithm::Std => [
            StdSort,
            StdSort<false>,
        ],
        Algorithm::Insertionsort => [
            insertionsort::InsertionSort,
            insertionsort::InsertionSort<true>,
        ],
        Algorithm::Quicksort => [
            quicksort::QuickSort,
            quicksort::QuickSort<
                quicksort::DefaultRngFactory,
                quicksort::DefaultInsertionSort,
                { quicksort::DEFAULT_INSERTION_THRESHOLD },
                { quicksort::DEFAULT_NINTHER_THRESHOLD },
                true
            >,
        ],
        Algorithm::Peeksort => [
            peeksort::PeekSort,
        ],
        Algorithm::Mergesort => [
            mergesort::TopDownMergeSort,
            mergesort::TopDownMergeSort<
                mergesort::DefaultInsertionSort,
                mergesort::DefaultMergingMethod,
                mergesort::DefaultBufGuardFactory,
                1,
                false
            >,
            mergesort::TopDownMergeSort<
                mergesort::DefaultInsertionSort,
                mergesort::DefaultMergingMethod,
                mergesort::DefaultBufGuardFactory,
                1,
                true
            >,
            mergesort::BottomUpMergeSort<
                mergesort::DefaultInsertionSort,
                mergesort::DefaultMergingMethod,
                mergesort::DefaultBufGuardFactory,
                { mergesort::DEFAULT_INSERTION_THRESHOLD },
                true,
            >,
        ],
        Algorithm::Timsort => [
            timsort::TimSort,
            timsort::TimSort<
                timsort::DefaultInsertionSort,
                merging::two_way::CopyBoth,
                timsort::DefaultBufGuardFactory,
                { timsort::DEFAULT_MIN_MERGE },
            >,
            timsort::TimSort<
                insertionsort::InsertionSort<false>,
                merging::two_way::CopyBoth,
                timsort::DefaultBufGuardFactory,
                { timsort::DEFAULT_MIN_MERGE },
            >,
        ],
        Algorithm::Powersort => [
            powersort::PowerSort,
        ],
        Algorithm::MultiwayPowersort => [
            powersort::MultiwayPowerSort,
            powersort::MultiwayPowerSort<
                powersort::node_power::DivisionLoop,
                powersort::DefaultInsertionSort,
                merging::multi_way::Fourway,
                powersort::DefaultBufGuardFactory,
                4,
                { powersort::DEFAULT_MIN_RUN_LENGTH },
                { powersort::DEFAULT_ONLY_INCREASING_RUNS },
            >,
        ],
    }
}

impl AlgorithmVariants {
    pub fn validate(algorithm: Algorithm, variant: isize) -> Option<usize> {
        match variant.try_into() {
            Err(_) => None,
            Ok(result) => {
                if result < Self::variants(algorithm).count() {
                    Some(result)
                } else {
                    None
                }
            }
        }
    }
}

macro_rules! declare_data_types {
    (
        $(
            $name:ident : $type:ty, $d_type:ty
        ),*
        $(,)?
    ) => {
        /// Available data types for sorting
        #[derive(Clone, Copy, clap::ValueEnum)]
        pub enum DataType {
            $(
                $name
            ),*
        }

        declare_data_types! {
            @declare_match_macro
            $($name : $type, $d_type),* | $
        }
    };
    (@declare_match_macro $($name:ident : $type:ty, $d_type:ty),* | $dollar:tt) => {
        /// A hacky macro to dynamically "match" on type (:
        #[macro_export]
        #[expect(clippy::crate_in_macro_def)]
        macro_rules! with_match_type {
            ($dollar arg:expr; $dollar t:ident, $dollar d:ident => $dollar code:block) => {
                {
                    use crate::cli::*;
                    match $dollar arg {
                        $(
                            crate::cli::DataType::$name => {
                                type $dollar t = $type;
                                type $dollar d = $d_type;

                                $dollar code
                            }
                        ),*
                    }
                }
            };
        }
    };
}

pub type Blob2U64CmpFirst = crate::data::Blob<u64, crate::data::CompareFirstEntry, 2>;

declare_data_types! {
    // u32
    PermutationU32: u32, crate::data::PermutationData,
    RandomRunsSqrtU32: u32, crate::data::RandomRunsSqrtData,
    RandomRuns3U32: u32, crate::data::RandomRunsConstData<3>,
    RandomRuns30U32: u32, crate::data::RandomRunsConstData<30>,
    RandomRuns300U32: u32, crate::data::RandomRunsConstData<300>,
    RandomRuns3000U32: u32, crate::data::RandomRunsConstData<3000>,
    RandomRuns30000U32: u32, crate::data::RandomRunsConstData<30000>,
    RandomRuns300000U32: u32, crate::data::RandomRunsConstData<300000>,
    RandomRuns3000000U32: u32, crate::data::RandomRunsConstData<3000000>,

    // blob2u64
    PermutationLP: Blob2U64CmpFirst, crate::data::PermutationData,
    RandomRunsSqrtLP: Blob2U64CmpFirst, crate::data::RandomRunsSqrtData,
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(clap::ValueEnum::to_possible_value(self).unwrap().get_name())
    }
}
