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
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    } else if p == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(p)
}

/// Return ordered list of sqlite table names for diagnostics.
pub fn list_sqlite_tables(db_path: &PathBuf) -> anyhow::Result<Vec<String>> {
    let conn = rusqlite::Connection::open(db_path)?;
    let mut stmt =
        conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in iter {
        out.push(r?);
    }
    Ok(out)
}

/// Destructive helper to clear Convex data (admin function).
// Removed Convex admin helpers

/// Detect the repository root directory so tools run from the right place.
/// Heuristics:
/// - Prefer the nearest ancestor that contains both `expo/` and `crates/` directories.
/// - If not found, fall back to the process current_dir.
#[allow(dead_code)]
pub fn detect_repo_root(start: Option<PathBuf>) -> PathBuf {
    fn is_repo_root(p: &std::path::Path) -> bool {
        p.join("expo").is_dir() && p.join("crates").is_dir()
    }
    let mut cur =
        start.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let original = cur.clone();
    loop {
        if is_repo_root(&cur) {
            return cur;
        }
        if !cur.pop() {
            return original;
        }
    }
}

/// Determine the OpenAgents home directory. Defaults to ~/.openagents unless
/// overridden by the `OPENAGENTS_HOME` environment variable.
pub fn openagents_home() -> PathBuf {
    if let Ok(base) = std::env::var("OPENAGENTS_HOME") {
        PathBuf::from(base)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".openagents")
    } else {
        PathBuf::from(".openagents")
    }
}

/// Attempt to read a WebSocket auth token from `~/.openagents/bridge.json`.
/// Returns `Ok(Some(token))` when present, `Ok(None)` when the file is missing,
/// and `Err` for malformed JSON or I/O errors.
pub fn read_bridge_token_from_home() -> anyhow::Result<Option<String>> {
    let p = openagents_home().join("bridge.json");
    if !p.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(&p)?;
    #[derive(serde::Deserialize)]
    struct BridgeCfg {
        token: Option<String>,
    }
    let cfg: BridgeCfg = serde_json::from_str(&data)?;
    Ok(cfg.token)
}

/// Generate a random 32-byte token encoded as lowercase hex.
pub fn generate_bridge_token() -> String {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("secure RNG");
    let mut out = String::with_capacity(64);
    for b in buf {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Persist the token into `~/.openagents/bridge.json`.
pub fn write_bridge_token_to_home(token: &str) -> anyhow::Result<()> {
    let base = openagents_home();
    std::fs::create_dir_all(&base)?;
    let path = base.join("bridge.json");
    let body = serde_json::json!({ "token": token }).to_string();
    std::fs::write(path, body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn expands_home_prefix() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
        assert_eq!(
            expand_home("~/abc").to_string_lossy(),
            format!("{home}/abc")
        );
        assert_eq!(expand_home("~").to_string_lossy(), home);
        assert_eq!(expand_home("/x").to_string_lossy(), "/x");
    }

    #[test]
    fn detects_repo_root_shape() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path();
        std::fs::create_dir_all(p.join("expo")).unwrap();
        std::fs::create_dir_all(p.join("crates")).unwrap();
        let got = detect_repo_root(Some(p.to_path_buf()));
        assert_eq!(got, p);
    }

    // These tests mutate process-wide env vars; run them serially to avoid races.
    use serial_test::serial;

    #[test]
    #[serial]
    fn openagents_home_prefers_env() {
        let td = tempfile::tempdir().unwrap();
        let prev = std::env::var("OPENAGENTS_HOME").ok();
        unsafe { std::env::set_var("OPENAGENTS_HOME", td.path()); }
        let got = openagents_home();
        assert_eq!(got, PathBuf::from(td.path()));
        match prev {
            Some(v) => unsafe { std::env::set_var("OPENAGENTS_HOME", v); },
            None => unsafe { std::env::remove_var("OPENAGENTS_HOME"); },
        }
    }

    #[test]
    #[serial]
    fn reads_bridge_token_when_present() {
        let td = tempfile::tempdir().unwrap();
        let prev = std::env::var("OPENAGENTS_HOME").ok();
        unsafe { std::env::set_var("OPENAGENTS_HOME", td.path()); }
        let cfg_path = openagents_home().join("bridge.json");
        std::fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
        std::fs::write(&cfg_path, "{\n  \"token\": \"abc123\"\n}").unwrap();
        let tok = read_bridge_token_from_home().unwrap();
        assert_eq!(tok.as_deref(), Some("abc123"));
        match prev {
            Some(v) => unsafe { std::env::set_var("OPENAGENTS_HOME", v); },
            None => unsafe { std::env::remove_var("OPENAGENTS_HOME"); },
        }
    }

    #[test]
    fn generates_hex_token() {
        let t = generate_bridge_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
    }

    #[test]
    fn lists_sqlite_tables_in_order() {
        let td = tempfile::tempdir().unwrap();
        let db = td.path().join("test.sqlite3");
        {
            let conn = Connection::open(&db).unwrap();
            conn.execute("CREATE TABLE b (id INTEGER)", ()).unwrap();
            conn.execute("CREATE TABLE a (id INTEGER)", ()).unwrap();
        }
        let names = list_sqlite_tables(&db).expect("tables");
        // Should include at least our two tables (order by name ascending)
        let filtered: Vec<String> = names
            .into_iter()
            .filter(|n| n == "a" || n == "b")
            .collect();
        assert_eq!(filtered, vec!["a".to_string(), "b".to_string()]);
    }
}
