use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the merge mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct MergeConfig {
    /// folder containing `.txt` files to merge
    pub input: Option<PathBuf>,

    /// output file path
    pub output: Option<PathBuf>,

    /// Shuffle cleaned lines before writing.
    #[serde(default)]
    pub shuffle: bool,

    /// Optional random seed for deterministic shuffling.
    pub seed: Option<u64>,
}
