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
use std::path::{Path, PathBuf};
use tracing::{info, trace, warn};

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    match cfg.mode {
        Mode::Merge => {
            merge::merge(cfg.merge, app)?;
        }
        Mode::Stats => {
            stats::stats(cfg.stats, app)?;
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
                    let batches = load_evaluate_batches(&eval)?;
                    if batches.is_empty() {
                        info!("No evaluate.input CSV files found; skipping evaluation");
                        return Ok(());
                    }
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
                    ga.seed = load_optimization_seed(opt.input.as_deref())?;
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

fn load_evaluate_batches(eval: &evaluate::EvaluateConfig) -> Result<Vec<(PathBuf, Vec<Layout>)>> {
    if eval.input.is_empty() {
        return Err(miette::miette!("evaluate.input requires at least one CSV"));
    }

    let mut batches = Vec::new();
    for path in &eval.input {
        if should_skip_missing_output_seed(path, eval.output.as_deref()) {
            warn!(
                file = %path.display(),
                "Skipping missing evaluate.input entry because it matches evaluate.output"
            );
            continue;
        }
        if !path.exists() {
            warn!(file = %path.display(), "Skipping missing evaluate.input CSV");
            continue;
        }

        let layouts = Layout::load(path)
            .wrap_err_with(|| format!("Failed to load evaluate.input CSV: {}", path.display()))?;
        batches.push((path.clone(), layouts));
    }

    if batches.is_empty() {
        warn!("No evaluate.input CSV files found; nothing to score");
    }

    Ok(batches)
}

fn should_skip_missing_output_seed(path: &Path, output: Option<&Path>) -> bool {
    output == Some(path) && !path.exists()
}

fn load_optimization_seed(path: Option<&Path>) -> Result<Vec<Vec<char>>> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    if !path.exists() {
        warn!(file = %path.display(), "Skipping missing optimization input CSV");
        return Ok(Vec::new());
    }

    let loaded = Layout::load(path)?;
    info!("Loaded {} seed layouts from file", loaded.len());
    Ok(loaded.into_iter().map(layout_to_genome).collect())
}

/// Convert a `Layout` into a 30-slot genome; empty slots filled with `EMPTY_SLOT`.
pub fn layout_to_genome(layout: Layout) -> Vec<char> {
    let mut slots = vec![EMPTY_SLOT; 30];
    for (c, pos) in layout.keys {
        slots[pos as usize] = c;
    }
    slots
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::evaluate::EvaluateConfig;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn load_evaluate_batches_skips_missing_output_bootstrap_file() {
        let dir = temp_test_dir("skip-missing-output-bootstrap");
        let output = dir.join("layouts.csv");
        let input = dir.join("layouts.seed.csv");
        write_layout_csv(&input);

        let eval = EvaluateConfig {
            input: vec![output.clone(), input.clone()],
            output: Some(output.clone()),
            ..EvaluateConfig::default()
        };

        let batches = load_evaluate_batches(&eval).unwrap();

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].0, input);
        assert_eq!(batches[0].1.len(), 1);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn load_evaluate_batches_skips_missing_input_csvs() {
        let dir = temp_test_dir("skip-missing-input-csvs");
        let missing = dir.join("missing.csv");
        let input = dir.join("present.csv");
        write_layout_csv(&input);

        let eval = EvaluateConfig {
            input: vec![missing.clone(), input.clone()],
            output: None,
            ..EvaluateConfig::default()
        };

        let batches = load_evaluate_batches(&eval).unwrap();

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].0, input);
        assert_eq!(batches[0].1.len(), 1);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn load_evaluate_batches_returns_empty_when_only_missing_files_exist() {
        let dir = temp_test_dir("empty-only-missing-files");
        let output = dir.join("layouts.csv");
        let missing = dir.join("missing.csv");
        let eval = EvaluateConfig {
            input: vec![output.clone(), missing.clone()],
            output: Some(output),
            ..EvaluateConfig::default()
        };

        let batches = load_evaluate_batches(&eval).unwrap();

        assert!(batches.is_empty());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn load_optimization_seed_skips_missing_file() {
        let dir = temp_test_dir("skip-missing-opt-seed");
        let missing = dir.join("missing.csv");

        let seed = load_optimization_seed(Some(missing.as_path())).unwrap();

        assert!(seed.is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    fn write_layout_csv(path: &Path) {
        fs::write(
            path,
            "keys_1,keys_2,keys_3,keys_4,keys_5,keys_6,name\nabcde,fghij,klmno,pqrst,uvwxy,z____,seed\n",
        )
        .unwrap();
    }

    fn temp_test_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("keyvolve-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
