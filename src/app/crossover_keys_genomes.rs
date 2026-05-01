use crate::{
    LayoutEvaluator,
    models::{KeyPos, KeysGenome, ScoreResult},
};
use darwin::{Context as GaContext, Individual};

/// Cross two parent genomes.
pub fn crossover_keys_genomes(
    _: &Individual<KeyPos, ScoreResult>,
    _: &Individual<KeyPos, ScoreResult>,
    _: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>,
) -> Vec<KeysGenome> {
    todo!()
}
