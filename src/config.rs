use crate::app::OptimizationConfig;
use crate::app::merge::MergeConfig;
use crate::app::synthesise::SynthesiseConfig;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// keyboard json settings
    pub keyboard: Option<PathBuf>,

    /// Input layouts csv file for evaluation.
    pub layouts: Option<PathBuf>,

    /// sample text file
    pub text: Option<PathBuf>,

    /// darwin config for the genetic algorithm
    pub ga: darwin::Config<char>,

    /// mode of operation: optimize, evaluate, or synthesise
    pub mode: Mode,

    /// settings for `Mode::Synthesise`
    #[serde(default)]
    pub synthesise: SynthesiseConfig,

    /// settings for `Mode::Merge`
    #[serde(default)]
    pub merge: MergeConfig,

    /// Optimization settings, including optional seed layouts input.
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
