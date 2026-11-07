use crate::modes::{
    evaluate, frequencies, merge, optimize, rank, stats, synthesise, synthesise::read_stats_cache,
};
use crate::{
    Config, Mode,
    evaluator::{CorpusCounts, EMPTY_SLOT, LayoutEvaluator, LayoutEvaluatorConfig},
    models::{Keyboard, Layout},
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
                    let constraints = eval
                        .filter_invalid
                        .then(|| cfg.optimization.compile())
                        .transpose()?;
                    let keyboard = Keyboard::load(&eval.keyboard)?;
                    let evaluator = build_evaluator(&keyboard, &eval.corpus_stats, evaluator_cfg)?;
                    let mut batches = load_evaluate_batches(&eval)?;
                    if batches.is_empty() {
                        info!("No evaluate.input CSV files found; skipping evaluation");
                        return Ok(());
                    }
                    let total = batches
                        .iter()
                        .map(|(_, layouts)| layouts.len())
                        .sum::<usize>();
                    info!("Loaded {} layouts", total);

                    if let Some(constraints) = constraints {
                        filter_evaluate_batches(&mut batches, &constraints);
                    }

                    if eval.output.is_some() {
                        let layouts = batches
                            .into_iter()
                            .flat_map(|(_, layouts)| layouts)
                            .collect::<Vec<_>>();
                        evaluate::evaluate(evaluator, layouts, &eval, app)?
                    } else {
                        evaluate::evaluate_in_place(evaluator, batches, &eval, app)?;
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

/// Remove invalid original layouts while preserving their source files, including empty batches.
fn filter_evaluate_batches(
    batches: &mut [(PathBuf, Vec<Layout>)],
    constraints: &optimize::PlacementConstraints,
) {
    for (path, layouts) in batches {
        let total = layouts.len();
        layouts.retain(|layout| constraints.is_genome_valid(&layout_to_genome(layout.clone())));
        info!(
            file = %path.display(),
            removed = total - layouts.len(),
            retained = layouts.len(),
            "Filtered layouts under current placement constraints"
        );
    }
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
    use crate::modes::optimize::OptimizationConfig;
    use crate::modes::synthesise::{CachedSourceStats, CorpusStatsCounter, write_stats_cache};
    use crate::output::Side;
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

    #[test]
    fn evaluation_filter_uses_all_placement_rules_without_repairing_layouts() {
        let valid = Layout::new("abcde,fghij,klmno,pqrst,uvwxy,z____,valid");
        for (rules, swap) in [
            (r#"{"frozen":{"a":0}}"#, (0, 1)),
            (r#"{"blocked":[29]}"#, (25, 29)),
            (r#"{"allowed":{"e":[4]}}"#, (4, 9)),
            (r#"{"allowed":{"_":[10,11,12,13,14]}}"#, (24, 26)),
            (r#"{"left":["a"]}"#, (0, 15)),
            (r#"{"right":["p"]}"#, (0, 15)),
            (r#"{"pairs":["ab","bc"]}"#, (2, 17)),
            (r#"{"pairs":["ab"]}"#, (1, 5)),
            (r#"{"vertical":["ag"]}"#, (6, 10)),
            (r#"{"vertical":["pv"]}"#, (20, 21)),
            (r#"{}"#, (1, 26)),
            (
                r#"{"frozen":{"a":0},"blocked":[0],"right":["a"],"allowed":{"a":[14]}}"#,
                (0, 1),
            ),
        ] {
            let opt: OptimizationConfig = serde_json::from_str(rules).unwrap();
            let constraints = opt.compile().unwrap();
            let mut genome = layout_to_genome(valid.clone());
            let original = Layout::from_keys(&genome);
            assert!(constraints.is_genome_valid(&genome), "{rules}");
            genome.swap(swap.0, swap.1);
            let invalid = Layout::from_keys(&genome);
            let path = PathBuf::from("input.csv");
            let mut batches = vec![(path.clone(), vec![original.clone(), invalid])];

            filter_evaluate_batches(&mut batches, &constraints);

            assert_eq!(batches[0].0, path);
            assert_eq!(batches[0].1.len(), 1, "{rules}");
            assert_eq!(batches[0].1[0].keys, original.keys, "{rules}");
            assert_eq!(batches[0].1[0].name, original.name, "{rules}");
        }
    }

    #[test]
    fn evaluation_filter_writes_combined_or_per_file_results_including_empty_csvs() {
        for combined in [false, true] {
            for survivors in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let mut cfg = evaluation_config(dir.path());
                let first = dir.path().join("first.csv");
                let second = dir.path().join("second.csv");
                let output = dir.path().join("output.csv");
                let valid = Layout::new("abcde,fghij,klmno,pqrst,uvwxy,z____,valid");
                let invalid = valid.mirrored();
                let rejected = format!("{invalid},rejected\n");
                let first_content = if survivors {
                    format!("{valid},valid\n{rejected}")
                } else {
                    rejected.clone()
                };
                fs::write(&first, &first_content).unwrap();
                fs::write(&second, &rejected).unwrap();
                cfg.evaluate.input = vec![first.clone(), second.clone()];
                cfg.evaluate.output = combined.then_some(output.clone());
                cfg.evaluate.filter_invalid = true;
                cfg.optimization.frozen.insert('a', 0);

                run(Some(cfg), AppHandle::new()).unwrap();

                let result = if combined {
                    assert_eq!(fs::read_to_string(&first).unwrap(), first_content);
                    assert_eq!(fs::read_to_string(&second).unwrap(), rejected);
                    output
                } else {
                    assert_header_only(&second);
                    first
                };
                let rows = Layout::load(&result).unwrap();
                assert_eq!(rows.len(), usize::from(survivors));
                if survivors {
                    assert_eq!(rows[0].keys, valid.keys);
                    assert_eq!(rows[0].name, "valid");
                    assert!(fs::read_to_string(result).unwrap().contains(",fitness,"));
                } else {
                    assert_header_only(&result);
                }
            }
        }
    }

    #[test]
    fn evaluation_filter_checks_original_orientation_independently_of_save_side() {
        for side in [Side::Any, Side::Left, Side::Right] {
            let dir = tempfile::tempdir().unwrap();
            let mut cfg = evaluation_config(dir.path());
            let input = dir.path().join("input.csv");
            let output = dir.path().join("output.csv");
            let left = Layout::new("abcde,fghij,klmno,pqrst,uvwxy,z____,left");
            let right = left.mirrored();
            fs::write(&input, format!("{left},rejected\n{right},retained\n")).unwrap();
            cfg.evaluate.input = vec![input];
            cfg.evaluate.output = Some(output.clone());
            cfg.evaluate.filter_invalid = true;
            cfg.evaluate.e_side = side;
            cfg.optimization.right.insert('e');
            let constraints = cfg.optimization.compile().unwrap();

            run(Some(cfg), AppHandle::new()).unwrap();

            let rows = Layout::load(output).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].name, "retained");
            let expected = if side == Side::Left { &left } else { &right };
            assert_eq!(rows[0].keys, expected.keys);
            assert_eq!(
                constraints.is_genome_valid(&layout_to_genome(rows[0].clone())),
                side != Side::Left
            );
        }
    }

    #[test]
    fn contradictory_evaluation_constraints_fail_before_writes_only_when_enabled() {
        for enabled in [false, true] {
            for combined in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let mut cfg = evaluation_config(dir.path());
                let input = dir.path().join("input.csv");
                let output = dir.path().join("output.csv");
                // Complete, but violates the implicit no-gap rule.
                let original = "a_cde,fghij,klmno,pqrst,uvwxy,zb___,gap\n";
                fs::write(&input, original).unwrap();
                fs::write(&output, "original output").unwrap();
                cfg.evaluate.input = vec![input.clone()];
                cfg.evaluate.output = combined.then_some(output.clone());
                cfg.evaluate.filter_invalid = enabled;
                cfg.optimization.left.insert('a');
                cfg.optimization.right.insert('a');

                let result = run(Some(cfg), AppHandle::new());

                if enabled {
                    assert!(result.unwrap_err().to_string().contains("no legal slots"));
                    assert_eq!(fs::read_to_string(&input).unwrap(), original);
                    assert_eq!(fs::read_to_string(&output).unwrap(), "original output");
                } else {
                    result.unwrap();
                    let rows = Layout::load(if combined { &output } else { &input }).unwrap();
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].keys, Layout::new(original).keys);
                }
            }
        }
    }

    /// Build evaluation settings with a tiny cached corpus and no save normalization.
    fn evaluation_config(dir: &Path) -> Config {
        let mut counter = CorpusStatsCounter::default();
        counter.add_word("hello");
        let stats_path = dir.join("stats.json");
        write_stats_cache(
            &stats_path,
            &CachedSourceStats {
                stats: counter.finish(),
                word_count: 1,
            },
        )
        .unwrap();
        let mut cfg: Config = serde_json::from_str(r#"{"ga":{}}"#).unwrap();
        cfg.evaluate.keyboard = "data/keyboard.json".into();
        cfg.evaluate.corpus_stats = stats_path;
        cfg.evaluate.print = 0;
        cfg.evaluate.e_side = Side::Any;
        cfg
    }

    /// Assert an empty result still has the scored CSV header.
    fn assert_header_only(path: &Path) {
        let csv = fs::read_to_string(path).unwrap();
        assert!(csv.starts_with("keys_1,"));
        assert!(csv.contains(",fitness,"));
        assert_eq!(csv.lines().count(), 1);
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
            std::env::temp_dir().join(format!("ikeya-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
