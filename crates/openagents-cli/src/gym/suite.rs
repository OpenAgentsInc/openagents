//! `openagents gym suite` — list, show, and check pinned suite manifests.
//!
//! This is the Rust port of the suite manifest tooling in
//! `packages/coder-effectiveness/src/suite-manifest.ts` and the `--check`
//! semantics from `bench/build-suites.mjs`. It reads the checked-in
//! `openagents.effectiveness_suite.v1` manifests under `bench/suites` and
//! refuses drift against the suite digests recorded in `bench-results`.

use crate::errors::CliError;
use crate::gym::schemas::{
    SUITE_MANIFEST_VIEW_SCHEMA, SuiteManifestView, SuiteManifestViewTask, SuiteTaskPin,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const SUITE_MANIFEST_SCHEMA: &str = "openagents.effectiveness_suite.v1";

/// The checked-in manifest format. `openagents.effectiveness_suite.v1`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SuiteManifest {
    schema: String,
    id: String,
    tier: String,
    description: String,
    tasks: Vec<SuiteTask>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SuiteTask {
    id: String,
    pin: Pin,
    environment_available: bool,
    rationale: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Pin {
    #[serde(rename = "harbor-registry")]
    HarborRegistry {
        dataset: String,
        #[serde(rename = "gitUrl")]
        git_url: String,
        commit: String,
        path: String,
    },
    #[serde(rename = "tracker-closed-issue")]
    TrackerClosedIssue {
        repo: String,
        issue: u64,
        #[serde(rename = "acceptedCommit")]
        accepted_commit: String,
    },
}

/// Subcommand surface for `openagents gym suite`.
#[derive(clap::Args, Debug)]
pub struct SuiteArgs {
    #[command(subcommand)]
    pub action: SuiteAction,
}

#[derive(clap::Subcommand, Debug)]
pub enum SuiteAction {
    /// List installed suite manifests with tier and digest.
    List,
    /// Show one suite and its task pins.
    Show {
        /// Suite id (the `id` field in the manifest, not the file name).
        id: String,
    },
    /// Rebuild the manifest from its pins and diff against the recorded digest.
    Check {
        /// Path to a `.suite.json` manifest.
        file: PathBuf,
    },
}

/// Run one `gym suite` subcommand.
pub fn run_suite(action: SuiteAction, json: bool) -> Result<(), CliError> {
    match action {
        SuiteAction::List => list_suites(json),
        SuiteAction::Show { id } => show_suite(&id, json),
        SuiteAction::Check { file } => check_suite(&file, json),
    }
}

// ---------------------------------------------------------------------------
// manifest reading
// ---------------------------------------------------------------------------

fn read_manifest(path: &Path) -> Result<SuiteManifest, CliError> {
    let text = fs::read_to_string(path).map_err(|e| {
        CliError::Configuration(format!("could not read {}: {}", path.display(), e))
    })?;
    let mut manifest: SuiteManifest = serde_json::from_str(&text).map_err(|e| {
        CliError::Configuration(format!(
            "{} is not a valid suite manifest: {}",
            path.display(),
            e
        ))
    })?;
    if manifest.schema != SUITE_MANIFEST_SCHEMA {
        return Err(CliError::Configuration(format!(
            "{} has schema '{}', expected '{}'",
            path.display(),
            manifest.schema,
            SUITE_MANIFEST_SCHEMA
        )));
    }
    if manifest.tasks.is_empty() {
        return Err(CliError::Configuration(format!(
            "suite {} declares no tasks; an empty suite covers itself and would score nothing",
            manifest.id
        )));
    }
    let mut seen = HashSet::new();
    for task in &manifest.tasks {
        if !seen.insert(task.id.clone()) {
            return Err(CliError::Configuration(format!(
                "suite {} names task {} twice",
                manifest.id, task.id
            )));
        }
    }
    if manifest.tier == "score" {
        let unavailable: Vec<_> = manifest
            .tasks
            .iter()
            .filter(|t| !t.environment_available)
            .map(|t| t.id.as_str())
            .collect();
        if !unavailable.is_empty() {
            return Err(CliError::Configuration(format!(
                "suite {} is tier score but holds {} task(s) with no environment: {}",
                manifest.id,
                unavailable.len(),
                unavailable.join(", ")
            )));
        }
    }
    // Stable order makes listing and show output deterministic.
    manifest
        .tasks
        .sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
    Ok(manifest)
}

fn suite_dir() -> Result<PathBuf, CliError> {
    let start = std::env::current_dir()
        .map_err(|e| CliError::Configuration(format!("could not read current directory: {e}")))?;
    for dir in start.ancestors() {
        let candidate = dir.join("bench").join("suites");
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }
    Err(CliError::Configuration(
        "could not find bench/suites from the current directory".to_string(),
    ))
}

fn manifest_path(suite_dir: &Path, id: &str) -> PathBuf {
    suite_dir.join(format!("{}.suite.json", id))
}

fn manifest_files() -> Result<Vec<PathBuf>, CliError> {
    let dir = suite_dir()?;
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| CliError::Configuration(format!("could not read {}: {}", dir.display(), e)))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .filter(|p| p.to_string_lossy().ends_with(".suite.json"))
        .collect();
    paths.sort();
    Ok(paths)
}

// ---------------------------------------------------------------------------
// views and rendering
// ---------------------------------------------------------------------------

fn to_view(
    manifest: &SuiteManifest,
    include_tasks: bool,
    include_description: bool,
) -> SuiteManifestView {
    let task_digests: Vec<_> = manifest.tasks.iter().map(task_digest).collect();
    let digest = suite_digest(manifest, &task_digests);
    SuiteManifestView {
        schema: crate::gym::schemas::SUITE_MANIFEST_VIEW_SCHEMA.to_string(),
        suite_id: manifest.id.clone(),
        suite_digest: digest,
        tier: manifest.tier.clone(),
        description: if include_description {
            manifest.description.clone()
        } else {
            String::new()
        },
        task_count: manifest.tasks.len() as u64,
        source_path: None,
        tasks: if include_tasks {
            manifest.tasks.iter().map(task_to_view).collect()
        } else {
            Vec::new()
        },
    }
}

fn task_to_view(task: &SuiteTask) -> SuiteManifestViewTask {
    let task_digest = task_digest(task);
    SuiteManifestViewTask {
        id: task.id.clone(),
        task_digest,
        environment_available: task.environment_available,
        rationale: task.rationale.clone(),
        pin: match &task.pin {
            Pin::HarborRegistry {
                dataset,
                git_url,
                commit,
                path,
            } => SuiteTaskPin::HarborRegistry {
                dataset: dataset.clone(),
                git_url: git_url.clone(),
                commit: commit.clone(),
                path: path.clone(),
            },
            Pin::TrackerClosedIssue {
                repo,
                issue,
                accepted_commit,
            } => SuiteTaskPin::TrackerClosedIssue {
                repo: repo.clone(),
                issue: *issue,
                accepted_commit: accepted_commit.clone(),
            },
        },
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), CliError> {
    let text = serde_json::to_string(value)
        .map_err(|e| CliError::Output(format!("could not render JSON output: {e}")))?;
    println!("{text}");
    Ok(())
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn list_suites(json: bool) -> Result<(), CliError> {
    let paths = manifest_files()?;
    let mut views = Vec::with_capacity(paths.len());
    for path in &paths {
        let manifest = read_manifest(path)?;
        views.push(to_view(&manifest, false, false));
    }
    if json {
        let document = serde_json::json!({
            "schema": SUITE_MANIFEST_VIEW_SCHEMA,
            "suites": views,
        });
        return print_json(&document);
    }
    if views.is_empty() {
        println!("No suite manifests found.");
        return Ok(());
    }
    for v in views {
        println!(
            "{}  {}  {}  {}",
            v.suite_id, v.tier, v.task_count, v.suite_digest
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------

fn show_suite(id: &str, json: bool) -> Result<(), CliError> {
    let dir = suite_dir()?;
    let path = manifest_path(&dir, id);
    let manifest = read_manifest(&path)?;
    let view = to_view(&manifest, true, true);
    if json {
        let document = serde_json::json!({
            "schema": SUITE_MANIFEST_VIEW_SCHEMA,
            "manifest": view,
        });
        return print_json(&document);
    }
    println!(
        "{}  {}  {}  {} task{}",
        view.suite_id,
        view.tier,
        view.suite_digest,
        view.task_count,
        if view.task_count == 1 { "" } else { "s" }
    );
    for task in &view.tasks {
        match &task.pin {
            SuiteTaskPin::HarborRegistry {
                dataset,
                git_url,
                commit,
                path,
            } => {
                println!(
                    "{}  {}  {}  {}  {}",
                    task.id, dataset, git_url, commit, path
                );
            }
            SuiteTaskPin::TrackerClosedIssue {
                repo,
                issue,
                accepted_commit,
            } => {
                println!(
                    "{}  tracker-closed-issue  {}  #{}  {}",
                    task.id, repo, issue, accepted_commit
                );
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// check
// ---------------------------------------------------------------------------

fn repo_root_for_suite(path: &Path) -> Option<PathBuf> {
    // bench/suites/<id>.suite.json -> repo root is two directories up from the file.
    path.parent()?.parent()?.parent().map(PathBuf::from)
}

fn recorded_digest(id: &str, repo_root: &Path) -> Option<String> {
    let results = repo_root
        .join("bench-results")
        .join(format!("{}.jsonl", id));
    let text = fs::read_to_string(&results).ok()?;
    let first = text.lines().next()?;
    let value: serde_json::Value = serde_json::from_str(first).ok()?;
    value.get("suiteDigest")?.as_str().map(String::from)
}

fn check_suite(path: &Path, json: bool) -> Result<(), CliError> {
    let manifest = read_manifest(path)?;
    let task_digests: Vec<_> = manifest.tasks.iter().map(task_digest).collect();
    let digest = suite_digest(&manifest, &task_digests);

    let repo_root = repo_root_for_suite(path).ok_or_else(|| {
        CliError::Configuration(format!(
            "could not locate the repository root for {}",
            path.display()
        ))
    })?;
    let recorded = recorded_digest(&manifest.id, &repo_root);

    let drifted = recorded.as_ref().is_some_and(|r| r != &digest);
    if !drifted {
        if json {
            let document = serde_json::json!({
                "schema": SUITE_MANIFEST_VIEW_SCHEMA,
                "id": manifest.id,
                "drifted": false,
                "digest": digest,
                "recorded_digest": recorded,
            });
            return print_json(&document);
        }
        match recorded {
            Some(r) => println!("ok  {}  {}  recorded {}", manifest.id, digest, r),
            None => println!(
                "ok  {}  {}  (no recorded results to compare)",
                manifest.id, digest
            ),
        }
        return Ok(());
    }

    let recorded = recorded.unwrap();
    let mut diff = Vec::new();
    diff.push(format!("drift detected in {}", path.display()));
    diff.push(format!("  recorded digest: {}", recorded));
    diff.push(format!("  current digest:  {}", digest));
    diff.push("  current task pins:".to_string());
    for (task, td) in manifest.tasks.iter().zip(task_digests.iter()) {
        match &task.pin {
            Pin::HarborRegistry {
                dataset,
                git_url,
                commit,
                path,
            } => {
                diff.push(format!(
                    "    {}  {}  {}  {}  {}  {}",
                    task.id, dataset, git_url, commit, path, td
                ));
            }
            Pin::TrackerClosedIssue {
                repo,
                issue,
                accepted_commit,
            } => {
                diff.push(format!(
                    "    {}  tracker-closed-issue  {}  #{}  {}  {}",
                    task.id, repo, issue, accepted_commit, td
                ));
            }
        }
    }

    if json {
        let document = serde_json::json!({
            "schema": SUITE_MANIFEST_VIEW_SCHEMA,
            "id": manifest.id,
            "drifted": true,
            "recorded_digest": recorded,
            "current_digest": digest,
            "tasks": manifest.tasks.iter().map(task_to_view).collect::<Vec<_>>(),
        });
        print_json(&document)?;
    } else {
        for line in &diff {
            println!("{line}");
        }
    }
    Err(CliError::Internal(format!(
        "suite {} has drifted from its recorded digest",
        manifest.id
    )))
}

// ---------------------------------------------------------------------------
// digests
// ---------------------------------------------------------------------------

fn hex_digest(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in hash.iter() {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalHarborTask<'a> {
    id: &'a str,
    kind: &'a str,
    dataset: &'a str,
    git_url: &'a str,
    commit: &'a str,
    path: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalTrackerTask<'a> {
    id: &'a str,
    kind: &'a str,
    repo: &'a str,
    issue: u64,
    accepted_commit: &'a str,
}

#[derive(Serialize)]
struct CanonicalSuite<'a> {
    schema: &'a str,
    id: &'a str,
    tier: &'a str,
    tasks: &'a [String],
}

fn task_digest(task: &SuiteTask) -> String {
    let text = match &task.pin {
        Pin::HarborRegistry {
            dataset,
            git_url,
            commit,
            path,
        } => serde_json::to_string(&CanonicalHarborTask {
            id: &task.id,
            kind: "harbor-registry",
            dataset,
            git_url,
            commit,
            path,
        }),
        Pin::TrackerClosedIssue {
            repo,
            issue,
            accepted_commit,
        } => serde_json::to_string(&CanonicalTrackerTask {
            id: &task.id,
            kind: "tracker-closed-issue",
            repo,
            issue: *issue,
            accepted_commit,
        }),
    }
    .unwrap_or_default();
    format!("task:{}", hex_digest(text.as_bytes()))
}

fn suite_digest(manifest: &SuiteManifest, task_digests: &[String]) -> String {
    let mut sorted = task_digests.to_vec();
    sorted.sort();
    let source = serde_json::to_string(&CanonicalSuite {
        schema: &manifest.schema,
        id: &manifest.id,
        tier: &manifest.tier,
        tasks: &sorted,
    })
    .unwrap_or_default();
    format!("suite-manifest:{}", hex_digest(source.as_bytes()))
}

/// Manifest facts `gym results` needs without running the suite.
#[derive(Debug, Clone)]
pub struct SuiteMeta {
    pub id: String,
    pub tier: String,
    pub digest: String,
    pub task_ids: Vec<String>,
}

/// Read a suite manifest by id. Drift is reported, not refused — scoring a
/// job directory does not require the pins to still match.
pub fn suite_meta(id: &str) -> Result<SuiteMeta, CliError> {
    suite_meta_in(&suite_dir()?, id)
}

/// Test seam: read a suite from an explicit suites directory.
pub fn suite_meta_in(suites_dir: &Path, id: &str) -> Result<SuiteMeta, CliError> {
    let path = manifest_path(suites_dir, id);
    let manifest = read_manifest(&path)?;
    let task_digests: Vec<_> = manifest.tasks.iter().map(task_digest).collect();
    let digest = suite_digest(&manifest, &task_digests);
    Ok(SuiteMeta {
        id: manifest.id,
        tier: manifest.tier,
        digest,
        task_ids: manifest.tasks.iter().map(|t| t.id.clone()).collect(),
    })
}

/// A suite resolved for execution. `gym run` consumes this.
#[derive(Debug, Clone)]
pub struct ResolvedTask {
    pub id: String,
    pub dataset: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedSuite {
    pub id: String,
    pub tasks: Vec<ResolvedTask>,
}

/// Resolve a suite by id, verify its pins have not drifted, and return the
/// data `gym run` needs. Drift is refused before any run is registered.
pub fn resolve_for_run(id: &str) -> Result<ResolvedSuite, CliError> {
    resolve_for_run_in(&suite_dir()?, id)
}

/// Resolve a suite from an explicit suites directory. Used by tests.
pub fn resolve_for_run_in(suites_dir: &Path, id: &str) -> Result<ResolvedSuite, CliError> {
    let path = manifest_path(suites_dir, id);
    let manifest = read_manifest(&path)?;
    let task_digests: Vec<_> = manifest.tasks.iter().map(task_digest).collect();
    let digest = suite_digest(&manifest, &task_digests);

    let repo_root = repo_root_for_suite(&path).ok_or_else(|| {
        CliError::Configuration(format!(
            "could not locate the repository root for {}",
            path.display()
        ))
    })?;
    if let Some(recorded) = recorded_digest(&manifest.id, &repo_root) {
        if recorded != digest {
            return Err(CliError::Configuration(format!(
                "suite {} has drifted from its recorded digest (recorded {}, current {})",
                manifest.id, recorded, digest
            )));
        }
    }

    let mut tasks = Vec::with_capacity(manifest.tasks.len());
    for t in &manifest.tasks {
        match &t.pin {
            Pin::HarborRegistry { dataset, .. } => tasks.push(ResolvedTask {
                id: t.id.clone(),
                dataset: dataset.clone(),
            }),
            Pin::TrackerClosedIssue { .. } => {
                return Err(CliError::Configuration(format!(
                    "task {} uses a tracker-closed-issue pin; gym run supports harbor-registry tasks only",
                    t.id
                )));
            }
        }
    }

    Ok(ResolvedSuite {
        id: manifest.id.clone(),
        tasks,
    })
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn suite_digest_matches_recorded_tb2_quick() {
        let dir = suite_dir().expect("suite dir");
        let path = manifest_path(&dir, "tb2-quick");
        let manifest = read_manifest(&path).expect("read tb2-quick");
        let digests: Vec<_> = manifest.tasks.iter().map(task_digest).collect();
        let digest = suite_digest(&manifest, &digests);
        // The recorded digest in bench-results/tb2-quick.jsonl
        assert_eq!(
            digest,
            "suite-manifest:b418d40416b197791d4f148758c9d78680783bcd05bf8c011c1d460b25ea6125"
        );
    }

    #[test]
    fn check_passes_on_clean_tb2_quick() {
        let dir = suite_dir().expect("suite dir");
        let path = manifest_path(&dir, "tb2-quick");
        assert!(check_suite(&path, false).is_ok());
    }

    #[test]
    fn check_fails_when_a_pin_is_edited() {
        let dir = tempfile::tempdir().unwrap();
        let bench = dir.path().join("bench").join("suites");
        fs::create_dir_all(&bench).unwrap();
        let suite_path = bench.join("tb2-quick.suite.json");

        let original =
            fs::read_to_string(manifest_path(&suite_dir().unwrap(), "tb2-quick")).unwrap();
        // Swap the commit to a different 40-character hex string.
        let edited = original.replace(
            "69671fbaac6d67a7ef0dfec016cc38a64ef7a77c",
            "0000000000000000000000000000000000000000",
        );
        fs::write(&suite_path, edited).unwrap();

        // Record the original digest for the suite id.
        let results = dir.path().join("bench-results");
        fs::create_dir_all(&results).unwrap();
        let record = r#"{"schema":"openagents.bench_result.v2","suiteId":"tb2-quick","suiteDigest":"suite-manifest:b418d40416b197791d4f148758c9d78680783bcd05bf8c011c1d460b25ea6125"}"#;
        let mut f = fs::File::create(results.join("tb2-quick.jsonl")).unwrap();
        f.write_all(record.as_bytes()).unwrap();
        f.write_all(b"\n").unwrap();

        let result = check_suite(&suite_path, false);
        assert!(result.is_err(), "expected drift to be reported");
    }

    #[test]
    fn list_includes_tb2_quick() {
        let dir = suite_dir().expect("suite dir");
        let paths = manifest_files().expect("manifest files");
        assert!(paths.iter().any(|p| p == &dir.join("tb2-quick.suite.json")));
    }

    #[test]
    fn show_tb2_quick_has_two_harbor_registry_pins() {
        let dir = suite_dir().expect("suite dir");
        let path = manifest_path(&dir, "tb2-quick");
        let manifest = read_manifest(&path).expect("read");
        let view = to_view(&manifest, true, true);
        assert_eq!(view.suite_id, "tb2-quick");
        assert_eq!(view.task_count, 2);
        assert_eq!(view.tasks.len(), 2);
        assert!(
            view.tasks
                .iter()
                .all(|t| matches!(t.pin, SuiteTaskPin::HarborRegistry { .. }))
        );
    }

    #[test]
    fn coderbench_agent_building_v1_resolves_as_harbor_registry_smoke() {
        let dir = suite_dir().expect("suite dir");
        let suite = resolve_for_run_in(&dir, "coderbench-agent-building-v1").expect("resolve");
        assert_eq!(suite.id, "coderbench-agent-building-v1");
        assert_eq!(suite.tasks.len(), 2);
        assert!(
            suite
                .tasks
                .iter()
                .all(|t| t.dataset == "terminal-bench@2.0")
        );
        let meta = suite_meta_in(&dir, "coderbench-agent-building-v1").expect("meta");
        assert_eq!(meta.tier, "smoke");
    }
}
