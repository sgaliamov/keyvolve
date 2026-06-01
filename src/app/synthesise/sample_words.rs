use crate::app::synthesise::{
    SynthesiseConfig,
    counter::{CorpusScore, CorpusStats, CorpusStatsCounter, score_stats},
    shared::{report_path, write_corpus, write_report},
};
use miette::{Context, IntoDiagnostic, Result};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufReader, Read},
    path::Path,
};

/// Best candidate found during prefix search.
#[derive(Debug, Clone)]
struct Candidate {
    words: usize,
    score: CorpusScore,
}

/// Summary of source corpus built in one streaming pass.
#[derive(Debug, Clone)]
struct SourceSummary {
    stats: CorpusStats,
    word_count: usize,
}

/// Prefix evaluation cache.
#[derive(Debug, Default)]
struct PrefixSearch {
    cache: BTreeMap<usize, Candidate>,
}

/// Prefix words read result.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PrefixWords {
    words: Vec<String>,
}

/// Run the sample-word synthesise pipeline.
pub(super) fn synthesise_sample_words(cfg: SynthesiseConfig) -> Result<()> {
    let input = cfg
        .text
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.text` path")?;
    let output = cfg
        .output
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.output` path")?;
    tracing::info!(
        input = %input.display(),
        output = %output.display(),
        requested_words = cfg.words,
        tolerance = cfg.tolerance,
        method = "sampleWords",
        "Scanning source corpus"
    );
    let source = scan_source(input)?;
    let target_words = cfg.words.unwrap_or(source.word_count);
    tracing::info!(
        source_words = source.word_count,
        target_words,
        average_word_length = source.stats.average_word_length,
        "Source corpus indexed"
    );
    let best = find_best_candidate(input, &source, target_words, &cfg)?;

    let best_words = read_prefix_words(input, best.words)?.words;
    write_corpus(&best_words, output)?;
    let report = report_path(output);
    write_report(
        &report,
        &best.score,
        source.word_count,
        best.words,
        cfg.tolerance,
    )?;

    tracing::info!(
        input = %input.display(),
        output = %output.display(),
        report = %report.display(),
        source_words = source.word_count,
        generated_words = best.words,
        max_error = best.score.max_error,
        tolerance = cfg.tolerance,
        method = "sampleWords",
        "Synthesise complete"
    );
    Ok(())
}

/// Find smallest prefix that matches tolerance using exponential growth then binary search.
fn find_best_candidate(
    input: &Path,
    source: &SourceSummary,
    target_words: usize,
    cfg: &SynthesiseConfig,
) -> Result<Candidate> {
    if source.word_count == 0 || target_words == 0 {
        tracing::warn!("Source corpus empty; skipping sampling");
        return Ok(Candidate {
            words: 0,
            score: CorpusScore {
                letters: 0.0,
                bigrams: 0.0,
                first_letters: 0.0,
                average_word_length: 0.0,
                max_error: 0.0,
            },
        });
    }

    let cap = target_words.min(source.word_count);
    let mut search = PrefixSearch::default();
    let mut low = 0usize;
    let mut high = 1usize.min(cap);

    tracing::debug!(cap, tolerance = cfg.tolerance, "Searching prefix bounds");

    loop {
        let candidate = search.evaluate(input, high, &source.stats)?;
        tracing::debug!(
            words = high,
            max_error = candidate.score.max_error,
            tolerance = cfg.tolerance,
            "Evaluated prefix"
        );

        if candidate.score.max_error <= cfg.tolerance || high == cap {
            break;
        }

        low = high;
        high = (high.saturating_mul(2)).min(cap);
    }

    let high_candidate = search.evaluate(input, high, &source.stats)?;
    if high_candidate.score.max_error > cfg.tolerance {
        tracing::warn!(
            cap,
            max_error = high_candidate.score.max_error,
            tolerance = cfg.tolerance,
            "Tolerance not reached within cap; using largest prefix"
        );
        return Ok(high_candidate);
    }

    tracing::info!(
        low,
        high,
        tolerance = cfg.tolerance,
        "Tolerance reached; starting binary search"
    );

    while low + 1 < high {
        let mid = low + (high - low) / 2;
        let candidate = search.evaluate(input, mid, &source.stats)?;
        tracing::trace!(
            low,
            mid,
            high,
            max_error = candidate.score.max_error,
            "Binary search step"
        );

        if candidate.score.max_error <= cfg.tolerance {
            high = mid;
        } else {
            low = mid;
        }
    }

    let best = search.evaluate(input, high, &source.stats)?;
    tracing::info!(
        words = best.words,
        max_error = best.score.max_error,
        tolerance = cfg.tolerance,
        "Smallest matching prefix selected"
    );
    Ok(best)
}

impl PrefixSearch {
    /// Evaluate one prefix length, using cache to avoid rescanning same size.
    fn evaluate(
        &mut self,
        input: &Path,
        words: usize,
        source_stats: &CorpusStats,
    ) -> Result<Candidate> {
        if let Some(candidate) = self.cache.get(&words) {
            return Ok(candidate.clone());
        }

        let stats = read_prefix_stats(input, words)?;
        let candidate = Candidate {
            score: score_stats(source_stats, &stats),
            words,
        };
        self.cache.insert(words, candidate.clone());
        Ok(candidate)
    }
}

/// Scan full source corpus once to collect overall stats and total word count.
fn scan_source(path: &Path) -> Result<SourceSummary> {
    tracing::debug!(input = %path.display(), "Opening source corpus for full scan");
    let file = fs::File::open(path)
        .into_diagnostic()
        .wrap_err("Failed to open synth source text")?;
    let mut reader = BufReader::new(file);
    let mut counter = CorpusStatsCounter::default();
    let mut buf = [0u8; 64 * 1024];
    let mut word = Vec::new();
    let mut word_count = 0usize;
    let mut bytes_scanned = 0u64;

    loop {
        let read = reader
            .read(&mut buf)
            .into_diagnostic()
            .wrap_err("Failed while reading synth source text")?;
        if read == 0 {
            break;
        }

        for &byte in &buf[..read] {
            if byte.is_ascii_whitespace() {
                finish_word(&mut counter, &mut word, &mut word_count)?;
            } else {
                word.push(byte);
            }
        }

        bytes_scanned += read as u64;
    }

    finish_word(&mut counter, &mut word, &mut word_count)?;
    tracing::debug!(word_count, bytes_scanned, "Source scan complete");

    Ok(SourceSummary {
        stats: counter.finish(),
        word_count,
    })
}

/// Read stats for first N words from source file.
fn read_prefix_stats(path: &Path, limit: usize) -> Result<CorpusStats> {
    let file = fs::File::open(path)
        .into_diagnostic()
        .wrap_err("Failed to open synth source text")?;
    let mut reader = BufReader::new(file);
    let mut buf = [0u8; 64 * 1024];
    let mut counter = CorpusStatsCounter::default();
    let mut word = Vec::new();
    let mut seen = 0usize;

    while seen < limit {
        let read = reader
            .read(&mut buf)
            .into_diagnostic()
            .wrap_err("Failed while reading synth source text")?;
        if read == 0 {
            break;
        }

        for &byte in &buf[..read] {
            if byte.is_ascii_whitespace() {
                finish_stat_word(&mut counter, &mut word, &mut seen)?;
                if seen == limit {
                    break;
                }
            } else {
                word.push(byte);
            }
        }
    }

    if seen < limit {
        finish_stat_word(&mut counter, &mut word, &mut seen)?;
    }

    Ok(counter.finish())
}

/// Read first N words from source file.
fn read_prefix_words(path: &Path, limit: usize) -> Result<PrefixWords> {
    let file = fs::File::open(path)
        .into_diagnostic()
        .wrap_err("Failed to open synth source text")?;
    let mut reader = BufReader::new(file);
    let mut buf = [0u8; 64 * 1024];
    let mut words = Vec::with_capacity(limit);
    let mut word = Vec::new();

    while words.len() < limit {
        let read = reader
            .read(&mut buf)
            .into_diagnostic()
            .wrap_err("Failed while reading synth source text")?;
        if read == 0 {
            break;
        }

        for &byte in &buf[..read] {
            if byte.is_ascii_whitespace() {
                finish_prefix_word(&mut words, &mut word)?;
                if words.len() == limit {
                    break;
                }
            } else {
                word.push(byte);
            }
        }
    }

    if words.len() < limit {
        finish_prefix_word(&mut words, &mut word)?;
    }

    Ok(PrefixWords { words })
}

/// Finalize one buffered word during full-source scan.
fn finish_word(
    counter: &mut CorpusStatsCounter,
    word: &mut Vec<u8>,
    word_count: &mut usize,
) -> Result<()> {
    if word.is_empty() {
        return Ok(());
    }

    let text = String::from_utf8(std::mem::take(word))
        .into_diagnostic()
        .wrap_err("Synth source contains invalid UTF-8 word")?;
    counter.add_word(&text);
    *word_count += 1;
    Ok(())
}

/// Finalize one buffered word while reading a prefix block.
fn finish_prefix_word(words: &mut Vec<String>, word: &mut Vec<u8>) -> Result<()> {
    if word.is_empty() {
        return Ok(());
    }

    words.push(
        String::from_utf8(std::mem::take(word))
            .into_diagnostic()
            .wrap_err("Synth source contains invalid UTF-8 word")?,
    );
    Ok(())
}

/// Finalize one buffered word while reading prefix stats.
fn finish_stat_word(
    counter: &mut CorpusStatsCounter,
    word: &mut Vec<u8>,
    seen: &mut usize,
) -> Result<()> {
    if word.is_empty() {
        return Ok(());
    }

    let text = String::from_utf8(std::mem::take(word))
        .into_diagnostic()
        .wrap_err("Synth source contains invalid UTF-8 word")?;
    counter.add_word(&text);
    *seen += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn read_prefix_words_uses_requested_count() {
        let path = fixture_path("aa bb cc");
        let words = read_prefix_words(&path, 2).unwrap();
        assert_eq!(words.words, vec!["aa".to_owned(), "bb".to_owned()]);
    }

    #[test]
    fn report_path_uses_output_stem() {
        let path = Path::new("data/synthesised.txt");
        assert_eq!(
            report_path(path),
            PathBuf::from("data").join("synthesised.synth-report.txt")
        );
    }

    #[test]
    fn scan_source_reads_stats_and_word_count() {
        let path = fixture_path("ab ac\nzzz");
        let source = scan_source(&path).unwrap();
        assert_eq!(source.word_count, 3);

        let expected = crate::app::synthesise::counter::calculate_stats(&[
            "ab".to_owned(),
            "ac".to_owned(),
            "zzz".to_owned(),
        ]);
        assert_eq!(source.stats, expected);
    }

    #[test]
    fn find_best_candidate_picks_smallest_passing_prefix() {
        let path = fixture_path("aa aa aa ab");
        let source = scan_source(&path).unwrap();
        let cfg = SynthesiseConfig {
            text: Some(path.clone()),
            output: Some(path.clone()),
            tolerance: 0.0,
            words: Some(source.word_count),
            ..SynthesiseConfig::default()
        };

        let best = find_best_candidate(&path, &source, source.word_count, &cfg).unwrap();
        assert_eq!(best.words, source.word_count);
        assert_eq!(best.score.max_error, 0.0);
    }

    #[test]
    fn read_prefix_stats_matches_materialized_stats() {
        let path = fixture_path("ab ac zzz");
        let stats = read_prefix_stats(&path, 2).unwrap();
        let expected =
            crate::app::synthesise::counter::calculate_stats(&["ab".to_owned(), "ac".to_owned()]);
        assert_eq!(stats, expected);
    }

    fn fixture_path(contents: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("keyvolve-prefix-sample-{stamp}.txt"));
        fs::write(&path, contents).unwrap();
        path
    }
}
