//! Coder One policy manifests as the Gym reads them: from the `policy`
//! record an episode manifest carries, and from manifest files such as
//! `crates/coder-one/policies/*.json`.
//!
//! The digest rule is Coder One's (`coder_one::policy`): SHA-256 of the
//! manifest's canonical JSON, object keys sorted and no whitespace, with
//! `name`, `note`, and `search` removed. A file that spells out every field
//! has the digest an episode records for it. The Gym reimplements the rule
//! rather than depending on the agent crate.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::terminal_bench::Records;

/// The manifest schema Coder One writes.
pub const SCHEMA: &str = "openagents.coder-one.policy.v1";

/// One resolved policy manifest.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyRecord {
    /// The manifest digest, hex.
    pub digest: String,
    /// The manifest's label, when it has one.
    pub name: Option<String>,
    /// Where the episode resolved it from: `builtin`, `inline`, or
    /// `file:<path>`; for a file the Gym read, its path.
    pub source: Option<String>,
    /// Each environment override the episode applied, as `SOURCE sets
    /// FIELD = VALUE`.
    pub overrides: Vec<String>,
    /// The resolved manifest.
    pub manifest: Value,
}

impl PolicyRecord {
    /// The `policy` record of an episode manifest.
    pub fn from_episode(record: &Value) -> Option<Self> {
        let manifest = record.get("manifest")?.clone();
        let digest = record
            .get("digest")
            .and_then(Value::as_str)
            .map_or_else(|| digest(&manifest), str::to_owned);
        Some(Self {
            digest,
            name: record
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned),
            source: record
                .get("source")
                .and_then(Value::as_str)
                .map(str::to_owned),
            overrides: record
                .get("overrides")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|o| {
                    format!(
                        "{} sets {} = {}",
                        o["source"].as_str().unwrap_or("?"),
                        o["field"].as_str().unwrap_or("?"),
                        o["value"].as_str().unwrap_or("?")
                    )
                })
                .collect(),
            manifest,
        })
    }

    /// A manifest file.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let manifest: Value =
            serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        if manifest.get("schema").and_then(Value::as_str) != Some(SCHEMA) {
            return Err(format!("{}: not a {SCHEMA} manifest", path.display()));
        }
        Ok(Self {
            digest: digest(&manifest),
            name: manifest
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned),
            source: Some(path.display().to_string()),
            overrides: Vec::new(),
            manifest,
        })
    }

    /// The first twelve hex digits of the digest.
    pub fn short(&self) -> &str {
        short(&self.digest)
    }
}

/// The first twelve characters of a digest.
pub fn short(digest: &str) -> &str {
    digest.get(..12).unwrap_or(digest)
}

/// SHA-256 of a manifest's canonical JSON without `name`, `note`, and
/// `search`.
pub fn digest(manifest: &Value) -> String {
    let mut value = manifest.clone();
    if let Some(object) = value.as_object_mut() {
        for key in ["name", "note", "search"] {
            object.remove(key);
        }
    }
    Sha256::digest(canonical(&value).as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// JSON with object keys sorted and no whitespace.
pub fn canonical(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let body: Vec<String> = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonical(&map[key])
                    )
                })
                .collect();
            format!("{{{}}}", body.join(","))
        }
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        other => other.to_string(),
    }
}

/// Every `policy.*` and `protected.*` field of a manifest, flattened to a
/// dotted path.
pub fn fields(manifest: &Value) -> BTreeMap<String, Value> {
    fn walk(prefix: &str, value: &Value, out: &mut BTreeMap<String, Value>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    walk(&format!("{prefix}.{key}"), child, out);
                }
            }
            other => {
                out.insert(prefix.to_owned(), other.clone());
            }
        }
    }
    let mut out = BTreeMap::new();
    for part in ["policy", "protected"] {
        if let Some(value) = manifest.get(part) {
            walk(part, value, &mut out);
        }
    }
    out
}

/// The fields whose values differ, with each side's value; a field only
/// one side has shows `null` on the other.
pub fn diff(a: &Value, b: &Value) -> Vec<(String, Value, Value)> {
    let (a, b) = (fields(a), fields(b));
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter_map(|key| {
            let left = a.get(key).cloned().unwrap_or(Value::Null);
            let right = b.get(key).cloned().unwrap_or(Value::Null);
            (left != right).then(|| (key.clone(), left, right))
        })
        .collect()
}

/// One distinct manifest and where the Gym saw it.
#[derive(Clone, Debug)]
pub struct Entry {
    pub record: PolicyRecord,
    /// Manifest files with this digest.
    pub files: Vec<PathBuf>,
    /// Arms whose attempts recorded this digest.
    pub arms: Vec<String>,
    /// How many attempts recorded it.
    pub attempts: usize,
}

/// Every distinct manifest among the attempts and the files under `dirs`,
/// by digest.
pub fn catalog(records: &Records, dirs: &[&Path]) -> (BTreeMap<String, Entry>, Vec<String>) {
    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    let mut errors = Vec::new();
    for attempt in &records.attempts {
        let Some(record) = &attempt.policy else {
            continue;
        };
        let entry = entries
            .entry(record.digest.clone())
            .or_insert_with(|| Entry {
                record: record.clone(),
                files: Vec::new(),
                arms: Vec::new(),
                attempts: 0,
            });
        entry.attempts += 1;
        if !entry.arms.contains(&attempt.arm) {
            entry.arms.push(attempt.arm.clone());
        }
    }
    for dir in dirs {
        let Ok(read) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = read
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();
        for path in paths {
            match PolicyRecord::from_file(&path) {
                Ok(record) => {
                    entries
                        .entry(record.digest.clone())
                        .or_insert_with(|| Entry {
                            record,
                            files: Vec::new(),
                            arms: Vec::new(),
                            attempts: 0,
                        })
                        .files
                        .push(path);
                }
                Err(error) => errors.push(error),
            }
        }
    }
    (entries, errors)
}

/// The entry a query names: a digest or a prefix of at least six hex
/// digits, a manifest name, an arm, or a manifest file path.
pub fn find<'a>(entries: &'a BTreeMap<String, Entry>, query: &str) -> Result<&'a Entry, String> {
    if let Some(entry) = entries.get(query) {
        return Ok(entry);
    }
    let path = Path::new(query);
    let by_digest: Vec<&Entry> = if query.len() >= 6 && query.chars().all(|c| c.is_ascii_hexdigit())
    {
        entries
            .iter()
            .filter(|(digest, _)| digest.starts_with(query))
            .map(|(_, entry)| entry)
            .collect()
    } else {
        Vec::new()
    };
    let matches: Vec<&Entry> = if !by_digest.is_empty() {
        by_digest
    } else {
        entries
            .values()
            .filter(|entry| {
                entry.record.name.as_deref() == Some(query)
                    || entry.arms.iter().any(|arm| arm == query)
                    || entry.files.iter().any(|file| {
                        file == path
                            || file.file_name()
                                == path.file_name().filter(|_| path.components().count() == 1)
                    })
            })
            .collect()
    };
    match matches.as_slice() {
        [entry] => Ok(entry),
        [] => Err(format!("no policy manifest matches {query}")),
        many => Err(format!(
            "{query} matches {} manifests: {}",
            many.len(),
            many.iter()
                .map(|entry| entry.record.short().to_owned())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../coder-one/policies")
            .join(name)
    }

    #[test]
    fn the_digest_ignores_labels_and_key_order() {
        let a: Value = serde_json::from_str(
            r#"{"schema":"s","name":"a","note":"n","search":["x"],"policy":{"b":1,"a":[1,{"d":2,"c":3}]}}"#,
        )
        .unwrap();
        let b: Value = serde_json::from_str(
            r#"{"policy":{"a":[1,{"c":3,"d":2}],"b":1},"schema":"s","name":"other"}"#,
        )
        .unwrap();
        assert_eq!(digest(&a), digest(&b));
        assert_eq!(
            canonical(&b),
            r#"{"name":"other","policy":{"a":[1,{"c":3,"d":2}],"b":1},"schema":"s"}"#
        );
    }

    #[test]
    fn the_reference_manifests_differ_in_executor_and_directions_only() {
        let luna = PolicyRecord::from_file(&reference("jevprobe3-luna.json")).unwrap();
        let opus = PolicyRecord::from_file(&reference("jevprobe2-opus-lean-low-5m.json")).unwrap();
        let changed: Vec<String> = diff(&luna.manifest, &opus.manifest)
            .into_iter()
            .map(|(field, ..)| field)
            .collect();
        assert_eq!(
            changed,
            [
                "policy.brief.directions",
                "policy.executor.agent",
                "policy.executor.effort",
                "policy.executor.model",
                "policy.executor.prompt_cache_ttl",
                "policy.executor.tools",
                "policy.executor.version",
            ]
        );
        assert_ne!(luna.digest, opus.digest);
        // Coder One's own tests pin the same digest for this file.
        assert_eq!(
            luna.digest,
            "bdefda51a03c05ed4322f668c6ea8e1695b3102f50261afb649ce87dd5411f85"
        );
        assert_eq!(luna.name.as_deref(), Some("coder-one-jevprobe3-luna"));
    }

    #[test]
    fn comparisons_group_by_policy_digest_when_present() {
        use crate::terminal_bench::{ComparisonGroup, test_attempt};
        let luna = PolicyRecord::from_file(&reference("jevprobe3-luna.json")).unwrap();
        let opus = PolicyRecord::from_file(&reference("jevprobe2-opus-lean-low-5m.json")).unwrap();
        let attempt = |arm: &str, trial: &str, policy: Option<&PolicyRecord>| {
            let mut attempt = test_attempt();
            attempt.arm = arm.to_owned();
            attempt.trial = trial.to_owned();
            attempt.policy = policy.cloned();
            attempt
        };
        let records = Records {
            attempts: vec![
                // Two arm names running one manifest pool.
                attempt("coder-one-jevprobe3-luna", "a", Some(&luna)),
                attempt("luna-renamed", "b", Some(&luna)),
                // One arm name whose manifest changed splits.
                attempt("coder-one-jevprobe3-luna", "c", Some(&opus)),
                // An attempt without a manifest groups by arm.
                attempt("coder-one-jevprobe3-luna", "d", None),
            ],
            ..Records::default()
        };
        let groups = ComparisonGroup::from_records(&records);
        assert_eq!(groups.len(), 3);
        let pooled = groups
            .iter()
            .find(|group| group.policy.as_deref() == Some(luna.digest.as_str()))
            .unwrap();
        assert_eq!(pooled.attempts.len(), 2);
        assert_eq!(pooled.arms, ["coder-one-jevprobe3-luna", "luna-renamed"]);
        assert_eq!(pooled.arm, format!("policy {}", luna.short()));
        assert!(pooled.named("luna-renamed") && pooled.named(&luna.digest[..8]));
        assert!(
            groups
                .iter()
                .any(|group| group.policy.is_none() && group.arm == "coder-one-jevprobe3-luna")
        );
    }

    #[test]
    fn a_query_finds_a_manifest_by_prefix_name_or_file() {
        let dir = reference("");
        let (entries, errors) = catalog(&Records::default(), &[dir.as_path()]);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(entries.len() >= 2);
        let luna = find(&entries, "coder-one-jevprobe3-luna").unwrap();
        assert_eq!(
            find(&entries, luna.record.short()).unwrap().record,
            luna.record
        );
        assert_eq!(
            find(&entries, "jevprobe3-luna.json").unwrap().record,
            luna.record
        );
        assert!(find(&entries, "nothing-like-it").is_err());
    }
}
