use crate::evaluator::LayoutEvaluatorConfig;
use crate::evaluator::corpus::CorpusCounts;
use crate::evaluator::keys::{row_distance, slot};
use crate::evaluator::penalty::{breakdown, penalty};
use crate::models::{Keyboard, Keys, ScoreResult};
#[cfg(test)]
use crate::modes::synthesise::CachedSourceStats;
#[cfg(test)]
use itertools::Itertools;
use rustc_hash::FxHashMap;

/// Evaluates layouts by scoring a corpus against a precomputed bigram effort table.
#[derive(Clone)]
pub struct LayoutEvaluator {
    /// Flat bigram effort map: (from_key, to_key) → effort value.
    pairs: FxHashMap<(u8, u8), f64>,

    /// Static scoring knobs.
    config: LayoutEvaluatorConfig,

    /// Corpus collapsed to first-char + bigram frequencies.
    counts: CorpusCounts,
}

impl LayoutEvaluator {
    /// Build from an in-memory word list (tests and small inputs).
    #[cfg(test)]
    pub fn new(keyboard: &Keyboard, words: Vec<String>, config: LayoutEvaluatorConfig) -> Self {
        let mut counts = CorpusCounts::default();
        for word in &words {
            counts.add(word);
        }
        Self::from_counts(keyboard, counts, config)
    }

    /// Build from precomputed corpus frequencies (streaming path for large corpora).
    pub fn from_counts(
        keyboard: &Keyboard,
        counts: CorpusCounts,
        config: LayoutEvaluatorConfig,
    ) -> Self {
        let mut pairs = FxHashMap::default();

        for (from, targets) in &keyboard.pairs {
            for (to, group) in targets {
                let effort = keyboard.efforts[*group];
                pairs.insert((*from, *to), effort);
            }
        }

        LayoutEvaluator {
            pairs,
            config,
            counts,
        }
    }

    /// Score a single word against a layout. Test-only; production scores via
    /// [`Self::score_corpus`] over the precomputed frequency maps.
    #[cfg(test)]
    fn score_word(&self, word: &str, keys: &Keys) -> ScoreResult {
        let mut chars = word.chars();
        let Some(first) = chars.next() else {
            return ScoreResult::default();
        };

        word.chars()
            .tuple_windows()
            .fold(self.score_first(first, keys), |acc, (a, b)| {
                acc + self.score_bigram(a, b, keys)
            })
    }

    /// Seed cost for a word's first character: self-effort baseline, one key press.
    fn score_first(&self, c: char, keys: &Keys) -> ScoreResult {
        let key = slot(keys, c);
        let effort = self.lookup(key, key);
        ScoreResult::press(key, effort)
    }

    /// Cost of one adjacent character pair within a word. Effort charged on the
    /// "to" key, since "from" was already counted by the previous press.
    fn score_bigram(&self, a: char, b: char, keys: &Keys) -> ScoreResult {
        let ka = slot(keys, a);
        let kb = slot(keys, b);
        let a_left = ka < 15;
        let b_left = kb < 15;
        let same_hand = a_left == b_left;

        let (effort, hand_switches, row_cost) = if same_hand {
            (self.lookup(ka, kb), 0, row_distance(ka, kb))
        } else {
            // Hands alternate: key `a` was already counted in the previous press.
            // Charge `b` as an independent press (self-effort, like the first letter).
            // The switch is recorded; its price lives in the `mean_streak_power` factor
            // at corpus level, since a switch is exactly what ends a run.
            (self.lookup(kb, kb), 1, 0)
        };

        let mut score = ScoreResult::press(kb, effort);
        score.hand_switches = hand_switches;
        // Row steps only occur same-hand; charge them to that hand.
        score.left_row_switch_cost = if b_left { row_cost } else { 0 };
        score.right_row_switch_cost = if !b_left { row_cost } else { 0 };
        // Same-hand bigram lands wholly on one hand; alternating pairs add to neither.
        score.left_rolls = (same_hand && a_left) as u64;
        score.right_rolls = (same_hand && !a_left) as u64;
        score
    }

    /// Score the corpus: raw effort scaled by uniform multiplicative penalty factors.
    pub fn score_corpus(&self, keys: &Keys) -> ScoreResult {
        let seeds = self
            .counts
            .first_chars
            .iter()
            .map(|(&c, &n)| self.score_first(c, keys) * n);

        let bigrams = self
            .counts
            .bigrams
            .iter()
            .map(|(&(a, b), &n)| self.score_bigram(a, b, keys) * n);

        let mut result = seeds
            .chain(bigrams)
            .fold(ScoreResult::default(), |acc, x| acc + x);

        let penalty = penalty(&self.config, &result);

        // Fitness (CSV column) = scale / (effort · penalty). Higher = better.
        // - effort: raw bigram cost from the pairs table
        // - penalty: dimensionless multiplier built from per-press ratios
        result.fitness = self.config.fitness_scale / (result.effort * penalty);

        result
    }

    /// Per-metric penalty contributions for a scored layout, worst first. Empty in
    /// powers mode, where the factors are not attributable to single metrics.
    pub fn breakdown(&self, score: &ScoreResult) -> Vec<(&'static str, f64)> {
        breakdown(&self.config, score)
    }

    /// Look up precomputed bigram effort. Right-hand pairs were expanded at init by `Keyboard::expand_pairs`.
    #[inline]
    fn lookup(&self, from: u8, to: u8) -> f64 {
        *self
            .pairs
            .get(&(from, to))
            .unwrap_or_else(|| panic!("no pair effort for keys ({from}, {to})"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn score_word_returns_zero_score_for_empty_input() {
        let evaluator = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("", &test_keys());

        assert_close(score.effort, 0.0);
        assert_close(score.fitness, 0.0);
        assert_eq!(score.left_count, 0);
        assert_eq!(score.right_count, 0);
        assert_eq!(score.hand_switches, 0);
        assert_eq!(score.row_switch_distance(), 0);
        assert_close(score.left_effort, 0.0);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_word_adds_pair_effort_to_same_hand() {
        let evaluator = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("ab", &test_keys());

        assert_eq!(score.left_count, 2);
        assert_eq!(score.right_count, 0);
        assert_eq!(score.left_rolls, 1);
        assert_eq!(score.right_rolls, 0);
        assert_eq!(score.hand_switches, 0);
        assert_eq!(score.row_switch_distance(), 0);
        assert_close(score.effort, 3.0);
        assert_close(score.fitness, 0.0);
        assert_close(score.left_effort, 3.0);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_word_uses_pair_table_for_repeated_key() {
        let evaluator = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("aa", &test_keys());

        assert_eq!(score.left_count, 2);
        assert_eq!(score.right_count, 0);
        assert_eq!(score.hand_switches, 0);
        assert_eq!(score.row_switch_distance(), 0);
        assert_close(score.effort, 2.0);
        assert_close(score.fitness, 0.0);
        assert_close(score.left_effort, 2.0);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_word_charges_self_effort_on_hand_switch() {
        let evaluator = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("ac", &test_keys());

        assert_eq!(score.left_count, 1);
        assert_eq!(score.right_count, 1);
        assert_eq!(score.left_rolls, 0);
        assert_eq!(score.right_rolls, 0);
        assert_eq!(score.hand_switches, 1);
        assert_eq!(score.row_switch_distance(), 0);
        assert_close(score.effort, 2.0);
        assert_close(score.fitness, 0.0);
        assert_close(score.left_effort, 1.0);
        assert_close(score.right_effort, 1.0);
    }

    #[test]
    fn score_word_yields_average_hand_streaks() {
        let evaluator = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());

        // "abc" hands L L R: left run of 2, right run of 1.
        let score = evaluator.score_word("abc", &test_keys());

        assert_close(score.left_streak(), 2.0);
        assert_close(score.right_streak(), 1.0);
    }

    #[test]
    fn score_word_counts_adjacent_same_hand_row_switch() {
        let evaluator = LayoutEvaluator::new(&row_switch_test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("ad", &test_keys());

        assert_eq!(score.hand_switches, 0);
        assert_eq!(score.row_switch_distance(), 1);
        assert_close(score.effort, 3.0);
    }

    #[test]
    fn score_word_counts_jump_row_switch_as_double() {
        let evaluator = LayoutEvaluator::new(&row_switch_test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("ae", &test_keys());

        assert_eq!(score.hand_switches, 0);
        assert_eq!(score.row_switch_distance(), 2);
        assert_close(score.effort, 5.0);
    }

    /// A run ends only at a hand switch or a word boundary, so `runs = switches + words`
    /// and `mean_streak = presses / runs`. Checked across layouts that spread the same
    /// corpus over the hands differently: the identity is structural, not a coincidence.
    /// This is why the streak divisor subsumes a separate hand-switch knob.
    #[test]
    fn mean_streak_equals_presses_over_runs() {
        let words: Vec<String> = ["abc", "cab", "bca"].map(String::from).to_vec();
        let evaluator = LayoutEvaluator::new(&test_keyboard(), words.clone(), test_config());

        for layout in [
            [('a', 0), ('b', 1), ('c', 19)],
            [('a', 0), ('b', 19), ('c', 18)],
            [('a', 1), ('b', 0), ('c', 18)],
        ] {
            let score = evaluator.score_corpus(&Keys::from_iter(layout));
            let presses = (score.left_count + score.right_count) as f64;
            let runs = score.hand_switches as f64 + words.len() as f64;

            assert_close(score.mean_streak(), presses / runs);
        }
    }

    #[test]
    fn counts_from_cached_stats_match_direct_counts() {
        use crate::modes::synthesise::CorpusStatsCounter;

        let words = ["abc", "cab", "aa", "bca", "cc"];
        let mut direct = CorpusCounts::default();
        let mut counter = CorpusStatsCounter::default();
        for w in words {
            direct.add(w);
            counter.add_word(w);
        }

        let cached = CachedSourceStats {
            stats: counter.finish(),
            word_count: words.len(),
        };
        let rebuilt = CorpusCounts::from(&cached);

        assert_eq!(rebuilt.first_chars, direct.first_chars);
        assert_eq!(rebuilt.bigrams, direct.bigrams);
    }

    /// Build minimal keyboard for evaluator tests using production JSON parsing.
    fn test_keyboard() -> Keyboard {
        Keyboard::new(
            json!({
                "efforts": [1.0, 2.0, 3.0, 5.0],
                "pairs": {
                    "0": {"0": 0, "1": 1},
                    "1": {"1": 2, "0": 3}
                }
            })
            .to_string(),
        )
    }

    /// Build keyboard that covers same-hand row transitions used by row-switch tests.
    fn row_switch_test_keyboard() -> Keyboard {
        Keyboard::new(
            json!({
                "efforts": [1.0, 2.0, 4.0],
                "pairs": {
                    "0": {"0": 0, "5": 1, "10": 2},
                    "5": {"5": 0},
                    "10": {"10": 0}
                }
            })
            .to_string(),
        )
    }

    /// Build tiny layout for evaluator tests.
    fn test_keys() -> Keys {
        FxHashMap::from_iter([('a', 0), ('b', 1), ('c', 19), ('d', 5), ('e', 10)])
    }

    /// Minimal config fixture for evaluator tests; targets empty.
    fn test_config() -> LayoutEvaluatorConfig {
        LayoutEvaluatorConfig {
            fitness_scale: 1_000_000.,
            ..Default::default()
        }
    }

    /// Compare floats without drama.
    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-2,
            "expected {expected}, got {actual}"
        );
    }
}
