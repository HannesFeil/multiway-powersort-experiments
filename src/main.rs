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

    let sorter = cli::AlgorithmVariants::sorter(algorithm, variant).unwrap();

    let (samples, stats);

    with_match_type! {
        data;
        T, D => {
            (samples, stats) =
                perform_experiment::<T, D>(sorter, runs, size, &mut rng);
        }
    };

    println!("Stats: {stats:?}, samples: {s}", s = samples.len());
}

/// Perform a time sampling experiment on the given sorting algorithm
///
/// - runs: The number of samples to measure
/// - size: The size of the slices to sort
/// - rng: The rng used for sampling the data
fn perform_experiment<T: Ord + std::fmt::Debug, D: data::Data<T>>(
    sorter: fn(&mut [T]),
    runs: usize,
    size: usize,
    rng: &mut impl rand::Rng,
) -> (Vec<std::time::Duration>, rolling_stats::Stats<f64>) {
    let mut samples = Vec::with_capacity(runs);

    let mut stats: rolling_stats::Stats<f64> = rolling_stats::Stats::new();

    let bar = indicatif::ProgressBar::new(runs as u64);

    for run in 0..=runs {
        let mut data = D::default().initialize(size, rng);

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
