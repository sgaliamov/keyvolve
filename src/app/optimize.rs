use crate::{
    LayoutEvaluator,
    models::{KeyPos, Layout},
};
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
        super::generate_keys_genome,
        super::mutate_keys_genome,
        NoopCrossover,
        super::corpus_evaluator,
        super::optimize_callback,
    );

    GeneticAlgorithm::set_state(&mut ga, evaluator);

    ga.run();

    Ok(())
}
