use super::target::Target;
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

    /// Target for left pinky effort share. Default: 7%.
    #[serde(default = "default_pinky_ratio")]
    pub pinky_ratio: Option<Target>,

    /// Target for left ring effort share. Default: 11.5%.
    #[serde(default = "default_ring_ratio")]
    pub ring_ratio: Option<Target>,

    /// Target for left middle effort share. Default: 13%.
    #[serde(default = "default_middle_ratio")]
    pub middle_ratio: Option<Target>,

    /// Target for left index (inner) effort share. Default: 10.5%.
    #[serde(default = "default_index_inner_ratio")]
    pub index_inner_ratio: Option<Target>,

    /// Target for left index (outer) effort share. Default: 8%.
    /// Right-hand column targets are computed/mirrored from left-hand values.
    #[serde(default = "default_index_outer_ratio")]
    pub index_outer_ratio: Option<Target>,

    /// Limit for `pinky_balance`: pinky column left/right effort asymmetry. Default: 3%.
    #[serde(default = "default_column_balance")]
    pub pinky_balance: Option<Target>,

    /// Limit for `ring_balance`: ring column left/right effort asymmetry. Default: 3%.
    #[serde(default = "default_column_balance")]
    pub ring_balance: Option<Target>,

    /// Limit for `middle_balance`: middle column left/right effort asymmetry. Default: 3%.
    #[serde(default = "default_column_balance")]
    pub middle_balance: Option<Target>,

    /// Limit for `index_inner_balance`: index-inner column left/right effort asymmetry. Default: 3%.
    #[serde(default = "default_column_balance")]
    pub index_inner_balance: Option<Target>,

    /// Limit for `index_outer_balance`: index-outer column left/right effort asymmetry. Default: 3%.
    #[serde(default = "default_column_balance")]
    pub index_outer_balance: Option<Target>,
}

impl Targets {
    /// True when no metric is configured.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Serde default for top row ratio target: 25%.
fn default_top_row_ratio() -> Option<Target> {
    Some(Target::target(25.0, 1.0))
}

/// Serde default for home row ratio target: 60%.
fn default_home_row_ratio() -> Option<Target> {
    Some(Target::target(60.0, 1.0))
}

/// Serde default for home row balance target: 15%.
fn default_home_row_balance() -> Option<Target> {
    Some(Target::max(15.0, 1.0))
}

/// Serde default for bottom row ratio target: 15%.
fn default_bottom_row_ratio() -> Option<Target> {
    Some(Target::target(15.0, 1.0))
}

/// Serde default for pinky ratio target: 7%.
fn default_pinky_ratio() -> Option<Target> {
    Some(Target::target(7.0, 1.0))
}

/// Serde default for ring ratio target: 11.5%.
fn default_ring_ratio() -> Option<Target> {
    Some(Target::target(11.5, 1.0))
}

/// Serde default for middle ratio target: 13%.
fn default_middle_ratio() -> Option<Target> {
    Some(Target::target(13.0, 1.0))
}

/// Serde default for index inner ratio target: 10.5%.
fn default_index_inner_ratio() -> Option<Target> {
    Some(Target::target(10.5, 1.0))
}

/// Serde default for index outer ratio target: 8%.
fn default_index_outer_ratio() -> Option<Target> {
    Some(Target::target(8.0, 1.0))
}

/// Serde default for per-finger column balance targets: 3% max skew, weight 0.5.
fn default_column_balance() -> Option<Target> {
    Some(Target::max(3.0, 0.5))
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
                top_row_ratio: Some(Target::target(25.0, 1.0)),
                ..Default::default()
            }
            .is_empty()
        );
    }
}
