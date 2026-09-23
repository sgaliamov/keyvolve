use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the corpus stats mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatsConfig {
    /// source text file to scan
    pub input: Option<PathBuf>,

    /// cached corpus stats JSON output path
    pub output: Option<PathBuf>,

    /// minimum accepted relative frequency for saved bigrams
    #[serde(default = "default_min_frequency")]
    pub min_frequency: f64,
}

impl Default for StatsConfig {
    fn default() -> Self {
        Self {
            input: None,
            output: None,
            min_frequency: default_min_frequency(),
        }
    }
}

fn default_min_frequency() -> f64 {
    0.0001
}
