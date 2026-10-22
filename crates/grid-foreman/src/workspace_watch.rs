//! Observe native Kiln workspace saves without reading their advance lock.
//!
//! Only fixed action summaries, scoped paths, check results, and returned
//! serving identities leave this process. Action times are source-save times;
//! the workspace does not record when individual commands ran.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pylons_view::work::{
    self, WorkAnswer, WorkCheck, WorkPhase, WorkSnapshot, WorkStep, WorkStepKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;
const CURSORS: &str = "workspace-watch.json";

/// The explicit public mapping from a private state file to its provider.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub statefile: PathBuf,
    pub pylon: String,
    pub issue: Option<u64>,
    pub title: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    entries: Vec<Entry>,
}

/// Settings for the read-only workspace observer.
pub struct Options {
    /// Contains the existing publisher key and observer cursors.
    pub state: PathBuf,
    pub config: PathBuf,
    pub site: String,
    pub run_for: Option<Duration>,
}

#[derive(Default, Deserialize, Serialize)]
struct Cursor {
    session: String,
    started: u64,
    sequence: u64,
    #[serde(default)]
    mapping: String,
    #[serde(default)]
    actions: Vec<SeenAction>,
}

#[derive(Deserialize, Serialize)]
struct SeenAction {
    hash: String,
    at: u64,
}

#[derive(Deserialize)]
struct SourceConfig {
    base: String,
    paths: Vec<String>,
    pylon: String,
    model: String,
    profile: String,
}

#[derive(Deserialize)]
struct Source {
    config: SourceConfig,
    agent: Agent,
    files: BTreeMap<String, String>,
    original: BTreeMap<String, String>,
    phase: String,
    turns: u32,
    calls: u32,
    until: u64,
    checked: Option<String>,
    #[serde(default)]
    answers: Vec<Answer>,
}

#[derive(Deserialize)]
struct Agent {
    steps: Vec<Step>,
}

#[derive(Deserialize)]
struct Step {
    observations: Vec<Observation>,
}

#[derive(Deserialize)]
struct Observation {
    command: String,
    code: i32,
}

#[derive(Deserialize)]
struct Answer {
    request: Option<String>,
    pylon: Option<String>,
    model: Option<String>,
    profile: Option<String>,
    lossy: Option<bool>,
}

fn hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn producer(address: &str) -> Option<&str> {
    let mut parts = address.split(':');
    let (Some("30200"), Some(key), Some(slug), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    (hex(key)
        && !slug.is_empty()
        && slug.len() <= 128
        && slug
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)))
    .then_some(key)
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 240
        && !path.starts_with('/')
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
        && path.split('/').all(|part| {
            let lower = part.to_ascii_lowercase();
            !matches!(part, "" | "." | ".." | ".git")
                && !lower.starts_with(".env")
                && !lower.contains("secret")
                && !lower.contains("credential")
                && !matches!(
                    lower.as_str(),
                    "provider.key" | "id_rsa" | "id_ed25519" | "google_compute_engine"
                )
        })
}

fn scoped(path: &str, source: &Source) -> bool {
    valid_path(path)
        && source.config.paths.iter().any(|scope| {
            path == scope
                || path
                    .strip_prefix(scope)
                    .is_some_and(|tail| tail.starts_with('/'))
        })
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn action(command: &str, code: i32, source: &Source, sequence: u64, at: u64) -> WorkStep {
    let args: Vec<_> = command.split_whitespace().collect();
    let (kind, label, path) = match args.first().copied() {
        Some("view") => (WorkStepKind::Read, "Read file", args.get(1).copied()),
        Some("ls") => (WorkStepKind::Read, "List files", args.get(1).copied()),
        Some("grep") => (WorkStepKind::Read, "Search files", args.last().copied()),
        Some("write" | "patch") => (WorkStepKind::Write, "Edit file", args.get(1).copied()),
        Some("copy") => (WorkStepKind::Write, "Copy file", args.get(2).copied()),
        Some("check") if args.len() == 1 => (WorkStepKind::Check, "Workspace checks", None),
        _ => (WorkStepKind::Command, "Tool action", None),
    };
    // Search terms, content, shell text, and model summaries are never copied.
    let path = path
        .map(|p| p.trim_matches(['\'', '"']))
        .filter(|p| scoped(p, source))
        .map(str::to_owned);
    WorkStep {
        sequence,
        at,
        kind,
        label: label.into(),
        path,
        summary: if kind == WorkStepKind::Check && code == 0 {
            "Checks updated."
        } else if code == 0 {
            "Completed."
        } else {
            "Needs attention."
        }
        .into(),
    }
}

fn answer(source: &Source, entry: &Entry) -> Result<Option<WorkAnswer>, String> {
    let expected = producer(&entry.pylon).ok_or("invalid observer provider")?;
    let Some(answer) = source.answers.last() else {
        return Ok(None);
    };
    let (Some(request), Some(provider), Some(model), Some(profile), Some(lossy)) = (
        &answer.request,
        &answer.pylon,
        &answer.model,
        &answer.profile,
        answer.lossy,
    ) else {
        return Ok(None);
    };
    if !hex(request)
        || producer(provider) != Some(expected)
        || model != &source.config.model
        || profile != &source.config.profile
        || model.len() > 256
    {
        return Err("returned serving identity differs from the watched workspace".into());
    }
    Ok(Some(WorkAnswer {
        request: request.clone(),
        provider: expected.into(),
        model: model.clone(),
        profile: profile.clone(),
        lossy,
    }))
}

fn snapshot(
    entry: &Entry,
    source: &Source,
    saved_at: u64,
    now: u64,
    cursor: &mut Cursor,
) -> Result<WorkSnapshot, String> {
    if producer(&entry.pylon) != Some(source.config.pylon.as_str())
        || source.config.profile != "consumer-q"
        || source.config.base.len() != 40
        || !source.config.base.bytes().all(|b| b.is_ascii_hexdigit())
        || source.config.paths.is_empty()
        || source.config.paths.iter().any(|p| !valid_path(p))
        || entry.title.is_empty()
        || entry.title.len() > 160
        || entry.title.chars().any(char::is_control)
    {
        return Err("workspace does not match its approved observer mapping".into());
    }
    let identity = serde_json::to_vec(&(
        &entry.pylon,
        &entry.statefile,
        &source.config.base,
        source.until,
    ))
    .map_err(|_| "could not identify workspace session")?;
    let session = hash(&identity);
    let mapping = hash(
        &serde_json::to_vec(&(&entry.issue, &entry.title))
            .map_err(|_| "could not identify task mapping")?,
    );
    if cursor.session == session && !cursor.mapping.is_empty() && cursor.mapping != mapping {
        return Err("task label changed without a new workspace session".into());
    }
    if cursor.session != session {
        cursor.started = saved_at.min(now).max(cursor.started.saturating_add(1));
        cursor.session = session.clone();
        cursor.sequence = 0;
        cursor.actions.clear();
    }
    cursor.mapping = mapping;
    cursor.sequence = cursor
        .sequence
        .checked_add(1)
        .ok_or("observer sequence exhausted")?;
    let phase = match source.phase.as_str() {
        "model" => WorkPhase::Working,
        "awaiting-check" => WorkPhase::Checking,
        "review" => WorkPhase::Done,
        "stopped" => WorkPhase::Failed,
        _ => return Err("unknown workspace phase".into()),
    };
    let digest = hash(
        &serde_json::to_vec(&(&source.config.base, &source.files))
            .map_err(|_| "could not check workspace files")?,
    );
    let checked = source.checked.as_deref() == Some(digest.as_str());
    if source.phase == "review" && !checked {
        return Err("review workspace has no passing check for its current files".into());
    }
    let mut steps = Vec::new();
    let mut sequence = 0_u64;
    let mut last_edit = 0;
    let mut failed_check = 0;
    for step in &source.agent.steps {
        for observation in &step.observations {
            let index = sequence as usize;
            sequence += 1;
            let fingerprint = hash(
                &serde_json::to_vec(&(&observation.command, observation.code))
                    .map_err(|_| "could not identify workspace action")?,
            );
            if let Some(seen) = cursor.actions.get(index) {
                if seen.hash != fingerprint {
                    return Err("saved workspace action history changed".into());
                }
            } else {
                cursor.actions.push(SeenAction {
                    hash: fingerprint,
                    at: saved_at.min(now),
                });
            }
            let action = action(
                &observation.command,
                observation.code,
                source,
                sequence,
                cursor.actions[index].at,
            );
            if action.kind == WorkStepKind::Write && observation.code == 0 {
                last_edit = sequence;
            }
            if action.kind == WorkStepKind::Check && observation.code != 0 {
                failed_check = sequence;
            }
            steps.push(action);
        }
    }
    // A successful `check` command requests coordinator checks. Only a matching
    // current-files digest establishes that those checks passed.
    let checks = vec![WorkCheck {
        label: "Current files".into(),
        status: if checked {
            "passed"
        } else if failed_check > last_edit {
            "failed"
        } else {
            "pending"
        }
        .into(),
        summary: if checked {
            "Checked the current files."
        } else {
            "Current files need checks."
        }
        .into(),
    }];
    if cursor.actions.len() > sequence as usize {
        return Err("saved workspace action history moved backward".into());
    }
    steps.drain(..steps.len().saturating_sub(work::MAX_STEPS));
    let mut current = steps.last().cloned().unwrap_or(WorkStep {
        sequence: 1,
        at: saved_at.min(now),
        kind: WorkStepKind::Command,
        label: "Workspace opened".into(),
        path: None,
        summary: "Waiting for the next update.".into(),
    });
    if source.phase == "awaiting-check" {
        current.at = saved_at.min(now);
        current.label = "Waiting for checks".into();
        current.kind = WorkStepKind::Check;
        current.path = None;
        current.summary = "The coordinator needs to check these files.".into();
    } else if source.phase == "review" {
        current.at = saved_at.min(now);
        current.label = "Ready for review".into();
        current.summary = "The workspace is waiting for review.".into();
    } else if source.phase == "stopped" {
        current.at = saved_at.min(now);
        current.label = "Workspace stopped".into();
        current.summary = "The workspace reached its work limit.".into();
    } else if now.saturating_sub(current.at) > 120 {
        current.summary = "Waiting for the next update.".into();
    }
    let changed: Vec<_> = source
        .files
        .iter()
        .filter(|(path, text)| source.original.get(*path) != Some(*text))
        .map(|(path, _)| path)
        .collect();
    Ok(WorkSnapshot {
        schema: work::SCHEMA.into(),
        pylon: entry.pylon.clone(),
        session_id: session,
        session_started_at: cursor.started,
        sequence: cursor.sequence,
        observed_at: now,
        issue: entry.issue,
        title: entry.title.clone(),
        phase,
        current: Some(current),
        turns: source.turns,
        calls: source.calls,
        changed: changed.len().try_into().unwrap_or(u32::MAX),
        files: changed
            .into_iter()
            .filter(|p| scoped(p, source))
            .take(work::MAX_FILES)
            .cloned()
            .collect(),
        checks,
        steps,
        answer: answer(source, entry)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (Entry, Source) {
        let entry = Entry {
            statefile: "/private/example/workspace.json".into(),
            pylon: format!("30200:{}:maya-consumer-q-1-head", "11".repeat(32)),
            issue: Some(12503),
            title: "Extract the preview worker".into(),
        };
        let source = serde_json::from_value(json!({
            "config": {"base":"a".repeat(40),"paths":["apps/haunt-preview"],
                "pylon":"11".repeat(32),"model":"deepseek-v4.1-flash@sha256:test",
                "profile":"consumer-q","key":"PRIVATE_KEY_PATH","task":"PRIVATE_TASK"},
            "agent": {"thought":"PRIVATE_THOUGHT","instructions":"PRIVATE_INSTRUCTIONS",
                "steps":[{"thought":"PRIVATE_THOUGHT","problem":"PRIVATE_PROBLEM",
                    "observations":[{"command":"grep PRIVATE_SEARCH apps/haunt-preview/src/main.ts",
                        "code":0,"output":"PRIVATE_OUTPUT"}]}]},
            "files":{"apps/haunt-preview/src/main.ts":"PRIVATE_CONTENT"},
            "original":{"apps/haunt-preview/src/main.ts":"PRIVATE_ORIGINAL"},
            "pending":[{"Write":{"path":"apps/haunt-preview/src/main.ts","content":"PRIVATE_PENDING"}}],
            "phase":"model","turns":2,"calls":1,"until":22600,"checked":null,"answers":[]
        })).unwrap();
        (entry, source)
    }

    #[test]
    fn publishes_actual_actions_without_private_text() {
        let (entry, source) = fixture();
        let report = snapshot(&entry, &source, 1000, 1010, &mut Cursor::default()).unwrap();
        report.validate(1010).unwrap();
        let bytes = serde_json::to_string(&report).unwrap();
        assert!(!bytes.contains("PRIVATE_"));
        assert_eq!(report.steps[0].label, "Search files");
        assert_eq!(
            report.steps[0].path.as_deref(),
            Some("apps/haunt-preview/src/main.ts")
        );
        assert_eq!((report.turns, report.calls, report.changed), (2, 1, 1));
        assert!(report.answer.is_none());
    }

    #[test]
    fn real_answer_must_match_selected_provider_and_model() {
        let (entry, mut source) = fixture();
        source.answers = vec![Answer {
            request: Some("ab".repeat(32)),
            pylon: Some(entry.pylon.clone()),
            model: Some(source.config.model.clone()),
            profile: Some("consumer-q".into()),
            lossy: Some(true),
        }];
        let report = snapshot(&entry, &source, 1000, 1010, &mut Cursor::default()).unwrap();
        report.validate(1010).unwrap();
        assert_eq!(report.answer.unwrap().provider, "11".repeat(32));
        source.answers[0].pylon = Some(format!("30200:{}:other", "22".repeat(32)));
        assert!(snapshot(&entry, &source, 1000, 1010, &mut Cursor::default()).is_err());
        source.answers[0].pylon = Some(entry.pylon.clone());
        source.answers[0].model = Some("different-model".into());
        assert!(snapshot(&entry, &source, 1000, 1010, &mut Cursor::default()).is_err());
    }

    #[test]
    fn mapping_cannot_attribute_another_workspace_to_this_provider() {
        let (entry, mut source) = fixture();
        source.config.pylon = "22".repeat(32);
        assert!(snapshot(&entry, &source, 1000, 1010, &mut Cursor::default()).is_err());
    }

    #[test]
    fn heartbeat_and_restart_preserve_progress_time_and_increase_sequence() {
        let (entry, source) = fixture();
        let mut cursor = Cursor::default();
        let first = snapshot(&entry, &source, 1000, 1010, &mut cursor).unwrap();
        // The real cursor serialization is restored after a publisher restart.
        let mut cursor = serde_json::from_slice(&serde_json::to_vec(&cursor).unwrap()).unwrap();
        let next = snapshot(&entry, &source, 1000, 1201, &mut cursor).unwrap();
        next.validate(1201).unwrap();
        assert!(next.is_after(&first));
        assert_eq!(next.steps, first.steps);
        assert_eq!(next.current.as_ref().unwrap().at, 1000);
        assert_eq!(
            next.current.unwrap().summary,
            "Waiting for the next update."
        );
        assert_eq!((next.turns, next.calls), (first.turns, first.calls));
    }

    #[test]
    fn newly_observed_actions_do_not_retime_previous_actions() {
        let (entry, mut source) = fixture();
        let mut cursor = Cursor::default();
        let first = snapshot(&entry, &source, 1000, 1010, &mut cursor).unwrap();
        source.agent.steps[0].observations.push(Observation {
            command: "patch apps/haunt-preview/src/main.ts".into(),
            code: 0,
        });
        let next = snapshot(&entry, &source, 1100, 1110, &mut cursor).unwrap();
        next.validate(1110).unwrap();
        assert_eq!(next.steps[0], first.steps[0]);
        assert_eq!(next.steps[1].at, 1100);
        assert_eq!(next.steps[1].kind, WorkStepKind::Write);
        source.agent.steps[0].observations[0].command = "check".into();
        assert!(snapshot(&entry, &source, 1200, 1210, &mut cursor).is_err());
    }

    #[test]
    fn newer_source_save_does_not_retime_the_last_action() {
        let (entry, source) = fixture();
        let mut cursor = Cursor::default();
        let first = snapshot(&entry, &source, 1000, 1010, &mut cursor).unwrap();
        // Starting another model call or adding guidance saves the source, but
        // no new tool action has happened.
        let next = snapshot(&entry, &source, 1200, 1210, &mut cursor).unwrap();
        next.validate(1210).unwrap();
        assert!(next.is_after(&first));
        assert_eq!(next.steps, first.steps);
        assert_eq!(next.current.as_ref().unwrap().at, 1000);
        assert_eq!(
            next.current.unwrap().summary,
            "Waiting for the next update."
        );
    }

    #[test]
    fn new_workspace_session_advances_start_time() {
        let (entry, mut source) = fixture();
        let mut cursor = Cursor::default();
        let first = snapshot(&entry, &source, 1000, 1010, &mut cursor).unwrap();
        source.until += 100;
        let next = snapshot(&entry, &source, 1100, 1110, &mut cursor).unwrap();
        assert!(next.is_after(&first));
        assert_ne!(next.session_id, first.session_id);
    }

    #[test]
    fn checked_digest_applies_to_current_files_only() {
        let (entry, mut source) = fixture();
        source.phase = "review".into();
        source.checked = Some("incorrect".into());
        assert!(snapshot(&entry, &source, 1000, 1010, &mut Cursor::default()).is_err());
        // Computed independently from the Kiln base+sorted-files JSON shape.
        source.checked =
            Some("1cc809020db7d5b055f597a6cd1cb2ac76d0a4c74a520cea6859c724d3dea6db".into());
        let report = snapshot(&entry, &source, 1000, 1010, &mut Cursor::default()).unwrap();
        assert_eq!(report.phase, WorkPhase::Done);
        assert_eq!(report.checks.last().unwrap().status, "passed");
        source.files.insert(
            "apps/haunt-preview/src/main.ts".into(),
            "changed again".into(),
        );
        source.phase = "model".into();
        let report = snapshot(&entry, &source, 1100, 1110, &mut Cursor::default()).unwrap();
        assert_eq!(report.checks.last().unwrap().status, "pending");
    }

    #[test]
    fn bounded_history_omits_sensitive_paths_and_raw_unknown_commands() {
        let (entry, mut source) = fixture();
        source.agent.steps[0].observations = (0..100)
            .map(|_| Observation {
                command: "arbitrary PRIVATE_SHELL_TEXT".into(),
                code: 1,
            })
            .collect();
        source
            .files
            .insert("apps/haunt-preview/.env".into(), "PRIVATE_ENV".into());
        let report = snapshot(&entry, &source, 1000, 1010, &mut Cursor::default()).unwrap();
        report.validate(1010).unwrap();
        assert_eq!(report.steps.len(), work::MAX_STEPS);
        assert_eq!(report.files, vec!["apps/haunt-preview/src/main.ts"]);
        let text = serde_json::to_string(&report).unwrap();
        assert!(!text.contains("PRIVATE_"));
        assert!(!text.contains(".env"));
        assert_eq!(report.steps.last().unwrap().summary, "Needs attention.");
    }

    #[test]
    fn initial_workspace_has_valid_current_action_without_invented_history() {
        let (entry, mut source) = fixture();
        source.agent.steps.clear();
        let report = snapshot(&entry, &source, 1000, 1010, &mut Cursor::default()).unwrap();
        report.validate(1010).unwrap();
        assert!(report.steps.is_empty());
        assert_eq!(report.current.unwrap().label, "Workspace opened");
    }

    #[test]
    fn asking_for_a_check_does_not_report_a_passing_check() {
        let (entry, mut source) = fixture();
        source.phase = "awaiting-check".into();
        source.agent.steps[0].observations.push(Observation {
            command: "check".into(),
            code: 0,
        });
        let report = snapshot(&entry, &source, 1000, 1010, &mut Cursor::default()).unwrap();
        assert_eq!(report.phase, WorkPhase::Checking);
        assert!(report.checks.iter().all(|check| check.status != "passed"));
        assert_eq!(report.current.unwrap().label, "Waiting for checks");
    }

    #[test]
    fn changing_task_labels_requires_a_new_workspace() {
        let (mut entry, source) = fixture();
        let mut cursor = Cursor::default();
        snapshot(&entry, &source, 1000, 1010, &mut cursor).unwrap();
        entry.issue = Some(12505);
        entry.title = "Another task".into();
        assert!(snapshot(&entry, &source, 1000, 1010, &mut cursor).is_err());
        entry.statefile = "/private/example/new-workspace.json".into();
        snapshot(&entry, &source, 1100, 1110, &mut cursor)
            .unwrap()
            .validate(1110)
            .unwrap();
    }
}

fn read(entry: &Entry) -> Result<(Source, u64), String> {
    let file =
        std::fs::File::open(&entry.statefile).map_err(|_| "workspace state is unavailable")?;
    let metadata = file
        .metadata()
        .map_err(|_| "could not inspect workspace state")?;
    if !metadata.is_file() || metadata.len() > MAX_SOURCE_BYTES {
        return Err("workspace state exceeds the observer limit".into());
    }
    let saved = metadata
        .modified()
        .map_err(|_| "workspace save time is unavailable")?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "workspace save time is invalid")?
        .as_secs();
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "could not read workspace state")?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err("workspace state exceeds the observer limit".into());
    }
    let source = serde_json::from_slice(&bytes)
        .map_err(|_| "workspace state is not a supported saved workspace")?;
    Ok((source, saved))
}

fn save_cursors(path: &Path, cursors: &BTreeMap<String, Cursor>) -> Result<(), String> {
    use std::io::Write as _;
    let temporary = path.with_extension(format!("{}.new", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options
            .open(&temporary)
            .map_err(|_| "could not save observer positions")?;
        serde_json::to_writer(&mut file, cursors)
            .map_err(|_| "could not save observer positions")?;
        file.flush()
            .and_then(|()| file.sync_all())
            .map_err(|_| "could not save observer positions")?;
        std::fs::rename(&temporary, path).map_err(|_| "could not save observer positions")?;
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}

fn load_config(path: &Path) -> Result<Config, String> {
    let bytes = std::fs::read(path).map_err(|_| "could not read observer configuration")?;
    if bytes.len() > 16 * 1024 {
        return Err("observer configuration is too large".into());
    }
    let config: Config =
        serde_json::from_slice(&bytes).map_err(|_| "invalid observer configuration")?;
    if config.entries.is_empty()
        || config.entries.len() > 2
        || config
            .entries
            .iter()
            .any(|e| !e.statefile.is_absolute() || producer(&e.pylon).is_none())
        || (config.entries.len() == 2 && config.entries[0].pylon == config.entries[1].pylon)
    {
        return Err("observer needs one or two distinct provider mappings".into());
    }
    Ok(config)
}

/// Publish safe observations every five seconds, including unchanged saves.
///
/// # Errors
/// Returns an error for invalid configuration, an unreadable private key, or
/// invalid observer cursors. Individual missing states and network failures
/// are retried without publishing invented progress.
pub fn serve(options: &Options) -> Result<(), String> {
    let id = crate::host::load_key(&options.state)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(options.state.join("workspace-watch.lock"))
        .map_err(|_| "could not open observer lock")?;
    lock.try_lock()
        .map_err(|_| "another workspace watcher is already running")?;
    load_config(&options.config)?;
    let url = format!("{}{}", options.site.trim_end_matches('/'), work::PATH);
    if !url.starts_with("https://") || reqwest::Url::parse(&url).is_err() {
        return Err("observer site must use HTTPS".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "could not start workspace watcher")?;
    let path = options.state.join(CURSORS);
    let mut cursors: BTreeMap<String, Cursor> = match std::fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|_| "invalid saved observer positions")?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(_) => return Err("could not read saved observer positions".into()),
    };
    let started = std::time::Instant::now();
    loop {
        // A reviewed task handoff can replace this mapping atomically without
        // restarting either the observer or the actual workspace worker.
        let config = load_config(&options.config)?;
        for entry in &config.entries {
            let result = (|| {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| "clock is unavailable")?
                    .as_secs();
                let (source, saved) = read(entry)?;
                let observed = snapshot(
                    entry,
                    &source,
                    saved,
                    now,
                    cursors.entry(entry.pylon.clone()).or_default(),
                )?;
                observed.validate(now).map_err(str::to_owned)?;
                let body = serde_json::to_vec(&observed)
                    .map_err(|_| "could not encode workspace observation")?;
                if body.len() > work::MAX_BODY_BYTES {
                    return Err("workspace observation exceeds the public limit".into());
                }
                // Persist before sending, so retries/restarts never reuse an accepted sequence.
                save_cursors(&path, &cursors)?;
                let authorization = crate::host::sign_command(&id.signer, &url, &body, now);
                let response = client
                    .post(&url)
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(body)
                    .send()
                    .map_err(|_| "workspace observation could not reach the site")?;
                if !response.status().is_success() {
                    return Err("the site refused the workspace observation".into());
                }
                Ok::<_, String>(())
            })();
            if let Err(error) = result {
                eprintln!("workspace-watch: {error}");
            }
        }
        if options
            .run_for
            .is_some_and(|limit| started.elapsed() >= limit)
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}
