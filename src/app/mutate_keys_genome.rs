use crate::{
    LayoutEvaluator,
    models::{KeyPos, KeysGenome, ScoreResult},
};
use darwin::{Context as GaContext, Individual};

/// Mutate a candidate genome.
pub fn mutate_keys_genome(
    _: &Individual<KeyPos, ScoreResult>,
    _: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>,
) -> Option<KeysGenome> {
    todo!()
}
