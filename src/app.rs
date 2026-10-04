use crate::modes::{
    evaluate, frequencies, log_breakdown, merge, optimize, rank, stats, synthesise,
    synthesise::read_stats_cache,
};
use crate::{
    Config, Mode,
    evaluator::{CorpusCounts, EMPTY_SLOT, LayoutEvaluator, LayoutEvaluatorConfig},
    models::{Keyboard, Layout},
    output::write_layouts,
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
        Mode::Stats => {
            stats::stats(cfg.stats)?;
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
            let evaluator_cfg = cfg.evaluator;
            match mode {
                Mode::Evaluate => {
                    let eval = cfg.evaluate;
                    let keyboard = Keyboard::load(&eval.keyboard)?;
                    let evaluator = build_evaluator(&keyboard, &eval.corpus_stats, evaluator_cfg)?;
                    if eval.input.is_empty() {
                        return Err(miette::miette!("evaluate.input requires at least one CSV"));
                    }
                    let batches = eval
                        .input
                        .iter()
                        .map(|path| Ok((path.clone(), Layout::load(path)?)))
                        .collect::<Result<Vec<_>>>()?;
                    let total = batches
                        .iter()
                        .map(|(_, layouts)| layouts.len())
                        .sum::<usize>();
                    info!("Loaded {} layouts", total);

                    if eval.output.is_some() {
                        let layouts = batches
                            .into_iter()
                            .flat_map(|(_, layouts)| layouts)
                            .collect::<Vec<_>>();
                        evaluate::evaluate(evaluator, layouts, &eval, app)?
                    } else {
                        let mut all_scored = Vec::with_capacity(total);
                        let mut per_file = Vec::new();
                        let mut interrupted = false;
                        for (path, layouts) in batches {
                            let expected = layouts.len();
                            let scored = evaluate::score_layouts(&evaluator, layouts, app.clone());
                            if app.should_finish() && scored.len() != expected {
                                interrupted = true;
                                break;
                            }
                            all_scored.extend(scored.iter().cloned());
                            per_file.push((path, scored));
                        }
                        if interrupted {
                            info!(
                                "Evaluation interrupted before all files were scored; skipped rewriting input files"
                            );
                            return Ok(());
                        }
                        evaluate::sort_scored(&mut all_scored);

                        if let Some((layout, score, _)) = all_scored.first() {
                            log_breakdown(&evaluator, layout, score);
                        }
                        write_layouts(&all_scored, eval.print, None, true, eval.e_side)?;

                        for (path, scored) in per_file {
                            write_layouts(&scored, 0, Some(path.as_path()), true, eval.e_side)?;
                        }
                    }
                }
                Mode::Optimize => {
                    let opt = cfg.optimization;
                    let keyboard = Keyboard::load(&opt.keyboard)?;
                    let evaluator = build_evaluator(&keyboard, &opt.corpus_stats, evaluator_cfg)?;
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
                Mode::Synthesise | Mode::Merge | Mode::Frequencies | Mode::Rank | Mode::Stats => {
                    unreachable!()
                }
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
