//! Read-only landing, pool, and capacity facts shared by app tools and the CLI.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::pool::{self, Pool};

pub const DEFAULT_QUEUE: &str = "gs://openagentsgemini-coder-artifacts/land-queue/openagents";
pub const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Run an allowlisted read through the process supervisor, including cleanup.
pub async fn read_command(program: &str, args: &[&str]) -> Result<String, String> {
    read_optional(program, args)
        .await
        .map(Option::unwrap_or_default)
}

fn allowed_read(program: &str, args: &[&str]) -> bool {
    match (program, args) {
        ("pgrep", ["-x", "cargo"]) => true,
        ("gcloud", ["storage", "ls" | "cat", url, "--quiet"]) => url.starts_with("gs://"),
        (
            "gcloud",
            [
                "compute",
                "instances",
                "list",
                filter,
                "--format=json",
                "--project",
                project,
                "--quiet",
            ],
        ) => filter.starts_with("--filter=labels.openagents-pool=") && !project.starts_with('-'),
        _ => false,
    }
}

#[cfg(not(unix))]
async fn read_optional(_: &str, _: &[&str]) -> Result<Option<String>, String> {
    Err("machine status reads require Unix".into())
}

#[cfg(unix)]
async fn read_optional(program: &str, args: &[&str]) -> Result<Option<String>, String> {
    if !allowed_read(program, args) {
        return Err("this status reader only runs approved reads".into());
    }
    let command = if program == "gcloud" {
        pool::gcloud_command()
    } else {
        std::process::Command::new(program)
    };
    let ended = supervise::Job::from_command(command)
        .args(args.iter().copied())
        .bounded(supervise::Limits::within(READ_TIMEOUT).keeping(2 * 1024 * 1024))
        .run()
        .await;
    if ended.stdout.truncated || ended.stderr.truncated {
        return Err(format!("{program} returned too much data"));
    }
    match ended.ending {
        supervise::Ending::Exited(Some(0)) => Ok(Some(ended.stdout.text)),
        supervise::Ending::Exited(Some(1)) if program == "pgrep" => Ok(Some(String::new())),
        supervise::Ending::Exited(Some(1))
            if program == "gcloud"
                && args.first() == Some(&"storage")
                && ["matched no objects", "not found", "404"]
                    .iter()
                    .any(|text| ended.stderr.text.contains(text)) =>
        {
            Ok(None)
        }
        supervise::Ending::TimedOut => Err(format!("{program} timed out after 10 seconds")),
        _ => Err(format!("{program} couldn't read status")),
    }
}

async fn object(root: &str, name: &str) -> Result<Option<Value>, String> {
    if root.starts_with("gs://") {
        let url = format!("{}/{name}", root.trim_end_matches('/'));
        let Some(text) = read_optional("gcloud", &["storage", "cat", &url, "--quiet"]).await?
        else {
            return Ok(None);
        };
        return serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| e.to_string());
    }
    match std::fs::read(Path::new(root.strip_prefix("file://").unwrap_or(root)).join(name)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// The CLI's queue objects, read without submitting, withdrawing, or waking work.
pub async fn landing_status(root: &str, all: bool, now: u64) -> Result<Value, String> {
    let (entries, worker) = tokio::try_join!(landing_entries(root), object(root, "worker.json"))?;
    if worker.as_ref().is_some_and(|w| w["at"].as_u64().is_none()) {
        return Err("landing worker status is unreadable".into());
    }
    Ok(landing_facts(root, entries, worker, all, now))
}

async fn landing_entries(root: &str) -> Result<Vec<Value>, String> {
    let entries = if root.starts_with("gs://") {
        // Read all entry objects in one request, rather than starting gcloud
        // once for every historical landing.
        let url = format!("{}/entries/*.json", root.trim_end_matches('/'));
        let text = read_command("gcloud", &["storage", "cat", &url, "--quiet"]).await?;
        decode_entries(&text)?
    } else {
        let path = Path::new(root.strip_prefix("file://").unwrap_or(root)).join("entries");
        let files = match std::fs::read_dir(path) {
            Ok(files) => files,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.to_string()),
        };
        let mut entries = Vec::new();
        for file in files {
            let file = file.map_err(|e| e.to_string())?;
            if file.path().is_file() && file.path().extension().is_some_and(|e| e == "json") {
                let bytes = std::fs::read(file.path()).map_err(|e| e.to_string())?;
                entries.push(serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string())?);
            }
        }
        entries
    };
    for entry in &entries {
        if entry["id"].as_str().is_none_or(str::is_empty)
            || !matches!(
                entry["state"].as_str(),
                Some("queued" | "landing" | "landed" | "bounced" | "withdrawn")
            )
        {
            return Err("a landing entry is unreadable".into());
        }
    }
    Ok(entries)
}

fn decode_entries(text: &str) -> Result<Vec<Value>, String> {
    serde_json::Deserializer::from_str(text)
        .into_iter::<Value>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn landing_facts(
    root: &str,
    mut entries: Vec<Value>,
    worker: Option<Value>,
    all: bool,
    now: u64,
) -> Value {
    entries.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let open = |e: &Value| matches!(e["state"].as_str(), Some("queued" | "landing"));
    let count = entries.iter().filter(|e| open(e)).count();
    let failed: Vec<Value> = entries
        .iter()
        .rev()
        .filter(|e| e["state"] == "bounced")
        .take(20)
        .cloned()
        .collect();
    let worker_alive = worker
        .as_ref()
        .and_then(|w| w["at"].as_u64())
        .is_some_and(|at| now.saturating_sub(at) < 300);
    if !all {
        entries.retain(open);
    }
    json!({"queue": root, "now": now, "worker": worker, "worker_alive": worker_alive,
        "open": count, "entries": entries, "recent_failures": failed})
}

/// The same pool record and host list used by `openagents cloud status`.
pub async fn cloud_status(pool: Option<Pool>, agent: &str) -> Result<Value, String> {
    let hosts = if let Some(pool) = &pool {
        let filter = format!("--filter=labels.openagents-pool={}", pool.pool);
        let text = read_command(
            "gcloud",
            &[
                "compute",
                "instances",
                "list",
                &filter,
                "--format=json",
                "--project",
                &pool.project,
                "--quiet",
            ],
        )
        .await?;
        let listing: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if !listing.is_array() {
            return Err("GCE returned an invalid host list".into());
        }
        pool::hosts_from(&listing)
    } else {
        Vec::new()
    };
    Ok(cloud_facts(
        pool,
        hosts,
        crate::budget::Budget::load(),
        crate::budget::Ledger::load(),
        pool::now(),
        agent,
    ))
}

/// Update only the in-memory ledger copy. A status read never writes spend files.
pub fn cloud_facts(
    pool: Option<Pool>,
    hosts: Vec<pool::Host>,
    budget: crate::budget::Budget,
    mut ledger: crate::budget::Ledger,
    now: u64,
    agent: &str,
) -> Value {
    if let Some(pool) = &pool {
        ledger.observe(&pool.pool, &hosts, now);
    }
    let hourly: f64 = hosts
        .iter()
        .filter(|h| h.status == "RUNNING")
        .map(|h| pool::hourly_usd(&h.machine, h.spot))
        .sum();
    json!({"pool": pool, "hosts": hosts, "estimated_hourly_usd": hourly,
        "spent_usd": ledger.spent(now, None), "agent_spent_usd": ledger.spent(now, Some(agent)),
        "agent_grant": budget.agents.get(agent), "budget": budget,
        "spend_window_seconds": crate::budget::WINDOW_SECONDS, "spend_is_estimate": true})
}

pub fn capacity_at(store: &Path, now: u64, identify: microcoder_loop::account::Identify) -> Value {
    use microcoder_loop::capacity::{Book, Provider};
    let book = Book::load_with(store, identify);
    let rows: Vec<Value> = Provider::ALL
        .into_iter()
        .map(|provider| match book.blocking(provider, now) {
            Some(r) => json!({"provider": provider.as_str(), "capacity": false, "kind": r.kind,
                "observed_at": r.observed_at, "resets_at": r.resets_at, "until": r.until}),
            None => json!({"provider": provider.as_str(), "capacity": true}),
        })
        .collect();
    json!(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn command_reader_refuses_actions_before_starting_a_process() {
        assert!(allowed_read("pgrep", &["-x", "cargo"]));
        assert!(allowed_read(
            "gcloud",
            &["storage", "cat", "gs://fixture/worker.json", "--quiet"]
        ));
        for (program, args) in [
            ("sh", vec!["-c", "exit 0"]),
            ("gcloud", vec!["compute", "instances", "delete", "fixture"]),
            ("gcloud", vec!["storage", "rm", "gs://fixture", "--quiet"]),
        ] {
            assert!(read_command(program, &args).await.is_err());
        }
    }

    #[test]
    fn bulk_queue_read_parses_separate_json_objects_and_rejects_truncation() {
        let rows = decode_entries(concat!(r#"{"id":"a"}"#, "\n", r#"{"id":"b"}"#)).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(decode_entries(concat!(r#"{"id":"a"}"#, "\n{")).is_err());
        assert!(decode_entries("").unwrap().is_empty());
    }

    #[tokio::test]
    async fn landing_reads_fixture_objects_without_changing_them() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("entries")).unwrap();
        let queued = json!({"id":"b", "state":"queued", "issue":12039});
        let landed = json!({"id":"a", "state":"landed"});
        let bounced = json!({"id":"c", "state":"bounced", "issue":12040});
        for entry in [&queued, &landed, &bounced] {
            std::fs::write(
                dir.path()
                    .join(format!("entries/{}.json", entry["id"].as_str().unwrap())),
                entry.to_string(),
            )
            .unwrap();
        }
        std::fs::write(dir.path().join("worker.json"), r#"{"at":100}"#).unwrap();
        let facts = landing_status(dir.path().to_str().unwrap(), false, 101)
            .await
            .unwrap();
        assert_eq!(facts["open"], 1);
        assert_eq!(facts["entries"], json!([queued]));
        assert_eq!(facts["worker_alive"], true);
        assert_eq!(facts["recent_failures"], json!([bounced]));
        let all = landing_status(dir.path().to_str().unwrap(), true, 400)
            .await
            .unwrap();
        assert_eq!(all["entries"][0], landed);
        assert_eq!(all["worker_alive"], false);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("worker.json")).unwrap(),
            r#"{"at":100}"#
        );
    }

    #[test]
    fn no_pool_is_not_a_failed_probe() {
        let status = cloud_facts(
            None,
            Vec::new(),
            crate::budget::Budget::default(),
            crate::budget::Ledger::default(),
            100,
            "fixture",
        );
        assert_eq!(status["hosts"], json!([]));
        assert_eq!(status["spent_usd"], 0.0);
    }

    #[test]
    fn pool_hosts_and_spend_use_the_shared_budget_without_saving() {
        let pool: Pool = serde_json::from_value(json!({
            "schema":"openagents.cloud-pool.v1", "computer":"gce", "pool":"fixture",
            "project":"fixture", "grant":"test", "epoch":1, "granted_at":1,
            "machine":"c3-standard-8", "spot":true, "max_hosts":8,
            "idle_minutes":10, "slots_per_host":2
        }))
        .unwrap();
        let host = pool::Host {
            name: "fixture-host".into(),
            zone: "us-central1-a".into(),
            status: "RUNNING".into(),
            machine: "c3-standard-8".into(),
            spot: true,
            created: "2026-10-18T02:00:00Z".into(),
            address: None,
        };
        let now = crate::budget::rfc3339_seconds("2026-10-18T03:00:00Z").unwrap();
        let budget = crate::budget::Budget::default();
        let cap = budget.daily_usd;
        let result = cloud_facts(
            Some(pool),
            vec![host],
            budget,
            crate::budget::Ledger::default(),
            now,
            "agent-a",
        );
        assert_eq!(result["hosts"][0]["name"], "fixture-host");
        assert_eq!(result["budget"]["daily_usd"], cap);
        assert!(result["spent_usd"].as_f64().unwrap() > 0.0);
        assert_eq!(result["spent_usd"], result["estimated_hourly_usd"]);
        assert_eq!(result["agent_spent_usd"], 0.0);
        assert!(result["agent_grant"].is_null());
    }

    #[test]
    fn capacity_keeps_a_recorded_limit_until_its_reset() {
        use microcoder_loop::capacity::{Book, FILE, Kind, Provider, Refusal, SCHEMA};
        let dir = tempfile::tempdir().unwrap();
        let book = Book {
            schema: SCHEMA.into(),
            refusals: vec![Refusal::new(
                Provider::Claude,
                Kind::UsageLimit,
                100,
                Some(200),
            )],
        };
        std::fs::write(dir.path().join(FILE), serde_json::to_vec(&book).unwrap()).unwrap();
        let rows = capacity_at(dir.path(), 150, |_| None);
        let claude = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["provider"] == "claude")
            .unwrap();
        assert_eq!(claude["capacity"], false);
        assert_eq!(claude["until"], 200);
        let rows = capacity_at(dir.path(), 201, |_| None);
        assert!(
            rows.as_array()
                .unwrap()
                .iter()
                .all(|r| r["capacity"] == true)
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn capacity_uses_the_shared_book_on_a_fixture() {
        let dir = tempfile::tempdir().unwrap();
        let rows = capacity_at(dir.path(), 100, |_| None);
        assert_eq!(rows.as_array().unwrap().len(), 6);
        assert!(
            rows.as_array()
                .unwrap()
                .iter()
                .all(|r| r["capacity"] == true)
        );
    }
}
