use crate::{app::LayoutEvaluator, models::ScoreResult};

/// Genome: 30 chars occupying physical keyboard slots by index; `` ` `` = empty slot.
pub type KeysGenome = Vec<char>;

/// Individual in the GA population.
pub type KeysIndividual = darwin::Individual<char, ScoreResult>;

/// GA context for layout optimization.
pub type GaContext<'a> =
    darwin::Context<'a, char, (LayoutEvaluator, cliffa::cli::AppHandle), ScoreResult>;
