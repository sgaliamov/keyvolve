mod app;
mod config;
mod models;

use cliffa::cli;
pub use config::*;
use miette::Result;
use tracing::Level;

fn main() -> Result<()> {
    cli::Builder::default()
        .with_level(Level::TRACE)
        .with_target(false)
        .with_cli_alias("m", "mode")
        .with_time(false)
        .show_level(false)
        .run(app::run)
}
