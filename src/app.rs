use crate::{
    Config, LayoutEvaluator, Mode,
    models::{Keyboard, KeysGenome, Layout, ScoreResult},
};
use cliffa::cli::AppHandle;
use darwin::{Context, GeneticAlgorithm, Individual};
use itertools::Itertools;
use miette::{Context, IntoDiagnostic, Result};
use rayon::prelude::*;
use std::io::Write;
use tracing::{info, trace};

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    let words = std::fs::read_to_string(cfg.text.unwrap())
        .into_diagnostic()
        .wrap_err("Failed to read text file")?
        .split_whitespace()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let words_ref = words.iter().map(|s| s.as_str()).collect_vec();

    let layouts_path = cfg.layouts.wrap_err("Missing layouts path in config")?;
    let layouts = Layout::load(&layouts_path);
    info!("Loaded {} layouts", layouts.len());

    let keyboard = Keyboard::load(cfg.keyboard.unwrap())?;
    let evaluator = LayoutEvaluator::new(&keyboard);

    match cfg.mode {
        Mode::Evaluate => {
            evaluate(evaluator, words_ref, &layouts, &layouts_path, app)?;
        }
        Mode::Optimize => {
            optimize(evaluator, words_ref, &layouts, cfg.ga, app)?;
        }
    }

    Ok(())
}

fn optimize(
    evaluator: LayoutEvaluator,
    words: Vec<&str>,
    layouts: &[Layout],
    config: darwin::Config<crate::models::KeyPos>,
    app: AppHandle,
) -> Result<()> {
    let generator_fn = |_| -> KeysGenome { todo!() };

    let mutator = |ind, ctx| -> Option<KeysGenome> { todo!() };

    let crossover = |dad, mom, ctx| -> Vec<KeysGenome> { todo!() };

    let evaluator_fn = |ind: &Individual<_, _>,
                        ctx: &darwin::Context<'_, KeyPos, LayoutEvaluator, ScoreResult>|
     -> (f64, Option<ScoreResult>) {
        let layout = Layout::from_keys(&ind.genome);
        let score = ctx.state.unwrap().score_corpus(&words, &layout.keys);
        (-score.fitness, Some(score))
    };

    let callback = |ctx: &Context<'_, crate::models::KeyPos, LayoutEvaluator, ScoreResult>| {
        let _ = ctx;
        todo!()
    };

    let mut ga = GeneticAlgorithm::new(
        &config,
        generator_fn,
        mutator,
        crossover,
        evaluator_fn,
        callback,
    );

    ga.set_state(evaluator);

    // ga.run();

    Ok(())
}

fn evaluate(
    evaluator: LayoutEvaluator,
    words: Vec<&str>,
    layouts: &Vec<Layout>,
    layouts_path: impl AsRef<std::path::Path>,
    app: AppHandle,
) -> Result<()> {
    let mut scored: Vec<_> = layouts
        .par_iter()
        .filter_map(|layout| {
            if app.should_finish() {
                return None;
            }
            Some((layout, evaluator.score_corpus(&words, &layout.keys)))
        })
        .collect();
    scored.sort_by(|a, b| {
        a.1.fitness
            .partial_cmp(&b.1.fitness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.iter().take(10).for_each(|(layout, layout_score)| {
        info!("{} {}", layout.name(), layout_score);
    });
    let mut file = std::fs::File::create(&layouts_path)
        .into_diagnostic()
        .wrap_err("Failed to open layouts file for writing")?;
    writeln!(
        file,
        "keys_1;keys_2;keys_3;keys_4;keys_5;keys_6;{}",
        ScoreResult::csv_header()
    )
    .into_diagnostic()
    .wrap_err("Failed to write evaluated layouts header")?;
    Ok(for (layout, layout_score) in scored {
        writeln!(file, "{};{}", layout.name(), layout_score.to_csv())
            .into_diagnostic()
            .wrap_err("Failed to write evaluated layouts")?;
    })
}
