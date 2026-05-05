pub mod config;
mod graph;
mod count;

pub use config::*;
use graph::build_corpus;
use count::{filter_and_scale, read_counts, read_scaled_csv, write_scaled_csv};
use miette::{Context, IntoDiagnostic, Result};
use std::{io::Write, path::Path};

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

/// Write space-separated fake words to a text file.
fn write_corpus(words: &[String], path: &Path) -> Result<()> {
    let mut out = std::fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create corpus output file")?;
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            out.write_all(b" ").into_diagnostic()?;
        }
        out.write_all(word.as_bytes()).into_diagnostic()?;
    }
    out.write_all(b"\n").into_diagnostic()?;
    Ok(())
}
