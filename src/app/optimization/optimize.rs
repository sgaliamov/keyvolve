use crate::app::LayoutEvaluator;
use crate::app::OptimizationConfig;
use crate::models::Layout;
use cliffa::cli::AppHandle;
use darwin::{GeneticAlgorithm, NoopCrossover};
use miette::Result;

use super::{OptimizerState, callback, corpus_evaluator, generate, mutate};

pub fn optimize(
    evaluator: LayoutEvaluator,
    config: darwin::Config<char>,
    app: AppHandle,
    optimization: OptimizationConfig,
) -> Result<()> {
    use tracing::info;
    info!("Initializing genetic algorithm");
    let mut ga = GeneticAlgorithm::new(
        config,
        generate,
        mutate,
        NoopCrossover,
        corpus_evaluator,
        callback,
    );

    GeneticAlgorithm::set_state(
        &mut ga,
        OptimizerState {
            cache: optimization.cache(),
            evaluator,
            app,
            optimization,
        },
    );

    info!("Running genetic algorithm");
    let pools = ga.run();
    info!("Algorithm complete");

    let mut top: Vec<(usize, &darwin::Genome<char>, f64)> = pools
        .iter()
        .flat_map(|pool| {
            pool.individuals
                .iter()
                .map(|ind| (pool.number, &ind.genome, ind.fitness))
        })
        .collect();
    top.sort_by(|a, b| b.2.total_cmp(&a.2));

    println!("\n--- top 10 ---");
    for (pool, genome, fitness) in top.into_iter().take(10) {
        let name = Layout::from_keys(genome).to_string();
        println!("\"{name};{fitness:.4};{pool}\",");
    }

    Ok(())
}
