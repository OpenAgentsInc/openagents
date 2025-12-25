//! Log rotation and cleanup for trajectory logs

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use colored::*;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// Configuration for log management
#[derive(Debug, Clone)]
pub struct LogsConfig {
    /// Root directory for logs (e.g., docs/logs/)
    pub logs_dir: PathBuf,
    /// Maximum total size in bytes before rotation (default: 1GB)
    pub max_total_size: u64,
    /// Age in days before archiving (default: 30)
    pub archive_after_days: i64,
    /// Age in days before deletion (default: 90)
    pub delete_after_days: i64,
    /// Database path for checking linked issues
    pub db_path: Option<PathBuf>,
}

impl Default for LogsConfig {
    fn default() -> Self {
        Self {
            logs_dir: PathBuf::from("docs/logs"),
            max_total_size: 1024 * 1024 * 1024, // 1GB
            archive_after_days: 30,
            delete_after_days: 90,
            db_path: Some(crate::default_db_path()),
        }
    }
}

/// Statistics about log files
#[derive(Debug, Default)]
pub struct LogsStats {
    pub total_files: usize,
    pub total_size: u64,
    pub archived_files: usize,
    pub archived_size: u64,
    pub regular_files: usize,
    pub regular_size: u64,
}

/// Calculate total size of all logs
pub fn calculate_log_size(config: &LogsConfig) -> Result<LogsStats> {
    let mut stats = LogsStats::default();

    if !config.logs_dir.exists() {
        return Ok(stats);
    }

    for entry in walkdir::WalkDir::new(&config.logs_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let size = entry.metadata()?.len();
                stats.total_files += 1;
                stats.total_size += size;

                if ext == "gz" {
                    stats.archived_files += 1;
                    stats.archived_size += size;
                } else if ext == "rlog" || ext == "json" {
                    stats.regular_files += 1;
                    stats.regular_size += size;
                }
            }
        }
    }

    Ok(stats)
}

/// Archive logs older than specified days
pub fn archive_logs(config: &LogsConfig, dry_run: bool) -> Result<Vec<PathBuf>> {
    let cutoff = Utc::now() - Duration::days(config.archive_after_days);
    let mut archived = Vec::new();

    if !config.logs_dir.exists() {
        return Ok(archived);
    }

    for entry in walkdir::WalkDir::new(&config.logs_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let path = entry.path();

            // Only archive .rlog and .json files (not already archived .gz files)
            if let Some(ext) = path.extension() {
                if ext != "rlog" && ext != "json" {
                    continue;
                }
            } else {
                continue;
            }

            // Check file modification time
            let metadata = entry.metadata()?;
            let modified: DateTime<Utc> = metadata.modified()?.into();

            if modified < cutoff {
                if dry_run {
                    println!("{} Would archive: {}", "DRY".yellow(), path.display());
                    archived.push(path.to_path_buf());
                } else {
                    let archived_path = archive_file(path)?;
                    println!(
                        "{} Archived: {} -> {}",
                        "✓".green(),
                        path.display(),
                        archived_path.display()
                    );
                    archived.push(path.to_path_buf());
                }
            }
        }
    }

    Ok(archived)
}

/// Archive a single file with gzip compression using streaming I/O
fn archive_file(path: &Path) -> Result<PathBuf> {
    let input = File::open(path)?;
    let archived_path = path.with_extension(format!(
        "{}.gz",
        path.extension().and_then(|s| s.to_str()).unwrap_or("")
    ));

    let output = File::create(&archived_path)?;
    let mut encoder = GzEncoder::new(output, Compression::default());

    // Stream the file in chunks instead of loading entirely into memory
    let mut reader = std::io::BufReader::new(input);
    std::io::copy(&mut reader, &mut encoder)?;

    encoder.finish()?;

    // Remove original file after successful archiving
    fs::remove_file(path)?;

    Ok(archived_path)
}

/// Delete archived logs older than specified days
pub fn cleanup_logs(config: &LogsConfig, dry_run: bool) -> Result<Vec<PathBuf>> {
    let cutoff = Utc::now() - Duration::days(config.delete_after_days);
    let mut deleted = Vec::new();

    if !config.logs_dir.exists() {
        return Ok(deleted);
    }

    // Get date directories with open issues if db available
    let protected_dates = if let Some(db_path) = &config.db_path {
        get_protected_dates(db_path)?
    } else {
        Vec::new()
    };

    for entry in walkdir::WalkDir::new(&config.logs_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let path = entry.path();

            // Only delete archived .gz files
            if let Some(ext) = path.extension() {
                if ext != "gz" {
                    continue;
                }
            } else {
                continue;
            }

            // Check if file belongs to a protected date directory
            if is_protected_date(path, &protected_dates) {
                continue;
            }

            // Check file modification time
            let metadata = entry.metadata()?;
            let modified: DateTime<Utc> = metadata.modified()?.into();

            if modified < cutoff {
                if dry_run {
                    println!("{} Would delete: {}", "DRY".yellow(), path.display());
                    deleted.push(path.to_path_buf());
                } else {
                    fs::remove_file(path)?;
                    println!("{} Deleted: {}", "✓".green(), path.display());
                    deleted.push(path.to_path_buf());
                }
            }
        }
    }

    Ok(deleted)
}

/// Get list of date directories that have open issues
fn get_protected_dates(db_path: &Path) -> Result<Vec<String>> {
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let conn = rusqlite::Connection::open(db_path).context("Failed to open database")?;

    // Get unique dates (YYYYMMDD) from created_at timestamps of open issues
    let mut stmt = conn.prepare(
        "SELECT DISTINCT substr(created_at, 1, 10) as date
         FROM issues
         WHERE status IN ('open', 'in_progress', 'blocked')",
    )?;

    let dates: Result<Vec<String>, _> = stmt
        .query_map([], |row| {
            let date_str: String = row.get(0)?;
            // Convert YYYY-MM-DD to YYYYMMDD
            Ok(date_str.replace("-", ""))
        })?
        .collect();

    Ok(dates?)
}

/// Check if a log file belongs to a protected date directory
fn is_protected_date(path: &Path, protected_dates: &[String]) -> bool {
    if protected_dates.is_empty() {
        return false;
    }

    // Check if the parent directory name matches any protected date
    // Path structure: docs/logs/YYYYMMDD/HHMMSS-slug.ext
    if let Some(parent) = path.parent() {
        if let Some(date_dir) = parent.file_name().and_then(|d| d.to_str()) {
            return protected_dates.iter().any(|d| date_dir == d);
        }
    }

    false
}

/// Print log statistics
pub fn print_stats(stats: &LogsStats) {
    println!("\n{}", "=".repeat(60).cyan());
    println!("{} Log Statistics", "LOGS:".cyan().bold());
    println!("{}", "=".repeat(60).cyan());

    println!("\n{}", "Total:".bold());
    println!("  Files:  {}", stats.total_files);
    println!("  Size:   {}", format_bytes(stats.total_size));

    if stats.regular_files > 0 {
        println!("\n{}", "Regular Logs:".bold());
        println!("  Files:  {}", stats.regular_files);
        println!("  Size:   {}", format_bytes(stats.regular_size));
    }

    if stats.archived_files > 0 {
        println!("\n{}", "Archived:".bold());
        println!("  Files:  {}", stats.archived_files);
        println!("  Size:   {}", format_bytes(stats.archived_size));
    }

    println!();
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    #[test]
    fn test_is_protected_date() {
        let protected = vec!["20251223".to_string(), "20251222".to_string()];

        // Should protect files in protected date directories
        let path = PathBuf::from("docs/logs/20251223/033937-call-issue-ready-now-to.rlog.gz");
        assert!(is_protected_date(&path, &protected));

        let path = PathBuf::from("docs/logs/20251222/120000-some-other-log.jsonl.gz");
        assert!(is_protected_date(&path, &protected));

        // Should not protect files in unprotected date directories
        let path = PathBuf::from("docs/logs/20251220/120000-old-log.rlog.gz");
        assert!(!is_protected_date(&path, &protected));

        // Empty protected list should not protect anything
        let path = PathBuf::from("docs/logs/20251223/033937-call-issue-ready-now-to.rlog.gz");
        assert!(!is_protected_date(&path, &[]));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1536 * 1024), "1.50 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_archive_file_streaming() {
        use flate2::read::GzDecoder;
        use std::io::Read;
        use tempfile::NamedTempFile;

        // Create a temporary file with test content
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_content = "This is test content for streaming compression.\n".repeat(1000);
        std::io::Write::write_all(&mut temp_file, test_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let temp_path = temp_file.path().to_path_buf();

        // Archive the file (this will delete the original)
        let archived_path = archive_file(&temp_path).unwrap();

        // Verify original file was removed
        assert!(!temp_path.exists());

        // Verify archived file exists and has .gz extension
        assert!(archived_path.exists());
        assert!(archived_path.to_string_lossy().ends_with(".gz"));

        // Decompress and verify content matches
        let archived_file = File::open(&archived_path).unwrap();
        let mut decoder = GzDecoder::new(archived_file);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();

        assert_eq!(decompressed, test_content);

        // Cleanup
        std::fs::remove_file(archived_path).ok();
    }
}
