use serde::Deserialize;
use std::path::PathBuf;

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

    /// output path for synthesise mode (csv + corpus txt)
    pub output: Option<PathBuf>,

    /// target total digraph edge count for synthesised corpus (default: 100_000)
    #[serde(default = "default_target")]
    pub target: usize,
}

fn default_target() -> usize {
    100_000
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
}
