use crate::app::{GaContext, KeysIndividual};
use crate::models::{Layout, ScoreResult};

type CorpusEvalResult = (f64, Option<ScoreResult>);

/// Evaluate a genome candidate against the stored corpus.
pub fn corpus_evaluator(ind: &KeysIndividual, ctx: &GaContext) -> CorpusEvalResult {
    let layout = Layout::from_keys(&ind.genome);
    let score = ctx
        .state
        .as_ref()
        .expect("GA evaluator state must be set before optimize run")
        .0
        .score_corpus(&layout.keys);

    (score.fitness, Some(score))
}
