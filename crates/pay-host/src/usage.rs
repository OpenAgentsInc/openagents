//! Public call and run events from the existing append-only JSONL journals.

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufRead, BufReader, Read as _},
    path::{Path, PathBuf},
};

use openagents_chat_app::route_map::Map;
use route_contract::{
    lifecycle::Lifecycle,
    record::RouteRecord,
    route::{PluginRoute, RouteResult},
};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use super::{Error, EventType, Resource, SourceRecord, Store, safe_id};

const MAX_LINE: u64 = 1_048_576;

impl Store {
    /// Import a route journal, or all JSONL files in its directory.
    /// Repeated snapshots of a request produce one call and one event per settled run.
    pub fn sync_route_journal(&mut self, path: &Path) -> Result<(), Error> {
        self.sync_routes(path, true)
    }

    /// Import settled runs when another source supplies call events.
    pub fn sync_run_journal(&mut self, path: &Path) -> Result<(), Error> {
        self.sync_routes(path, false)
    }

    fn sync_routes(&mut self, path: &Path, include_calls: bool) -> Result<(), Error> {
        for file in journal_files(path)? {
            read_lines(&file, |scope, _, line| {
                let record: RouteRecord = serde_json::from_slice(line)
                    .map_err(|_| "Invalid complete route journal record")?;
                validate_route(&record)?;
                let request_key = serde_json::to_string(&(
                    &record.thread,
                    &record.request,
                    &record.snapshot.identity.caller.id,
                ))?;
                if include_calls {
                    let mut call = event(
                        format!("route:{scope}:{request_key}:call"),
                        millis(record.received_ms)?,
                        EventType::Call,
                        Resource::Route,
                        "front".into(),
                        record.snapshot.identity.caller.id.clone(),
                    );
                    if let RouteResult::Plugin {
                        plugin: PluginRoute::Run { capability, .. },
                    } = &record.result
                    {
                        let plugin = if safe_id(&capability.id) {
                            capability.id.clone()
                        } else {
                            self.alias("plugin", &capability.id, None)
                        };
                        call.resource = Resource::Plugin;
                        call.node = format!("plugin:{plugin}");
                        call.plugin = Some(plugin);
                    }
                    self.record(call)?;
                }
                if let Some(settled) = record.settled_ms {
                    for run in &record.runs {
                        if !terminal(run.projection.state) {
                            continue;
                        }
                        let run_key = serde_json::to_string(&run.task)?;
                        self.record(event(
                            format!("route:{scope}:{request_key}:run:{run_key}"),
                            millis(settled)?,
                            EventType::Run,
                            Resource::Coder,
                            "coder".into(),
                            record.snapshot.identity.caller.id.clone(),
                        ))?;
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Import the chat worker's usage JSONL file or daily journal directory.
    /// The writer has no job ID, so each append's file and byte offset identify it.
    pub fn sync_usage_journal(&mut self, path: &Path) -> Result<(), Error> {
        let map = Map::committed();
        for file in journal_files(path)? {
            read_lines(&file, |scope, offset, line| {
                let record: UsageRecord = serde_json::from_slice(line)
                    .map_err(|_| "Invalid complete usage journal record")?;
                let at = record.timestamp()?;
                let node = record
                    .route
                    .as_deref()
                    .map(|route| format!("route:{route}"))
                    .filter(|id| map.find(id).is_some())
                    .unwrap_or_else(|| "front".into());
                self.record(event(
                    format!("usage:{scope}:{offset}"),
                    at,
                    EventType::Call,
                    Resource::Route,
                    node,
                    record.key,
                ))?;
                Ok(())
            })?;
        }
        Ok(())
    }
}

fn event(
    source: String,
    at: i64,
    kind: EventType,
    resource: Resource,
    node: String,
    payer: String,
) -> SourceRecord {
    SourceRecord {
        source,
        at,
        kind,
        resource,
        plugin: None,
        node,
        // The journals report dollar costs, which are not Lightning amounts.
        amount_sats: None,
        rail: None,
        split: BTreeMap::new(),
        author_identity: None,
        published_author_npub: None,
        payer_identity: Some(payer),
    }
}

fn terminal(state: Lifecycle) -> bool {
    matches!(
        state,
        Lifecycle::Completed | Lifecycle::Failed | Lifecycle::Cancelled
    )
}

fn millis(at: u64) -> Result<i64, Error> {
    i64::try_from(at).map_err(|_| "Invalid journal timestamp".into())
}

fn validate_route(record: &RouteRecord) -> Result<(), Error> {
    if record.schema != route_contract::RECORD_SCHEMA
        || record.snapshot.schema != route_contract::SNAPSHOT_SCHEMA
        || record.request.is_empty()
        || record.snapshot.identity.caller.id.is_empty()
        || record.result.validate().is_err()
        || record.snapshot.route.result != record.result.digest()
        || record.snapshot.route.family != record.result.family()
        || record.snapshot_digest != record.snapshot.digest()
        || record.runs.iter().any(|run| run.task.is_empty())
        || record
            .settled_ms
            .is_some_and(|at| at < record.received_ms || !terminal(record.state))
    {
        return Err("Invalid complete route journal record".into());
    }
    millis(record.received_ms)?;
    if let Some(at) = record.settled_ms {
        millis(at)?;
    }
    Ok(())
}

// The required fields of coder::relay::usage::Record, plus the route used by
// this projection. Keeping this wire reader here avoids linking the worker.
// Optional model, provider, key-fingerprint, and cost fields are ignored.
#[derive(Deserialize)]
struct UsageRecord {
    time: String,
    key: String,
    #[serde(rename = "kind")]
    _kind: UsageKind,
    route: Option<String>,
    #[serde(rename = "total_ms")]
    _total_ms: u64,
    #[serde(rename = "outcome")]
    _outcome: UsageOutcome,
    #[serde(rename = "bytes_in")]
    _bytes_in: usize,
    #[serde(rename = "bytes_out")]
    _bytes_out: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum UsageKind {
    Turn,
    Rank,
    Probe,
    Delegation,
    Unread,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum UsageOutcome {
    Answered,
    Failed,
    Refused,
    Unfinished,
}

impl UsageRecord {
    fn timestamp(&self) -> Result<i64, Error> {
        if self.key.is_empty()
            || self.key.len() > 64
            || !self.key.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid complete usage journal record".into());
        }
        let at = chrono::DateTime::parse_from_rfc3339(&self.time)
            .map_err(|_| "Invalid usage journal timestamp")?
            .timestamp_millis();
        if at < 0 {
            return Err("Invalid usage journal timestamp".into());
        }
        Ok(at)
    }
}

fn journal_files(path: &Path) -> Result<Vec<PathBuf>, Error> {
    if !path.is_dir() {
        return Ok(vec![path.to_owned()]);
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().is_some_and(|ext| ext == "jsonl")
        {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

fn read_lines(
    path: &Path,
    mut visit: impl FnMut(&str, u64, &[u8]) -> Result<(), Error>,
) -> Result<(), Error> {
    let canonical = path.canonicalize()?;
    let scope = hex::encode(Sha256::digest(canonical.as_os_str().as_encoded_bytes()));
    let mut reader = BufReader::new(File::open(canonical)?);
    let mut offset = 0u64;
    let mut line = Vec::new();
    loop {
        line.clear();
        let count = (&mut reader)
            .take(MAX_LINE + 1)
            .read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        if count as u64 > MAX_LINE {
            return Err("Journal record exceeds the size limit".into());
        }
        // A concurrent append may have written only part of its last record.
        if line.last() != Some(&b'\n') {
            break;
        }
        if !line.iter().all(u8::is_ascii_whitespace) {
            visit(&scope, offset, &line)?;
        }
        offset = offset
            .checked_add(count as u64)
            .ok_or("Journal offset overflow")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Write as _};

    use route_contract::{
        digest::Digest,
        lifecycle::{TaskChecks, TaskDisposition, TaskExecution, TaskStatus},
        record::Observation,
        route::{
            Chosen, DispatchPlan, FanOut, PlannedRun, RouteResult, RunMode, Summary, TaskClass,
        },
        snapshot::{CapabilityPin, Effects},
    };
    use rusqlite::Connection;
    use serde_json::{Value, json};

    use super::*;

    // Serialized result of coder/src/relay/usage.rs's fixture
    // a_turn_is_recorded_from_what_it_published_without_its_text.
    const WORKER_FIXTURE: &str = r#"{"time":"2026-10-01T21:37:15.000Z","key":"ab","kind":"turn","client":"openagents-mobile","surface":"phone","route":"general","tier":"model","model":"example/primary-model","door":"openrouter.ai","jev_door":"https://api.typesafe.ai","jev_model":"jev-1.13","first_token_ms":900,"total_ms":1410,"tokens_in":120,"tokens_out":40,"outcome":"answered","bytes_in":812,"bytes_out":18}"#;

    fn store() -> Store {
        Store::from_connection(Connection::open_in_memory().unwrap(), [42; 32]).unwrap()
    }

    fn append(path: &Path, text: &str) {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{text}").unwrap();
    }

    #[test]
    fn worker_fixture_projects_only_the_public_call() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.jsonl");
        append(&path, WORKER_FIXTURE);
        let mut store = store();
        store.sync_usage_journal(&path).unwrap();
        let events = store.since(0).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].at, 1_790_890_635_000);
        assert_eq!(events[0].kind, EventType::Call);
        assert!(matches!(events[0].resource, Resource::Route));
        assert!(Map::committed().find(&events[0].node).is_some());
        assert!(events[0].payer.as_deref().unwrap().starts_with("caller-"));
        let wire = serde_json::to_value(&events[0]).unwrap();
        assert!(wire.get("amount_sats").is_none());
        assert!(wire.get("split").is_none());
        for private in ["key", "client", "model", "door", "source", "tokens_in"] {
            assert!(wire.get(private).is_none());
        }
    }

    #[test]
    fn usage_replay_preserves_identical_jobs_and_waits_for_the_last_newline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("2026-10-01.jsonl");
        append(&path, WORKER_FIXTURE);
        append(&path, WORKER_FIXTURE);
        let mut store = store();
        store.sync_usage_journal(dir.path()).unwrap();
        store.sync_usage_journal(&path).unwrap();
        assert_eq!(store.since(0).unwrap().len(), 2);
        let cut = WORKER_FIXTURE.len() / 2;
        let mut writer = OpenOptions::new().append(true).open(&path).unwrap();
        writer.write_all(WORKER_FIXTURE[..cut].as_bytes()).unwrap();
        store.sync_usage_journal(&path).unwrap();
        assert_eq!(store.since(0).unwrap().len(), 2);
        writer.write_all(WORKER_FIXTURE[cut..].as_bytes()).unwrap();
        writer.write_all(b"\n").unwrap();
        store.sync_usage_journal(&path).unwrap();
        assert_eq!(store.since(0).unwrap().len(), 3);
        append(&dir.path().join("2026-10-02.jsonl"), WORKER_FIXTURE);
        fs::write(dir.path().join("ignore.txt"), "invalid").unwrap();
        store.sync_usage_journal(dir.path()).unwrap();
        assert_eq!(store.since(0).unwrap().len(), 4);
    }

    #[test]
    fn private_route_words_and_dollar_costs_are_never_published() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.jsonl");
        let mut fixture: Value = serde_json::from_str(WORKER_FIXTURE).unwrap();
        fixture["route"] = json!("private-route-secret");
        fixture["cost_usd"] = json!(0.0123);
        fixture["payer_fingerprint"] = json!("private-fingerprint-secret");
        append(&path, &fixture.to_string());
        let mut store = store();
        store.sync_usage_journal(&path).unwrap();
        let events = store.since(0).unwrap();
        assert_eq!(events[0].node, "front");
        let wire = serde_json::to_string(&events).unwrap();
        assert!(!wire.contains("secret"));
        assert!(!wire.contains("cost"));
        assert!(!wire.contains("amount_sats"));
    }

    #[test]
    fn complete_invalid_usage_records_fail_without_echoing_their_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.jsonl");
        let fixture: Value = serde_json::from_str(WORKER_FIXTURE).unwrap();
        let mut bad_kind = fixture.clone();
        bad_kind["kind"] = json!("private-secret");
        let mut bad_outcome = fixture.clone();
        bad_outcome["outcome"] = json!("private-secret");
        let mut bad_time = fixture.clone();
        bad_time["time"] = json!("private-secret");
        let mut missing_size = fixture;
        missing_size.as_object_mut().unwrap().remove("bytes_in");
        for text in [
            "{private-secret}".into(),
            bad_kind.to_string(),
            bad_outcome.to_string(),
            bad_time.to_string(),
            missing_size.to_string(),
        ] {
            fs::write(&path, format!("{text}\n")).unwrap();
            let mut store = store();
            let error = store.sync_usage_journal(&path).unwrap_err().to_string();
            assert!(!error.contains("private-secret"));
            assert!(store.since(0).unwrap().is_empty());
        }
    }

    fn route() -> RouteRecord {
        let digest = Digest::of_bytes(b"private-input");
        let result = RouteResult::Coder {
            plan: DispatchPlan {
                class: TaskClass::Exploration,
                fan_out: FanOut::Single,
                runs: vec![PlannedRun {
                    engine: "codex".into(),
                    chosen: Chosen::Default,
                    mode: RunMode::ReadOnly,
                    input: digest.clone(),
                    continuation: None,
                }],
                summary: Summary::None,
            },
        };
        let snapshot = serde_json::from_value(json!({
            "schema": route_contract::SNAPSHOT_SCHEMA,
            "identity": {
                "caller": {"kind":"app_user", "id":"private-caller"},
                "surface":"terminal", "request":"private-request", "thread":"private-thread"
            },
            "input": {"request":digest},
            "route": {
                "family":"coder", "result":result.digest(), "policy":"fixture",
                "question_set":{"id":"fixture", "digest":digest}
            },
            "placement":{"workspace":{"project":"private-project", "path":"/private/repo"}},
            "effects":Effects::none(),
            "disclosure":{"recipients":[], "context":[]},
            "resources":{"capacity":{"kind":"available"}},
            "money":{"byok":"ours", "payers":[], "funding":"none", "shown":false},
            "evidence":{"deliverables":[], "check":"none"}
        }))
        .unwrap();
        RouteRecord::received(
            "private-request",
            Some("private-thread".into()),
            result,
            snapshot,
            1_000,
        )
        .unwrap()
    }

    fn observation(status: TaskStatus, execution: TaskExecution) -> Observation {
        Observation {
            disposition: TaskDisposition {
                status,
                execution,
                checks: TaskChecks::NotRun,
            },
            revision: 1,
            cost_microusd: Some(123_456),
            wall_ms: Some(1_000),
            artifacts: vec![Digest::of_bytes(b"private-artifact")],
            payer: Some("theirs".into()),
            payer_keys: Vec::new(),
        }
    }

    #[test]
    fn route_snapshots_emit_one_call_and_one_settled_run() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("route.jsonl");
        let mut record = route();
        append(&path, &serde_json::to_string(&record).unwrap());
        record.step(Lifecycle::Admitted, "fixture", 1_100).unwrap();
        record.dispatched("private-task", Some("codex")).unwrap();
        record.observe(
            "private-task",
            observation(TaskStatus::Unknown, TaskExecution::Unknown),
            2_000,
        );
        append(&path, &serde_json::to_string(&record).unwrap());
        let mut store = store();
        store.sync_route_journal(&path).unwrap();
        assert_eq!(store.since(0).unwrap().len(), 1);
        record.observe(
            "private-task",
            observation(TaskStatus::Finished, TaskExecution::Finished),
            3_000,
        );
        append(&path, &serde_json::to_string(&record).unwrap());
        append(&path, &serde_json::to_string(&record).unwrap());
        store.sync_route_journal(dir.path()).unwrap();
        store.sync_route_journal(&path).unwrap();
        let events = store.since(0).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventType::Call);
        assert_eq!(events[0].at, 1_000);
        assert_eq!(events[0].node, "front");
        assert_eq!(events[1].kind, EventType::Run);
        assert_eq!(events[1].at, 3_000);
        assert_eq!(events[1].node, "coder");
        let wire = serde_json::to_string(&events).unwrap();
        for private in ["private-", "/private/", "cost_microusd", "amount_sats"] {
            assert!(!wire.contains(private));
        }
        for event in &events {
            let value = serde_json::to_value(event).unwrap();
            assert!(value.get("source").is_none());
        }
    }

    #[test]
    fn plugin_calls_use_the_capability_id_and_alias_private_package_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("route.jsonl");
        for id in ["weather".to_owned(), "ab".repeat(32)] {
            let mut record = route();
            let capability = CapabilityPin {
                id: id.clone(),
                version: "1.0.0".into(),
                digest: Digest::of_bytes(b"private-package"),
            };
            record.result = RouteResult::Plugin {
                plugin: PluginRoute::Run {
                    capability: capability.clone(),
                    arguments: json!({"private-secret":"never publish arguments"}),
                },
            };
            record.snapshot.route.family = record.result.family();
            record.snapshot.route.result = record.result.digest();
            record.snapshot.route.capability = Some(capability);
            record.snapshot_digest = record.snapshot.digest();
            record.step(Lifecycle::Admitted, "fixture", 1_100).unwrap();
            fs::write(&path, "").unwrap();
            append(&path, &serde_json::to_string(&record).unwrap());
            append(&path, &serde_json::to_string(&record).unwrap());
            let mut store = store();
            let expected = if safe_id(&id) {
                id.clone()
            } else {
                store.alias("plugin", &id, None)
            };
            store.sync_route_journal(&path).unwrap();
            store.sync_route_journal(&path).unwrap();
            let events = store.since(0).unwrap();
            assert_eq!(events.len(), 1);
            assert!(matches!(events[0].resource, Resource::Plugin));
            assert_eq!(events[0].plugin.as_deref(), Some(expected.as_str()));
            assert_eq!(events[0].node, format!("plugin:{expected}"));
            assert_eq!(store.stats(2_000).unwrap().per_plugin[&expected].calls, 1);
            let wire = serde_json::to_string(&events).unwrap();
            assert!(!wire.contains("private-secret"));
            assert!(!wire.contains("arguments"));
            if !safe_id(&id) {
                assert!(!wire.contains(&id));
                assert!(expected.starts_with("plugin-"));
            }
        }
    }

    #[test]
    fn plugin_creation_does_not_guess_a_package_from_its_brief() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("route.jsonl");
        let mut record = route();
        record.result = RouteResult::Plugin {
            plugin: PluginRoute::Create {
                brief: "private-secret plugin weather".into(),
            },
        };
        record.snapshot.route.family = record.result.family();
        record.snapshot.route.result = record.result.digest();
        record.snapshot_digest = record.snapshot.digest();
        append(&path, &serde_json::to_string(&record).unwrap());
        let mut store = store();
        store.sync_route_journal(&path).unwrap();
        let events = store.since(0).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].resource, Resource::Route));
        assert!(events[0].plugin.is_none());
        assert_eq!(events[0].node, "front");
        assert!(store.stats(2_000).unwrap().per_plugin.is_empty());
    }

    #[test]
    fn run_journal_leaves_calls_to_the_worker_and_deduplicates_runs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("route.jsonl");
        let mut record = route();
        append(&path, &serde_json::to_string(&record).unwrap());
        record.step(Lifecycle::Admitted, "fixture", 1_100).unwrap();
        record.dispatched("private-task", Some("codex")).unwrap();
        append(&path, &serde_json::to_string(&record).unwrap());
        let mut store = store();
        store.sync_run_journal(&path).unwrap();
        assert!(store.since(0).unwrap().is_empty());
        record.observe(
            "private-task",
            observation(TaskStatus::Finished, TaskExecution::Finished),
            3_000,
        );
        append(&path, &serde_json::to_string(&record).unwrap());
        append(&path, &serde_json::to_string(&record).unwrap());
        store.sync_run_journal(&path).unwrap();
        store.sync_run_journal(dir.path()).unwrap();
        let events = store.since(0).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventType::Run);
        assert_eq!(events[0].at, 3_000);
        assert_eq!(events[0].node, "coder");
        // Both adapters share the same run identity if configuration changes.
        store.sync_route_journal(&path).unwrap();
        let events = store.since(0).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].kind, EventType::Call);
    }

    #[test]
    fn route_records_require_the_bound_snapshot_and_supported_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("route.jsonl");
        let mut bad_digest = route();
        bad_digest.snapshot.identity.caller.id = "private-replacement".into();
        let mut bad_schema = route();
        bad_schema.schema = "unsupported".into();
        for record in [bad_digest, bad_schema] {
            fs::write(
                &path,
                format!("{}\n", serde_json::to_string(&record).unwrap()),
            )
            .unwrap();
            let mut store = store();
            assert!(store.sync_route_journal(&path).is_err());
            assert!(store.since(0).unwrap().is_empty());
        }
    }
}
