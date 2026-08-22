use crate::math::ratio;
use serde::Deserialize;

/// Static scoring configuration for desired metric limits.
/// Penalty = 1 + Σ weight · (|value| / max) ^ sharpness.
/// See [`crate::app::layout_evaluator::penalty`] for the algebra.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutEvaluatorConfig {
    /// Multiplier applied to the inverted fitness; sets the "ideal" score magnitude.
    #[serde(default = "default_fitness_scale")]
    pub fitness_scale: f64,

    /// Curve steepness: term = `weight · deviation^sharpness`.
    /// `1.0` = linear, higher = forgiving under the limit and brutal over it.
    #[serde(default = "default_sharpness")]
    pub sharpness: f64,

    /// Desired limits per metric, in the percent units the CSV prints.
    #[serde(flatten, default)]
    pub targets: Targets,
}

/// Serde default for [`LayoutEvaluatorConfig::fitness_scale`].
fn default_fitness_scale() -> f64 {
    1_000_000.
}

/// Serde default for [`LayoutEvaluatorConfig::sharpness`].
fn default_sharpness() -> f64 {
    4.0
}

/// Serde default for [`Target::weight`]: every metric equally important at its own limit.
fn default_weight() -> f64 {
    1.0
}

/// Serde default for top row ratio target: 25%.
fn default_top_row_ratio() -> Option<Target> {
    Some(Target {
        max: 25.0,
        weight: default_weight(),
    })
}

/// Serde default for home row ratio target: 60%.
fn default_home_row_ratio() -> Option<Target> {
    Some(Target {
        max: 60.0,
        weight: default_weight(),
    })
}

/// Serde default for bottom row ratio target: 15%.
fn default_bottom_row_ratio() -> Option<Target> {
    Some(Target {
        max: 15.0,
        weight: default_weight(),
    })
}

/// Desired limits per metric, in the percent units the CSV prints.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct Targets {
    /// Limit for `row_switch_ratio`: row jumps inside a hand.
    pub row_switch_ratio: Option<Target>,

    /// Limit for `hand_switch_ratio`: hand alternation. Replaces `mean_streak_power`.
    pub hand_switch_ratio: Option<Target>,

    /// Limit for `efforts_imbalance`: left/right effort asymmetry.
    pub efforts_imbalance: Option<Target>,

    /// Limit for `hands_imbalance`: left/right press-count asymmetry.
    pub hands_imbalance: Option<Target>,

    /// Limit for `roll_imbalance`: left/right roll asymmetry.
    pub roll_imbalance: Option<Target>,

    /// Limit for `row_switch_imbalance`: left/right row-step asymmetry.
    pub row_switch_imbalance: Option<Target>,

    /// Limit for `streak_imbalance`: left/right run-length asymmetry.
    pub streak_imbalance: Option<Target>,

    /// Limit for `top_row_ratio`: top row effort share. Default: 25%.
    #[serde(default = "default_top_row_ratio")]
    pub top_row_ratio: Option<Target>,

    /// Limit for `home_row_ratio`: home row effort share. Default: 60%.
    #[serde(default = "default_home_row_ratio")]
    pub home_row_ratio: Option<Target>,

    /// Limit for `bottom_row_ratio`: bottom row effort share. Default: 15%.
    #[serde(default = "default_bottom_row_ratio")]
    pub bottom_row_ratio: Option<Target>,
}

impl Targets {
    /// True when no metric is configured.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Desired limit for one metric. The ideal is always `0`; `max` is the value that
/// costs one full `weight`, which also normalizes metrics against each other.
/// YAML accepts a bare number (`rowSwitchRatio: 20`) or `{ max: 20, weight: 2 }`.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(from = "TargetSpec")]
pub struct Target {
    /// Value that counts as one full unit of pain; also the scale normalizer.
    pub max: f64,

    /// Priority against the other metrics. Only needed when one metric should
    /// give way before another.
    pub weight: f64,
}

impl Target {
    /// Normalized deviation: `0` = perfect, `1` = at the limit, `>1` = over it.
    /// Sign is dropped — magnitude is what costs.
    pub fn deviation(&self, value: f64) -> f64 {
        ratio(value.abs(), self.max)
    }
}

/// Deserialization shim accepting the bare-number shorthand alongside the full form.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
enum TargetSpec {
    Max(f64),
    Full {
        max: f64,
        #[serde(default = "default_weight")]
        weight: f64,
    },
}

impl From<TargetSpec> for Target {
    fn from(spec: TargetSpec) -> Self {
        match spec {
            TargetSpec::Max(max) => Target {
                max,
                weight: default_weight(),
            },
            TargetSpec::Full { max, weight } => Target { max, weight },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bare number is the common case: a limit with default priority. It must
    /// parse into the same target as the explicit form.
    #[test]
    fn bare_number_and_full_target_forms_agree() {
        let bare: LayoutEvaluatorConfig =
            serde_json::from_str(r#"{"rowSwitchRatio": 20}"#).unwrap();
        let full: LayoutEvaluatorConfig =
            serde_json::from_str(r#"{"rowSwitchRatio": {"max": 20, "weight": 1}}"#).unwrap();

        assert_eq!(bare, full);
        assert_eq!(
            bare.targets.row_switch_ratio,
            Some(Target {
                max: 20.0,
                weight: 1.0
            })
        );
        assert!(!bare.targets.is_empty());
    }

    /// Deviation is the metric measured in "limits": on target, at the limit, over it.
    #[test]
    fn deviation_normalizes_against_the_limit() {
        let target = Target {
            max: 20.0,
            weight: 1.0,
        };

        assert_eq!(target.deviation(0.0), 0.0);
        assert_eq!(target.deviation(10.0), 0.5);
        assert_eq!(target.deviation(20.0), 1.0);
        assert_eq!(target.deviation(40.0), 2.0);
        // Sign carries direction, not cost.
        assert_eq!(target.deviation(-20.0), 1.0);
    }

    /// A misspelled metric name must fail the load, not silently score nothing.
    #[test]
    fn unknown_target_names_are_rejected() {
        let json = r#"{"rowSwitchRation": 20}"#;

        assert!(serde_json::from_str::<LayoutEvaluatorConfig>(json).is_err());
    }
}
