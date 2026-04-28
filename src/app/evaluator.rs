use crate::app::{Keyboard, Keys, ScoreResult};
use itertools::Itertools;
use rustc_hash::FxHashMap;

/// Evaluates layouts by scoring words against a precomputed bigram effort table.
pub struct LayoutEvaluator {
    /// Flat bigram effort map: (from_key, to_key) → effort value.
    pairs: FxHashMap<(u8, u8), f64>,

    /// Multiplier applied to hand-switching bigrams, ≥ 1.
    switch_penalty: f64,

    /// Multiplier applied to same-key bigrams.
    same_key_penalty: f64,
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
            same_key_penalty: keyboard.same_key_penalty,
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
                    // Hand switch: charge self-effort of destination × switch penalty.
                    let effort = self.lookup(kb, kb) * self.switch_penalty;

                    ScoreResult {
                        effort,
                        switches: 1,
                        left_count: b_left as u32,
                        right_count: (!b_left) as u32,
                        left_effort: if both_left { effort } else { 0. },
                        right_effort: if both_right { effort } else { 0. },
                    }
                } else {
                    let effort = self.lookup(ka, kb);

                    let effort = if ka == kb {
                        effort * self.same_key_penalty
                    } else {
                        effort
                    };

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
