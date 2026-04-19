use std::path::PathBuf;

pub struct Config {
    /// keyboard settings
    pub keyboard: Option<PathBuf>,

    /// sample text
    pub text: Option<PathBuf>,

    pub digraphs: Option<PathBuf>,

    pub frozen_left: String,

    pub frozen_right: String,

    pub ga: darwin::Config,

    /// how much we render in at the end
    pub results_count: u8,

    pub left_count: u8,
}
