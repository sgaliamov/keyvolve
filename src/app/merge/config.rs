use serde::Deserialize;
use std::path::PathBuf;

fn default_print() -> usize {
    100
}

fn default_min_frequency() -> f64 {
    0.0001
}

/// Settings for the merge mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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

    /// Number of merged lines to print to stdout.
    #[serde(default = "default_print")]
    pub print: usize,

    /// Minimum bigram frequency kept in saved corpus stats.
    #[serde(default = "default_min_frequency")]
    pub min_frequency: f64,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            input: None,
            output: None,
            shuffle: false,
            seed: None,
            print: default_print(),
            min_frequency: default_min_frequency(),
        }
    }
}
