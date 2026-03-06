//! Command line input handling

/// Run sorting algorithms on random data and measure their performance
#[derive(clap::Parser)]
#[command(
    version,
    subcommand_value_name = "sort",
    subcommand_help_heading = "Sorts",
    disable_help_subcommand = true
)]
pub struct Args {
    /// The sorting algorithm to run
    #[arg()]
    pub algorithm: Algorithm,
    /// The datatype and distribution to use for sorting
    #[arg(short, long, default_value_t = DataType::RandomRunsSqrtU32)]
    pub data: DataType,
    /// The algorithm variant, use `-v=-1` to print available options
    #[arg(short, long, default_value_t = 0)]
    pub variant: isize,
    /// The number of runs to do
    #[arg(short, long, default_value_t = 1_000)]
    pub runs: usize,
    /// The size of the data slices to sort
    #[arg(short, long, default_value_t = 1_000_000)]
    pub size: usize,
    /// Seed for the RNG
    #[arg(long)]
    pub seed: Option<u64>,
    /// An optional output file to write the samples to (formatted as CSV)
    pub output: Option<std::path::PathBuf>,
}

/// The available top level sorting algorithms
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

/// Returns the multiline string representation of a sorting algorithm.
pub fn display<S: Sort>() -> String {
    format!(
        "{base}\n{parameters}",
        base = S::BASE_NAME,
        parameters = S::parameters()
            .map(|(key, value)| format!("\t{key} = {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// Returns the inline string representation of a sorting algorithm.
pub fn display_inline<S: Sort>() -> String {
    format!(
        "{base} {parameters}",
        base = S::BASE_NAME,
        parameters = S::parameters()
            .map(|(key, value)| format!("({key} = {value})"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// Declare the available algorithm variants.
///
/// We use a macro to statically dispatch on the respective type, given an algorithm and variant.
///
/// # Example usage
///
/// ```rust
/// declare_variants! {
///     AlgorithmVariants {
///         Algorithm::Std => [
///             StdSort,
///             StdSort<false>,
///         ],
///         // ...
///     }
/// }
/// ```
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
        // Create the struct (mainly used as a namespace)
        pub struct $name;

        // Create the struct implementation
        impl $name {
            /// Returns an iterator over every available variant for the given algorithm.
            ///
            /// The variants are returned in form of their display representation, see
            /// [`display()`]
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

            /// Returns the sorting function for the given datatype `T` and `algorithm` variant.
            ///
            /// If the `variant` is invalid, returns `None`.
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

            /// Returns if the `algorithm` `variant` is stable.
            ///
            /// If the `variant` is invalid returns `None`.
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
    // Statically dispatch with [`crate::algorithm::Sort`] type, depending on the algorithm and variant
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

// Use namespace to reduce declaration verbosity
use crate::algorithms::*;

// Statically declare all available algorithm variants
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
                true,
            >,
        ],
        Algorithm::Peeksort => [
            peeksort::PeekSort<
                peeksort::DefaultInsertionSort,
                peeksort::DefaultMergingMethod,
                peeksort::DefaultBufGuardFactory,
                { peeksort::DEFAULT_INSERTION_THRESHOLD },
                false,
            >,
        ],
        Algorithm::Mergesort => [
            mergesort::MergeSort,
            mergesort::MergeSort<
                mergesort::DefaultInsertionSort,
                mergesort::DefaultMergingMethod,
                mergesort::DefaultBufGuardFactory,
                { mergesort::DEFAULT_BOTTOM_UP },
                1,
                false,
            >,
            mergesort::MergeSort<
                mergesort::DefaultInsertionSort,
                mergesort::DefaultMergingMethod,
                mergesort::DefaultBufGuardFactory,
                { mergesort::DEFAULT_BOTTOM_UP },
                1,
                true,
            >,
            mergesort::MergeSort<
                mergesort::DefaultInsertionSort,
                mergesort::DefaultMergingMethod,
                mergesort::DefaultBufGuardFactory,
                true,
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
                powersort::DefaultNodePowerMethod,
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
    /// Returns the given variant index as `usize` if valid and `None` otherwise.
    ///
    /// Negative values are always invalid.
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

/// Declare the available data types and distributions variants.
///
/// We the input to generate a macro that statically dispatches on the type, given a value.
///
/// # Example usage
///
/// ```rust
/// declare_data_types! {
///     /// Datatype and distribution description
///     Name = DataType : DistributionType,
///     /// A random permutation of u32 values
///     PermutationU32 = u32 : PermutationData,
///     // ...
/// }
/// ```
///
/// It generates the macro `with_match_type!`, see its documentation for how to use it.
macro_rules! declare_data_types {
    (
        $(
            $(
                #[$attribute:meta]
            )*
            $name:ident = $type:ty : $d_type:ty
        ),*
        $(,)?
    ) => {
        /// Available data types and distributions for sorting.
        #[derive(Clone, Copy, clap::ValueEnum)]
        pub enum DataType {
            $(
                $(
                    #[$attribute]
                )*
                $name
            ),*
        }

        // Generate the macro for dispatching on the datatype and distribution type
        // We pass along a '$' used to generate the inner macro
        declare_data_types! {
            @declare_match_macro
            $($name : $type, $d_type),* | $
        }
    };
    (@declare_match_macro $($name:ident : $type:ty, $d_type:ty),* | $dollar:tt) => {
        /// A macro to dynamically dispatch on the corresponding datatype and distribution type (:
        ///
        /// # Example usage
        ///
        /// ```rust
        /// let data = crate::cli::DataType::PermutationU32;
        ///
        /// with_match_type! {
        ///     data;
        ///     DataType, DistributionType => {
        ///         println!("{}", std::any::type_name::<DataType>());
        ///         println!("{}", std::any::type_name::<DistributionType>());
        ///     }
        /// };
        /// ```
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

/// L+P datatype, should be equivalent to the C++ original definition
pub type Blob2U64CmpFirst = crate::data::Blob<u64, crate::data::CompareFirstEntry, 2>;

// Declare the available data types
declare_data_types! {
    /// A random permutation of u32 values
    PermutationU32       = u32 : crate::data::PermutationData,
    /// Random runs with average length of `n.isqrt()` of u32 values
    RandomRunsSqrtU32    = u32 : crate::data::RandomRunsSqrtData,
    /// Random runs with average length of `3` of u32 values
    RandomRuns3U32       = u32 : crate::data::RandomRunsConstData<3>,
    /// Random runs with average length of `30` of u32 values
    RandomRuns30U32      = u32 : crate::data::RandomRunsConstData<30>,
    /// Random runs with average length of `300` of u32 values
    RandomRuns300U32     = u32 : crate::data::RandomRunsConstData<300>,
    /// Random runs with average length of `3000` of u32 values
    RandomRuns3000U32    = u32 : crate::data::RandomRunsConstData<3000>,
    /// Random runs with average length of `30000` of u32 values
    RandomRuns30000U32   = u32 : crate::data::RandomRunsConstData<30000>,
    /// Random runs with average length of `300000` of u32 values
    RandomRuns300000U32  = u32 : crate::data::RandomRunsConstData<300000>,
    /// Random runs with average length of `3000000` of u32 values
    RandomRuns3000000U32 = u32 : crate::data::RandomRunsConstData<3000000>,

    /// A random permutation of L+P blobs
    PermutationLP    = Blob2U64CmpFirst : crate::data::PermutationData,
    /// Random runs with average length of `n.isqrt()` of L+P blobs
    RandomRunsSqrtLP = Blob2U64CmpFirst : crate::data::RandomRunsSqrtData,
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(clap::ValueEnum::to_possible_value(self).unwrap().get_name())
    }
}
