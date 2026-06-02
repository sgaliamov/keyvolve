use serde::Deserialize;
use std::path::PathBuf;

/// Synthesise generation method.
#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SynthesiseMethod {
    /// Original graph-based digraph synthesis.
    #[default]
    Digraph,

    /// Sample words from the source corpus and score against source metrics.
    Sample,

    /// Generate words from a bigram Markov chain, optimizing CorpusScore metrics.
    Markov,
}

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

    /// synthesis method to run
    #[serde(default)]
    pub method: SynthesiseMethod,

    /// minimum accepted relative frequency (pairs below this are dropped)
    #[serde(default = "default_min_freq")]
    pub min_frequency: f64,

    /// optional RNG seed for reproducible sampling
    pub seed: Option<u64>,

    /// digraph method config
    #[serde(default)]
    pub digraph: DigraphSynthesiseConfig,

    /// sample method config
    #[serde(default)]
    pub sample: SampleSynthesiseConfig,

    /// markov method config
    #[serde(default)]
    pub markov: MarkovSynthesiseConfig,
}

/// Parameters used by the digraph synthesis method.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DigraphSynthesiseConfig {
    /// target total digraph edge count in the synthesised corpus
    #[serde(default = "default_target")]
    pub target: usize,

    /// max characters per output word
    #[serde(default = "default_digraph_max_word_len")]
    pub max_word_len: usize,
}

/// Parameters used by the sample synthesis method.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SampleSynthesiseConfig {
    /// output word count sampled from source
    #[serde(default = "default_target")]
    pub word_count: usize,
}

/// Parameters used by the markov synthesis method.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MarkovSynthesiseConfig {
    /// target total digraph edge count in generated corpus
    #[serde(default = "default_markov_target")]
    pub target: usize,

    /// max characters per output word
    #[serde(default = "default_markov_max_word_len")]
    pub max_word_len: usize,

    /// tries before giving up and returning the best candidate
    #[serde(default = "default_attempts")]
    pub attempts: usize,
}

pub(super) fn default_target() -> usize {
    100_000
}

pub(super) fn default_markov_target() -> usize {
    2_000_000
}

pub(super) fn default_min_freq() -> f64 {
    0.0001
}

pub(super) fn default_digraph_max_word_len() -> usize {
    5
}

pub(super) fn default_markov_max_word_len() -> usize {
    15
}

pub(super) fn default_attempts() -> usize {
    32
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

impl Default for DigraphSynthesiseConfig {
    fn default() -> Self {
        Self {
            target: default_target(),
            max_word_len: default_digraph_max_word_len(),
        }
    }
}

impl Default for SampleSynthesiseConfig {
    fn default() -> Self {
        Self {
            word_count: default_target(),
        }
    }
}

impl Default for MarkovSynthesiseConfig {
    fn default() -> Self {
        Self {
            target: default_markov_target(),
            max_word_len: default_markov_max_word_len(),
            attempts: default_attempts(),
        }
    }
}
