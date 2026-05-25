use crate::app::{LayoutEvaluator, OptimizationConfig, write_layouts};
use crate::models::{Layout, ScoreResult};
use cliffa::cli::AppHandle;
use darwin::{GeneticAlgorithm, Individual, NoopCrossover, Pool};
use itertools::Itertools;
use miette::Result;

use super::{OptimizerState, callback, evaluator as evaluator_fn, generate, mutate};

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
        evaluator_fn,
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

    let pools = &pools;

    let top: Vec<_> = pools
        .best_n(10)
        .into_iter()
        .chain(top_from_top_pools(pools, 10))
        .unique_by(|ind| &ind.genome)
        .map(to_output_row)
        .collect();

    write_layouts(&top, top.len(), output_path.as_deref(), false)
}

fn top_from_top_pools(
    pools: &darwin::Pools<char, ScoreResult>,
    count: usize,
) -> Vec<&Individual<char, ScoreResult>> {
    pools
        .iter()
        .sorted_unstable_by(|a, b| compare_pools(a, b))
        .take(count)
        .flat_map(|pool| pool.best_n(1))
        .sorted_unstable_by(|a, b| b.fitness.total_cmp(&a.fitness))
        .take(count)
        .collect()
}

fn compare_pools(a: &Pool<char, ScoreResult>, b: &Pool<char, ScoreResult>) -> std::cmp::Ordering {
    best_fitness(b).total_cmp(&best_fitness(a))
}

fn to_output_row(individual: &Individual<char, ScoreResult>) -> (Layout, ScoreResult) {
    let score = individual.state.as_ref().unwrap().clone();
    (Layout::from_keys(&individual.genome), score)
}

fn best_fitness(pool: &Pool<char, ScoreResult>) -> f64 {
    pool.best_n(1)
        .into_iter()
        .next()
        .map(|ind| ind.fitness)
        .unwrap_or(f64::NEG_INFINITY)
}
