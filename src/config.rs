use crate::evaluator::LayoutEvaluatorConfig;
use crate::modes::evaluate::EvaluateConfig;
use crate::modes::frequencies::FrequenciesConfig;
use crate::modes::merge::MergeConfig;
use crate::modes::optimize::OptimizationConfig;
use crate::modes::rank::RankConfig;
use crate::modes::stats::BuildStatsConfig;
use crate::modes::synthesise::SynthesiseConfig;
use serde::Deserialize;

/// Root config.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Darwin config for the genetic algorithm.
    pub ga: darwin::Config<char>,

    /// Mode of operation: optimize, evaluate, synthesise, stats, etc.
    #[serde(default)]
    pub mode: Mode,

    /// Settings for `Mode::BuildStats`.
    #[serde(default)]
    pub stats: BuildStatsConfig,

    /// Settings for `Mode::Synthesise`.
    #[serde(default)]
    pub synthesise: SynthesiseConfig,

    /// Settings for `Mode::Evaluate`.
    #[serde(default)]
    pub evaluate: EvaluateConfig,

    /// Layout scoring settings shared by evaluation and optimization.
    #[serde(default)]
    pub evaluator: LayoutEvaluatorConfig,

    /// Settings for `Mode::Merge`.
    #[serde(default)]
    pub merge: MergeConfig,

    /// Settings for `Mode::Frequencies`.
    #[serde(default)]
    pub frequencies: FrequenciesConfig,

    /// Optimization settings, including optional seed layouts input.
    #[serde(default)]
    pub optimization: OptimizationConfig,

    /// Settings for `Mode::Rank`.
    #[serde(default)]
    pub rank: RankConfig,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Run the genetic algorithm to optimize the keyboard layout.
    Optimize,

    /// Evaluate the score of a specific layout.
    #[default]
    Evaluate,

    /// Build cached corpus stats from a raw text corpus.
    Stats,

    /// Build a compact fake-word corpus from the source text and frequency stats.
    Synthesise,

    /// Merge all `.txt` files in a folder into one cleaned file.
    Merge,

    /// Count per-key char frequencies (incl. punctuation) across files in a folder.
    Frequencies,

    /// Interactively rank bigram pairs to calibrate keyboard efforts.
    Rank,
}
