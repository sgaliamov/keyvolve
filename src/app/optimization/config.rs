use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use std::path::PathBuf;

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
    Ok(raw
        .into_iter()
        .map(|(ch, slots)| (ch, expand_half(&slots)))
        .collect())
}

/// Column index within a hand (0–4).
#[inline]
fn slot_col(slot: u8) -> u8 {
    slot % 5
}

/// Row index (0 = top, 2 = bottom).
#[inline]
fn slot_row(slot: u8) -> u8 {
    (slot % 15) / 5
}

/// True when two slots are on the same hand, 1–2 columns apart, and within one row of each other.
pub fn are_roll_neighbors(a: u8, b: u8) -> bool {
    let a_hand = a / 15;
    let b_hand = b / 15;
    let col_dist = slot_col(a).abs_diff(slot_col(b));
    a_hand == b_hand && matches!(col_dist, 1 | 2) && slot_row(a).abs_diff(slot_row(b)) <= 1
}

/// Deserialize `["th", "st"]` → `[[t,h],[s,t]]`.
fn de_rolls<'de, D>(de: D) -> Result<Vec<[char; 2]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Vec<String> = Vec::deserialize(de)?;
    raw.iter()
        .map(|s| {
            let mut cs = s.chars();
            let a = cs
                .next()
                .ok_or_else(|| serde::de::Error::custom("empty roll pair"))?;
            let b = cs
                .next()
                .ok_or_else(|| serde::de::Error::custom("roll pair needs 2 chars"))?;
            Ok([a, b])
        })
        .collect()
}

/// Per-key constraints for optimization.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationConfig {
    /// Multiplier applied to per-switch self-effort; `1.0` = no penalty.
    pub bigram_switch_penalty: f64,

    /// Max multiplier applied for extreme hand imbalance.
    pub balance_penalty: f64,

    /// Coefficient `k` for corpus-level alternation-rate penalty.
    pub alternation_penalty: f64,

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

    /// Char pairs that should occupy roll-neighbor slots (same hand, adjacent column, ±1 row).
    /// Defined as left-hand positions; right hand is symmetric. Both `[a,b]` and `[b,a]` checked.
    /// Format: `["th", "st"]`.
    #[serde(default, deserialize_with = "de_rolls")]
    pub rolls: Vec<[char; 2]>,

    /// Input layouts csv file, used as optimization seed.
    pub input: Option<PathBuf>,

    /// Output layouts csv file
    pub output: Option<PathBuf>,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            bigram_switch_penalty: 1.5,
            balance_penalty: 2.0,
            alternation_penalty: 0.25,
            frozen: Default::default(),
            blocked: Default::default(),
            allowed: Default::default(),
            rolls: Default::default(),
            input: None,
            output: None,
        }
    }
}

impl OptimizationConfig {
    /// Check whether placing `ch` at `slot` is permitted.
    /// Letters with no `allowed` entry are unconstrained.
    /// Frozen chars always stay at their pinned slot and ignore `allowed` constraints.
    pub fn is_slot_allowed(&self, ch: char, slot: u8) -> bool {
        if let Some(&frozen_slot) = self.frozen.get(&ch) {
            return slot == frozen_slot;
        }

        self.allowed
            .get(&ch)
            .is_none_or(|slots| slots.contains(&slot))
    }

    /// Pre-compute derived lookups that are hot in the generator loop.
    pub fn cache(&self) -> OptimizationCache {
        OptimizationCache {
            frozen_slots: self.frozen.values().copied().collect(),
            frozen_chars: self.frozen.keys().copied().collect(),
            roll_partner: self
                .rolls
                .iter()
                .flat_map(|&[a, b]| [(a, b), (b, a)])
                .collect(),
        }
    }
}

/// Pre-computed lookups derived from [`OptimizationConfig`]; build once per run via [`OptimizationConfig::cache`].
#[derive(Debug, Clone)]
pub struct OptimizationCache {
    pub frozen_slots: FxHashSet<u8>,
    pub frozen_chars: FxHashSet<char>,
    pub roll_partner: FxHashMap<char, char>,
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
        assert!(cfg.is_slot_allowed('a', 0));
        assert!(cfg.is_slot_allowed('z', 29));
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
    fn deserialize_allowed_map() {
        let json = r#"{"bigramSwitchPenalty": 1.5, "balancePenalty": 2.0, "alternationPenalty": 0.25, "allowed": {"a": [0, 4]}}"#;
        let cfg: OptimizationConfig = serde_json::from_str(json).unwrap();
        let a_slots = &cfg.allowed[&'a'];
        assert!(a_slots.contains(&0));
        assert!(a_slots.contains(&19)); // mirror of 0
        assert!(a_slots.contains(&4));
        assert!(a_slots.contains(&15)); // mirror of 4
    }

    #[test]
    fn are_roll_neighbors_adjacent_col_same_row() {
        // slots 3 and 4: same row 0, col dist 1 → neighbors
        assert!(are_roll_neighbors(3, 4));
        // slots 2 and 4: same row 0, col dist 2 → neighbors
        assert!(are_roll_neighbors(2, 4));
        // slots 4 and 4: same slot, col dist 0 → not neighbors
        assert!(!are_roll_neighbors(4, 4));
        // slots 0 and 4: same row 0, col dist 4 → not neighbors
        assert!(!are_roll_neighbors(0, 4));
    }

    #[test]
    fn are_roll_neighbors_adjacent_col_adjacent_row() {
        // slot 3 (row 0, col 3) and slot 9 (row 1, col 4) → neighbors
        assert!(are_roll_neighbors(3, 9));
        // slot 3 (row 0, col 3) and slot 10 (row 2, col 0) → not neighbors (2 rows apart)
        assert!(!are_roll_neighbors(3, 10));
    }

    #[test]
    fn are_roll_neighbors_cross_hand_rejected() {
        // slot 4 (left index) and slot 15 (right index) → different hands
        assert!(!are_roll_neighbors(4, 15));
    }

    #[test]
    fn deserialize_rolls() {
        let json = r#"{"bigramSwitchPenalty": 1.5, "balancePenalty": 2.0, "alternationPenalty": 0.25, "rolls": ["th", "st"]}"#;
        let cfg: OptimizationConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.rolls, vec![['t', 'h'], ['s', 't']]);
    }
}
