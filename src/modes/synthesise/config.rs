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

    /// stats directory; defaults to `output/../stats`
    pub stats: Option<PathBuf>,

    /// minimum accepted relative frequency (pairs below this are dropped)
    #[serde(default = "default_min_freq")]
    pub min_frequency: f64,

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
    0.0001
}

impl SynthesiseConfig {
    /// Resolved stats directory: explicit `stats` field, or `output/../stats`.
    pub fn stats_dir(&self) -> Option<std::path::PathBuf> {
        if let Some(s) = &self.stats {
            return Some(s.clone());
        }
        let out = self.output.as_deref()?;
        Some(out.parent()?.parent()?.join("stats"))
    }
}

impl Default for SampleSynthesiseConfig {
    fn default() -> Self {
        Self {
            target: default_target(),
        }
    }
}
