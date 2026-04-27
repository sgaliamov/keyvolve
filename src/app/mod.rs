mod config;
mod evaluator;
mod keyboard;
mod layout;

use crate::app::layout::Layout;
use cliffa::cli::AppHandle;
pub use config::*;
pub use evaluator::*;
use itertools::Itertools;
pub use keyboard::*;
use miette::{Context, IntoDiagnostic, Result};
use tracing::{info, trace};

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, _app: AppHandle) -> Result<()> {
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

            layouts.iter().for_each(|layout| {
                let layout_score = evaluator.score_corpus(&words_ref, &layout.keys);
                info!("Layout score: {} {:#?}", layout.name, layout_score);
            });
        }
        _ => unimplemented!("Only evaluation mode is implemented currently."),
    }

    Ok(())
}
