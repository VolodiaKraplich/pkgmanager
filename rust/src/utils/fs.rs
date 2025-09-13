//! File system utility functions
//!
//! Provides safe file operations with proper error handling.

use std::fs;
use std::io;
use std::path::Path;
use tracing::{debug, instrument};

/// Utility struct for file system operations
#[derive(Debug)]
pub struct FileSystemUtils;

impl FileSystemUtils {
    /// Create a new file system utilities instance
    pub fn new() -> Self {
        Self
    }

    /// Copy a file from source to destination, preserving metadata
    #[instrument(skip(self))]
    pub fn copy_file<P: AsRef<Path> + std::fmt::Debug, Q: AsRef<Path> + std::fmt::Debug>(
        &self,
        src: P,
        dst: Q,
    ) -> io::Result<u64> {
        let src = src.as_ref();
        let dst = dst.as_ref();
        
        debug!("Copying file: {} -> {}", src.display(), dst.display());
        
        // Create parent directories if they don't exist
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Copy the file
        let bytes_copied = fs::copy(src, dst)?;
        
        // Copy permissions
        let metadata = fs::metadata(src)?;
        fs::set_permissions(dst, metadata.permissions())?;
        
        debug!("Successfully copied {} bytes", bytes_copied);
        Ok(bytes_copied)
    }

    /// Move a file from source to destination
    #[instrument(skip(self))]
    pub fn move_file<P: AsRef<Path> + std::fmt::Debug, Q: AsRef<Path> + std::fmt::Debug>(
        &self,
        src: P,
        dst: Q,
    ) -> io::Result<()> {
        let src = src.as_ref();
        let dst = dst.as_ref();
        
        debug!("Moving file: {} -> {}", src.display(), dst.display());
        
        // Create parent directories if they don't exist
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Try to rename first (faster if on same filesystem)
        match fs::rename(src, dst) {
            Ok(()) => {
                debug!("File moved successfully via rename");
                Ok(())
            }
            Err(e) => {
                // If rename fails (e.g., across filesystems), copy and delete
                debug!("Rename failed ({}), trying copy + delete", e);
                self.copy_file(src, dst)?;
                fs::remove_file(src)?;
                debug!("File moved successfully via copy + delete");
                Ok(())
            }
        }
    }

    /// Create directories recursively
    #[instrument(skip(self))]
    pub fn create_dir_all<P: AsRef<Path> + std::fmt::Debug>(&self, path: P) -> io::Result<()> {
        let path = path.as_ref();
        debug!("Creating directory: {}", path.display());
        fs::create_dir_all(path)
    }

    /// Remove a file if it exists
    #[instrument(skip(self))]
    pub fn remove_file_if_exists<P: AsRef<Path> + std::fmt::Debug>(&self, path: P) -> io::Result<bool> {
        let path = path.as_ref();
        
        match fs::remove_file(path) {
            Ok(()) => {
                debug!("Removed file: {}", path.display());
                Ok(true)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                debug!("File does not exist: {}", path.display());
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    /// Remove a directory and all its contents if it exists
    #[instrument(skip(self))]
    pub fn remove_dir_all_if_exists<P: AsRef<Path> + std::fmt::Debug>(&self, path: P) -> io::Result<bool> {
        let path = path.as_ref();
        
        match fs::remove_dir_all(path) {
            Ok(()) => {
                debug!("Removed directory: {}", path.display());
                Ok(true)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                debug!("Directory does not exist: {}", path.display());
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    /// Check if a path exists and is a file
    pub fn is_file<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref().is_file()
    }

    /// Check if a path exists and is a directory
    pub fn is_dir<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref().is_dir()
    }

    /// Get file size in bytes
    #[instrument(skip(self))]
    pub fn file_size<P: AsRef<Path> + std::fmt::Debug>(&self, path: P) -> io::Result<u64> {
        let path = path.as_ref();
        let metadata = fs::metadata(path)?;
        Ok(metadata.len())
    }

    /// Write content to a file, creating parent directories if needed
    #[instrument(skip(self, contents))]
    pub fn write_file<P: AsRef<Path> + std::fmt::Debug, C: AsRef<[u8]>>(
        &self,
        path: P,
        contents: C,
    ) -> io::Result<()> {
        let path = path.as_ref();
        
        debug!("Writing file: {}", path.display());
        
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            self.create_dir_all(parent)?;
        }
        
        fs::write(path, contents)?;
        debug!("File written successfully");
        Ok(())
    }

    /// Read file contents as string
    #[instrument(skip(self))]
    pub fn read_file_to_string<P: AsRef<Path> + std::fmt::Debug>(&self, path: P) -> io::Result<String> {
        let path = path.as_ref();
        debug!("Reading file: {}", path.display());
        fs::read_to_string(path)
    }

    /// Get the current working directory
    pub fn current_dir(&self) -> io::Result<std::path::PathBuf> {
        std::env::current_dir()
    }

    /// Change the current working directory
    #[instrument(skip(self))]
    pub fn set_current_dir<P: AsRef<Path> + std::fmt::Debug>(&self, path: P) -> io::Result<()> {
        let path = path.as_ref();
        debug!("Changing directory to: {}", path.display());
        std::env::set_current_dir(path)
    }
}

impl Default for FileSystemUtils {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_copy_file() {
        let temp_dir = TempDir::new().unwrap();
        let fs_utils = FileSystemUtils::new();
        
        let src = temp_dir.path().join("source.txt");
        let dst = temp_dir.path().join("dest.txt");
        
        fs::write(&src, "test content").unwrap();
        
        let bytes_copied = fs_utils.copy_file(&src, &dst).unwrap();
        assert_eq!(bytes_copied, 12); // "test content"
        
        assert!(dst.exists());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "test content");
        assert!(src.exists()); // Source should still exist
    }

    #[test]
    fn test_move_file() {
        let temp_dir = TempDir::new().unwrap();
        let fs_utils = FileSystemUtils::new();
        
        let src = temp_dir.path().join("source.txt");
        let dst = temp_dir.path().join("dest.txt");
        
        fs::write(&src, "test content").unwrap();
        
        fs_utils.move_file(&src, &dst).unwrap();
        
        assert!(dst.exists());
        assert!(!src.exists()); // Source should be removed
        assert_eq!(fs::read_to_string(&dst).unwrap(), "test content");
    }

    #[test]
    fn test_create_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let fs_utils = FileSystemUtils::new();
        
        let nested_path = temp_dir.path().join("a").join("b").join("c");
        
        fs_utils.create_dir_all(&nested_path).unwrap();
        assert!(nested_path.exists());
        assert!(nested_path.is_dir());
    }

    #[test]
    fn test_remove_file_if_exists() {
        let temp_dir = TempDir::new().unwrap();
        let fs_utils = FileSystemUtils::new();
        
        let file_path = temp_dir.path().join("test.txt");
        
        // File doesn't exist
        let removed = fs_utils.remove_file_if_exists(&file_path).unwrap();
        assert!(!removed);
        
        // Create file and remove it
        fs::write(&file_path, "content").unwrap();
        let removed = fs_utils.remove_file_if_exists(&file_path).unwrap();
        assert!(removed);
        assert!(!file_path.exists());
    }

    #[test]
    fn test_write_and_read_file() {
        let temp_dir = TempDir::new().unwrap();
        let fs_utils = FileSystemUtils::new();
        
        let file_path = temp_dir.path().join("subdir").join("test.txt");
        let content = "Hello, world!";
        
        fs_utils.write_file(&file_path, content).unwrap();
        let read_content = fs_utils.read_file_to_string(&file_path).unwrap();
        
        assert_eq!(content, read_content);
    }

    #[test]
    fn test_file_size() {
        let temp_dir = TempDir::new().unwrap();
        let fs_utils = FileSystemUtils::new();
        
        let file_path = temp_dir.path().join("test.txt");
        let content = "Hello, world!";
        
        fs::write(&file_path, content).unwrap();
        let size = fs_utils.file_size(&file_path).unwrap();
        
        assert_eq!(size, content.len() as u64);
    }

    #[test]
    fn test_is_file_and_is_dir() {
        let temp_dir = TempDir::new().unwrap();
        let fs_utils = FileSystemUtils::new();
        
        let file_path = temp_dir.path().join("test.txt");
        let dir_path = temp_dir.path().join("testdir");
        
        fs::write(&file_path, "content").unwrap();
        fs::create_dir(&dir_path).unwrap();
        
        assert!(fs_utils.is_file(&file_path));
        assert!(!fs_utils.is_dir(&file_path));
        
        assert!(fs_utils.is_dir(&dir_path));
        assert!(!fs_utils.is_file(&dir_path));
        
        assert!(!fs_utils.is_file("nonexistent"));
        assert!(!fs_utils.is_dir("nonexistent"));
    }
}