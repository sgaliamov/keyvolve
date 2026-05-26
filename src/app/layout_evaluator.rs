use crate::models::{Keyboard, Keys, ScoreResult, slot_row};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use serde::Deserialize;

/// Static scoring knobs for layout evaluation.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutEvaluatorConfig {
    /// Per-switch effort multiplier; `1.0` means no penalty, `1.5` means +50%.
    pub bigram_switch_penalty: f64,

    /// Max multiplier for extreme hand imbalance.
    pub balance_penalty: f64,

    /// Coefficient `k` for corpus-level alternation-rate penalty.
    pub alternation_penalty: f64,

    /// Coefficient `k` for weighted same-hand row-switch penalty.
    pub row_switch_penalty: f64,
}

impl Default for LayoutEvaluatorConfig {
    fn default() -> Self {
        Self {
            bigram_switch_penalty: 1.5,
            balance_penalty: 2.0,
            alternation_penalty: 0.25,
            row_switch_penalty: 0.25,
        }
    }
}

/// Evaluates layouts by scoring words against a precomputed bigram effort table.
#[derive(Clone)]
pub struct LayoutEvaluator {
    /// Flat bigram effort map: (from_key, to_key) → effort value.
    pairs: FxHashMap<(u8, u8), f64>,

    /// Static scoring knobs.
    config: LayoutEvaluatorConfig,

    /// Corpus words to evaluate.
    words: Vec<String>,
}

impl LayoutEvaluator {
    /// Build from keyboard config, corpus, and scoring config.
    pub fn new(keyboard: &Keyboard, words: Vec<String>, config: LayoutEvaluatorConfig) -> Self {
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
            words,
        }
    }

    /// Score a single word against a layout.
    fn score_word(&self, word: &str, keys: &Keys) -> ScoreResult {
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

                let (effort, bigram_switches, row_switch_cost) = if a_left == b_left {
                    (self.lookup(ka, kb), 0, row_switch_cost(ka, kb))
                } else {
                    // When hands alternate, key `a` was already counted in the
                    // previous iteration.  We charge the self-effort of key `b`
                    // here because the new hand is starting a fresh sequence
                    // (analogous to the first-letter cost above), multiplied by
                    // `bigram_switch_penalty` so `1.0` means no extra cost.
                    (
                        self.lookup(kb, kb) * self.config.bigram_switch_penalty,
                        1,
                        0,
                    )
                };

                // count efforts on the "to" key, since "from" was already counted in the previous iteration
                let bigram = ScoreResult {
                    effort,
                    fitness: 0.0,
                    bigram_switches,
                    row_switch_cost,
                    left_count: b_left as u32,
                    right_count: (!b_left) as u32,
                    left_effort: if b_left { effort } else { 0. },
                    right_effort: if !b_left { effort } else { 0. },
                };

                acc + bigram
            })
    }

    /// Score the corpus, applying a hand-balance factor to total effort.
    pub fn score_corpus(&self, keys: &Keys) -> ScoreResult {
        let mut result = self
            .words
            .iter()
            .map(|w| self.score_word(w, keys))
            .fold(ScoreResult::default(), |acc, x| acc + x);

        // balance_factor is based on the actual usage of keys
        result.fitness = result.effort;
        result.fitness *= balance_factor(
            result.left_count.into(),
            result.right_count.into(),
            self.config.balance_penalty,
        );
        result.fitness *= linear_rate_penalty(
            result.bigram_switches,
            result.left_count + result.right_count,
            self.config.alternation_penalty,
        );
        // Same-hand row changes only: same row = 0, adjacent row = 1, top↔bottom jump = 2.
        result.fitness *= linear_rate_penalty(
            result.row_switch_cost,
            result.left_count + result.right_count,
            self.config.row_switch_penalty,
        );
        result.fitness = 1. / result.fitness * 1_000_000.; // lower effort → higher fitness

        result
    }

    /// Look up precomputed bigram effort. Right-hand pairs were expanded at init by `Keyboard::expand_pairs`.
    #[inline]
    fn lookup(&self, from: u8, to: u8) -> f64 {
        *self.pairs.get(&(from, to)).unwrap()
    }
}

/// Weighted same-hand row-switch cost. Adjacent-row move = 1, jump-over-row = 2.
#[inline]
fn row_switch_cost(from: u8, to: u8) -> u32 {
    slot_row(from).abs_diff(slot_row(to)).into()
}

/// Multiplier ≥ 1 penalizing imbalanced effort. At 50/50 → 1.0, approaches `max` at extremes.
fn balance_factor(left: f64, right: f64, max: f64) -> f64 {
    fn ratio(left: f64, right: f64) -> f64 {
        if left > right {
            left / right
        } else {
            right / left
        }
    }

    if left == 0. || right == 0. {
        return max;
    }

    let ratio = ratio(left, right);
    max - ((max - 1.) / ((ratio - 1.).powi(2) + 1.))
}

/// Linear corpus-level penalty `1 + k * (count / (presses - 1))`.
/// `k` scales the penalty strength: `0.0` disables it, larger values increase the multiplier linearly.
/// Returns `1.0` when fewer than two presses exist, so no transition can happen.
fn linear_rate_penalty(count: u32, presses: u32, k: f64) -> f64 {
    if presses <= 1 {
        return 1.0;
    }

    1.0 + k * (count as f64 / (presses - 1) as f64)
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
        assert_eq!(score.bigram_switches, 0);
        assert_eq!(score.row_switch_cost, 0);
        assert_close(score.left_effort, 0.0);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_word_adds_pair_effort_to_same_hand() {
        let evaluator = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("ab", &test_keys());

        assert_eq!(score.left_count, 2);
        assert_eq!(score.right_count, 0);
        assert_eq!(score.bigram_switches, 0);
        assert_eq!(score.row_switch_cost, 0);
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
        assert_eq!(score.bigram_switches, 0);
        assert_eq!(score.row_switch_cost, 0);
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
        assert_eq!(score.bigram_switches, 1);
        assert_eq!(score.row_switch_cost, 0);
        assert_close(score.effort, 2.5);
        assert_close(score.fitness, 0.0);
        assert_close(score.left_effort, 1.0);
        assert_close(score.right_effort, 1.5);
    }

    #[test]
    fn score_word_zero_bigram_switch_penalty_removes_switch_cost() {
        let keyboard = Keyboard::new(
            json!({
                "efforts": [1.0, 2.0],
                "pairs": {
                    "0": {"0": 0, "1": 1},
                    "1": {"1": 0, "0": 1}
                }
            })
            .to_string(),
        );
        let evaluator = LayoutEvaluator::new(
            &keyboard,
            vec![],
            LayoutEvaluatorConfig {
                bigram_switch_penalty: 0.0,
                ..test_config()
            },
        );

        let score = evaluator.score_word("ac", &test_keys());

        assert_close(score.effort, 1.0);
        assert_close(score.fitness, 0.0);
        assert_eq!(score.bigram_switches, 1);
        assert_eq!(score.row_switch_cost, 0);
        assert_close(score.right_effort, 0.0);
    }

    #[test]
    fn score_word_counts_adjacent_same_hand_row_switch() {
        let evaluator = LayoutEvaluator::new(&row_switch_test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("ad", &test_keys());

        assert_eq!(score.bigram_switches, 0);
        assert_eq!(score.row_switch_cost, 1);
        assert_close(score.effort, 3.0);
    }

    #[test]
    fn score_word_counts_jump_row_switch_as_double() {
        let evaluator = LayoutEvaluator::new(&row_switch_test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("ae", &test_keys());

        assert_eq!(score.bigram_switches, 0);
        assert_eq!(score.row_switch_cost, 2);
        assert_close(score.effort, 5.0);
    }

    #[test]
    fn score_corpus_applies_balance_penalty_to_aggregated_effort() {
        let evaluator = LayoutEvaluator::new(
            &test_keyboard(),
            vec!["ab".to_string(), "ac".to_string()],
            test_config(),
        );
        let keys = test_keys();

        let score = evaluator.score_corpus(&keys);

        assert_eq!(score.left_count, 3);
        assert_eq!(score.right_count, 1);
        assert_eq!(score.bigram_switches, 1);
        assert_eq!(score.row_switch_cost, 0);
        assert_close(score.left_effort, 4.0);
        assert_close(score.right_effort, 1.5);
        assert_close(score.effort, 5.5);
        assert_close(score.fitness, 101010.1);
    }

    #[test]
    fn balance_factor_returns_two_for_zero_hand_usage() {
        assert_close(balance_factor(0.0, 3.0, 2.0), 2.0);
        assert_close(balance_factor(3.0, 0.0, 2.0), 2.0);
        assert_close(balance_factor(0.0, 0.0, 2.0), 2.0);
    }

    #[test]
    fn balance_factor_returns_one_for_even_usage() {
        assert_close(balance_factor(1.0, 1.0, 2.0), 1.0);
        assert_close(balance_factor(5.0, 5.0, 2.0), 1.0);
    }

    #[test]
    fn balance_factor_is_symmetric_between_hands() {
        assert_close(balance_factor(3.0, 1.0, 2.0), balance_factor(1.0, 3.0, 2.0));
    }

    #[test]
    fn balance_factor_grows_with_hand_imbalance() {
        assert!(balance_factor(3.0, 2.0, 2.0) < balance_factor(3.0, 1.0, 2.0));
        assert!(balance_factor(3.0, 1.0, 2.0) < balance_factor(10.0, 1.0, 2.0));
    }

    #[test]
    fn balance_factor_respects_configured_max_penalty() {
        assert_close(balance_factor(1.0, 1.0, 3.0), 1.0);
        assert_close(balance_factor(0.0, 1.0, 3.0), 3.0);
        assert!(balance_factor(3.0, 1.0, 2.0) < balance_factor(3.0, 1.0, 3.0));
    }

    #[test]
    fn linear_rate_penalty_returns_one_without_transitions() {
        assert_close(linear_rate_penalty(0, 0, 0.5), 1.0);
        assert_close(linear_rate_penalty(0, 1, 0.5), 1.0);
    }

    #[test]
    fn linear_rate_penalty_scales_with_transition_rate() {
        assert_close(linear_rate_penalty(0, 3, 0.5), 1.0);
        assert_close(linear_rate_penalty(1, 3, 0.5), 1.25);
        assert_close(linear_rate_penalty(2, 3, 0.5), 1.5);
    }

    #[test]
    fn score_corpus_applies_configured_alternation_penalty() {
        let evaluator = LayoutEvaluator::new(
            &Keyboard::new(
                json!({
                    "efforts": [1.0, 2.0, 3.0, 5.0],
                    "pairs": {
                        "0": {"0": 0, "1": 1},
                        "1": {"1": 2, "0": 3}
                    }
                })
                .to_string(),
            ),
            vec!["ab".to_string(), "ac".to_string()],
            LayoutEvaluatorConfig {
                alternation_penalty: 0.5,
                ..test_config()
            },
        );

        let score = evaluator.score_corpus(&test_keys());

        assert_close(score.fitness, 86580.09);
    }

    #[test]
    fn score_corpus_applies_configured_row_switch_penalty() {
        let evaluator = LayoutEvaluator::new(
            &row_switch_test_keyboard(),
            vec!["ad".to_string()],
            LayoutEvaluatorConfig {
                row_switch_penalty: 0.5,
                ..test_config()
            },
        );

        let score = evaluator.score_corpus(&test_keys());

        assert_eq!(score.row_switch_cost, 1);
        assert_close(score.fitness, 111111.11);
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

    fn test_config() -> LayoutEvaluatorConfig {
        LayoutEvaluatorConfig {
            bigram_switch_penalty: 1.5,
            balance_penalty: 2.0,
            alternation_penalty: 0.0,
            row_switch_penalty: 0.0,
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
