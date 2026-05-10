use crate::app::{EMPTY_SLOT, GaContext, KeysGenome, KeysIndividual};

/// Mutate a candidate genome by swapping two random non-empty key positions.
pub fn mutate(ind: &KeysIndividual, _ctx: &GaContext) -> Option<KeysGenome> {
    // Collect all occupied indices; all swaps allowed.
    let free: Vec<usize> = ind
        .genome
        .iter()
        .enumerate()
        .filter(|(_, c)| **c != EMPTY_SLOT)
        .map(|(i, _)| i)
        .collect();

    let mut genome = ind.genome.clone();
    let mut rng = rand::rng();
    let swaps = rand::random_range(1u8..=5);

    for _ in 0..swaps {
        let idx = rand::seq::index::sample(&mut rng, free.len(), 2);
        genome.swap(free[idx.index(0)], free[idx.index(1)]);
    }

    Some(genome)
}
