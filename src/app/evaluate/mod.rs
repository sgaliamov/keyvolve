pub mod config;
use crate::{
    app::{LayoutEvaluator, write_layouts},
    models::{Layout, ScoreResult},
};
use cliffa::cli::AppHandle;
pub use config::*;
use itertools::Itertools;
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

    log_breakdown(&evaluator, scored.first());

    write_layouts(&scored, cfg.print, cfg.output.as_deref(), true, cfg.e_side)
}

/// Report which targets the best layout pays for; silent in powers mode.
fn log_breakdown(evaluator: &LayoutEvaluator, best: Option<&(Layout, ScoreResult, usize)>) {
    let Some((layout, score, _)) = best else {
        return;
    };

    let terms = evaluator.breakdown(score);
    if terms.is_empty() {
        return;
    }

    let costs = terms
        .iter()
        .map(|(m, cost)| format!("{m} {cost:.3}"))
        .join(", ");
    info!("{layout} penalty terms: {costs}");
}
