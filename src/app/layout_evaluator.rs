use crate::models::{Keyboard, Keys, ScoreResult, slot_row};
#[cfg(test)]
use itertools::Itertools;
use rustc_hash::FxHashMap;
use serde::Deserialize;

/// Static scoring knobs for layout evaluation.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutEvaluatorConfig {
    /// Coefficient `k` for corpus-level hand-switch-rate penalty.
    pub bigram_switch_penalty: f64,

    /// Coefficient `k` for weighted same-hand row-switch penalty.
    pub row_switch_penalty: f64,

    /// Extra effort multiplier applied to every pinky key press; `1.0` disables it.
    pub pinky_multiplier: f64,
}

impl Default for LayoutEvaluatorConfig {
    fn default() -> Self {
        Self {
            bigram_switch_penalty: 0.25,
            row_switch_penalty: 0.25,
            pinky_multiplier: 1.1,
        }
    }
}

/// Compact corpus representation: first-character and bigram frequencies.
/// Built by streaming so a multi-GB corpus never lands in memory whole.
#[derive(Debug, Default, Clone)]
pub struct CorpusCounts {
    /// How many words start with each character.
    pub first_chars: FxHashMap<char, u64>,

    /// How many times each adjacent character pair occurs within words.
    pub bigrams: FxHashMap<(char, char), u64>,
}

impl CorpusCounts {
    /// Fold one word's characters into the counts.
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
        let key = *keys.get(&c).expect("key not found in layout");
        let left = key < 15;
        let effort = self.lookup(key, key) * self.pinky_mul(key);
        ScoreResult {
            effort,
            left_count: left as u64,
            right_count: !left as u64,
            left_effort: if left { effort } else { 0. },
            right_effort: if !left { effort } else { 0. },
            ..Default::default()
        }
    }

    /// Cost of one adjacent character pair within a word. Effort charged on the
    /// "to" key, since "from" was already counted by the previous press.
    fn score_bigram(&self, a: char, b: char, keys: &Keys) -> ScoreResult {
        let ka = *keys.get(&a).expect("key not found in layout");
        let kb = *keys.get(&b).expect("key not found in layout");
        let a_left = ka < 15;
        let b_left = kb < 15;
        let same_hand = a_left == b_left;

        let (effort, bigram_switches, row_switch_cost) = if same_hand {
            (
                self.lookup(ka, kb) * self.pinky_mul(kb),
                0,
                row_distance(ka, kb),
            )
        } else {
            // Hands alternate: key `a` was already counted in the previous press.
            // Charge `b` as an independent press (self-effort, like the first letter).
            // The switch is recorded; corpus-wide pressure lives in `bigram_switch_penalty`.
            (self.lookup(kb, kb) * self.pinky_mul(kb), 1, 0)
        };

        ScoreResult {
            effort,
            fitness: 0.0,
            bigram_switches,
            row_switch_cost,
            left_count: b_left as u64,
            right_count: !b_left as u64,
            // Same-hand bigram lands wholly on one hand; alternating pairs add to neither.
            left_rolls: (same_hand && a_left) as u64,
            right_rolls: (same_hand && !a_left) as u64,
            left_effort: if b_left { effort } else { 0. },
            right_effort: if !b_left { effort } else { 0. },
        }
    }

    /// Score the corpus, applying a hand-balance factor to total effort.
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

        // Mean effort per keypress: dividing by total presses makes fitness
        // independent of corpus size, so layouts compare equally across input lengths.
        let presses = (result.left_count + result.right_count).max(1) as f64;

        result.fitness = result.effort / presses;
        result.fitness *= imbalance_ratio(result.left_count, result.right_count);
        result.fitness *= imbalance_ratio(result.left_rolls, result.right_rolls);

        result.fitness *= linear_rate_penalty(
            result.bigram_switches,
            result.left_count + result.right_count,
            self.config.bigram_switch_penalty,
        );

        // Same-hand row changes only: same row = 0, adjacent row = 1, top ↔ bottom jump = 2.
        // Rate measured over same-hand presses only — hand switches carry no row cost,
        // so excluding them keeps the rate undiluted by alternation.
        result.fitness *= linear_rate_penalty(
            result.row_switch_cost,
            (result.left_count + result.right_count).saturating_sub(result.bigram_switches),
            self.config.row_switch_penalty,
        );

        result.fitness = 1. / result.fitness * 100.; // lower mean effort → higher fitness; 100 ≈ ideal

        result
    }

    /// Look up precomputed bigram effort. Right-hand pairs were expanded at init by `Keyboard::expand_pairs`.
    #[inline]
    fn lookup(&self, from: u8, to: u8) -> f64 {
        *self.pairs.get(&(from, to)).unwrap()
    }

    /// Returns `config.pinky_multiplier` when `slot` is a pinky key, else `1.0`.
    #[inline]
    fn pinky_mul(&self, slot: u8) -> f64 {
        if is_pinky(slot) {
            self.config.pinky_multiplier
        } else {
            1.0
        }
    }
}

/// Returns `true` when `slot` is on the pinky finger.
/// Left pinky: col 0 (slots 0, 5, 10). Right pinky: col 4 (slots 19, 24, 29).
#[inline]
fn is_pinky(slot: u8) -> bool {
    if slot < 15 {
        slot.is_multiple_of(5)
    } else {
        slot % 5 == 4
    }
}

/// Weighted same-hand row-switch cost. Adjacent-row move = 1, jump-over-row = 2.
#[inline]
fn row_distance(from: u8, to: u8) -> u64 {
    slot_row(from).abs_diff(slot_row(to)).into()
}

/// Hand-imbalance multiplier `max(a, b) / min(a, b)`: `1.0` when balanced or when
/// either side is `0` (an empty hand carries no imbalance to penalize).
fn imbalance_ratio(a: u64, b: u64) -> f64 {
    match (a.max(b), a.min(b)) {
        (_, 0) => 1.0,
        (hi, lo) => hi as f64 / lo as f64,
    }
}

/// Linear corpus-level penalty `1 + k * (count / (presses - 1))`.
/// `k` scales the penalty strength: `0.0` disables it, larger values increase the multiplier linearly.
/// Returns `1.0` when fewer than two presses exist, so no transition can happen.
fn linear_rate_penalty(count: u64, presses: u64, k: f64) -> f64 {
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
        assert_eq!(score.left_rolls, 1);
        assert_eq!(score.right_rolls, 0);
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
        assert_eq!(score.left_rolls, 0);
        assert_eq!(score.right_rolls, 0);
        assert_eq!(score.bigram_switches, 1);
        assert_eq!(score.row_switch_cost, 0);
        assert_close(score.effort, 2.0);
        assert_close(score.fitness, 0.0);
        assert_close(score.left_effort, 1.0);
        assert_close(score.right_effort, 1.0);
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
    fn linear_rate_penalty_scales_with_transition_rate() {
        assert_close(linear_rate_penalty(0, 3, 0.5), 1.0);
        assert_close(linear_rate_penalty(1, 3, 0.5), 1.25);
        assert_close(linear_rate_penalty(2, 3, 0.5), 1.5);
    }

    #[test]
    fn score_corpus_applies_configured_bigram_switch_penalty() {
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
                bigram_switch_penalty: 0.5,
                ..test_config()
            },
        );

        let score = evaluator.score_corpus(&test_keys());

        assert_eq!(score.bigram_switches, 1);
        // base 5.0/4 = 1.25; ×count-ratio 3.0 ×bigram-penalty (1 + 0.5·1/3) = 4.375; 100/4.375.
        assert_close(score.fitness, 22.86);
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
        // Single-hand corpus: both imbalance ratios neutral (1.0).
        // base 3.0/2 = 1.5; ×row-penalty (1 + 0.5·1/1) = 2.25; 100/2.25.
        assert_close(score.fitness, 44.44);
    }

    #[test]
    fn score_corpus_excludes_hand_switches_from_row_switch_rate() {
        // "adc": a→d same-hand row switch (cost 1), d→c hand switch (cost 0).
        // Denominator drops the switch press, so the row-switch rate stays undiluted.
        let evaluator = LayoutEvaluator::new(
            &row_switch_test_keyboard(),
            vec!["adc".to_string()],
            LayoutEvaluatorConfig {
                row_switch_penalty: 0.5,
                ..test_config()
            },
        );

        let score = evaluator.score_corpus(&test_keys());

        assert_eq!(score.bigram_switches, 1);
        assert_eq!(score.row_switch_cost, 1);
        // base 4.0/3; ×count-ratio 2.0 ×row-penalty (1 + 0.5·1/1, over 2 same-hand presses) = 4.0; 100/4.
        assert_close(score.fitness, 25.00);
    }

    #[test]
    fn imbalance_ratio_is_neutral_when_balanced_or_one_sided() {
        assert_close(imbalance_ratio(0, 0), 1.0);
        assert_close(imbalance_ratio(5, 0), 1.0);
        assert_close(imbalance_ratio(0, 5), 1.0);
        assert_close(imbalance_ratio(3, 3), 1.0);
    }

    #[test]
    fn imbalance_ratio_grows_with_imbalance() {
        assert_close(imbalance_ratio(3, 1), 3.0);
        assert_close(imbalance_ratio(1, 3), 3.0);
        assert!(imbalance_ratio(3, 2) < imbalance_ratio(3, 1));
    }

    #[test]
    fn score_word_applies_pinky_multiplier_to_pinky_keys() {
        let evaluator = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                pinky_multiplier: 1.5,
                ..test_config()
            },
        );
        // 'a' → slot 0 (left pinky). Both presses in "aa" are pinky → 1.5× each.
        let score = evaluator.score_word("aa", &test_keys());
        assert_close(score.effort, 3.0); // 2 × 1.0 × 1.5
    }

    #[test]
    fn score_word_no_pinky_multiplier_on_non_pinky_keys() {
        let evaluator = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                pinky_multiplier: 1.5,
                ..test_config()
            },
        );
        // 'b' → slot 1 (not pinky). "bb": lookup(1,1) + lookup(1,1), no multiplier.
        let keys = FxHashMap::from_iter([('b', 1u8)]);
        let score = evaluator.score_word("bb", &keys);
        assert_close(score.effort, 6.0); // 2 × efforts[2] = 2 × 3.0
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
            bigram_switch_penalty: 0.0,
            row_switch_penalty: 0.0,
            pinky_multiplier: 1.0,
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
