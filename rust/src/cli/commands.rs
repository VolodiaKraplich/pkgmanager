//! Command implementations for the CLI

use crate::{
    cli::Command,
    config::Config,
    core::{artifacts::ArtifactCollector, builder::PackageBuilder, pkgbuild::PkgbuildParser},
    utils::env::VersionGenerator,
};
use anyhow::Context;
use tracing::{info, instrument};

/// Execute the appropriate command based on CLI arguments
#[instrument(skip(config))]
pub fn execute_command(config: &Config, command: &Command) -> anyhow::Result<()> {
    match command {
        Command::Deps => execute_deps_command(config),
        Command::Build { .. } => execute_build_command(config),
        Command::Artifacts { .. } => execute_artifacts_command(config),
        Command::Version { .. } => execute_version_command(config),
    }
}

/// Execute the dependencies command
#[instrument(skip(config))]
fn execute_deps_command(config: &Config) -> anyhow::Result<()> {
    info!("Installing PKGBUILD dependencies...");

    let parser = PkgbuildParser::new()?;
    let pkgbuild = parser
        .parse(&config.pkgbuild_path)
        .context("Failed to parse PKGBUILD")?;

    let mut builder = PackageBuilder::new(config.clone());
    builder
        .install_dependencies(&pkgbuild)
        .context("Failed to install dependencies")?;

    info!("Dependencies installation completed successfully");
    Ok(())
}

/// Execute the build command
#[instrument(skip(config))]
fn execute_build_command(config: &Config) -> anyhow::Result<()> {
    info!("Building package...");

    let parser = PkgbuildParser::new()?;
    let pkgbuild = parser
        .parse(&config.pkgbuild_path)
        .context("Failed to parse PKGBUILD")?;

    let builder = PackageBuilder::new(config.clone());

    if config.build.clean {
        builder.clean().context("Failed to clean previous builds")?;
    }

    let package_files = builder
        .build(&pkgbuild)
        .context("Failed to build package")?;

    info!(
        "Build completed successfully. Generated {} package(s): {:?}",
        package_files.len(),
        package_files
    );

    Ok(())
}

/// Execute the artifacts command
#[instrument(skip(config))]
fn execute_artifacts_command(config: &Config) -> anyhow::Result<()> {
    info!(
        "Collecting build artifacts to: {}",
        config.artifacts.output_dir.display()
    );

    let collector = ArtifactCollector::new(config.clone());
    let collected_files = collector.collect().context("Failed to collect artifacts")?;

    info!(
        "Artifacts collected successfully. {} files collected",
        collected_files.len()
    );

    Ok(())
}

/// Execute the version command
#[instrument(skip(config))]
fn execute_version_command(config: &Config) -> anyhow::Result<()> {
    info!(
        "Generating version information to: {}",
        config.artifacts.version_file.display()
    );

    let parser = PkgbuildParser::new()?;
    let pkgbuild = parser
        .parse(&config.pkgbuild_path)
        .context("Failed to parse PKGBUILD")?;

    let generator = VersionGenerator::new();
    generator
        .generate(&pkgbuild, &config.artifacts.version_file)
        .context("Failed to generate version file")?;

    info!("Version information generated successfully");
    Ok(())
}
