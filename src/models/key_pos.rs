use crate::{app::LayoutEvaluator, models::ScoreResult};

// todo: move to the optimization module
/// Genome: 30 chars occupying physical keyboard slots by index; `_` = empty slot.
pub type KeysGenome = Vec<char>;

/// Individual in the GA population.
pub type KeysIndividual = darwin::Individual<char, ScoreResult>;

/// GA context for layout optimization.
pub type GaContext<'a> =
    darwin::Context<'a, char, (LayoutEvaluator, cliffa::cli::AppHandle), ScoreResult>;
