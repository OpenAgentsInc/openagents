//! Vault management - flat folder of markdown notes

use openagents_utils::filenames::sanitize_filename;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

/// A file entry in the vault
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Full path to the file
    pub path: PathBuf,
    /// Title (first line of file content, or filename if empty)
    pub title: String,
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
                let fallback_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("untitled")
                    .to_string();

                // Read first line as title, fall back to filename
                let title = fs::read_to_string(&path)
                    .ok()
                    .and_then(|content| content.lines().next().map(|s| s.to_string()))
                    .unwrap_or(fallback_name);

                let modified = entry.metadata()?.modified()?;

                files.push(FileEntry {
                    path,
                    title,
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

    /// Rename a file based on its new title (first line of content)
    /// Returns the new path if renamed, or None if no rename was needed
    pub fn rename_file(&self, path: &PathBuf, new_title: &str) -> io::Result<Option<PathBuf>> {
        // Get current filename without extension
        let current_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        // Sanitize the new title
        let new_stem = match sanitize_filename(new_title) {
            Some(s) => s,
            None => return Ok(None), // Empty title, don't rename
        };

        // Check if rename is needed
        if current_stem == new_stem {
            return Ok(None);
        }

        // Build new path, handling conflicts
        let mut new_path = self.path.join(format!("{}.md", new_stem));
        let mut counter = 1;

        while new_path.exists() && new_path != *path {
            new_path = self.path.join(format!("{} {}.md", new_stem, counter));
            counter += 1;
        }

        // If we ended up with the same path, no rename needed
        if new_path == *path {
            return Ok(None);
        }

        // Perform the rename
        fs::rename(path, &new_path)?;
        Ok(Some(new_path))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper to create a test vault
    fn test_vault() -> (TempDir, Vault) {
        let temp_dir = TempDir::new().unwrap();
        let vault = Vault::open(temp_dir.path().to_path_buf()).unwrap();
        (temp_dir, vault)
    }

    // ============ sanitize_filename tests ============

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(
            sanitize_filename("Hello World"),
            Some("Hello World".to_string())
        );
        assert_eq!(sanitize_filename("My Note"), Some("My Note".to_string()));
    }

    #[test]
    fn test_sanitize_filename_removes_invalid_chars() {
        assert_eq!(
            sanitize_filename("Hello/World"),
            Some("Hello World".to_string())
        );
        assert_eq!(
            sanitize_filename("File:Name"),
            Some("File Name".to_string())
        );
        assert_eq!(sanitize_filename("A*B?C"), Some("A B C".to_string()));
        assert_eq!(
            sanitize_filename("path\\to\\file"),
            Some("path to file".to_string())
        );
        assert_eq!(sanitize_filename("<test>"), Some("test".to_string()));
        assert_eq!(sanitize_filename("\"quoted\""), Some("quoted".to_string()));
    }

    #[test]
    fn test_sanitize_filename_collapses_whitespace() {
        assert_eq!(
            sanitize_filename("Hello   World"),
            Some("Hello World".to_string())
        );
        assert_eq!(
            sanitize_filename("  Trimmed  "),
            Some("Trimmed".to_string())
        );
        assert_eq!(sanitize_filename("A / B / C"), Some("A B C".to_string()));
    }

    #[test]
    fn test_sanitize_filename_empty_returns_none() {
        assert_eq!(sanitize_filename(""), None);
        assert_eq!(sanitize_filename("   "), None);
        assert_eq!(sanitize_filename("///"), None);
        assert_eq!(sanitize_filename("***"), None);
    }

    #[test]
    fn test_sanitize_filename_truncates_long_titles() {
        let long_title = "a".repeat(150);
        let result = sanitize_filename(&long_title).unwrap();
        assert_eq!(result.len(), 100);
        assert!(result.chars().all(|c| c == 'a'));
    }

    #[test]
    fn test_sanitize_filename_preserves_unicode() {
        assert_eq!(
            sanitize_filename("日本語タイトル"),
            Some("日本語タイトル".to_string())
        );
        assert_eq!(
            sanitize_filename("Émoji 🎉 Test"),
            Some("Émoji 🎉 Test".to_string())
        );
    }

    // ============ rename_file tests ============

    #[test]
    fn test_rename_file_basic() {
        let (_temp, vault) = test_vault();

        // Create a file
        let path = vault.create_file("Untitled 1").unwrap();
        fs::write(&path, "New Title\n\nContent here").unwrap();

        // Rename it
        let result = vault.rename_file(&path, "New Title").unwrap();
        assert!(result.is_some());

        let new_path = result.unwrap();
        assert_eq!(new_path.file_stem().unwrap().to_str().unwrap(), "New Title");
        assert!(new_path.exists());
        assert!(!path.exists());
    }

    #[test]
    fn test_rename_file_no_change_needed() {
        let (_temp, vault) = test_vault();

        // Create a file with matching name and title
        let path = vault.create_file("My Note").unwrap();
        fs::write(&path, "My Note\n\nContent").unwrap();

        // Should return None (no rename needed)
        let result = vault.rename_file(&path, "My Note").unwrap();
        assert!(result.is_none());
        assert!(path.exists());
    }

    #[test]
    fn test_rename_file_handles_conflicts() {
        let (_temp, vault) = test_vault();

        // Create two files
        let path1 = vault.create_file("Original").unwrap();
        let path2 = vault.create_file("Target").unwrap();
        fs::write(&path1, "Target\n\nContent").unwrap();
        fs::write(&path2, "Target\n\nOther content").unwrap();

        // Rename Original to Target (but Target already exists)
        let result = vault.rename_file(&path1, "Target").unwrap();
        assert!(result.is_some());

        let new_path = result.unwrap();
        // Should be "Target 1.md" due to conflict
        assert_eq!(new_path.file_stem().unwrap().to_str().unwrap(), "Target 1");
        assert!(new_path.exists());
        assert!(path2.exists()); // Original "Target.md" still exists
    }

    #[test]
    fn test_rename_file_sanitizes_title() {
        let (_temp, vault) = test_vault();

        let path = vault.create_file("Untitled 1").unwrap();
        fs::write(&path, "My/Invalid:Title\n\nContent").unwrap();

        let result = vault.rename_file(&path, "My/Invalid:Title").unwrap();
        assert!(result.is_some());

        let new_path = result.unwrap();
        assert_eq!(
            new_path.file_stem().unwrap().to_str().unwrap(),
            "My Invalid Title"
        );
    }

    #[test]
    fn test_rename_file_empty_title_no_rename() {
        let (_temp, vault) = test_vault();

        let path = vault.create_file("Untitled 1").unwrap();
        fs::write(&path, "").unwrap();

        // Empty title should not rename
        let result = vault.rename_file(&path, "").unwrap();
        assert!(result.is_none());
        assert!(path.exists());
    }

    #[test]
    fn test_rename_file_whitespace_only_title_no_rename() {
        let (_temp, vault) = test_vault();

        let path = vault.create_file("Untitled 1").unwrap();

        // Whitespace-only title should not rename
        let result = vault.rename_file(&path, "   ").unwrap();
        assert!(result.is_none());
        assert!(path.exists());
    }

    #[test]
    fn test_rename_file_preserves_content() {
        let (_temp, vault) = test_vault();

        let path = vault.create_file("Untitled 1").unwrap();
        let content = "New Title\n\nThis is important content\nDon't lose it!";
        fs::write(&path, content).unwrap();

        let result = vault.rename_file(&path, "New Title").unwrap();
        let new_path = result.unwrap();

        // Content should be unchanged
        let read_content = fs::read_to_string(&new_path).unwrap();
        assert_eq!(read_content, content);
    }

    // ============ Integration tests ============

    #[test]
    fn test_list_files_shows_title_from_content() {
        let (_temp, vault) = test_vault();

        // Create file with different filename and title
        let path = vault.create_file("filename").unwrap();
        fs::write(&path, "Display Title\n\nBody content").unwrap();

        let files = vault.list_files().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].title, "Display Title");
    }

    #[test]
    fn test_rename_then_list_shows_correct_title() {
        let (_temp, vault) = test_vault();

        let path = vault.create_file("Untitled 1").unwrap();
        fs::write(&path, "My Project Notes\n\nContent").unwrap();

        // Rename based on title
        let new_path = vault
            .rename_file(&path, "My Project Notes")
            .unwrap()
            .unwrap();

        // List should show the title and the path should match
        let files = vault.list_files().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].title, "My Project Notes");
        assert_eq!(files[0].path, new_path);
    }
}
