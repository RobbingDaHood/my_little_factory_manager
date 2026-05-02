#[cfg(feature = "simulation")]
use std::fs;
use std::path::PathBuf;

#[cfg(feature = "simulation")]
use super::game_driver::{GameDriver, GameResult};
#[cfg(feature = "simulation")]
use super::strategies::smart_strategy::SmartStrategy;

#[cfg(feature = "simulation")]
/// Runs a single game with a given seed and returns JSON-serializable results.
pub fn run_single_game(seed: u64, max_actions: u64) -> GameResult {
    let strategy = SmartStrategy::new();
    let driver = GameDriver::new(max_actions, vec![10, 20, 30, 40, 50]);

    eprintln!("[batch] seed={} running...", seed);
    let result = driver.play_game(seed, &strategy);
    eprintln!("[batch] seed={} completed", seed);

    result
}

#[cfg(feature = "simulation")]
/// Runs multiple games with different seeds and saves results to files.
pub fn run_batch(seeds: &[u64], output_dir: &str, max_actions: u64) -> Result<(), Box<dyn std::error::Error>> {
    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir)?;

    for seed in seeds {
        let result = run_single_game(*seed, max_actions);

        // Save JSON result for this game
        let json_str = serde_json::to_string_pretty(&result)?;
        let output_path = PathBuf::from(output_dir).join(format!("seed_{}.json", seed));
        fs::write(&output_path, json_str)?;
        eprintln!("[batch] Saved results to {}", output_path.display());
    }

    Ok(())
}
