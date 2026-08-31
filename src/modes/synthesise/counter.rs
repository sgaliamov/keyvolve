use rustc_hash::FxHashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::hash::Hash;

/// Serde helper: serialize `FxHashMap<[char; 2], f64>` as `{"ab": 0.5, ...}`.
mod bigram_map_serde {
    use super::*;

    pub fn serialize<S: Serializer>(
        map: &FxHashMap<[char; 2], f64>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut entries: Vec<_> = map.iter().collect();
        entries.sort_by(|a, b| b.1.total_cmp(a.1));
        let mut m = s.serialize_map(Some(map.len()))?;
        for (k, v) in entries {
            let key: String = k.iter().collect();
            m.serialize_entry(&key, v)?;
        }
        m.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<FxHashMap<[char; 2], f64>, D::Error> {
        let raw: FxHashMap<String, f64> = FxHashMap::deserialize(d)?;
        raw.into_iter()
            .map(|(k, v)| {
                let mut chars = k.chars();
                let a = chars
                    .next()
                    .ok_or_else(|| serde::de::Error::custom("empty bigram key"))?;
                let b = chars
                    .next()
                    .ok_or_else(|| serde::de::Error::custom("bigram key too short"))?;
                Ok(([a, b], v))
            })
            .collect()
    }
}

/// Serde helper: serialize `FxHashMap<char, f64>` sorted by value descending.
mod char_map_serde {
    use super::*;

    pub fn serialize<S: Serializer>(map: &FxHashMap<char, f64>, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut entries: Vec<_> = map.iter().collect();
        entries.sort_by(|a, b| b.1.total_cmp(a.1));
        let mut m = s.serialize_map(Some(map.len()))?;
        for (k, v) in entries {
            m.serialize_entry(&k.to_string(), v)?;
        }
        m.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<FxHashMap<char, f64>, D::Error> {
        FxHashMap::deserialize(d)
    }
}

/// Corpus metrics used by synthesise mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusStats {
    /// normalized letter frequencies
    #[serde(with = "char_map_serde")]
    pub letters: FxHashMap<char, f64>,
    /// normalized bigram frequencies
    #[serde(with = "bigram_map_serde")]
    pub bigrams: FxHashMap<[char; 2], f64>,
    /// normalized first-letter frequencies
    #[serde(with = "char_map_serde")]
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
    /// average letter relative error
    pub letters: f64,
    /// average bigram relative error
    pub bigrams: f64,
    /// average first-letter relative error
    pub first_letters: f64,
    /// average word length relative error
    pub average_word_length: f64,
    /// max error across all metrics
    pub max_error: f64,
}

/// Build normalized stats from a word slice.
#[cfg(test)]
pub fn calculate_stats(words: &[String]) -> CorpusStats {
    let mut counter = CorpusStatsCounter::default();
    for word in words {
        counter.add_word(word);
    }
    counter.finish()
}

/// Compare source and candidate stats with average relative error per metric.
pub fn score_stats(source: &CorpusStats, candidate: &CorpusStats) -> CorpusScore {
    let letters = avg_map_error(&source.letters, &candidate.letters);
    let bigrams = avg_map_error(&source.bigrams, &candidate.bigrams);
    let first_letters = avg_map_error(&source.first_letters, &candidate.first_letters);
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

/// Total variation distance between two distributions: `0.5 * Σ|p(k) - q(k)|`.
/// Range [0, 1]. Mass-weighted, so sparse tail noise doesn't dominate the way it
/// does with mean-of-relative-errors. Missing keys count as 0.
fn avg_map_error<K: Copy + Eq + Hash>(
    source: &FxHashMap<K, f64>,
    candidate: &FxHashMap<K, f64>,
) -> f64 {
    let all_keys: rustc_hash::FxHashSet<K> =
        source.keys().chain(candidate.keys()).copied().collect();
    let sum: f64 = all_keys
        .into_iter()
        .map(|k| {
            let p = source.get(&k).copied().unwrap_or(0.0);
            let q = candidate.get(&k).copied().unwrap_or(0.0);
            (p - q).abs()
        })
        .sum();
    0.5 * sum
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
    fn score_stats_uses_total_variation_distance() {
        let source = calculate_stats(&["ab".to_owned(), "ac".to_owned()]);
        let candidate = calculate_stats(&["ab".to_owned(), "ab".to_owned()]);
        let score = score_stats(&source, &candidate);

        // letters: a 0.5 vs 0.5, b 0.25 vs 0.5, c 0.25 vs 0.0 → TVD = 0.5*(0+0.25+0.25) = 0.25
        assert!((score.letters - 0.25).abs() < 1e-9);
        // bigrams: ab 0.5 vs 1.0, ac 0.5 vs 0.0 → TVD = 0.5*(0.5+0.5) = 0.5
        assert!((score.bigrams - 0.5).abs() < 1e-9);
        assert_eq!(score.first_letters, 0.0);
        assert_eq!(score.average_word_length, 0.0);
        assert!((score.max_error - 0.5).abs() < 1e-9);
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
