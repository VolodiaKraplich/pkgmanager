//! Core functionality for package building
//!
//! Contains the main logic for parsing PKGBUILD files, building packages,
//! and collecting artifacts.

pub mod artifacts;
pub mod builder;
pub mod pkgbuild;

pub use artifacts::ArtifactCollector;
pub use builder::PackageBuilder;
pub use pkgbuild::{PkgbuildInfo, PkgbuildParser};
