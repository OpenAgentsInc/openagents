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

/// Destructive helper to clear Convex data (admin function).
pub async fn run_convex_clear_all(port: u16) -> anyhow::Result<()> {
    use convex::{ConvexClient, Value};
    use std::collections::BTreeMap;
    let url = format!("http://127.0.0.1:{}", port);
    let mut client = ConvexClient::new(&url).await?;
    let args: BTreeMap<String, Value> = BTreeMap::new();
    let _ = client.mutation("admin:clearAll", args).await?;
    Ok(())
}

/// Detect the repository root directory so tools run from the right place.
/// Heuristics:
/// - Prefer the nearest ancestor that contains both `expo/` and `crates/` directories.
/// - If not found, fall back to the process current_dir.
pub fn detect_repo_root(start: Option<PathBuf>) -> PathBuf {
    fn is_repo_root(p: &std::path::Path) -> bool { p.join("expo").is_dir() && p.join("crates").is_dir() }
    let mut cur = start.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let original = cur.clone();
    loop { if is_repo_root(&cur) { return cur; } if !cur.pop() { return original; } }
}
