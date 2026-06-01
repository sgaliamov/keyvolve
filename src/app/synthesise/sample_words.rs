use crate::app::synthesise::{
    SynthesiseConfig,
    counter::{CorpusScore, CorpusStats, CorpusStatsCounter, calculate_stats, score_stats},
    shared::{report_path, write_corpus},
};
use miette::{Context, IntoDiagnostic, Result};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::{
    cell::RefCell,
    fs,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
};

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
    tracing::info!(
        input = %input.display(),
        output = %output.display(),
        attempts = cfg.attempts.max(1),
        requested_words = cfg.words,
        tolerance = cfg.tolerance,
        method = "sampleWords",
        "Indexing source corpus"
    );
    let source = IndexedWords::from_path(input)?;
    let target_words = cfg.words.unwrap_or(source.word_count());
    let source_stats = source.stats().clone();
    tracing::info!(
        source_words = source.word_count(),
        target_words,
        average_word_length = source_stats.average_word_length,
        "Source corpus indexed"
    );
    let best = find_best_candidate(&source, &source_stats, target_words, &cfg)?;

    write_corpus(&best.words, output)?;
    let report = report_path(output);
    write_report(
        &report,
        &best.score,
        source.word_count(),
        best.words.len(),
        cfg.tolerance,
    )?;

    tracing::info!(
        input = %input.display(),
        output = %output.display(),
        report = %report.display(),
        source_words = source.word_count(),
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
    source: &IndexedWords,
    source_stats: &CorpusStats,
    target_words: usize,
    cfg: &SynthesiseConfig,
) -> Result<Candidate> {
    if source.is_empty() {
        tracing::warn!("Source corpus empty; skipping sampling");
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

    tracing::debug!(
        attempts,
        target_words,
        tolerance = cfg.tolerance,
        "Sampling candidates"
    );

    for attempt in 0..attempts {
        let attempt_number = attempt + 1;
        let words = sample_words(source, target_words, mix_seed(cfg.seed, attempt as u64))?;
        let stats = calculate_stats(&words);
        let score = score_stats(source_stats, &stats);

        let replace = best
            .as_ref()
            .map(|current| score.max_error < current.score.max_error)
            .unwrap_or(true);
        if replace {
            tracing::debug!(
                attempt = attempt_number,
                attempts,
                max_error = score.max_error,
                letters_error = score.letters,
                bigrams_error = score.bigrams,
                first_letters_error = score.first_letters,
                average_word_length_error = score.average_word_length,
                "New best candidate"
            );
            best = Some(Candidate { words, score });
        } else {
            tracing::trace!(
                attempt = attempt_number,
                attempts,
                max_error = score.max_error,
                "Candidate rejected"
            );
        }

        if best
            .as_ref()
            .is_some_and(|current| current.score.max_error <= cfg.tolerance)
        {
            tracing::info!(
                attempt = attempt_number,
                attempts,
                max_error = best
                    .as_ref()
                    .map(|candidate| candidate.score.max_error)
                    .unwrap_or(0.0),
                tolerance = cfg.tolerance,
                "Tolerance reached; stopping early"
            );
            break;
        }
    }

    if let Some(candidate) = best.as_ref() {
        tracing::debug!(
            generated_words = candidate.words.len(),
            max_error = candidate.score.max_error,
            "Best candidate selected"
        );
    }

    best.wrap_err("Failed to build synth candidate")
}

/// Sample words with replacement from the source corpus.
fn sample_words(source: &IndexedWords, count: usize, seed: Option<u64>) -> Result<Vec<String>> {
    if source.is_empty() || count == 0 {
        return Ok(Vec::new());
    }

    let mut rng = make_rng(seed);
    (0..count)
        .map(|_| {
            let index = rng.random_range(0..source.word_count());
            source.word_at(index)
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

/// Word source indexed by file byte offsets.
#[derive(Debug)]
struct IndexedWords {
    offsets: Vec<u64>,
    stats: CorpusStats,
    reader: RefCell<BufReader<fs::File>>,
}

impl IndexedWords {
    /// Index words from source file without loading full content into memory.
    fn from_path(path: &Path) -> Result<Self> {
        tracing::debug!(input = %path.display(), "Opening source corpus for indexing");
        let file = fs::File::open(path)
            .into_diagnostic()
            .wrap_err("Failed to open synth source text")?;
        let mut reader = BufReader::new(file);
        let mut offsets = Vec::new();
        let mut counter = CorpusStatsCounter::default();
        let mut buf = [0u8; 64 * 1024];
        let mut word = Vec::new();
        let mut word_start: Option<u64> = None;
        let mut pos = 0u64;

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
                    finish_word(&mut offsets, &mut counter, &mut word, &mut word_start)?;
                } else {
                    if word_start.is_none() {
                        word_start = Some(pos);
                    }
                    word.push(byte);
                }
                pos += 1;
            }
        }

        finish_word(&mut offsets, &mut counter, &mut word, &mut word_start)?;
        tracing::debug!(
            indexed_words = offsets.len(),
            bytes_scanned = pos,
            "Source scan complete"
        );

        let reader = BufReader::new(
            fs::File::open(path)
                .into_diagnostic()
                .wrap_err("Failed to reopen synth source text")?,
        );

        Ok(Self {
            offsets,
            stats: counter.finish(),
            reader: RefCell::new(reader),
        })
    }

    /// Count of indexed words.
    fn word_count(&self) -> usize {
        self.offsets.len()
    }

    /// Whether source has no words.
    fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    /// Source stats computed during indexing.
    fn stats(&self) -> &CorpusStats {
        &self.stats
    }

    /// Read one word by byte offset.
    fn word_at(&self, index: usize) -> Result<String> {
        let offset = *self
            .offsets
            .get(index)
            .ok_or_else(|| miette::miette!("Source word index out of range: {index}"))?;
        let mut reader = self.reader.borrow_mut();
        reader
            .seek(SeekFrom::Start(offset))
            .into_diagnostic()
            .wrap_err("Failed to seek synth source text")?;

        let mut bytes = Vec::new();
        let mut buf = [0u8; 256];

        loop {
            let read = reader
                .read(&mut buf)
                .into_diagnostic()
                .wrap_err("Failed while reading sampled word")?;
            if read == 0 {
                break;
            }

            let mut end = read;
            for (i, &byte) in buf[..read].iter().enumerate() {
                if byte.is_ascii_whitespace() {
                    end = i;
                    bytes.extend_from_slice(&buf[..end]);
                    return String::from_utf8(bytes)
                        .into_diagnostic()
                        .wrap_err("Synth source contains invalid UTF-8 word");
                }
            }

            bytes.extend_from_slice(&buf[..end]);
        }

        String::from_utf8(bytes)
            .into_diagnostic()
            .wrap_err("Synth source contains invalid UTF-8 word")
    }
}

/// Finalize one buffered word during indexing.
fn finish_word(
    offsets: &mut Vec<u64>,
    counter: &mut CorpusStatsCounter,
    word: &mut Vec<u8>,
    word_start: &mut Option<u64>,
) -> Result<()> {
    if word.is_empty() {
        *word_start = None;
        return Ok(());
    }

    let text = String::from_utf8(std::mem::take(word))
        .into_diagnostic()
        .wrap_err("Synth source contains invalid UTF-8 word")?;
    counter.add_word(&text);
    offsets.push(word_start.take().unwrap_or_default());
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
    fn sample_words_uses_requested_count() {
        let source = indexed_words_fixture("aa bb");
        let words = sample_words(&source, 5, Some(7)).unwrap();
        assert_eq!(words.len(), 5);
        assert!(words.iter().all(|word| word == "aa" || word == "bb"));
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
    fn indexed_words_reads_offsets_and_stats() {
        let source = indexed_words_fixture("ab ac\nzzz");
        assert_eq!(source.word_count(), 3);
        assert_eq!(source.word_at(0).unwrap(), "ab");
        assert_eq!(source.word_at(1).unwrap(), "ac");
        assert_eq!(source.word_at(2).unwrap(), "zzz");

        let expected = calculate_stats(&["ab".to_owned(), "ac".to_owned(), "zzz".to_owned()]);
        assert_eq!(source.stats(), &expected);
    }

    fn indexed_words_fixture(contents: &str) -> IndexedWords {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("keyvolve-sample-words-{stamp}.txt"));
        fs::write(&path, contents).unwrap();
        IndexedWords::from_path(&path).unwrap()
    }
}
