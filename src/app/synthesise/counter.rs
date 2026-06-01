use rustc_hash::FxHashMap;
use std::hash::Hash;

/// Count all `a-z` digraph pairs from a buffered reader, skipping cross-whitespace pairs.
pub fn count_bigrams(reader: impl std::io::BufRead) -> FxHashMap<[char; 2], u64> {
    let mut counts: FxHashMap<[char; 2], u64> = FxHashMap::default();
    let mut prev: Option<char> = None;

    for line in reader.lines().map_while(Result::ok) {
        for ch in line.chars() {
            if ch.is_ascii_alphabetic() {
                let lc = ch.to_ascii_lowercase();
                if let Some(p) = prev {
                    *counts.entry([p, lc]).or_insert(0) += 1;
                }
                prev = Some(lc);
            } else {
                prev = None;
            }
        }
        prev = None;
    }

    counts
}

/// Count `a-z` letter frequencies from a buffered reader.
pub fn count_letters(reader: impl std::io::BufRead) -> FxHashMap<char, u64> {
    let mut counts: FxHashMap<char, u64> = FxHashMap::default();
    for line in reader.lines().map_while(Result::ok) {
        for ch in line.chars() {
            if ch.is_ascii_alphabetic() {
                *counts.entry(ch.to_ascii_lowercase()).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// Corpus metrics used by synthesise mode.
#[derive(Debug, Clone, PartialEq)]
pub struct CorpusStats {
    /// normalized letter frequencies
    pub letters: FxHashMap<char, f64>,
    /// normalized bigram frequencies
    pub bigrams: FxHashMap<[char; 2], f64>,
    /// normalized first-letter frequencies
    pub first_letters: FxHashMap<char, f64>,
    /// average word length in characters
    pub average_word_length: f64,
}

/// Incremental corpus stats builder.
#[derive(Debug, Clone, Default)]
pub struct CorpusStatsCounter {
    letter_counts: FxHashMap<char, u64>,
    bigram_counts: FxHashMap<[char; 2], u64>,
    first_letter_counts: FxHashMap<char, u64>,
    total_letters: u64,
    total_bigrams: u64,
    total_words: u64,
    total_word_len: u64,
}

/// Relative errors for tracked metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct CorpusScore {
    /// max letter relative error
    pub letters: f64,
    /// max bigram relative error
    pub bigrams: f64,
    /// max first-letter relative error
    pub first_letters: f64,
    /// average word length relative error
    pub average_word_length: f64,
    /// max error across all metrics
    pub max_error: f64,
}

/// Build normalized stats from a word slice.
pub fn calculate_stats(words: &[String]) -> CorpusStats {
    let mut counter = CorpusStatsCounter::default();
    for word in words {
        counter.add_word(word);
    }
    counter.finish()
}

/// Compare source and candidate stats with max relative error per metric.
pub fn score_stats(source: &CorpusStats, candidate: &CorpusStats) -> CorpusScore {
    let letters = max_map_error(&source.letters, &candidate.letters);
    let bigrams = max_map_error(&source.bigrams, &candidate.bigrams);
    let first_letters = max_map_error(&source.first_letters, &candidate.first_letters);
    let average_word_length =
        relative_error(source.average_word_length, candidate.average_word_length);
    let max_error = letters
        .max(bigrams)
        .max(first_letters)
        .max(average_word_length);

    CorpusScore {
        letters,
        bigrams,
        first_letters,
        average_word_length,
        max_error,
    }
}

fn normalize_char_counts(counts: &FxHashMap<char, u64>, total: u64) -> FxHashMap<char, f64> {
    if total == 0 {
        return FxHashMap::default();
    }

    counts
        .iter()
        .map(|(&key, &count)| (key, count as f64 / total as f64))
        .collect()
}

fn normalize_bigram_counts(
    counts: &FxHashMap<[char; 2], u64>,
    total: u64,
) -> FxHashMap<[char; 2], f64> {
    if total == 0 {
        return FxHashMap::default();
    }

    counts
        .iter()
        .map(|(&key, &count)| (key, count as f64 / total as f64))
        .collect()
}

impl CorpusStatsCounter {
    /// Add one word to the running corpus stats.
    pub fn add_word(&mut self, word: &str) {
        if word.is_empty() {
            return;
        }

        self.total_words += 1;
        self.total_word_len += word.len() as u64;

        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            *self.first_letter_counts.entry(first).or_insert(0) += 1;
            *self.letter_counts.entry(first).or_insert(0) += 1;
            self.total_letters += 1;

            let mut prev = first;
            for ch in chars {
                *self.letter_counts.entry(ch).or_insert(0) += 1;
                *self.bigram_counts.entry([prev, ch]).or_insert(0) += 1;
                self.total_letters += 1;
                self.total_bigrams += 1;
                prev = ch;
            }
        }
    }

    /// Finish counts into normalized stats.
    pub fn finish(&self) -> CorpusStats {
        CorpusStats {
            letters: normalize_char_counts(&self.letter_counts, self.total_letters),
            bigrams: normalize_bigram_counts(&self.bigram_counts, self.total_bigrams),
            first_letters: normalize_char_counts(&self.first_letter_counts, self.total_words),
            average_word_length: if self.total_words > 0 {
                self.total_word_len as f64 / self.total_words as f64
            } else {
                0.0
            },
        }
    }
}

fn max_map_error<K: Copy + Eq + Hash>(
    source: &FxHashMap<K, f64>,
    candidate: &FxHashMap<K, f64>,
) -> f64 {
    let mut max_error: f64 = 0.0;

    for (&key, &expected) in source {
        let actual = candidate.get(&key).copied().unwrap_or(0.0);
        max_error = max_error.max(relative_error(expected, actual));
    }

    for (&key, &actual) in candidate {
        let expected = source.get(&key).copied().unwrap_or(0.0);
        max_error = max_error.max(relative_error(expected, actual));
    }

    max_error
}

fn relative_error(expected: f64, actual: f64) -> f64 {
    const EPSILON: f64 = 1e-12;

    if expected.abs() <= EPSILON {
        if actual.abs() <= EPSILON {
            0.0
        } else {
            actual.abs()
        }
    } else {
        (expected - actual).abs() / expected.abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn digraphs_counts_pairs_and_breaks_on_whitespace() {
        let counts = count_bigrams(Cursor::new("ab BC ba ba aa"));
        assert_eq!(counts[&['a', 'b']], 1);
        assert_eq!(counts[&['b', 'c']], 1);
        assert_eq!(counts[&['b', 'a']], 2);
        assert_eq!(counts[&['a', 'a']], 1);
        assert!(!counts.contains_key(&['b', 'b']));
    }

    #[test]
    fn digraphs_boundary_and_punctuation_break_chain() {
        let counts = count_bigrams(Cursor::new("ab\nbc"));
        assert_eq!(counts[&['a', 'b']], 1);
        assert_eq!(counts[&['b', 'c']], 1);
        assert!(!counts.contains_key(&['b', 'b']));

        let counts = count_bigrams(Cursor::new("a.b"));
        assert!(counts.is_empty());
        assert!(count_bigrams(Cursor::new("")).is_empty());
    }

    #[test]
    fn calculate_stats_counts_requested_metrics() {
        let words = vec!["ab".to_owned(), "ac".to_owned()];
        let stats = calculate_stats(&words);

        assert_eq!(stats.letters[&'a'], 0.5);
        assert_eq!(stats.letters[&'b'], 0.25);
        assert_eq!(stats.letters[&'c'], 0.25);
        assert_eq!(stats.bigrams[&['a', 'b']], 0.5);
        assert_eq!(stats.bigrams[&['a', 'c']], 0.5);
        assert_eq!(stats.first_letters[&'a'], 1.0);
        assert_eq!(stats.average_word_length, 2.0);
    }

    #[test]
    fn score_stats_uses_max_relative_error() {
        let source = calculate_stats(&["ab".to_owned(), "ac".to_owned()]);
        let candidate = calculate_stats(&["ab".to_owned(), "ab".to_owned()]);
        let score = score_stats(&source, &candidate);

        assert_eq!(score.letters, 1.0);
        assert_eq!(score.bigrams, 1.0);
        assert_eq!(score.first_letters, 0.0);
        assert_eq!(score.average_word_length, 0.0);
        assert_eq!(score.max_error, 1.0);
    }

    #[test]
    fn counter_matches_slice_calculation() {
        let words = vec!["ab".to_owned(), "ac".to_owned(), "bbb".to_owned()];
        let expected = calculate_stats(&words);

        let mut counter = CorpusStatsCounter::default();
        for word in &words {
            counter.add_word(word);
        }

        assert_eq!(counter.finish(), expected);
    }
}
