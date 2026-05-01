use crate::{LayoutEvaluator, models::{KeyPos, Layout}};
use cliffa::cli::AppHandle;
use darwin::GeneticAlgorithm;
use miette::Result;

pub fn optimize(
    evaluator: LayoutEvaluator,
    layouts: &[Layout],
    config: darwin::Config<KeyPos>,
    app: AppHandle,
) -> Result<()> {
    let _ = (layouts, app);

    let mut ga = GeneticAlgorithm::new(
        &config,
        super::generate_keys_genome,
        super::mutate_keys_genome,
        super::crossover_keys_genomes,
        super::corpus_evaluator,
        super::optimize_callback,
    );

    GeneticAlgorithm::set_state(&mut ga, evaluator);

    // ga.run();

    Ok(())
}

