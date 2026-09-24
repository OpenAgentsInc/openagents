//! The post-executor snapshot, and checks replayed against a workspace.
//!
//! A Terminal-Bench experiment that changes a check, a repair brief, or a
//! stop rule used to rerun the whole trial, model session included. With
//! `verify.snapshot` in the policy, the composition saves what the first
//! executor left, right after its session and before any check runs:
//!
//! ```text
//! <episode>/snapshot/snapshot.json       what was taken, its size and digest
//! <episode>/snapshot/subject.json        the check's whole input but the files
//! <episode>/snapshot/workspace.tar.gz    the workdir and the outputs outside
//!                                        it, with paths relative to `/`
//! ```
//!
//! `coder-one snapshot checks --bundle DIR --workdir DIR` then runs
//! `verify.checks` on any workspace, such as the snapshot restored into
//! the task's image, from that subject, and with `--repair` writes the
//! repair brief the episode would have sent. It calls no model. A bundle
//! without a snapshot, such as one retained before this existed, gets a
//! subject reconstructed from its requirement map, invocation log, first
//! executor stream, and composition record; the output says which.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::checks::generic::{self, Claimed};
use crate::checks::{self, Budget, Report, Subject, TaskText};
use crate::record::Recorder;
use crate::requirements::RequirementMap;

/// The schema of `snapshot/snapshot.json`.
pub const SCHEMA: &str = "openagents.coder-one.snapshot.v1";

/// The schema of a `snapshot checks` result.
pub const REPLAY_SCHEMA: &str = "openagents.coder-one.snapshot-checks.v1";

/// The snapshot's directory, relative to the episode's.
pub const DIR: &str = "snapshot";
/// The archive, the subject, and the record, relative to [`DIR`].
pub const ARCHIVE: &str = "workspace.tar.gz";
pub const SUBJECT: &str = "subject.json";
pub const MANIFEST: &str = "snapshot.json";

fn default_max_mb() -> u64 {
    256
}

fn default_max_files() -> usize {
    50_000
}

/// `verify.snapshot`: take the post-executor snapshot, within bounds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotPolicy {
    /// The most bytes the workspace may hold before compression; a larger
    /// one is recorded as skipped, not archived.
    #[serde(default = "default_max_mb")]
    pub max_mb: u64,
    /// The most files the workspace may hold.
    #[serde(default = "default_max_files")]
    pub max_files: usize,
}

impl Default for SnapshotPolicy {
    fn default() -> Self {
        SnapshotPolicy {
            max_mb: default_max_mb(),
            max_files: default_max_files(),
        }
    }
}

/// Files and bytes under `paths`, without following links, or `None`
/// past `max_files`.
fn measure(paths: &[PathBuf], max_files: usize) -> Option<(usize, u64)> {
    let (mut files, mut bytes) = (0usize, 0u64);
    let mut stack: Vec<PathBuf> = paths.to_vec();
    while let Some(at) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&at) else {
            continue;
        };
        if meta.is_dir() {
            for entry in std::fs::read_dir(&at).ok()?.flatten() {
                stack.push(entry.path());
            }
        } else {
            files += 1;
            bytes += meta.len();
            if files > max_files {
                return None;
            }
        }
    }
    Some((files, bytes))
}

fn sha256_file(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

/// A path relative to `/`, as `tar -C /` wants it.
fn from_root(path: &Path) -> String {
    path.to_string_lossy().trim_start_matches('/').to_string()
}

/// Saves the workspace and the check's subject under `<dir>/snapshot/`
/// and returns the record, which also lands in `snapshot.json`.
///
/// `outside` names the output paths the requirements give outside the
/// workdir. The subject is saved even when the workspace is too large to
/// archive, so the checks can still be replayed against a candidate.
#[must_use]
pub fn take(
    dir: &Path,
    workdir: &Path,
    outside: &[PathBuf],
    subject: &Subject,
    policy: &SnapshotPolicy,
) -> Value {
    let started = Instant::now();
    let target = dir.join(DIR);
    let mut record = json!({
        "schema": SCHEMA,
        "stage": "post-executor",
        "workdir": workdir.to_string_lossy(),
        "outside": outside.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
        "policy": policy,
        "subject": format!("{DIR}/{SUBJECT}"),
        "archive": Value::Null,
        "taken": false,
    });
    let finish = |mut record: Value| {
        record["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
        if let Ok(text) = serde_json::to_string_pretty(&record) {
            let _ = crate::record::write_atomic(&target.join(MANIFEST), text.as_bytes());
        }
        record
    };
    match serde_json::to_string_pretty(subject) {
        Ok(text) => {
            if let Err(error) = crate::record::write_atomic(&target.join(SUBJECT), text.as_bytes())
            {
                record["reason"] = json!(error);
                return finish(record);
            }
        }
        Err(error) => {
            record["reason"] = json!(format!("cannot serialize the subject: {error}"));
            return finish(record);
        }
    }
    let mut roots = vec![workdir.to_path_buf()];
    roots.extend(outside.iter().filter(|p| p.exists()).cloned());
    let Some((files, bytes)) = measure(&roots, policy.max_files) else {
        record["reason"] = json!(format!("more than {} files", policy.max_files));
        return finish(record);
    };
    record["files"] = json!(files);
    record["bytes"] = json!(bytes);
    if bytes > policy.max_mb.saturating_mul(1024 * 1024) {
        record["reason"] = json!(format!("{bytes} bytes is over {} MB", policy.max_mb));
        return finish(record);
    }
    let archive = target.join(ARCHIVE);
    let mut command = std::process::Command::new("tar");
    command.arg("-czf").arg(&archive);
    // The episode's own directory never goes in, even under the workdir.
    if let Ok(inside) = dir.strip_prefix(workdir) {
        command.arg(format!("--exclude={}", from_root(&workdir.join(inside))));
    }
    command.arg("-C").arg("/");
    for root in &roots {
        command.arg(from_root(root));
    }
    match command.output() {
        // GNU tar exits 1 when a file changed while it read it; the
        // archive is still whole.
        Ok(out) if out.status.code().is_some_and(|code| code <= 1) && archive.is_file() => {
            record["taken"] = json!(true);
            record["archive"] = json!({
                "path": format!("{DIR}/{ARCHIVE}"),
                "bytes": std::fs::metadata(&archive).map(|m| m.len()).unwrap_or(0),
                "sha256": sha256_file(&archive),
                "paths": roots.iter().map(|p| from_root(p)).collect::<Vec<_>>(),
            });
            if out.status.code() == Some(1) {
                record["note"] = json!("tar reported files that changed while it read them");
            }
        }
        Ok(out) => {
            record["reason"] = json!(format!(
                "tar exited {}: {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
            let _ = std::fs::remove_file(&archive);
        }
        Err(error) => {
            record["reason"] = json!(format!("cannot run tar: {error}"));
        }
    }
    finish(record)
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// The episode directory of a bundle path: the episode itself, a Harbor
/// trial (`<trial>/agent/episode`), or a retained `<trial>.episode`.
#[must_use]
pub fn episode_dir(path: &Path) -> PathBuf {
    let nested = path.join("agent/episode");
    if nested.is_dir() {
        nested
    } else {
        path.to_path_buf()
    }
}

/// Where a check's subject came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// The episode's own `snapshot/subject.json`.
    Snapshot,
    /// Rebuilt from a bundle retained without one.
    Reconstructed,
}

/// The episode's instruction, title, first check input, and workdir, read
/// back from a bundle without a snapshot.
///
/// # Errors
///
/// Returns a message when the bundle has no invocation log or no user
/// instruction in it.
pub fn reconstruct(episode: &Path) -> Result<Subject, String> {
    let log = std::fs::read_to_string(episode.join("episode.atif.jsonl"))
        .map_err(|e| format!("{} has no episode.atif.jsonl: {e}", episode.display()))?;
    let mut instruction = None;
    let mut claimed = Vec::new();
    for line in log.lines() {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let step = &record["step"];
        if instruction.is_none() && step["source"] == "User" {
            instruction = step["message"].as_str().map(str::to_string);
        }
        let invocation = &step["extensions"]["invocation"];
        // The first check reads the commands reported before it started.
        if invocation["event"] == "start" && invocation["component"] == "verify.checks" {
            break;
        }
        let event = &step["extensions"][crate::session::EVENT_KEY]["event"];
        if event["kind"] == "command_completed" {
            claimed.push(Claimed {
                command: crate::stream::unwrap_shell(event["command"].as_str().unwrap_or_default()),
                exit_code: event["exit_code"].as_i64(),
            });
        }
    }
    let instruction = instruction.ok_or("the invocation log holds no user instruction")?;
    let title = read_json(&episode.join("artifacts/state.json"))
        .and_then(|state| state["issue"]["title"].as_str().map(str::to_string))
        .unwrap_or_else(|| instruction.lines().next().unwrap_or_default().to_string());
    let requirements: Option<RequirementMap> =
        read_json(&episode.join("artifacts/requirements.json"))
            .and_then(|v| serde_json::from_value(v).ok());
    let report = std::fs::read_to_string(episode.join("artifacts/delegate-1.stream.jsonl"))
        .ok()
        .and_then(|stream| checks::replay::final_report(&stream));
    let composition = read_json(&episode.join(crate::compose::FILE)).unwrap_or(Value::Null);
    let budget: Budget =
        serde_json::from_value(composition["horizon"]["check_budget"].clone()).unwrap_or_default();
    let command_sec = composition["horizon"]["command_sec"].as_u64().unwrap_or(60);
    let verify = &composition["verify"];
    let workdir = read_json(&episode.join("manifest.json"))
        .and_then(|m| m["workdir"].as_str().map(str::to_string))
        .unwrap_or_else(|| "/app".to_string());
    Ok(Subject {
        label: "terminal-bench task".to_string(),
        task: TaskText { title, instruction },
        requirements,
        provided: Vec::new(),
        inputs: None,
        budget,
        live: Some(generic::Workspace {
            dir: workdir,
            claimed,
            command_sec,
            report,
            options: generic::Options {
                self_report: verify["self_report"].as_bool().unwrap_or(false),
                optional_outputs: verify["optional_outputs"].as_bool().unwrap_or(false),
                behavior: verify["behavior"].as_bool().unwrap_or(false),
            },
            root: None,
            collected: Vec::new(),
            suite: None,
        }),
        distrust: verify["distrust"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|k| k.as_str().map(str::to_string))
            .collect(),
    })
}

/// The bundle's subject: the snapshot's when it has one, else rebuilt.
///
/// # Errors
///
/// Returns a message when neither is readable.
pub fn load(episode: &Path) -> Result<(Subject, Source), String> {
    let saved = episode.join(DIR).join(SUBJECT);
    if saved.is_file() {
        let text = std::fs::read_to_string(&saved)
            .map_err(|e| format!("cannot read {}: {e}", saved.display()))?;
        let subject = serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a check subject: {e}", saved.display()))?;
        return Ok((subject, Source::Snapshot));
    }
    reconstruct(episode).map(|subject| (subject, Source::Reconstructed))
}

/// Points the subject at `workdir`, the workspace the replay checks, and
/// sets its input records the way the episode does.
#[must_use]
pub fn retarget(mut subject: Subject, workdir: &Path) -> Subject {
    if subject.task.instruction.contains("logs/") && workdir.join("logs").is_dir() {
        subject.inputs = Some("logs".to_string());
    }
    if let Some(live) = &mut subject.live {
        live.dir = workdir.to_string_lossy().into_owned();
    }
    subject
}

/// The repair brief the episode's policy would send after `report`, with
/// the retained support report, and whether its trigger fires.
#[must_use]
pub fn repair_brief(episode: &Path, input: &checks::Input, report: &Report) -> Value {
    let composition = read_json(&episode.join(crate::compose::FILE)).unwrap_or(Value::Null);
    let trigger = composition["verify"]["repair"]["trigger"]
        .as_str()
        .and_then(|word| crate::repair::Trigger::parse(word).ok())
        .unwrap_or(crate::repair::Trigger::Detected);
    let support: Option<crate::support::Report> =
        read_json(&episode.join(crate::support::FILE)).and_then(|v| serde_json::from_value(v).ok());
    let found = crate::repair::gaps(report, support.as_ref());
    let triggered = match trigger {
        crate::repair::Trigger::Detected | crate::repair::Trigger::Unobserved => !found.is_empty(),
        crate::repair::Trigger::Checked => found.iter().any(|gap| !gap.packets.is_empty()),
        crate::repair::Trigger::Always => true,
    };
    let brief = (!found.is_empty())
        .then(|| crate::repair::packet_brief(&input.task, &input.candidate, report, &found).text);
    json!({
        "trigger": trigger.word(),
        "triggered": triggered,
        "gaps": found.len(),
        "support": support.is_some(),
        "brief": brief,
    })
}

/// The snapshot commands' usage.
pub const USAGE: &str = "usage: coder-one snapshot checks --bundle DIR --workdir DIR [--out DIR]
                                 [--subject FILE] [--policy FILE] [--repair] [--json]
       coder-one snapshot subject --bundle DIR

checks runs verify.checks on the workspace at --workdir with the subject of
the episode bundle at --bundle (an episode directory, a Harbor trial, or a
retained <trial>.episode): the snapshot's subject.json when the episode took
one, else one reconstructed from the bundle. --subject reads another subject
file. --policy runs the checks with that policy manifest's check options
(self-report, optional outputs, behavior scenarios) instead of the
subject's. The report goes to <out>/verification/checks.json (a scratch directory
by default). --repair also writes <out>/verification/repair-brief.md, the
brief the episode's repair policy would send, with the bundle's retained
support report. It calls no model. subject prints the subject.";

/// Runs a snapshot command and returns the exit code.
///
/// # Errors
///
/// Returns a message for bad arguments or an unreadable bundle.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let (mut bundle, mut workdir, mut out, mut subject_file) = (None, None, None, None);
    let mut policy: Option<PathBuf> = None;
    let (mut repair, mut json_output) = (false, false);
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--bundle" => bundle = Some(value("--bundle")?),
            "--workdir" => workdir = Some(value("--workdir")?),
            "--out" => out = Some(value("--out")?),
            "--subject" => subject_file = Some(value("--subject")?),
            "--policy" => policy = Some(value("--policy")?),
            "--repair" => repair = true,
            "--json" => json_output = true,
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    let episode = episode_dir(&bundle.ok_or("snapshot needs --bundle DIR")?);
    let (subject, source) = match subject_file {
        Some(path) => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let subject: Subject = serde_json::from_str(&text)
                .map_err(|e| format!("{} is not a check subject: {e}", path.display()))?;
            (subject, Source::Snapshot)
        }
        None => load(&episode)?,
    };
    match verb.as_str() {
        "subject" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&subject).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        "checks" => {
            let workdir = workdir.ok_or("snapshot checks needs --workdir DIR")?;
            let workdir = std::fs::canonicalize(&workdir)
                .map_err(|e| format!("cannot read {}: {e}", workdir.display()))?;
            let out = out.unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "coder-one-snapshot-checks-{}-{}",
                    std::process::id(),
                    atif::now_ms()
                ))
            });
            std::fs::create_dir_all(&out)
                .map_err(|e| format!("cannot create {}: {e}", out.display()))?;
            let mut subject = retarget(subject, &workdir);
            if let Some(path) = &policy {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
                let manifest = crate::policy::Manifest::parse(&text)?;
                let options = manifest
                    .policy
                    .verify
                    .map(|verify| verify.check_options())
                    .unwrap_or_default();
                if let Some(live) = &mut subject.live {
                    live.options = options;
                }
            }
            let started = Instant::now();
            let (input, report) = checks::check_subject_as(
                &subject,
                &workdir,
                &out,
                &Recorder::default(),
                checks::COVERAGE_FILE,
            )
            .await;
            let elapsed_ms = started.elapsed().as_millis() as u64;
            let retained: Option<Report> = read_json(&episode.join(checks::COVERAGE_FILE))
                .and_then(|v| serde_json::from_value(v).ok());
            let mut result = json!({
                "schema": REPLAY_SCHEMA,
                "bundle": episode.to_string_lossy(),
                "subject_source": source,
                "workdir": workdir.to_string_lossy(),
                "report": out.join(checks::COVERAGE_FILE).to_string_lossy(),
                "elapsed_ms": elapsed_ms,
                "detected": report.detected(),
                "summary": report.summary(),
                "verdicts": report.verdicts.iter().map(|v| json!({"scenario": v.scenario, "verdict": v.verdict})).collect::<Vec<_>>(),
                "retained": retained.as_ref().map(|r| json!({"detected": r.detected(), "summary": r.summary()})),
            });
            if repair {
                let mut brief = repair_brief(&episode, &input, &report);
                if let Some(text) = brief["brief"].as_str() {
                    let path = out.join("verification/repair-brief.md");
                    crate::record::write_atomic(&path, text.as_bytes())?;
                    brief["file"] = json!(path.to_string_lossy());
                }
                if let Some(object) = brief.as_object_mut() {
                    object.remove("brief");
                }
                result["repair"] = brief;
            }
            let _ = crate::record::write_atomic(
                &out.join("snapshot-checks.json"),
                serde_json::to_string_pretty(&result)
                    .unwrap_or_default()
                    .as_bytes(),
            );
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
                );
            } else {
                println!(
                    "checks on {} ({} subject): {} in {elapsed_ms} ms",
                    workdir.display(),
                    match source {
                        Source::Snapshot => "snapshot",
                        Source::Reconstructed => "reconstructed",
                    },
                    if report.detected() {
                        "detected a failure"
                    } else {
                        "no failure detected"
                    },
                );
                for verdict in &report.verdicts {
                    println!("  {:<40} {}", verdict.scenario, verdict.verdict);
                }
                if let Some(retained) = &retained {
                    println!(
                        "  the episode's own first check: {}",
                        if retained.detected() {
                            "detected a failure"
                        } else {
                            "no failure detected"
                        }
                    );
                }
                if let Some(repair) = result.get("repair") {
                    println!(
                        "  repair ({}): {}, {} gaps{}",
                        repair["trigger"].as_str().unwrap_or_default(),
                        if repair["triggered"] == true {
                            "would run"
                        } else {
                            "would not run"
                        },
                        repair["gaps"],
                        repair["file"]
                            .as_str()
                            .map(|f| format!(", brief at {f}"))
                            .unwrap_or_default()
                    );
                }
                println!("  report: {}", out.join(checks::COVERAGE_FILE).display());
            }
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(dir: &str) -> Subject {
        Subject {
            label: "terminal-bench task".to_string(),
            task: TaskText {
                title: "t".to_string(),
                instruction: "Write /app/out.txt with the answer.".to_string(),
            },
            requirements: None,
            provided: Vec::new(),
            inputs: None,
            budget: Budget::default(),
            live: Some(generic::Workspace {
                dir: dir.to_string(),
                claimed: vec![Claimed {
                    command: "python3 solve.py".to_string(),
                    exit_code: Some(0),
                }],
                command_sec: 30,
                report: Some("Done.".to_string()),
                options: generic::Options::default(),
                root: None,
                collected: Vec::new(),
                suite: None,
            }),
            distrust: Vec::new(),
        }
    }

    #[test]
    fn the_canary_is_luna_v2_with_a_snapshot() {
        let read = |name: &str| {
            let text = std::fs::read_to_string(crate::policy::reference_dir().join(name)).unwrap();
            crate::policy::Manifest::parse(&text).unwrap()
        };
        let canary = read("tunable-luna-snapshot.json");
        canary.validate().unwrap();
        let snapshot = canary.policy.verify.clone().unwrap().snapshot.unwrap();
        assert_eq!(snapshot, SnapshotPolicy::default());
        // Without the field, a policy serializes as it did before, so its
        // digest doesn't move.
        let before = read("tunable-luna-v2.json");
        let value = serde_json::to_value(&before).unwrap();
        assert!(value["policy"]["verify"].get("snapshot").is_none());
        let mut same = canary.clone();
        if let Some(verify) = &mut same.policy.verify {
            verify.snapshot = None;
        }
        assert_eq!(same.digest(), before.digest());
    }

    #[test]
    fn a_policy_without_fields_takes_the_defaults() {
        let policy: SnapshotPolicy = serde_json::from_str("{}").unwrap();
        assert_eq!(policy, SnapshotPolicy::default());
        assert!(serde_json::from_str::<SnapshotPolicy>(r#"{"max_gb": 1}"#).is_err());
    }

    #[test]
    fn take_archives_the_workdir_and_saves_the_subject() {
        let root = tempfile::tempdir().unwrap();
        let work = root.path().join("app");
        std::fs::create_dir_all(work.join("src")).unwrap();
        std::fs::write(work.join("src/main.py"), "print(1)\n").unwrap();
        let episode = root.path().join("episode");
        let taken = take(
            &episode,
            &work,
            &[],
            &subject(&work.to_string_lossy()),
            &SnapshotPolicy::default(),
        );
        assert_eq!(taken["taken"], true, "{taken}");
        assert_eq!(taken["files"], 1);
        let archive = episode.join(DIR).join(ARCHIVE);
        let listed = std::process::Command::new("tar")
            .arg("-tzf")
            .arg(&archive)
            .output()
            .unwrap();
        let names = String::from_utf8_lossy(&listed.stdout);
        assert!(names.contains("app/src/main.py"), "{names}");
        let (loaded, source) = load(&episode).unwrap();
        assert_eq!(source, Source::Snapshot);
        assert_eq!(loaded, subject(&work.to_string_lossy()));
        let record = read_json(&episode.join(DIR).join(MANIFEST)).unwrap();
        assert_eq!(record["archive"]["sha256"], taken["archive"]["sha256"]);
    }

    #[test]
    fn a_workspace_over_the_bound_keeps_only_the_subject() {
        let root = tempfile::tempdir().unwrap();
        let work = root.path().join("app");
        std::fs::create_dir_all(&work).unwrap();
        for i in 0..3 {
            std::fs::write(work.join(format!("f{i}")), "x").unwrap();
        }
        let episode = root.path().join("episode");
        let policy = SnapshotPolicy {
            max_mb: 1,
            max_files: 2,
        };
        let taken = take(&episode, &work, &[], &subject("/app"), &policy);
        assert_eq!(taken["taken"], false);
        assert_eq!(taken["reason"], "more than 2 files");
        assert!(episode.join(DIR).join(SUBJECT).is_file());
        assert!(!episode.join(DIR).join(ARCHIVE).exists());
    }

    #[test]
    fn a_bundle_without_a_snapshot_is_reconstructed() {
        let root = tempfile::tempdir().unwrap();
        let episode = root.path();
        let steps = [
            json!({"record": "step", "step": {"at": 1, "source": "User", "message": "Fix /app/x.py so tests pass."}}),
            json!({"record": "step", "step": {"at": 2, "source": "System", "message": "", "extensions": {"executor_event": {"event": {"kind": "command_completed", "command": "bash -lc 'pytest -q'", "exit_code": 1}}}}}),
            json!({"record": "step", "step": {"at": 3, "source": "System", "message": "", "extensions": {"invocation": {"event": "start", "id": "inv-9", "component": "verify.checks"}}}}),
            json!({"record": "step", "step": {"at": 4, "source": "System", "message": "", "extensions": {"executor_event": {"event": {"kind": "command_completed", "command": "ls"}}}}}),
        ];
        let log: String = steps.iter().map(|s| format!("{s}\n")).collect();
        std::fs::write(episode.join("episode.atif.jsonl"), log).unwrap();
        std::fs::create_dir_all(episode.join("artifacts")).unwrap();
        std::fs::write(
            episode.join(crate::compose::FILE),
            json!({"horizon": {"check_budget": {"max_scenarios": 5, "seconds": 200}, "command_sec": 90}, "verify": {"self_report": true}}).to_string(),
        )
        .unwrap();
        std::fs::write(
            episode.join("manifest.json"),
            json!({"workdir": "/srv/work"}).to_string(),
        )
        .unwrap();
        let (subject, source) = load(episode).unwrap();
        assert_eq!(source, Source::Reconstructed);
        assert_eq!(subject.task.instruction, "Fix /app/x.py so tests pass.");
        assert_eq!(subject.budget.max_scenarios, 5);
        let live = subject.live.clone().unwrap();
        assert_eq!(live.dir, "/srv/work");
        assert_eq!(live.command_sec, 90);
        assert!(live.options.self_report);
        assert_eq!(
            live.claimed,
            vec![Claimed {
                command: "pytest -q".to_string(),
                exit_code: Some(1)
            }]
        );
        let moved = retarget(subject, Path::new("/tmp/elsewhere"));
        assert_eq!(moved.live.unwrap().dir, "/tmp/elsewhere");
    }
}
