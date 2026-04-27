use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::path::Path;

/// Represents the keyboard configuration loaded from `keyboard.json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Keyboard {
    /// Keys that are frozen in place: maps character to key index.
    pub frozen: FxHashMap<char, u8>,

    /// Key indices that are blocked (unavailable).
    pub blocked: Vec<u8>,

    /// Penalty applied when switching hands between consecutive keystrokes.
    pub switch_penalty: f64,

    /// Penalty applied when the same key is pressed consecutively.
    pub same_key_penalty: f64,

    /// Effort multipliers used to scale effort values.
    pub efforts: Vec<f64>,

    /// Key matrix: pairs[from][to] = group.
    pub pairs: FxHashMap<u8, FxHashMap<u8, usize>>,
}

impl Keyboard {
    /// Load and deserialize from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let data = std::fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read keyboard file: {}", path.display()))?;
        serde_json::from_str(&data)
            .into_diagnostic()
            .wrap_err("Failed to parse keyboard JSON")
    }

    pub fn calculate_score(&self, letters: &Vec<char>) -> f64 {
        if letters.len() == 0 {
            return 0.;
        }

        let mut score = 0.0;
        let mut j = 0;

        // while j < letters.len() - 1 {
        //     let mut i = j + 1;
        //     let first = letters[j];

        //     while i < letters.len() {
        //         let second = letters[i];
        //         let value = self.
        //         score += self.get_value(&first, &second);
        //         i += 1;
        //     }
        //     j += 1;
        // }

        score
    }
}
