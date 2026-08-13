use crate::app::LayoutEvaluatorConfig;
use crate::app::layout_evaluator::corpus::CorpusCounts;
use crate::app::layout_evaluator::math::{imbalance_ratio, row_distance, slot};
#[cfg(test)]
use crate::app::synthesise::CachedSourceStats;
use crate::models::{Keyboard, Keys, ScoreResult};
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
        let left = key < 15;
        let effort = self.lookup(key, key);
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
            // The switch is recorded; its price lives in the `switch_power` factor at corpus level.
            (self.lookup(kb, kb), 1, 0)
        };

        ScoreResult {
            effort,
            fitness: 0.0,
            hand_switches,
            // Row steps only occur same-hand; charge them to that hand.
            left_row_switch_cost: if b_left { row_cost } else { 0 },
            right_row_switch_cost: if !b_left { row_cost } else { 0 },
            left_count: b_left as u64,
            right_count: !b_left as u64,
            // Same-hand bigram lands wholly on one hand; alternating pairs add to neither.
            left_rolls: (same_hand && a_left) as u64,
            right_rolls: (same_hand && !a_left) as u64,
            left_effort: if b_left { effort } else { 0. },
            right_effort: if !b_left { effort } else { 0. },
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

        let mut result = seeds
            .chain(bigrams)
            .fold(ScoreResult::default(), |acc, x| acc + x);

        // Normalization by total presses (CSV: effort, left_effort, right_effort).
        // Dividing by presses makes fitness independent of corpus size.
        // Layouts with different input lengths compare equally.
        let presses = (result.left_count + result.right_count).max(1) as f64;

        let penalty = self.penalty(&result);

        // Fitness (CSV column) = (scale · presses) / (effort · penalty). Higher = better.
        // - effort: raw bigram cost from the pairs table
        // - penalty: dimensionless multiplier built from per-press ratios
        result.fitness = self.config.fitness_scale * presses / (result.effort * penalty);

        result
    }

    /// Uniform penalty multiplier: each factor is a dimensionless ratio raised to
    /// its power knob. `0.0` = off, `1.0` = full, between = softer, above = stricter.
    fn penalty(&self, r: &ScoreResult) -> f64 {
        // - count imbalance → hands_imbalance: left/right press asymmetry
        // - streak imbalance → streak_ratio: unequal run lengths
        // - row imbalance → row_switch_imbalance: unequal row-step burden
        // - hand-switch share → hand_switch_ratio: penalizes hand alternation
        // - row-switch share → row_switch_ratio: penalizes vertical jumps
        // - min streak divisor → mean_streak: rewards long runs on both hands
        imbalance_ratio(r.left_count as f64, r.right_count as f64).powf(self.config.count_power)
            * imbalance_ratio(r.left_streak(), r.right_streak()).powf(self.config.streak_power)
            * imbalance_ratio(
                r.left_row_switch_cost as f64,
                r.right_row_switch_cost as f64,
            )
            .powf(self.config.row_imbalance_power)
            * (1.0 + r.hand_switch_ratio()).powf(self.config.switch_power)
            * (1.0 + r.row_switch_ratio()).powf(self.config.row_power)
            / r.left_streak()
                .min(r.right_streak())
                .max(1.0)
                .powf(self.config.min_streak_power)
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
        assert_eq!(score.row_switch_cost(), 0);
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
        assert_eq!(score.row_switch_cost(), 0);
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
        assert_eq!(score.row_switch_cost(), 0);
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
        assert_eq!(score.row_switch_cost(), 0);
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
        assert_eq!(score.row_switch_cost(), 1);
        assert_close(score.effort, 3.0);
    }

    #[test]
    fn score_word_counts_jump_row_switch_as_double() {
        let evaluator = LayoutEvaluator::new(&row_switch_test_keyboard(), vec![], test_config());

        let score = evaluator.score_word("ae", &test_keys());

        assert_eq!(score.hand_switches, 0);
        assert_eq!(score.row_switch_cost(), 2);
        assert_close(score.effort, 5.0);
    }

    #[test]
    fn score_corpus_applies_switch_power_factor() {
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
                switch_power: 1.0,
                ..test_config()
            },
        );

        let score = evaluator.score_corpus(&test_keys());

        assert_eq!(score.hand_switches, 1);
        // effort 5.0, presses 4; penalty = count 3.0 × streak 1.5 × switch (1 + 1/4) = 5.625;
        // 1e6·4 / (5·5.625) = 142_222.22.
        assert_close(score.fitness, 142_222.22);
    }

    #[test]
    fn score_corpus_applies_row_power_factor() {
        let evaluator = LayoutEvaluator::new(
            &row_switch_test_keyboard(),
            vec!["ad".to_string()],
            LayoutEvaluatorConfig {
                row_power: 1.0,
                ..test_config()
            },
        );

        let score = evaluator.score_corpus(&test_keys());

        assert_eq!(score.row_switch_cost(), 1);
        // Single-hand corpus: imbalance ratios neutral; row factor (1 + 1/1) = 2;
        // min-streak divisor: min(2, 0).max(1) = 1. 1e6·2 / (3·2) = 333_333.33.
        assert_close(score.fitness, 333_333.33);
    }

    #[test]
    fn penalty_softens_count_imbalance_with_subunit_power() {
        let base = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());
        let softer = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                count_power: 0.5,
                ..test_config()
            },
        );

        let score = ScoreResult {
            left_count: 6,
            right_count: 2,
            left_rolls: 2,
            ..Default::default()
        };

        assert!(softer.penalty(&score) < base.penalty(&score));
    }

    #[test]
    fn penalty_softens_streak_imbalance_with_subunit_power() {
        let base = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());
        let softer = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                streak_power: 0.5,
                ..test_config()
            },
        );

        let score = ScoreResult {
            left_count: 6,
            right_count: 6,
            left_rolls: 1,
            right_rolls: 3,
            ..Default::default()
        };

        assert!(softer.penalty(&score) < base.penalty(&score));
    }

    #[test]
    fn penalty_softens_row_switch_imbalance_with_subunit_power() {
        let base = LayoutEvaluator::new(&row_switch_test_keyboard(), vec![], test_config());
        let softer = LayoutEvaluator::new(
            &row_switch_test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                row_imbalance_power: 0.5,
                ..test_config()
            },
        );

        let score = ScoreResult {
            left_count: 6,
            right_count: 6,
            left_rolls: 1,
            right_rolls: 1,
            left_row_switch_cost: 6,
            right_row_switch_cost: 2,
            ..Default::default()
        };

        assert!(softer.penalty(&score) < base.penalty(&score));
    }

    #[test]
    fn penalty_softens_hand_switches_with_subunit_power() {
        let base = LayoutEvaluator::new(
            &test_keyboard(),
            vec!["ac".to_string(), "ca".to_string()],
            LayoutEvaluatorConfig {
                switch_power: 1.0,
                ..test_config()
            },
        );
        let softer = LayoutEvaluator::new(
            &test_keyboard(),
            vec!["ac".to_string(), "ca".to_string()],
            LayoutEvaluatorConfig {
                switch_power: 0.5,
                ..test_config()
            },
        );

        assert!(
            softer.score_corpus(&test_keys()).fitness > base.score_corpus(&test_keys()).fitness
        );
    }

    #[test]
    fn penalty_softens_row_steps_with_subunit_power() {
        let base = LayoutEvaluator::new(
            &row_switch_test_keyboard(),
            vec!["ae".to_string()],
            LayoutEvaluatorConfig {
                row_power: 1.0,
                ..test_config()
            },
        );
        let softer = LayoutEvaluator::new(
            &row_switch_test_keyboard(),
            vec!["ae".to_string()],
            LayoutEvaluatorConfig {
                row_power: 0.5,
                ..test_config()
            },
        );

        let base_score = base.score_corpus(&test_keys());
        let softer_score = softer.score_corpus(&test_keys());

        assert!(softer_score.fitness > base_score.fitness);
    }

    #[test]
    fn penalty_weakens_min_streak_reward_with_subunit_power() {
        let full = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());
        let weaker = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                min_streak_power: 0.5,
                ..test_config()
            },
        );
        let off = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                min_streak_power: 0.0,
                ..test_config()
            },
        );

        let score = ScoreResult {
            left_count: 6,
            right_count: 6,
            left_rolls: 1,
            right_rolls: 1,
            ..Default::default()
        };

        assert!(full.penalty(&score) < weaker.penalty(&score));
        assert!(weaker.penalty(&score) < off.penalty(&score));
    }

    #[test]
    fn imbalance_ratio_is_neutral_when_balanced_or_one_sided() {
        assert_close(imbalance_ratio(0., 0.), 1.0);
        assert_close(imbalance_ratio(5., 0.), 1.0);
        assert_close(imbalance_ratio(0., 5.), 1.0);
        assert_close(imbalance_ratio(3., 3.), 1.0);
    }

    #[test]
    fn imbalance_ratio_grows_with_imbalance() {
        assert_close(imbalance_ratio(3., 1.), 3.0);
        assert_close(imbalance_ratio(1., 3.), 3.0);
        assert!(imbalance_ratio(3., 2.) < imbalance_ratio(3., 1.));
    }

    #[test]
    fn counts_from_cached_stats_match_direct_counts() {
        use crate::app::synthesise::CorpusStatsCounter;

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

    fn test_config() -> LayoutEvaluatorConfig {
        LayoutEvaluatorConfig {
            fitness_scale: 1_000_000.,
            count_power: 1.0,
            streak_power: 1.0,
            row_imbalance_power: 1.0,
            switch_power: 0.0,
            row_power: 0.0,
            min_streak_power: 1.0,
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
