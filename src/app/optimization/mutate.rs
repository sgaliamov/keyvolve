use crate::app::{EMPTY_SLOT, GaContext, KeysGenome, KeysIndividual};

/// Mutate a candidate genome by swapping two random non-frozen, non-empty key positions.
pub fn mutate(ind: &KeysIndividual, ctx: &GaContext) -> Option<KeysGenome> {
    let state = ctx.state.as_ref().expect("state must be set");
    let frozen_positions: rustc_hash::FxHashSet<u8> =
        state.optimization.frozen.values().copied().collect();
    let blocked = &state.optimization.blocked;

    // Collect indices that are free to swap: have a real letter and aren't frozen.
    let free: Vec<usize> = ind
        .genome
        .iter()
        .enumerate()
        .filter(|(i, c)| {
            **c != EMPTY_SLOT
                && !frozen_positions.contains(&(*i as u8))
                && !blocked.contains(&(*i as u8))
        })
        .map(|(i, _)| i)
        .collect();

    if free.len() < 2 {
        return None;
    }

    let mut genome = ind.genome.clone();
    let mut rng = rand::rng();
    let swaps = rand::random_range(1u8..=5);

    for _ in 0..swaps {
        let idx = rand::seq::index::sample(&mut rng, free.len(), 2);
        genome.swap(free[idx.index(0)], free[idx.index(1)]);
    }

    Some(genome)
}
