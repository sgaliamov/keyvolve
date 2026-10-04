pub mod config;
use crate::{
    evaluator::LayoutEvaluator,
    models::{Layout, ScoreResult},
    modes::log_breakdown,
    output::write_layouts,
};
use cliffa::cli::AppHandle;
pub use config::*;
use miette::Result;
use rayon::prelude::*;
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
    let scored = score_layouts(&evaluator, layouts, app);

    if let Some((layout, score, _)) = scored.first() {
        log_breakdown(&evaluator, layout, score);
    }

    write_layouts(&scored, cfg.print, cfg.output.as_deref(), true, cfg.e_side)
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
