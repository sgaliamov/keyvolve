use crate::app::OptimizationConfig;
use crate::app::{EMPTY_SLOT, GaContext, KeysGenome};
use rand::seq::SliceRandom;

/// Generate a genome for optimization, respecting frozen/blocked constraints.
pub fn generate(ctx: &GaContext) -> KeysGenome {
    let state = ctx.state.as_ref().expect("state must be set");
    let opt = &state.optimization;
    constrained_keys(opt)
}

/// Place frozen chars at fixed positions, shuffle the rest into remaining free slots.
/// Per-letter `allowed` constraints are respected; unconstrained letters fill the rest.
fn constrained_keys(opt: &OptimizationConfig) -> KeysGenome {
    let frozen = &opt.frozen;
    let blocked = &opt.blocked;
    let mut genome = vec![EMPTY_SLOT; 30];

    // Pin frozen keys.
    for (&ch, &idx) in frozen {
        genome[idx as usize] = ch;
    }

    let frozen_positions: rustc_hash::FxHashSet<u8> = frozen.values().copied().collect();
    let frozen_chars: rustc_hash::FxHashSet<char> = frozen.keys().copied().collect();

    // Remaining letters and positions.
    let mut letters: Vec<char> = ('a'..='z').filter(|c| !frozen_chars.contains(c)).collect();
    let mut free: Vec<u8> = (0u8..30)
        .filter(|i| !blocked.contains(i) && !frozen_positions.contains(i))
        .collect();

    letters.shuffle(&mut rand::rng());
    free.shuffle(&mut rand::rng());

    // Constrained letters first (has `allowed` entry), then unconstrained.
    // Ensures constrained letters get priority picking their valid slots.
    letters.sort_by_key(|c| if opt.allowed.contains_key(c) { 0u8 } else { 1 });

    // Assign each letter to its first valid free slot; fall back to any free slot.
    for ch in letters {
        let pos = free
            .iter()
            .position(|&s| opt.is_slot_valid(ch, s))
            .or(if free.is_empty() { None } else { Some(0) });

        if let Some(idx) = pos {
            genome[free[idx] as usize] = ch;
            free.swap_remove(idx);
        }
    }

    genome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::OptimizationConfig;
    use rustc_hash::FxHashSet;

    fn make_frozen(pairs: &[(char, u8)]) -> OptimizationConfig {
        OptimizationConfig {
            frozen: pairs.iter().copied().collect(),
            ..Default::default()
        }
    }

    #[test]
    fn unconstrained_has_all_26_chars() {
        let g = constrained_keys(&OptimizationConfig::default());
        assert_eq!(g.len(), 30);
        let mut chars: Vec<char> = g.iter().copied().filter(|&c| c != EMPTY_SLOT).collect();
        chars.sort_unstable();
        assert_eq!(chars, ('a'..='z').collect::<Vec<_>>());
    }

    #[test]
    fn frozen_keys_stay_in_place() {
        let opt = make_frozen(&[('a', 0), ('z', 29)]);
        let g = constrained_keys(&opt);
        assert_eq!(g[0], 'a');
        assert_eq!(g[29], 'z');
    }

    #[test]
    fn blocked_slots_are_empty() {
        let blocked: FxHashSet<u8> = [5, 6, 7, 8].iter().copied().collect();
        let opt = OptimizationConfig {
            blocked,
            ..Default::default()
        };
        let g = constrained_keys(&opt);
        for &b in &[5usize, 6, 7, 8] {
            assert_eq!(g[b], EMPTY_SLOT, "slot {b} should be empty");
        }
    }

    #[test]
    fn allowed_constraint_respected() {
        use crate::app::optimization::expand_half;
        let mut opt = OptimizationConfig::default();
        opt.allowed.insert('a', expand_half(&[0])); // 'a' only allowed at 0 or 19
        for _ in 0..20 {
            let g = constrained_keys(&opt);
            let pos = g.iter().position(|&c| c == 'a').unwrap();
            assert!(pos == 0 || pos == 19, "a landed at {pos}");
        }
    }
}
