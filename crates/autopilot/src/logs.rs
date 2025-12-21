//! Log rotation and cleanup for trajectory logs

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use colored::*;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{self, File};
use std::io::{Read, Write};
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
            db_path: Some(PathBuf::from("autopilot.db")),
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
                    println!("{} Archived: {} -> {}",
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

/// Archive a single file with gzip compression
fn archive_file(path: &Path) -> Result<PathBuf> {
    let mut input = File::open(path)?;
    let mut buffer = Vec::new();
    input.read_to_end(&mut buffer)?;

    let archived_path = path.with_extension(format!("{}.gz",
        path.extension().and_then(|s| s.to_str()).unwrap_or("")
    ));

    let output = File::create(&archived_path)?;
    let mut encoder = GzEncoder::new(output, Compression::default());
    encoder.write_all(&buffer)?;
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

    // Get sessions linked to open issues if db available
    let protected_sessions = if let Some(db_path) = &config.db_path {
        get_protected_sessions(db_path)?
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

            // Check if file belongs to a protected session
            if is_protected_session(path, &protected_sessions) {
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

/// Get list of session IDs that are linked to open issues
fn get_protected_sessions(db_path: &Path) -> Result<Vec<String>> {
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let conn = rusqlite::Connection::open(db_path)
        .context("Failed to open database")?;

    // Get session_ids from sessions table where there's a linked open issue
    let mut stmt = conn.prepare(
        "SELECT DISTINCT s.id FROM sessions s
         JOIN issues i ON i.number IN (
             SELECT CAST(value AS INTEGER) FROM json_each(s.issues_completed)
         )
         WHERE i.status != 'done'"
    )?;

    let sessions: Result<Vec<String>, _> = stmt
        .query_map([], |row| row.get(0))?
        .collect();

    Ok(sessions?)
}

/// Check if a log file belongs to a protected session
fn is_protected_session(path: &Path, protected: &[String]) -> bool {
    if protected.is_empty() {
        return false;
    }

    // Extract potential session ID from filename
    // Filenames typically: HHMMSS-slug.rlog.gz or HHMMSS-slug.json.gz
    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
        for session_id in protected {
            if filename.contains(session_id) {
                return true;
            }
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
