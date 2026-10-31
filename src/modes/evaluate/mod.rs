pub mod config;
use crate::{
    evaluator::LayoutEvaluator,
    models::{Layout, ScoreResult},
    modes::log_breakdown,
    output::{write_evaluation_files, write_layouts},
};
use cliffa::cli::AppHandle;
pub use config::*;
use miette::Result;
use rayon::prelude::*;
use std::path::PathBuf;
use tracing::info;

/// Scored layout row with source pool metadata.
pub type ScoredLayout = (Layout, ScoreResult, usize);

/// Evaluate layouts and write scored results.
pub fn evaluate(
    evaluator: LayoutEvaluator,
    layouts: Vec<Layout>,
    cfg: &EvaluateConfig,
    app: AppHandle,
) -> Result<()> {
    info!("Evaluating {} layouts", layouts.len());
    let scored = score_layouts(&evaluator, layouts, app.clone());

    if app.should_finish() {
        info!("Evaluation interrupted; skipped writing output");
        return Ok(());
    }

    if let Some((layout, score, _)) = scored.first() {
        log_breakdown(&evaluator, layout, score);
    }

    write_layouts(&scored, cfg.print, None, true, cfg.e_side)?;
    if let Some(path) = &cfg.output {
        let files = [(path.clone(), scored)];
        if !write_evaluation_files(&files, cfg.e_side, || app.should_finish())? {
            info!("Evaluation interrupted; skipped writing output");
        }
    }
    Ok(())
}

/// Score all input files before staging and replacing them in place.
pub fn evaluate_in_place(
    evaluator: LayoutEvaluator,
    batches: Vec<(PathBuf, Vec<Layout>)>,
    cfg: &EvaluateConfig,
    app: AppHandle,
) -> Result<()> {
    let mut all_scored = Vec::new();
    let mut per_file = Vec::new();
    for (path, layouts) in batches {
        let scored = score_layouts(&evaluator, layouts, app.clone());
        if app.should_finish() {
            info!(
                "Evaluation interrupted before all files were scored; skipped rewriting input files"
            );
            return Ok(());
        }
        all_scored.extend(scored.iter().cloned());
        per_file.push((path, scored));
    }
    sort_scored(&mut all_scored);

    if let Some((layout, score, _)) = all_scored.first() {
        log_breakdown(&evaluator, layout, score);
    }
    write_layouts(&all_scored, cfg.print, None, true, cfg.e_side)?;
    if !write_evaluation_files(&per_file, cfg.e_side, || app.should_finish())? {
        info!("Evaluation interrupted; skipped rewriting input files");
    }
    Ok(())
}

/// Score layouts and keep best rows first.
pub fn score_layouts(
    evaluator: &LayoutEvaluator,
    layouts: Vec<Layout>,
    app: AppHandle,
) -> Vec<ScoredLayout> {
    let mut scored: Vec<_> = layouts
        .into_par_iter()
        .filter_map(|layout| {
            if app.should_finish() {
                return None;
            }
            let score_corpus = evaluator.score_corpus(&layout.keys);
            Some((layout, score_corpus, 0usize))
        })
        .collect();
    sort_scored(&mut scored);
    scored
}

/// Sort scored layouts by fitness.
pub fn sort_scored(scored: &mut [ScoredLayout]) {
    scored.sort_by(|a, b| {
        b.1.fitness
            .partial_cmp(&a.1.fitness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluator::LayoutEvaluatorConfig, models::Keyboard, output::Side};
    use std::fs;

    fn evaluator() -> LayoutEvaluator {
        let keyboard = Keyboard::load("data/keyboard.json").unwrap();
        LayoutEvaluator::new(
            &keyboard,
            vec!["hello".into()],
            LayoutEvaluatorConfig::default(),
        )
    }

    fn layout() -> Layout {
        Layout::new("abcde,fghij,klmno,pqrst,uvwxy,z____,example")
    }

    #[test]
    fn cancelled_evaluation_leaves_output_untouched_even_with_no_layouts() {
        for existing in [false, true] {
            for layouts in [vec![], vec![layout()]] {
                let dir = tempfile::tempdir().unwrap();
                let output = dir.path().join("output.csv");
                if existing {
                    fs::write(&output, "original").unwrap();
                }
                let cfg = EvaluateConfig {
                    output: Some(output.clone()),
                    print: 0,
                    ..Default::default()
                };
                let app = AppHandle::new();
                app.finish();
                evaluate(evaluator(), layouts, &cfg, app).unwrap();
                if existing {
                    assert_eq!(fs::read_to_string(output).unwrap(), "original");
                } else {
                    assert!(!output.exists());
                }
            }
        }
    }

    #[test]
    fn normal_evaluation_writes_scored_output_and_in_place_files() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output.csv");
        let cfg = EvaluateConfig {
            output: Some(output.clone()),
            print: 0,
            e_side: Side::Any,
            ..Default::default()
        };
        evaluate(evaluator(), vec![layout()], &cfg, AppHandle::new()).unwrap();
        let expected = fs::read(&output).unwrap();
        assert!(
            String::from_utf8_lossy(&expected)
                .contains("abcde,fghij,klmno,pqrst,uvwxy,z____,example,")
        );

        let first = dir.path().join("first.csv");
        let second = dir.path().join("second.csv");
        fs::write(&first, "first original").unwrap();
        fs::write(&second, "second original").unwrap();
        evaluate_in_place(
            evaluator(),
            vec![
                (first.clone(), vec![layout()]),
                (second.clone(), vec![layout()]),
            ],
            &cfg,
            AppHandle::new(),
        )
        .unwrap();
        assert_eq!(fs::read(first).unwrap(), expected);
        assert_eq!(fs::read(second).unwrap(), expected);
    }

    #[test]
    fn cancelled_in_place_evaluation_does_not_rewrite_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("input.csv");
        fs::write(&path, "original").unwrap();
        let app = AppHandle::new();
        app.finish();
        evaluate_in_place(
            evaluator(),
            vec![(path.clone(), vec![])],
            &EvaluateConfig::default(),
            app,
        )
        .unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "original");
    }
}
