use cliffa::cli::AppHandle;
use darwin::Config;
use miette::{Context, Result, bail};
use tracing::{info, trace};

pub fn run(config: Option<Config>, app: AppHandle) -> Result<()> {
    let cfg = config.wrap_err("Missing config.")?;
    trace!("Starting with config: {:#?}", cfg);

    Ok(())
}
