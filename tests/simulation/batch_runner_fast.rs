#[cfg(feature = "simulation")]
use std::fs;
use std::path::PathBuf;

#[cfg(feature = "simulation")]
use super::game_driver::{GameDriver, GameResult};
#[cfg(feature = "simulation")]
use super::strategies::smart_strategy::SmartStrategy;

#[cfg(feature = "simulation")]
/// Runs a single game with a given seed and returns JSON-serializable results.
/// This version uses reduced action limits for faster testing.
pub fn run_single_game_fast(seed: u64) -> GameResult {
    let strategy = SmartStrategy::new();
    // Use a smaller action limit for faster testing (100,000 instead of 500,000)
    let driver = GameDriver::new(100_000, vec![10, 20, 30, 40, 50]);

    eprintln!("[batch] seed={} running...", seed);
    let result = driver.play_game(seed, &strategy);
    eprintln!("[batch] seed={} completed: max_tier={:?}", seed, result.max_tier_reached);

    result
}

#[cfg(feature = "simulation")]
/// Runs multiple games with different seeds and saves results to files.
/// This is the fast version for quicker testing and demonstration.
pub fn run_batch_fast(seeds: &[u64], output_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir)?;

    for seed in seeds {
        let result = run_single_game_fast(*seed);

        // Save JSON result for this game
        let json_str = serde_json::to_string_pretty(&result)?;
        let output_path = PathBuf::from(output_dir).join(format!("seed_{}.json", seed));
        fs::write(&output_path, json_str)?;
        eprintln!("[batch] Saved results to {}", output_path.display());
    }

    Ok(())
}
