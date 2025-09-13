//! Package building functionality
//!
//! Handles dependency installation and package building with paru/pacman.

use crate::{
    config::Config,
    core::pkgbuild::PkgbuildInfo,
    error::{BuilderError, Result},
    utils::process::ProcessRunner,
};
use std::path::PathBuf;
use tracing::{debug, info, instrument, warn};

/// Package builder that handles dependencies and compilation
pub struct PackageBuilder {
    config: Config,
    process_runner: ProcessRunner,
}

impl PackageBuilder {
    /// Create a new package builder with the given configuration
    #[must_use]
    pub const fn new(config: Config) -> Self {
        Self {
            process_runner: ProcessRunner::new(config.debug),
            config,
        }
    }

    /// Install dependencies for the given PKGBUILD
    #[instrument(skip(self, pkgbuild))]
    pub fn install_dependencies(&mut self, pkgbuild: &PkgbuildInfo) -> Result<()> {
        let all_deps = pkgbuild.all_dependencies();

        if all_deps.is_empty() {
            info!("No dependencies found in PKGBUILD");
            return Ok(());
        }

        info!("Found {} dependencies: {:?}", all_deps.len(), all_deps);

        // Handle rust/rustup conflicts if enabled
        let filtered_deps = if self.config.package_manager.handle_rust_conflict {
            self.handle_rust_conflict(all_deps)
        } else {
            all_deps
        };

        if filtered_deps.is_empty() {
            info!("All dependencies are already satisfied");
            return Ok(());
        }

        self.install_package_list(&filtered_deps)
    }

    /// Handle rust/rustup package conflicts
    fn handle_rust_conflict(&self, deps: Vec<String>) -> Vec<String> {
        let mut has_rust = false;
        let mut has_rustup = false;
        let mut filtered_deps = Vec::new();

        for dep in deps {
            match dep.as_str() {
                "rust" => has_rust = true,
                "rustup" => has_rustup = true,
                _ => filtered_deps.push(dep),
            }
        }

        if has_rust || has_rustup {
            // Check if rustup is already available
            if self.process_runner.command_exists("rustup") {
                info!("rustup is already available, skipping rust package");
                // Remove cargo since rustup includes it
                filtered_deps.retain(|dep| dep != "cargo");
            } else {
                info!("Installing rustup for Rust toolchain");
                filtered_deps.push("rustup".to_string());
            }
        }

        filtered_deps
    }

    /// Install a list of packages using the configured package manager
    #[instrument(skip(self, packages))]
    fn install_package_list(&self, packages: &[String]) -> Result<()> {
        let (cmd, mut args) = self.config.get_package_manager_cmd();
        args.extend_from_slice(packages);

        info!("Installing packages with {}: {:?}", cmd, packages);

        // Try primary package manager first
        let args_str: Vec<&str> = args.iter().map(String::as_str).collect();
        match self.process_runner.run_command(&cmd, &args_str) {
            Ok(()) => {
                info!("Successfully installed dependencies with {}", cmd);
                Ok(())
            }
            Err(e) => {
                warn!("Primary package manager {} failed: {}", cmd, e);

                // Try fallback if configured
                self.config.package_manager.fallback.as_ref().map_or_else(
                    || {
                        Err(BuilderError::dependency(
                            format!("Failed to install dependencies with {cmd}"),
                            packages.to_vec(),
                        ))
                    },
                    |fallback| {
                        info!("Trying fallback package manager: {fallback}");
                        self.try_fallback_installation(fallback, packages)
                    },
                )
            }
        }
    }

    /// Try installing with fallback package manager (usually pacman with sudo)
    fn try_fallback_installation(&self, fallback: &str, packages: &[String]) -> Result<()> {
        let mut args = vec![fallback];
        args.extend(
            self.config
                .package_manager
                .install_args
                .iter()
                .map(String::as_str),
        );
        let package_strs: Vec<&str> = packages.iter().map(String::as_str).collect();
        args.extend(package_strs);

        match self.process_runner.run_command("sudo", &args) {
            Ok(()) => {
                info!("Successfully installed dependencies with {}", fallback);
                Ok(())
            }
            Err(e) => {
                warn!("Fallback installation failed: {}", e);
                Err(BuilderError::dependency(
                    "All package managers failed to install dependencies".to_string(),
                    packages.to_vec(),
                ))
            }
        }
    }

    /// Clean previous build artifacts
    #[instrument(skip(self))]
    pub fn clean(&self) -> Result<()> {
        info!("Cleaning previous build artifacts");

        // Remove package files
        let pkg_pattern = "*.pkg.tar.*";
        if let Ok(paths) = glob::glob(pkg_pattern) {
            for path in paths.flatten() {
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("Failed to remove {}: {}", path.display(), e);
                } else {
                    debug!("Removed package file: {}", path.display());
                }
            }
        }

        // Remove build directories
        for dir in &["src", "pkg"] {
            if let Err(e) = std::fs::remove_dir_all(dir) {
                debug!("Could not remove directory {} (may not exist): {}", dir, e);
            } else {
                debug!("Removed directory: {}", dir);
            }
        }

        Ok(())
    }

    /// Build the package using paru
    #[instrument(skip(self, pkgbuild))]
    pub fn build(&self, pkgbuild: &PkgbuildInfo) -> Result<Vec<PathBuf>> {
        info!("Building package: {}", pkgbuild.name);

        let (cmd, args) = self.config.get_build_cmd();
        let args_str: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut env_vars = Vec::new();

        // Set ccache environment if enabled
        if self.config.build.use_ccache {
            env_vars.push((
                "CCACHE_DIR".to_string(),
                self.config.build.ccache_dir.to_string_lossy().to_string(),
            ));
        }

        // Execute build command
        self.process_runner
            .run_command_with_env(&cmd, &args_str, &env_vars)
            .map_err(|e| BuilderError::build(format!("Package build failed: {e}")))?;

        // Find generated package files
        let package_files = Self::find_package_files()?;

        if package_files.is_empty() {
            return Err(BuilderError::build(
                "No package files (*.pkg.tar.*) were generated by the build process.\n\n\
                This usually means:\n\
                • The build was skipped (e.g. due to existing src/ or pkg/ directories)\n\
                • The PKGBUILD has a conditional 'exit 0'\n\
                • The build failed silently (check logs above)\n\
                • Dynamic pkgver/pkgrel caused unexpected naming\n\n\
                Please review the build output carefully for warnings or skipped steps.",
            ));
        }

        info!(
            "Build completed successfully. Generated {} package(s)",
            package_files.len()
        );

        // List generated files for verification
        self.list_package_files(&package_files);

        Ok(package_files)
    }

    /// Find generated package files
    fn find_package_files() -> Result<Vec<PathBuf>> {
        let mut package_files = Vec::new();
        let pkg_pattern = "*.pkg.tar.*";

        match glob::glob(pkg_pattern) {
            Ok(paths) => {
                for path_result in paths {
                    match path_result {
                        Ok(path) => {
                            debug!("Found package file: {}", path.display());
                            package_files.push(path);
                        }
                        Err(e) => warn!("Error reading package file path: {}", e),
                    }
                }
            }
            Err(e) => {
                return Err(BuilderError::build(format!(
                    "Failed to search for package files: {e}"
                )));
            }
        }

        // Sort for consistent output
        package_files.sort();
        Ok(package_files)
    }

    /// List package files with details
    fn list_package_files(&self, package_files: &[PathBuf]) {
        let mut args = vec!["-la"];
        let file_strs: Vec<&str> = package_files.iter().filter_map(|p| p.to_str()).collect();
        args.extend(file_strs);

        if let Err(e) = self.process_runner.run_command("ls", &args) {
            warn!("Could not list package files: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BuildConfig, PackageManagerConfig};

    fn create_test_config() -> Config {
        Config {
            debug: true,
            work_dir: std::env::current_dir().unwrap(),
            pkgbuild_path: "PKGBUILD".into(),
            package_manager: PackageManagerConfig::default(),
            build: BuildConfig::default(),
            artifacts: crate::config::ArtifactConfig::default(),
        }
    }

    #[test]
    fn test_handle_rust_conflict() {
        let config = create_test_config();
        let builder = PackageBuilder::new(config);

        let deps = vec![
            "rust".to_string(),
            "cargo".to_string(),
            "other-dep".to_string(),
        ];

        let filtered = builder.handle_rust_conflict(deps);

        // Should either have rustup or remove cargo if rustup exists
        assert!(filtered.contains(&"other-dep".to_string()));
        assert!(
            filtered.contains(&"rustup".to_string()) || !filtered.contains(&"cargo".to_string())
        );
    }

    #[test]
    fn test_builder_creation() {
        let config = create_test_config();
        let builder = PackageBuilder::new(config);
        assert!(!builder.config.build.clean);
    }
}
