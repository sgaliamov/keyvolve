mod config;
mod evaluator;
mod models;

use cliffa::cli::AppHandle;
pub use config::*;
pub use evaluator::*;
use itertools::Itertools;
use miette::{Context, IntoDiagnostic, Result};
pub use models::*;
use rayon::prelude::*;
use tracing::{info, trace};

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    let keyboard = Keyboard::load(cfg.keyboard.unwrap())?;

    let words = std::fs::read_to_string(cfg.text.unwrap())
        .into_diagnostic()
        .wrap_err("Failed to read text file")?
        .split_whitespace()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let words_ref = words.iter().map(|s| s.as_str()).collect_vec();

    match cfg.mode {
        Mode::Evaluate => {
            let evaluator = LayoutEvaluator::new(&keyboard);

            let layouts = Layout::load(cfg.layouts.unwrap());
            info!("Loaded {} layouts", layouts.len());

            let mut scored: Vec<_> = layouts
                .par_iter()
                .filter_map(|layout| {
                    if app.should_finish() {
                        return None;
                    }
                    Some((layout, evaluator.score_corpus(&words_ref, &layout.keys)))
                })
                .collect();

            scored.sort_by(|a, b| {
                b.1.effort
                    .partial_cmp(&a.1.effort)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            scored.iter().take(10).for_each(|(layout, layout_score)| {
                info!("{} {}", layout.name, layout_score);
            });
        }
        _ => unimplemented!("Only evaluation mode is implemented currently."),
    }

    Ok(())
}
