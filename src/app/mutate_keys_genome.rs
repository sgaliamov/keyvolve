use crate::models::{GaContext, KeysGenome, KeysIndividual};

/// Mutate a candidate genome by swapping two random keys.
pub fn mutate_keys_genome(ind: &KeysIndividual, _: &GaContext) -> Option<KeysGenome> {
    let mut genome = ind.genome.clone();
    let idx = rand::seq::index::sample(&mut rand::rng(), genome.len(), 2);
    genome.swap(idx.index(0), idx.index(1));
    Some(genome)
}
