use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the corpus synthesise mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SynthesiseConfig {
    /// output stem path — emits `<output>.csv` and `<output>.txt`
    pub output: Option<PathBuf>,

    /// target total digraph edge count in the synthesised corpus
    #[serde(default = "default_target")]
    pub target: usize,

    /// minimum accepted relative frequency (pairs below this are dropped)
    #[serde(default = "default_min_freq")]
    pub min_frequency: f64,
}

fn default_target() -> usize {
    100_000
}

fn default_min_freq() -> f64 {
    0.001
}

impl SynthesiseConfig {
    pub fn default_target() -> usize {
        100_000
    }

    pub fn default_min_freq() -> f64 {
        0.001
    }
}

impl Default for SynthesiseConfig {
    fn default() -> Self {
        Self {
            output: None,
            target: Self::default_target(),
            min_frequency: Self::default_min_freq(),
        }
    }
}
