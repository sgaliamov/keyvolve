use crate::app::synthesise::{
    SynthesiseConfig,
    corpus::build_corpus,
    counter::{CorpusStats, calculate_stats},
    digraph::{
        count_corpus_letters, filter_and_scale, read_counts, read_counts_csv, read_letter_counts,
        read_letter_counts_csv, write_bigrams, write_bigrams_aggregated,
        write_letter_freq_combined,
    },
    shared::{report_path, score_with_filter, write_corpus, write_report},
};
use miette::{Context, Result};
use rustc_hash::FxHashMap;

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
    let counts = if bigrams_path.exists() {
        tracing::info!(csv = %bigrams_path.display(), "Using saved bigram stats");
        read_counts_csv(&bigrams_path)?
    } else {
        read_counts(input)?
    };
    tracing::debug!(unique_pairs = counts.len(), "Counts loaded");

    let scaled = filter_and_scale(&counts, cfg.digraph.min_frequency, cfg.digraph.target);
    tracing::debug!(
        pairs_kept = scaled.len(),
        min_frequency = cfg.digraph.min_frequency,
        target = cfg.digraph.target,
        "Digraphs filtered and scaled"
    );
    write_bigrams(&scaled, &counts, cfg.digraph.min_frequency, &bigrams_path)?;
    tracing::debug!(csv = %bigrams_path.display(), "CSV written");

    let aggregated_path = output
        .parent()
        .unwrap_or(output)
        .join("stats")
        .join(format!("{src_stem}.bigrams.aggregated.csv"));
    write_bigrams_aggregated(
        &scaled,
        &counts,
        cfg.digraph.min_frequency,
        &aggregated_path,
    )?;
    tracing::debug!(csv = %aggregated_path.display(), "Aggregated CSV written");

    tracing::info!("Building corpus");
    let words = build_corpus(&scaled, cfg.digraph.max_word_len);
    write_corpus(&words, output)?;

    let freq_dir = output.parent().unwrap_or(output);
    let letter_freq_path = freq_dir
        .join("stats")
        .join(format!("{src_stem}.letters.csv"));

    let orig_letters = if letter_freq_path.exists() {
        tracing::info!(csv = %letter_freq_path.display(), "Using saved letter stats");
        read_letter_counts_csv(&letter_freq_path)?
    } else {
        read_letter_counts(input)?
    };
    let synth_letters = count_corpus_letters(&words);
    write_letter_freq_combined(&orig_letters, &synth_letters, &letter_freq_path)?;
    tracing::debug!(csv = %letter_freq_path.display(), "Letter frequencies written");

    // Score generated corpus against source stats.
    // Use only the kept (filtered) bigrams as source reference — rare pairs were
    // intentionally discarded, so including them would inflate error.
    let kept_bigrams: FxHashMap<[char; 2], u64> = scaled
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(pair, _)| (*pair, counts.get(pair).copied().unwrap_or(0)))
        .collect();
    let kept_total: u64 = kept_bigrams.values().sum();
    let total_letters: u64 = orig_letters.values().sum();
    let source_stats = CorpusStats {
        bigrams: kept_bigrams
            .iter()
            .map(|(&k, &v)| (k, v as f64 / kept_total.max(1) as f64))
            .collect(),
        letters: orig_letters
            .iter()
            .map(|(&k, &v)| (k, v as f64 / total_letters.max(1) as f64))
            .collect(),
        first_letters: FxHashMap::default(),
        average_word_length: 0.0,
    };
    let generated_stats = calculate_stats(&words);
    let score = score_with_filter(&source_stats, &generated_stats, 0.0);

    let report = report_path(output);
    write_report(&report, &score, 0, words.len(), 0.01)?;

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
