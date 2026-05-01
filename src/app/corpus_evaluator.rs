use crate::{LayoutEvaluator, models::{KeyPos, Layout, ScoreResult}};
use darwin::{Context as GaContext, Individual};

type CorpusEvalResult = (f64, Option<ScoreResult>);

/// Build the corpus evaluator closure used by optimize mode.
pub fn corpus_evaluator(
    words: Vec<String>,
) -> impl Fn(&Individual<KeyPos, ScoreResult>, &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>) -> CorpusEvalResult
       + Send
       + Sync {
    move |
        ind: &Individual<KeyPos, ScoreResult>,
        ctx: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>,
    | {
        let layout = Layout::from_keys(&ind.genome);
        let words = words.iter().map(String::as_str).collect::<Vec<_>>();
        let score = ctx
            .state
            .as_ref()
            .expect("GA evaluator state must be set before optimize run")
            .score_corpus(&words, &layout.keys);
        (-score.fitness, Some(score))
    }
}
