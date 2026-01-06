//! Vault management - flat folder of markdown notes

use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

/// A file entry in the vault
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Full path to the file
    pub path: PathBuf,
    /// File name without .md extension
    pub name: String,
    /// Last modified time
    pub modified: SystemTime,
}

/// A vault is a flat directory containing markdown notes
pub struct Vault {
    /// Root path of the vault
    pub path: PathBuf,
}

impl Vault {
    /// Get the default vault path (~/.openagents/onyx/)
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .expect("No home directory found")
            .join(".openagents")
            .join("onyx")
    }

    /// Open a vault at the given path, creating it if needed
    pub fn open(path: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    /// List all markdown files in the vault, sorted by modified time (most recent first)
    pub fn list_files(&self) -> io::Result<Vec<FileEntry>> {
        let mut files = Vec::new();

        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();

            // Only include .md files
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("untitled")
                    .to_string();

                let modified = entry.metadata()?.modified()?;

                files.push(FileEntry {
                    path,
                    name,
                    modified,
                });
            }
        }

        // Sort by modified time, most recent first
        files.sort_by(|a, b| b.modified.cmp(&a.modified));

        Ok(files)
    }

    /// Read a file's contents
    pub fn read_file(&self, path: &PathBuf) -> io::Result<String> {
        fs::read_to_string(path)
    }

    /// Write content to a file
    pub fn write_file(&self, path: &PathBuf, content: &str) -> io::Result<()> {
        fs::write(path, content)
    }

    /// Create a new file with the given name
    pub fn create_file(&self, name: &str) -> io::Result<PathBuf> {
        let path = self.path.join(format!("{}.md", name));
        fs::write(&path, "")?;
        Ok(path)
    }

    /// Delete a file permanently
    #[allow(dead_code)] // Prefer archive_file for recoverable deletion
    pub fn delete_file(&self, path: &PathBuf) -> io::Result<()> {
        fs::remove_file(path)
    }

    /// Archive a file (move to .archive subfolder)
    pub fn archive_file(&self, path: &PathBuf) -> io::Result<()> {
        let archive_dir = self.path.join(".archive");
        fs::create_dir_all(&archive_dir)?;

        if let Some(filename) = path.file_name() {
            let archive_path = archive_dir.join(filename);
            fs::rename(path, archive_path)?;
        }
        Ok(())
    }

    /// Generate a unique "Untitled N" name by finding the next available number
    pub fn generate_unique_name(&self) -> String {
        let mut max_num = 0;

        // Scan existing files for "Untitled N" pattern
        if let Ok(entries) = fs::read_dir(&self.path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Some(num_str) = stem.strip_prefix("Untitled ") {
                            if let Ok(num) = num_str.parse::<u32>() {
                                max_num = max_num.max(num);
                            }
                        }
                    }
                }
            }
        }

        // Also check archive folder
        let archive_dir = self.path.join(".archive");
        if let Ok(entries) = fs::read_dir(&archive_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Some(num_str) = stem.strip_prefix("Untitled ") {
                            if let Ok(num) = num_str.parse::<u32>() {
                                max_num = max_num.max(num);
                            }
                        }
                    }
                }
            }
        }

        format!("Untitled {}", max_num + 1)
    }
}
