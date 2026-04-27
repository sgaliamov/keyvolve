use darwin::Gene;
use serde::Deserialize;
use std::path::PathBuf;

/// Key position: (char, position index)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub struct KeyPos(pub char, pub u8);

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
    pub ga: darwin::Config<KeyPos>,

    /// mode of operation: optimize or evaluate
    pub mode: Mode,
}

#[derive(Debug, Default,  Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Run the genetic algorithm to optimize the keyboard layout.
    Optimize,

    /// Evaluate the score of a specific layout.
    #[default]
    Evaluate,
}

impl Gene for KeyPos {
    fn to_f64(self) -> f64 {
        ((self.0 as u16) << 8 | self.1 as u16) as f64
    }
}
