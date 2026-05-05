pub mod config;
pub mod graph;
mod count;
mod io;

pub use config::*;
use graph::build_corpus;
use io::{filter_and_scale, read_counts, read_scaled_csv, write_scaled_csv, write_corpus};
use miette::{Context, Result};
use std::path::Path;

/// Run the full synthesise pipeline.
pub fn run(input: &Path, cfg: SynthesiseConfig) -> Result<()> {
    let output = cfg
        .output
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.output` path")?;
    let csv_path = output.with_extension("csv");

    let scaled = if csv_path.exists() {
        tracing::info!(csv = %csv_path.display(), "Using cached digraph CSV");
        read_scaled_csv(&csv_path)?
    } else {
        let counts = read_counts(input)?;
        let scaled = filter_and_scale(&counts, cfg.min_freq, cfg.target);
        write_scaled_csv(&scaled, &csv_path)?;
        scaled
    };

    let words = build_corpus(&scaled);
    write_corpus(&words, &output.with_extension("txt"))?;

    tracing::info!(
        csv = %csv_path.display(),
        corpus = %output.with_extension("txt").display(),
        words = words.len(),
        "Synthesise complete"
    );
    Ok(())
}
