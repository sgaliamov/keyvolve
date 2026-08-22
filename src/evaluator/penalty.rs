//! Corpus-level penalty: the dimensionless multiplier that turns raw effort into fitness.
//!
//! One metric, one number: `max`, the value you would still accept, written in the percent
//! units the CSV prints. It is not a wall — it is the normalizer that makes 20% row switches
//! comparable to 1% effort imbalance:
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
//! # Seven + three + ten metrics
//!
//! | metric               | meaning                          |
//! |----------------------|----------------------------------|
//! | `row_switch_ratio`   | row jumps inside a hand          |
//! | `hand_switch_ratio`  | hand alternation (replaces `mean_streak_power`) |
//! | `efforts_imbalance`  | left/right effort asymmetry      |
//! | `hands_imbalance`    | left/right press-count asymmetry |
//! | `roll_imbalance`     | left/right roll asymmetry        |
//! | `row_switch_imbalance` | left/right row-step asymmetry  |
//! | `streak_imbalance`   | left/right run-length asymmetry  |
//! | `home_row_balance`   | home row left/right balance (absolute value) |
//! | `top_row_ratio`      | top row effort share (default: 25%) |
//! | `home_row_ratio`     | home row effort share (default: 60%) |
//! | `bottom_row_ratio`   | bottom row effort share (default: 15%) |
//! | `left_c1_ratio`      | left pinky effort share (default: 18%) |
//! | `left_c2_ratio`      | left ring effort share (default: 19%) |
//! | `left_c3_ratio`      | left middle effort share (default: 20%) |
//! | `left_c4_ratio`      | left index effort share (default: 22%) |
//! | `left_c5_ratio`      | left thumb effort share (default: 21%) |
//! | `right_c1_ratio`     | right index effort share (mirrored from left) |
//! | `right_c2_ratio`     | right middle effort share (mirrored from left) |
//! | `right_c3_ratio`     | right ring effort share (mirrored from left) |
//! | `right_c4_ratio`     | right pinky effort share (mirrored from left) |
//! | `right_c5_ratio`     | right thumb effort share (mirrored from left) |
//!
//! # Why no `mean_streak` target
//!
//! A run ends exactly when the hand switches or the word ends, so run count *is* switch
//! count. With `P` presses, `S` hand switches, `W` words:
//!
//! ```text
//! mean_streak = P / (S + W)
//! 1 / mean_streak = (S + W) / P = hand_switch_ratio + W / P
//! ```
//!
//! `W/P` is fixed for a corpus, so both formulations are monotone in the same variable `S`.
//! To convert a wish, `mean_streak >= m` means `hand_switch_ratio <= 1/m − W/P`; in practice
//! both columns sit side by side in the CSV, so read the pair off a real run instead.
//!
//! # Corpus invariance
//!
//! Every factor is a per-press ratio, so doubling the corpus leaves the penalty unchanged.
//! Fitness stays comparable across corpus sizes; `W/P` shifts it slightly across corpora
//! with different average word length.

use crate::evaluator::LayoutEvaluatorConfig;
use crate::models::ScoreResult;
use itertools::Itertools;

/// Penalty multiplier for a scored corpus. `1.0` = neutral, higher = worse layout.
/// See the module docs for the algebra behind each factor.
pub fn penalty(config: &LayoutEvaluatorConfig, r: &ScoreResult) -> f64 {
    1.0 + terms(config, r).map(|(_, cost)| cost).sum::<f64>()
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
        (
            "home_row_balance",
            t.home_row_balance,
            r.home_row_balance(),
        ),
        ("top_row_ratio", t.top_row_ratio, r.top_row_ratio() * 100.0),
        (
            "home_row_ratio",
            t.home_row_ratio,
            r.home_row_ratio() * 100.0,
        ),
        (
            "bottom_row_ratio",
            t.bottom_row_ratio,
            r.bottom_row_ratio() * 100.0,
        ),
        ("left_c1_ratio", t.c1_ratio, r.left_c1_ratio() * 100.0),
        ("left_c2_ratio", t.c2_ratio, r.left_c2_ratio() * 100.0),
        ("left_c3_ratio", t.c3_ratio, r.left_c3_ratio() * 100.0),
        ("left_c4_ratio", t.c4_ratio, r.left_c4_ratio() * 100.0),
        ("left_c5_ratio", t.c5_ratio, r.left_c5_ratio() * 100.0),
        ("right_c1_ratio", t.c1_ratio, r.right_c1_ratio() * 100.0),
        ("right_c2_ratio", t.c2_ratio, r.right_c2_ratio() * 100.0),
        ("right_c3_ratio", t.c3_ratio, r.right_c3_ratio() * 100.0),
        ("right_c4_ratio", t.c4_ratio, r.right_c4_ratio() * 100.0),
        ("right_c5_ratio", t.c5_ratio, r.right_c5_ratio() * 100.0),
    ]
    .into_iter()
    .filter_map(move |(name, target, value)| {
        target.map(|t| (name, t.weight * t.deviation(value).powf(config.sharpness)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::{Target, Targets};

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

    /// Breakdown ranks offenders so the loudest metric is obvious while tuning.
    #[test]
    fn breakdown_lists_worst_offender_first() {
        let terms = breakdown(&targets_config(), &skewed());

        assert!(terms.windows(2).all(|w| w[0].1 >= w[1].1));
        assert_eq!(terms.len(), 21);
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
                top_row_ratio: limit(25.0),
                home_row_ratio: limit(60.0),
                home_row_balance: limit(15.0),
                bottom_row_ratio: limit(15.0),
                c1_ratio: limit(7.0),
                c2_ratio: limit(11.5),
                c3_ratio: limit(13.0),
                c4_ratio: limit(10.5),
                c5_ratio: limit(8.0),
            },
            ..Default::default()
        }
    }
}
