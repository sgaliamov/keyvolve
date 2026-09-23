mod config;
pub mod corpus;
pub mod penalty;
mod term_report;

#[allow(unused_imports)]
pub use crate::models::{Target, TargetType, Targets};
pub use config::LayoutEvaluatorConfig;
pub use corpus::CorpusCounts;
pub use term_report::TermReport;

use crate::models::{Keyboard, Keys, ScoreResult, row_distance, slot};
#[cfg(test)]
use crate::modes::synthesise::CachedSourceStats;
#[cfg(test)]
use itertools::Itertools;
use penalty::penalty;
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

    /// Static scoring knobs for diagnostics and penalty tuning.
    pub fn config(&self) -> &LayoutEvaluatorConfig {
        &self.config
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
        let from_finger = logical_finger(ka);
        let to_finger = logical_finger(kb);
        let same_finger = same_hand && from_finger == to_finger;

        let (effort, hand_switches, row_cost) = if same_hand {
            (self.lookup(ka, kb), 0, row_distance(ka, kb))
        } else {
            // Hands alternate: key `a` was already counted in the previous press.
            // Charge `b` as an independent press (self-effort, like the first letter).
            // The switch is recorded; row position on the other hand does not matter here.
            // Its price lives in the hand-switch factor at corpus level.
            (self.lookup(kb, kb), 1, 0)
        };

        let mut score = ScoreResult::press(kb, effort);
        let finger_row_cost = if same_finger { row_cost } else { 0 };
        score.hand_switches = hand_switches;
        // Row steps only matter same-hand; alternating hands ignore row distance.
        score.left_row_switch_cost = if b_left { finger_row_cost } else { 0 };
        score.right_row_switch_cost = if !b_left { finger_row_cost } else { 0 };
        score.left_finger_row_switch_cost[to_finger] = if b_left { finger_row_cost } else { 0 };
        score.right_finger_row_switch_cost[to_finger] = if !b_left { finger_row_cost } else { 0 };
        // Same-hand bigram lands wholly on one hand; alternating pairs add to neither.
        score.left_rolls = (same_hand && a_left) as u64;
        score.right_rolls = (same_hand && !a_left) as u64;
        score
    }

    /// Geometric same-finger skipgram penalty. True when key 3 reuses the same finger
    /// as key 1 while key 2 is not on that same finger; unlike the pair table, this is
    /// a structural indicator over three consecutive presses rather than a calibrated pair.
    fn score_sfs(&self, a: char, b: char, c: char, keys: &Keys) -> ScoreResult {
        let ka = slot(keys, a);
        let kb = slot(keys, b);
        let kc = slot(keys, c);
        ScoreResult {
            sfs_count: is_sfs(ka, kb, kc) as u64,
            ..Default::default()
        }
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

        let sfs = self
            .counts
            .trigrams
            .iter()
            .map(|(&(a, b, c), &n)| self.score_sfs(a, b, c, keys) * n);

        let mut result = seeds
            .chain(bigrams)
            .chain(sfs)
            .fold(ScoreResult::default(), |acc, x| acc + x);

        let penalty = penalty(&self.config, &result);

        // Fitness (CSV column) = scale / (effort · penalty). Higher = better.
        // - effort: raw bigram cost from the pairs table
        // - penalty: dimensionless multiplier built from per-press ratios
        result.fitness = self.config.fitness_scale / (result.effort * penalty);

        result
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

/// Placeholder char for empty/non-alpha genome slots.
pub const EMPTY_SLOT: char = '`';

/// Map physical slot to logical finger index: pinky, ring, middle, merged index.
#[inline]
fn logical_finger(slot: u8) -> usize {
    let column = if slot < 15 {
        (slot % 5) as usize
    } else {
        4 - (slot % 5) as usize
    };
    column.min(3)
}

#[inline]
fn same_finger(a: u8, b: u8) -> bool {
    (a < 15) == (b < 15) && logical_finger(a) == logical_finger(b)
}

#[inline]
fn is_sfs(a: u8, b: u8, c: u8) -> bool {
    same_finger(a, c) && !same_finger(a, b)
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
        assert_eq!(score.left_finger_row_switch_cost, [1, 0, 0, 0]);
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

    #[test]
    fn score_word_ignores_row_switch_when_finger_changes() {
        let evaluator = LayoutEvaluator::new(&row_switch_test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("af", &test_keys());

        assert_eq!(score.row_switch_distance(), 0);
        assert_eq!(score.left_finger_row_switch_cost, [0, 0, 0, 0]);
    }

    #[test]
    fn is_sfs_matches_same_finger_skipgram_geometry() {
        assert!(is_sfs(0, 15, 5));
        assert!(is_sfs(0, 7, 0));
        assert!(!is_sfs(0, 5, 10));
        assert!(!is_sfs(0, 1, 2));
        assert!(!is_sfs(0, 1, 1));
    }

    #[test]
    fn score_corpus_counts_same_finger_skipgrams() {
        let evaluator = LayoutEvaluator::new(&test_keyboard(), vec!["aba".to_string()], test_config());
        let score = evaluator.score_corpus(&Keys::from_iter([('a', 0), ('b', 1)]));

        assert_eq!(score.sfs_count, 1);
        assert_close(score.sfs_ratio(), 1.0 / 3.0);
    }

    #[test]
    fn score_word_treats_index_inner_outer_as_same_finger() {
        let evaluator = LayoutEvaluator::new(&row_switch_test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("gh", &test_keys());

        assert_eq!(score.row_switch_distance(), 1);
        assert_eq!(score.left_finger_row_switch_cost, [0, 0, 0, 1]);
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
        assert_eq!(rebuilt.trigrams, direct.trigrams);
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
                    "0": {"0": 0, "5": 1, "10": 2, "6": 1},
                    "5": {"5": 0},
                    "10": {"10": 0},
                    "3": {"3": 0, "9": 1},
                    "9": {"9": 0},
                    "6": {"6": 0}
                }
            })
            .to_string(),
        )
    }

    /// Build tiny layout for evaluator tests.
    fn test_keys() -> Keys {
        FxHashMap::from_iter([
            ('a', 0),
            ('b', 1),
            ('c', 19),
            ('d', 5),
            ('e', 10),
            ('f', 6),
            ('g', 3),
            ('h', 9),
        ])
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
