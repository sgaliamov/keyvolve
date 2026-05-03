use crate::{LayoutEvaluator, models::KeyPos};
use cliffa::cli::AppHandle;
use darwin::{GeneticAlgorithm, NoopCrossover};
use miette::Result;

pub fn optimize(
    evaluator: LayoutEvaluator,
    config: darwin::Config<KeyPos>,
    app: AppHandle,
) -> Result<()> {
    let mut ga = GeneticAlgorithm::new(
        &config,
        super::generate,
        super::mutate,
        NoopCrossover,
        super::corpus_evaluator,
        super::callback,
    );

    GeneticAlgorithm::set_state(&mut ga, (evaluator, app));

    ga.run();

    Ok(())
}
