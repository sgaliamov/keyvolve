use crate::app::synthesise::counter::{CorpusScore, CorpusStats, score_stats};
use miette::{Context, IntoDiagnostic, Result};
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

/// Path for cached source stats: `{stats_dir}/{source.stem}.source-stats.json`.
pub fn stats_cache_path(source: &Path, stats_dir: &Path) -> PathBuf {
    let stem = source.file_stem().unwrap_or_default().to_string_lossy();
    stats_dir.join(format!("{stem}.source-stats.json"))
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

/// Remove bigrams below `min_frequency` and re-normalize the remainder.
pub fn filter_stats_bigrams(stats: &mut CorpusStats, min_frequency: f64) {
    if min_frequency <= 0.0 {
        return;
    }
    stats.bigrams.retain(|_, f| *f >= min_frequency);
    let total: f64 = stats.bigrams.values().sum();
    if total > 0.0 {
        for v in stats.bigrams.values_mut() {
            *v /= total;
        }
    }
}

/// Score `candidate` against `source`, filtering source bigrams below `min_frequency` first.
/// Use this everywhere instead of calling `score_stats` directly.
pub fn score_with_filter(
    source: &CorpusStats,
    candidate: &CorpusStats,
    min_frequency: f64,
) -> CorpusScore {
    let mut filtered = source.clone();
    filter_stats_bigrams(&mut filtered, min_frequency);
    score_stats(&filtered, candidate)
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
