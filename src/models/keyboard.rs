use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::path::Path;

/// Row index within a hand (0 = top, 2 = bottom).
#[inline]
pub fn slot_row(slot: u8) -> u8 {
    (slot % 15) / 5
}

/// Represents the keyboard configuration loaded from `keyboard.json`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Keyboard {
    /// Effort multipliers used to scale effort values.
    pub efforts: Vec<f64>,

    /// Key matrix: pairs[from][to] = group.
    /// Pairs are defined for the left hand only; the right hand is inferred by symmetry.
    pub pairs: FxHashMap<u8, FxHashMap<u8, usize>>,
}

impl Keyboard {
    pub fn new(json: String) -> Keyboard {
        let keyboard: Keyboard =
            serde_json::from_str(&json).expect("Failed to parse keyboard JSON");
        keyboard.expand_pairs()
    }

    /// Load and deserialize from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        let json = std::fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read keyboard file: {}", path.display()))?;

        let keyboard = Self::new(json);
        Ok(keyboard)
    }

    /// Mirror left-hand pairs (0–14) to the right hand (15–29) by column symmetry.
    ///
    /// Key index layout (5 keys/row, 3 rows, 2 hands):
    /// ```text
    /// Left:   0  1  2  3  4      Right:  15 16 17 18 19
    ///         5  6  7  8  9              20 21 22 23 24
    ///        10 11 12 13 14              25 26 27 28 29
    /// ```
    /// Left hand: col 0 = pinky, col 4 = index.
    /// Right hand: col 0 (15) = index, col 4 (19) = pinky.
    /// Columns are mirrored: left-col-k ↔ right-col-(4-k).
    /// Mirror formula: `mirror(i) = (i/5)*5 + (4 - i%5) + 15`.
    fn expand_pairs(mut self) -> Self {
        let mirror = |i: u8| -> u8 { (i / 5) * 5 + (4 - i % 5) + 15 };

        let left: Vec<(u8, u8, usize)> = self
            .pairs
            .iter()
            .flat_map(|(from, targets)| targets.iter().map(move |(to, group)| (*from, *to, *group)))
            .collect();

        for (from, to, group) in left {
            self.pairs
                .entry(mirror(from))
                .or_default()
                .entry(mirror(to))
                .or_insert(group);
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_pairs_mirrors_left_to_right() {
        let kb = Keyboard {
            efforts: vec![1.0],
            pairs: FxHashMap::from_iter([(0u8, FxHashMap::from_iter([(5u8, 1usize)]))]),
        }
        .expand_pairs();

        // mirror(0) = 0/5*5 + (4-0%5) + 15 = 19, mirror(5) = 1*5 + (4-0) + 15 = 24
        assert_eq!(kb.pairs[&19][&24], 1);
    }
}
