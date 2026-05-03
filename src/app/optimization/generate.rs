use crate::app::{GaContext, KeysGenome};
use rand::seq::SliceRandom;

/// Generate a genome for optimization.
pub fn generate(_: &GaContext) -> KeysGenome {
    shuffled_keys()
}

/// Core: 26 letters + 4 empty slots shuffled into 30 physical key positions.
fn shuffled_keys() -> KeysGenome {
    let mut genome: Vec<char> = ('a'..='z').chain(['`', '`', '`', '`']).collect();
    genome.shuffle(&mut rand::rng());
    genome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_has_all_26_chars() {
        let genome = shuffled_keys();
        assert_eq!(genome.len(), 30);
        let mut chars: Vec<char> = genome.iter().copied().filter(|&c| c != '`').collect();
        chars.sort_unstable();
        assert_eq!(chars, ('a'..='z').collect::<Vec<_>>());
    }

    #[test]
    fn generate_has_four_empty_slots() {
        let genome = shuffled_keys();
        assert_eq!(genome.iter().filter(|&&c| c == '`').count(), 4);
    }

    #[test]
    fn generate_is_shuffled() {
        // Probability of two independent shuffles being identical: 1/30! ≈ 0.
        let a = shuffled_keys();
        let b = shuffled_keys();
        assert_ne!(
            a, b,
            "two shuffles should differ (astronomically unlikely to collide)"
        );
    }
}
