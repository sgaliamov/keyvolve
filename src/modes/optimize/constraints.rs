use crate::evaluator::EMPTY_SLOT;
use crate::modes::optimize::{OptimizationConfig, match_slots};
use miette::{Result, miette};

/// Physical slots, alphabet size, and masks used by the assignment engine.
pub const SLOT_COUNT: usize = 30;
pub const LETTER_COUNT: usize = 26;
pub const ALL_SLOTS: u32 = (1 << SLOT_COUNT) - 1;
pub const ALL_LETTERS: u32 = (1 << LETTER_COUNT) - 1;
pub const HANDS: [u32; 2] = [(1 << 15) - 1, ALL_SLOTS ^ ((1 << 15) - 1)];

/// Validated domains and feasible hand choices, shared by all placement operators.
#[derive(Debug)]
pub struct PlacementConstraints {
    // Only the optimizer's solver may access compiled internals.
    pub(super) domains: [u32; SLOT_COUNT],
    pub(super) groups: Vec<u32>,
    pub(super) orientations: Vec<u16>,
    pub(super) units: Vec<u32>,
}

impl PlacementConstraints {
    /// Compile effective domains and prove that at least one complete layout exists.
    pub fn new(opt: &OptimizationConfig) -> Result<Self> {
        opt.validate()?;
        let pins = opt.frozen.values().fold(0, |mask, &slot| mask | 1 << slot);
        let domains = std::array::from_fn(|token| {
            (0..SLOT_COUNT).fold(0, |mask, slot| {
                let permitted = if token < LETTER_COUNT {
                    let ch = letter(token);
                    opt.frozen.get(&ch) == Some(&(slot as u8))
                        || (pins & (1 << slot) == 0
                            && !opt.blocked.contains(&(slot as u8))
                            && opt.is_slot_allowed(ch, slot as u8))
                } else {
                    pins & (1 << slot) == 0 && opt.is_empty_slot_allowed(slot as u8)
                };
                mask | if permitted { 1 << slot } else { 0 }
            })
        });
        for (token, &domain) in domains.iter().enumerate() {
            if domain == 0 {
                let ch = if token < LETTER_COUNT {
                    letter(token)
                } else {
                    '_'
                };
                return Err(miette!("optimization leaves no legal slots for {ch:?}"));
            }
        }

        let groups = connected_groups(&opt.same_side);
        let frozen = opt
            .frozen
            .keys()
            .fold(0, |mask, &ch| mask | 1 << letter_index(ch).unwrap());
        let grouped = groups.iter().fold(0, |mask, group| mask | group);
        let units = groups
            .iter()
            .copied()
            .chain(bits(ALL_LETTERS & !grouped).map(|i| 1 << i))
            .map(|group| group & !frozen)
            .filter(|&group| group != 0)
            .collect();
        let mut constraints = Self {
            domains,
            groups,
            orientations: Vec::new(),
            units,
        };
        constraints.collect_orientations(0, 0, domains);
        if constraints.orientations.is_empty() {
            return Err(miette!(
                "optimization constraints have no complete layout: check frozen pins, blocked/allowed slots, \
                 hand restrictions and sameSide groups; all 26 letters and four empties need distinct slots"
            ));
        }
        Ok(constraints)
    }

    /// Validate a complete canonical genome, never a partially filled solver state.
    pub fn is_genome_valid(&self, genome: &[char]) -> bool {
        if genome.len() != SLOT_COUNT {
            return false;
        }
        let mut seen = 0;
        let mut left = 0;
        let mut empties = 0;
        for (slot, &ch) in genome.iter().enumerate() {
            if ch == EMPTY_SLOT {
                empties += 1;
                if self.domains[LETTER_COUNT] & (1 << slot) == 0 {
                    return false;
                }
            } else {
                let Some(token) = letter_index(ch) else {
                    return false;
                };
                let bit = 1 << token;
                if seen & bit != 0 || self.domains[token] & (1 << slot) == 0 {
                    return false;
                }
                seen |= bit;
                if slot < 15 {
                    left |= bit;
                }
            }
        }
        seen == ALL_LETTERS
            && empties == SLOT_COUNT - LETTER_COUNT
            && self
                .groups
                .iter()
                .all(|&group| group & left == 0 || group & left == group)
    }

    /// Restrict each connected group's letter domains to its selected hand.
    pub(super) fn oriented_domains(&self, orientation: u16) -> [u32; SLOT_COUNT] {
        let mut domains = self.domains;
        for (i, &group) in self.groups.iter().enumerate() {
            let hand = HANDS[usize::from((orientation >> i) & 1)];
            for token in bits(group) {
                domains[token] &= hand;
            }
        }
        domains
    }

    /// Enumerate only hand combinations that admit a complete slot matching.
    fn collect_orientations(&mut self, index: usize, orientation: u16, domains: [u32; SLOT_COUNT]) {
        let order = std::array::from_fn(|i| i);
        if match_slots(&domains, &[0; SLOT_COUNT], &order).is_none() {
            return;
        }
        let Some(&group) = self.groups.get(index) else {
            self.orientations.push(orientation);
            return;
        };
        for (hand, &mask) in HANDS.iter().enumerate() {
            let mut next = domains;
            for token in bits(group) {
                next[token] &= mask;
            }
            self.collect_orientations(index + 1, orientation | (hand as u16) << index, next);
        }
    }
}

/// Iterate set-bit indices without allocating a slot or character list.
pub fn bits(mut mask: u32) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        if mask == 0 {
            return None;
        }
        let index = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        Some(index)
    })
}

/// Alphabet token for a lowercase key; all other symbols are not letters.
pub fn letter_index(ch: char) -> Option<usize> {
    ch.is_ascii_lowercase().then(|| ch as usize - 'a' as usize)
}

/// Character for an alphabet token.
pub fn letter(token: usize) -> char {
    (b'a' + token as u8) as char
}

/// Merge overlapping pairs into disjoint multi-letter same-side components.
fn connected_groups(pairs: &[[char; 2]]) -> Vec<u32> {
    let mut groups = Vec::<u32>::new();
    for &[a, b] in pairs {
        let mut group = (1 << letter_index(a).unwrap()) | (1 << letter_index(b).unwrap());
        groups.retain(|&existing| {
            if existing & group == 0 {
                true
            } else {
                group |= existing;
                false
            }
        });
        groups.push(group);
    }
    groups.sort_unstable();
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_validation_rejects_missing_duplicate_unknown_and_wrong_length() {
        let constraints = OptimizationConfig::default().compile().unwrap();
        let valid: Vec<_> = ('a'..='z').chain([EMPTY_SLOT; 4]).collect();
        assert!(constraints.is_genome_valid(&valid));
        assert!(!constraints.is_genome_valid(&valid[..29]));
        assert!(!constraints.is_genome_valid(&[EMPTY_SLOT; 30]));
        for ch in [EMPTY_SLOT, 'b', '_', 'A', '?'] {
            let mut invalid = valid.clone();
            invalid[0] = ch;
            assert!(!constraints.is_genome_valid(&invalid));
        }
        let mut long = valid;
        long.push(EMPTY_SLOT);
        assert!(!constraints.is_genome_valid(&long));
    }

    #[test]
    fn impossible_domains_and_side_groups_fail_before_generation() {
        for json in [
            r#"{"blocked":[0,1,2,3,4]}"#,
            r#"{"allowed":{"_":[14]}}"#,
            r#"{"frozen":{"t":0,"h":15},"sameSide":["th"]}"#,
            r#"{"left":["t"],"right":["h"],"sameSide":["th"]}"#,
            r#"{"allowed":{"a":[0],"b":[0],"c":[0]}}"#,
            r#"{"left":["a","b","c","d","e","f","g","h","i","j","k","l","m","n","o","p"]}"#,
        ] {
            let opt: OptimizationConfig = serde_json::from_str(json).unwrap();
            assert!(opt.compile().is_err(), "{json}");
        }
    }

    #[test]
    fn pin_and_blocked_overrides_survive_compilation() {
        let opt: OptimizationConfig = serde_json::from_str(
            r#"{"blocked":[0,1,2,3,4],"frozen":{"a":0},"right":["a"],"allowed":{"a":[0],"_":[14]}}"#
        ).unwrap();
        let constraints = opt.compile().unwrap();
        assert_eq!(constraints.domains[0], 1);
        assert_eq!(
            constraints.domains[LETTER_COUNT],
            (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 14) | (1 << 25)
        );
        let mut rng = rand::rng();
        let genome = constraints.generate(&mut rng);
        assert_eq!(genome[0], 'a');
        assert!(genome[1..5].iter().all(|&ch| ch == EMPTY_SLOT));
        assert!(constraints.is_genome_valid(&genome));
    }

    #[test]
    fn overlapping_pairs_merge_but_disjoint_pairs_remain_independent() {
        let opt: OptimizationConfig =
            serde_json::from_str(r#"{"sameSide":["ab","cd","bc","ba","th"]}"#).unwrap();
        let constraints = opt.compile().unwrap();
        assert_eq!(constraints.groups.len(), 2);
        assert!(constraints.groups.contains(&0b1111));
        assert_eq!(constraints.orientations.len(), 4);
    }

    #[test]
    fn maximal_pair_count_prunes_infeasible_hand_capacities() {
        let opt = OptimizationConfig {
            same_side: (0..LETTER_COUNT)
                .step_by(2)
                .map(|i| [letter(i), letter(i + 1)])
                .collect(),
            ..Default::default()
        };
        let constraints = opt.compile().unwrap();
        // Thirteen pairs require six on one hand and seven on the other.
        assert_eq!(constraints.orientations.len(), 3432);
        let mut rng = rand::rng();
        for _ in 0..20 {
            assert!(constraints.is_genome_valid(&constraints.generate(&mut rng)));
        }
    }
}
