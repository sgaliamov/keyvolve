use crate::app::{Keyboard, layout::Keys};
use itertools::Itertools;
use rustc_hash::FxHashMap;

/// Full breakdown of a scoring pass over a word or corpus.
#[derive(Debug, Clone, Default)]
pub struct ScoreResult {
    /// Total weighted effort, with balance factor applied at corpus level.
    pub effort: f64,
    /// Number of consecutive same-hand bigrams on the left.
    pub left_count: u32,
    /// Number of consecutive same-hand bigrams on the right.
    pub right_count: u32,
    /// Number of hand switches.
    pub switches: u32,
    /// Effort accumulated on the left hand.
    pub left_effort: f64,
    /// Effort accumulated on the right hand.
    pub right_effort: f64,
}

impl ScoreResult {
    fn add(self, other: ScoreResult) -> Self {
        ScoreResult {
            effort: self.effort + other.effort,
            left_count: self.left_count + other.left_count,
            right_count: self.right_count + other.right_count,
            switches: self.switches + other.switches,
            left_effort: self.left_effort + other.left_effort,
            right_effort: self.right_effort + other.right_effort,
        }
    }
}

/// Evaluates layouts by scoring words against a precomputed bigram effort table.
pub struct LayoutEvaluator {
    /// Flat bigram effort map: (from_key, to_key) → effort value.
    pairs: FxHashMap<(u8, u8), f64>,
    switch_penalty: f64,
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

        let first_key = match keys.get(&chars[0]) {
            Some(&k) => k,
            None => return ScoreResult::default(),
        };
        // First character: self-effort as baseline, no predecessor.
        let first_effort = self.lookup(first_key, first_key);
        let seed = ScoreResult { effort: first_effort, ..Default::default() };

        chars
            .iter()
            .tuple_windows()
            .filter_map(|(a, b)| {
                let ka = *keys.get(a)?;
                let kb = *keys.get(b)?;
                Some((ka, kb))
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
                        left_effort: if both_left { effort } else { 0. },
                        right_effort: if both_right { effort } else { 0. },
                        ..Default::default()
                    }
                } else {
                    let effort = self.lookup(ka, kb);
                    let effort = if ka == kb { effort * self.same_key_penalty } else { effort };
                    ScoreResult {
                        effort,
                        left_count: both_left as u32,
                        right_count: both_right as u32,
                        left_effort: if both_left { effort } else { 0. },
                        right_effort: if both_right { effort } else { 0. },
                        ..Default::default()
                    }
                };

                acc.add(bigram)
            })
    }

    /// Score an entire corpus, applying a hand-balance factor to total effort.
    pub fn score_corpus(&self, words: &[&str], keys: &Keys) -> ScoreResult {
        let mut result = words
            .iter()
            .map(|w| self.score_word(w, keys))
            .fold(ScoreResult::default(), ScoreResult::add);

        result.effort *= balance_factor(result.left_effort, result.right_effort);
        result
    }

    /// Look up bigram effort, mirroring right-half keys (15–29) to left (0–14).
    #[inline]
    fn lookup(&self, from: u8, to: u8) -> f64 {
        let from = from % 15;
        let to = to % 15;
        self.pairs.get(&(from, to)).copied().unwrap_or(0.)
    }
}

/// Multiplier ≥ 1 penalising imbalanced effort. At 50/50 → 1.0, approaches 3 at extremes.
fn balance_factor(left: f64, right: f64) -> f64 {
    if left == 0. || right == 0. {
        return 1.;
    }
    let ratio = balance_ratio(left, right);
    3. - (2. / ((ratio - 1.).powi(2) + 1.))
}

fn balance_ratio(left: f64, right: f64) -> f64 {
    if left > right { left / right } else { right / left }
}
