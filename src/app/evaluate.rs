use crate::{
    app::LayoutEvaluator,
    models::{Layout, ScoreResult},
};
use cliffa::cli::AppHandle;
use miette::{Context, IntoDiagnostic, Result};
use rayon::prelude::*;
use std::io::Write;
use tracing::info;

/// Evaluate the layouts and write results to a file.
pub fn evaluate(
    evaluator: LayoutEvaluator,
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
            Some((layout, evaluator.score_corpus(&layout.keys)))
        })
        .collect();
    scored.sort_by(|a, b| {
        a.1.fitness
            .partial_cmp(&b.1.fitness)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.iter().take(10).for_each(|(layout, layout_score)| {
        info!("{} {}", layout, layout_score);
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

    for (layout, layout_score) in scored {
        writeln!(file, "{};{}", layout, layout_score.to_csv())
            .into_diagnostic()
            .wrap_err("Failed to write evaluated layouts")?;
    }

    Ok(())
}
