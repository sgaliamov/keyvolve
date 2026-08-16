//! Corpus-level penalty: the dimensionless multiplier that turns raw effort into fitness.
//!
//! # Shape
//!
//! Every knob is one scheme — `factor ^ power`, where `factor` is dimensionless and
//! `>= 1.0` means "worse". `0.0` = off, `1.0` = full, between = softer, above = stricter.
//! With every power at `0.0` the penalty is exactly `1.0`, so knobs never leak a bias.
//!
//! Two knobs per metric family, never more:
//!
//! | family         | level (how much)                | balance (how evenly split) |
//! |----------------|---------------------------------|----------------------------|
//! | effort         | implicit: fitness divides by it | `effort_power`              |
//! | rolls          | -                               | `roll_imbalance_power`      |
//! | same-hand runs | `mean_streak_power`             | `streak_power`             |
//! | row jumps      | `row_power`                     | `row_imbalance_power`      |
//!
//! # Why there is no `switch_power`
//!
//! A run ends exactly when the hand switches or the word ends, so run count *is* switch
//! count. With `P` presses, `S` hand switches, `W` words and `rolls` same-hand bigrams:
//!
//! ```text
//! rolls + S = P − W          a word of n chars yields n − 1 bigrams
//! runs      = P − rolls      definition of a run (see crate::math::streak)
//!           = S + W          substitute
//!
//! mean_streak = P / runs = P / (S + W)
//! ```
//!
//! Dividing by it is therefore already a hand-switch penalty:
//!
//! ```text
//! 1 / mean_streak^p = ((S + W) / P)^p = (hand_switch_ratio + W/P)^p
//! ```
//!
//! `W/P` is fixed for a corpus, so the factor is strictly monotone in `S`. A separate
//! `(1 + hand_switch_ratio)^q` factor would penalize the same trait twice with two knobs.
//! The divisor form is kept because it discriminates harder: `mean_streak` spans
//! `[1, P/W]` (~5x on typical corpora) against `[1, 2]` for `1 + hand_switch_ratio`.
//! Proven by `mean_streak_equals_presses_over_runs` and
//! `mean_streak_falls_as_hand_switches_rise`.
//!
//! # Direction
//!
//! This scheme rewards *long same-hand runs* — it pushes against hand alternation.
//! Invert the divisor into a multiplier to flip that preference.
//!
//! # Corpus invariance
//!
//! Every factor is a per-press ratio, so doubling the corpus leaves the penalty unchanged.
//! Fitness stays comparable across corpus sizes; `W/P` shifts it slightly across corpora
//! with different average word length.

use crate::app::LayoutEvaluatorConfig;
use crate::math::imbalance_ratio;
use crate::models::ScoreResult;

/// Penalty multiplier for a scored corpus. `1.0` = neutral, higher = worse layout.
/// See the module docs for the algebra behind each factor.
pub fn penalty(config: &LayoutEvaluatorConfig, r: &ScoreResult) -> f64 {
    // Level: row jumps cost, long same-hand runs pay back.
    // `max(1.0)` keeps an empty corpus neutral, where `mean_streak` is `0.0`.
    let row_jumps = (1.0 + r.row_switch_ratio()).powf(config.row_power);

    let rows = imbalance_ratio(
        r.left_row_switch_cost as f64,
        r.right_row_switch_cost as f64,
    )
    .powf(config.row_imbalance_power);

    let runs = r.mean_streak().powf(config.mean_streak_power).max(1.0);

    // Balance: both hands should carry comparable effort load.
    let efforts = imbalance_ratio(r.left_effort, r.right_effort).powf(config.effort_power);

    let counts = imbalance_ratio(r.left_count as f64, r.right_count as f64).powf(config.effort_power);

    let rolls = imbalance_ratio(r.left_rolls as f64, r.right_rolls as f64)
        .powf(config.roll_imbalance_power);

    let streaks = imbalance_ratio(r.left_streak(), r.right_streak()).powf(config.streak_power);

    efforts * counts * streaks * rolls * rows * row_jumps / runs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One 8-press word under four hand patterns. Rolls and switches trade off, so
    /// `mean_streak` drops monotonically as alternation rises — it *is* a switch penalty.
    #[test]
    fn mean_streak_falls_as_hand_switches_rise() {
        let sample = |left, right, left_rolls, right_rolls| ScoreResult {
            left_count: left,
            right_count: right,
            left_rolls,
            right_rolls,
            ..Default::default()
        };

        assert_eq!(sample(8, 0, 7, 0).mean_streak(), 8.0); // LLLLLLLL
        assert_eq!(sample(4, 4, 3, 3).mean_streak(), 4.0); // LLLLRRRR
        assert_eq!(sample(4, 4, 2, 2).mean_streak(), 2.0); // LLRRLLRR
        assert_eq!(sample(4, 4, 0, 0).mean_streak(), 1.0); // LRLRLRLR
    }

    /// Every power at `0.0` must bottom out at a neutral multiplier: a knob turned off
    /// may not leak a bias into fitness.
    #[test]
    fn zero_powers_leave_penalty_neutral() {
        let config = LayoutEvaluatorConfig {
            effort_power: 0.0,
            streak_power: 0.0,
            roll_imbalance_power: 0.0,
            mean_streak_power: 0.0,
            row_imbalance_power: 0.0,
            row_power: 0.0,
            ..Default::default()
        };

        assert_eq!(penalty(&config, &skewed()), 1.0);
    }

    /// Each knob at full strength must move the penalty the way its docs promise:
    /// balance and row-jump knobs punish, the streak-level knob rewards.
    #[test]
    fn each_power_moves_penalty_in_its_documented_direction() {
        let off = LayoutEvaluatorConfig {
            effort_power: 0.0,
            streak_power: 0.0,
            roll_imbalance_power: 0.0,
            mean_streak_power: 0.0,
            row_imbalance_power: 0.0,
            row_power: 0.0,
            ..Default::default()
        };
        let neutral = penalty(&off, &skewed());
        let with = |f: fn(&mut LayoutEvaluatorConfig)| {
            let mut config = off;
            f(&mut config);
            penalty(&config, &skewed())
        };

        assert!(with(|c| c.effort_power = 1.0) > neutral);
        assert!(with(|c| c.streak_power = 1.0) > neutral);
        assert!(with(|c| c.roll_imbalance_power = 1.0) > neutral);
        assert!(with(|c| c.row_imbalance_power = 1.0) > neutral);
        assert!(with(|c| c.row_power = 1.0) > neutral);
        assert!(with(|c| c.mean_streak_power = 1.0) < neutral);
    }

    /// Raising a power past `1.0` must sharpen an existing penalty, lowering it must soften.
    #[test]
    fn subunit_power_softens_and_superunit_sharpens() {
        let scaled = |power: f64| {
            penalty(
                &LayoutEvaluatorConfig {
                    effort_power: power,
                    streak_power: 0.0,
                    roll_imbalance_power: 0.0,
                    mean_streak_power: 0.0,
                    row_imbalance_power: 0.0,
                    row_power: 0.0,
                    ..Default::default()
                },
                &skewed(),
            )
        };

        assert!(scaled(0.5) < scaled(1.0));
        assert!(scaled(1.0) < scaled(2.0));
    }

    /// A layout skewed on every axis, so no factor sits at its neutral value.
    fn skewed() -> ScoreResult {
        ScoreResult {
            left_count: 9,
            right_count: 3,
            left_rolls: 6,
            right_rolls: 1,
            left_row_switch_cost: 4,
            right_row_switch_cost: 1,
            left_effort: 30.0,
            right_effort: 10.0,
            ..Default::default()
        }
    }
}
