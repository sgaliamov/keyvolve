use crate::evaluator::EMPTY_SLOT;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use std::path::PathBuf;

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

    /// Letters confined to the left hand (slots 0–14). Not mirrored, unlike `allowed`.
    /// Format: `["a", "s", "x"]`.
    #[serde(default)]
    pub left: FxHashSet<char>,

    /// Letters confined to the right hand (slots 15–29). Not mirrored, unlike `allowed`.
    /// Format: `["o", "e", "r"]`.
    #[serde(default)]
    pub right: FxHashSet<char>,

    /// Char pairs that must end up on the same hand (left-left or right-right).
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

/// Pre-computed lookups derived from [`OptimizationConfig`]; build once per run via [`OptimizationConfig::cache`].
#[derive(Debug, Clone)]
pub struct OptimizationCache {
    pub frozen_slots: FxHashSet<u8>,
    pub frozen_chars: FxHashSet<char>,
    pub same_side_partner: FxHashMap<char, char>,
}

impl OptimizationConfig {
    /// Check whether placing `ch` at `slot` is permitted.
    /// Letters with no `allowed` entry are unconstrained.
    /// Frozen chars always stay at their pinned slot, ignoring `allowed`/side constraints.
    /// `left`/`right` letters are confined to that hand (slots 0–14 / 15–29).
    pub fn is_slot_allowed(&self, ch: char, slot: u8) -> bool {
        if let Some(&frozen_slot) = self.frozen.get(&ch) {
            return slot == frozen_slot;
        }
        if self.left.contains(&ch) && slot >= 15 {
            return false;
        }
        if self.right.contains(&ch) && slot < 15 {
            return false;
        }

        self.allowed
            .get(&ch)
            .is_none_or(|slots| slots.contains(&slot))
    }

    /// True when every same-side pair present in `genome` sits on one hand.
    /// Pairs with a not-yet-placed char are skipped (mid-placement tolerance).
    /// Catches split pairs from foreign genomes injected under different constraints.
    pub fn same_side_satisfied(&self, genome: &[char]) -> bool {
        self.same_side.iter().all(|&[a, b]| {
            match (
                genome.iter().position(|&c| c == a),
                genome.iter().position(|&c| c == b),
            ) {
                (Some(ia), Some(ib)) => on_same_hand(ia as u8, ib as u8),
                _ => true,
            }
        })
    }

    /// True when every placed char sits on a permitted slot (frozen chars at their
    /// pins, constrained chars within `allowed`, nothing on blocked slots) AND every
    /// same-side pair occupies one hand.
    /// Guards against genomes from external sources (seed csv, dump) and starved
    /// fallback placements that were produced under or drifted from the constraints.
    pub fn is_genome_valid(&self, genome: &[char]) -> bool {
        genome.iter().enumerate().all(|(i, &ch)| {
            let slot = i as u8;
            // Frozen outranks blocked: a pin on a blocked slot is still valid.
            ch == EMPTY_SLOT
                || self.frozen.get(&ch) == Some(&slot)
                || (!self.blocked.contains(&slot) && self.is_slot_allowed(ch, slot))
        }) && self.same_side_satisfied(genome)
    }

    /// Pre-compute derived lookups that are hot in the generator loop.
    pub fn cache(&self) -> OptimizationCache {
        OptimizationCache {
            frozen_slots: self.frozen.values().copied().collect(),
            frozen_chars: self.frozen.keys().copied().collect(),
            same_side_partner: self
                .same_side
                .iter()
                .flat_map(|&[a, b]| [(a, b), (b, a)])
                .collect(),
        }
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
        .flat_map(|&i| {
            if i < 15 {
                vec![i, mirror_slot(i)]
            } else {
                vec![i]
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

/// True when two slots are on the same hand.
#[inline]
fn on_same_hand(a: u8, b: u8) -> bool {
    a / 15 == b / 15
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
            Ok([a, b])
        })
        .collect()
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
    fn genome_validity_rejects_wrong_side() {
        let mut cfg = OptimizationConfig::default();
        cfg.right.insert('o');
        let mut g = vec![EMPTY_SLOT; 30];
        g[20] = 'o';
        assert!(cfg.is_genome_valid(&g));
        g[20] = EMPTY_SLOT;
        g[5] = 'o'; // left hand → violates right pin
        assert!(!cfg.is_genome_valid(&g));
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
    fn genome_validity_checks_allowed_and_blocked() {
        let mut cfg = OptimizationConfig::default();
        cfg.allowed.insert('a', [0u8, 1].into_iter().collect());
        cfg.blocked.insert(29);

        let mut g = vec![EMPTY_SLOT; 30];
        g[0] = 'a';
        g[5] = 'b';
        assert!(cfg.is_genome_valid(&g));

        g[2] = 'a'; // second 'a' on disallowed slot
        assert!(!cfg.is_genome_valid(&g));

        g[2] = EMPTY_SLOT;
        g[29] = 'b'; // blocked slot occupied
        assert!(!cfg.is_genome_valid(&g));

        g[29] = EMPTY_SLOT; // empty blocked slot is fine
        assert!(cfg.is_genome_valid(&g));
    }

    #[test]
    fn frozen_pin_outranks_blocked() {
        let mut cfg = OptimizationConfig::default();
        cfg.frozen.insert('f', 29);
        cfg.blocked.insert(29);

        let mut g = vec![EMPTY_SLOT; 30];
        g[29] = 'f'; // pinned on blocked slot — frozen wins
        assert!(cfg.is_genome_valid(&g));

        g[29] = 'x'; // non-frozen char on blocked slot still invalid
        assert!(!cfg.is_genome_valid(&g));
    }

    #[test]
    fn same_side_satisfied_true_for_same_hand() {
        let cfg = OptimizationConfig {
            same_side: vec![['t', 'h']],
            ..Default::default()
        };
        let mut g = vec![EMPTY_SLOT; 30];
        g[3] = 't';
        g[4] = 'h';
        assert!(cfg.same_side_satisfied(&g));
        assert!(cfg.is_genome_valid(&g));
    }

    #[test]
    fn same_side_satisfied_false_for_cross_hand_pair() {
        let cfg = OptimizationConfig {
            same_side: vec![['t', 'h']],
            ..Default::default()
        };
        let mut g = vec![EMPTY_SLOT; 30];
        g[0] = 't';
        g[15] = 'h';
        assert!(!cfg.same_side_satisfied(&g));
        assert!(!cfg.is_genome_valid(&g)); // guard rejects split pair
    }

    #[test]
    fn same_side_satisfied_skips_absent_char() {
        let cfg = OptimizationConfig {
            same_side: vec![['t', 'h']],
            ..Default::default()
        };
        let mut g = vec![EMPTY_SLOT; 30];
        g[0] = 't'; // 'h' absent → nothing to violate yet
        assert!(cfg.same_side_satisfied(&g));
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
}
