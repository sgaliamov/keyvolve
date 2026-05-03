use crate::app::LayoutEvaluator;
use cliffa::cli::AppHandle;
use darwin::{GeneticAlgorithm, NoopCrossover};
use miette::Result;

use super::{callback, corpus_evaluator, generate, mutate};

pub fn optimize(
    evaluator: LayoutEvaluator,
    config: darwin::Config<char>,
    app: AppHandle,
) -> Result<()> {
    let mut ga = GeneticAlgorithm::new(
        &config,
        generate,
        mutate,
        NoopCrossover,
        corpus_evaluator,
        callback,
    );

    GeneticAlgorithm::set_state(&mut ga, (evaluator, app));

    ga.run();

    Ok(())
}
