use serde::Deserialize;
use std::path::PathBuf;

fn default_print() -> usize {
    50
}

/// Settings for the frequencies mode.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FrequenciesConfig {
    /// folder scanned recursively for matching files
    pub input: Option<PathBuf>,

    /// filename masks (`*`/`?` wildcards, case-insensitive); empty matches all files
    #[serde(default)]
    pub masks: Vec<String>,

    /// output csv path (`key,count,frequency`); stdout only when omitted
    pub output: Option<PathBuf>,

    /// number of top keys to print to stdout
    #[serde(default = "default_print")]
    pub print: usize,
}

impl Default for FrequenciesConfig {
    fn default() -> Self {
        Self {
            input: None,
            masks: Vec::new(),
            output: None,
            print: default_print(),
        }
    }
}
