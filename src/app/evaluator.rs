use crate::app::{Keyboard, Keys, ScoreResult};
use itertools::Itertools;
use rustc_hash::FxHashMap;

/// Evaluates layouts by scoring words against a precomputed bigram effort table.
pub struct LayoutEvaluator {
    /// Flat bigram effort map: (from_key, to_key) → effort value.
    pairs: FxHashMap<(u8, u8), f64>,

    /// Switch multiplier; `1.0` means no penalty, `1.5` means +50%.
    switch_penalty: f64,
}

impl LayoutEvaluator {
    /// Build from keyboard config. Groups are 1-based → `efforts[group - 1]`.
    pub fn new(keyboard: &Keyboard) -> Self {
        let mut pairs = FxHashMap::default();

        for (from, targets) in &keyboard.pairs {
            for (to, group) in targets {
                let effort = keyboard.efforts[group - 1];
                pairs.insert((*from, *to), effort);
            }
        }

        LayoutEvaluator {
            pairs,
            switch_penalty: keyboard.switch_penalty,
        }
    }

    /// Score a single word against a layout.
    pub fn score_word(&self, word: &str, keys: &Keys) -> ScoreResult {
        let chars = word.chars().collect_vec();
        if chars.is_empty() {
            return ScoreResult::default();
        }

        let first_key = *keys.get(&chars[0]).expect("key not found in layout");
        let first_left = first_key < 15;

        // First character: self-effort as baseline, count the first key press.
        let effort = self.lookup(first_key, first_key);
        let seed = ScoreResult {
            effort,
            left_count: first_left as u32,
            right_count: (!first_left) as u32,
            left_effort: if first_left { effort } else { 0. },
            right_effort: if !first_left { effort } else { 0. },
            ..Default::default()
        };

        chars
            .iter()
            .tuple_windows()
            .map(|(a, b)| {
                let ka = *keys.get(a).expect("key not found in layout");
                let kb = *keys.get(b).expect("key not found in layout");
                (ka, kb)
            })
            .fold(seed, |acc, (ka, kb)| {
                let a_left = ka < 15;
                let b_left = kb < 15;

                let (effort, switches) = if a_left == b_left {
                    (self.lookup(ka, kb), 0)
                } else {
                    // When hands alternate, key `a` was already counted in the
                    // previous iteration.  We charge the self-effort of key `b`
                    // here because the new hand is starting a fresh sequence
                    // (analogous to the first-letter cost above), multiplied by
                    // `switch_penalty` so `1.0` means no extra cost.
                    (self.lookup(kb, kb) * self.switch_penalty, 1)
                };

                // count efforts on the "to" key, since "from" was already counted in the previous iteration
                let bigram = ScoreResult {
                    effort,
                    switches,
                    left_count: b_left as u32,
                    right_count: (!b_left) as u32,
                    left_effort: if b_left { effort } else { 0. },
                    right_effort: if !b_left { effort } else { 0. },
                };

                acc + bigram
            })
    }

    /// Score an entire corpus, applying a hand-balance factor to total effort.
    pub fn score_corpus(&self, words: &[&str], keys: &Keys) -> ScoreResult {
        let mut result = words
            .iter()
            .map(|w| self.score_word(w, keys))
            .fold(ScoreResult::default(), |acc, x| acc + x);

        // balance_factor is based on the actual usage of keys
        result.effort *= balance_factor(result.left_count.into(), result.right_count.into());
        result
    }

    /// Look up bigram effort, mirroring right-half keys (15–29) to left (0–14).
    #[inline]
    fn lookup(&self, from: u8, to: u8) -> f64 {
        *self.pairs.get(&(from, to)).unwrap()
    }
}

/// Multiplier ≥ 1 penalizing imbalanced effort. At 50/50 → 1.0, approaches 2 at extremes.
fn balance_factor(left: f64, right: f64) -> f64 {
    fn ratio(left: f64, right: f64) -> f64 {
        if left > right {
            left / right
        } else {
            right / left
        }
    }

    if left == 0. || right == 0. {
        return 1.;
    }

    let ratio = ratio(left, right);
    2. - (2. / ((ratio - 1.).powi(2) + 1.))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn score_word_returns_default_for_empty_word() {
        let evaluator = LayoutEvaluator::new(&test_keyboard());

        let score = evaluator.score_word("", &test_keys());

        assert_close(score.effort, 0.0);
        assert_eq!(score.left_count, 0);
        assert_eq!(score.right_count, 0);
        assert_eq!(score.switches, 0);
        assert_close(score.left_effort, 0.0);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_word_counts_same_hand_bigrams() {
        let evaluator = LayoutEvaluator::new(&test_keyboard());

        let score = evaluator.score_word("ab", &test_keys());

        assert_eq!(score.left_count, 2);
        assert_eq!(score.right_count, 0);
        assert_eq!(score.switches, 0);
        assert_close(score.effort, 3.0);
        assert_close(score.left_effort, 3.0);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_word_scores_same_key_from_pair_table() {
        let evaluator = LayoutEvaluator::new(&test_keyboard());

        let score = evaluator.score_word("aa", &test_keys());

        assert_eq!(score.left_count, 2);
        assert_eq!(score.right_count, 0);
        assert_eq!(score.switches, 0);
        assert_close(score.effort, 2.0);
        assert_close(score.left_effort, 2.0);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_word_applies_switch_penalty() {
        let evaluator = LayoutEvaluator::new(&test_keyboard());

        let score = evaluator.score_word("ac", &test_keys());

        assert_eq!(score.left_count, 1);
        assert_eq!(score.right_count, 1);
        assert_eq!(score.switches, 1);
        assert_close(score.effort, 2.5);
        assert_close(score.left_effort, 1.0);
        assert_close(score.right_effort, 1.5);
    }

    #[test]
    fn score_word_zero_switch_multiplier_zeroes_switch_effort() {
        let keyboard = Keyboard::new(
            json!({
                "switchPenalty": 0.0,
                "efforts": [1.0, 2.0],
                "pairs": {
                    "0": {"0": 1, "1": 2},
                    "1": {"1": 1, "0": 2}
                }
            })
            .to_string(),
        );
        let evaluator = LayoutEvaluator::new(&keyboard);

        let score = evaluator.score_word("ac", &test_keys());

        assert_close(score.effort, 1.0);
        assert_eq!(score.switches, 1);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_corpus_applies_balance_factor_after_aggregation() {
        let evaluator = LayoutEvaluator::new(&test_keyboard());
        let keys = test_keys();

        let score = evaluator.score_corpus(&["ab", "ac"], &keys);

        assert_eq!(score.left_count, 3);
        assert_eq!(score.right_count, 1);
        assert_eq!(score.switches, 1);
        assert_close(score.left_effort, 4.0);
        assert_close(score.right_effort, 1.5);
        assert_close(score.effort, 8.8); // (4.0 + 1.5) * balance_factor(3, 1) ≈ 5.5 * 1.6
    }

    /// Build minimal keyboard for evaluator tests using production JSON parsing.
    fn test_keyboard() -> Keyboard {
        Keyboard::new(
            json!({
                "switchPenalty": 1.5,
                "efforts": [1.0, 2.0, 3.0, 5.0],
                "pairs": {
                    "0": {"0": 1, "1": 2},
                    "1": {"1": 3, "0": 4}
                }
            })
            .to_string(),
        )
    }

    /// Build tiny layout for evaluator tests.
    fn test_keys() -> Keys {
        FxHashMap::from_iter([('a', 0), ('b', 1), ('c', 19)])
    }

    /// Compare floats without drama.
    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }
}
