use crate::{app::LayoutEvaluator, models::ScoreResult, OptimizationConfig};
use cliffa::cli::AppHandle;
use rustc_hash::FxHashSet;

/// Genome: 30 chars occupying physical keyboard slots by index; `` ` `` = empty slot.
pub type KeysGenome = Vec<char>;

/// Individual in the GA population.
pub type KeysIndividual = darwin::Individual<char, ScoreResult>;

/// Shared state threaded through all GA callbacks.
pub struct OptimizerState {
    pub evaluator: LayoutEvaluator,
    pub app: AppHandle,
    pub optimization: OptimizationConfig,
    /// Physical key indices unavailable for placement (sourced from keyboard config).
    pub blocked: FxHashSet<u8>,
}

/// GA context for layout optimization.
pub type GaContext<'a> =
    darwin::Context<'a, char, OptimizerState, ScoreResult>;
