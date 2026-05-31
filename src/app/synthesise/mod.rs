pub mod config;
mod corpus;
mod counter;
mod digraph;

pub use config::*;
use corpus::build_corpus;
use counter::{CorpusScore, calculate_stats, score_stats};
use digraph::{
    count_corpus_letters, filter_and_scale, read_counts, read_letter_counts, write_bigrams,
    write_bigrams_aggregated, write_letter_freq_combined,
};
use miette::{Context, IntoDiagnostic, Result};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Best candidate found during sampling.
#[derive(Debug, Clone)]
struct Candidate {
    words: Vec<String>,
    score: CorpusScore,
}

/// Run the configured synthesise pipeline.
pub fn synthesise(cfg: SynthesiseConfig) -> Result<()> {
    match cfg.method {
        SynthesiseMethod::Digraph => synthesise_digraph(cfg),
        SynthesiseMethod::SampleWords => synthesise_sample_words(cfg),
    }
}

/// Run the original digraph-based synthesise pipeline.
fn synthesise_digraph(cfg: SynthesiseConfig) -> Result<()> {
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

/// Run the sample-word synthesise pipeline.
fn synthesise_sample_words(cfg: SynthesiseConfig) -> Result<()> {
    let input = cfg
        .text
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.text` path")?;
    let output = cfg
        .output
        .as_deref()
        .wrap_err("Synthesise mode requires `synthesise.output` path")?;
    let source_words = load_words(input)?;
    let target_words = cfg.words.unwrap_or(source_words.len());
    let source_stats = calculate_stats(&source_words);
    let best = find_best_candidate(&source_words, &source_stats, target_words, &cfg)?;

    write_corpus(&best.words, output)?;
    let report = report_path(output);
    write_report(
        &report,
        &best.score,
        source_words.len(),
        best.words.len(),
        cfg.tolerance,
    )?;

    tracing::info!(
        input = %input.display(),
        output = %output.display(),
        report = %report.display(),
        source_words = source_words.len(),
        generated_words = best.words.len(),
        max_error = best.score.max_error,
        tolerance = cfg.tolerance,
        method = "sampleWords",
        "Synthesise complete"
    );
    Ok(())
}

/// Write space-separated words to a text file.
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

/// Load whitespace-separated words from source file.
fn load_words(path: &Path) -> Result<Vec<String>> {
    Ok(fs::read_to_string(path)
        .into_diagnostic()
        .wrap_err("Failed to read synth source text")?
        .split_whitespace()
        .map(str::to_owned)
        .collect())
}

/// Sample candidate corpora and keep the best one.
fn find_best_candidate(
    source_words: &[String],
    source_stats: &counter::CorpusStats,
    target_words: usize,
    cfg: &SynthesiseConfig,
) -> Result<Candidate> {
    if source_words.is_empty() {
        return Ok(Candidate {
            words: Vec::new(),
            score: CorpusScore {
                letters: 0.0,
                bigrams: 0.0,
                first_letters: 0.0,
                average_word_length: 0.0,
                max_error: 0.0,
            },
        });
    }

    let mut best: Option<Candidate> = None;
    let attempts = cfg.attempts.max(1);

    for attempt in 0..attempts {
        let words = sample_words(
            source_words,
            target_words,
            mix_seed(cfg.seed, attempt as u64),
        );
        let stats = calculate_stats(&words);
        let score = score_stats(source_stats, &stats);

        let replace = best
            .as_ref()
            .map(|current| score.max_error < current.score.max_error)
            .unwrap_or(true);
        if replace {
            best = Some(Candidate { words, score });
        }

        if best
            .as_ref()
            .is_some_and(|current| current.score.max_error <= cfg.tolerance)
        {
            break;
        }
    }

    best.wrap_err("Failed to build synth candidate")
}

/// Sample words with replacement from the source corpus.
fn sample_words(source_words: &[String], count: usize, seed: Option<u64>) -> Vec<String> {
    if source_words.is_empty() || count == 0 {
        return Vec::new();
    }

    let mut rng = make_rng(seed);
    (0..count)
        .map(|_| {
            let index = rng.random_range(0..source_words.len());
            source_words[index].clone()
        })
        .collect()
}

/// Report path next to corpus output.
fn report_path(output: &Path) -> PathBuf {
    let stem = output
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    output
        .parent()
        .unwrap_or(output)
        .join(format!("{stem}.synth-report.txt"))
}

/// Write compact synth score report.
fn write_report(
    path: &Path,
    score: &CorpusScore,
    source_words: usize,
    generated_words: usize,
    tolerance: f64,
) -> Result<()> {
    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create synth report")?;
    writeln!(out, "source_words={source_words}").into_diagnostic()?;
    writeln!(out, "generated_words={generated_words}").into_diagnostic()?;
    writeln!(out, "tolerance={tolerance:.6}").into_diagnostic()?;
    writeln!(out, "letters_error={:.6}", score.letters).into_diagnostic()?;
    writeln!(out, "bigrams_error={:.6}", score.bigrams).into_diagnostic()?;
    writeln!(out, "first_letters_error={:.6}", score.first_letters).into_diagnostic()?;
    writeln!(
        out,
        "average_word_length_error={:.6}",
        score.average_word_length
    )
    .into_diagnostic()?;
    writeln!(out, "max_error={:.6}", score.max_error).into_diagnostic()?;
    writeln!(out, "passed={}", score.max_error <= tolerance).into_diagnostic()?;
    Ok(())
}

/// Create RNG from optional seed.
fn make_rng(seed: Option<u64>) -> StdRng {
    match seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => {
            let mut rng = rand::rng();
            StdRng::from_rng(&mut rng)
        }
    }
}

/// Mix optional seed with an attempt salt.
fn mix_seed(seed: Option<u64>, salt: u64) -> Option<u64> {
    seed.map(|seed| seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_words_uses_requested_count() {
        let source = vec!["aa".to_owned(), "bb".to_owned()];
        let words = sample_words(&source, 5, Some(7));
        assert_eq!(words.len(), 5);
        assert!(words.iter().all(|word| source.contains(word)));
    }

    #[test]
    fn report_path_uses_output_stem() {
        let path = Path::new("data/synthesised.txt");
        assert_eq!(
            report_path(path),
            PathBuf::from("data").join("synthesised.synth-report.txt")
        );
    }
}
