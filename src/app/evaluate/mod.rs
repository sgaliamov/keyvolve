pub mod config;

pub use config::*;

use crate::{
    app::{LayoutEvaluator, write_layouts},
    models::Layout,
};
use cliffa::cli::AppHandle;
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

    write_layouts(&scored, cfg.print, cfg.output.as_deref(), true)
}
