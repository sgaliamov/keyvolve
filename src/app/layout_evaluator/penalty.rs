//! Corpus-level penalty: the dimensionless multiplier that turns raw effort into fitness.
//!
//! Two modes, picked by whether [`Targets`] holds anything.
//!
//! # Targets mode
//!
//! One number per metric: `max`, the value you would still accept, written in the percent
//! units the CSV prints. It is not a wall — it is the normalizer that makes 20% row
//! switches comparable to 1% effort imbalance:
//!
//! ```text
//! deviation = |value| / max          0 = perfect, 1 = at the limit, >1 = over it
//! penalty   = 1 + Σ weight · deviation^sharpness
//! ```
//!
//! The `1` is the neutral element of a *multiplier*, not an added cost: with every metric
//! on target each term is `0`, the penalty is exactly `1.0`, and fitness falls back to the
//! effort-only ideal `scale / effort`. It also guards the divide, since a penalty near `0`
//! would send fitness to infinity and drown out effort.
//!
//! Lower is always better for every metric here, so there is no lower bound. `weight`
//! defaults to `1`, meaning each metric costs the same at its own limit; raise it only for
//! a metric that should give way last. `sharpness` shapes the whole trade-off: at `4`, half
//! the limit costs `weight / 16` and double the limit costs `16 · weight`.
//!
//! ## Where the power knobs went
//!
//! | power knob           | target replacement                       |
//! |----------------------|------------------------------------------|
//! | `row_power`          | `row_switch_ratio`                       |
//! | `switch_power`       | `row_switch_ratio` + `hand_switch_ratio` |
//! | `mean_streak_power`  | `hand_switch_ratio` (same trait inverted)|
//! | `balance_power`      | `efforts_imbalance` + `hands_imbalance`  |
//! | `roll_imbalance_power` | `roll_imbalance`                       |
//! | `row_imbalance_power`  | `row_switch_imbalance`                 |
//! | `streak_power`         | `streak_imbalance`                     |
//!
//! # Powers mode (legacy)
//!
//! Every knob is one scheme — `factor ^ power`, where `factor` is dimensionless and
//! `>= 1.0` means "worse". `0.0` = off, `1.0` = full, between = softer, above = stricter.
//! With every power at `0.0` the penalty is exactly `1.0`, so knobs never leak a bias.
//!
//! One or two knobs per metric family (level and optional balance):
//!
//! | family         | level (how much)                | balance (how evenly split) |
//! |----------------|---------------------------------|----------------------------|
//! | effort         | implicit: fitness divides by it | `balance_power`              |
//! | rolls          | -                               | `roll_imbalance_power`      |
//! | same-hand runs | `mean_streak_power`             | `streak_power`             |
//! | row jumps      | `row_power`                     | `row_imbalance_power`      |
//! | switches+rows  | `switch_power`                  | -                          |
//!
//! # Why there is no `mean_streak` target, and no `switch_power` twin
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
//! The divisor form is kept in powers mode because it discriminates harder: `mean_streak`
//! spans `[1, P/W]` (~5x on typical corpora) against `[1, 2]` for `1 + hand_switch_ratio`.
//! Proven by `mean_streak_equals_presses_over_runs` and
//! `mean_streak_falls_as_hand_switches_rise`.
//!
//! Targets mode keeps only the "lower is better" half of that pair: a `hand_switch_ratio`
//! limit *is* the `mean_streak` knob. To convert a wish, `mean_streak >= m` means
//! `hand_switch_ratio <= 1/m − W/P`; in practice both columns sit side by side in the CSV,
//! so read the pair off a real run instead.
//!
//! # Direction
//!
//! Powers mode rewards *long same-hand runs* — it pushes against hand alternation.
//! Invert the divisor into a multiplier to flip that preference. Targets mode expresses the
//! same preference as a `hand_switch_ratio` limit.
//!
//! # Corpus invariance
//!
//! Every factor is a per-press ratio, so doubling the corpus leaves the penalty unchanged.
//! Fitness stays comparable across corpus sizes; `W/P` shifts it slightly across corpora
//! with different average word length.

use crate::app::LayoutEvaluatorConfig;
use crate::math::imbalance_ratio;
use crate::models::ScoreResult;
use itertools::Itertools;

/// Penalty multiplier for a scored corpus. `1.0` = neutral, higher = worse layout.
/// Targets configured → normalized-deviation sum; otherwise the legacy power knobs.
/// See the module docs for the algebra behind each factor.
pub fn penalty(config: &LayoutEvaluatorConfig, r: &ScoreResult) -> f64 {
    match config.targets.is_empty() {
        true => powers(config, r),
        false => 1.0 + terms(config, r).map(|(_, cost)| cost).sum::<f64>(),
    }
}

/// Per-metric penalty contribution, worst first — the tuning aid that says which limit is
/// losing and therefore which weight (if any) is worth raising.
pub fn breakdown(config: &LayoutEvaluatorConfig, r: &ScoreResult) -> Vec<(&'static str, f64)> {
    terms(config, r)
        .sorted_by(|a, b| b.1.total_cmp(&a.1))
        .collect()
}

/// Metric name paired with its penalty contribution; metrics without a target drop out.
/// Values are normalized to percent: `*_ratio` metrics are fractions and scale by 100,
/// `*_imbalance` metrics already come as percent. Sign is dropped by `Target::deviation`.
fn terms<'a>(
    config: &'a LayoutEvaluatorConfig,
    r: &ScoreResult,
) -> impl Iterator<Item = (&'static str, f64)> + 'a {
    let t = config.targets;

    [
        (
            "row_switch_ratio",
            t.row_switch_ratio,
            r.row_switch_ratio() * 100.0,
        ),
        (
            "hand_switch_ratio",
            t.hand_switch_ratio,
            r.hand_switch_ratio() * 100.0,
        ),
        (
            "efforts_imbalance",
            t.efforts_imbalance,
            r.efforts_imbalance(),
        ),
        ("hands_imbalance", t.hands_imbalance, r.hands_imbalance()),
        ("roll_imbalance", t.roll_imbalance, r.roll_imbalance()),
        (
            "row_switch_imbalance",
            t.row_switch_imbalance,
            r.row_switch_imbalance(),
        ),
        ("streak_imbalance", t.streak_imbalance, r.streak_imbalance()),
    ]
    .into_iter()
    .filter_map(move |(name, target, value)| {
        target.map(|t| (name, t.weight * t.deviation(value).powf(config.sharpness)))
    })
}

/// Legacy penalty: one `factor ^ power` per knob, multiplied together.
fn powers(config: &LayoutEvaluatorConfig, r: &ScoreResult) -> f64 {
    // Level: row jumps cost, long same-hand runs pay back.
    // `max(1.0)` keeps an empty corpus neutral, where `mean_streak` is `0.0`.
    let row_jumps = (1.0 + r.row_switch_ratio()).powf(config.row_power);
    // Hand switches are weighted by 0.5 so this term stays comparable to row-switch ratio.
    let switch_factor =
        (1.0 + r.hand_switch_ratio() / 2.0 + r.row_switch_ratio()).powf(config.switch_power);

    let rows = imbalance_ratio(
        r.left_row_switch_cost as f64,
        r.right_row_switch_cost as f64,
    )
    .powf(config.row_imbalance_power);

    let runs = r.mean_streak().powf(config.mean_streak_power).max(1.0);

    // Balance: both hands should carry comparable effort load.
    let efforts = imbalance_ratio(r.left_effort, r.right_effort).powf(config.balance_power);

    let counts =
        imbalance_ratio(r.left_count as f64, r.right_count as f64).powf(config.balance_power);

    let rolls = imbalance_ratio(r.left_rolls as f64, r.right_rolls as f64)
        .powf(config.roll_imbalance_power);

    let streaks = imbalance_ratio(r.left_streak(), r.right_streak()).powf(config.streak_power);

    efforts * counts * streaks * rolls * rows * row_jumps * switch_factor / runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Target, Targets};

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
            balance_power: 0.0,
            streak_power: 0.0,
            roll_imbalance_power: 0.0,
            mean_streak_power: 0.0,
            row_imbalance_power: 0.0,
            row_power: 0.0,
            switch_power: 0.0,
            ..Default::default()
        };

        assert_eq!(penalty(&config, &skewed()), 1.0);
    }

    /// Each knob at full strength must move the penalty the way its docs promise:
    /// balance and row-jump knobs punish, the streak-level knob rewards.
    #[test]
    fn each_power_moves_penalty_in_its_documented_direction() {
        let off = LayoutEvaluatorConfig {
            balance_power: 0.0,
            streak_power: 0.0,
            roll_imbalance_power: 0.0,
            mean_streak_power: 0.0,
            row_imbalance_power: 0.0,
            row_power: 0.0,
            switch_power: 0.0,
            ..Default::default()
        };
        let neutral = penalty(&off, &skewed());
        let with = |f: fn(&mut LayoutEvaluatorConfig)| {
            let mut config = off;
            f(&mut config);
            penalty(&config, &skewed())
        };

        assert!(with(|c| c.balance_power = 1.0) > neutral);
        assert!(with(|c| c.streak_power = 1.0) > neutral);
        assert!(with(|c| c.roll_imbalance_power = 1.0) > neutral);
        assert!(with(|c| c.row_imbalance_power = 1.0) > neutral);
        assert!(with(|c| c.row_power = 1.0) > neutral);
        assert!(with(|c| c.switch_power = 1.0) > neutral);
        assert!(with(|c| c.mean_streak_power = 1.0) < neutral);
    }

    /// Raising a power past `1.0` must sharpen an existing penalty, lowering it must soften.
    #[test]
    fn subunit_power_softens_and_superunit_sharpens() {
        let scaled = |power: f64| {
            penalty(
                &LayoutEvaluatorConfig {
                    balance_power: power,
                    streak_power: 0.0,
                    roll_imbalance_power: 0.0,
                    mean_streak_power: 0.0,
                    row_imbalance_power: 0.0,
                    row_power: 0.0,
                    switch_power: 0.0,
                    ..Default::default()
                },
                &skewed(),
            )
        };

        assert!(scaled(0.5) < scaled(1.0));
        assert!(scaled(1.0) < scaled(2.0));
    }

    /// Combined switch+row knob should rise with either hand switching or row movement.
    /// Hand-switch ratio contributes at half weight.
    #[test]
    fn switch_power_penalizes_combined_switch_and_row_ratios() {
        let config = LayoutEvaluatorConfig {
            balance_power: 0.0,
            streak_power: 0.0,
            roll_imbalance_power: 0.0,
            mean_streak_power: 0.0,
            row_imbalance_power: 0.0,
            row_power: 0.0,
            switch_power: 1.0,
            ..Default::default()
        };
        let neutral = ScoreResult {
            left_count: 10,
            right_count: 10,
            ..Default::default()
        };
        let combined = ScoreResult {
            left_count: 10,
            right_count: 10,
            hand_switches: 5,        // 5 / 20 = 0.25
            left_row_switch_cost: 3, // 5 / 20 = 0.25 total row ratio
            right_row_switch_cost: 2,
            ..Default::default()
        };

        assert_eq!(penalty(&config, &neutral), 1.0);
        // 1 + (0.25 / 2) + 0.25 = 1.375
        assert_eq!(penalty(&config, &combined), 1.375);
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

    // Targets mode.

    /// Metrics on target must leave the multiplier neutral: the leading `1` is the
    /// identity of a product, not a surcharge.
    #[test]
    fn perfect_metrics_leave_penalty_neutral() {
        let balanced = ScoreResult {
            left_count: 10,
            right_count: 10,
            left_rolls: 5,
            right_rolls: 5,
            left_effort: 20.0,
            right_effort: 20.0,
            ..Default::default()
        };

        assert_eq!(penalty(&targets_config(), &balanced), 1.0);
    }

    /// A metric sitting exactly on its limit costs exactly its weight — that is what
    /// makes `max` a normalizer and `weight` a plain statement of priority.
    #[test]
    fn metric_at_its_limit_costs_its_weight() {
        let config = LayoutEvaluatorConfig {
            sharpness: 4.0,
            targets: Targets {
                hand_switch_ratio: Some(Target {
                    max: 35.0,
                    weight: 3.0,
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        // 7 switches over 20 presses = 35%, exactly the limit.
        let at_limit = ScoreResult {
            left_count: 10,
            right_count: 10,
            hand_switches: 7,
            ..Default::default()
        };

        assert_eq!(penalty(&config, &at_limit), 4.0);
    }

    /// Under the limit the cost is negligible, over it the cost explodes — the point
    /// of raising deviation to `sharpness`.
    #[test]
    fn cost_stays_small_below_the_limit_and_grows_above_it() {
        let config = targets_config();
        let with_switches = |switches| {
            penalty(
                &config,
                &ScoreResult {
                    left_count: 10,
                    right_count: 10,
                    hand_switches: switches,
                    left_rolls: 5,
                    right_rolls: 5,
                    left_effort: 20.0,
                    right_effort: 20.0,
                    ..Default::default()
                },
            )
        };

        // Half the limit (17.5% of 35%) with sharpness 4 → weight / 16.
        assert!((with_switches(3) - 1.0).abs() < 0.07);
        // Double the limit → 16 · weight.
        assert!(with_switches(14) > 16.0);
    }

    /// Sharpness is the shape of the trade-off: softer under the limit, harsher over it.
    #[test]
    fn sharpness_softens_below_and_sharpens_above_the_limit() {
        let scaled = |sharpness: f64, switches| {
            penalty(
                &LayoutEvaluatorConfig {
                    sharpness,
                    ..targets_config()
                },
                &ScoreResult {
                    left_count: 10,
                    right_count: 10,
                    hand_switches: switches,
                    left_rolls: 5,
                    right_rolls: 5,
                    left_effort: 20.0,
                    right_effort: 20.0,
                    ..Default::default()
                },
            )
        };

        assert!(scaled(4.0, 3) < scaled(2.0, 3)); // below the limit
        assert!(scaled(4.0, 14) > scaled(2.0, 14)); // above it
    }

    /// Metrics without a target contribute nothing, whatever they read.
    #[test]
    fn metrics_without_a_target_are_ignored() {
        let config = LayoutEvaluatorConfig {
            targets: Targets {
                row_switch_ratio: Some(Target {
                    max: 20.0,
                    weight: 1.0,
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        // `skewed` has no row switches relative to nothing else configured.
        let only_row = penalty(&config, &skewed());
        let terms = breakdown(&config, &skewed());

        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].0, "row_switch_ratio");
        assert_eq!(only_row, 1.0 + terms[0].1);
    }

    /// A filled `targets` block owns scoring; the power knobs stay silent.
    #[test]
    fn targets_override_the_power_knobs() {
        let both = LayoutEvaluatorConfig {
            balance_power: 1.0,
            mean_streak_power: 1.0,
            row_power: 1.0,
            ..targets_config()
        };

        assert_eq!(
            penalty(&both, &skewed()),
            penalty(&targets_config(), &skewed())
        );
    }

    /// Breakdown ranks offenders so the loudest metric is obvious while tuning.
    #[test]
    fn breakdown_lists_worst_offender_first() {
        let terms = breakdown(&targets_config(), &skewed());

        assert!(terms.windows(2).all(|w| w[0].1 >= w[1].1));
        assert_eq!(terms.len(), 7);
    }

    /// Every metric limited, all weights at 1 — the recommended starting point.
    fn targets_config() -> LayoutEvaluatorConfig {
        let limit = |max| Some(Target { max, weight: 1.0 });

        LayoutEvaluatorConfig {
            sharpness: 4.0,
            targets: Targets {
                row_switch_ratio: limit(20.0),
                hand_switch_ratio: limit(35.0),
                efforts_imbalance: limit(1.0),
                hands_imbalance: limit(1.0),
                roll_imbalance: limit(1.0),
                row_switch_imbalance: limit(1.0),
                streak_imbalance: limit(1.0),
            },
            ..Default::default()
        }
    }
}
