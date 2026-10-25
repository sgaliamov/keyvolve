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

fn write_csv_rows(path: &Path, header: &str, rows: &[(String, f64, f64)]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err("Failed to create stats CSV directory")?;
    }
    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create stats CSV report")?;
    writeln!(out, "{header}").into_diagnostic()?;
    for (key, share, count) in rows {
        writeln!(out, "{key},{share:.12},{count:.0}").into_diagnostic()?;
    }
    Ok(())
}

fn make_distribution_rows<K: Copy + Eq + std::hash::Hash + std::fmt::Display>(
    values: &FxHashMap<K, f64>,
    total_count: f64,
) -> Vec<(String, f64, f64)> {
    let mut rows = values
        .iter()
        .map(|(key, share)| (key.to_string(), *share, *share * total_count))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

fn make_bigram_rows(bigrams: &FxHashMap<[char; 2], f64>, total_count: f64) -> Vec<(String, f64, f64)> {
    let mut rows = bigrams
        .iter()
        .map(|(pair, share)| (pair.iter().collect::<String>(), *share, *share * total_count))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

fn make_aggregated_bigram_rows(bigrams: &FxHashMap<[char; 2], f64>, total_count: f64) -> Vec<(String, f64, f64)> {
    let mut aggregated = FxHashMap::<String, f64>::default();
    for (pair, share) in bigrams {
        let mut chars = [pair[0], pair[1]];
        chars.sort_unstable();
        let key: String = chars.iter().collect();
        *aggregated.entry(key).or_insert(0.0) += *share;
    }
    let mut rows = aggregated
        .into_iter()
        .map(|(key, share)| (key, share, share * total_count))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

/// Write human-readable CSV reports next to the JSON cache.
pub fn write_stats_csv_reports(path: &Path, data: &CachedSourceStats) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "stats".to_string());

    let letters_total = data.word_count as f64 * data.stats.average_word_length.max(0.0);
    let bigrams_total = data.word_count as f64 * (data.stats.average_word_length - 1.0).max(0.0);
    let first_letters_total = data.word_count as f64;

    let letters_path = dir.join(format!("{stem}.letters.csv"));
    let bigrams_path = dir.join(format!("{stem}.bigrams.csv"));
    let aggregated_path = dir.join(format!("{stem}.bigrams.aggregated.csv"));
    let first_letters_path = dir.join(format!("{stem}.first-letters.csv"));

    write_csv_rows(
        &letters_path,
        "letter,share,count",
        &make_distribution_rows(&data.stats.letters, letters_total),
    )?;
    write_csv_rows(
        &bigrams_path,
        "bigram,share,count",
        &make_bigram_rows(&data.stats.bigrams, bigrams_total),
    )?;
    write_csv_rows(
        &aggregated_path,
        "pair,share,count",
        &make_aggregated_bigram_rows(&data.stats.bigrams, bigrams_total),
    )?;
    write_csv_rows(
        &first_letters_path,
        "first_letter,share,count",
        &make_distribution_rows(&data.stats.first_letters, first_letters_total),
    )?;

    Ok(())
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

/// Drop entries below the configured thresholds from the normalized maps and re-normalize.
pub fn filter_stats_frequencies(
    stats: &mut CorpusStats,
    min_frequency: f64,
    min_trigram_frequency: f64,
) {
    if min_frequency > 0.0 {
        filter_frequency_map(&mut stats.letters, min_frequency);
        filter_frequency_map(&mut stats.bigrams, min_frequency);
        filter_frequency_map(&mut stats.first_letters, min_frequency);
    }

    if min_trigram_frequency > 0.0 {
        filter_frequency_map(&mut stats.trigrams, min_trigram_frequency);
    }
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
    min_trigram_frequency: f64,
) -> CorpusScore {
    let mut filtered_source = source.clone();
    let mut filtered_candidate = candidate.clone();
    filter_stats_frequencies(&mut filtered_source, min_frequency, min_trigram_frequency);
    filter_stats_frequencies(
        &mut filtered_candidate,
        min_frequency,
        min_trigram_frequency,
    );
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
    writeln!(out, "letters_error={:05.2}%", score.letters * 100.0).into_diagnostic()?;
    writeln!(out, "bigrams_error={:05.2}%", score.bigrams * 100.0).into_diagnostic()?;
    writeln!(
        out,
        "first_letters_error={:05.2}%",
        score.first_letters * 100.0
    )
    .into_diagnostic()?;
    writeln!(
        out,
        "average_word_length_error={:05.2}%",
        score.average_word_length * 100.0
    )
    .into_diagnostic()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_stats_csv_reports_writes_textual_reports() {
        let dir = std::env::temp_dir();
        let path = dir.join("keyvolve-stats-csv.json");
        let data = CachedSourceStats {
            stats: CorpusStats {
                letters: FxHashMap::from_iter([('e', 0.6), ('t', 0.4)]),
                bigrams: FxHashMap::from_iter([(['t', 'h'], 0.75), (['h', 't'], 0.25)]),
                trigrams: FxHashMap::default(),
                first_letters: FxHashMap::from_iter([('t', 0.75), ('e', 0.25)]),
                average_word_length: 2.0,
            },
            word_count: 4,
        };

        write_stats_csv_reports(&path, &data).unwrap();

        let letters = fs::read_to_string(dir.join("keyvolve-stats-csv.letters.csv")).unwrap();
        assert!(letters.contains("letter,share,count\n"));
        assert!(letters.contains("e,0.6"));

        let bigrams = fs::read_to_string(dir.join("keyvolve-stats-csv.bigrams.csv")).unwrap();
        assert!(bigrams.contains("th,0.750000000000,3"));
        assert!(bigrams.contains("ht,0.250000000000,1"));

        let aggregated = fs::read_to_string(dir.join("keyvolve-stats-csv.bigrams.aggregated.csv")).unwrap();
        assert!(aggregated.contains("ht,1.000000000000,4"));

        let first_letters = fs::read_to_string(dir.join("keyvolve-stats-csv.first-letters.csv")).unwrap();
        assert!(first_letters.contains("t,0.750000000000,3"));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(dir.join("keyvolve-stats-csv.letters.csv"));
        let _ = fs::remove_file(dir.join("keyvolve-stats-csv.bigrams.csv"));
        let _ = fs::remove_file(dir.join("keyvolve-stats-csv.bigrams.aggregated.csv"));
        let _ = fs::remove_file(dir.join("keyvolve-stats-csv.first-letters.csv"));
    }

    #[test]
    fn filter_stats_frequencies_keeps_all_letter_and_bigram_entries() {
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

        filter_stats_frequencies(&mut stats, 0.0, 0.15);

        assert_eq!(stats.letters.len(), 3);
        assert!(stats.letters.contains_key(&'a'));
        assert!(stats.letters.contains_key(&'b'));
        assert!(stats.letters.contains_key(&'c'));

        assert_eq!(stats.bigrams.len(), 3);
        assert!(stats.bigrams.contains_key(&['a', 'a']));
        assert!(stats.bigrams.contains_key(&['a', 'b']));
        assert!(stats.bigrams.contains_key(&['b', 'c']));

        assert_eq!(stats.first_letters.len(), 3);
        assert!(stats.first_letters.contains_key(&'a'));
        assert!(stats.first_letters.contains_key(&'b'));
        assert!(stats.first_letters.contains_key(&'c'));

        assert_eq!(stats.trigrams.len(), 2);
        assert!(stats.trigrams.contains_key(&['a', 'a', 'a']));
        assert!(stats.trigrams.contains_key(&['a', 'a', 'b']));
        assert!(!stats.trigrams.contains_key(&['a', 'b', 'c']));
        assert!((stats.trigrams.values().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn score_with_filter_keeps_letter_and_bigram_distributions() {
        let source = CorpusStats {
            letters: FxHashMap::from_iter([('a', 0.8), ('b', 0.2)]),
            bigrams: FxHashMap::from_iter([(['a', 'a'], 0.8), (['a', 'b'], 0.2)]),
            trigrams: FxHashMap::from_iter([(['a', 'a', 'a'], 1.0)]),
            first_letters: FxHashMap::from_iter([('a', 1.0)]),
            average_word_length: 2.0,
        };
        let candidate = CorpusStats {
            letters: FxHashMap::from_iter([('a', 0.8), ('b', 0.2)]),
            bigrams: FxHashMap::from_iter([(['a', 'a'], 0.8), (['a', 'b'], 0.2)]),
            trigrams: FxHashMap::from_iter([(['a', 'a', 'a'], 1.0)]),
            first_letters: FxHashMap::from_iter([('a', 1.0)]),
            average_word_length: 2.0,
        };

        let score = score_with_filter(&source, &candidate, 0.0, 0.2);

        assert_eq!(score.first_letters, 0.0);
        assert!((score.letters - 0.0).abs() < 1e-9);
        assert!((score.bigrams - 0.0).abs() < 1e-9);
    }
}
