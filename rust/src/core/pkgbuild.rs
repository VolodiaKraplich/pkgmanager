//! PKGBUILD file parsing functionality
//!
//! Safely parses PKGBUILD files without executing shell code.

use crate::error::{BuilderError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, instrument};

/// Information extracted from a PKGBUILD file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[derive(Default)]
pub struct PkgbuildInfo {
    /// Package name
    pub name: String,
    /// Package version
    pub version: String,
    /// Package release number
    pub release: String,
    /// Supported architectures
    pub arch: Vec<String>,
    /// Runtime dependencies
    pub depends: Vec<String>,
    /// Build-time dependencies
    pub make_depends: Vec<String>,
    /// Test dependencies
    pub check_depends: Vec<String>,
}

impl PkgbuildInfo {
    /// Create a new empty PKGBUILD info structure
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Get the full package version (version-release)
    pub fn full_version(&self) -> String {
        format!("{}-{}", self.version, self.release)
    }
    
    /// Get all dependencies combined
    pub fn all_dependencies(&self) -> Vec<String> {
        let mut deps = self.depends.clone();
        deps.extend(self.make_depends.clone());
        deps.extend(self.check_depends.clone());
        deps
    }
    
    /// Check if the package has any dependencies
    pub fn has_dependencies(&self) -> bool {
        !self.depends.is_empty() || !self.make_depends.is_empty() || !self.check_depends.is_empty()
    }
}


/// PKGBUILD parser with support for various variable assignment patterns
pub struct PkgbuildParser {
    /// Regex for double-quoted variables
    re_double_quoted: Regex,
    /// Regex for single-quoted variables
    re_single_quoted: Regex,
    /// Regex for unquoted variables
    re_unquoted: Regex,
    /// Regex for array variables
    re_array: Regex,
    /// Regex for comment removal
    re_comment: Regex,
    /// Regex for simple fallback parsing
    re_simple: Regex,
}

impl PkgbuildParser {
    /// Create a new PKGBUILD parser
    pub fn new() -> Result<Self> {
        Ok(Self {
            re_double_quoted: Regex::new(r#"(?m)^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*"([^"\n#]*?)"\s*(?:#.*)?$"#)
                .map_err(|e| BuilderError::config(format!("Failed to compile regex: {}", e)))?,
            re_single_quoted: Regex::new(r#"(?m)^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*'([^'\n#]*?)'\s*(?:#.*)?$"#)
                .map_err(|e| BuilderError::config(format!("Failed to compile regex: {}", e)))?,
            re_unquoted: Regex::new(r#"(?m)^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*([^'"\n#]+?)\s*(?:#.*)?$"#)
                .map_err(|e| BuilderError::config(format!("Failed to compile regex: {}", e)))?,
            re_array: Regex::new(r#"(?ms)^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*\(\s*(.*?)\s*\)"#)
                .map_err(|e| BuilderError::config(format!("Failed to compile regex: {}", e)))?,
            re_comment: Regex::new(r#"(?m)#.*$"#)
                .map_err(|e| BuilderError::config(format!("Failed to compile regex: {}", e)))?,
            re_simple: Regex::new(r#"(?m)^([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(.*)$"#)
                .map_err(|e| BuilderError::config(format!("Failed to compile regex: {}", e)))?,
        })
    }

    /// Parse a PKGBUILD file and extract package information
    #[instrument(skip(self))]
    pub fn parse<P: AsRef<Path> + std::fmt::Debug>(&self, path: P) -> Result<PkgbuildInfo> {
        let path = path.as_ref();
        debug!("Parsing PKGBUILD file: {}", path.display());

        let content = std::fs::read_to_string(path)
            .map_err(|e| BuilderError::file_system("read", path.to_path_buf(), e))?;

        let mut info = PkgbuildInfo::new();

        // Log first few lines for debugging
        let lines: Vec<&str> = content.lines().collect();
        debug!("PKGBUILD has {} lines", lines.len());
        for (i, line) in lines.iter().take(15).enumerate() {
            debug!("{:2}: {}", i + 1, line);
        }

        // Parse single-value variables
        self.parse_single_variables(&content, &mut info)?;
        
        // Parse array variables
        self.parse_array_variables(&content, &mut info)?;

        // Fallback parsing if required fields are missing
        if info.name.is_empty() || info.version.is_empty() || info.release.is_empty() {
            debug!("Primary parsing incomplete, trying fallback method");
            self.fallback_parse(&content, &mut info)?;
        }

        debug!(
            "Parsed PKGBUILD: name='{}', version='{}', release='{}'",
            info.name, info.version, info.release
        );

        self.validate_info(&info, path)?;
        Ok(info)
    }

    /// Parse single-value variables (pkgname, pkgver, pkgrel)
    fn parse_single_variables(&self, content: &str, info: &mut PkgbuildInfo) -> Result<()> {
        // Process different quote types
        self.process_variable_matches(&self.re_double_quoted, content, info, 2)?;
        self.process_variable_matches(&self.re_single_quoted, content, info, 2)?;
        self.process_variable_matches(&self.re_unquoted, content, info, 2)?;
        Ok(())
    }

    /// Parse array variables (arch, depends, makedepends, checkdepends)
    fn parse_array_variables(&self, content: &str, info: &mut PkgbuildInfo) -> Result<()> {
        let array_matches: Vec<_> = self.re_array.captures_iter(content).collect();
        debug!("Found {} array variable matches", array_matches.len());

        for cap in array_matches {
            if let (Some(key_match), Some(val_match)) = (cap.get(1), cap.get(2)) {
                let key = key_match.as_str().trim();
                let val = val_match.as_str();

                let cleaned_array = self.clean_array_content(val)?;
                debug!("Parsed array {}: {:?}", key, cleaned_array);

                match key {
                    "arch" => info.arch = cleaned_array,
                    "depends" => info.depends = cleaned_array,
                    "makedepends" => info.make_depends = cleaned_array,
                    "checkdepends" => info.check_depends = cleaned_array,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Process variable matches for a given regex
    fn process_variable_matches(
        &self,
        regex: &Regex,
        content: &str,
        info: &mut PkgbuildInfo,
        value_index: usize,
    ) -> Result<()> {
        let matches: Vec<_> = regex.captures_iter(content).collect();
        debug!("Found {} variable matches", matches.len());

        for cap in matches {
            if let (Some(key_match), Some(val_match)) = (cap.get(1), cap.get(value_index)) {
                let key = key_match.as_str().trim();
                let val = val_match.as_str().trim();

                debug!("Found variable: {} = '{}'", key, val);

                match key {
                    "pkgname" if info.name.is_empty() => info.name = val.to_string(),
                    "pkgver" if info.version.is_empty() => info.version = val.to_string(),
                    "pkgrel" if info.release.is_empty() => info.release = val.to_string(),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Clean array content by removing comments, quotes, and normalizing whitespace
    fn clean_array_content(&self, content: &str) -> Result<Vec<String>> {
        // Remove comments
        let cleaned = self.re_comment.replace_all(content, "");
        
        // Normalize whitespace and remove quotes
        let normalized = cleaned
            .replace(['\n', '\t'], " ")
            .replace(['\'', '"'], "")
            .replace("  ", " ");

        // Split and filter
        let fields: Vec<String> = normalized
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        Ok(fields)
    }

    /// Fallback parsing with simpler regex
    fn fallback_parse(&self, content: &str, info: &mut PkgbuildInfo) -> Result<()> {
        let simple_matches: Vec<_> = self.re_simple.captures_iter(content).collect();
        debug!("Fallback found {} matches", simple_matches.len());

        for cap in simple_matches {
            if let (Some(key_match), Some(val_match)) = (cap.get(1), cap.get(2)) {
                let key = key_match.as_str().trim();
                let mut val = val_match.as_str().trim();

                // Clean up the value
                let cleaned_val = self.re_comment.replace_all(val, "");
                val = cleaned_val.trim().trim_matches(|c| c == '"' || c == '\'');

                debug!("Fallback found: {} = '{}'", key, val);

                match key {
                    "pkgname" if info.name.is_empty() => info.name = val.to_string(),
                    "pkgver" if info.version.is_empty() => info.version = val.to_string(),
                    "pkgrel" if info.release.is_empty() => info.release = val.to_string(),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Validate that required fields are present
    fn validate_info(&self, info: &PkgbuildInfo, path: &Path) -> Result<()> {
        if info.name.is_empty() || info.version.is_empty() || info.release.is_empty() {
            return Err(BuilderError::pkgbuild_parse(
                format!(
                    "Missing required variables. Found: pkgname='{}', pkgver='{}', pkgrel='{}'. \
                     This suggests the PKGBUILD format is unusual or contains complex variable assignments.",
                    info.name, info.version, info.release
                ),
                path,
            ));
        }
        Ok(())
    }
}

impl Default for PkgbuildParser {
    fn default() -> Self {
        Self::new().expect("Failed to create default PKGBUILD parser")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    fn create_test_pkgbuild(content: &str) -> NamedTempFile {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), content).unwrap();
        file
    }

    #[test]
    fn test_parse_simple_pkgbuild() {
        let content = r#"
pkgname="test-package"
pkgver="1.0.0"
pkgrel=1
arch=('x86_64')
depends=('glibc')
makedepends=('gcc' 'make')
"#;
        let file = create_test_pkgbuild(content);
        let parser = PkgbuildParser::new().unwrap();
        let info = parser.parse(file.path()).unwrap();

        assert_eq!(info.name, "test-package");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.release, "1");
        assert_eq!(info.arch, vec!["x86_64"]);
        assert_eq!(info.depends, vec!["glibc"]);
        assert_eq!(info.make_depends, vec!["gcc", "make"]);
    }

    #[test]
    fn test_parse_complex_arrays() {
        let content = r#"
pkgname=complex-package
pkgver=2.1.0
pkgrel=3
arch=('x86_64' 'aarch64')
depends=(
    'dep1'
    'dep2>=1.0'  # Comment here
    'dep3'
)
makedepends=('build-dep1' 'build-dep2')
"#;
        let file = create_test_pkgbuild(content);
        let parser = PkgbuildParser::new().unwrap();
        let info = parser.parse(file.path()).unwrap();

        assert_eq!(info.arch, vec!["x86_64", "aarch64"]);
        assert_eq!(info.depends, vec!["dep1", "dep2>=1.0", "dep3"]);
        assert_eq!(info.make_depends, vec!["build-dep1", "build-dep2"]);
    }

    #[test]
    fn test_full_version() {
        let info = PkgbuildInfo {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            release: "1".to_string(),
            ..Default::default()
        };
        assert_eq!(info.full_version(), "1.0.0-1");
    }

    #[test]
    fn test_all_dependencies() {
        let info = PkgbuildInfo {
            depends: vec!["dep1".to_string()],
            make_depends: vec!["makedep1".to_string()],
            check_depends: vec!["checkdep1".to_string()],
            ..Default::default()
        };
        let all_deps = info.all_dependencies();
        assert_eq!(all_deps.len(), 3);
        assert!(all_deps.contains(&"dep1".to_string()));
        assert!(all_deps.contains(&"makedep1".to_string()));
        assert!(all_deps.contains(&"checkdep1".to_string()));
    }
}