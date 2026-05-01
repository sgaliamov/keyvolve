use crate::{
    Config, LayoutEvaluator, Mode,
    models::{KeyPos, Keyboard, KeysGenome, Layout, ScoreResult},
};
use cliffa::cli::AppHandle;
use darwin::{Context as GaContext, Evaluator, GeneticAlgorithm, Individual};
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
    let evaluator_fn = CorpusEvaluator { words: &words };

    let mut ga = GeneticAlgorithm::new(
        &config,
        generate_keys_genome,
        mutate_keys_genome,
        crossover_keys_genomes,
        evaluator_fn,
        optimize_callback,
    );

    GeneticAlgorithm::set_state(&mut ga, evaluator);

    // ga.run();

    Ok(())
}

/// Generate a genome for optimization.
fn generate_keys_genome(_: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>) -> KeysGenome {
    todo!()
}

/// Mutate a candidate genome.
fn mutate_keys_genome(
    _: &Individual<KeyPos, ScoreResult>,
    _: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>,
) -> Option<KeysGenome> {
    todo!()
}

/// Cross two parent genomes.
fn crossover_keys_genomes(
    _: &Individual<KeyPos, ScoreResult>,
    _: &Individual<KeyPos, ScoreResult>,
    _: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>,
) -> Vec<KeysGenome> {
    todo!()
}

/// Evaluate one individual against the corpus.
struct CorpusEvaluator<'a> {
    words: &'a [&'a str],
}

impl Evaluator<KeyPos, LayoutEvaluator, ScoreResult> for CorpusEvaluator<'_> {
    fn evaluate(
        &self,
        ind: &Individual<KeyPos, ScoreResult>,
        ctx: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>,
    ) -> (f64, Option<ScoreResult>) {
        let layout = Layout::from_keys(&ind.genome);
        let score = ctx
            .state
            .as_ref()
            .expect("GA evaluator state must be set before optimize run")
            .score_corpus(self.words, &layout.keys);
        (-score.fitness, Some(score))
    }
}

/// Progress callback for optimize mode.
fn optimize_callback(ctx: &GaContext<'_, KeyPos, LayoutEvaluator, ScoreResult>) {
    let _ = ctx;
    todo!()
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
