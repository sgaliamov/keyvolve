use crate::models::Targets;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Target, TargetType};

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
