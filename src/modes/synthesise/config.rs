use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the corpus synthesise mode.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SynthesiseConfig {
    /// input source text path
    pub text: Option<PathBuf>,

    /// output corpus path
    pub output: Option<PathBuf>,

    /// minimum accepted relative frequency for letters and bigrams
    #[serde(default = "default_min_freq")]
    pub min_frequency: f64,

    /// minimum accepted relative frequency for trigrams
    #[serde(default = "default_min_trigram_freq")]
    pub min_trigram_frequency: f64,

    /// optional RNG seed for reproducible sampling
    pub seed: Option<u64>,

    /// sample method config
    #[serde(default)]
    pub sample: SampleSynthesiseConfig,
}

/// Parameters used by the sample synthesis method.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SampleSynthesiseConfig {
    /// output word count sampled from source
    #[serde(default = "default_target")]
    pub target: usize,
}

pub(super) fn default_target() -> usize {
    100_000
}

pub(super) fn default_min_freq() -> f64 {
    0.000_001
}

pub(super) fn default_min_trigram_freq() -> f64 {
    0.000_001
}

impl Default for SampleSynthesiseConfig {
    fn default() -> Self {
        Self {
            target: default_target(),
        }
    }
}
