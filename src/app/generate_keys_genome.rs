use crate::models::{GaContext, KeyPos, KeysGenome};
use itertools::Itertools;
use rand::seq::SliceRandom;

/// Generate a genome for optimization.
pub fn generate_keys_genome(_: &GaContext) -> KeysGenome {
    let mut chars: Vec<char> = ('a'..='z').collect();
    chars.shuffle(&mut rand::rng());
    chars
        .into_iter()
        .enumerate()
        .map(|(i, c)| KeyPos(c, i as u8))
        .collect_vec()
}
