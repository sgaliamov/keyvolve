use crate::app::{LayoutEvaluator, OptimizationConfig, write_layouts};
use crate::models::Layout;
use cliffa::cli::AppHandle;
use darwin::{GeneticAlgorithm, NoopCrossover};
use itertools::Itertools;
use miette::Result;

use super::{OptimizerState, callback, corpus_evaluator, generate, mutate};

pub fn optimize(
    evaluator: LayoutEvaluator,
    ga_cfg: darwin::Config<char>,
    opt_cfg: OptimizationConfig,
    app: AppHandle,
) -> Result<()> {
    use tracing::info;
    info!("Initializing genetic algorithm");
    let mut ga = GeneticAlgorithm::new(
        ga_cfg,
        generate,
        mutate,
        NoopCrossover,
        corpus_evaluator,
        callback,
    );

    let output_path = opt_cfg.output.clone();

    GeneticAlgorithm::set_state(
        &mut ga,
        OptimizerState {
            cache: opt_cfg.cache(),
            evaluator,
            app,
            optimization: opt_cfg,
        },
    );
    ga.seed();

    info!("Running genetic algorithm");
    let pools = ga.run();
    info!("Algorithm complete");

    let top: Vec<_> = pools
        .iter()
        .flat_map(|p| {
            p.individuals
                .iter()
                .sorted_unstable_by(|a, b| b.fitness.total_cmp(&a.fitness))
                .take(3)
        })
        .sorted_unstable_by(|a, b| b.fitness.total_cmp(&a.fitness))
        .take(20)
        .map(|ind| {
            let score = ind.state.as_ref().unwrap().clone();
            (Layout::from_keys(&ind.genome), score)
        })
        .collect();

    write_layouts(&top, 10, output_path.as_deref(), false)
}
