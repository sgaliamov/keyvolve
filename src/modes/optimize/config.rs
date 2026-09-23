use crate::evaluator::EMPTY_SLOT;
use crate::modes::optimize::PlacementConstraints;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use std::path::PathBuf;

/// Per-key constraints for optimization.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptimizationConfig {
    /// Characters whose physical position is locked: maps char → key index (0-29).
    #[serde(default)]
    pub frozen: FxHashMap<char, u8>,

    /// Physical key indices (0-29) that are unavailable for placement.
    #[serde(default)]
    pub blocked: FxHashSet<u8>,

    /// Per-letter allowed slots (half-positions 0–14, auto-mirrored).
    /// `{ "a": [0,1,2], "e": [3,4], "_": [10,11,12,13] }`.
    /// `"_"` is normalized to `EMPTY_SLOT`.
    #[serde(default, deserialize_with = "de_letter_slot_map")]
    pub allowed: FxHashMap<char, FxHashSet<u8>>,

    /// Letters confined to the left hand (slots 0–14). Not mirrored, unlike `allowed`.
    /// Format: `["a", "s", "x"]`.
    #[serde(default)]
    pub left: FxHashSet<char>,

    /// Letters confined to the right hand (slots 15–29). Not mirrored, unlike `allowed`.
    /// Format: `["o", "e", "r"]`.
    #[serde(default)]
    pub right: FxHashSet<char>,

    /// Each pair shares a hand; disjoint pairs choose independently, overlaps form groups.
    /// Format: `["th", "st"]`.
    #[serde(default, deserialize_with = "de_same_side_pairs")]
    pub same_side: Vec<[char; 2]>,

    /// Number of independent mutants produced per parent per generation. Default: 10.
    #[serde(default = "default_mutation_count")]
    pub mutation_count: usize,

    /// Max home-row groups kept in final output. Default: 10.
    #[serde(default = "default_max_groups")]
    pub max_groups: usize,

    /// Number of layouts kept per home-row group in final output. Default: 6.
    #[serde(default = "default_items_per_group")]
    pub items_per_group: usize,

    /// Input layouts csv file, used as optimization seed.
    pub input: Option<PathBuf>,

    /// Output layouts csv file
    pub output: Option<PathBuf>,
}

impl OptimizationConfig {
    /// Validate and compile placement constraints for optimization.
    pub fn compile(&self) -> miette::Result<PlacementConstraints> {
        PlacementConstraints::new(self)
    }

    /// Validate raw constraint values without rejecting overridden restrictions.
    pub fn validate(&self) -> miette::Result<()> {
        let mut frozen_slots = FxHashSet::default();
        for (&ch, &slot) in &self.frozen {
            miette::ensure!(
                ch.is_ascii_lowercase(),
                "frozen key {ch:?} must be a lowercase letter a-z"
            );
            miette::ensure!(slot < 30, "frozen slot {slot} must be in 0..29");
            miette::ensure!(
                frozen_slots.insert(slot),
                "multiple frozen keys use slot {slot}"
            );
        }
        for &slot in &self.blocked {
            miette::ensure!(slot < 30, "blocked slot {slot} must be in 0..29");
        }
        for (&ch, slots) in &self.allowed {
            let key = normalize_allowed_key(ch);
            miette::ensure!(
                key.is_ascii_lowercase() || key == EMPTY_SLOT,
                "allowed key {ch:?} must be a lowercase letter a-z, '_' or EMPTY_SLOT"
            );
            for &slot in slots {
                miette::ensure!(slot < 30, "allowed slot {slot} must be in 0..29");
            }
        }
        for (side, letters) in [("left", &self.left), ("right", &self.right)] {
            for &ch in letters {
                miette::ensure!(
                    ch.is_ascii_lowercase(),
                    "{side} key {ch:?} must be a lowercase letter a-z"
                );
            }
        }
        for &[a, b] in &self.same_side {
            miette::ensure!(
                a.is_ascii_lowercase() && b.is_ascii_lowercase() && a != b,
                "same-side pair {a:?}, {b:?} must contain two distinct lowercase letters a-z"
            );
        }
        Ok(())
    }

    /// Check whether placing `ch` at `slot` is permitted.
    /// Letters with no `allowed` entry are unconstrained.
    /// Frozen chars always stay at their pinned slot, ignoring `allowed`/side constraints.
    /// `left`/`right` letters are confined to that hand (slots 0–14 / 15–29).
    pub fn is_slot_allowed(&self, ch: char, slot: u8) -> bool {
        if slot >= 30 {
            return false;
        }
        let ch = normalize_allowed_key(ch);
        if let Some(&frozen_slot) = self.frozen.get(&ch) {
            return slot == frozen_slot;
        }
        if self.left.contains(&ch) && slot >= 15 {
            return false;
        }
        if self.right.contains(&ch) && slot < 15 {
            return false;
        }

        self.allowed_slots_contain(ch, slot)
    }

    /// Check whether `EMPTY_SLOT` is permitted at `slot`.
    /// Blocked slots are always valid empties.
    pub fn is_empty_slot_allowed(&self, slot: u8) -> bool {
        slot < 30 && (self.blocked.contains(&slot) || self.allowed_slots_contain(EMPTY_SLOT, slot))
    }

    /// Match allowed slots, combining both spellings of the empty-slot key.
    fn allowed_slots_contain(&self, ch: char, slot: u8) -> bool {
        let slots = self.allowed.get(&ch);
        let alias = (ch == EMPTY_SLOT).then(|| self.allowed.get(&'_')).flatten();
        (slots.is_none() && alias.is_none())
            || slots
                .into_iter()
                .chain(alias)
                .any(|slots| slots.contains(&slot))
    }
}

fn default_mutation_count() -> usize {
    10
}

fn default_max_groups() -> usize {
    10
}

fn default_items_per_group() -> usize {
    6
}

/// Mirror a left-hand slot (0–14) to its right-hand counterpart (15–29).
/// Layout:  left 0–14, right 15–29, 5 cols/row, 3 rows.
/// Formula: `(i / 5) * 5 + (4 - i % 5) + 15`
fn mirror_slot(i: u8) -> u8 {
    (i / 5) * 5 + (4 - i % 5) + 15
}

/// Expand a half-position set (0–14) to both hands (adds mirrored slots 15–29).
fn expand_half(slots: &[u8]) -> FxHashSet<u8> {
    slots
        .iter()
        .flat_map(|&i| [Some(i), (i < 15).then(|| mirror_slot(i))])
        .flatten()
        .collect()
}

/// Deserialize a `FxHashMap<char, FxHashSet<u8>>` where each value is a list of
/// half-positions (0–14) that are auto-mirrored to both hands. Empty slots also
/// accept full physical coordinates; values outside 0..29 are rejected.
fn de_letter_slot_map<'de, D>(de: D) -> Result<FxHashMap<char, FxHashSet<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: FxHashMap<char, Vec<u8>> = FxHashMap::deserialize(de)?;
    let mut out = FxHashMap::default();
    for (ch, slots) in raw {
        let key = normalize_allowed_key(ch);
        if let Some(slot) = slots.iter().find(|&&slot| slot >= 30) {
            return Err(serde::de::Error::custom(format!(
                "allowed slot {slot} must be in 0..29"
            )));
        }
        if key != EMPTY_SLOT
            && let Some(slot) = slots.iter().find(|&&slot| slot >= 15)
        {
            return Err(serde::de::Error::custom(format!(
                "allowed slot {slot} must be in 0..14 for letter keys"
            )));
        }
        out.entry(key)
            .or_insert_with(FxHashSet::default)
            .extend(expand_half(&slots));
    }
    Ok(out)
}

/// Normalize the user-facing empty-slot alias.
#[inline]
fn normalize_allowed_key(ch: char) -> char {
    if ch == '_' { EMPTY_SLOT } else { ch }
}

/// Deserialize `["th", "st"]` → `[[t,h],[s,t]]`.
fn de_same_side_pairs<'de, D>(de: D) -> Result<Vec<[char; 2]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Vec<String> = Vec::deserialize(de)?;
    raw.iter()
        .map(|s| {
            let mut cs = s.chars();
            let a = cs
                .next()
                .ok_or_else(|| serde::de::Error::custom("empty same-side pair"))?;
            let b = cs
                .next()
                .ok_or_else(|| serde::de::Error::custom("same-side pair needs 2 chars"))?;
            if cs.next().is_some() || !a.is_ascii_lowercase() || !b.is_ascii_lowercase() || a == b {
                return Err(serde::de::Error::custom(
                    "same-side pair must contain exactly two distinct lowercase letters a-z",
                ));
            }
            Ok([a, b])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_validation_rejects_missing_letters() {
        assert!(
            !OptimizationConfig::default()
                .compile()
                .unwrap()
                .is_genome_valid(&[EMPTY_SLOT; 30])
        );
    }

    #[test]
    fn pair_parser_rejects_extra_characters() {
        assert!(serde_json::from_str::<OptimizationConfig>(r#"{"sameSide":["the"]}"#).is_err());
    }

    #[test]
    fn obsolete_rolls_setting_is_rejected() {
        assert!(serde_json::from_str::<OptimizationConfig>(r#"{"rolls":["th"]}"#).is_err());
    }

    #[test]
    fn unknown_optimization_setting_is_rejected() {
        assert!(serde_json::from_str::<OptimizationConfig>(r#"{"same_side":["th"]}"#).is_err());
    }

    #[test]
    fn validation_accepts_boundary_slots_and_empty_aliases() {
        let cfg = OptimizationConfig {
            frozen: [('a', 0), ('z', 29)].into_iter().collect(),
            blocked: [0, 29].into_iter().collect(),
            allowed: [
                ('a', [0, 14].into_iter().collect()),
                ('z', [0, 14].into_iter().collect()),
                ('_', [0, 29].into_iter().collect()),
                (EMPTY_SLOT, [0, 29].into_iter().collect()),
            ]
            .into_iter()
            .collect(),
            left: ['a'].into_iter().collect(),
            right: ['z'].into_iter().collect(),
            same_side: vec![['a', 'z']],
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validation_rejects_out_of_range_slots() {
        for slot in [30, u8::MAX] {
            let configs = [
                OptimizationConfig {
                    frozen: [('a', slot)].into_iter().collect(),
                    ..Default::default()
                },
                OptimizationConfig {
                    blocked: [slot].into_iter().collect(),
                    ..Default::default()
                },
                OptimizationConfig {
                    allowed: [('a', [slot].into_iter().collect())].into_iter().collect(),
                    ..Default::default()
                },
            ];
            for cfg in configs {
                assert!(cfg.validate().is_err(), "{cfg:?}");
            }
        }
    }

    #[test]
    fn validation_rejects_duplicate_frozen_slots() {
        let cfg = OptimizationConfig {
            frozen: [('a', 0), ('b', 0)].into_iter().collect(),
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
        assert!(cfg.compile().is_err());
    }

    #[test]
    fn validation_rejects_nonletters_in_frozen_and_sides() {
        for ch in ['A', '1', '_', EMPTY_SLOT, 'é'] {
            let configs = [
                OptimizationConfig {
                    frozen: [(ch, 0)].into_iter().collect(),
                    ..Default::default()
                },
                OptimizationConfig {
                    left: [ch].into_iter().collect(),
                    ..Default::default()
                },
                OptimizationConfig {
                    right: [ch].into_iter().collect(),
                    ..Default::default()
                },
            ];
            for cfg in configs {
                assert!(cfg.validate().is_err(), "{cfg:?}");
            }
        }
    }

    #[test]
    fn validation_rejects_invalid_allowed_keys() {
        for ch in ['A', '1', ' ', 'é'] {
            let cfg = OptimizationConfig {
                allowed: [(ch, [0].into_iter().collect())].into_iter().collect(),
                ..Default::default()
            };
            assert!(cfg.validate().is_err(), "{ch:?}");
        }
    }

    #[test]
    fn validation_accepts_overridden_and_globally_conflicting_constraints() {
        let cfg = OptimizationConfig {
            frozen: [('a', 29)].into_iter().collect(),
            blocked: (0..30).collect(),
            allowed: [('a', FxHashSet::default()), ('b', FxHashSet::default())]
                .into_iter()
                .collect(),
            left: ['a', 'b'].into_iter().collect(),
            right: ['a', 'b'].into_iter().collect(),
            ..Default::default()
        };
        assert!(cfg.validate().is_ok());
        assert!(cfg.is_slot_allowed('a', 29));
    }

    #[test]
    fn validation_rejects_malformed_same_side_pairs() {
        for pair in [['a', 'a'], ['A', 'b'], ['a', '_'], ['a', 'é'], ['`', 'a']] {
            let cfg = OptimizationConfig {
                same_side: vec![pair],
                ..Default::default()
            };
            assert!(cfg.validate().is_err(), "{pair:?}");
        }
    }

    #[test]
    fn duplicate_and_reversed_pairs_are_valid() {
        let cfg: OptimizationConfig =
            serde_json::from_str(r#"{"sameSide":["th","ht","th"]}"#).unwrap();
        assert_eq!(cfg.same_side, vec![['t', 'h'], ['h', 't'], ['t', 'h']]);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn pair_parser_rejects_malformed_pairs() {
        for pair in ["", "t", "tt", "Th", "té", "_h", "`h", "th "] {
            let json = serde_json::json!({ "sameSide": [pair] });
            assert!(
                serde_json::from_value::<OptimizationConfig>(json).is_err(),
                "{pair:?}"
            );
        }
        for value in [
            serde_json::json!(null),
            serde_json::json!(["t", "h"]),
            serde_json::json!(7),
        ] {
            let json = serde_json::json!({ "sameSide": [value] });
            assert!(serde_json::from_value::<OptimizationConfig>(json).is_err());
        }
    }

    #[test]
    fn allowed_parser_rejects_out_of_range_slots_before_expansion() {
        for slot in [30, 255] {
            for ch in ["a", "_", "`"] {
                let json = serde_json::json!({ "allowed": { ch: [slot] } });
                assert!(
                    serde_json::from_value::<OptimizationConfig>(json).is_err(),
                    "{ch:?}: {slot}"
                );
            }
        }
        for slot in [15, 29] {
            let json = serde_json::json!({ "allowed": { "a": [slot] } });
            assert!(
                serde_json::from_value::<OptimizationConfig>(json).is_err(),
                "a: {slot}"
            );
            let empty_json = serde_json::json!({ "allowed": { "_": [slot] } });
            assert!(
                serde_json::from_value::<OptimizationConfig>(empty_json).is_ok(),
                "_: {slot}"
            );
        }
    }

    #[test]
    fn slot_parsers_reject_invalid_numbers() {
        for slot in [-1, 256] {
            let inputs = [
                serde_json::json!({ "frozen": { "a": slot } }),
                serde_json::json!({ "blocked": [slot] }),
                serde_json::json!({ "allowed": { "a": [slot] } }),
            ];
            for json in inputs {
                assert!(serde_json::from_value::<OptimizationConfig>(json).is_err());
            }
        }
    }

    #[test]
    fn mirror_slot_maps_correctly() {
        assert_eq!(mirror_slot(0), 19);
        assert_eq!(mirror_slot(4), 15);
        assert_eq!(mirror_slot(7), 22);
    }

    #[test]
    fn expand_half_adds_mirrors() {
        let slots = expand_half(&[0, 4]);
        assert!(slots.contains(&0));
        assert!(slots.contains(&19));
        assert!(slots.contains(&4));
        assert!(slots.contains(&15));
    }

    #[test]
    fn is_slot_valid_no_constraint() {
        let cfg = OptimizationConfig::default();
        assert!(cfg.is_slot_allowed('a', 0));
        assert!(cfg.is_slot_allowed('z', 29));
        assert!(cfg.is_empty_slot_allowed(0));
    }

    #[test]
    fn slot_helpers_reject_out_of_bounds_even_with_overrides() {
        for slot in [30, u8::MAX] {
            let mut cfg = OptimizationConfig::default();
            assert!(!cfg.is_slot_allowed('a', slot));
            assert!(!cfg.is_empty_slot_allowed(slot));
            cfg.frozen.insert('a', slot);
            cfg.blocked.insert(slot);
            cfg.allowed.insert(EMPTY_SLOT, [slot].into_iter().collect());
            assert!(!cfg.is_slot_allowed('a', slot));
            assert!(!cfg.is_slot_allowed('_', slot));
            assert!(!cfg.is_empty_slot_allowed(slot));
        }
    }

    #[test]
    fn empty_slot_helpers_normalize_direct_aliases() {
        for key in ['_', EMPTY_SLOT] {
            let cfg = OptimizationConfig {
                allowed: [(key, [29].into_iter().collect())].into_iter().collect(),
                ..Default::default()
            };
            for ch in ['_', EMPTY_SLOT] {
                assert!(cfg.is_slot_allowed(ch, 29));
                assert!(!cfg.is_slot_allowed(ch, 0));
            }
            assert!(cfg.is_empty_slot_allowed(29));
            assert!(!cfg.is_empty_slot_allowed(0));
        }
    }

    #[test]
    fn empty_slot_helpers_union_direct_aliases() {
        let cfg = OptimizationConfig {
            allowed: [
                ('_', [0].into_iter().collect()),
                (EMPTY_SLOT, [29].into_iter().collect()),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        for slot in [0, 29] {
            assert!(cfg.is_slot_allowed('_', slot));
            assert!(cfg.is_slot_allowed(EMPTY_SLOT, slot));
            assert!(cfg.is_empty_slot_allowed(slot));
        }
        assert!(!cfg.is_slot_allowed('_', 1));
        assert!(!cfg.is_empty_slot_allowed(1));
    }

    #[test]
    fn blocked_slots_override_empty_allowed_slots() {
        for key in ['_', EMPTY_SLOT] {
            let cfg = OptimizationConfig {
                blocked: [0].into_iter().collect(),
                allowed: [(key, FxHashSet::default())].into_iter().collect(),
                ..Default::default()
            };
            assert!(cfg.is_empty_slot_allowed(0));
            assert!(!cfg.is_empty_slot_allowed(1));
        }
    }

    #[test]
    fn is_slot_valid_allowed() {
        let mut cfg = OptimizationConfig::default();
        cfg.allowed.insert('a', expand_half(&[0]));
        assert!(cfg.is_slot_allowed('a', 0));
        assert!(cfg.is_slot_allowed('a', 19)); // mirrored
        assert!(!cfg.is_slot_allowed('a', 1));
    }

    #[test]
    fn is_slot_valid_frozen_ignores_allowed() {
        let mut cfg = OptimizationConfig::default();
        cfg.frozen.insert('a', 4);
        cfg.allowed.insert('a', expand_half(&[0]));

        assert!(cfg.is_slot_allowed('a', 4));
        assert!(!cfg.is_slot_allowed('a', 0));
        assert!(!cfg.is_slot_allowed('a', 19));
    }

    #[test]
    fn is_slot_valid_left_side_confines_to_left() {
        let mut cfg = OptimizationConfig::default();
        cfg.left.insert('a');
        assert!(cfg.is_slot_allowed('a', 0));
        assert!(cfg.is_slot_allowed('a', 14));
        assert!(!cfg.is_slot_allowed('a', 15));
        assert!(!cfg.is_slot_allowed('a', 29));
    }

    #[test]
    fn is_slot_valid_right_side_confines_to_right() {
        let mut cfg = OptimizationConfig::default();
        cfg.right.insert('o');
        assert!(cfg.is_slot_allowed('o', 15));
        assert!(cfg.is_slot_allowed('o', 29));
        assert!(!cfg.is_slot_allowed('o', 0));
        assert!(!cfg.is_slot_allowed('o', 14));
    }

    #[test]
    fn is_slot_valid_side_intersects_allowed() {
        // 'a' allowed at 0 & 19 (mirror), but pinned left → only slot 0 survives.
        let mut cfg = OptimizationConfig::default();
        cfg.left.insert('a');
        cfg.allowed.insert('a', expand_half(&[0]));
        assert!(cfg.is_slot_allowed('a', 0));
        assert!(!cfg.is_slot_allowed('a', 19)); // allowed slot, wrong hand
    }

    #[test]
    fn is_slot_valid_frozen_overrides_side() {
        // frozen pin wins even when side says otherwise.
        let mut cfg = OptimizationConfig::default();
        cfg.left.insert('a');
        cfg.frozen.insert('a', 20); // right hand
        assert!(cfg.is_slot_allowed('a', 20));
        assert!(!cfg.is_slot_allowed('a', 0));
    }

    #[test]
    fn deserialize_side_maps() {
        let json = r#"{"left": ["a","s"], "right": ["o","e"]}"#;
        let cfg: OptimizationConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.left.contains(&'a'));
        assert!(cfg.left.contains(&'s'));
        assert!(cfg.right.contains(&'o'));
        assert!(cfg.right.contains(&'e'));
    }

    #[test]
    fn frozen_pin_outranks_blocked() {
        let mut cfg = OptimizationConfig::default();
        cfg.frozen.insert('f', 29);
        cfg.blocked.insert(29);

        assert!(cfg.is_slot_allowed('f', 29));
        assert!(!cfg.is_slot_allowed('f', 0));
    }

    #[test]
    fn deserialize_allowed_map() {
        let json = r#"{"allowed": {"a": [0, 4]}}"#;
        let cfg: OptimizationConfig = serde_json::from_str(json).unwrap();
        let a_slots = &cfg.allowed[&'a'];
        assert!(a_slots.contains(&0));
        assert!(a_slots.contains(&19)); // mirror of 0
        assert!(a_slots.contains(&4));
        assert!(a_slots.contains(&15)); // mirror of 4
    }

    #[test]
    fn deserialize_allowed_map_normalizes_underscore_to_empty_slot() {
        let json = r#"{"allowed": {"_": [0, 4]}}"#;
        let cfg: OptimizationConfig = serde_json::from_str(json).unwrap();
        let empty_slots = &cfg.allowed[&EMPTY_SLOT];
        assert!(empty_slots.contains(&0));
        assert!(empty_slots.contains(&19));
        assert!(empty_slots.contains(&4));
        assert!(empty_slots.contains(&15));
        assert!(!empty_slots.contains(&14));
        assert_eq!(
            cfg.allowed[&EMPTY_SLOT],
            [0, 19, 4, 15].into_iter().collect()
        );
    }

    #[test]
    fn deserialize_allowed_map_unions_empty_aliases() {
        let cfg: OptimizationConfig =
            serde_json::from_str(r#"{"allowed":{"_": [0], "`": [14]}}"#).unwrap();
        assert!(!cfg.allowed.contains_key(&'_'));
        assert_eq!(
            cfg.allowed[&EMPTY_SLOT],
            [0, 19, 14, 25].into_iter().collect()
        );
    }

    #[test]
    fn deserialize_allowed_map_rejects_right_hand_indices() {
        for slot in [15, 20, 29] {
            let json = serde_json::json!({ "allowed": { "a": [slot] } });
            assert!(serde_json::from_value::<OptimizationConfig>(json).is_err());
        }
    }

    #[test]
    fn empty_slot_helper_checks_allowed() {
        let mut cfg = OptimizationConfig::default();
        cfg.allowed
            .insert(EMPTY_SLOT, [26u8, 27, 28, 29].into_iter().collect());
        assert!((26..30).all(|slot| cfg.is_empty_slot_allowed(slot)));
        assert!(!cfg.is_empty_slot_allowed(25));
    }

    #[test]
    fn deserialize_same_side() {
        let json = r#"{"sameSide": ["th", "st"]}"#;
        let cfg: OptimizationConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.same_side, vec![['t', 'h'], ['s', 't']]);
    }

    #[test]
    fn deserialize_items_per_group() {
        let json = r#"{"itemsPerGroup": 3}"#;
        let cfg: OptimizationConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.items_per_group, 3);
    }

    #[test]
    fn numeric_defaults_are_unchanged() {
        let defaults = OptimizationConfig::default();
        assert_eq!(defaults.mutation_count, 0);
        assert_eq!(defaults.max_groups, 0);
        assert_eq!(defaults.items_per_group, 0);

        let parsed: OptimizationConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed.mutation_count, 10);
        assert_eq!(parsed.max_groups, 10);
        assert_eq!(parsed.items_per_group, 6);
        assert!(parsed.validate().is_ok());
    }
}
