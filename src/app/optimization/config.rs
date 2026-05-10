use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;

/// Mirror a left-hand slot (0–14) to its right-hand counterpart (15–29).
/// Layout:  left 0–14, right 15–29, 5 cols/row, 3 rows.
/// Formula: `(i / 5) * 5 + (4 - i % 5) + 15`
pub fn mirror_slot(i: u8) -> u8 {
    (i / 5) * 5 + (4 - i % 5) + 15
}

/// Expand a half-position set (0–14) to both hands (adds mirrored slots 15–29).
pub fn expand_half(slots: &[u8]) -> FxHashSet<u8> {
    slots
        .iter()
        .flat_map(|&i| {
            if i < 15 {
                [i, mirror_slot(i)].into_iter()
            } else {
                [i, i].into_iter() // already full-range; no-op dup, deduped by HashSet
            }
        })
        .collect()
}

/// Deserialize a `FxHashMap<char, FxHashSet<u8>>` where each value is a list of
/// half-positions (0–14) that are auto-mirrored to both hands.
fn de_letter_slot_map<'de, D>(de: D) -> Result<FxHashMap<char, FxHashSet<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: FxHashMap<char, Vec<u8>> = FxHashMap::deserialize(de)?;
    Ok(raw.into_iter().map(|(ch, slots)| (ch, expand_half(&slots))).collect())
}

/// Per-key constraints for optimization.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationConfig {
    /// Characters whose physical position is locked: maps char → key index (0-29).
    #[serde(default)]
    pub frozen: FxHashMap<char, u8>,

    /// Physical key indices (0-29) that are unavailable for placement.
    #[serde(default)]
    pub blocked: FxHashSet<u8>,

    /// Per-letter allowed slots (half-positions 0–14, auto-mirrored).
    /// `{ "a": [0,1,2], "e": [3,4] }` — letters not listed are unconstrained.
    #[serde(default, deserialize_with = "de_letter_slot_map")]
    pub allowed: FxHashMap<char, FxHashSet<u8>>,
}

impl OptimizationConfig {
    /// Check whether placing `ch` at `slot` is permitted.
    /// Letters with no `allowed` entry are unconstrained.
    pub fn is_slot_valid(&self, ch: char, slot: u8) -> bool {
        self.allowed.get(&ch).is_none_or(|slots| slots.contains(&slot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(cfg.is_slot_valid('a', 0));
        assert!(cfg.is_slot_valid('z', 29));
    }

    #[test]
    fn is_slot_valid_allowed() {
        let mut cfg = OptimizationConfig::default();
        cfg.allowed.insert('a', expand_half(&[0]));
        assert!(cfg.is_slot_valid('a', 0));
        assert!(cfg.is_slot_valid('a', 19)); // mirrored
        assert!(!cfg.is_slot_valid('a', 1));
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
}
