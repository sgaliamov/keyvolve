use serde::Deserialize;

/// Static scoring knobs for layout evaluation.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutEvaluatorConfig {
    /// Extra effort charged per hand switch, in pairs-table effort units; `0.0` disables.
    #[serde(default)]
    pub switch_cost: f64,

    /// Extra effort charged per same-hand row step (adjacent = 1, jump = 2); `0.0` disables.
    #[serde(default)]
    pub row_cost: f64,

    /// Multiplier applied to the inverted fitness; sets the "ideal" score magnitude.
    #[serde(default = "default_fitness_scale")]
    pub fitness_scale: f64,

    /// Exponent applied to hand-count imbalance; `< 1.0` softens balance pressure.
    #[serde(default = "default_balance_penalty_power")]
    pub balance_penalty_power: f64,

    /// Exponent applied to left/right streak imbalance.
    #[serde(default = "default_streak_penalty_power")]
    pub streak_penalty_power: f64,

    /// Exponent applied to left/right row-switch imbalance.
    #[serde(default = "default_row_switch_penalty_power")]
    pub row_switch_penalty_power: f64,

    /// Strength applied to overall row-switch ratio; `0.0` disables, `1.0` uses full excess.
    #[serde(default = "default_row_switch_ratio_penalty_power")]
    pub row_switch_ratio_penalty_power: f64,

    /// Exponent scale applied to the shorter-hand streak divisor; `< 1.0` softens.
    #[serde(default = "default_min_streak_penalty_power")]
    pub min_streak_penalty_power: f64,
}

/// Serde default for [`LayoutEvaluatorConfig::fitness_scale`].
fn default_fitness_scale() -> f64 {
    1_000_000.
}

/// Serde default for [`LayoutEvaluatorConfig::balance_penalty_power`].
fn default_balance_penalty_power() -> f64 {
    1.0
}

/// Serde default for [`LayoutEvaluatorConfig::streak_penalty_power`].
fn default_streak_penalty_power() -> f64 {
    1.0
}

/// Serde default for [`LayoutEvaluatorConfig::row_switch_penalty_power`].
fn default_row_switch_penalty_power() -> f64 {
    1.0
}

/// Serde default for [`LayoutEvaluatorConfig::row_switch_ratio_penalty_power`].
fn default_row_switch_ratio_penalty_power() -> f64 {
    0.0
}

/// Serde default for [`LayoutEvaluatorConfig::min_streak_penalty_power`].
fn default_min_streak_penalty_power() -> f64 {
    1.0
}

impl Default for LayoutEvaluatorConfig {
    fn default() -> Self {
        Self {
            switch_cost: 0.0,
            row_cost: 0.0,
            fitness_scale: default_fitness_scale(),
            balance_penalty_power: default_balance_penalty_power(),
            streak_penalty_power: default_streak_penalty_power(),
            row_switch_penalty_power: default_row_switch_penalty_power(),
            row_switch_ratio_penalty_power: default_row_switch_ratio_penalty_power(),
            min_streak_penalty_power: default_min_streak_penalty_power(),
        }
    }
}
