use crate::app::synthesise::CachedSourceStats;
use rustc_hash::FxHashMap;

/// Compact corpus counts: first chars plus adjacent pairs.
#[derive(Debug, Default, Clone)]
pub struct CorpusCounts {
    /// How many words start with each character.
    pub first_chars: FxHashMap<char, u64>,

    /// How many times each adjacent character pair occurs within words.
    pub bigrams: FxHashMap<(char, char), u64>,
}

impl CorpusCounts {
    /// Fold one word's characters into the counts.
    #[cfg(test)]
    pub fn add(&mut self, word: &str) {
        let mut chars = word.chars();
        let Some(mut prev) = chars.next() else {
            return;
        };
        *self.first_chars.entry(prev).or_default() += 1;
        for c in chars {
            *self.bigrams.entry((prev, c)).or_default() += 1;
            prev = c;
        }
    }
}

/// Rebuild approximate counts from cached normalized stats.
impl From<&CachedSourceStats> for CorpusCounts {
    fn from(cached: &CachedSourceStats) -> Self {
        let words = cached.word_count as f64;
        let bigram_total = words * (cached.stats.average_word_length - 1.0).max(0.0);

        CorpusCounts {
            first_chars: cached
                .stats
                .first_letters
                .iter()
                .map(|(&c, &f)| (c, (f * words).round() as u64))
                .collect(),
            bigrams: cached
                .stats
                .bigrams
                .iter()
                .map(|(&[a, b], &f)| ((a, b), (f * bigram_total).round() as u64))
                .collect(),
        }
    }
}
