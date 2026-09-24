use crate::modes::synthesise::counter::{CorpusScore, CorpusStats, score_stats};
use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Cached source corpus stats written alongside the source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSourceStats {
    pub stats: CorpusStats,
    pub word_count: usize,
}

/// Load cached source stats.
pub fn read_stats_cache(path: &Path) -> Result<CachedSourceStats> {
    let text = fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err("Failed to read source stats cache")?;
    serde_json::from_str(&text)
        .into_diagnostic()
        .wrap_err("Failed to parse source stats cache")
}

/// Save cached source stats, creating parent directories as needed.
pub fn write_stats_cache(path: &Path, data: &CachedSourceStats) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err("Failed to create stats cache directory")?;
    }
    let json = serde_json::to_string_pretty(data)
        .into_diagnostic()
        .wrap_err("Failed to serialize source stats cache")?;
    fs::write(path, json)
        .into_diagnostic()
        .wrap_err("Failed to write source stats cache")
}

fn filter_frequency_map<K>(map: &mut FxHashMap<K, f64>, min_frequency: f64) {
    if min_frequency <= 0.0 {
        return;
    }

    map.retain(|_, frequency| *frequency >= min_frequency);
    let total: f64 = map.values().sum();
    if total > 0.0 {
        for value in map.values_mut() {
            *value /= total;
        }
    }
}

/// Drop entries below `min_frequency` from all normalized frequency maps and re-normalize.
pub fn filter_stats_frequencies(stats: &mut CorpusStats, min_frequency: f64) {
    if min_frequency <= 0.0 {
        return;
    }

    filter_frequency_map(&mut stats.letters, min_frequency);
    filter_frequency_map(&mut stats.bigrams, min_frequency);
    filter_frequency_map(&mut stats.trigrams, min_frequency);
    filter_frequency_map(&mut stats.first_letters, min_frequency);
}

/// Backward-compatible helper for bigram-only filtering.
#[allow(dead_code)]
pub fn filter_stats_bigrams(stats: &mut CorpusStats, min_frequency: f64) {
    filter_frequency_map(&mut stats.bigrams, min_frequency);
}

/// Score `candidate` against `source`, dropping low-frequency entries first.
pub fn score_with_filter(
    source: &CorpusStats,
    candidate: &CorpusStats,
    min_frequency: f64,
) -> CorpusScore {
    let mut filtered_source = source.clone();
    let mut filtered_candidate = candidate.clone();
    filter_stats_frequencies(&mut filtered_source, min_frequency);
    filter_stats_frequencies(&mut filtered_candidate, min_frequency);
    score_stats(&filtered_source, &filtered_candidate)
}

/// Write space-separated words to a text file, creating parent directories as needed.
pub(super) fn write_corpus(words: &[String], path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err("Failed to create corpus output directory")?;
    }
    let mut out = fs::File::create(path)
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

/// Report path next to corpus output, with method suffix.
pub(super) fn report_path(output: &Path) -> PathBuf {
    let stem = output
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    output
        .parent()
        .unwrap_or(output)
        .join(format!("{stem}.rpt"))
}

/// Write compact synth score report.
pub(super) fn write_report(
    path: &Path,
    score: &CorpusScore,
    source_words: usize,
    generated_words: usize,
) -> Result<()> {
    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create synth report")?;
    writeln!(out, "source_words={source_words}").into_diagnostic()?;
    writeln!(out, "generated_words={generated_words}").into_diagnostic()?;
    writeln!(out, "letters_error={:.2}%", score.letters * 100.0).into_diagnostic()?;
    writeln!(out, "bigrams_error={:.2}%", score.bigrams * 100.0).into_diagnostic()?;
    writeln!(
        out,
        "first_letters_error={:.2}%",
        score.first_letters * 100.0
    )
    .into_diagnostic()?;
    writeln!(
        out,
        "average_word_length_error={:.2}%",
        score.average_word_length * 100.0
    )
    .into_diagnostic()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_stats_frequencies_keeps_only_entries_above_minimum() {
        let mut stats = CorpusStats {
            letters: FxHashMap::from_iter([('a', 0.9), ('b', 0.09), ('c', 0.01)]),
            bigrams: FxHashMap::from_iter([
                (['a', 'a'], 0.8),
                (['a', 'b'], 0.1),
                (['b', 'c'], 0.09),
            ]),
            trigrams: FxHashMap::from_iter([
                (['a', 'a', 'a'], 0.7),
                (['a', 'a', 'b'], 0.2),
                (['a', 'b', 'c'], 0.1),
            ]),
            first_letters: FxHashMap::from_iter([('a', 0.8), ('b', 0.15), ('c', 0.05)]),
            average_word_length: 3.0,
        };

        filter_stats_frequencies(&mut stats, 0.1);

        assert_eq!(stats.letters.len(), 1);
        assert!(stats.letters.contains_key(&'a'));
        assert!(!stats.letters.contains_key(&'b'));
        assert!((stats.letters.values().sum::<f64>() - 1.0).abs() < 1e-9);

        assert_eq!(stats.bigrams.len(), 2);
        assert!(stats.bigrams.contains_key(&['a', 'a']));
        assert!(stats.bigrams.contains_key(&['a', 'b']));
        assert!(!stats.bigrams.contains_key(&['b', 'c']));
        assert!((stats.bigrams.values().sum::<f64>() - 1.0).abs() < 1e-9);

        assert_eq!(stats.first_letters.len(), 2);
        assert!(stats.first_letters.contains_key(&'a'));
        assert!(stats.first_letters.contains_key(&'b'));
        assert!(!stats.first_letters.contains_key(&'c'));
        assert!((stats.first_letters.values().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn score_with_filter_filters_both_distributions() {
        let source = CorpusStats {
            letters: FxHashMap::from_iter([('a', 0.8), ('b', 0.2)]),
            bigrams: FxHashMap::from_iter([(['a', 'a'], 0.8), (['a', 'b'], 0.2)]),
            trigrams: FxHashMap::default(),
            first_letters: FxHashMap::from_iter([('a', 1.0)]),
            average_word_length: 2.0,
        };
        let candidate = CorpusStats {
            letters: FxHashMap::from_iter([('a', 0.8), ('b', 0.2)]),
            bigrams: FxHashMap::from_iter([(['a', 'a'], 0.8), (['a', 'b'], 0.2)]),
            trigrams: FxHashMap::default(),
            first_letters: FxHashMap::from_iter([('a', 1.0)]),
            average_word_length: 2.0,
        };

        let score = score_with_filter(&source, &candidate, 0.2);

        assert_eq!(score.first_letters, 0.0);
        assert!((score.letters - 0.0).abs() < 1e-9);
        assert!((score.bigrams - 0.0).abs() < 1e-9);
    }
}
