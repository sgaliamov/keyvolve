use crate::app::merge::MergeConfig;
use crate::app::synthesise::SynthesiseConfig;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::path::PathBuf;

/// Per-key constraints for optimization.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationConfig {
    /// Characters whose physical position is locked: maps char → key index (0-29).
    #[serde(default)]
    pub frozen: FxHashMap<char, u8>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// keyboard json settings
    pub keyboard: Option<PathBuf>,

    /// layouts csv file
    pub layouts: Option<PathBuf>,

    /// sample text file
    pub text: Option<PathBuf>,

    /// darwin config for the genetic algorithm
    pub ga: darwin::Config<char>,

    /// seed layouts in semicolon format, e.g. "jpdmq;eaurv;xyblz;khoc_;gnsit;wf___"
    #[serde(default)]
    pub seed: Vec<String>,

    /// mode of operation: optimize, evaluate, or synthesise
    pub mode: Mode,

    /// settings for `Mode::Synthesise`
    #[serde(default)]
    pub synthesise: SynthesiseConfig,

    /// settings for `Mode::Merge`
    #[serde(default)]
    pub merge: MergeConfig,

    /// frozen/blocked key constraints for `Mode::Optimize`
    #[serde(default)]
    pub optimization: OptimizationConfig,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Run the genetic algorithm to optimize the keyboard layout.
    Optimize,

    /// Evaluate the score of a specific layout.
    #[default]
    Evaluate,

    /// Build a digraph frequency CSV and synthesise a compact fake-word corpus.
    Synthesise,

    /// Merge all `.txt` files in a folder into one cleaned file.
    Merge,
}
