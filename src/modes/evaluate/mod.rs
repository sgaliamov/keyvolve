pub mod config;
pub mod optimization;
use crate::{
    evaluator::{LayoutEvaluator, penalty::table},
    models::{Layout, ScoreResult},
    output::write_layouts,
};
use cliffa::cli::AppHandle;
pub use config::*;
use miette::Result;
use rayon::prelude::*;
use tracing::info;

/// Evaluate layouts and write scored results.
pub fn evaluate(
    evaluator: LayoutEvaluator,
    layouts: Vec<Layout>,
    cfg: &EvaluateConfig,
    app: AppHandle,
) -> Result<()> {
    info!("Evaluating {} layouts", layouts.len());

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

    scored.sort_by(|a, b| {
        b.1.fitness
            .partial_cmp(&a.1.fitness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some((layout, score, _)) = scored.first() {
        log_breakdown(&evaluator, layout, score);
    }

    write_layouts(&scored, cfg.print, cfg.output.as_deref(), true, cfg.e_side)
}

/// Log the per-metric penalty table for a layout — the tuning aid that shows which
/// goal pays what and which term wins the next percentage point.
pub fn log_breakdown(evaluator: &LayoutEvaluator, layout: &Layout, score: &ScoreResult) {
    let terms = evaluator.breakdown(score);
    if terms.is_empty() {
        return;
    }

    info!("{layout} penalty breakdown:\n{}", table(&terms));
}
