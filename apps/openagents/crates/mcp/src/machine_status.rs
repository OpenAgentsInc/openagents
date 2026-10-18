//! Machine and queue facts for any agent (#12039). No actions run here.

use coder_cloud::machine_status as remote;
pub use coder_cloud::owner_status::{
    DeployTarget, NewEntries, Report as OwnerStatus, Sources as OwnerSources,
};
pub use coder_lease::machine_status::{Alerts, MachineFacts, ResourceAlert};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::tools::ToolDef;

pub const NAME: &str = "machine_status";
pub const REFRESH_EVERY: Duration = Duration::from_secs(5 * 60);

pub fn tool() -> ToolDef {
    ToolDef {
        name: NAME,
        description: "Read this computer's free disk against the 100 GiB floor, Cargo jobs against the build cap, held and waiting build slots, the last build sweep and recent evictions, provider capacity, this agent's machine grants and recent actions, the landing queue, GCE pool hosts and spend, read-only service health, and what is waiting on the owner with source and age, including a timed deploy permission test. Call at the start of each check-in; use these facts instead of writing shell probes. This tool only reads. Missing readings include an error; they never mean zero.",
        input_schema: json!({"type":"object", "properties": {
            "agent_id": {"type":"string", "minLength":1,
                "description":"The agent definition ID whose machine grants and actions to read. Defaults to the calling chat's ID."},
            "repository": {"type":"string", "description":"Checkout whose NEEDS_OWNER.md and After-close checks to read. Defaults to OPENAGENTS_FACTS_REPOSITORY or the current checkout."},
            "after_close": {"type":"string", "description":"After-close document path, relative to repository."},
            "deploy_project": {"type":"string", "description":"GCP project containing the runtime service account."},
            "deploy_service_account": {"type":"string", "description":"Runtime service account whose iam.serviceAccounts.actAs permission to test. No deploy runs."}
        }, "additionalProperties":false}),
    }
}

#[derive(serde::Deserialize)]
struct Args {
    agent_id: Option<String>,
    #[serde(flatten)]
    owner: OwnerOptions,
}

/// Repository and deployment inputs are available to every custom agent.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct OwnerOptions {
    pub repository: Option<PathBuf>,
    pub after_close: Option<PathBuf>,
    pub deploy_project: Option<String>,
    pub deploy_service_account: Option<String>,
}

/// The complete block that check-ins and the tool share.
#[derive(Debug, serde::Serialize)]
pub struct Status {
    #[serde(flatten)]
    pub machine: MachineFacts,
    pub capacity: Value,
    pub landing_queue: Value,
    pub cloud: Value,
    pub health: Vec<Value>,
    pub owner_wait: OwnerStatus,
}

impl Status {
    /// Append this block to a check-in prompt without another collection pass.
    pub fn prompt_block(&self) -> String {
        format!(
            "Machine and queue facts:\n{}",
            serde_json::to_string(self).expect("serializable facts")
        )
    }
}

#[derive(Clone, Copy)]
pub struct HealthProbe {
    pub name: &'static str,
    pub method: &'static str,
    pub url: &'static str,
    pub timeout: Duration,
}

/// Only public liveness reads; no visitor, login, chat, or token endpoints.
pub const HEALTH_PROBES: &[HealthProbe] = &[
    HealthProbe {
        name: "website",
        method: "GET",
        url: "https://openagents.com/health",
        timeout: Duration::from_secs(5),
    },
    HealthProbe {
        name: "sync",
        method: "GET",
        url: "https://sync.openagents.com/health",
        timeout: Duration::from_secs(5),
    },
];

async fn health() -> Vec<Value> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build();
    let mut out = Vec::new();
    for probe in HEALTH_PROBES {
        let result = match &client {
            Ok(client) => client.get(probe.url).timeout(probe.timeout).send().await,
            Err(_) => {
                out.push(json!({"name":probe.name, "error":"couldn't start the health reader"}));
                continue;
            }
        };
        out.push(match result {
            Ok(r) => json!({"name":probe.name, "url":probe.url, "method":probe.method, "status":r.status().as_u16(), "healthy":r.status().is_success()}),
            Err(_) => json!({"name":probe.name, "url":probe.url, "method":probe.method, "error":"health read failed or timed out"}),
        });
    }
    out
}

async fn bounded<T>(
    future: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::time::timeout(Duration::from_secs(20), future)
        .await
        .map_err(|_| "status read timed out after 20 seconds".to_owned())?
}

fn reading(value: Result<Value, String>) -> Value {
    value.unwrap_or_else(|error| json!({"error":error}))
}

/// Collect once per five-minute host tick, keyed by agent ID, then reuse in
/// every check-in. Feed `machine` to `coder_lease::machine_status::Alerts`.
pub async fn collect(agent_id: &str) -> Result<Status, String> {
    collect_with(agent_id, OwnerOptions::default()).await
}

/// The same check-in facts for a caller-selected repository and deployment.
pub async fn collect_with(agent_id: &str, owner: OwnerOptions) -> Result<Status, String> {
    let (root, layout) = coder_lease::machine::here()?;
    let home = layout
        .work
        .parent()
        .ok_or("no home folder is known")?
        .to_path_buf();
    let now = coder_lease::now_ms();
    let queue =
        std::env::var("OPENAGENTS_LAND_QUEUE").unwrap_or_else(|_| remote::DEFAULT_QUEUE.into());
    let (cargo, landing_queue, cloud, health, owner_wait) = tokio::join!(
        remote::read_command("pgrep", &["-x", "cargo"]),
        bounded(remote::landing_status(&queue, false, now / 1000)),
        bounded(remote::cloud_status(
            coder_cloud::pool::Pool::load(),
            agent_id
        )),
        health(),
        owner_facts(&home, owner, now / 1000),
    );
    let free = coder_lease::free_disk(&home).map_err(|e| e.to_string());
    let computer =
        coder_lease::machine_status::computer_name().unwrap_or_else(|_| "this computer".into());
    let machine =
        coder_lease::machine_status::collect(&home, &root, agent_id, &computer, now, free, cargo);
    let store = std::env::var_os("OPENAGENTS_TASKS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| home.join(".openagents/tasks"));
    let capacity = remote::capacity_at(&store, now / 1000, microcoder_loop::account::identify);
    Ok(Status {
        machine,
        capacity,
        landing_queue: reading(landing_queue),
        cloud: reading(cloud),
        health,
        owner_wait,
    })
}

fn checkout(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|path| path.join(".git").exists())
        .map(Path::to_path_buf)
}

async fn owner_facts(home: &Path, options: OwnerOptions, now: u64) -> OwnerStatus {
    let explicit = options
        .repository
        .clone()
        .or_else(|| std::env::var_os("OPENAGENTS_FACTS_REPOSITORY").map(PathBuf::from));
    let repository = match explicit {
        Some(path) if path.is_dir() => Some(checkout(&path).unwrap_or(path)),
        Some(_) => None,
        None => std::env::current_dir()
            .ok()
            .and_then(|path| checkout(&path)),
    };
    let Some(repository) = repository else {
        return OwnerStatus {
            errors: vec!["owner list: give an available checkout with repository".into()],
            ..OwnerStatus::default()
        };
    };
    let config = std::env::var_os("CLOUDSDK_CONFIG")
        .map(PathBuf::from)
        .or_else(|| {
            coder_cloud::pool::Pool::load()
                .and_then(|pool| pool.gcloud_config)
                .map(PathBuf::from)
        })
        .or_else(|| {
            let path = home.join("work/.secrets/gcloud-sa-config");
            path.is_dir().then_some(path)
        });
    let target = DeployTarget {
        project: options
            .deploy_project
            .or_else(|| std::env::var("OPENAGENTS_DEPLOY_PROJECT").ok())
            .unwrap_or_else(|| "openagentsgemini".into()),
        service_account: options
            .deploy_service_account
            .or_else(|| std::env::var("OPENAGENTS_DEPLOY_SERVICE_ACCOUNT").ok())
            .unwrap_or_else(|| "157437760789-compute@developer.gserviceaccount.com".into()),
        gcloud_config: config,
    };
    let sources = OwnerSources {
        after_close: options
            .after_close
            .unwrap_or_else(|| OwnerSources::default().after_close),
        ..OwnerSources::default()
    };
    let permission = coder_cloud::owner_status::probe(&target).await;
    coder_cloud::owner_status::collect(&repository, &sources, now, permission).await
}

pub async fn call(chat_id: Option<&str>, args: Value) -> Result<Value, String> {
    call_using(chat_id, args, |agent, owner| async move {
        collect_with(&agent, owner).await
    })
    .await
}

async fn call_using<F, Fut>(chat_id: Option<&str>, args: Value, collect: F) -> Result<Value, String>
where
    F: FnOnce(String, OwnerOptions) -> Fut,
    Fut: std::future::Future<Output = Result<Status, String>>,
{
    if let Some(object) = args.as_object() {
        if object.keys().any(|key| {
            ![
                "agent_id",
                "repository",
                "after_close",
                "deploy_project",
                "deploy_service_account",
            ]
            .contains(&key.as_str())
        }) {
            return Err("invalid arguments: unknown field".into());
        }
    }
    let args: Args = serde_json::from_value(if args.is_null() { json!({}) } else { args })
        .map_err(|e| format!("invalid arguments: {e}"))?;
    let agent = args
        .agent_id
        .as_deref()
        .or(chat_id)
        .ok_or("give agent_id when calling outside a chat")?;
    if agent.trim().is_empty() {
        return Err("agent_id must not be empty".into());
    }
    serde_json::to_value(collect(agent.to_owned(), args.owner).await?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_health_probes_are_timed_gets() {
        assert!(!HEALTH_PROBES.is_empty());
        for p in HEALTH_PROBES {
            assert_eq!(p.method, "GET");
            assert!(p.timeout > Duration::ZERO && p.timeout <= Duration::from_secs(5));
            assert!(p.url.ends_with("/health"));
        }
    }

    #[tokio::test]
    async fn fixture_facts_are_the_same_block_in_the_prompt_and_tool_result() {
        let dir = tempfile::tempdir().unwrap();
        let machine = coder_lease::machine_status::collect(
            dir.path(),
            &dir.path().join("leases"),
            "agent-a",
            "fixture",
            1000,
            Ok(99 << 30),
            Ok("42\n43\n".into()),
        );
        let mut owner_wait = OwnerStatus {
            entries: coder_cloud::owner_status::document(
                "## Approve the store release",
                "NEEDS_OWNER.md",
                false,
                &std::collections::BTreeMap::from([(1, 100)]),
                500,
            ),
            ..OwnerStatus::default()
        };
        owner_wait
            .entries
            .extend(coder_cloud::owner_status::document(
                "## After-close checks\n| #9 | Owner: check iPhone | To do |",
                "checks.md",
                true,
                &std::collections::BTreeMap::from([(2, 200)]),
                500,
            ));
        let status = Status {
            machine,
            capacity: json!([]),
            landing_queue: json!({"open":2}),
            cloud: json!({"hosts":[]}),
            health: vec![json!({"status":200})],
            owner_wait,
        };
        let block = status.prompt_block();
        let result: Value =
            serde_json::from_str(block.strip_prefix("Machine and queue facts:\n").unwrap())
                .unwrap();
        let tool_result = call_using(
            Some("chat-a"),
            json!({"agent_id":"agent-a"}),
            |agent, _| async move {
                assert_eq!(agent, "agent-a");
                Ok(status)
            },
        )
        .await
        .unwrap();
        assert_eq!(result, tool_result);
        assert_eq!(result["disk_below_floor"], true);
        assert_eq!(result["cargo_jobs"], 2);
        assert_eq!(result["landing_queue"]["open"], 2);
        assert_eq!(
            result["owner_wait"]["entries"][0]["what"],
            "Approve the store release"
        );
        assert_eq!(
            result["owner_wait"]["entries"][0]["source"],
            "NEEDS_OWNER.md"
        );
        assert_eq!(result["owner_wait"]["entries"][0]["age_seconds"], 400);
        assert_eq!(result["owner_wait"]["entries"][1]["source"], "checks.md");
        assert_eq!(result["owner_wait"]["entries"][1]["age_seconds"], 300);
        assert!(!dir.path().join("leases").exists());
    }

    #[tokio::test]
    async fn any_agent_can_select_its_repository_and_deployment() {
        let error = call_using(Some("fork-id"), json!({
            "repository":"/fixture/repository", "after_close":"checks.md",
            "deploy_project":"example", "deploy_service_account":"runtime@example.iam.gserviceaccount.com"
        }), |agent, options| async move {
            assert_eq!(agent, "fork-id");
            assert_eq!(options.repository, Some(PathBuf::from("/fixture/repository")));
            assert_eq!(options.after_close, Some(PathBuf::from("checks.md")));
            assert_eq!(options.deploy_project.as_deref(), Some("example"));
            assert_eq!(options.deploy_service_account.as_deref(), Some("runtime@example.iam.gserviceaccount.com"));
            Err("fixture stopped before collection".into())
        }).await.unwrap_err();
        assert_eq!(error, "fixture stopped before collection");
    }

    #[tokio::test]
    async fn invalid_arguments_never_collect_or_run_actions() {
        assert!(call(None, json!({})).await.is_err());
        assert!(call(Some("agent"), json!({"agent_id":" "})).await.is_err());
        assert!(call(Some("agent"), json!({"limit":4})).await.is_err());
        let dir = tempfile::tempdir().unwrap();
        assert!(checkout(dir.path()).is_none());
        let report = owner_facts(
            dir.path(),
            OwnerOptions {
                repository: Some(dir.path().join("missing")),
                ..OwnerOptions::default()
            },
            500,
        )
        .await;
        assert!(!report.errors.is_empty());
        assert!(report.read_sources.is_empty());
    }
}
