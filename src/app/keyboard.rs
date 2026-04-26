use miette::{Context, IntoDiagnostic, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Represents the keyboard configuration loaded from `keyboard.json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Keyboard {
    /// Keys that are frozen in place: maps character to key index.
    pub frozen: HashMap<char, u8>,

    /// Key indices that are blocked (unavailable).
    pub blocked: Vec<u8>,

    /// Penalty applied when switching hands between consecutive keystrokes.
    pub switch_penalty: f64,

    /// Penalty applied when the same key is pressed consecutively.
    pub same_key_penalty: f64,

    /// Effort multipliers used to scale effort values.
    pub efforts_map: Vec<f64>,

    /// Effort cost matrix: efforts[from][to] = cost.
    pub efforts: HashMap<u8, HashMap<u8, usize>>,
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
}
