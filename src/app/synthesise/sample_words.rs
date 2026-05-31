use crate::app::synthesise::{
    SynthesiseConfig,
    counter::{CorpusScore, CorpusStats, calculate_stats, score_stats},
    shared::{load_words, report_path, write_corpus},
};
use miette::{Context, IntoDiagnostic, Result};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::{fs, io::Write, path::Path};

/// Best candidate found during sampling.
#[derive(Debug, Clone)]
struct Candidate {
    words: Vec<String>,
    score: CorpusScore,
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

/// Sample candidate corpora and keep the best one.
fn find_best_candidate(
    source_words: &[String],
    source_stats: &CorpusStats,
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
    use std::path::{Path, PathBuf};

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
