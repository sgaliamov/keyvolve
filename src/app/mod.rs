mod config;

use cliffa::cli::AppHandle;
use miette::{Context, Result, bail};
use tracing::{info, trace};
pub use config::*;

use crate::app::Config;

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, _app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    Ok(())
}
