use darwin::Gene;
use serde::Deserialize;
use std::path::PathBuf;

/// Key position: (row, col)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
pub struct KeyPos(pub char, pub u8);

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// keyboard json settings
    pub keyboard: Option<PathBuf>,

    /// sample text file
    pub text: Option<PathBuf>,

    /// digraphs json file
    pub digraphs: Option<PathBuf>,

    /// darwin config for the genetic algorithm
    pub ga: darwin::Config<KeyPos>,
}

impl Gene for KeyPos {
    fn to_f64(self) -> f64 {
        ((self.0 as u16) << 8 | self.1 as u16) as f64
    }
}
