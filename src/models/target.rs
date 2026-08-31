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

    /// Accepted miss for `target` kind, in percentage points; cost = weight at this
    /// distance from `value`, wall beyond. Ignored by `max`. Default: 5.
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,

    /// Deviation behavior, serialized as YAML key `type`.
    #[serde(rename = "type")]
    pub kind: TargetType,
}

/// Serde default for [`Target::weight`]: every metric equally important at its own limit.
pub(crate) fn default_weight() -> f64 {
    1.0
}

/// Serde default for [`Target::tolerance`]: 5 percentage points.
pub(crate) fn default_tolerance() -> f64 {
    5.0
}

impl Target {
    /// Cap: lower is better, `value` is the accepted upper limit.
    pub fn max(value: f64, weight: f64) -> Self {
        Self {
            value,
            weight,
            tolerance: default_tolerance(),
            kind: TargetType::Max,
        }
    }

    /// Point goal: closer is better, default tolerance edge at 5pp.
    /// Named after the YAML `type: target`; symmetry with [`Self::max`] beats the lint.
    #[allow(clippy::self_named_constructors)]
    pub fn target(value: f64, weight: f64) -> Self {
        Self {
            value,
            weight,
            tolerance: default_tolerance(),
            kind: TargetType::Target,
        }
    }

    /// Normalized deviation: zero at the ideal, `1.0` at the accepted edge —
    /// the limit for `max`, `tolerance` away from the point for `target`.
    pub fn deviation(&self, value: f64) -> f64 {
        match self.kind {
            TargetType::Max => ratio(value.abs(), self.value),
            TargetType::Target => (value - self.value).abs() / self.tolerance,
        }
    }

    /// Normalizer mapping percentage points to deviation units; the accepted edge.
    pub fn norm(&self) -> f64 {
        match self.kind {
            TargetType::Max => self.value,
            TargetType::Target => self.tolerance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Max deviation measures the metric in accepted-limit units.
    #[test]
    fn max_deviation_normalizes_against_the_limit() {
        let target = Target::max(20.0, 1.0);

        assert_eq!(target.deviation(0.0), 0.0);
        assert_eq!(target.deviation(10.0), 0.5);
        assert_eq!(target.deviation(20.0), 1.0);
        assert_eq!(target.deviation(40.0), 2.0);
        // Sign carries direction, not cost.
        assert_eq!(target.deviation(-20.0), 1.0);
    }

    /// Point targets penalize equal percentage-point misses symmetrically,
    /// hitting deviation `1.0` exactly at the tolerance edge.
    #[test]
    fn target_deviation_is_symmetric_around_value() {
        let target = Target::target(25.0, 1.0);

        assert_eq!(target.deviation(25.0), 0.0);
        assert_eq!(target.deviation(20.0), 1.0);
        assert_eq!(target.deviation(30.0), 1.0);
        assert_eq!(target.deviation(27.5), 0.5);
    }

    /// Tolerance rescales the accepted miss: tighter tolerance, faster wall.
    #[test]
    fn tolerance_sets_the_accepted_edge() {
        let tight = Target {
            tolerance: 2.0,
            ..Target::target(25.0, 1.0)
        };

        assert_eq!(tight.deviation(27.0), 1.0);
        assert_eq!(tight.deviation(29.0), 2.0);
    }

    /// Target behavior is explicit and uses the neutral `value` field name.
    #[test]
    fn explicit_target_form_parses() {
        let target: Target =
            serde_json::from_str(r#"{"type": "max", "value": 20, "weight": 1}"#).unwrap();

        assert_eq!(target, Target::max(20.0, 1.0));
    }

    /// Tolerance parses when given and defaults to 5 when omitted.
    #[test]
    fn tolerance_parses_and_defaults() {
        let custom: Target =
            serde_json::from_str(r#"{"type": "target", "value": 10, "tolerance": 2}"#).unwrap();
        let default: Target = serde_json::from_str(r#"{"type": "target", "value": 10}"#).unwrap();

        assert_eq!(custom.tolerance, 2.0);
        assert_eq!(default.tolerance, 5.0);
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
