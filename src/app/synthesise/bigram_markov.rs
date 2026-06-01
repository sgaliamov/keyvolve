use crate::app::synthesise::{
    SynthesiseConfig,
    counter::{CorpusStats, CorpusStatsCounter, calculate_stats, score_stats},
    shared::{report_path, write_corpus, write_report},
};
use miette::{Context, IntoDiagnostic, Result};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use rustc_hash::FxHashMap;
use std::{
    fs,
    io::{BufReader, Read},
    path::Path,
};

/// Weighted discrete sampler built from a char → weight map.
struct WeightedSampler {
    chars: Vec<char>,
    /// prefix-sum of normalized weights; last entry ≈ 1.0
    cumulative: Vec<f64>,
}

/// Bigram Markov chain: for each char, a weighted sampler over successors.
struct MarkovChain {
    transitions: FxHashMap<char, WeightedSampler>,
}

impl WeightedSampler {
    fn new(weights: &FxHashMap<char, f64>) -> Self {
        let mut pairs: Vec<(char, f64)> = weights.iter().map(|(&c, &w)| (c, w)).collect();
        pairs.sort_unstable_by_key(|(c, _)| *c);
        let total: f64 = pairs.iter().map(|(_, w)| w).sum();
        let mut cum = 0.0f64;
        let cumulative = pairs
            .iter()
            .map(|(_, w)| {
                cum += w / total;
                cum
            })
            .collect();
        let chars = pairs.into_iter().map(|(c, _)| c).collect();
        Self { chars, cumulative }
    }

    /// Sample one char; returns `None` if empty.
    fn sample(&self, rng: &mut StdRng) -> Option<char> {
        if self.chars.is_empty() {
            return None;
        }
        let r: f64 = rng.random();
        let idx = self.cumulative.partition_point(|&c| c < r);
        Some(self.chars[idx.min(self.chars.len() - 1)])
    }
}

impl MarkovChain {
    /// Build transitions from normalized bigram frequencies.
    fn from_bigrams(bigrams: &FxHashMap<[char; 2], f64>) -> Self {
        let mut by_from: FxHashMap<char, FxHashMap<char, f64>> = FxHashMap::default();
        for (&[a, b], &w) in bigrams {
            *by_from.entry(a).or_default().entry(b).or_insert(0.0) += w;
        }
        let transitions = by_from
            .into_iter()
            .map(|(c, w)| (c, WeightedSampler::new(&w)))
            .collect();
        Self { transitions }
    }

    /// Follow one step from `from`; returns `None` if `from` has no outgoing edges.
    fn step(&self, from: char, rng: &mut StdRng) -> Option<char> {
        self.transitions.get(&from)?.sample(rng)
    }
}

/// Keep only bigrams whose frequency ≥ `min_frequency`; re-normalize.
fn filter_bigrams(
    bigrams: &FxHashMap<[char; 2], f64>,
    min_frequency: f64,
) -> FxHashMap<[char; 2], f64> {
    let filtered: FxHashMap<[char; 2], f64> = bigrams
        .iter()
        .filter(|(_, f)| **f >= min_frequency)
        .map(|(&k, &f)| (k, f))
        .collect();
    let total: f64 = filtered.values().sum();
    if total == 0.0 {
        return filtered;
    }
    filtered.into_iter().map(|(k, f)| (k, f / total)).collect()
}

/// Score comparing only bigrams present in filtered source (avoids max-error explosion on rare pairs).
fn filtered_score(
    source_filtered: &FxHashMap<[char; 2], f64>,
    candidate: &CorpusStats,
    source: &CorpusStats,
) -> f64 {
    // Re-normalize candidate bigrams over the same filtered key set.
    let cand_total: f64 = candidate
        .bigrams
        .iter()
        .filter(|(k, _)| source_filtered.contains_key(*k))
        .map(|(_, &f)| f)
        .sum();

    let mut max_err: f64 = 0.0;
    for (k, &expected) in source_filtered {
        let actual = if cand_total > 0.0 {
            candidate.bigrams.get(k).copied().unwrap_or(0.0) / cand_total
        } else {
            0.0
        };
        let err = if expected.abs() < 1e-12 {
            actual.abs()
        } else {
            (expected - actual).abs() / expected
        };
        max_err = max_err.max(err);
    }
    // Check remaining metrics against unfiltered source.
    let score = score_stats(source, candidate);
    max_err
        .max(score.letters)
        .max(score.first_letters)
        .max(score.average_word_length)
}

/// Generate one word: start at `first`, extend via chain until geometric stop or `max_len`.
/// `stop_p = 1 / avg_word_len` yields the correct expected length.
fn generate_word(
    first: char,
    chain: &MarkovChain,
    stop_p: f64,
    max_len: usize,
    rng: &mut StdRng,
) -> String {
    let mut word = vec![first];
    let mut cur = first;
    while word.len() < max_len {
        let r: f64 = rng.random();
        if r < stop_p {
            break;
        }
        match chain.step(cur, rng) {
            Some(next) => {
                word.push(next);
                cur = next;
            }
            None => break,
        }
    }
    word.into_iter().collect()
}

/// Run `attempts` independent generation passes seeded deterministically;
/// return the corpus with the lowest max error vs source stats.
/// Bigrams below `min_frequency` are excluded from both chain and scoring.
fn best_of_attempts(
    source: &CorpusStats,
    min_frequency: f64,
    target_bigrams: usize,
    max_word_len: usize,
    attempts: usize,
    seed: Option<u64>,
) -> Vec<String> {
    // Filter rare bigrams — reduces scoring variance and chain noise.
    let filtered_bigrams = filter_bigrams(&source.bigrams, min_frequency);
    let chain = MarkovChain::from_bigrams(&filtered_bigrams);
    // Seed words from letter dist (≈ stationary dist of chain) so generated
    // bigram frequencies converge to source bigram frequencies.
    let letter_sampler = WeightedSampler::new(&source.letters);
    // geometric stop probability → E[word_len] = avg_word_len
    let stop_p = 1.0 / source.average_word_length.max(1.0);

    let mut best_words: Vec<String> = Vec::new();
    let mut best_err = f64::MAX;

    for attempt in 0..attempts.max(1) {
        let seed_val = seed
            .map(|s| s.wrapping_add(attempt as u64))
            .unwrap_or(attempt as u64 ^ 0xcafe_babe_dead_beef);
        let mut rng = StdRng::seed_from_u64(seed_val);
        let mut words: Vec<String> = Vec::new();
        let mut bigrams_emitted: usize = 0;

        while bigrams_emitted < target_bigrams {
            if let Some(first) = letter_sampler.sample(&mut rng) {
                let word = generate_word(first, &chain, stop_p, max_word_len, &mut rng);
                bigrams_emitted += word.len().saturating_sub(1);
                words.push(word);
            }
        }

        let candidate = calculate_stats(&words);
        let err = filtered_score(&filtered_bigrams, &candidate, source);
        tracing::debug!(
            attempt,
            max_error = err,
            words = words.len(),
            "Generated candidate"
        );

        if err < best_err {
            best_err = err;
            best_words = words;
        }

        if best_err == 0.0 {
            break;
        }
    }

    best_words
}

/// Scan full source corpus for `CorpusStats` and total word count.
fn scan_source(path: &Path) -> Result<(CorpusStats, usize)> {
    let file = fs::File::open(path)
        .into_diagnostic()
        .wrap_err("Failed to open source text")?;
    let mut reader = BufReader::new(file);
    let mut counter = CorpusStatsCounter::default();
    let mut buf = [0u8; 64 * 1024];
    let mut word: Vec<u8> = Vec::new();
    let mut word_count = 0usize;

    loop {
        let n = reader
            .read(&mut buf)
            .into_diagnostic()
            .wrap_err("Failed reading source text")?;
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            if b.is_ascii_whitespace() {
                if !word.is_empty() {
                    let s = String::from_utf8(std::mem::take(&mut word))
                        .into_diagnostic()
                        .wrap_err("Source contains invalid UTF-8")?;
                    counter.add_word(&s);
                    word_count += 1;
                }
            } else {
                word.push(b);
            }
        }
    }
    if !word.is_empty() {
        let s = String::from_utf8(word)
            .into_diagnostic()
            .wrap_err("Source contains invalid UTF-8")?;
        counter.add_word(&s);
        word_count += 1;
    }

    Ok((counter.finish(), word_count))
}

/// Run the Markov-chain bigram synthesise pipeline.
pub(super) fn synthesise_bigram_markov(cfg: SynthesiseConfig) -> Result<()> {
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
        target = cfg.target,
        attempts = cfg.attempts,
        method = "bigramMarkov",
        "Scanning source corpus"
    );

    let (source_stats, source_word_count) = scan_source(input)?;
    tracing::info!(
        source_words = source_word_count,
        avg_word_len = source_stats.average_word_length,
        bigrams = source_stats.bigrams.len(),
        "Source scanned"
    );

    let words = best_of_attempts(
        &source_stats,
        cfg.min_frequency,
        cfg.target,
        cfg.max_word_len,
        cfg.attempts,
        cfg.seed,
    );

    let filtered_bigrams = filter_bigrams(&source_stats.bigrams, cfg.min_frequency);
    let final_candidate = calculate_stats(&words);
    let final_score = {
        let mut s = score_stats(&source_stats, &final_candidate);
        // Override bigrams_error with filtered version (matches generation target).
        let filtered_bigrams_err = filtered_score(&filtered_bigrams, &final_candidate, &source_stats)
            - s.letters.max(s.first_letters).max(s.average_word_length);
        s.bigrams = filtered_bigrams_err.max(0.0);
        s.max_error = s.letters.max(s.bigrams).max(s.first_letters).max(s.average_word_length);
        s
    };
    tracing::info!(
        generated_words = words.len(),
        max_error = final_score.max_error,
        letters_error = final_score.letters,
        bigrams_error = final_score.bigrams,
        first_letters_error = final_score.first_letters,
        avg_word_len_error = final_score.average_word_length,
        tolerance = cfg.tolerance,
        passed = final_score.max_error <= cfg.tolerance,
        method = "bigramMarkov",
        "Generation complete"
    );

    if final_score.max_error > cfg.tolerance {
        tracing::warn!(
            max_error = final_score.max_error,
            tolerance = cfg.tolerance,
            "Tolerance not met; increase `attempts` or relax `tolerance`"
        );
    }

    write_corpus(&words, output)?;

    let report = report_path(output);
    write_report(
        &report,
        &final_score,
        source_word_count,
        words.len(),
        cfg.tolerance,
    )?;

    tracing::info!(
        corpus = %output.display(),
        report = %report.display(),
        words = words.len(),
        method = "bigramMarkov",
        "Synthesise complete"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::synthesise::counter::calculate_stats;

    fn source_stats() -> CorpusStats {
        calculate_stats(&[
            "ab".to_owned(),
            "ab".to_owned(),
            "ac".to_owned(),
            "ba".to_owned(),
        ])
    }

    #[test]
    fn weighted_sampler_covers_full_distribution() {
        let weights: FxHashMap<char, f64> =
            [('a', 0.6), ('b', 0.3), ('c', 0.1)].into_iter().collect();
        let sampler = WeightedSampler::new(&weights);
        assert_eq!(sampler.chars.len(), 3);
        assert!((sampler.cumulative.last().copied().unwrap_or(0.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn markov_chain_steps_to_known_successors() {
        let stats = source_stats();
        let chain = MarkovChain::from_bigrams(&stats.bigrams);
        // 'a' should have successors 'b' and 'c'
        assert!(chain.transitions.contains_key(&'a'));
        let mut rng = StdRng::seed_from_u64(42);
        let next = chain.step('a', &mut rng);
        assert!(matches!(next, Some('b') | Some('c')));
    }

    #[test]
    fn generate_word_respects_max_len() {
        let stats = source_stats();
        let chain = MarkovChain::from_bigrams(&stats.bigrams);
        let mut rng = StdRng::seed_from_u64(99);
        for _ in 0..50 {
            let word = generate_word('a', &chain, 0.0, 4, &mut rng);
            assert!(word.len() <= 4, "word too long: {word}");
        }
    }

    #[test]
    fn best_of_attempts_produces_corpus_close_to_source() {
        let source = source_stats();
        let words = best_of_attempts(&source, 0.0, 1_000, 5, 8, Some(0));
        let score = score_stats(&source, &calculate_stats(&words));
        // Should be well under 60% error with 1k bigrams and 8 attempts.
        assert!(
            score.max_error < 0.6,
            "max_error too high: {:.4}",
            score.max_error
        );
    }
}
