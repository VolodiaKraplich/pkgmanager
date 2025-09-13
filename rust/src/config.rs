//! Configuration management for the package builder
//!
//! Centralizes configuration options and provides validation.

use crate::{cli::Args, error::BuilderError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Enable debug logging
    pub debug: bool,
    /// Working directory for operations
    pub work_dir: PathBuf,
    /// PKGBUILD file path
    pub pkgbuild_path: PathBuf,
    /// Package manager configuration
    pub package_manager: PackageManagerConfig,
    /// Build configuration
    pub build: BuildConfig,
    /// Artifact configuration
    pub artifacts: ArtifactConfig,
}

/// Package manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManagerConfig {
    /// Primary package manager (paru, yay, pacman)
    pub primary: String,
    /// Fallback package manager
    pub fallback: Option<String>,
    /// Additional arguments for package installation
    pub install_args: Vec<String>,
    /// Handle rust/rustup conflicts
    pub handle_rust_conflict: bool,
}

/// Build configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Clean previous builds
    pub clean: bool,
    /// Sign packages
    pub sign: bool,
    /// Enable ccache
    pub use_ccache: bool,
    /// ccache directory
    pub ccache_dir: PathBuf,
    /// Additional build arguments
    pub build_args: Vec<String>,
}

/// Artifact configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactConfig {
    /// Output directory for artifacts
    pub output_dir: PathBuf,
    /// Version file path
    pub version_file: PathBuf,
    /// File patterns to collect
    pub patterns: Vec<String>,
    /// Whether to preserve source files
    pub preserve_sources: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            debug: false,
            work_dir: PathBuf::from("."),
            pkgbuild_path: PathBuf::from("PKGBUILD"),
            package_manager: PackageManagerConfig::default(),
            build: BuildConfig::default(),
            artifacts: ArtifactConfig::default(),
        }
    }
}

impl Default for PackageManagerConfig {
    fn default() -> Self {
        Self {
            primary: "paru".to_string(),
            fallback: Some("pacman".to_string()),
            install_args: vec![
                "-S".to_string(),
                "--noconfirm".to_string(),
                "--needed".to_string(),
                "--asdeps".to_string(),
            ],
            handle_rust_conflict: true,
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            clean: false,
            sign: false,
            use_ccache: true,
            ccache_dir: PathBuf::from("/home/builder/.ccache"),
            build_args: vec!["-B".to_string(), "--noconfirm".to_string()],
        }
    }
}

impl Default for ArtifactConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("artifacts"),
            version_file: PathBuf::from("version.env"),
            patterns: vec![
                "*.pkg.tar.*".to_string(),
                "*.log".to_string(),
                "PKGBUILD".to_string(),
                ".SRCINFO".to_string(),
            ],
            preserve_sources: true,
        }
    }
}

impl Config {
    /// Create configuration from command line arguments
    pub fn from_args(args: &Args) -> Result<Self, BuilderError> {
        let mut config = Self {
            debug: args.debug,
            ..Self::default()
        };
        
        // Override with command-specific options
        match &args.command {
            crate::cli::Command::Build { clean, sign } => {
                config.build.clean = *clean;
                config.build.sign = *sign;
            }
            crate::cli::Command::Artifacts { output_dir } => {
                config.artifacts.output_dir = output_dir.clone();
            }
            crate::cli::Command::Version { output_file } => {
                config.artifacts.version_file = output_file.clone();
            }
            _ => {}
        }
        
        config.validate()?;
        Ok(config)
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), BuilderError> {
        if !self.pkgbuild_path.exists() {
            return Err(BuilderError::validation(
                format!("PKGBUILD file not found: {}", self.pkgbuild_path.display())
            ));
        }
        
        if !self.work_dir.exists() {
            return Err(BuilderError::validation(
                format!("Working directory not found: {}", self.work_dir.display())
            ));
        }
        
        Ok(())
    }
    
    /// Get package manager command with arguments
    pub fn get_package_manager_cmd(&self) -> (String, Vec<String>) {
        let cmd_args = self.package_manager.install_args.clone();
        (self.package_manager.primary.clone(), cmd_args)
    }
    
    /// Get build command with arguments
    pub fn get_build_cmd(&self) -> (String, Vec<String>) {
        let mut args = self.build.build_args.clone();
        args.push("./".to_string());
        
        if self.build.sign {
            args.push("--sign".to_string());
        }
        
        ("paru".to_string(), args)
    }
}