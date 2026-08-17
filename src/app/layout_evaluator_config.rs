use serde::Deserialize;

/// Static scoring knobs. Every penalty knob shares one scheme: `factor ^ power`,
/// where `factor` is dimensionless and `>= 1.0` means "worse".
/// `0.0` = off, `1.0` = full strength, between = softer, above = stricter.
/// Two knobs per metric family: *level* (how much of the trait) and *balance*
/// (how evenly the hands split it). See [`crate::app::layout_evaluator::penalty`]
/// for the derivation behind each factor.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutEvaluatorConfig {
    /// Multiplier applied to the inverted fitness; sets the "ideal" score magnitude.
    #[serde(default = "default_fitness_scale")]
    pub fitness_scale: f64,

    /// Balance: hand-effort imbalance (CSV: `efforts_imbalance`).
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

/// Serde default for every penalty power: disabled.
fn default_power() -> f64 {
    0.0
}

impl Default for LayoutEvaluatorConfig {
    fn default() -> Self {
        Self {
            fitness_scale: default_fitness_scale(),
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
    }
}
