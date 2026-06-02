use crate::app::synthesise::{
    SynthesiseConfig,
    corpus::build_corpus,
    counter::{CorpusStats, calculate_stats},
    digraph::{
        count_corpus_letters, filter_and_scale, read_counts, read_counts_csv, read_letter_counts,
        read_letter_counts_csv, write_bigrams, write_bigrams_aggregated,
        write_letter_freq_combined,
    },
    shared::{
        CachedSourceStats, read_stats_cache, report_path, score_with_filter, stats_cache_path,
        write_corpus, write_report, write_stats_cache,
    },
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
    let stats_dir = cfg
        .stats_dir()
        .wrap_err("Synthesise mode requires `synthesise.output` path to resolve stats dir")?;
    let bigrams_name = format!("{src_stem}.bigrams.csv");
    let bigrams_path = stats_dir.join(bigrams_name);

    tracing::info!(input = %input.display(), "Reading digraph counts");
    let counts = if bigrams_path.exists() {
        tracing::info!(csv = %bigrams_path.display(), "Using saved bigram stats");
        read_counts_csv(&bigrams_path)?
    } else {
        read_counts(input)?
    };
    tracing::debug!(unique_pairs = counts.len(), "Counts loaded");

    let scaled = filter_and_scale(&counts, cfg.min_frequency, cfg.digraph.target);
    tracing::debug!(
        pairs_kept = scaled.len(),
        min_frequency = cfg.min_frequency,
        target = cfg.digraph.target,
        "Digraphs filtered and scaled"
    );
    if !bigrams_path.exists() {
        write_bigrams(&scaled, &counts, cfg.min_frequency, &bigrams_path)?;
        tracing::debug!(csv = %bigrams_path.display(), "CSV written");
    }

    let aggregated_path = stats_dir.join(format!("{src_stem}.bigrams.aggregated.csv"));
    if !aggregated_path.exists() {
        write_bigrams_aggregated(&scaled, &counts, cfg.min_frequency, &aggregated_path)?;
        tracing::debug!(csv = %aggregated_path.display(), "Aggregated CSV written");
    }

    tracing::info!("Building corpus");
    let words = build_corpus(&scaled, cfg.digraph.max_word_len);
    write_corpus(&words, output)?;

    let letter_freq_path = stats_dir.join(format!("{src_stem}.letters.csv"));

    let orig_letters = if letter_freq_path.exists() {
        tracing::info!(csv = %letter_freq_path.display(), "Using saved letter stats");
        read_letter_counts_csv(&letter_freq_path)?
    } else {
        let counts = read_letter_counts(input)?;
        let synth_letters = count_corpus_letters(&words);
        write_letter_freq_combined(&counts, &synth_letters, &letter_freq_path)?;
        tracing::debug!(csv = %letter_freq_path.display(), "Letter frequencies written");
        counts
    };

    // Score generated corpus against source stats.
    // Use only the kept (filtered) bigrams as source reference — rare pairs were
    // intentionally discarded, so including them would inflate error.
    let cache_path = stats_cache_path(input, &stats_dir);
    let source_stats = if cache_path.exists() {
        tracing::info!(cache = %cache_path.display(), "Using saved source stats");
        read_stats_cache(&cache_path)?.stats
    } else {
        let kept_bigrams: rustc_hash::FxHashMap<[char; 2], u64> = scaled
            .iter()
            .filter(|(_, n)| *n > 0)
            .map(|(pair, _)| (*pair, counts.get(pair).copied().unwrap_or(0)))
            .collect();
        let kept_total: u64 = kept_bigrams.values().sum();
        let total_letters: u64 = orig_letters.values().sum();
        let stats = CorpusStats {
            bigrams: kept_bigrams
                .iter()
                .map(|(&k, &v)| (k, v as f64 / kept_total.max(1) as f64))
                .collect(),
            letters: orig_letters
                .iter()
                .map(|(&k, &v)| (k, v as f64 / total_letters.max(1) as f64))
                .collect(),
            first_letters: rustc_hash::FxHashMap::default(),
            average_word_length: 0.0,
        };
        let word_count = words.len();
        write_stats_cache(
            &cache_path,
            &CachedSourceStats {
                stats: stats.clone(),
                word_count,
            },
        )?;
        tracing::info!(cache = %cache_path.display(), "Saved source stats cache");
        stats
    };
    let generated_stats = calculate_stats(&words);
    let score = score_with_filter(&source_stats, &generated_stats, 0.0);

    let report = report_path(output);
    write_report(&report, &score, 0, words.len())?;

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
