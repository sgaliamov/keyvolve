use crate::{LayoutEvaluator, models::{KeyPos, ScoreResult}};
use darwin::Context as GaContext;

/// Progress callback for optimize mode.
pub fn optimize_callback(ctx: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>) {
    let _ = ctx;
    todo!()
}

