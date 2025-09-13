//! Environment and version handling utilities
//!
//! Provides functionality for generating version information and handling
//! environment variables.

use crate::{core::pkgbuild::PkgbuildInfo, error::Result, utils::fs::FileSystemUtils};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, path::Path};
use tracing::{debug, info, instrument};

/// Version information generator for GitLab CI integration
#[derive(Debug)]
pub struct VersionGenerator {
    fs_utils: FileSystemUtils,
}

/// Complete version information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Package version
    pub version: String,
    /// Package release number
    pub pkg_release: String,
    /// Full version string (version-release)
    pub full_version: String,
    /// Package name
    pub package_name: String,
    /// Git tag version (from CI_COMMIT_TAG or fallback to version)
    pub tag_version: String,
    /// Build job ID (from CI_JOB_ID or "local")
    pub build_job_id: String,
    /// Build timestamp in RFC3339 format
    pub build_date: String,
    /// Supported architectures
    pub arch: String,
}

impl VersionGenerator {
    /// Create a new version generator
    pub fn new() -> Self {
        Self {
            fs_utils: FileSystemUtils::new(),
        }
    }

    /// Generate version information file from PKGBUILD
    #[instrument(skip(self, pkgbuild, output_file))]
    pub fn generate<P: AsRef<Path>>(
        &self,
        pkgbuild: &PkgbuildInfo,
        output_file: P,
    ) -> Result<VersionInfo> {
        let output_file = output_file.as_ref();
        info!(
            "Generating version information to: {}",
            output_file.display()
        );

        let version_info = self.create_version_info(pkgbuild)?;
        let env_content = self.format_as_env_file(&version_info)?;

        self.fs_utils
            .write_file(output_file, env_content.as_bytes())
            .map_err(|e| {
                crate::error::BuilderError::file_system("write", output_file.to_path_buf(), e)
            })?;

        info!("Version information generated successfully");
        debug!("Generated version info: {:?}", version_info);

        Ok(version_info)
    }

    /// Create version information from PKGBUILD and environment
    fn create_version_info(&self, pkgbuild: &PkgbuildInfo) -> Result<VersionInfo> {
        let ci_commit_tag = env::var("CI_COMMIT_TAG").unwrap_or_else(|_| pkgbuild.version.clone());
        let ci_job_id = env::var("CI_JOB_ID").unwrap_or_else(|_| "local".to_string());
        let build_date = Utc::now().to_rfc3339();

        let version_info = VersionInfo {
            version: pkgbuild.version.clone(),
            pkg_release: pkgbuild.release.clone(),
            full_version: pkgbuild.full_version(),
            package_name: pkgbuild.name.clone(),
            tag_version: ci_commit_tag,
            build_job_id: ci_job_id,
            build_date,
            arch: pkgbuild.arch.join(" "),
        };

        Ok(version_info)
    }

    /// Format version information as environment file (.env format)
    fn format_as_env_file(&self, info: &VersionInfo) -> Result<String> {
        let content = format!(
            r#"VERSION={}
PKG_RELEASE={}
FULL_VERSION={}
PACKAGE_NAME={}
TAG_VERSION={}
BUILD_JOB_ID={}
BUILD_DATE={}
ARCH="{}"
"#,
            info.version,
            info.pkg_release,
            info.full_version,
            info.package_name,
            info.tag_version,
            info.build_job_id,
            info.build_date,
            info.arch
        );

        Ok(content)
    }

    /// Load version information from an existing file
    #[instrument(skip(self, file_path))]
    pub fn load_from_file<P: AsRef<Path>>(&self, file_path: P) -> Result<VersionInfo> {
        let file_path = file_path.as_ref();
        debug!("Loading version information from: {}", file_path.display());

        let content = self.fs_utils.read_file_to_string(file_path).map_err(|e| {
            crate::error::BuilderError::file_system("read", file_path.to_path_buf(), e)
        })?;

        self.parse_env_content(&content)
    }

    /// Parse environment file content into VersionInfo
    fn parse_env_content(&self, content: &str) -> Result<VersionInfo> {
        let mut env_vars = HashMap::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');
                env_vars.insert(key.to_string(), value.to_string());
            }
        }

        let version_info = VersionInfo {
            version: env_vars
                .get("VERSION")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            pkg_release: env_vars
                .get("PKG_RELEASE")
                .cloned()
                .unwrap_or_else(|| "1".to_string()),
            full_version: env_vars
                .get("FULL_VERSION")
                .cloned()
                .unwrap_or_else(|| "unknown-1".to_string()),
            package_name: env_vars
                .get("PACKAGE_NAME")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            tag_version: env_vars
                .get("TAG_VERSION")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            build_job_id: env_vars
                .get("BUILD_JOB_ID")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            build_date: env_vars
                .get("BUILD_DATE")
                .cloned()
                .unwrap_or_else(|| Utc::now().to_rfc3339()),
            arch: env_vars
                .get("ARCH")
                .cloned()
                .unwrap_or_else(|| "any".to_string()),
        };

        Ok(version_info)
    }
}

impl Default for VersionGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Environment variable utilities
#[derive(Debug)]
pub struct EnvUtils;

impl EnvUtils {
    /// Get an environment variable with a default value
    pub fn get_var_or_default(key: &str, default: &str) -> String {
        env::var(key).unwrap_or_else(|_| default.to_string())
    }

    /// Get an environment variable and parse it to a specific type
    pub fn get_var_parsed<T>(key: &str) -> Option<T>
    where
        T: std::str::FromStr,
    {
        env::var(key).ok()?.parse().ok()
    }

    /// Check if running in CI environment
    pub fn is_ci() -> bool {
        env::var("CI").is_ok() || env::var("CONTINUOUS_INTEGRATION").is_ok()
    }

    /// Check if running in GitLab CI
    pub fn is_gitlab_ci() -> bool {
        env::var("GITLAB_CI").is_ok()
    }

    /// Get GitLab CI variables as a map
    pub fn get_gitlab_ci_vars() -> HashMap<String, String> {
        let mut vars = HashMap::new();

        let gitlab_vars = [
            "CI_COMMIT_SHA",
            "CI_COMMIT_SHORT_SHA",
            "CI_COMMIT_REF_NAME",
            "CI_COMMIT_TAG",
            "CI_JOB_ID",
            "CI_JOB_NAME",
            "CI_PIPELINE_ID",
            "CI_PROJECT_NAME",
            "CI_PROJECT_PATH",
            "CI_REGISTRY_IMAGE",
        ];

        for var in &gitlab_vars {
            if let Ok(value) = env::var(var) {
                vars.insert(var.to_string(), value);
            }
        }

        vars
    }

    /// Set environment variable (mainly for testing)
    pub fn set_var<K, V>(key: K, value: V)
    where
        K: AsRef<str>,
        V: AsRef<str>,
    {
        unsafe { env::set_var(key.as_ref(), value.as_ref()) }
    }

    /// Remove environment variable (mainly for testing)
    pub fn remove_var<K: AsRef<str>>(key: K) {
        unsafe { env::remove_var(key.as_ref()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_test_pkgbuild() -> PkgbuildInfo {
        PkgbuildInfo {
            name: "test-package".to_string(),
            version: "1.2.3".to_string(),
            release: "2".to_string(),
            arch: vec!["x86_64".to_string(), "aarch64".to_string()],
            depends: vec![],
            make_depends: vec![],
            check_depends: vec![],
        }
    }

    #[test]
    fn test_version_generator_creation() {
        let generator = VersionGenerator::new();
        assert!(generator.fs_utils.is_dir("."));
    }

    #[test]
    fn test_create_version_info() {
        let generator = VersionGenerator::new();
        let pkgbuild = create_test_pkgbuild();

        // Set test environment variables
        EnvUtils::set_var("CI_COMMIT_TAG", "v1.2.3");
        EnvUtils::set_var("CI_JOB_ID", "12345");

        let version_info = generator.create_version_info(&pkgbuild).unwrap();

        assert_eq!(version_info.version, "1.2.3");
        assert_eq!(version_info.pkg_release, "2");
        assert_eq!(version_info.full_version, "1.2.3-2");
        assert_eq!(version_info.package_name, "test-package");
        assert_eq!(version_info.tag_version, "v1.2.3");
        assert_eq!(version_info.build_job_id, "12345");
        assert_eq!(version_info.arch, "x86_64 aarch64");

        // Clean up
        EnvUtils::remove_var("CI_COMMIT_TAG");
        EnvUtils::remove_var("CI_JOB_ID");
    }

    #[test]
    fn test_format_as_env_file() {
        let generator = VersionGenerator::new();
        let version_info = VersionInfo {
            version: "1.0.0".to_string(),
            pkg_release: "1".to_string(),
            full_version: "1.0.0-1".to_string(),
            package_name: "test".to_string(),
            tag_version: "v1.0.0".to_string(),
            build_job_id: "123".to_string(),
            build_date: "2023-01-01T00:00:00Z".to_string(),
            arch: "x86_64".to_string(),
        };

        let content = generator.format_as_env_file(&version_info).unwrap();

        assert!(content.contains("VERSION=1.0.0"));
        assert!(content.contains("PKG_RELEASE=1"));
        assert!(content.contains("FULL_VERSION=1.0.0-1"));
        assert!(content.contains("PACKAGE_NAME=test"));
        assert!(content.contains("TAG_VERSION=v1.0.0"));
        assert!(content.contains("BUILD_JOB_ID=123"));
        assert!(content.contains("ARCH=\"x86_64\""));
    }

    #[test]
    fn test_generate_and_load() {
        let generator = VersionGenerator::new();
        let pkgbuild = create_test_pkgbuild();
        let temp_file = NamedTempFile::new().unwrap();

        // Generate version file
        let generated_info = generator.generate(&pkgbuild, temp_file.path()).unwrap();

        // Load it back
        let loaded_info = generator.load_from_file(temp_file.path()).unwrap();

        assert_eq!(generated_info.version, loaded_info.version);
        assert_eq!(generated_info.package_name, loaded_info.package_name);
        assert_eq!(generated_info.arch, loaded_info.arch);
    }

    #[test]
    fn test_env_utils() {
        // Test default value
        let value = EnvUtils::get_var_or_default("NONEXISTENT_VAR", "default");
        assert_eq!(value, "default");

        // Test set and get
        EnvUtils::set_var("TEST_VAR", "test_value");
        let value = EnvUtils::get_var_or_default("TEST_VAR", "default");
        assert_eq!(value, "test_value");

        // Test parsing
        EnvUtils::set_var("TEST_NUMBER", "42");
        let number: Option<i32> = EnvUtils::get_var_parsed("TEST_NUMBER");
        assert_eq!(number, Some(42));

        // Clean up
        EnvUtils::remove_var("TEST_VAR");
        EnvUtils::remove_var("TEST_NUMBER");
    }

    #[test]
    fn test_gitlab_ci_detection() {
        // Test without CI vars
        assert!(!EnvUtils::is_ci());
        assert!(!EnvUtils::is_gitlab_ci());

        // Test with CI vars
        EnvUtils::set_var("CI", "true");
        assert!(EnvUtils::is_ci());

        EnvUtils::set_var("GITLAB_CI", "true");
        assert!(EnvUtils::is_gitlab_ci());

        // Clean up
        EnvUtils::remove_var("CI");
        EnvUtils::remove_var("GITLAB_CI");
    }
}
