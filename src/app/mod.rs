mod config;
mod keyboard;
mod layout;
mod scorer;

use crate::app::layout::Layout;
use cliffa::cli::AppHandle;
pub use config::*;
pub use keyboard::*;
pub use scorer::*;
use miette::{Context, Result};
use std::path::Path;
use tracing::{info, trace};

/// Entry point called by the CLI builder after argument parsing.
pub fn run(config: Option<Config>, _app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    let keyboard = Keyboard::load(
        cfg.keyboard
            .as_deref()
            .unwrap_or(Path::new("data/keyboard.json")),
    )?;
    info!("Keyboard loaded: {} efforts entries", keyboard.pairs.len());

    let layouts = Layout::load(
        cfg.layouts
            .as_deref()
            .unwrap_or(Path::new("data/layouts.csv")),
    );
    info!("Loaded {} layouts", layouts.len());

    Ok(())
}
