use crate::app::{Keyboard, Keys, ScoreResult};
use itertools::Itertools;
use rustc_hash::FxHashMap;

/// Evaluates layouts by scoring words against a precomputed bigram effort table.
pub struct LayoutEvaluator {
    /// Flat bigram effort map: (from_key, to_key) → effort value.
    pairs: FxHashMap<(u8, u8), f64>,

    /// Multiplier applied to hand-switching bigrams, ≥ 1.
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

        // First character: self-effort as baseline, count the first key press.
        let first_effort = self.lookup(first_key, first_key);
        let seed = ScoreResult {
            effort: first_effort,
            left_count: if first_key < 15 { 1 } else { 0 },
            right_count: if first_key >= 15 { 1 } else { 0 },
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
                let switching = a_left != b_left;
                let both_left = a_left && b_left;
                let both_right = !a_left && !b_left;

                let bigram = if switching {
                    // When hands alternate, key `a` was already counted in the
                    // previous iteration.  We charge the self-effort of key `b`
                    // here because the new hand is starting a fresh sequence
                    // (analogous to the first-letter cost above), multiplied by
                    // the switch penalty to discourage frequent alternation on
                    // high-cost positions.
                    let effort = self.lookup(kb, kb) * self.switch_penalty;

                    ScoreResult {
                        effort,
                        switches: 1,
                        left_count: b_left as u32,
                        right_count: (!b_left) as u32,
                        left_effort: if b_left { effort } else { 0. },
                        right_effort: if !b_left { effort } else { 0. },
                    }
                } else {
                    let effort = self.lookup(ka, kb);

                    ScoreResult {
                        effort,
                        left_count: b_left as u32,
                        right_count: (!b_left) as u32,
                        left_effort: if both_left { effort } else { 0. },
                        right_effort: if both_right { effort } else { 0. },
                        ..Default::default()
                    }
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

        result.effort *= balance_factor(result.left_effort, result.right_effort);
        result
    }

    /// Look up bigram effort, mirroring right-half keys (15–29) to left (0–14).
    #[inline]
    fn lookup(&self, from: u8, to: u8) -> f64 {
        *self.pairs.get(&(from, to)).unwrap()
    }
}

/// Multiplier ≥ 1 penalizing imbalanced effort. At 50/50 → 1.0, approaches 3 at extremes.
fn balance_factor(left: f64, right: f64) -> f64 {
    if left == 0. || right == 0. {
        return 1.;
    }
    let ratio = balance_ratio(left, right);
    3. - (2. / ((ratio - 1.).powi(2) + 1.))
}

fn balance_ratio(left: f64, right: f64) -> f64 {
    if left > right {
        left / right
    } else {
        right / left
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirror left-hand key index to matching right-hand index.
    fn mirror_key(i: u8) -> u8 {
        (i / 5) * 5 + (4 - i % 5) + 15
    }

    /// Expand left-hand pair groups like production keyboard loading.
    fn expand_test_pairs(pairs: &mut FxHashMap<u8, FxHashMap<u8, usize>>) {
        let left = pairs
            .iter()
            .flat_map(|(from, targets)| targets.iter().map(move |(to, group)| (*from, *to, *group)))
            .collect_vec();

        for (from, to, group) in left {
            pairs.entry(mirror_key(from)).or_default().entry(mirror_key(to)).or_insert(group);
        }
    }

    /// Build minimal keyboard for evaluator tests.
    fn test_keyboard() -> Keyboard {
        let mut keyboard = Keyboard {
            frozen: FxHashMap::default(),
            blocked: vec![],
            switch_penalty: 1.5,
            efforts: vec![1.0, 2.0, 4.0],
            pairs: FxHashMap::from_iter([
                (0, FxHashMap::from_iter([(0, 1), (1, 2)])),
                (1, FxHashMap::from_iter([(1, 1), (0, 2)])),
            ]),
        };

        expand_test_pairs(&mut keyboard.pairs);
        keyboard
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

        assert_close(score.effort, 3.0);
        assert_eq!(score.left_count, 2);
        assert_eq!(score.right_count, 0);
        assert_eq!(score.switches, 0);
        assert_close(score.left_effort, 2.0);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_word_scores_same_key_from_pair_table() {
        let evaluator = LayoutEvaluator::new(&test_keyboard());

        let score = evaluator.score_word("aa", &test_keys());

        assert_close(score.effort, 2.0);
        assert_eq!(score.left_count, 2);
        assert_eq!(score.right_count, 0);
        assert_eq!(score.switches, 0);
        assert_close(score.left_effort, 1.0);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_word_applies_switch_penalty() {
        let evaluator = LayoutEvaluator::new(&test_keyboard());

        let score = evaluator.score_word("ac", &test_keys());

        assert_close(score.effort, 2.5);
        assert_eq!(score.left_count, 1);
        assert_eq!(score.right_count, 1);
        assert_eq!(score.switches, 1);
        assert_close(score.left_effort, 0.0);
        assert_close(score.right_effort, 1.5);
    }

    #[test]
    fn score_corpus_applies_balance_factor_after_aggregation() {
        let evaluator = LayoutEvaluator::new(&test_keyboard());
        let keys = test_keys();

        let score = evaluator.score_corpus(&["ab", "ac"], &keys);

        assert_eq!(score.left_count, 3);
        assert_eq!(score.right_count, 1);
        assert_eq!(score.switches, 1);
        assert_close(score.left_effort, 2.0);
        assert_close(score.right_effort, 1.5);
        assert_close(score.effort, 6.6);
    }
}
