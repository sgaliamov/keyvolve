use crate::app::evaluate::EvaluateConfig;
use crate::app::frequencies::FrequenciesConfig;
use crate::app::merge::MergeConfig;
use crate::app::synthesise::SynthesiseConfig;
use crate::app::{LayoutEvaluatorConfig, OptimizationConfig};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// keyboard json settings
    pub keyboard: std::path::PathBuf,

    /// darwin config for the genetic algorithm
    pub ga: darwin::Config<char>,

    /// mode of operation: optimize, evaluate, or synthesise
    #[serde(default)]
    pub mode: Mode,

    /// settings for `Mode::Synthesise`
    #[serde(default)]
    pub synthesise: SynthesiseConfig,

    /// settings for `Mode::Evaluate`
    #[serde(default)]
    pub evaluate: EvaluateConfig,

    /// Layout scoring settings shared by evaluation and optimization.
    #[serde(default)]
    pub evaluator: LayoutEvaluatorConfig,

    /// settings for `Mode::Merge`
    #[serde(default)]
    pub merge: MergeConfig,

    /// settings for `Mode::Frequencies`
    #[serde(default)]
    pub frequencies: FrequenciesConfig,

    /// Optimization settings, including optional seed layouts input.
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

    /// Count per-key char frequencies (incl. punctuation) across files in a folder.
    Frequencies,
}
