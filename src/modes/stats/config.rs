use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the build-stats mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildStatsConfig {
    /// source text file to scan
    pub input: Option<PathBuf>,

    /// cached corpus stats JSON output path
    pub output: Option<PathBuf>,

    /// minimum accepted relative frequency for letters and bigrams
    #[serde(default = "default_min_frequency")]
    pub min_frequency: f64,

    /// minimum accepted relative frequency for trigrams
    #[serde(default = "default_min_trigram_frequency")]
    pub min_trigram_frequency: f64,
}

impl Default for BuildStatsConfig {
    fn default() -> Self {
        Self {
            input: None,
            output: None,
            min_frequency: default_min_frequency(),
            min_trigram_frequency: default_min_trigram_frequency(),
        }
    }
}

fn default_min_frequency() -> f64 {
    0.000_001
}

fn default_min_trigram_frequency() -> f64 {
    0.0001
}
