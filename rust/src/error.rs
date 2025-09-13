//! Error types for the package builder
//!
//! Provides structured error handling with context and proper error chains.

use std::path::PathBuf;
use thiserror::Error;

/// Main error type for the package builder
#[derive(Error, Debug)]
pub enum BuilderError {
    /// Errors related to PKGBUILD file parsing
    #[error("PKGBUILD parsing error: {message}")]
    PkgbuildParse {
        message: String,
        path: PathBuf,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Errors related to dependency management
    #[error("Dependency error: {message}")]
    Dependency {
        message: String,
        dependencies: Vec<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Errors related to package building
    #[error("Build error: {message}")]
    Build {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Errors related to artifact collection
    #[error("Artifact error: {message}")]
    Artifact {
        message: String,
        path: PathBuf,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// File system operation errors
    #[error("File system error: {operation} failed on {path}")]
    FileSystem {
        operation: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Process execution errors
    #[error("Process error: {command} failed")]
    Process {
        command: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Config {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Validation errors
    #[error("Validation error: {message}")]
    Validation { message: String },
}

impl BuilderError {
    /// Create a new PKGBUILD parsing error
    pub fn pkgbuild_parse<P: Into<PathBuf>>(
        message: impl Into<String>,
        path: P,
    ) -> Self {
        Self::PkgbuildParse {
            message: message.into(),
            path: path.into(),
            source: None,
        }
    }

    /// Create a new dependency error
    pub fn dependency(
        message: impl Into<String>,
        dependencies: Vec<String>,
    ) -> Self {
        Self::Dependency {
            message: message.into(),
            dependencies,
            source: None,
        }
    }

    /// Create a new build error
    pub fn build(message: impl Into<String>) -> Self {
        Self::Build {
            message: message.into(),
            source: None,
        }
    }

    /// Create a new artifact error
    pub fn artifact<P: Into<PathBuf>>(
        message: impl Into<String>,
        path: P,
    ) -> Self {
        Self::Artifact {
            message: message.into(),
            path: path.into(),
            source: None,
        }
    }

    /// Create a new file system error
    pub fn file_system<P: Into<PathBuf>>(
        operation: impl Into<String>,
        path: P,
        source: std::io::Error,
    ) -> Self {
        Self::FileSystem {
            operation: operation.into(),
            path: path.into(),
            source,
        }
    }

    /// Create a new process error
    pub fn process(
        command: impl Into<String>,
        exit_code: Option<i32>,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
    ) -> Self {
        Self::Process {
            command: command.into(),
            exit_code,
            stdout: stdout.into(),
            stderr: stderr.into(),
            source: None,
        }
    }

    /// Create a new configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            source: None,
        }
    }

    /// Create a new validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, BuilderError>;