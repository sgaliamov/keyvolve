use crate::models::{Layout, ScoreResult};
use crate::modes::optimize::{GaContext, KeysIndividual};

type CorpusEvalResult = (f64, Option<ScoreResult>);

/// Evaluate a genome candidate against the stored corpus.
/// Reject incomplete or invalid external genomes before any corpus lookup.
pub fn evaluator(ind: &KeysIndividual, ctx: &GaContext) -> CorpusEvalResult {
    let state = ctx
        .state
        .as_ref()
        .expect("GA evaluator state must be set before optimize run");

    if !state.constraints.is_genome_valid(&ind.genome) {
        return (f64::NEG_INFINITY, None);
    }

    let layout = Layout::from_keys(&ind.genome);
    let score = state.evaluator.score_corpus(&layout.keys);
    (score.fitness, Some(score))
}
