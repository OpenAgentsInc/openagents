//! Small utilities shared across bridge modules.

use std::path::PathBuf;

#[inline]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Expand ~/ prefix to HOME for user-friendly paths sent from clients.
pub fn expand_home(p: &str) -> PathBuf {
    if let Some(stripped) = p.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") { return PathBuf::from(home).join(stripped); }
    } else if p == "~" {
        if let Ok(home) = std::env::var("HOME") { return PathBuf::from(home); }
    }
    PathBuf::from(p)
}

/// Return ordered list of sqlite table names for diagnostics.
pub fn list_sqlite_tables(db_path: &PathBuf) -> anyhow::Result<Vec<String>> {
    let conn = rusqlite::Connection::open(db_path)?;
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in iter { out.push(r?); }
    Ok(out)
}

