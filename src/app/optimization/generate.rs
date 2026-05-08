use crate::app::{EMPTY_SLOT, GaContext, KeysGenome};
use rand::seq::SliceRandom;

/// Generate a genome for optimization, respecting frozen/blocked constraints.
pub fn generate(ctx: &GaContext) -> KeysGenome {
    let state = ctx.state.as_ref().expect("state must be set");
    let opt = &state.optimization;
    constrained_keys(&opt.frozen, &opt.blocked)
}

/// Place frozen chars at fixed positions, shuffle the rest into remaining free slots.
fn constrained_keys(
    frozen: &rustc_hash::FxHashMap<char, u8>,
    blocked: &rustc_hash::FxHashSet<u8>,
) -> KeysGenome {
    let mut genome = vec![EMPTY_SLOT; 30];

    // Pin frozen keys.
    for (&ch, &idx) in frozen {
        genome[idx as usize] = ch;
    }

    let frozen_positions: rustc_hash::FxHashSet<u8> = frozen.values().copied().collect();
    let frozen_chars: rustc_hash::FxHashSet<char> = frozen.keys().copied().collect();

    // Remaining letters and positions.
    let mut letters: Vec<char> = ('a'..='z')
        .filter(|c| !frozen_chars.contains(c))
        .collect();

    let mut free_positions: Vec<usize> = (0u8..30)
        .filter(|i| !blocked.contains(i) && !frozen_positions.contains(i))
        .map(|i| i as usize)
        .collect();

    letters.shuffle(&mut rand::rng());
    free_positions.shuffle(&mut rand::rng());

    // Fill free positions: letters first, then EMPTY_SLOT for the rest.
    for (pos, ch) in free_positions.iter().zip(
        letters.iter().copied().chain(std::iter::repeat(EMPTY_SLOT)),
    ) {
        genome[*pos] = ch;
    }

    genome
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hash::{FxHashMap, FxHashSet};

    fn make_frozen(pairs: &[(char, u8)]) -> FxHashMap<char, u8> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn unconstrained_has_all_26_chars() {
        let g = constrained_keys(&FxHashMap::default(), &FxHashSet::default());
        assert_eq!(g.len(), 30);
        let mut chars: Vec<char> = g.iter().copied().filter(|&c| c != EMPTY_SLOT).collect();
        chars.sort_unstable();
        assert_eq!(chars, ('a'..='z').collect::<Vec<_>>());
    }

    #[test]
    fn frozen_keys_stay_in_place() {
        let frozen = make_frozen(&[('a', 0), ('z', 29)]);
        let g = constrained_keys(&frozen, &FxHashSet::default());
        assert_eq!(g[0], 'a');
        assert_eq!(g[29], 'z');
    }

    #[test]
    fn blocked_slots_are_empty() {
        let blocked: FxHashSet<u8> = [5, 6, 7, 8].iter().copied().collect();
        let g = constrained_keys(&FxHashMap::default(), &blocked);
        for &b in &[5usize, 6, 7, 8] {
            assert_eq!(g[b], EMPTY_SLOT, "slot {b} should be empty");
        }
    }
}
