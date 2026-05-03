use crate::app::evaluate;
use crate::{
    Config, Mode,
    app::{LayoutEvaluator, optimize},
    models::{Keyboard, Layout},
};
use cliffa::cli::AppHandle;
use miette::{Context, IntoDiagnostic, Result};
use tracing::{info, trace};

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    let words = std::fs::read_to_string(cfg.text.unwrap())
        .into_diagnostic()
        .wrap_err("Failed to read text file")?
        .split_whitespace()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let keyboard = Keyboard::load(cfg.keyboard.unwrap())?;
    let evaluator = LayoutEvaluator::new(&keyboard, words);

    match cfg.mode {
        Mode::Evaluate => {
            let layouts_path = cfg.layouts.wrap_err("Missing layouts path in config")?;
            let layouts = Layout::load(&layouts_path);
            info!("Loaded {} layouts", layouts.len());
            evaluate(evaluator, &layouts, &layouts_path, app)?;
        }
        Mode::Optimize => {
            optimize(evaluator, cfg.ga, app)?;
        }
    }

    Ok(())
}
