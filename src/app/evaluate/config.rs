use serde::Deserialize;
use std::path::PathBuf;

/// Settings for the evaluation mode.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateConfig {
    /// input layouts csv file
    pub input: Option<PathBuf>,

    /// output file path; overwrites input when omitted
    pub output: Option<PathBuf>,

    /// number of layouts to print to stdout
    #[serde(default = "default_print")]
    pub print: usize,
}

fn default_print() -> usize {
    10
}
