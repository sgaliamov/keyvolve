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
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SynthesiseConfig {
    /// input source text path
    pub text: Option<PathBuf>,

    /// output corpus path
    pub output: Option<PathBuf>,

    /// synthesis method to run
    #[serde(default)]
    pub method: SynthesiseMethod,

    /// target total digraph edge count in the synthesised corpus
    #[serde(default = "default_target")]
    pub target: usize,

    /// minimum accepted relative frequency (pairs below this are dropped)
    #[serde(default = "default_min_freq")]
    pub min_frequency: f64,

    /// max characters per output word for digraph method
    #[serde(default = "default_digraph_max_word_len")]
    pub digraph_max_word_len: usize,

    /// max characters per output word for markov method
    #[serde(default = "default_markov_max_word_len")]
    pub markov_max_word_len: usize,

    /// global max allowed relative error across tracked metrics
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,

    /// tries before giving up and returning the best candidate
    #[serde(default = "default_attempts")]
    pub attempts: usize,

    /// output word count for sample method
    #[serde(default = "default_word_count")]
    pub word_count: usize,

    /// optional RNG seed for reproducible sampling
    #[serde(default)]
    pub seed: Option<u64>,
}

pub(super) fn default_tolerance() -> f64 {
    0.01
}

pub(super) fn default_target() -> usize {
    100_000
}

pub(super) fn default_min_freq() -> f64 {
    0.001
}

pub(super) fn default_digraph_max_word_len() -> usize {
    5
}

pub(super) fn default_markov_max_word_len() -> usize {
    5
}

pub(super) fn default_attempts() -> usize {
    32
}

pub(super) fn default_word_count() -> usize {
    100_000
}

impl Default for SynthesiseConfig {
    fn default() -> Self {
        Self {
            text: None,
            output: None,
            method: SynthesiseMethod::default(),
            target: default_target(),
            min_frequency: default_min_freq(),
            digraph_max_word_len: default_digraph_max_word_len(),
            markov_max_word_len: default_markov_max_word_len(),
            tolerance: default_tolerance(),
            attempts: default_attempts(),
            word_count: default_word_count(),
            seed: None,
        }
    }
}
