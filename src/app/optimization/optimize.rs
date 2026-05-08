use crate::OptimizationConfig;
use crate::app::LayoutEvaluator;
use crate::models::Layout;
use cliffa::cli::AppHandle;
use darwin::{GeneticAlgorithm, NoopCrossover};
use miette::Result;
use rustc_hash::FxHashSet;

use super::{OptimizerState, callback, corpus_evaluator, generate, mutate};

pub fn optimize(
    evaluator: LayoutEvaluator,
    config: darwin::Config<char>,
    app: AppHandle,
    optimization: OptimizationConfig,
    blocked: FxHashSet<u8>,
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
            evaluator,
            app,
            optimization,
            blocked,
        },
    );

    info!("Running genetic algorithm");
    let pools = ga.run();
    info!("Algorithm complete");

    println!("\n--- top 10 ---");
    for (genome, fitness)  in pools.best_n(10).into_iter() {
        let name = Layout::from_keys(genome).to_string();
        println!("\"{};{:.4}\",", name, fitness);
    }

    Ok(())
}
