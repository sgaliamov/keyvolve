pub mod evaluate;
pub mod frequencies;
pub mod merge;
pub mod optimize;
pub mod rank;
pub mod synthesise;
use crate::{
    evaluator::{LayoutEvaluator, penalty::table},
    models::{Layout, ScoreResult},
};
use tracing::info;

/// Log the per-metric penalty table for a layout.
pub fn log_breakdown(evaluator: &LayoutEvaluator, layout: &Layout, score: &ScoreResult) {
    let terms = score.breakdown(evaluator.config());
    if terms.is_empty() {
        return;
    }

    info!("{layout} penalty breakdown:\n{}", table(&terms));
}
