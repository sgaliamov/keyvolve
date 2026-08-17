use crate::math::ratio;
use serde::Deserialize;

/// Static scoring knobs. Two mutually exclusive modes:
///
/// * **targets** — desired limits per metric, in the percent units the CSV prints.
///   Active whenever [`Targets`] holds at least one entry.
/// * **powers** — legacy `factor ^ power` knobs, where `factor` is dimensionless and
///   `>= 1.0` means "worse". `0.0` = off, `1.0` = full strength. Used when `targets` is empty.
///
/// See [`crate::app::layout_evaluator::penalty`] for the derivation behind each factor.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutEvaluatorConfig {
    /// Multiplier applied to the inverted fitness; sets the "ideal" score magnitude.
    #[serde(default = "default_fitness_scale")]
    pub fitness_scale: f64,

    /// Curve steepness in targets mode: term = `weight · deviation^sharpness`.
    /// `1.0` = linear, higher = forgiving under the limit and brutal over it.
    #[serde(default = "default_sharpness")]
    pub sharpness: f64,

    /// Desired metric limits. Empty → the power knobs below drive the penalty.
    #[serde(default)]
    pub targets: Targets,

    /// Balance: hand-effort imbalance (CSV: `efforts_imbalance`). Legacy, ignored in targets mode.
    #[serde(default = "default_power")]
    pub balance_power: f64,

    /// Balance: left/right run-length imbalance (CSV: `streak_ratio`).
    #[serde(default = "default_power")]
    pub streak_power: f64,

    /// Balance: left/right roll imbalance (CSV: `roll_ratio`).
    #[serde(default = "default_power")]
    pub roll_imbalance_power: f64,

    /// Level: long same-hand runs, applied as a reward divisor (CSV: `mean_streak`).
    #[serde(default = "default_power")]
    pub mean_streak_power: f64,

    /// Balance: left/right row-step imbalance (CSV: `row_switch_imbalance`).
    #[serde(default = "default_power")]
    pub row_imbalance_power: f64,

    /// Level: row jumps within a hand (CSV: `row_switch_ratio`).
    #[serde(default = "default_power")]
    pub row_power: f64,

    /// Level: combined hand switches + row jumps (CSV: `hand_switch_ratio/2 + row_switch_ratio`).
    #[serde(default = "default_power")]
    pub switch_power: f64,
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

/// Serde default for every penalty power: disabled.
fn default_power() -> f64 {
    0.0
}

impl Default for LayoutEvaluatorConfig {
    fn default() -> Self {
        Self {
            fitness_scale: default_fitness_scale(),
            sharpness: default_sharpness(),
            targets: Targets::default(),
            balance_power: default_power(),
            streak_power: default_power(),
            roll_imbalance_power: default_power(),
            mean_streak_power: default_power(),
            row_imbalance_power: default_power(),
            row_power: default_power(),
            switch_power: default_power(),
        }
    }
}

/// Desired limits per metric, in the percent units the CSV prints. Every entry is
/// optional; all absent means the legacy power knobs own the penalty.
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
}

impl Targets {
    /// No metric configured → the legacy power penalty owns scoring.
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

    /// A stale knob name must not deserialize into silence. `minStreakPower`
    /// was removed; a config still carrying it is asking for a
    /// penalty that no longer exists, so the load has to fail rather than default.
    #[test]
    fn stale_knob_names_are_rejected() {
        for stale in ["minStreakPower", "balancePenaltyPower", "countPower"] {
            let json = format!(r#"{{"balancePower": 1.0, "{stale}": 0.8}}"#);

            assert!(
                serde_json::from_str::<LayoutEvaluatorConfig>(&json).is_err(),
                "{stale} should be rejected"
            );
        }
    }

    /// Every knob is optional and defaults to disabled.
    #[test]
    fn omitted_knobs_default_to_disabled() {
        let config: LayoutEvaluatorConfig = serde_json::from_str("{}").unwrap();

        assert_eq!(config, LayoutEvaluatorConfig::default());
        assert_eq!(config.mean_streak_power, 0.0);
        assert_eq!(config.switch_power, 0.0);
        assert!(config.targets.is_empty());
    }

    /// A bare number is the common case: a limit with default priority. It must
    /// parse into the same target as the explicit form.
    #[test]
    fn bare_number_and_full_target_forms_agree() {
        let bare: LayoutEvaluatorConfig =
            serde_json::from_str(r#"{"targets": {"rowSwitchRatio": 20}}"#).unwrap();
        let full: LayoutEvaluatorConfig =
            serde_json::from_str(r#"{"targets": {"rowSwitchRatio": {"max": 20, "weight": 1}}}"#)
                .unwrap();

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
        let json = r#"{"targets": {"rowSwitchRation": 20}}"#;

        assert!(serde_json::from_str::<LayoutEvaluatorConfig>(json).is_err());
    }
}
