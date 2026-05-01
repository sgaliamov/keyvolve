use crate::{
    LayoutEvaluator,
    models::{KeyPos, KeysGenome, ScoreResult},
};
use darwin::Context as GaContext;

/// Generate a genome for optimization.
pub fn generate_keys_genome(_: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>) -> KeysGenome {
    todo!()
}
