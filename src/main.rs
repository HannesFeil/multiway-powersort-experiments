use clap::Parser as _;
use rand::SeedableRng as _;

mod algorithms;
mod cli;
mod data;

#[cfg(test)]
mod test;

// General TODO: check for overflows and stuff?

/// Program entry point
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

    let Some(variant) = cli::AlgorithmVariants::validate(algorithm, variant) else {
        println!("Invalid variant {variant} for algorithm {algorithm}");
        println!("Possible variants:");
        for (index, variant) in cli::AlgorithmVariants::variants(algorithm).enumerate() {
            println!("{index:>3}: {variant}");
        }
        return;
    };

    println!(
        "Running measurements for the following (stable: {stable}) algorithm:\n{alg}",
        alg = cli::AlgorithmVariants::variants(algorithm)
            .nth(variant)
            .unwrap(),
        stable = cli::AlgorithmVariants::is_stable(algorithm, variant).unwrap(),
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

    let (samples, stats);

    with_match_type! {
        data;
        T, D => {
            let sorter = cli::AlgorithmVariants::sorter(algorithm, variant).unwrap();
            #[cfg(not(feature = "counters"))]
            {
                (samples, stats) =
                    perform_time_experiment::<T, D>(sorter, runs, size, &mut rng);
            }
            #[cfg(feature = "counters")]
            {
                (samples, stats) = perform_counters_experiment::<T, D>(sorter, runs, size, &mut rng);
            };
        }
    };

    println!("Stats: {stats:#?}");

    if let Some(output) = output {
        #[cfg(not(feature = "counters"))]
        let data: Vec<String> = samples
            .into_iter()
            .map(|duration| duration.as_nanos().to_string())
            .collect();
        #[cfg(feature = "counters")]
        let data: Vec<String> = samples
            .into_iter()
            .map(
                |[comparisons, alloc, slice_merge_cost, buffer_merge_cost]| {
                    format!("{comparisons},{alloc},{slice_merge_cost},{buffer_merge_cost}")
                },
            )
            .collect();
        std::fs::write(&output, data.join("\n")).unwrap();
    }
}

/// Perform a time sampling experiment on the given sorting algorithm
///
/// - runs: The number of samples to measure
/// - size: The size of the slices to sort
/// - rng: The rng used for sampling the data
#[allow(dead_code)]
fn perform_time_experiment<T: Ord + std::fmt::Debug, D: data::Data<T>>(
    sorter: fn(&mut [T]),
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) -> (Vec<std::time::Duration>, rolling_stats::Stats<f64>) {
    let samples = Vec::with_capacity(runs);
    let stats: rolling_stats::Stats<f64> = rolling_stats::Stats::new();

    perform_experiment::<_, _, T, D>(
        (samples, stats),
        |(samples, stats), elapsed| {
            samples.push(elapsed);
            stats.update(elapsed.as_nanos() as f64);
        },
        sorter,
        runs,
        size,
        rng,
    )
}

/// Perform a time sampling experiment on the given sorting algorithm
///
/// - runs: The number of samples to measure
/// - size: The size of the slices to sort
/// - rng: The rng used for sampling the data
#[allow(dead_code)]
fn perform_counters_experiment<
    T: Ord + std::fmt::Debug,
    D: data::Data<crate::data::CountComparisons<T>>,
>(
    sorter: fn(&mut [crate::data::CountComparisons<T>]),
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) -> (Vec<[u64; 4]>, [rolling_stats::Stats<f64>; 4]) {
    let samples = Vec::with_capacity(runs);
    let stats = std::array::from_fn::<_, 4, _>(|_| rolling_stats::Stats::new());

    perform_experiment::<_, _, crate::data::CountComparisons<T>, D>(
        (samples, stats),
        |(samples, stats), _| {
            let comparisons = data::COMPARISON_COUNTER.read_and_reset();
            let alloc = algorithms::merging::ALLOC_COUNTER.read_and_reset();
            let merge_slice_cost = algorithms::merging::MERGE_SLICE_COUNTER.read_and_reset();
            let merge_buffer_cost =
                crate::algorithms::merging::MERGE_BUFFER_COUNTER.read_and_reset();

            let sample = [comparisons, alloc, merge_slice_cost, merge_buffer_cost];

            samples.push(sample);
            for (stat, value) in stats.iter_mut().zip(sample.iter()) {
                stat.update(*value as f64);
            }
        },
        sorter,
        runs,
        size,
        rng,
    )
}

/// Perform a time sampling experiment on the given sorting algorithm
///
/// - runs: The number of samples to measure
/// - size: The size of the slices to sort
/// - rng: The rng used for sampling the data
fn perform_experiment<
    S,
    F: FnMut(&mut S, std::time::Duration),
    T: Ord + std::fmt::Debug,
    D: data::Data<T>,
>(
    mut initial: S,
    mut sampler: F,
    sorter: fn(&mut [T]),
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) -> S {
    let bar = indicatif::ProgressBar::new(runs as u64);

    for run in 0..=runs {
        let mut data = D::default().initialize(size, rng);

        reset_global_counters();

        let now = std::time::Instant::now();
        sorter(std::hint::black_box(&mut data));
        let elapsed = now.elapsed();

        // NOTE: Skip first sample (behavior taken from original codebase)
        if !run == 0 {
            sampler(&mut initial, elapsed);
            bar.inc(1);
        }

        debug_assert!(
            data.is_sorted(),
            "{data:?} is not sorted after algorithm run"
        );
    }

    initial
}

fn reset_global_counters() {
    data::COMPARISON_COUNTER.read_and_reset();
    algorithms::merging::ALLOC_COUNTER.read_and_reset();
    algorithms::merging::MERGE_SLICE_COUNTER.read_and_reset();
    algorithms::merging::MERGE_BUFFER_COUNTER.read_and_reset();
}
