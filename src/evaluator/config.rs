use crate::math::ratio;
use serde::Deserialize;

/// Static scoring configuration for desired metric goals.
/// Penalty = 1 + Σ weight · deviation ^ sharpness.
/// See [`crate::layout_evaluator::penalty`] for the algebra.
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

    /// Desired goals per metric, in the percent units the CSV prints.
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
        value: 25.0,
        weight: default_weight(),
        kind: TargetType::Target,
    })
}

/// Serde default for home row ratio target: 60%.
fn default_home_row_ratio() -> Option<Target> {
    Some(Target {
        value: 60.0,
        weight: default_weight(),
        kind: TargetType::Target,
    })
}

/// Serde default for home row balance target: 15%.
fn default_home_row_balance() -> Option<Target> {
    Some(Target {
        value: 15.0,
        weight: default_weight(),
        kind: TargetType::Max,
    })
}

/// Serde default for bottom row ratio target: 15%.
fn default_bottom_row_ratio() -> Option<Target> {
    Some(Target {
        value: 15.0,
        weight: default_weight(),
        kind: TargetType::Target,
    })
}

/// Serde default for column 1 (pinky) ratio target: 7%.
fn default_c1_ratio() -> Option<Target> {
    Some(Target {
        value: 7.0,
        weight: default_weight(),
        kind: TargetType::Target,
    })
}

/// Serde default for column 2 (ring) ratio target: 11.5%.
fn default_c2_ratio() -> Option<Target> {
    Some(Target {
        value: 11.5,
        weight: default_weight(),
        kind: TargetType::Target,
    })
}

/// Serde default for column 3 (middle) ratio target: 13%.
fn default_c3_ratio() -> Option<Target> {
    Some(Target {
        value: 13.0,
        weight: default_weight(),
        kind: TargetType::Target,
    })
}

/// Serde default for column 4 (index) ratio target: 10.5%.
fn default_c4_ratio() -> Option<Target> {
    Some(Target {
        value: 10.5,
        weight: default_weight(),
        kind: TargetType::Target,
    })
}

/// Serde default for column 5 (index) ratio target: 8%.
fn default_c5_ratio() -> Option<Target> {
    Some(Target {
        value: 8.0,
        weight: default_weight(),
        kind: TargetType::Target,
    })
}

/// Desired goals per metric, in the percent units the CSV prints.
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

    /// Target for `top_row_ratio`: top row effort share. Default: 25%.
    #[serde(default = "default_top_row_ratio")]
    pub top_row_ratio: Option<Target>,

    /// Target for `home_row_ratio`: home row effort share. Default: 60%.
    #[serde(default = "default_home_row_ratio")]
    pub home_row_ratio: Option<Target>,

    /// Limit for `home_row_balance`: home row left/right balance (absolute value). Default: 15%.
    #[serde(default = "default_home_row_balance")]
    pub home_row_balance: Option<Target>,

    /// Target for `bottom_row_ratio`: bottom row effort share. Default: 15%.
    #[serde(default = "default_bottom_row_ratio")]
    pub bottom_row_ratio: Option<Target>,

    /// Target for left column 1 (pinky) effort share. Default: 7%.
    #[serde(default = "default_c1_ratio")]
    pub c1_ratio: Option<Target>,

    /// Target for left column 2 (ring) effort share. Default: 11.5%.
    #[serde(default = "default_c2_ratio")]
    pub c2_ratio: Option<Target>,

    /// Target for left column 3 (middle) effort share. Default: 13%.
    #[serde(default = "default_c3_ratio")]
    pub c3_ratio: Option<Target>,

    /// Target for left column 4 (index) effort share. Default: 10.5%.
    #[serde(default = "default_c4_ratio")]
    pub c4_ratio: Option<Target>,

    /// Target for left column 5 (index) effort share. Default: 8%.
    /// Right-hand column targets are computed/mirrored from left-hand values.
    #[serde(default = "default_c5_ratio")]
    pub c5_ratio: Option<Target>,
}

impl Targets {
    /// True when no metric is configured.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Behavior used to measure one metric's deviation.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TargetType {
    /// Lower is better; `value` is the accepted upper limit.
    Max,
    /// Closer is better; `value` is the desired point.
    Target,
}

/// Desired goal for one metric.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    /// Limit or target point, selected by `type`.
    pub value: f64,

    /// Priority against the other metrics.
    #[serde(default = "default_weight")]
    pub weight: f64,

    /// Deviation behavior, serialized as YAML key `type`.
    #[serde(rename = "type")]
    pub kind: TargetType,
}

impl Target {
    /// Normalized deviation: zero at the ideal for the selected behavior.
    pub fn deviation(&self, value: f64) -> f64 {
        match self.kind {
            TargetType::Max => ratio(value.abs(), self.value),
            TargetType::Target => (value - self.value).abs() / 100.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Target behavior is explicit and uses the neutral `value` field name.
    #[test]
    fn explicit_target_form_parses() {
        let config: LayoutEvaluatorConfig = serde_json::from_str(
            r#"{"rowSwitchRatio": {"type": "max", "value": 20, "weight": 1}}"#,
        )
        .unwrap();

        assert_eq!(
            config.targets.row_switch_ratio,
            Some(Target {
                value: 20.0,
                weight: 1.0,
                kind: TargetType::Max,
            })
        );
        assert!(!config.targets.is_empty());
    }

    /// Max deviation measures the metric in accepted-limit units.
    #[test]
    fn max_deviation_normalizes_against_the_limit() {
        let target = Target {
            value: 20.0,
            weight: 1.0,
            kind: TargetType::Max,
        };

        assert_eq!(target.deviation(0.0), 0.0);
        assert_eq!(target.deviation(10.0), 0.5);
        assert_eq!(target.deviation(20.0), 1.0);
        assert_eq!(target.deviation(40.0), 2.0);
        // Sign carries direction, not cost.
        assert_eq!(target.deviation(-20.0), 1.0);
    }

    /// Point targets penalize equal percentage-point misses symmetrically.
    #[test]
    fn target_deviation_is_symmetric_around_value() {
        let target = Target {
            value: 25.0,
            weight: 1.0,
            kind: TargetType::Target,
        };

        assert_eq!(target.deviation(25.0), 0.0);
        assert_eq!(target.deviation(20.0), 0.05);
        assert_eq!(target.deviation(30.0), 0.05);
    }

    /// Ambiguous legacy forms must fail instead of silently choosing behavior.
    #[test]
    fn target_type_and_value_are_required() {
        let bare = r#"{"rowSwitchRatio": 20}"#;
        let old = r#"{"rowSwitchRatio": {"max": 20, "weight": 1}}"#;

        assert!(serde_json::from_str::<LayoutEvaluatorConfig>(bare).is_err());
        assert!(serde_json::from_str::<LayoutEvaluatorConfig>(old).is_err());
    }

    /// A misspelled metric name must fail the load, not silently score nothing.
    #[test]
    fn unknown_target_names_are_rejected() {
        let json = r#"{"rowSwitchRation": {"type": "max", "value": 20, "weight": 1}}"#;

        assert!(serde_json::from_str::<LayoutEvaluatorConfig>(json).is_err());
    }
}
