use super::target::{Target, TargetType, default_weight};
use serde::Deserialize;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// A target set is empty only when every field is absent.
    #[test]
    fn empty_target_set_detects_default() {
        assert!(Targets::default().is_empty());
        assert!(
            !Targets {
                top_row_ratio: Some(Target {
                    value: 25.0,
                    weight: 1.0,
                    kind: TargetType::Target,
                }),
                ..Default::default()
            }
            .is_empty()
        );
    }
}
