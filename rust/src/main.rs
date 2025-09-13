#![allow(clippy::cargo_common_metadata)]
use anyhow::Result;
use pkgmanager_builder::{cli, config::Config, setup_logging};

fn main() -> Result<()> {
    // Parse command line arguments
    let args = cli::parse_args();

    // Setup logging based on debug flag
    setup_logging(args.debug)?;

    // Initialize configuration
    let config = Config::from_args(&args)?;

    // Execute the appropriate command
    cli::execute_command(&config, &args.command)
}
