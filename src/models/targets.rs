use super::target::Target;
use serde::Deserialize;

/// Desired goals per metric, in the percent units the CSV prints.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Targets {
    /// Limit for `row_switch_ratio`: row jumps inside a hand.
    pub row_switch_ratio: Option<Target>,

    /// Limit for `hand_switch_ratio`: hand alternation. Replaces `mean_streak_power`.
    pub hand_switch_ratio: Option<Target>,

    /// Limit for `sfs_ratio`: same-finger skipgram share. Default: none until measured.
    pub sfs_ratio: Option<Target>,

    /// Limit for `directional_outward_ratio`: outward share among directional same-hand moves.
    pub directional_outward_ratio: Option<Target>,

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

    /// Target for `top_row_ratio`: top row effort share.
    pub top_row_ratio: Option<Target>,

    /// Target for `home_row_ratio`: home row effort share.
    pub home_row_ratio: Option<Target>,

    /// Limit for `home_row_balance`: home row left/right balance (absolute value).
    pub home_row_balance: Option<Target>,

    /// Target for `bottom_row_ratio`: bottom row effort share.
    pub bottom_row_ratio: Option<Target>,

    /// Target for left pinky effort share.
    pub pinky_ratio: Option<Target>,

    /// Target for left ring effort share.
    pub ring_ratio: Option<Target>,

    /// Target for left middle effort share.
    pub middle_ratio: Option<Target>,

    /// Target for left index (inner) effort share.
    pub index_inner_ratio: Option<Target>,

    /// Target for left index (outer) effort share.
    /// Right-hand column targets are computed/mirrored from left-hand values.
    pub index_outer_ratio: Option<Target>,

    /// Limit for `pinky_balance`: pinky column left/right effort asymmetry.
    pub pinky_balance: Option<Target>,

    /// Limit for `ring_balance`: ring column left/right effort asymmetry.
    pub ring_balance: Option<Target>,

    /// Limit for `middle_balance`: middle column left/right effort asymmetry.
    pub middle_balance: Option<Target>,

    /// Limit for `index_inner_balance`: index-inner column left/right effort asymmetry.
    pub index_inner_balance: Option<Target>,

    /// Limit for `index_outer_balance`: index-outer column left/right effort asymmetry.
    pub index_outer_balance: Option<Target>,

    /// Limit for `pinky_row_switch_ratio`: pinky same-finger row-switch ratio cap.
    pub pinky_row_switch_ratio: Option<Target>,

    /// Limit for `ring_row_switch_ratio`: ring same-finger row-switch ratio cap.
    pub ring_row_switch_ratio: Option<Target>,

    /// Limit for `middle_row_switch_ratio`: middle same-finger row-switch ratio cap.
    pub middle_row_switch_ratio: Option<Target>,

    /// Limit for `index_row_switch_ratio`: merged-index same-finger row-switch ratio cap.
    pub index_row_switch_ratio: Option<Target>,

    /// Limit for `pinky_row_switch_balance`: pinky same-finger row-switch left/right asymmetry.
    pub pinky_row_switch_balance: Option<Target>,

    /// Limit for `ring_row_switch_balance`: ring same-finger row-switch left/right asymmetry.
    pub ring_row_switch_balance: Option<Target>,

    /// Limit for `middle_row_switch_balance`: middle same-finger row-switch left/right asymmetry.
    pub middle_row_switch_balance: Option<Target>,

    /// Limit for `index_row_switch_balance`: merged-index same-finger row-switch left/right asymmetry.
    pub index_row_switch_balance: Option<Target>,
}

impl Targets {
    /// True when no metric is configured.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
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
        assert!(serde_json::from_str::<Targets>("{}").unwrap().is_empty());
    }

    /// Same-finger row-switch ratio and balance knobs deserialize with camelCase names.
    #[test]
    fn finger_row_switch_targets_parse() {
        let targets: Targets = serde_json::from_str(
            r#"{
                "pinkyRowSwitchRatio": {"type": "max", "value": 10, "weight": 0.25},
                "ringRowSwitchRatio": {"type": "max", "value": 11, "weight": 0.3},
                "middleRowSwitchBalance": {"type": "max", "value": 12, "weight": 0.4},
                "indexRowSwitchBalance": {"type": "max", "value": 13, "weight": 0.5},
                "sfsRatio": {"type": "max", "value": 4, "weight": 0.1},
                "directionalOutwardRatio": {"type": "max", "value": 40, "weight": 1.25}
            }"#,
        )
        .unwrap();

        assert_eq!(
            targets.pinky_row_switch_ratio,
            Some(Target::max(10.0, 0.25))
        );
        assert_eq!(targets.ring_row_switch_ratio, Some(Target::max(11.0, 0.3)));
        assert_eq!(
            targets.middle_row_switch_balance,
            Some(Target::max(12.0, 0.4))
        );
        assert_eq!(
            targets.index_row_switch_balance,
            Some(Target::max(13.0, 0.5))
        );
        assert_eq!(
            targets.directional_outward_ratio,
            Some(Target::max(40.0, 1.25))
        );
    }
}
