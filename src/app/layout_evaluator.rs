use crate::app::synthesise::CachedSourceStats;
use crate::models::{Keyboard, Keys, ScoreResult, slot_row};
#[cfg(test)]
use itertools::Itertools;
use rustc_hash::FxHashMap;
use serde::Deserialize;

/// Static scoring knobs for layout evaluation.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutEvaluatorConfig {
    /// Extra effort charged per hand switch, in pairs-table effort units; `0.0` disables.
    #[serde(default)]
    pub switch_cost: f64,

    /// Extra effort charged per same-hand row step (adjacent = 1, jump = 2); `0.0` disables.
    #[serde(default)]
    pub row_cost: f64,

    /// Multiplier applied to the inverted fitness; sets the "ideal" score magnitude.
    #[serde(default = "default_fitness_scale")]
    pub fitness_scale: f64,

    /// Exponent applied to hand-count imbalance; `< 1.0` softens balance pressure.
    #[serde(default = "default_balance_penalty_power")]
    pub balance_penalty_power: f64,

    /// Exponent applied to left/right streak imbalance.
    #[serde(default = "default_streak_penalty_power")]
    pub streak_penalty_power: f64,

    /// Exponent applied to left/right row-switch imbalance.
    #[serde(default = "default_row_switch_penalty_power")]
    pub row_switch_penalty_power: f64,

    /// Strength applied to overall row-switch ratio; `0.0` disables, `1.0` uses full excess.
    #[serde(default = "default_row_switch_ratio_penalty_power")]
    pub row_switch_ratio_penalty_power: f64,

    /// Exponent scale applied to the shorter-hand streak divisor; `< 1.0` softens.
    #[serde(default = "default_min_streak_penalty_power")]
    pub min_streak_penalty_power: f64,
}

/// Serde default for [`LayoutEvaluatorConfig::fitness_scale`].
fn default_fitness_scale() -> f64 {
    1_000_000.
}

/// Serde default for [`LayoutEvaluatorConfig::balance_penalty_power`].
fn default_balance_penalty_power() -> f64 {
    1.0
}

/// Serde default for [`LayoutEvaluatorConfig::streak_penalty_power`].
fn default_streak_penalty_power() -> f64 {
    1.0
}

/// Serde default for [`LayoutEvaluatorConfig::row_switch_penalty_power`].
fn default_row_switch_penalty_power() -> f64 {
    1.0
}

/// Serde default for [`LayoutEvaluatorConfig::row_switch_ratio_penalty_power`].
fn default_row_switch_ratio_penalty_power() -> f64 {
    0.0
}

/// Serde default for [`LayoutEvaluatorConfig::min_streak_penalty_power`].
fn default_min_streak_penalty_power() -> f64 {
    1.0
}

impl Default for LayoutEvaluatorConfig {
    fn default() -> Self {
        Self {
            switch_cost: 0.0,
            row_cost: 0.0,
            fitness_scale: default_fitness_scale(),
            balance_penalty_power: default_balance_penalty_power(),
            streak_penalty_power: default_streak_penalty_power(),
            row_switch_penalty_power: default_row_switch_penalty_power(),
            row_switch_ratio_penalty_power: default_row_switch_ratio_penalty_power(),
            min_streak_penalty_power: default_min_streak_penalty_power(),
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

/// Rebuild approximate counts from cached normalized stats. Fitness is
/// scale-invariant, so scaling frequencies by corpus size preserves ranking.
impl From<&CachedSourceStats> for CorpusCounts {
    fn from(cached: &CachedSourceStats) -> Self {
        let words = cached.word_count as f64;
        // Bigrams per word ≈ average word length − 1.
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
            // The switch is recorded; its price lives in `switch_cost` at corpus level.
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

    /// Score the corpus: physical effort plus flat per-event surcharges, balance-scaled.
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

        // Flat surcharges in effort units: each hand switch and each same-hand row step
        // (jump counts double) costs like extra key presses. Comparable to the pairs table.
        // - hand_switches (CSV: hand_switch_ratio) → penalizes hand alternation
        // - row_switch_cost (CSV: row_switch_ratio) → penalizes vertical jumps, especially when reaching
        let surcharge = self.config.switch_cost * result.hand_switches as f64
            + self.config.row_cost * result.row_switch_cost() as f64;

        // Normalization by total presses (CSV: effort, left_effort, right_effort).
        // Dividing by presses makes fitness independent of corpus size.
        // Layouts with different input lengths compare equally.
        let presses = (result.left_count + result.right_count).max(1) as f64;

        let penalty = self.balance_penalty(&result);

        // Fitness (CSV column) = (scale · presses) / ((effort + surcharge) · penalty)
        // Higher = better. Inverted denominator structure:
        // - effort: mean bigram/switch cost (from pairs table)
        // - surcharge: explicit hand_switch + row penalties (same units as effort)
        // - penalty: dimensionless multiplier on effort, driving balance/streak metrics
        result.fitness =
            self.config.fitness_scale * presses / ((result.effort + surcharge) * penalty);

        result
    }

    /// Balance penalty multiplier for the corpus score.
    fn balance_penalty(&self, r: &ScoreResult) -> f64 {
        // Default 1.0 on each knob preserves existing behavior.
        imbalance_ratio(r.left_count as f64, r.right_count as f64)
            .powf(self.config.balance_penalty_power)
            * imbalance_ratio(r.left_streak(), r.right_streak())
                .powf(self.config.streak_penalty_power)
            * imbalance_ratio(
                r.left_row_switch_cost as f64,
                r.right_row_switch_cost as f64,
            )
            .powf(self.config.row_switch_penalty_power)
            * row_switch_ratio_penalty(r, self.config.row_switch_ratio_penalty_power)
            / r.left_streak()
                .min(r.right_streak())
                .max(1.0)
                .powf(1.0 / self.config.min_streak_penalty_power.max(f64::MIN_POSITIVE))
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

/// Slot for `c`; panic names the offending char so corpus/layout mismatches are debuggable.
#[inline]
fn slot(keys: &Keys, c: char) -> u8 {
    *keys
        .get(&c)
        .unwrap_or_else(|| panic!("char {c:?} (U+{:04X}) not in layout: {keys:?}", c as u32))
}

/// Weighted same-hand row-switch cost. Adjacent-row move = 1, jump-over-row = 2.
#[inline]
fn row_distance(from: u8, to: u8) -> u64 {
    slot_row(from).abs_diff(slot_row(to)).into()
}

/// Hand-imbalance multiplier `max(a, b) / min(a, b)`: `1.0` when balanced or when
/// either side is `0` (an empty hand carries no imbalance to penalize).
fn imbalance_ratio(a: f64, b: f64) -> f64 {
    match (a.max(b), a.min(b)) {
        (_, 0.0) => 1.0,
        (hi, lo) => hi / lo,
    }
}

/// Row-switch ratio: weighted row steps per same-hand transition.
#[inline]
fn row_switch_ratio(r: &ScoreResult) -> f64 {
    let denom = (r.left_count.saturating_sub(1) + r.right_count.saturating_sub(1)).max(1) as f64;
    r.row_switch_cost() as f64 / denom
}

/// Row-switch ratio penalty: only the excess above 1.0 counts.
#[inline]
fn row_switch_ratio_penalty(r: &ScoreResult, strength: f64) -> f64 {
    1.0 + (row_switch_ratio(r) - 1.0).max(0.0) * strength.max(0.0)
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
    fn score_corpus_applies_configured_switch_cost() {
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
                switch_cost: 3.0,
                ..test_config()
            },
        );

        let score = evaluator.score_corpus(&test_keys());

        assert_eq!(score.hand_switches, 1);
        // (effort 5.0 + 3.0·1 switch)/4 = 2.0; ×count-ratio 3.0 ×streak-ratio 1.5 = 9.0; 1e6/9.
        assert_close(score.fitness, 111_111.11);
    }

    #[test]
    fn score_corpus_applies_configured_row_cost() {
        let evaluator = LayoutEvaluator::new(
            &row_switch_test_keyboard(),
            vec!["ad".to_string()],
            LayoutEvaluatorConfig {
                row_cost: 1.0,
                ..test_config()
            },
        );

        let score = evaluator.score_corpus(&test_keys());

        assert_eq!(score.row_switch_cost(), 1);
        // Single-hand corpus: both imbalance ratios neutral (1.0).
        // (effort 3.0 + 1.0·1 row step)/2 = 2.0; 1e6/2.
        assert_close(score.fitness, 500_000.0);
    }

    #[test]
    fn balance_penalty_softens_count_imbalance_with_subunit_power() {
        let base = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());
        let softer = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                balance_penalty_power: 0.5,
                ..test_config()
            },
        );

        let score = ScoreResult {
            left_count: 6,
            right_count: 2,
            left_rolls: 2,
            ..Default::default()
        };

        assert!(softer.balance_penalty(&score) < base.balance_penalty(&score));
    }

    #[test]
    fn balance_penalty_softens_streak_imbalance_with_subunit_power() {
        let base = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());
        let softer = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                streak_penalty_power: 0.5,
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

        assert!(softer.balance_penalty(&score) < base.balance_penalty(&score));
    }

    #[test]
    fn balance_penalty_softens_row_switch_imbalance_with_subunit_power() {
        let base = LayoutEvaluator::new(&row_switch_test_keyboard(), vec![], test_config());
        let softer = LayoutEvaluator::new(
            &row_switch_test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                row_switch_penalty_power: 0.5,
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

        assert!(softer.balance_penalty(&score) < base.balance_penalty(&score));
    }

    #[test]
    fn balance_penalty_softens_row_switch_ratio_with_subunit_power() {
        let base = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                row_switch_ratio_penalty_power: 1.0,
                ..test_config()
            },
        );
        let softer = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                row_switch_ratio_penalty_power: 0.5,
                ..test_config()
            },
        );

        let score = ScoreResult {
            left_count: 6,
            right_count: 6,
            left_rolls: 1,
            right_rolls: 1,
            left_row_switch_cost: 10,
            right_row_switch_cost: 10,
            ..Default::default()
        };

        assert!(softer.balance_penalty(&score) < base.balance_penalty(&score));
    }

    #[test]
    fn balance_penalty_softens_min_streak_divisor_with_subunit_power() {
        let base = LayoutEvaluator::new(&test_keyboard(), vec![], test_config());
        let softer = LayoutEvaluator::new(
            &test_keyboard(),
            vec![],
            LayoutEvaluatorConfig {
                min_streak_penalty_power: 0.5,
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

        assert!(softer.balance_penalty(&score) < base.balance_penalty(&score));
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
            switch_cost: 0.0,
            row_cost: 0.0,
            fitness_scale: 1_000_000.,
            balance_penalty_power: 1.0,
            streak_penalty_power: 1.0,
            row_switch_penalty_power: 1.0,
            row_switch_ratio_penalty_power: 0.0,
            min_streak_penalty_power: 1.0,
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
