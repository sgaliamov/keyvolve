use crate::app::{LayoutEvaluator, OptimizationConfig, Side, write_layouts};
use crate::models::{Layout, ScoreResult};
use cliffa::cli::AppHandle;
use darwin::{GeneticAlgorithm, Individual, NoopCrossover};
use itertools::Itertools;
use miette::Result;
use rayon::prelude::*;

use super::{OptimizerState, callback, evaluator as evaluator_fn, generate, mutate};

/// Physical slot indices for the home (middle) row — left 5–9, right 20–24.
const HOME_ROW: [usize; 10] = [5, 6, 7, 8, 9, 20, 21, 22, 23, 24];

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

    let top = top_by_home_row(pools, 10);

    let rows: Vec<_> = top
        .iter()
        .map(|(pool, ind)| to_output_row(*pool, ind))
        .collect();

    write_layouts(&rows, rows.len(), output_path.as_deref(), false, Side::Any)
}

/// Sorted chars at home-row slots — group identity.
fn home_row_key(genome: &[char]) -> [char; 10] {
    HOME_ROW.map(|i| genome[i])
}

/// Collect individuals grouped by home-row content, tiered by group rank.
///
/// Top `max_groups` groups by champion fitness; groups 0–1 → 8 picks, 2–3 → 4, rest → 2.
fn top_by_home_row(
    pools: &darwin::Pools<char, ScoreResult>,
    max_groups: usize,
) -> Vec<(usize, &Individual<char, ScoreResult>)> {
    // Parallel collect all scored individuals, tagged with pool number.
    let all: Vec<(usize, &Individual<char, ScoreResult>)> = pools
        .par_iter()
        .flat_map_iter(|p| {
            p.individuals
                .iter()
                .filter(|ind| ind.fitness.is_finite())
                .map(|ind| (p.number, ind))
        })
        .collect();

    // Group by home-row fingerprint; sort within groups in parallel.
    let mut groups: Vec<Vec<_>> = all
        .into_iter()
        .into_group_map_by(|(_, ind)| home_row_key(&ind.genome))
        .into_values()
        .collect();

    groups.par_iter_mut().for_each(|g| {
        g.sort_unstable_by(|a, b| b.1.fitness.total_cmp(&a.1.fitness));
    });

    // Sort groups by their champion, keep top `max_groups`.
    groups.sort_unstable_by(|a, b| b[0].1.fitness.total_cmp(&a[0].1.fitness));
    groups.truncate(max_groups);

    // Tier-based extraction with cross-group dedup.
    groups
        .iter()
        .enumerate()
        .flat_map(|(i, g)| {
            let n = match i {
                0 | 1 => 6,
                2 | 3 => 4,
                _ => 2,
            };
            g.iter().take(n).copied()
        })
        .unique_by(|(_, ind)| &ind.genome)
        .collect()
}

fn to_output_row(
    pool: usize,
    individual: &Individual<char, ScoreResult>,
) -> (Layout, ScoreResult, usize) {
    let score = individual.state.as_ref().unwrap().clone();
    (Layout::from_keys(&individual.genome), score, pool)
}
