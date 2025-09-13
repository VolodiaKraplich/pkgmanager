//! Command-line argument parsing and validation

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Arch Package Builder - A reliable tool for building Arch Linux packages
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(name = "builder")]
pub struct Args {
    /// Enable debug output
    #[arg(long, global = true)]
    pub debug: bool,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Command,
}

/// Available commands
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Parse PKGBUILD and install dependencies
    Deps,
    
    /// Build the package using paru
    Build {
        /// Clean previous build artifacts before building
        #[arg(long)]
        clean: bool,
        
        /// Sign the package using GPG
        #[arg(long)]
        sign: bool,
    },
    
    /// Collect build artifacts
    Artifacts {
        /// Output directory for artifacts
        #[arg(short = 'o', long = "output-dir", default_value = "artifacts")]
        output_dir: PathBuf,
    },
    
    /// Generate version information file
    Version {
        /// Output file for version information
        #[arg(short = 'o', long = "output-file", default_value = "version.env")]
        output_file: PathBuf,
    },
}

/// Parse command line arguments
pub fn parse_args() -> Args {
    Args::parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_basic_args() {
        let args = Args::try_parse_from(["builder", "deps"]).unwrap();
        assert!(!args.debug);
        assert!(matches!(args.command, Command::Deps));
    }
    
    #[test]
    fn test_parse_debug_flag() {
        let args = Args::try_parse_from(["builder", "--debug", "deps"]).unwrap();
        assert!(args.debug);
    }
    
    #[test]
    fn test_parse_build_with_options() {
        let args = Args::try_parse_from(["builder", "build", "--clean", "--sign"]).unwrap();
        match args.command {
            Command::Build { clean, sign } => {
                assert!(clean);
                assert!(sign);
            }
            _ => panic!("Expected Build command"),
        }
    }
}