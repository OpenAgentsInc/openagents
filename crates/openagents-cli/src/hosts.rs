//! Names for hosts and a record of what ran on them.
//!
//! Host keys are 64 hex characters; nobody wants to type one. An alias file
//! in the computer store maps short names to keys, and every host argument
//! accepts a name, a whole key, or a unique key prefix. The journal is an
//! append-only NDJSON file in the same store, one line per remote command,
//! so someone following along can read what ran, when, and how it ended,
//! without the output itself (a digest stands in for it).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::args::Args;
use crate::terminal::Run;

const ALIASES: &str = "aliases.json";
const JOURNAL: &str = "exec.jsonl";

fn aliases_path(store: &Path) -> PathBuf {
    store.join(ALIASES)
}

/// Every alias, name to host key.
pub fn aliases(store: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(aliases_path(store))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_aliases(store: &Path, aliases: &BTreeMap<String, String>) -> Result<(), String> {
    std::fs::create_dir_all(store).map_err(|e| format!("{}: {e}", store.display()))?;
    let path = aliases_path(store);
    std::fs::write(
        &path,
        serde_json::to_string_pretty(aliases).unwrap_or_default(),
    )
    .map_err(|e| format!("{}: {e}", path.display()))
}

/// Name `host` (a key, prefix, or existing alias) `name`.
pub fn set_alias(store: &Path, name: &str, host: &str) -> Result<(), String> {
    if name.is_empty() || name.len() == 64 && name.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("an alias must be a short name, not a host key".to_owned());
    }
    if name.contains(char::is_whitespace) {
        return Err("an alias can't contain spaces".to_owned());
    }
    let mut all = aliases(store);
    all.insert(name.to_owned(), host.to_owned());
    write_aliases(store, &all)
}

/// Forget the alias `name`; `Ok(false)` when there wasn't one.
pub fn remove_alias(store: &Path, name: &str) -> Result<bool, String> {
    let mut all = aliases(store);
    let had = all.remove(name).is_some();
    if had {
        write_aliases(store, &all)?;
    }
    Ok(had)
}

/// The alias for `host`, if one names it.
pub fn alias_of(store: &Path, host: &str) -> Option<String> {
    aliases(store)
        .into_iter()
        .find(|(_, key)| key == host)
        .map(|(name, _)| name)
}

/// The host key `text` names: an alias, a whole key, or a prefix that
/// matches exactly one of `known`. Anything else is an error that says so.
pub fn resolve(store: &Path, known: &[String], text: &str) -> Result<String, String> {
    if let Some(key) = aliases(store).get(text) {
        return Ok(key.clone());
    }
    if text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(text.to_owned());
    }
    let matches: Vec<&String> = known.iter().filter(|k| k.starts_with(text)).collect();
    match matches.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(format!(
            "`{text}` is not an alias, a host key, or a prefix of a known host (see `openagents computer list`)"
        )),
        many => Err(format!(
            "`{text}` matches {} hosts; give more of the key or an alias",
            many.len()
        )),
    }
}

/// Seconds since the Unix epoch, now.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `HH:MM:SS` UTC for a Unix time.
pub fn clock(unix: u64) -> String {
    let day = unix % 86_400;
    format!("{:02}:{:02}:{:02}", day / 3600, day % 3600 / 60, day % 60)
}

/// Append one line for `run` to the journal. Journaling never fails a
/// command: a store that can't be written just leaves no record.
pub fn journal(args: &Args, kind: &str, host: &str, words: &[String], run: &Run) {
    let store = crate::computer::store_dir(args.option("store"));
    let digest = Sha256::digest(run.output.as_bytes());
    let line = json!({
        "when": now(),
        "kind": kind,
        "host": host,
        "alias": alias_of(&store, host),
        "command": words,
        "exit": run.exit,
        "timed_out": run.timed_out,
        "seconds": (run.seconds * 10.0).round() / 10.0,
        "bytes": run.output.len(),
        "output_sha256": digest.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        "route": run.route,
    });
    if std::fs::create_dir_all(&store).is_err() {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(store.join(JOURNAL))
    {
        let _ = writeln!(file, "{line}");
    }
}

/// The last `lines` journal entries, oldest first, for `host` when given.
pub fn journal_entries(store: &Path, host: Option<&str>, lines: usize) -> Vec<Value> {
    let text = std::fs::read_to_string(store.join(JOURNAL)).unwrap_or_default();
    let mut entries: Vec<Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .filter(|entry: &Value| host.is_none_or(|h| entry["host"].as_str() == Some(h)))
        .collect();
    let skip = entries.len().saturating_sub(lines);
    entries.drain(..skip);
    entries
}

/// One journal entry as a line someone can read.
pub fn render_entry(entry: &Value) -> String {
    let when = entry["when"].as_u64().unwrap_or(0);
    let host = entry["alias"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let key = entry["host"].as_str().unwrap_or("");
            key.chars().take(12).collect()
        });
    let command = entry["command"]
        .as_array()
        .map(|words| {
            words
                .iter()
                .filter_map(Value::as_str)
                .map(crate::terminal::quote)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    format!(
        "{} {:<5} {:<12} exit {:<3} {:>6.1}s {:>7}B  {}",
        clock(when),
        entry["kind"].as_str().unwrap_or(""),
        host,
        entry["exit"].as_i64().unwrap_or(-1),
        entry["seconds"].as_f64().unwrap_or(0.0),
        entry["bytes"].as_u64().unwrap_or(0),
        command
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openagents-hosts-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    const KEY: &str = "93235bef199da408f5b6a6628ba62ffa23027b083928c6968131d8ea8ccad759";
    const OTHER: &str = "9c91229018246205933d4dcbd5fd780d92180141c3aa16cbaeeffe67a6faf316";

    #[test]
    fn resolves_aliases_keys_and_unique_prefixes() {
        let store = temp("resolve");
        let known = vec![KEY.to_owned(), OTHER.to_owned()];
        set_alias(&store, "coderos", KEY).unwrap();
        assert_eq!(resolve(&store, &known, "coderos").unwrap(), KEY);
        assert_eq!(resolve(&store, &known, KEY).unwrap(), KEY);
        assert_eq!(resolve(&store, &known, "9c9").unwrap(), OTHER);
        assert!(
            resolve(&store, &known, "9")
                .unwrap_err()
                .contains("matches 2")
        );
        assert!(resolve(&store, &known, "nope").is_err());
        assert_eq!(alias_of(&store, KEY).as_deref(), Some("coderos"));
        assert!(remove_alias(&store, "coderos").unwrap());
        assert!(!remove_alias(&store, "coderos").unwrap());
        assert!(resolve(&store, &known, "coderos").is_err());
        let _ = std::fs::remove_dir_all(&store);
    }

    #[test]
    fn refuses_keys_and_spaces_as_alias_names() {
        let store = temp("refuse");
        assert!(set_alias(&store, KEY, KEY).is_err());
        assert!(set_alias(&store, "two words", KEY).is_err());
        assert!(set_alias(&store, "", KEY).is_err());
    }

    #[test]
    fn journal_keeps_the_last_lines_per_host() {
        let store = temp("journal");
        std::fs::create_dir_all(&store).unwrap();
        let mut file = std::fs::File::create(store.join(JOURNAL)).unwrap();
        for (i, host) in [KEY, OTHER, KEY, KEY].iter().enumerate() {
            writeln!(
                file,
                "{}",
                json!({ "when": i, "kind": "exec", "host": host, "command": ["true"], "exit": 0, "seconds": 1.5, "bytes": 0 })
            )
            .unwrap();
        }
        drop(file);
        let all = journal_entries(&store, None, 10);
        assert_eq!(all.len(), 4);
        let mine = journal_entries(&store, Some(KEY), 2);
        assert_eq!(mine.len(), 2);
        assert_eq!(mine[0]["when"], 2);
        assert!(render_entry(&mine[0]).contains("exit 0"));
        let _ = std::fs::remove_dir_all(&store);
    }

    #[test]
    fn clock_is_utc_time_of_day() {
        assert_eq!(clock(0), "00:00:00");
        assert_eq!(clock(3_661), "01:01:01");
        assert_eq!(clock(86_400 + 59), "00:00:59");
    }
}
