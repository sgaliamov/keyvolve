use crate::modes::optimize::PlacementConstraints;
use crate::{evaluator::LayoutEvaluator, models::ScoreResult};
use cliffa::cli::AppHandle;
use std::sync::Arc;

/// Genome: 30 chars occupying physical keyboard slots by index; `` ` `` = empty slot.
pub type KeysGenome = Vec<char>;

/// Individual in the GA population.
pub type KeysIndividual = darwin::Individual<char, ScoreResult>;

/// Shared state threaded through all GA callbacks.
#[derive(Clone)]
pub struct OptimizerState {
    pub evaluator: LayoutEvaluator,
    pub app: AppHandle,
    pub constraints: Arc<PlacementConstraints>,
    pub mutation_count: usize,
}

/// GA context for layout optimization.
pub type GaContext<'a> = darwin::Context<'a, char, OptimizerState, ScoreResult>;
