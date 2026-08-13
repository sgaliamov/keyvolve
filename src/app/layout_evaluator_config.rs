use serde::Deserialize;

/// Static scoring knobs for layout evaluation.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
/// All penalty knobs share one scheme: dimensionless factor ^ power.
/// `0.0` = off, `1.0` = full strength, between = softer, above = stricter.
pub struct LayoutEvaluatorConfig {
    /// Multiplier applied to the inverted fitness; sets the "ideal" score magnitude.
    #[serde(default = "default_fitness_scale")]
    pub fitness_scale: f64,

    /// Exponent applied to hand-count imbalance; `< 1.0` softens balance pressure.
    #[serde(default = "default_count_power", alias = "balancePenaltyPower")]
    pub count_power: f64,

    /// Exponent applied to left/right streak imbalance.
    #[serde(default = "default_streak_power", alias = "streakPenaltyPower")]
    pub streak_power: f64,

    /// Exponent applied to left/right row-switch imbalance.
    #[serde(
        default = "default_row_imbalance_power",
        alias = "rowSwitchPenaltyPower"
    )]
    pub row_imbalance_power: f64,

    /// Exponent applied to the hand-switch share factor `1 + hand_switch_ratio`.
    #[serde(default)]
    pub switch_power: f64,

    /// Exponent applied to the row-switch share factor `1 + row_switch_ratio`.
    #[serde(default)]
    pub row_power: f64,

    /// Exponent on the shorter-hand streak reward divisor `min_streak`.
    #[serde(default = "default_min_streak_power", alias = "minStreakPenaltyPower")]
    pub min_streak_power: f64,
}

/// Serde default for [`LayoutEvaluatorConfig::fitness_scale`].
fn default_fitness_scale() -> f64 {
    1_000_000.
}

/// Serde default for [`LayoutEvaluatorConfig::count_power`].
fn default_count_power() -> f64 {
    1.0
}

/// Serde default for [`LayoutEvaluatorConfig::streak_power`].
fn default_streak_power() -> f64 {
    1.0
}

/// Serde default for [`LayoutEvaluatorConfig::row_imbalance_power`].
fn default_row_imbalance_power() -> f64 {
    1.0
}

/// Serde default for [`LayoutEvaluatorConfig::min_streak_power`].
fn default_min_streak_power() -> f64 {
    1.0
}

impl Default for LayoutEvaluatorConfig {
    fn default() -> Self {
        Self {
            fitness_scale: default_fitness_scale(),
            count_power: default_count_power(),
            streak_power: default_streak_power(),
            row_imbalance_power: default_row_imbalance_power(),
            switch_power: 0.0,
            row_power: 0.0,
            min_streak_power: default_min_streak_power(),
        }
    }
}
