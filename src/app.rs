use crate::{
    Config, LayoutEvaluator, Mode,
    models::{Keyboard, Layout, ScoreResult},
};
use cliffa::cli::AppHandle;
use itertools::Itertools;
use miette::{Context, IntoDiagnostic, Result};
use rayon::prelude::*;
use std::io::Write;
use tracing::{info, trace};

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    let keyboard = Keyboard::load(cfg.keyboard.unwrap())?;

    let words = std::fs::read_to_string(cfg.text.unwrap())
        .into_diagnostic()
        .wrap_err("Failed to read text file")?
        .split_whitespace()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let words_ref = words.iter().map(|s| s.as_str()).collect_vec();

    match cfg.mode {
        Mode::Evaluate => {
            let evaluator = LayoutEvaluator::new(&keyboard);

            let layouts_path = cfg.layouts.unwrap();
            let layouts = Layout::load(&layouts_path);
            info!("Loaded {} layouts", layouts.len());

            let mut scored: Vec<_> = layouts
                .par_iter()
                .filter_map(|layout| {
                    if app.should_finish() {
                        return None;
                    }
                    Some((layout, evaluator.score_corpus(&words_ref, &layout.keys)))
                })
                .collect();

            scored.sort_by(|a, b| {
                a.1.fitness
                    .partial_cmp(&b.1.fitness)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            scored.iter().take(10).for_each(|(layout, layout_score)| {
                info!("{} {}", layout.name, layout_score);
            });

            let mut file = std::fs::File::create(&layouts_path)
                .into_diagnostic()
                .wrap_err("Failed to open layouts file for writing")?;

            writeln!(
                file,
                "layout_1;layout_2;layout_3;layout_4;layout_5;layout_6;{}",
                ScoreResult::csv_header()
            )
            .into_diagnostic()
            .wrap_err("Failed to write evaluated layouts header")?;

            for (layout, layout_score) in scored {
                writeln!(file, "{};{}", layout.name, layout_score.to_csv())
                    .into_diagnostic()
                    .wrap_err("Failed to write evaluated layouts")?;
            }
        }
        _ => unimplemented!("Only evaluation mode is implemented currently."),
    }

    Ok(())
}
