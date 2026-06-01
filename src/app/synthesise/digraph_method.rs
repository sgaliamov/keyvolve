use crate::app::synthesise::{
    SynthesiseConfig,
    corpus::build_corpus,
    digraph::{
        count_corpus_letters, filter_and_scale, read_counts, read_letter_counts, write_bigrams,
        write_bigrams_aggregated, write_letter_freq_combined,
    },
    shared::{report_path, write_corpus, write_report_words},
};
use miette::{Context, Result};

/// Run the original digraph-based synthesise pipeline.
pub(super) fn synthesise_digraph(cfg: SynthesiseConfig) -> Result<()> {
    let input = cfg
        .text
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.text` path")?;
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

    let aggregated_path = output
        .parent()
        .unwrap_or(output)
        .join("stats")
        .join(format!("{src_stem}.bigrams.aggregated.csv"));
    write_bigrams_aggregated(&scaled, &counts, cfg.min_frequency, &aggregated_path)?;
    tracing::debug!(csv = %aggregated_path.display(), "Aggregated CSV written");

    tracing::info!("Building corpus");
    let words = build_corpus(&scaled, cfg.max_word_len);
    write_corpus(&words, output)?;

    let report = report_path(output);
    write_report_words(&report, words.len())?;
    tracing::info!(
        report = %report.display(),
        words = words.len(),
        method = "digraph",
        "Report written"
    );

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
        aggregated_csv = %aggregated_path.display(),
        corpus = %output.display(),
        letter_freq = %letter_freq_path.display(),
        words = words.len(),
        method = "digraph",
        "Synthesise complete"
    );
    Ok(())
}
