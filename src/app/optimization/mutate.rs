use crate::app::{GaContext, KeysGenome, KeysIndividual};

/// Mutate a candidate genome by swapping two random keys.
pub fn mutate(ind: &KeysIndividual, _: &GaContext) -> Option<KeysGenome> {
    let mut genome = ind.genome.clone();
    let mut rng = rand::rng();
    let swaps = rand::random_range(1u8..=5);
    for _ in 0..swaps {
        let idx = rand::seq::index::sample(&mut rng, genome.len(), 2);
        genome.swap(idx.index(0), idx.index(1));
    }
    Some(genome)
}
