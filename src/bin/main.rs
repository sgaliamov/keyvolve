use cliffa::cli;
use keyvolve::app;
use miette::Result;
use tracing::Level;

fn main() -> Result<()> {
    cli::Builder::default()
        .with_level(Level::INFO)
        .with_target(false)
        .with_time(false)
        .show_level(false)
        .run(app::run)
}
