use crate::{LayoutEvaluator, models::{KeyPos, Layout, ScoreResult}};
use darwin::{Context as GaContext, Evaluator, Individual};

/// Evaluate one individual against the corpus.
pub struct CorpusEvaluator<'a> {
    pub words: &'a [&'a str],
}

impl Evaluator<KeyPos, LayoutEvaluator, ScoreResult> for CorpusEvaluator<'_> {
    fn evaluate(
        &self,
        ind: &Individual<KeyPos, ScoreResult>,
        ctx: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>,
    ) -> (f64, Option<ScoreResult>) {
        let layout = Layout::from_keys(&ind.genome);
        let score = ctx
            .state
            .as_ref()
            .expect("GA evaluator state must be set before optimize run")
            .score_corpus(self.words, &layout.keys);
        (-score.fitness, Some(score))
    }
}
