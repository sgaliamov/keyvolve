use crate::app::{evaluate, merge, synthesise};
use crate::{
    Config, Mode,
    app::{EMPTY_SLOT, LayoutEvaluator, optimize},
    models::{Keyboard, Layout},
};
use cliffa::cli::AppHandle;
use miette::{Context, IntoDiagnostic, Result};
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
            let words = std::fs::read_to_string(cfg.text.unwrap())
                .into_diagnostic()
                .wrap_err("Failed to read text file")?
                .split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<_>>();

            let keyboard = Keyboard::load(cfg.keyboard.unwrap())?;
            let opt = cfg.optimization;
            let evaluator = LayoutEvaluator::new(
                &keyboard,
                words,
                opt.bigram_switch_penalty,
                opt.balance_penalty,
                opt.alternation_penalty,
            );

            match mode {
                Mode::Evaluate => {
                    let layouts_path = cfg.layouts.wrap_err("Missing layouts path in config")?;
                    let layouts = Layout::load(&layouts_path);
                    info!("Loaded {} layouts", layouts.len());
                    evaluate(evaluator, layouts, &layouts_path, app)?
                }
                Mode::Optimize => {
                    let mut ga = cfg.ga;
                    ga.ranges = vec![vec![(EMPTY_SLOT, 'z'); 30]];
                    let mut seed: Vec<_> = vec![];
                    if let Some(layouts_path) = cfg.layouts {
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

/// Convert a `Layout` into a 30-slot genome; empty slots filled with `EMPTY_SLOT`.
pub fn layout_to_genome(layout: Layout) -> Vec<char> {
    let mut slots = vec![EMPTY_SLOT; 30];
    for (c, pos) in layout.keys {
        slots[pos as usize] = c;
    }
    slots
}
