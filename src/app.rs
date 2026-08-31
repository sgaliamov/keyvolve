use crate::modes::{
    evaluate, frequencies, merge, optimize, rank, synthesise, synthesise::read_stats_cache,
};
use crate::{
    Config, Mode,
    evaluator::{CorpusCounts, EMPTY_SLOT, LayoutEvaluator, LayoutEvaluatorConfig},
    models::{Keyboard, Layout},
};
use cliffa::cli::AppHandle;
use miette::{Context, Result};
use std::path::Path;
use tracing::{info, trace};

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    match cfg.mode {
        Mode::Merge => {
            merge::merge(cfg.merge, app)?;
        }
        Mode::Synthesise => {
            synthesise::synthesise(cfg.synthesise)?;
        }
        Mode::Frequencies => {
            frequencies::frequencies(cfg.frequencies, app)?;
        }
        Mode::Rank => {
            rank::rank(cfg.rank, app)?;
        }
        mode => {
            let keyboard = Keyboard::load(cfg.keyboard)?;
            let evaluator_cfg = cfg.evaluator;
            let stats = cfg.stats;
            let opt = cfg.optimization;

            match mode {
                Mode::Evaluate => {
                    let eval = cfg.evaluate;
                    let evaluator = build_evaluator(&keyboard, &stats, evaluator_cfg)?;
                    let mut eval = eval;
                    if eval.input.is_empty() {
                        return Err(miette::miette!("evaluate.input requires at least one CSV"));
                    }
                    if eval.output.is_none() {
                        if eval.input.len() == 1 {
                            eval.output = Some(eval.input[0].clone());
                        } else {
                            return Err(miette::miette!(
                                "evaluate.output is required when evaluate.input has multiple CSVs"
                            ));
                        }
                    }
                    let layouts = eval
                        .input
                        .iter()
                        .map(Layout::load)
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();
                    info!("Loaded {} layouts", layouts.len());
                    evaluate::evaluate(evaluator, layouts, &eval, app)?
                }
                Mode::Optimize => {
                    let evaluator = build_evaluator(&keyboard, &stats, evaluator_cfg)?;
                    let mut ga = cfg.ga;
                    ga.ranges = vec![vec![(EMPTY_SLOT, 'z'); 30]];
                    let mut seed: Vec<_> = vec![];
                    if let Some(layouts_path) = opt.input.clone() {
                        let loaded = Layout::load(&layouts_path)?;
                        info!("Loaded {} seed layouts from file", loaded.len());
                        seed.extend(loaded.into_iter().map(layout_to_genome));
                    }
                    ga.seed = seed;
                    optimize::optimize(evaluator, ga, opt, app)?;
                }
                Mode::Synthesise | Mode::Merge | Mode::Frequencies | Mode::Rank => unreachable!(),
            }
        }
    }

    Ok(())
}

/// Build evaluator from keyboard and cached stats.
fn build_evaluator(
    keyboard: &Keyboard,
    stats_path: impl AsRef<Path>,
    config: LayoutEvaluatorConfig,
) -> Result<LayoutEvaluator> {
    let stats_path = stats_path.as_ref();
    if !stats_path.exists() {
        return Err(miette::miette!(
            "Missing corpus stats file: {}",
            stats_path.display()
        ));
    }

    info!(stats = %stats_path.display(), "Building corpus counts from cached stats");
    let counts = CorpusCounts::from(&read_stats_cache(stats_path)?);
    Ok(LayoutEvaluator::from_counts(keyboard, counts, config))
}

/// Convert a `Layout` into a 30-slot genome; empty slots filled with `EMPTY_SLOT`.
pub fn layout_to_genome(layout: Layout) -> Vec<char> {
    let mut slots = vec![EMPTY_SLOT; 30];
    for (c, pos) in layout.keys {
        slots[pos as usize] = c;
    }
    slots
}
