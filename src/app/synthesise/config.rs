use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the corpus synthesise mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SynthesiseConfig {
    /// input source text path
    pub text: Option<PathBuf>,

    /// output stem path — emits `<output>.csv` and `<output>.txt`
    pub output: Option<PathBuf>,

    /// target total digraph edge count in the synthesised corpus
    #[serde(default = "default_target")]
    pub target: usize,

    /// minimum accepted relative frequency (pairs below this are dropped)
    #[serde(default = "default_min_freq")]
    pub min_frequency: f64,

    /// max characters per output word
    #[serde(default = "default_max_word_len")]
    pub max_word_len: usize,
}

pub(super) fn default_target() -> usize {
    100_000
}

pub(super) fn default_min_freq() -> f64 {
    0.001
}

pub(super) fn default_max_word_len() -> usize {
    10
}

impl Default for SynthesiseConfig {
    fn default() -> Self {
        Self {
            text: None,
            output: None,
            target: default_target(),
            min_frequency: default_min_freq(),
            max_word_len: default_max_word_len(),
        }
    }
}
