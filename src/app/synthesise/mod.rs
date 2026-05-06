pub mod config;
mod corpus;
mod counter;
mod digraph;

pub use config::*;
use corpus::build_corpus;
use digraph::{
    count_corpus_letters, filter_and_scale, read_counts, read_letter_counts, write_bigrams,
    write_letter_freq_combined,
};
use miette::{Context, IntoDiagnostic, Result};
use std::{io::Write, path::Path};

/// Run the full synthesise pipeline.
pub fn synthesise(input: &Path, cfg: SynthesiseConfig) -> Result<()> {
    let output = cfg
        .output
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.output` path")?;
    let src_stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let bigrams_name = format!("{src_stem}.bigrams.csv");
    let bigrams_path = output
        .parent()
        .unwrap_or(output)
        .join("stats")
        .join(bigrams_name);

    tracing::info!(input = %input.display(), "Reading digraph counts");
    let counts = read_counts(input)?;
    tracing::debug!(unique_pairs = counts.len(), "Counts loaded");

    let scaled = filter_and_scale(&counts, cfg.min_frequency, cfg.target);
    tracing::debug!(
        pairs_kept = scaled.len(),
        min_frequency = cfg.min_frequency,
        target = cfg.target,
        "Digraphs filtered and scaled"
    );
    write_bigrams(&scaled, &counts, cfg.min_frequency, &bigrams_path)?;
    tracing::debug!(csv = %bigrams_path.display(), "CSV written");

    tracing::info!("Building corpus");
    let words = build_corpus(&scaled, cfg.max_word_len);
    let corpus_path = output.with_extension("txt");
    write_corpus(&words, &corpus_path)?;

    // Letter frequencies: original vs synthesised in one CSV.
    let freq_dir = output.parent().unwrap_or(output);
    let letter_freq_path = freq_dir
        .join("stats")
        .join(format!("{src_stem}.letters.csv"));

    let orig_letters = read_letter_counts(input)?;
    let synth_letters = count_corpus_letters(&words);
    write_letter_freq_combined(&orig_letters, &synth_letters, &letter_freq_path)?;
    tracing::debug!(csv = %letter_freq_path.display(), "Letter frequencies written");

    tracing::info!(
        csv = %bigrams_path.display(),
        corpus = %corpus_path.display(),
        letter_freq = %letter_freq_path.display(),
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
