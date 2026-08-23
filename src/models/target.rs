use crate::math::ratio;
use serde::Deserialize;

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

/// Serde default for [`Target::weight`]: every metric equally important at its own limit.
pub(crate) fn default_weight() -> f64 {
    1.0
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

    /// Target behavior is explicit and uses the neutral `value` field name.
    #[test]
    fn explicit_target_form_parses() {
        let target: Target =
            serde_json::from_str(r#"{"type": "max", "value": 20, "weight": 1}"#).unwrap();

        assert_eq!(
            target,
            Target {
                value: 20.0,
                weight: 1.0,
                kind: TargetType::Max,
            }
        );
    }

    /// Ambiguous legacy forms must fail instead of silently choosing behavior.
    #[test]
    fn target_type_and_value_are_required() {
        let bare = r#"20"#;
        let old = r#"{"max": 20, "weight": 1}"#;

        assert!(serde_json::from_str::<Target>(bare).is_err());
        assert!(serde_json::from_str::<Target>(old).is_err());
    }
}
