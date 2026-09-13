use crate::evaluator::EMPTY_SLOT;
use crate::modes::optimize::{
    ALL_LETTERS, ALL_SLOTS, LETTER_COUNT, PlacementConstraints, SLOT_COUNT, bits, letter,
    letter_index,
};
use rand::{Rng, RngExt, seq::SliceRandom};

const UNASSIGNED: usize = usize::MAX;
const MUTATION_ATTEMPTS: usize = 3;

impl PlacementConstraints {
    /// Generate a complete layout from a proven feasible hand combination.
    pub fn generate(&self, rng: &mut impl Rng) -> [char; SLOT_COUNT] {
        let orientation = self.orientations[rng.random_range(0..self.orientations.len())];
        let domains = self.oriented_domains(orientation);
        let mut order = std::array::from_fn(|i| i);
        order.shuffle(rng);
        let assignment = match_slots(&domains, &[0; SLOT_COUNT], &order)
            .expect("compiled hand combination must have a complete matching");
        to_genome(assignment)
    }

    /// Repair a stale layout, retaining legal old positions as preferences, not pins.
    pub fn repair(&self, genome: &[char], rng: &mut impl Rng) -> [char; SLOT_COUNT] {
        if self.is_genome_valid(genome) {
            return genome.try_into().expect("validated genome length");
        }
        let preferred = position_hints(genome);
        let orientation = self
            .orientations
            .iter()
            .copied()
            .max_by_key(|&orientation| {
                self.oriented_domains(orientation)
                    .iter()
                    .zip(preferred)
                    .filter(|(domain, old)| **domain & *old != 0)
                    .count()
            })
            .expect("compiled constraints have a feasible hand combination");
        let mut order = std::array::from_fn(|i| i);
        order.shuffle(rng);
        let assignment = match_slots(&self.oriented_domains(orientation), &preferred, &order)
            .expect("preferences cannot invalidate a feasible matching");
        to_genome(assignment)
    }

    /// Mutate whole movable groups and single letters; existing empties can move too.
    pub fn mutate(&self, genome: &[char; SLOT_COUNT], rng: &mut impl Rng) -> [char; SLOT_COUNT] {
        assert!(
            self.is_genome_valid(genome),
            "mutation requires a validated parent"
        );
        let positions = position_hints(genome);
        let mut units = [0; LETTER_COUNT];
        units[..self.units.len()].copy_from_slice(&self.units);
        for _ in 0..MUTATION_ATTEMPTS {
            units[..self.units.len()].shuffle(rng);
            let count = rng.random_range(2..=8).min(self.units.len());
            let released = units[..count].iter().fold(0, |mask, unit| mask | unit);
            let retained = ALL_LETTERS & !released;
            if let Some(candidate) = self.place_retaining(retained, &positions, rng)
                && candidate != *genome
            {
                return candidate;
            }
        }
        // Fully pinned or locally unique assignments are legal no-change mutations.
        *genome
    }

    /// Solve a local mutation with unselected letters fixed, and all blank tokens free.
    fn place_retaining(
        &self,
        retained: u32,
        positions: &[u32; SLOT_COUNT],
        rng: &mut impl Rng,
    ) -> Option<[char; SLOT_COUNT]> {
        let mut required = 0u16;
        let mut fixed = 0u16;
        for (i, &group) in self.groups.iter().enumerate() {
            if let Some(token) = bits(group & retained).next() {
                fixed |= 1 << i;
                if positions[token].trailing_zeros() >= 15 {
                    required |= 1 << i;
                }
            }
        }
        let mut order = std::array::from_fn(|i| i);
        order.shuffle(rng);
        let start = rng.random_range(0..self.orientations.len());
        for offset in 0..self.orientations.len() {
            let orientation = self.orientations[(start + offset) % self.orientations.len()];
            if orientation & fixed != required {
                continue;
            }
            let mut domains = self.oriented_domains(orientation);
            for token in bits(retained) {
                domains[token] &= positions[token];
            }
            if let Some(assignment) = match_slots(&domains, &[0; SLOT_COUNT], &order) {
                return Some(to_genome(assignment));
            }
        }
        None
    }
}

/// Match every token to one distinct legal slot; preferences never restrict domains.
pub fn match_slots(
    domains: &[u32; SLOT_COUNT],
    preferred: &[u32; SLOT_COUNT],
    order: &[usize; SLOT_COUNT],
) -> Option<[usize; SLOT_COUNT]> {
    let mut tokens = *order;
    tokens.sort_by_key(|&token| domains[token].count_ones());
    let mut matching = Matching {
        domains,
        preferred,
        order,
        owners: [UNASSIGNED; SLOT_COUNT],
        free: ALL_SLOTS,
        letters: 0,
    };
    for token in tokens {
        if !matching.augment(token, &mut 0) {
            return None;
        }
    }
    Some(matching.owners)
}

/// Stack-only matching state; masks avoid rescanning occupied slots on relocation.
struct Matching<'a> {
    domains: &'a [u32; SLOT_COUNT],
    preferred: &'a [u32; SLOT_COUNT],
    order: &'a [usize; SLOT_COUNT],
    owners: [usize; SLOT_COUNT],
    free: u32,
    letters: u32,
}

impl Matching<'_> {
    /// Prefer free slots, then relocate a chain if direct placement is impossible.
    fn augment(&mut self, token: usize, visited: &mut u32) -> bool {
        let contiguous = if token < LETTER_COUNT {
            contiguous_slots(self.letters)
        } else {
            0
        };
        for free_only in [true, false] {
            for preference in [self.preferred[token], contiguous, ALL_SLOTS] {
                let candidates = self.domains[token]
                    & preference
                    & !*visited
                    & if free_only { self.free } else { ALL_SLOTS };
                if candidates == 0 {
                    continue;
                }
                for &slot in self.order {
                    let bit = 1 << slot;
                    if candidates & bit == 0 || *visited & bit != 0 {
                        continue;
                    }
                    *visited |= bit;
                    let occupant = self.owners[slot];
                    if occupant == UNASSIGNED || self.augment(occupant, visited) {
                        self.owners[slot] = token;
                        self.free &= !bit;
                        self.letters = if token < LETTER_COUNT {
                            self.letters | bit
                        } else {
                            self.letters & !bit
                        };
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Prefer growing existing row clusters; gaps remain legal under hard constraints.
fn contiguous_slots(letters: u32) -> u32 {
    let mut mask = 0;
    for start in (0..SLOT_COUNT).step_by(5) {
        let row = (letters >> start) & 0b11111;
        if row == 0 {
            mask |= 0b11111 << start;
        } else {
            let first = row.trailing_zeros().saturating_sub(1);
            let last = (32 - row.leading_zeros()).min(4);
            mask |= ((1 << (last - first + 1)) - 1) << (start + first as usize);
        }
    }
    mask
}

/// Collect only recognizable position hints, accepting underscore at input boundaries.
fn position_hints(genome: &[char]) -> [u32; SLOT_COUNT] {
    let mut hints = [0; SLOT_COUNT];
    for (slot, &ch) in genome.iter().take(SLOT_COUNT).enumerate() {
        if let Some(token) = letter_index(ch) {
            hints[token] |= 1 << slot;
        } else if ch == EMPTY_SLOT || ch == '_' {
            for mask in &mut hints[LETTER_COUNT..] {
                *mask |= 1 << slot;
            }
        }
    }
    hints
}

/// Convert internal distinct blank tokens to the shared empty character.
fn to_genome(assignment: [usize; SLOT_COUNT]) -> [char; SLOT_COUNT] {
    assignment.map(|token| {
        if token < LETTER_COUNT {
            letter(token)
        } else {
            EMPTY_SLOT
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::optimize::OptimizationConfig;
    use rand::{SeedableRng, rngs::StdRng};
    use rustc_hash::FxHashSet;

    /// Compile a JSON fixture through the same normalization as user configuration.
    fn constraints(json: &str) -> PlacementConstraints {
        serde_json::from_str::<OptimizationConfig>(json)
            .unwrap()
            .compile()
            .unwrap()
    }

    /// Return the canonical complete alphabet layout.
    fn alphabet() -> [char; SLOT_COUNT] {
        std::array::from_fn(|i| {
            if i < LETTER_COUNT {
                letter(i)
            } else {
                EMPTY_SLOT
            }
        })
    }

    /// Find a known-present letter in a validated fixture.
    fn slot(genome: &[char], ch: char) -> usize {
        genome.iter().position(|&c| c == ch).unwrap()
    }

    #[test]
    fn empty_constraint_outranks_contiguity() {
        let mut opt = OptimizationConfig::default();
        opt.allowed
            .insert(EMPTY_SLOT, [1, 2, 3, 29].into_iter().collect());
        for (position, ch) in std::iter::once(0).chain(5..29).zip('a'..='y') {
            opt.frozen.insert(ch, position);
        }
        let constraints = opt.compile().unwrap();
        let mut rng = StdRng::seed_from_u64(1);
        let g = constraints.generate(&mut rng);
        assert_eq!(g[4], 'z');
        assert!(constraints.is_genome_valid(&g));
    }

    #[test]
    fn generation_and_mutation_keep_each_independent_pair_on_one_hand() {
        let constraints = constraints(r#"{"sameSide":["th","re"]}"#);
        let mut rng = StdRng::seed_from_u64(12);
        let mut orientations = FxHashSet::default();
        for _ in 0..100 {
            let mut g = constraints.generate(&mut rng);
            for _ in 0..20 {
                assert!(constraints.is_genome_valid(&g));
                orientations.insert((slot(&g, 't') / 15, slot(&g, 'r') / 15));
                g = constraints.mutate(&g, &mut rng);
            }
        }
        assert_eq!(orientations.len(), 4);
    }

    #[test]
    fn mutation_can_switch_independent_group_hands() {
        let constraints = constraints(r#"{"sameSide":["ab","cd"]}"#);
        let mut rng = StdRng::seed_from_u64(51);
        let mut g = alphabet();
        let mut orientations = FxHashSet::default();
        for _ in 0..500 {
            g = constraints.mutate(&g, &mut rng);
            assert!(constraints.is_genome_valid(&g));
            orientations.insert((slot(&g, 'a') / 15, slot(&g, 'c') / 15));
        }
        assert_eq!(orientations.len(), 4);
    }

    #[test]
    fn overlapping_pairs_and_frozen_anchors_survive_mutation_and_repair() {
        let constraints = constraints(
            r#"{"sameSide":["ab","cd","bc","ba","th"],"frozen":{"a":0,"t":19},"allowed":{"c":[1,2]}}"#,
        );
        let mut rng = StdRng::seed_from_u64(14);
        let mut g = constraints.repair(&alphabet(), &mut rng);
        for _ in 0..500 {
            assert!(constraints.is_genome_valid(&g));
            assert_eq!(g[0], 'a');
            assert_eq!(g[19], 't');
            assert!(slot(&g, 'h') >= 15);
            for ch in ['a', 'b', 'c', 'd'] {
                assert!(slot(&g, ch) < 15);
            }
            g = constraints.mutate(&g, &mut rng);
        }
    }

    #[test]
    fn mutation_moves_empties_without_breaking_their_allowed_domain() {
        let constraints =
            constraints(r#"{"allowed":{"_":[0,1,10,11,18,19,28,29]},"blocked":[29]}"#);
        let mut rng = StdRng::seed_from_u64(42);
        let mut g = constraints.generate(&mut rng);
        let mut patterns = FxHashSet::default();
        for _ in 0..200 {
            assert!(constraints.is_genome_valid(&g));
            assert_eq!(g[29], EMPTY_SLOT);
            patterns.insert(g.iter().enumerate().fold(0u32, |m, (i, &ch)| {
                m | if ch == EMPTY_SLOT { 1 << i } else { 0 }
            }));
            g = constraints.mutate(&g, &mut rng);
        }
        assert!(
            patterns.len() > 1,
            "mutation must explore different empty positions"
        );
    }

    #[test]
    fn repair_handles_split_pairs_forbidden_empties_and_malformed_genomes() {
        let constraints =
            constraints(r#"{"sameSide":["th","st"],"allowed":{"_":[0,10]},"frozen":{"t":5}}"#);
        let mut rng = StdRng::seed_from_u64(9);
        for input in [
            alphabet().to_vec(),
            vec!['x'; 30],
            vec!['_'; 30],
            vec!['?'; 30],
            vec![],
            vec!['a'; 31],
        ] {
            let g = constraints.repair(&input, &mut rng);
            assert!(constraints.is_genome_valid(&g));
            assert_eq!(g[5], 't');
        }
    }

    #[test]
    fn repair_keeps_valid_inputs_and_prefers_unaffected_positions() {
        let constraints = constraints(r#"{"allowed":{"a":[1]}}"#);
        let mut rng = StdRng::seed_from_u64(9);
        let g = constraints.repair(&alphabet(), &mut rng);
        assert_eq!(g[1], 'a');
        assert!(constraints.is_genome_valid(&g));
        let unchanged = g.iter().zip(alphabet()).filter(|(a, b)| **a == *b).count();
        assert!(
            unchanged >= 26,
            "repair should not reshuffle unrelated keys"
        );
        assert_eq!(constraints.repair(&g, &mut rng), g);
    }

    #[test]
    fn fully_frozen_layout_has_a_legal_no_change_mutation() {
        let opt = OptimizationConfig {
            frozen: ('a'..='z')
                .enumerate()
                .map(|(i, ch)| (ch, i as u8))
                .collect(),
            ..Default::default()
        };
        let constraints = opt.compile().unwrap();
        let mut rng = StdRng::seed_from_u64(3);
        assert_eq!(constraints.mutate(&alphabet(), &mut rng), alphabet());
    }

    #[test]
    fn current_configuration_survives_repeated_generation_and_mutation() {
        let constraints = constraints(
            r#"{"blocked":[29],"allowed":{"e":[6,7,8],"_":[0,1,10,11,18,19,28,29],"h":[1,2,3,6,7,8,9,12,13]},"sameSide":["er","th"]}"#,
        );
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..100 {
            let mut g = constraints.generate(&mut rng);
            for _ in 0..50 {
                assert!(constraints.is_genome_valid(&g));
                g = constraints.mutate(&g, &mut rng);
            }
        }
    }

    #[test]
    fn soft_contiguity_prefers_neighbors_but_does_not_require_gap_free_rows() {
        let mask = contiguous_slots(1 << 2);
        assert_eq!(mask & 0b11111, 0b01110);
        assert_eq!((mask >> 15) & 0b11111, 0b11111);
        let constraints = constraints(r#"{"frozen":{"a":0,"b":4},"allowed":{"_":[1,2]}}"#);
        let mut rng = StdRng::seed_from_u64(17);
        let g = constraints.generate(&mut rng);
        assert!(constraints.is_genome_valid(&g));
        assert_eq!(g[1], EMPTY_SLOT);
        assert_eq!(g[2], EMPTY_SLOT);
    }

    #[test]
    fn matching_finds_relocation_chains_and_matches_small_exhaustive_search() {
        let order = std::array::from_fn(|i| i);
        for a in 0u32..8 {
            for b in 0u32..8 {
                for c in 0u32..8 {
                    let mut domains = std::array::from_fn(|i| 1 << i);
                    domains[..3].copy_from_slice(&[a, b, c]);
                    let brute = (0..3).any(|i| {
                        (0..3).any(|j| {
                            (0..3).any(|k| {
                                i != j
                                    && i != k
                                    && j != k
                                    && a & (1 << i) != 0
                                    && b & (1 << j) != 0
                                    && c & (1 << k) != 0
                            })
                        })
                    });
                    let found = match_slots(&domains, &[0; SLOT_COUNT], &order);
                    assert_eq!(found.is_some(), brute, "{a:b}, {b:b}, {c:b}");
                    if let Some(owners) = found {
                        let mut seen = 0u32;
                        for (slot, token) in owners.into_iter().enumerate() {
                            assert_ne!(domains[token] & (1 << slot), 0);
                            assert_eq!(seen & (1 << token), 0);
                            seen |= 1 << token;
                        }
                        assert_eq!(seen, ALL_SLOTS);
                    }
                }
            }
        }
    }
}
