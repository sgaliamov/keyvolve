use crate::app::{evaluate, merge, synthesise};
use crate::{
    Config, Mode,
    app::{EMPTY_SLOT, LayoutEvaluator, LayoutEvaluatorConfig, optimize},
    models::{Keyboard, Layout},
};
use cliffa::cli::AppHandle;
use miette::{Context, IntoDiagnostic, Result};
use std::path::Path;
use tracing::{info, trace};

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    match cfg.mode {
        Mode::Merge => {
            merge::merge(cfg.merge)?;
        }
        Mode::Synthesise => {
            synthesise::synthesise(cfg.synthesise)?;
        }
        mode => {
            let keyboard = Keyboard::load(cfg.keyboard)?;
            let evaluator_cfg = cfg.evaluator;
            let opt = cfg.optimization;

            match mode {
                Mode::Evaluate => {
                    let eval = cfg.evaluate;
                    let evaluator = build_evaluator(&keyboard, &eval.text, evaluator_cfg)?;
                    let layouts_path = eval.input.clone();
                    let mut eval = eval;
                    if eval.output.is_none() {
                        eval.output = Some(layouts_path.clone());
                    }
                    let layouts = Layout::load(&layouts_path);
                    info!("Loaded {} layouts", layouts.len());
                    evaluate::evaluate(evaluator, layouts, &eval, app)?
                }
                Mode::Optimize => {
                    let evaluator = build_evaluator(&keyboard, &opt.text, evaluator_cfg)?;
                    let mut ga = cfg.ga;
                    ga.ranges = vec![vec![(EMPTY_SLOT, 'z'); 30]];
                    let mut seed: Vec<_> = vec![];
                    if let Some(layouts_path) = opt.input.clone() {
                        let loaded = Layout::load(&layouts_path);
                        info!("Loaded {} seed layouts from file", loaded.len());
                        seed.extend(loaded.into_iter().map(layout_to_genome));
                    }
                    ga.seed = seed;
                    optimize(evaluator, ga, opt, app)?;
                }
                Mode::Synthesise | Mode::Merge => unreachable!(),
            }
        }
    }

    Ok(())
}

/// Build evaluator from keyboard, corpus file, and optimization penalties.
fn build_evaluator(
    keyboard: &Keyboard,
    text_path: impl AsRef<Path>,
    config: LayoutEvaluatorConfig,
) -> Result<LayoutEvaluator> {
    let words = load_words(text_path)?;
    Ok(LayoutEvaluator::new(keyboard, words, config))
}

/// Load whitespace-separated corpus words from text file.
fn load_words(text_path: impl AsRef<Path>) -> Result<Vec<String>> {
    Ok(std::fs::read_to_string(text_path)
        .into_diagnostic()
        .wrap_err("Failed to read text file")?
        .split_whitespace()
        .map(str::to_owned)
        .collect())
}

/// Convert a `Layout` into a 30-slot genome; empty slots filled with `EMPTY_SLOT`.
pub fn layout_to_genome(layout: Layout) -> Vec<char> {
    let mut slots = vec![EMPTY_SLOT; 30];
    for (c, pos) in layout.keys {
        slots[pos as usize] = c;
    }
    slots
}
