//! Corpus-level penalty: the dimensionless multiplier that turns raw effort into fitness.
//!
//! Each metric configures a `value`, `weight`, and `type`. Limits use `type: max`;
//! distributions use `type: target` with an optional `tolerance` (default 5pp):
//!
//! ```text
//! max deviation    = |metric| / value
//! target deviation = |metric - value| / tolerance
//! penalty   = 1 + Σ weight · deviation^sharpness
//! ```
//!
//! Both kinds share the same algebra: deviation `1.0` at the accepted edge (the limit,
//! or `tolerance` points from the target) costs exactly `weight`; inside the edge the
//! cost fades to nothing, outside it walls up with `sharpness`.
//!
//! The `1` is the neutral element of a *multiplier*, not an added cost: with every metric
//! on target each term is `0`, the penalty is exactly `1.0`, and fitness falls back to the
//! effort-only ideal `scale / effort`. It also guards the divide, since a penalty near `0`
//! would send fitness to infinity and drown out effort.
//!
//! Lower is better for `max`; closer is better for `target`. `weight` defaults to `1`.
//! `sharpness` shapes the whole trade-off: for `max` at `4`, half the limit costs
//! `weight / 16` and double the limit costs `16 · weight`.
//!
//! # Tuning with the breakdown table
//!
//! [`breakdown`] emits one [`TermReport`] per configured metric; [`table`] renders them.
//! `share` says who pays the penalty now; `pressure` (marginal cost per percentage point)
//! says who wins the next point of movement. A metric resting off its goal has lower
//! pressure than its opponents — raise its weight or tighten its tolerance. Two off-goal
//! metrics with matching pressures signal a physical conflict no weight can fix.
//!
//! # Eight caps + thirteen distribution targets
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
//! | `top_row_ratio`      | top row effort share target (default: 25%) |
//! | `home_row_ratio`     | home row effort share target (default: 60%) |
//! | `bottom_row_ratio`   | bottom row effort share target (default: 15%) |
//! | `left_c1_ratio`      | left pinky effort share target (default: 7%) |
//! | `left_c2_ratio`      | left ring effort share target (default: 11.5%) |
//! | `left_c3_ratio`      | left middle effort share target (default: 13%) |
//! | `left_c4_ratio`      | left index effort share target (default: 10.5%) |
//! | `left_c5_ratio`      | left index effort share target (default: 8%) |
//! | `right_c1_ratio`     | right index effort share (mirrored from left) |
//! | `right_c2_ratio`     | right middle effort share (mirrored from left) |
//! | `right_c3_ratio`     | right ring effort share (mirrored from left) |
//! | `right_c4_ratio`     | right pinky effort share (mirrored from left) |
//! | `right_c5_ratio`     | right index effort share (mirrored from left) |
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
use crate::models::{ScoreResult, Target};
use itertools::Itertools;

/// One metric's diagnostic row: what it reads, what it wants, what it costs.
#[derive(Debug, Clone, Copy)]
pub struct TermReport {
    /// Metric name as printed in the CSV.
    pub name: &'static str,

    /// Measured value, percent units.
    pub value: f64,

    /// Configured limit or target point.
    pub goal: f64,

    /// Normalized deviation; `1.0` at the accepted edge.
    pub deviation: f64,

    /// Penalty contribution: `weight · deviation^sharpness`.
    pub cost: f64,

    /// Marginal cost per percentage point: `weight · sharpness · deviation^(s−1) / norm`.
    /// The term's pull in the tug-of-war — a metric rests off goal when its pressure is
    /// lower than what the opposing terms (and raw effort) gain from the same move.
    pub pressure: f64,
}

/// Penalty multiplier for a scored corpus. `1.0` = neutral, higher = worse layout.
/// See the module docs for the algebra behind each factor.
pub fn penalty(config: &LayoutEvaluatorConfig, r: &ScoreResult) -> f64 {
    1.0 + terms(config, r).map(|t| t.cost).sum::<f64>()
}

/// Per-metric diagnostics, worst first — the tuning aid that says which goal is losing
/// and therefore which weight or tolerance is worth adjusting.
pub fn breakdown(config: &LayoutEvaluatorConfig, r: &ScoreResult) -> Vec<TermReport> {
    terms(config, r)
        .sorted_by(|a, b| b.cost.total_cmp(&a.cost))
        .collect()
}

/// Render breakdown rows as an aligned table with a share-of-penalty column.
pub fn table(terms: &[TermReport]) -> String {
    let total: f64 = terms.iter().map(|t| t.cost).sum();
    let share = |cost: f64| {
        if total > 0.0 {
            100.0 * cost / total
        } else {
            0.0
        }
    };

    let header = format!(
        "{:<22}{:>9}{:>9}{:>8}{:>12}{:>8}{:>12}",
        "metric", "value", "goal", "dev", "cost", "share", "pressure"
    );

    terms
        .iter()
        .map(|t| {
            format!(
                "{:<22}{:>9.2}{:>9.2}{:>8.2}{:>12.4}{:>7.1}%{:>12.4}",
                t.name,
                t.value,
                t.goal,
                t.deviation,
                t.cost,
                share(t.cost),
                t.pressure
            )
        })
        .fold(header, |acc, row| acc + "\n" + &row)
}

/// Metric name paired with its penalty contribution; metrics without a target drop out.
/// Values are normalized to percent: `*_ratio` metrics are fractions and scale by 100,
/// `*_imbalance` metrics already come as percent. Behavior comes from each target's type.
fn terms<'a>(
    config: &'a LayoutEvaluatorConfig,
    r: &ScoreResult,
) -> impl Iterator<Item = TermReport> + 'a {
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
        ("home_row_balance", t.home_row_balance, r.home_row_balance()),
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
        target.map(|t| report(name, t, value, config.sharpness))
    })
}

/// Build one diagnostic row from a configured goal and its measured value.
fn report(name: &'static str, t: Target, value: f64, sharpness: f64) -> TermReport {
    let deviation = t.deviation(value);
    TermReport {
        name,
        value,
        goal: t.value,
        deviation,
        cost: t.weight * deviation.powf(sharpness),
        pressure: t.weight * sharpness * deviation.powf(sharpness - 1.0) / t.norm(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Targets;

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
            effort: 100.0,
            left_effort: 50.0,
            right_effort: 50.0,
            left_column_effort: [7.0, 11.5, 13.0, 10.5, 8.0],
            right_column_effort: [7.0, 11.5, 13.0, 10.5, 8.0],
            left_row_effort: [12.5, 30.0, 7.5],
            right_row_effort: [12.5, 30.0, 7.5],
            ..Default::default()
        };

        assert_eq!(penalty(&targets_config(), &balanced), 1.0);
    }

    /// A metric sitting exactly on its limit costs exactly its weight — that is what
    /// makes the configured `value` a normalizer and `weight` a plain priority.
    #[test]
    fn metric_at_its_limit_costs_its_weight() {
        let config = LayoutEvaluatorConfig {
            sharpness: 4.0,
            targets: Targets {
                hand_switch_ratio: Some(Target::max(35.0, 3.0)),
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
        let config = LayoutEvaluatorConfig {
            sharpness: 4.0,
            targets: Targets {
                hand_switch_ratio: Some(Target::max(35.0, 1.0)),
                ..Default::default()
            },
            ..Default::default()
        };
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
                    targets: Targets {
                        hand_switch_ratio: Some(Target::max(35.0, 1.0)),
                        ..Default::default()
                    },
                    ..Default::default()
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

    /// Distribution goals reward closeness, not values below the configured point,
    /// and cost exactly their weight at the tolerance edge — same algebra as `max`.
    #[test]
    fn target_metric_penalizes_both_sides_symmetrically() {
        let config = LayoutEvaluatorConfig {
            sharpness: 2.0,
            targets: Targets {
                top_row_ratio: Some(Target::target(25.0, 1.0)),
                ..Default::default()
            },
            ..Default::default()
        };
        let score = |top| ScoreResult {
            effort: 100.0,
            left_row_effort: [top, 0.0, 0.0],
            ..Default::default()
        };

        assert_eq!(penalty(&config, &score(25.0)), 1.0);
        assert_eq!(
            penalty(&config, &score(20.0)),
            penalty(&config, &score(30.0))
        );
        // 5pp miss = tolerance edge = weight on top of neutral 1.
        assert_eq!(penalty(&config, &score(20.0)), 2.0);
        assert!(penalty(&config, &score(0.0)) > penalty(&config, &score(20.0)));
    }

    /// Tighter tolerance walls up sooner: the same miss costs more.
    #[test]
    fn tighter_tolerance_raises_the_cost_of_the_same_miss() {
        let with_tolerance = |tolerance| {
            let config = LayoutEvaluatorConfig {
                sharpness: 4.0,
                targets: Targets {
                    top_row_ratio: Some(Target {
                        tolerance,
                        ..Target::target(25.0, 1.0)
                    }),
                    ..Default::default()
                },
                ..Default::default()
            };
            penalty(
                &config,
                &ScoreResult {
                    effort: 100.0,
                    left_row_effort: [20.0, 0.0, 0.0],
                    ..Default::default()
                },
            )
        };

        // 5pp miss: tolerance 5 → dev 1 → cost 1; tolerance 2.5 → dev 2 → cost 16.
        assert_eq!(with_tolerance(5.0), 2.0);
        assert_eq!(with_tolerance(2.5), 17.0);
    }

    /// Metrics without a target contribute nothing, whatever they read.
    #[test]
    fn metrics_without_a_target_are_ignored() {
        let config = LayoutEvaluatorConfig {
            targets: Targets {
                row_switch_ratio: Some(Target::max(20.0, 1.0)),
                ..Default::default()
            },
            ..Default::default()
        };

        // `skewed` has no row switches relative to nothing else configured.
        let only_row = penalty(&config, &skewed());
        let terms = breakdown(&config, &skewed());

        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].name, "row_switch_ratio");
        assert_eq!(only_row, 1.0 + terms[0].cost);
    }

    /// Breakdown ranks offenders so the loudest metric is obvious while tuning.
    #[test]
    fn breakdown_lists_worst_offender_first() {
        let terms = breakdown(&targets_config(), &skewed());

        assert!(terms.windows(2).all(|w| w[0].cost >= w[1].cost));
        assert_eq!(terms.len(), 21);
    }

    /// Pressure is zero at the goal and grows with the miss — the "who wins the next
    /// percentage point" number the tuning table is built around.
    #[test]
    fn pressure_is_zero_at_goal_and_grows_off_it() {
        let config = LayoutEvaluatorConfig {
            sharpness: 4.0,
            targets: Targets {
                top_row_ratio: Some(Target::target(25.0, 1.0)),
                ..Default::default()
            },
            ..Default::default()
        };
        let term = |top| {
            let score = ScoreResult {
                effort: 100.0,
                left_row_effort: [top, 0.0, 0.0],
                ..Default::default()
            };
            breakdown(&config, &score)[0]
        };

        assert_eq!(term(25.0).pressure, 0.0);
        // At the tolerance edge: w · s · dev^(s−1) / tol = 1 · 4 · 1 / 5.
        assert_eq!(term(20.0).pressure, 0.8);
        assert!(term(15.0).pressure > term(20.0).pressure);
    }

    /// The table renders one aligned row per term plus a header.
    #[test]
    fn table_renders_header_and_rows() {
        let terms = breakdown(&targets_config(), &skewed());
        let rendered = table(&terms);
        let lines: Vec<_> = rendered.lines().collect();

        assert_eq!(lines.len(), terms.len() + 1);
        assert!(lines[0].starts_with("metric"));
        assert!(lines[1].starts_with(terms[0].name));
        assert!(lines[1].contains('%'));
    }

    /// Every metric configured, all weights at 1 — the recommended starting point.
    fn targets_config() -> LayoutEvaluatorConfig {
        let limit = |value| Some(Target::max(value, 1.0));
        let target = |value| Some(Target::target(value, 1.0));

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
                top_row_ratio: target(25.0),
                home_row_ratio: target(60.0),
                home_row_balance: limit(15.0),
                bottom_row_ratio: target(15.0),
                c1_ratio: target(7.0),
                c2_ratio: target(11.5),
                c3_ratio: target(13.0),
                c4_ratio: target(10.5),
                c5_ratio: target(8.0),
            },
            ..Default::default()
        }
    }
}
