use crate::models::{GaContext, KeyPos, KeysGenome};
use itertools::Itertools;
use rand::seq::SliceRandom;

/// Generate a genome for optimization.
pub fn generate(_: &GaContext) -> KeysGenome {
    shuffled_keys()
}

/// Core: shuffled a–z mapped to positions 0–25.
fn shuffled_keys() -> KeysGenome {
    let mut chars: Vec<char> = ('a'..='z').collect();
    chars.shuffle(&mut rand::rng());
    chars
        .into_iter()
        .enumerate()
        .map(|(i, c)| KeyPos(c, i as u8))
        .collect_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_has_all_26_chars() {
        let genome = shuffled_keys();
        assert_eq!(genome.len(), 26);
        let mut chars: Vec<char> = genome.iter().map(|kp| kp.0).collect();
        chars.sort_unstable();
        assert_eq!(chars, ('a'..='z').collect::<Vec<_>>());
    }

    #[test]
    fn generate_positions_are_0_to_25() {
        let genome = shuffled_keys();
        let mut positions: Vec<u8> = genome.iter().map(|kp| kp.1).collect();
        positions.sort_unstable();
        assert_eq!(positions, (0u8..26).collect::<Vec<_>>());
    }

    #[test]
    fn generate_is_shuffled() {
        // Probability of two independent shuffles being identical: 1/26! ≈ 0.
        let a = shuffled_keys();
        let b = shuffled_keys();
        assert_ne!(
            a, b,
            "two shuffles should differ (astronomically unlikely to collide)"
        );
    }
}
