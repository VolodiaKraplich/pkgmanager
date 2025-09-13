//! # Arch Package Builder
//!
//! A reliable tool for building Arch Linux/PrismLinux packages in GitLab CI.
//! This library provides functionality to parse PKGBUILD files, install dependencies,
//! build packages, and collect artifacts without executing shell scripts.
//!
//! ## Features
//!
//! - Safe PKGBUILD parsing without shell execution
//! - Dependency management with conflict resolution
//! - Package building with paru integration
//! - Artifact collection and version generation
//! - Professional error handling and logging
//!
//! ## Example
//!
//! ```no_run
//! use arch_package_builder::{config::Config, core::PkgbuildParser};
//!
//! let parser = PkgbuildParser::new();
//! let pkgbuild = parser.parse("PKGBUILD")?;
//! println!("Package: {}-{}", pkgbuild.name, pkgbuild.version);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod cli;
pub mod config;
pub mod core;
pub mod error;
pub mod utils;

use anyhow::Result;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize logging with appropriate verbosity
pub fn setup_logging(debug: bool) -> Result<()> {
    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_level(true)
                .compact(),
        )
        .with(filter)
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize logging: {}", e))?;

    Ok(())
}
