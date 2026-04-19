use std::path::PathBuf;

#[derive(Debug)]
pub struct Config {
    /// keyboard json settings
    pub keyboard: Option<PathBuf>,

    /// sample text file
    pub text: Option<PathBuf>,

    pub digraphs: Option<PathBuf>,

    pub frozen_left: String,

    pub frozen_right: String,

    pub ga: darwin::Config,

    /// how much we render in at the end
    pub results_count: u8,

    pub left_count: u8,
}
