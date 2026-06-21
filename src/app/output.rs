use crate::models::{Layout, ScoreResult};
use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashSet;
use std::{fs::OpenOptions, io::Write, path::Path};
use tracing::info;

/// Print top N layouts and optionally append them to a CSV file.
/// Creates the file with a header row if it doesn't exist yet.
pub fn write_layouts(
    layouts: &[(Layout, ScoreResult, usize)],
    to_print: usize,
    output_path: Option<&Path>,
    overwrite: bool,
) -> Result<()> {
    // Drop hand-swapped mirror reflections: each pair shares fitness, keep one.
    let mut seen = FxHashSet::default();
    let layouts: Vec<_> = layouts
        .iter()
        .filter(|(layout, _, _)| seen.insert(layout.mirror_key()))
        .collect();

    for (layout, score, pool) in layouts.iter().copied().take(to_print) {
        println!("[pool {pool:>2}] {layout} | {score}");
    }

    let Some(path) = output_path else {
        return Ok(());
    };

    let is_new =
        overwrite || !path.exists() || path.metadata().map(|m| m.len() == 0).unwrap_or(true);

    let mut file = OpenOptions::new()
        .create(true)
        .write(overwrite)
        .truncate(overwrite)
        .append(!overwrite)
        .open(path)
        .into_diagnostic()
        .wrap_err("Failed to open layouts file")?;

    if is_new {
        writeln!(
            file,
            "keys_1, keys_2, keys_3, keys_4, keys_5, keys_6, {}",
            ScoreResult::csv_header()
        )
        .into_diagnostic()
        .wrap_err("Failed to write header")?;
    }

    for (layout, score, _) in layouts {
        writeln!(file, "{layout}, {}", score.to_csv())
            .into_diagnostic()
            .wrap_err("Failed to write layout row")?;
    }

    info!("Results written to {}", path.display());
    Ok(())
}
