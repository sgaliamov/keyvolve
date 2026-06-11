use crate::app::{GaContext, KeysIndividual};
use crate::models::{Layout, ScoreResult};

type CorpusEvalResult = (f64, Option<ScoreResult>);

/// Evaluate a genome candidate against the stored corpus.
/// Genomes violating placement constraints (stale seeds/dump, starved fallback
/// placements) are rejected with `NEG_INFINITY` so they never breed or win.
pub fn evaluator(ind: &KeysIndividual, ctx: &GaContext) -> CorpusEvalResult {
    let state = ctx
        .state
        .as_ref()
        .expect("GA evaluator state must be set before optimize run");

    if !state.optimization.is_genome_valid(&ind.genome) {
        return (f64::NEG_INFINITY, None);
    }

    let layout = Layout::from_keys(&ind.genome);
    let score = state.evaluator.score_corpus(&layout.keys);
    (score.fitness, Some(score))
}
