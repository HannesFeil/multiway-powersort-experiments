#![warn(
    clippy::as_conversions,
    clippy::missing_safety_doc,
    reason = "Check for scrutiny"
)]

use std::io::Write;

use clap::Parser as _;
use rand::SeedableRng as _;

mod algorithms;
mod cli;
mod data;

#[cfg(test)]
mod test;

/// Executable entry point
fn main() {
    let cli::Args {
        algorithm,
        variant,
        runs,
        size,
        data,
        seed,
        output,
    } = cli::Args::parse();

    // Validate the given algorithm variant
    if !cli::Algorithm::validate(algorithm, &variant) {
        println!("Invalid variant {variant} for algorithm {algorithm}");
        println!("Possible variants:");
        for (variant, description) in cli::Algorithm::variant_descriptions(algorithm) {
            println!("{variant}: {description}");
        }
        return;
    };

    println!(
        "Running measurements for the following (stable: {stable}) algorithm:\n({variant}) {alg}",
        alg = algorithm
            .variant_descriptions()
            .find_map(|(id, description)| (*id == variant).then_some(description))
            .unwrap(),
        stable = algorithm.is_stable(&variant).unwrap(),
    );
    println!("Runs: {runs}, Slice size: {size}, Data type: {data}");

    // Create RNG for data generation
    let mut rng = match seed {
        Some(partial_seed) => rand::rngs::StdRng::seed_from_u64(partial_seed),
        None => {
            println!("No seed provided, generating one using system rng");
            rand::rngs::StdRng::from_os_rng()
        }
    };

    // Collect different samples if we count comparisons
    #[cfg(not(feature = "counters"))]
    /// The sample type to collect, measured runtime.
    type SampleOutput = std::time::Duration;
    #[cfg(feature = "counters")]
    /// The sample type to collect, measured comparisons and other counters.
    type SampleOutput = CounterSample;

    /// Dummy struct to allow dispatching on different data types.
    ///
    /// This is effectively a generic closure.
    struct DataTypeDispatcher<'rng, R: rand::Rng> {
        algorithm: cli::Algorithm,
        variant: String,
        runs: usize,
        size: usize,
        rng: &'rng mut R,
    }
    impl<R: rand::Rng> cli::DataTypeDispatcher for DataTypeDispatcher<'_, R> {
        type Output = Vec<SampleOutput>;

        fn dispatch<
            T: Ord + std::fmt::Debug,
            D: crate::data::DataGenerator<T>
                + crate::data::DataGenerator<crate::data::CountComparisons<T>>,
        >(
            self,
        ) -> Self::Output {
            // Get the sort function pointer (data type can be inferred at this point)
            let sorter = self.algorithm.sorter(&self.variant).unwrap();

            // Measure running times
            #[cfg(not(feature = "counters"))]
            {
                let (samples, stats) =
                    perform_time_experiment::<T, D>(sorter, self.runs, self.size, self.rng);

                println!("Run times in ms:\n{stats:#?}");

                samples
            }

            // Measure comparisons and merge costs
            #[cfg(feature = "counters")]
            {
                let (samples, stats) =
                    perform_counters_experiment::<T, D>(sorter, self.runs, self.size, self.rng);

                println!("Comparisons:\n{stats:#?}");

                samples
            }
        }
    }

    // Run the experiment with the given algorithm and data
    let samples = data.dispatch(DataTypeDispatcher {
        algorithm,
        variant,
        runs,
        size,
        rng: &mut rng,
    });

    // Write samples to output file if given
    if let Some(output) = output {
        write_output(&output, samples).unwrap_or_else(|error| {
            eprintln!("An error occurred while trying to write output at {output:?}: {error}");
        });
    }
}

/// Writes `samples` to a file at `path`, which is created in case it does not exist.
///
/// Returns IO error if writing to the file is not possible.
fn write_output<S: Samples<N>, const N: usize>(
    path: impl AsRef<std::path::Path>,
    samples: S,
) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;

    // Write the CSV header
    file.write_all(S::headers().join(",").as_bytes())?;
    file.write_all(b"\n")?;

    // Write the individual lines (escaping should not be necessary since we only write integers)
    for line in samples.csv_lines() {
        file.write_all(line.join(",").as_bytes())?;
        file.write_all(b"\n")?;
    }

    Ok(())
}

/// A trait for encoding samples as CSV with `N` columns
trait Samples<const N: usize> {
    /// Returns the column headers for this data
    fn headers() -> [String; N];

    /// Returns the individual CSV lines, with one [`String`] per column
    fn csv_lines(self) -> impl Iterator<Item = [String; N]>;
}

impl Samples<1> for Vec<std::time::Duration> {
    fn headers() -> [std::string::String; 1] {
        ["ns".to_string()]
    }

    fn csv_lines(self) -> impl Iterator<Item = [String; 1]> {
        self.into_iter()
            .map(|duration| [duration.as_nanos().to_string()])
    }
}

impl Samples<4> for Vec<CounterSample> {
    fn headers() -> [std::string::String; 4] {
        ["comparisons", "alloc", "slice", "buffer"].map(str::to_string)
    }

    fn csv_lines(self) -> impl Iterator<Item = [String; 4]> {
        self.into_iter().map(|sample| {
            [
                sample.comparisons.to_string(),
                sample.merge_alloc_cost.to_string(),
                sample.merge_slice_cost.to_string(),
                sample.merge_buffer_cost.to_string(),
            ]
        })
    }
}

/// The global counters used during the experiment
pub static GLOBAL_COUNTERS: GlobalCounters = GlobalCounters {
    comparisons: data::GlobalCounter::new(),
    merge_alloc: data::GlobalCounter::new(),
    merge_slice: data::GlobalCounter::new(),
    merge_buffer: data::GlobalCounter::new(),
};

/// Container for global counters used during the experiment
pub struct GlobalCounters {
    pub comparisons: data::GlobalCounter,
    pub merge_alloc: data::GlobalCounter,
    pub merge_slice: data::GlobalCounter,
    pub merge_buffer: data::GlobalCounter,
}

impl GlobalCounters {
    /// Reset all global counters
    pub fn reset(&self) {
        self.comparisons.read_and_reset();
        self.merge_alloc.read_and_reset();
        self.merge_slice.read_and_reset();
        self.merge_buffer.read_and_reset();
    }
}

/// A single sample point for measuring comparisons and merge costs
#[derive(Debug)]
struct CounterSample {
    /// The number of comparisons
    comparisons: u64,
    /// The number of elements needed as additional merge allocation
    merge_alloc_cost: u64,
    /// The number of elements written to the original slice during merging
    merge_slice_cost: u64,
    /// The number of elements written to the buffer during merging
    merge_buffer_cost: u64,
}

/// Performs a time sampling experiment on the given sorting algorithm
///
/// - `sorter`: The function used for sorting
/// - `runs`: The number of samples to measure
/// - `size`: The size of the slices to sort
/// - `rng`: The RNG used for sampling the data
#[allow(dead_code, reason = "Unused when feature 'counters' is active")]
fn perform_time_experiment<T: Ord + std::fmt::Debug, D: data::DataGenerator<T>>(
    sorter: fn(&mut [T]),
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) -> (Vec<std::time::Duration>, rolling_stats::Stats<f64>) {
    let mut samples = Vec::with_capacity(runs);
    let mut stats: rolling_stats::Stats<f64> = rolling_stats::Stats::new();

    perform_experiment::<_, T, D>(
        |sort| {
            let now = std::time::Instant::now();
            sort();
            let elapsed = now.elapsed();

            samples.push(elapsed);
            #[expect(
                clippy::as_conversions,
                reason = "Millis should not get high enough for this cast to become inaccurate"
            )]
            stats.update(elapsed.as_millis() as f64);
        },
        sorter,
        runs,
        size,
        rng,
    );

    (samples, stats)
}

/// Performs a sampling experiment on the given sorting algorithm.
///
/// Records comparisons, as well as different merge costs, see [`CounterSample`].
///
/// - `sorter`: The function used for sorting
/// - `runs`: The number of samples to measure
/// - `size`: The size of the slices to sort
/// - `rng`: The RNG used for sampling the data
#[allow(dead_code, reason = "Unused when feature 'counters' is inactive")]
fn perform_counters_experiment<
    T: Ord + std::fmt::Debug,
    D: data::DataGenerator<crate::data::CountComparisons<T>>,
>(
    sorter: fn(&mut [crate::data::CountComparisons<T>]),
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) -> (Vec<CounterSample>, rolling_stats::Stats<f64>) {
    let mut samples = Vec::with_capacity(runs);
    let mut stats = rolling_stats::Stats::<f64>::new();

    perform_experiment::<_, crate::data::CountComparisons<T>, D>(
        |sort| {
            GLOBAL_COUNTERS.reset();

            sort();

            let comparisons = GLOBAL_COUNTERS.comparisons.read_and_reset();
            let merge_alloc_cost = GLOBAL_COUNTERS.merge_alloc.read_and_reset();
            let merge_slice_cost = GLOBAL_COUNTERS.merge_slice.read_and_reset();
            let merge_buffer_cost = GLOBAL_COUNTERS.merge_buffer.read_and_reset();

            let sample = CounterSample {
                comparisons,
                merge_alloc_cost,
                merge_slice_cost,
                merge_buffer_cost,
            };

            samples.push(sample);

            #[expect(
                clippy::as_conversions,
                reason = "Comparisons should not get high enough for this cast to become inaccurate"
            )]
            stats.update(comparisons as f64);
        },
        sorter,
        runs,
        size,
        rng,
    );

    (samples, stats)
}

/// Perform a generic sampling experiment on the given sorting algorithm.
///
/// - `sampler`: The function used for sampling, receiving the running time of each sort iteration
/// - `sorter`: The function used for sorting
/// - `runs`: The number of samples to measure
/// - `size`: The size of the slices to sort
/// - `rng`: The RNG used for sampling the data
fn perform_experiment<
    F: FnMut(&mut dyn FnMut()),
    T: Ord + std::fmt::Debug,
    D: data::DataGenerator<T>,
>(
    mut sampler: F,
    sorter: fn(&mut [T]),
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) {
    #[expect(
        clippy::as_conversions,
        reason = "Realistically runs is not gonna be higher than u64::MAX"
    )]
    let bar = indicatif::ProgressBar::new(runs as u64);
    let mut generator = D::default();
    let mut data = generator.initialize(size, rng);

    for run in 0..=runs {
        let mut sort = || sorter(std::hint::black_box(&mut data));

        // Skip first sample (behavior taken from original codebase)
        if run != 0 {
            sampler(&mut sort);
            bar.inc(1);
        } else {
            sort();
        }

        assert!(
            data.is_sorted(),
            "Data was not sorted after algorithm run: {run}"
        );

        generator.reinitialize(&mut data, rng);
    }
}
