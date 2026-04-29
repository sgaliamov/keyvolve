use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::path::Path;

/// Represents the keyboard configuration loaded from `keyboard.json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Keyboard {
    /// Keys that are frozen in place: maps character to key index.
    #[serde(default)]
    pub frozen: FxHashMap<char, u8>,

    /// Key indices that are blocked (unavailable).
    #[serde(default)]
    pub blocked: Vec<u8>,

    /// Extra penalty applied on hand switches; `0.0` means no penalty.
    #[serde(default)]
    pub switch_penalty: f64,

    /// Max multiplier applied for extreme hand imbalance.
    #[serde(default = "default_balance_penalty")]
    pub balance_penalty: f64,

    /// Effort multipliers used to scale effort values.
    pub efforts: Vec<f64>,

    /// Key matrix: pairs[from][to] = group.
    /// Pairs are defined for the left hand only; the right hand is inferred by symmetry.
    pub pairs: FxHashMap<u8, FxHashMap<u8, usize>>,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self {
            frozen: FxHashMap::default(),
            blocked: Vec::new(),
            switch_penalty: 1.0,
            balance_penalty: default_balance_penalty(),
            efforts: Vec::new(),
            pairs: FxHashMap::default(),
        }
    }
}

/// Default max imbalance multiplier.
fn default_balance_penalty() -> f64 {
    2.0
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

    /// Mirror left-hand pairs (0–14) to the right hand (15–29) by symmetry.
    /// Mirror formula (5 keys/row, 3 rows): `mirror(i) = (i/5)*5 + (4 - i%5) + 15`.
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
            frozen: FxHashMap::default(),
            blocked: vec![],
            switch_penalty: 0.0,
            balance_penalty: 2.0,
            efforts: vec![1.0],
            pairs: FxHashMap::from_iter([(0u8, FxHashMap::from_iter([(5u8, 1usize)]))]),
        }
        .expand_pairs();

        // mirror(0) = 0/5*5 + (4-0%5) + 15 = 19, mirror(5) = 1*5 + (4-0) + 15 = 24
        assert_eq!(kb.pairs[&19][&24], 1);
    }
}
