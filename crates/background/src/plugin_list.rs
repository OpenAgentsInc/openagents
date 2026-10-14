//! The plugins installed on this computer and which are on: what every
//! platform reads (running their background rules is Unix-only, in
//! [`crate::plugins`]).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::layout::Layout;

/// The kinds a plugin may propose a folder as (`crate::judged`).
pub const KINDS: [&str; 3] = ["build_output", "package_cache", "app_cache"];

/// The key a plugin installs under when its record names no 64-hex
/// publisher: no signer yet.
pub const LOCAL_KEY: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// One plugin installed on this computer, its newest version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    /// `KEY:SLUG`, what enabling names.
    pub id: String,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub version: String,
    pub dir: PathBuf,
    /// The rule ids its package record pins.
    pub background: Vec<String>,
    /// Folders it proposes as caches (`"classes": [{"path", "kind"}]`):
    /// the person confirms each, as for Jev's proposals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<(String, String)>,
    pub enabled: bool,
}

/// Every plugin installed under `~/.openagents/extensions`, newest version
/// of each, in key and slug order.
#[must_use]
pub fn installed(layout: &Layout) -> Vec<Installed> {
    let enabled = enabled(layout);
    let mut found = Vec::new();
    for key in sorted(&layout.extensions()) {
        let Some(key_name) = name_of(&key) else {
            continue;
        };
        for slug in sorted(&key) {
            let Some(dir) = sorted(&slug)
                .into_iter()
                .filter(|version| {
                    name_of(version).is_some()
                        && version
                            .extension()
                            .is_none_or(|extension| extension != "installing")
                })
                .filter(|version| version.join("package.json").is_file())
                .next_back()
            else {
                continue;
            };
            let Ok(record) = read_record(&dir) else {
                continue;
            };
            let slug_name = text(&record, "slug");
            if slug_name.is_empty() {
                continue;
            }
            let id = format!("{key_name}:{slug_name}");
            found.push(Installed {
                enabled: enabled.contains(&id),
                name: Some(text(&record, "name"))
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| slug_name.clone()),
                summary: text(&record, "summary"),
                version: text(&record, "version"),
                background: references(&record)
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect(),
                classes: proposed(&record),
                slug: slug_name,
                dir,
                id,
            });
        }
    }
    found
}

/// The installed plugin `name` names: `KEY:SLUG`, or a slug only one
/// installed plugin has.
///
/// # Errors
/// None or several match, in words for the person.
pub fn find(layout: &Layout, name: &str) -> Result<Installed, String> {
    let all = installed(layout);
    let matches: Vec<Installed> = all
        .into_iter()
        .filter(|plugin| plugin.id == name || plugin.slug == name)
        .collect();
    match matches.len() {
        1 => Ok(matches.into_iter().next().unwrap_or_else(|| unreachable!())),
        0 => Err(format!(
            "No plugin `{name}` is installed on this computer. Install it with `openagents plugin install DIR`."
        )),
        _ => Err(format!(
            "Several installed plugins are called `{name}`; name one as KEY:SLUG."
        )),
    }
}

/// The plugins enabled on this computer, as `KEY:SLUG`.
#[must_use]
pub fn enabled(layout: &Layout) -> BTreeSet<String> {
    std::fs::read(layout.enabled_plugins())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| value["enabled"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

pub(crate) fn read_record(dir: &Path) -> Result<Value, String> {
    let path = dir.join("package.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The folders a record proposes as caches, kept only when each is a
/// folder a rule may name and a disposable kind.
fn proposed(record: &Value) -> Vec<(String, String)> {
    record["classes"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|item| (text(item, "path"), text(item, "kind")))
                .filter(|(path, kind)| {
                    path.starts_with("~/")
                        && path.len() > 2
                        && !path.contains("..")
                        && KINDS.contains(&kind.as_str())
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn references(record: &Value) -> Vec<(String, String)> {
    record["background"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|item| (text(item, "name"), text(item, "digest")))
                .filter(|(name, _)| !name.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn text(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_owned()
}

pub(crate) fn name_of(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.'))
}

pub(crate) fn sorted(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|read| {
            read.filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| !path.is_symlink())
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}
