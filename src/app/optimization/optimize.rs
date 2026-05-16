use crate::app::LayoutEvaluator;
use crate::app::OptimizationConfig;
use crate::models::Layout;
use cliffa::cli::AppHandle;
use darwin::{GeneticAlgorithm, NoopCrossover};
use itertools::Itertools;
use miette::Context;
use miette::IntoDiagnostic;
use miette::Result;
use std::io::Write;

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

    info!("Running genetic algorithm");
    let pools = ga.run();
    info!("Algorithm complete");

    println!("\n--- top 10 ---");
    let top = pools
        .iter()
        .flat_map(|p| {
            p.individuals
                .iter()
                .sorted_unstable_by(|a, b| b.fitness.total_cmp(&a.fitness))
                .take(1)
        })
        .sorted_unstable_by(|a, b| b.fitness.total_cmp(&a.fitness))
        .take(10);

    let mut file = output_path
        .as_ref()
        .and_then(|o| std::fs::File::create(o).ok());

    for ind in top {
        let score = ind.state.as_ref().unwrap();
        let genome = &ind.genome;
        let layout = Layout::from_keys(genome).to_string();
        let line = format!("{}, {}", layout, score.to_csv());
        println!("{line}");

        if let Some(ref mut file) = file {
            writeln!(file, "{line}")
                .into_diagnostic()
                .wrap_err("Failed to write evaluated layouts")?;
        }
    }

    if let Some(output) = output_path {
        info!("Results written to {}", output.display());
    }

    Ok(())
}
