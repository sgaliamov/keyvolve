mod config;
mod keyboard;

pub use config::*;
pub use keyboard::*;
use cliffa::cli::AppHandle;
use miette::{Context, Result};
use tracing::{info, trace};

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, _app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    let keyboard = Keyboard::load(cfg.keyboard.as_deref().unwrap_or("data/keyboard.json".as_ref()))?;
    info!("Keyboard loaded: {} efforts entries", keyboard.efforts.len());

    Ok(())
}

