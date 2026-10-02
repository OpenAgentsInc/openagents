//! Rules on disk, the audit log (`runs.jsonl`), and the runner's state.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::paths::Layout;
use crate::rule::{self, Rule};
use crate::run::Record;

/// Rotate the log at this size.
pub const LOG_BYTES: u64 = 10 * 1024 * 1024;
/// Keep this many rotated logs.
pub const LOG_FILES: usize = 10;

/// Write `bytes` to `path` through a temporary file and a rename.
///
/// # Errors
/// The write or rename failed.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp-{}", std::process::id()));
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// The rule `id`: its file when there is one, else the built-in, else the
/// rule an enabled plugin brings. A plugin's rule edited here (paused, or
/// changed with `edit`) is admitted again against the plugin, so an edit
/// never widens what the host grants it; a disabled plugin's rule is no
/// rule at all.
///
/// # Errors
/// No such rule, or its file does not parse, validate, or admit.
pub fn load(layout: &Layout, id: &str) -> Result<Rule, String> {
    if !rule::id_like(id) {
        return Err(format!("no rule `{id}`"));
    }
    let plugin = if rule::built_in(id).is_some() {
        None
    } else {
        match crate::plugins::rule(layout, id) {
            Some(Ok((plugin, rule))) => Some((plugin, rule)),
            Some(Err(why)) => return Err(why),
            // A rule made in conversation, or none.
            None => return load_conversation(layout, id),
        }
    };
    let path = layout.rules().join(format!("{id}.json"));
    match std::fs::read(&path) {
        Ok(bytes) => {
            let rule: Rule = serde_json::from_slice(&bytes)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            if rule.id != id {
                return Err(format!("{} holds rule `{}`", path.display(), rule.id));
            }
            match plugin {
                None => {
                    rule.validate()?;
                    Ok(rule)
                }
                Some((plugin, from_plugin)) => {
                    // The plugin's needs, not the edited file's.
                    let mut edited = rule.clone();
                    edited.needs = from_plugin.needs;
                    let mut admitted = crate::plugins::admit(edited, &plugin)?;
                    admitted.enabled = rule.enabled;
                    admitted.paused_until = rule.paused_until;
                    admitted.version = rule.version;
                    Ok(admitted)
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => match plugin {
            Some((_, rule)) => Ok(rule),
            None => rule::built_in(id).ok_or_else(|| format!("no rule `{id}`")),
        },
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// A rule made in conversation: its file, whose origin must say so.
fn load_conversation(layout: &Layout, id: &str) -> Result<Rule, String> {
    let path = layout.rules().join(format!("{id}.json"));
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("no rule `{id}`"));
        }
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let rule: Rule =
        serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    if rule.id != id {
        return Err(format!("{} holds rule `{}`", path.display(), rule.id));
    }
    if !rule.conversational() {
        // A plugin's rule whose plugin is off or gone: no rule here.
        return Err(format!("no rule `{id}`"));
    }
    rule.validate()?;
    Ok(rule)
}

/// The ids of the rules made in conversation, from the rules folder.
fn conversation_ids(layout: &Layout) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(layout.rules()) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let id = name.strip_suffix(".json")?.to_owned();
            (rule::id_like(&id) && rule::built_in(&id).is_none()).then_some(id)
        })
        .filter(|id| {
            std::fs::read(layout.rules().join(format!("{id}.json")))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .is_some_and(|value| value["origin"]["kind"] == "conversation")
        })
        .collect();
    ids.sort();
    ids
}

/// Every rule: the built-in `disk`, then each enabled plugin's rules,
/// then the rules made in conversation.
#[must_use]
pub fn list(layout: &Layout) -> Vec<Result<Rule, (String, String)>> {
    let mut rules = vec![load(layout, "disk").map_err(|why| ("disk".to_owned(), why))];
    for admitted in crate::plugins::rules(layout) {
        rules.push(match admitted {
            Ok(rule) => load(layout, &rule.id).map_err(|why| (rule.id.clone(), why)),
            Err(failed) => Err(failed),
        });
    }
    let listed: Vec<String> = rules
        .iter()
        .map(|rule| match rule {
            Ok(rule) => rule.id.clone(),
            Err((id, _)) => id.clone(),
        })
        .collect();
    for id in conversation_ids(layout) {
        if !listed.contains(&id) {
            rules.push(load_conversation(layout, &id).map_err(|why| (id.clone(), why)));
        }
    }
    rules
}

/// Remove a rule made in conversation. The built-in rule and plugins'
/// rules are paused or turned off instead.
///
/// # Errors
/// It is not a rule made in conversation, or the file cannot be removed.
pub fn remove(layout: &Layout, id: &str) -> Result<(), String> {
    load_conversation(layout, id)?;
    std::fs::remove_file(layout.rules().join(format!("{id}.json")))
        .map_err(|error| error.to_string())?;
    State::forget(layout, id);
    Ok(())
}

/// Save a validated rule as a new version of its file.
///
/// # Errors
/// It does not validate, or the write failed.
pub fn save(layout: &Layout, rule: &Rule) -> Result<Rule, String> {
    rule.validate()?;
    if rule::built_in(&rule.id).is_none() {
        // A plugin's rule: it must still admit against its plugin.
        match crate::plugins::rule(layout, &rule.id) {
            Some(Ok((plugin, from_plugin))) => {
                let mut edited = rule.clone();
                edited.needs = from_plugin.needs;
                crate::plugins::admit(edited, &plugin)?;
            }
            Some(Err(why)) => return Err(why),
            None if rule.conversational() => {}
            None => return Err(format!("no rule `{}`", rule.id)),
        }
    }
    let mut next = rule.clone();
    if let Ok(current) = load(layout, &rule.id)
        && current != *rule
    {
        next.version = current.version.max(rule.version) + 1;
    }
    let bytes = serde_json::to_vec_pretty(&next).map_err(|error| error.to_string())?;
    write_atomic(&layout.rules().join(format!("{}.json", next.id)), &bytes)
        .map_err(|error| error.to_string())?;
    Ok(next)
}

/// What the runner knows about one rule, for `list`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_check: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_check: Option<u64>,
    /// Free bytes and the volume size at the last check, of the fullest
    /// watched volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_id: Option<String>,
    /// The last run's one line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_result: Option<String>,
    /// The last notification and when it was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<(u64, String)>,
    /// When a `Daily` trigger last ran (or the runner first saw it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_daily: Option<u64>,
    /// The host process that runs the rules, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub rules: BTreeMap<String, RuleState>,
}

impl State {
    #[must_use]
    pub fn load(layout: &Layout) -> Self {
        std::fs::read(layout.state())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Drop a removed rule's state.
    pub fn forget(layout: &Layout, id: &str) {
        let mut state = Self::load(layout);
        if state.rules.remove(id).is_some()
            && let Ok(bytes) = serde_json::to_vec_pretty(&state)
        {
            let _ = write_atomic(&layout.state(), &bytes);
        }
    }

    /// Change one rule's state and save.
    pub fn update(layout: &Layout, id: &str, change: impl FnOnce(&mut RuleState)) {
        let mut state = Self::load(layout);
        change(state.rules.entry(id.to_owned()).or_default());
        if let Ok(bytes) = serde_json::to_vec_pretty(&state) {
            let _ = write_atomic(&layout.state(), &bytes);
        }
    }
}

/// Records that could not be written yet (a full disk), flushed with the
/// next append.
static PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Append a run to the audit log, rotating it at [`LOG_BYTES`]. A failed
/// write keeps the record in memory for the next append.
pub fn append(layout: &Layout, record: &Record) {
    let Ok(line) = serde_json::to_string(record) else {
        return;
    };
    let mut pending = PENDING.lock().unwrap_or_else(|poison| poison.into_inner());
    pending.push(line);
    let path = layout.runs();
    let _ = std::fs::create_dir_all(layout.background());
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() >= LOG_BYTES) {
        rotate(&path);
    }
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| {
            let mut text = pending.join("\n");
            text.push('\n');
            file.write_all(text.as_bytes())
        });
    if written.is_ok() {
        pending.clear();
    }
}

fn rotate(path: &Path) {
    let name = |n: usize| {
        let mut name = path.as_os_str().to_owned();
        name.push(format!(".{n}"));
        std::path::PathBuf::from(name)
    };
    let _ = std::fs::remove_file(name(LOG_FILES));
    for n in (1..LOG_FILES).rev() {
        let _ = std::fs::rename(name(n), name(n + 1));
    }
    let _ = std::fs::rename(path, name(1));
}

/// Every recorded run, oldest first, from the rotated logs and the
/// current one.
#[must_use]
pub fn read_log(layout: &Layout) -> Vec<Record> {
    let path = layout.runs();
    let mut files = Vec::new();
    for n in (1..=LOG_FILES).rev() {
        let mut name = path.as_os_str().to_owned();
        name.push(format!(".{n}"));
        files.push(std::path::PathBuf::from(name));
    }
    files.push(path);
    let mut records = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        records.extend(
            text.lines()
                .filter_map(|line| serde_json::from_str::<Record>(line).ok()),
        );
    }
    records
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_rotates_and_reads_back_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path(), None).unwrap();
        std::fs::create_dir_all(layout.background()).unwrap();
        // A current log over the limit rotates before the next append.
        std::fs::write(layout.runs(), vec![b'\n'; LOG_BYTES as usize]).unwrap();
        let record = Record::empty("r1", "disk");
        append(&layout, &record);
        let mut rotated = layout.runs().as_os_str().to_owned();
        rotated.push(".1");
        assert!(Path::new(&rotated).exists());
        append(&layout, &Record::empty("r2", "disk"));
        let runs: Vec<String> = read_log(&layout).into_iter().map(|r| r.run).collect();
        assert_eq!(runs, vec!["r1", "r2"]);
    }

    #[test]
    fn a_saved_rule_gets_a_new_version() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path(), None).unwrap();
        let mut rule = load(&layout, "disk").unwrap();
        assert_eq!(rule.version, 1);
        rule.classes.keep = 2;
        let saved = save(&layout, &rule).unwrap();
        assert_eq!(saved.version, 2);
        assert_eq!(load(&layout, "disk").unwrap(), saved);
        assert!(load(&layout, "nope").is_err());
    }
}
