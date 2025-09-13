//! Artifact collection functionality
//!
//! Handles collecting build artifacts like packages, logs, and source files.

use crate::{
    config::Config,
    error::{BuilderError, Result},
    utils::fs::FileSystemUtils,
};
use std::path::{Path, PathBuf};
use tracing::{debug, info, instrument, warn};

/// Artifact collector that gathers build outputs
pub struct ArtifactCollector {
    config: Config,
    fs_utils: FileSystemUtils,
}

/// Information about collected artifacts
#[derive(Debug)]
pub struct CollectedArtifact {
    /// Original file path
    pub source: PathBuf,
    /// Destination path
    pub destination: PathBuf,
    /// Whether the file was copied or moved
    pub operation: ArtifactOperation,
}

/// Type of operation performed on an artifact
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactOperation {
    /// File was copied (source preserved)
    Copied,
    /// File was moved (source removed)
    Moved,
}

impl ArtifactCollector {
    /// Create a new artifact collector
    pub fn new(config: Config) -> Self {
        Self {
            fs_utils: FileSystemUtils::new(),
            config,
        }
    }

    /// Collect all artifacts according to configuration
    #[instrument(skip(self))]
    pub fn collect(&self) -> Result<Vec<CollectedArtifact>> {
        info!(
            "Collecting artifacts to: {}",
            self.config.artifacts.output_dir.display()
        );

        // Ensure output directory exists
        self.fs_utils
            .create_dir_all(&self.config.artifacts.output_dir)
            .map_err(|e| {
                BuilderError::artifact(
                    format!("Failed to create artifacts directory: {}", e),
                    &self.config.artifacts.output_dir,
                )
            })?;

        let mut collected = Vec::new();
        let mut found_packages = false;

        // Collect files for each pattern
        for pattern in &self.config.artifacts.patterns {
            let artifacts = self.collect_pattern(pattern)?;

            // Check if we found any package files
            if pattern.contains(".pkg.tar.") && !artifacts.is_empty() {
                found_packages = true;
            }

            collected.extend(artifacts);
        }

        // Validate that we collected at least some package files
        if !found_packages {
            return Err(BuilderError::artifact(
                "No package files (*.pkg.tar.*) were found to collect",
                &self.config.artifacts.output_dir,
            ));
        }

        info!("Successfully collected {} artifacts", collected.len());
        Ok(collected)
    }

    /// Collect files matching a specific pattern
    #[instrument(skip(self))]
    fn collect_pattern(&self, pattern: &str) -> Result<Vec<CollectedArtifact>> {
        debug!("Collecting files matching pattern: {}", pattern);
        let mut artifacts = Vec::new();

        // Handle different pattern types
        let files = match pattern {
            "*.pkg.tar.*" => self.find_package_files()?,
            "*.log" => self.find_log_files()?,
            "PKGBUILD" => self.find_exact_file("PKGBUILD")?,
            ".SRCINFO" => self.find_exact_file(".SRCINFO")?,
            _ => self.find_glob_pattern(pattern)?,
        };

        for file_path in files {
            let artifact = self.collect_file(&file_path, pattern)?;
            artifacts.push(artifact);
        }

        debug!(
            "Collected {} files for pattern {}",
            artifacts.len(),
            pattern
        );
        Ok(artifacts)
    }

    /// Find package files (*.pkg.tar.*)
    fn find_package_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for pattern in &["*.pkg.tar.xz", "*.pkg.tar.zst", "*.pkg.tar.gz"] {
            if let Ok(paths) = glob::glob(pattern) {
                for path in paths.flatten() {
                    files.push(path);
                }
            }
        }

        Ok(files)
    }

    /// Find log files
    fn find_log_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        if let Ok(paths) = glob::glob("*.log") {
            for path in paths.flatten() {
                files.push(path);
            }
        }
        Ok(files)
    }

    /// Find an exact file by name
    fn find_exact_file(&self, filename: &str) -> Result<Vec<PathBuf>> {
        let path = PathBuf::from(filename);
        if path.exists() {
            Ok(vec![path])
        } else {
            Ok(vec![])
        }
    }

    /// Find files using glob pattern
    fn find_glob_pattern(&self, pattern: &str) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        match glob::glob(pattern) {
            Ok(paths) => {
                for path_result in paths {
                    match path_result {
                        Ok(path) => files.push(path),
                        Err(e) => warn!("Error reading path for pattern {}: {}", pattern, e),
                    }
                }
            }
            Err(e) => {
                warn!("Invalid glob pattern {}: {}", pattern, e);
            }
        }

        Ok(files)
    }

    /// Collect a single file
    #[instrument(skip(self))]
    fn collect_file(&self, file_path: &Path, pattern: &str) -> Result<CollectedArtifact> {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| BuilderError::artifact("Invalid file name", file_path.to_path_buf()))?;

        let destination = self.config.artifacts.output_dir.join(file_name);

        // Determine operation type based on file type and configuration
        let operation = if self.should_copy_file(file_path, pattern) {
            ArtifactOperation::Copied
        } else {
            ArtifactOperation::Moved
        };

        // Perform the operation
        match operation {
            ArtifactOperation::Copied => {
                self.fs_utils
                    .copy_file(file_path, &destination)
                    .map_err(|e| {
                        BuilderError::artifact(
                            format!("Failed to copy {}: {}", file_name, e),
                            file_path.to_path_buf(),
                        )
                    })?;
                info!(
                    "  Copied: {} -> {}",
                    file_path.display(),
                    destination.display()
                );
            }
            ArtifactOperation::Moved => {
                self.fs_utils
                    .move_file(file_path, &destination)
                    .map_err(|e| {
                        BuilderError::artifact(
                            format!("Failed to move {}: {}", file_name, e),
                            file_path.to_path_buf(),
                        )
                    })?;
                info!(
                    "  Moved: {} -> {}",
                    file_path.display(),
                    destination.display()
                );
            }
        }

        Ok(CollectedArtifact {
            source: file_path.to_path_buf(),
            destination,
            operation,
        })
    }

    /// Determine if a file should be copied (vs moved)
    fn should_copy_file(&self, file_path: &Path, pattern: &str) -> bool {
        // Always copy PKGBUILD and other source files if preserve_sources is enabled
        if self.config.artifacts.preserve_sources {
            match file_path.file_name().and_then(|n| n.to_str()) {
                Some("PKGBUILD") | Some(".SRCINFO") => return true,
                _ => {}
            }
        }

        // Copy files that match certain patterns if configured
        if pattern == "PKGBUILD" || pattern == ".SRCINFO" {
            return self.config.artifacts.preserve_sources;
        }

        // Default to moving files
        false
    }

    /// Get summary of collected artifacts by type
    pub fn get_collection_summary(&self, artifacts: &[CollectedArtifact]) -> CollectionSummary {
        let mut summary = CollectionSummary::default();

        for artifact in artifacts {
            if let Some(file_name) = artifact.source.file_name().and_then(|n| n.to_str()) {
                if file_name.contains(".pkg.tar.") {
                    summary.packages += 1;
                } else if file_name.ends_with(".log") {
                    summary.logs += 1;
                } else if file_name == "PKGBUILD" || file_name == ".SRCINFO" {
                    summary.sources += 1;
                } else {
                    summary.others += 1;
                }

                match artifact.operation {
                    ArtifactOperation::Copied => summary.copied += 1,
                    ArtifactOperation::Moved => summary.moved += 1,
                }
            }
        }

        summary.total = artifacts.len();
        summary
    }
}

/// Summary of artifact collection results
#[derive(Debug, Default)]
pub struct CollectionSummary {
    /// Total number of artifacts collected
    pub total: usize,
    /// Number of package files
    pub packages: usize,
    /// Number of log files
    pub logs: usize,
    /// Number of source files (PKGBUILD, .SRCINFO)
    pub sources: usize,
    /// Number of other files
    pub others: usize,
    /// Number of files copied
    pub copied: usize,
    /// Number of files moved
    pub moved: usize,
}

impl std::fmt::Display for CollectionSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Collected {} artifacts: {} packages, {} logs, {} sources, {} others ({} copied, {} moved)",
            self.total,
            self.packages,
            self.logs,
            self.sources,
            self.others,
            self.copied,
            self.moved
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config(temp_dir: &TempDir) -> Config {
        let mut config = Config::default();
        config.artifacts.output_dir = temp_dir.path().join("artifacts");
        config.artifacts.preserve_sources = true;
        config
    }

    #[test]
    fn test_artifact_collector_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let collector = ArtifactCollector::new(config);

        assert!(collector.config.artifacts.preserve_sources);
    }

    #[test]
    fn test_should_copy_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let collector = ArtifactCollector::new(config);

        let pkgbuild_path = Path::new("PKGBUILD");
        let package_path = Path::new("test-1.0.0-1-x86_64.pkg.tar.zst");

        assert!(collector.should_copy_file(pkgbuild_path, "PKGBUILD"));
        assert!(!collector.should_copy_file(package_path, "*.pkg.tar.*"));
    }

    #[test]
    fn test_find_exact_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let collector = ArtifactCollector::new(config);

        // Create a test file
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Change to temp directory for the test
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let found = collector.find_exact_file("test.txt").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], Path::new("test.txt"));

        let not_found = collector.find_exact_file("nonexistent.txt").unwrap();
        assert_eq!(not_found.len(), 0);

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_collection_summary() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let collector = ArtifactCollector::new(config);

        let artifacts = vec![
            CollectedArtifact {
                source: PathBuf::from("test-1.0.0-1.pkg.tar.zst"),
                destination: PathBuf::from("artifacts/test-1.0.0-1.pkg.tar.zst"),
                operation: ArtifactOperation::Moved,
            },
            CollectedArtifact {
                source: PathBuf::from("PKGBUILD"),
                destination: PathBuf::from("artifacts/PKGBUILD"),
                operation: ArtifactOperation::Copied,
            },
            CollectedArtifact {
                source: PathBuf::from("build.log"),
                destination: PathBuf::from("artifacts/build.log"),
                operation: ArtifactOperation::Moved,
            },
        ];

        let summary = collector.get_collection_summary(&artifacts);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.packages, 1);
        assert_eq!(summary.logs, 1);
        assert_eq!(summary.sources, 1);
        assert_eq!(summary.copied, 1);
        assert_eq!(summary.moved, 2);
    }
}
