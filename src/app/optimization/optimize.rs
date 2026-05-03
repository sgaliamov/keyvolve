use crate::app::LayoutEvaluator;
use crate::models::Layout;
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

    let pools = ga.run();

    println!("\n--- top 10 ---");
    for (rank, (genome, fitness)) in pools.best_n(10).into_iter().enumerate() {
        let name = Layout::from_keys(genome).name();
        println!("{:>2}.  fit {:.4}  {}", rank + 1, fitness, name);
    }

    Ok(())
}
