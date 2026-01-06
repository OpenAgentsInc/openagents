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

    /// Delete a file
    #[allow(dead_code)] // Will be used for file deletion feature
    pub fn delete_file(&self, path: &PathBuf) -> io::Result<()> {
        fs::remove_file(path)
    }

    /// Generate a unique name based on current timestamp
    pub fn generate_unique_name(&self) -> String {
        let now = chrono::Local::now();
        now.format("note-%Y%m%d-%H%M%S").to_string()
    }
}
