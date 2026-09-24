//! Each executor adapter's capability matrix, with the evidence for it.
//!
//! A policy may use only what its adapter has demonstrated
//! ([`crate::policy::Manifest::validate`] refuses the rest, and
//! [`crate::session::drive`] refuses it again at run time). This module
//! says where each cell of the matrix was demonstrated:
//!
//! - **Tests.** Every cell names the tests that exercise it: the scripted
//!   executor's in [`crate::scripted`], and the CLI adapters' against
//!   stand-in CLIs in [`crate::adapter`]. They run with the crate's tests.
//! - **The real CLIs.** [`demonstrate`] drives the installed Claude Code
//!   or Codex through each capability against the local model server in
//!   [`crate::capture`], which answers every model call with a short
//!   scripted turn. The CLI runs with a scratch home, a dummy credential,
//!   and a cleared environment, as a prompt capture does, so no real
//!   credential is read or sent and no inference runs.
//!
//! `coder-one capabilities` writes the matrix, with the last real
//! demonstration of each adapter, to
//! `~/.openagents/coder-one/capabilities.json`, which the Gym's runbooks
//! view and `gym coder capabilities` read.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::adapter::{self, CliSession};
use crate::delegate::{Agent, Briefing, BriefingInputs, Cli, Credential, Report, Status};
use crate::record::Recorder;
use crate::session::{self, Capabilities, Capability, Controls, Driven, Steer, Trigger};
use crate::stream::Kind;

/// The schema of the matrix file.
pub const SCHEMA: &str = "openagents.coder-one.capabilities.v1";

/// The schema of one real-CLI demonstration.
pub const DEMONSTRATION_SCHEMA: &str = "openagents.coder-one.capability-demonstration.v1";

/// Every adapter the matrix covers.
pub const ADAPTERS: [&str; 3] = ["scripted", "claude-code", "codex"];

/// The tests that demonstrate one capability of one adapter.
#[must_use]
pub fn tests(adapter: &str, capability: Capability) -> &'static [&'static str] {
    match (adapter, capability) {
        ("scripted", Capability::Start) => {
            &["scripted::tests::a_script_writes_its_files_at_their_times_and_answers"]
        }
        ("scripted", Capability::Observe) => {
            &["scripted::tests::the_host_observes_events_as_they_arrive"]
        }
        ("scripted", Capability::Stop) => &[
            "scripted::tests::the_host_stops_a_hung_session_at_its_deadline_with_an_acknowledgement",
        ],
        ("scripted", Capability::Resume) => {
            &["scripted::tests::a_stopped_session_resumes_under_its_own_id"]
        }
        ("scripted", Capability::Steer) => {
            &["scripted::tests::a_steer_unblocks_the_session_and_plays_its_list"]
        }
        ("claude-code" | "codex", Capability::Start | Capability::Observe) => &[
            "adapter::tests::claude_code_starts_under_the_hosts_session_id_and_is_observed_as_it_runs",
            "adapter::tests::codex_refuses_a_steer_it_has_not_demonstrated_and_still_runs",
            "tail::tests::chunks_of_any_size_read_the_same_events_as_the_whole_stream",
        ],
        ("claude-code" | "codex", Capability::Stop) => &[
            "adapter::tests::a_stop_ends_the_process_group_and_a_resume_continues_the_same_session",
            "adapter::tests::the_host_deadline_is_a_timeout_with_an_acknowledged_stop",
        ],
        ("claude-code" | "codex", Capability::Resume) => &[
            "adapter::tests::a_stop_ends_the_process_group_and_a_resume_continues_the_same_session",
        ],
        ("claude-code", Capability::Steer) => {
            &["adapter::tests::claude_code_is_steered_through_its_stream_json_input"]
        }
        ("codex", Capability::Steer) => &[
            "adapter::tests::codex_refuses_a_steer_it_has_not_demonstrated_and_still_runs",
            "policy::tests::a_policy_may_use_only_the_session_capabilities_its_adapter_demonstrated",
        ],
        _ => &[],
    }
}

/// An adapter's demonstrated matrix and its note.
#[must_use]
pub fn matrix(adapter: &str) -> Option<(Capabilities, &'static str)> {
    match adapter {
        "scripted" => Some((
            Capabilities::all(),
            "The scripted executor plays a script instead of a model; its tests demonstrate every capability, so the host's handling of each runs in milliseconds.",
        )),
        "claude-code" => Some(adapter::capabilities(Agent::ClaudeCode)),
        "codex" => Some(adapter::capabilities(Agent::Codex)),
        _ => None,
    }
}

/// The matrix file: every adapter's matrix, the tests behind each cell,
/// and each adapter's last real demonstration when there is one.
#[must_use]
pub fn document(demonstrations: &[Value]) -> Value {
    let adapters: Vec<Value> = ADAPTERS
        .iter()
        .filter_map(|name| {
            let (capabilities, note) = matrix(name)?;
            let cells: serde_json::Map<String, Value> = Capability::ALL
                .iter()
                .map(|capability| {
                    (
                        capability.word().to_string(),
                        json!({
                            "demonstrated": capabilities.has(*capability),
                            "tests": tests(name, *capability),
                        }),
                    )
                })
                .collect();
            let demonstration = demonstrations
                .iter()
                .find(|d| d["adapter"] == json!(name))
                .cloned()
                .unwrap_or(Value::Null);
            Some(json!({
                "adapter": name,
                "capabilities": cells,
                "note": note,
                "demonstration": demonstration,
            }))
        })
        .collect();
    json!({
        "schema": SCHEMA,
        "written_at": atif::document::iso(atif::now_ms()),
        "version": crate::episode::version(),
        "adapters": adapters,
    })
}

/// Where the matrix file lives: `~/.openagents/coder-one/capabilities.json`.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/capabilities.json"))
}

/// The demonstrations a matrix file already holds.
#[must_use]
pub fn previous(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|doc| doc.get("adapters").and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .into_iter()
        .map(|adapter| adapter["demonstration"].clone())
        .filter(|demonstration| !demonstration.is_null())
        .collect()
}

fn briefing(text: &str) -> Briefing {
    Briefing::build(
        &BriefingInputs {
            instruction: text.to_string(),
            requirements: Vec::new(),
            files: Vec::new(),
            spans: Vec::new(),
            commands: Vec::new(),
            last_output: None,
            conclusion: String::new(),
            directions: String::new(),
        },
        2_000,
    )
}

fn claims(driven: &Driven) -> Vec<String> {
    driven
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            Kind::AssistantClaim { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn outcome(driven: &Driven, capability: Capability) -> Option<String> {
    driven
        .actions
        .iter()
        .find(|action| action.capability == capability)
        .map(|action| format!("{}: {}", action.outcome, action.detail))
}

fn status(report: &Report) -> String {
    report.status.to_string()
}

/// Runs the installed `agent` through each capability against the local
/// model server and returns the demonstration record.
///
/// # Errors
///
/// Returns a message when the binary is missing or the scratch directory
/// or the server can't be set up. A capability that fails is recorded as
/// not demonstrated, not an error.
pub async fn demonstrate(agent: Agent, binary: &Path) -> Result<Value, String> {
    let scratch = std::env::temp_dir().join(format!(
        "coder-one-capabilities-{}-{}-{}",
        agent.word(),
        std::process::id(),
        atif::now_ms()
    ));
    let result = demonstrate_in(agent, binary, &scratch).await;
    if std::env::var_os("CODER_ONE_CAPTURE_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&scratch);
    }
    result
}

async fn demonstrate_in(agent: Agent, binary: &Path, scratch: &Path) -> Result<Value, String> {
    let home = scratch.join("home");
    let work = scratch.join("app");
    let artifacts = scratch.join("artifacts");
    for dir in [&home, &work, &artifacts, &home.join(".codex")] {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    }
    let server = crate::capture::Server::answering()?;
    let version = std::process::Command::new(binary)
        .arg("--version")
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());
    let binary = match agent {
        Agent::ClaudeCode => binary.to_path_buf(),
        Agent::Microluna => {
            return Err("Microluna runs in this process and has no CLI to demonstrate".to_string());
        }
        Agent::Codex => {
            let catalog = scratch.join("catalog.json");
            let cache = std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".codex/models_cache.json"));
            let catalog = cache
                .filter(|cache| crate::capture::codex_catalog(cache, &catalog).is_ok())
                .map(|_| catalog);
            crate::capture::codex_wrapper(scratch, binary, &server.url(), catalog.as_deref())?
        }
    };
    let cli = Cli {
        agent,
        binary: Some(binary.clone()),
        model: agent.default_model().to_string(),
        deadline: Duration::from_secs(60),
        workdir: work.clone(),
        artifacts: artifacts.clone(),
        artifacts_label: "artifacts".to_string(),
        env: Vec::new(),
        credential: match agent {
            Agent::ClaudeCode => Credential::OauthToken,
            Agent::Codex | Agent::Microluna => Credential::OpenAiKey,
        },
        effort: None,
        tools: None,
        prompt_cache_ttl: None,
        system: None,
        episode: crate::deadline::Deadline::unbounded(),
        gate: None,
        granted: None,
        runs: 1,
        control: crate::delegate::Control::default(),
    };
    let url = server.url();
    // Keep only what the dispatch itself sets, then the demonstration's
    // own variables: nothing inherited, so no real credential is present.
    let wrap = move |mut command: std::process::Command| {
        let explicit: Vec<(String, String)> = command
            .get_envs()
            .filter_map(|(name, value)| {
                Some((
                    name.to_string_lossy().into_owned(),
                    value?.to_string_lossy().into_owned(),
                ))
            })
            .collect();
        command.env_clear();
        for (name, value) in explicit {
            command.env(name, value);
        }
        command
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &home)
            .env("CODEX_HOME", home.join(".codex"))
            .env("ANTHROPIC_BASE_URL", &url)
            .env("CLAUDE_CODE_OAUTH_TOKEN", crate::capture::DUMMY_TOKEN)
            .env("CAPTURE_KEY", crate::capture::DUMMY_TOKEN);
        Ok(command)
    };
    let base = Controls {
        deadline_ms: 90_000,
        tick_ms: 50,
        ..Controls::default()
    };
    let mut cells = serde_json::Map::new();
    let mut runs = Vec::new();

    // Start and observe: one turn, read as it arrives.
    let started = Instant::now();
    let (driven, _) = drive(&cli, &binary, &wrap, "start", false, &base, "Say hello.").await;
    let observed = driven.events.len();
    let answered = driven.report.status == Status::Answered;
    let session_id = driven.session_id.clone();
    cells.insert(
        "start".to_string(),
        json!({
            "demonstrated": answered && session_id.is_some(),
            "evidence": format!("status {}; session {}", status(&driven.report), session_id.clone().unwrap_or_else(|| "none".to_string())),
        }),
    );
    cells.insert(
        "observe".to_string(),
        json!({
            "demonstrated": observed > 0 && claims(&driven).iter().any(|c| c == "heard briefing"),
            "evidence": format!("{observed} normalized events; claims {:?}", claims(&driven)),
        }),
    );
    runs.push(run_record("start", &driven, started));

    // Stop and resume: a turn the server holds back, stopped, then resumed.
    let started = Instant::now();
    let controls = Controls {
        stop_when: Some(Trigger::After { ms: 3_000 }),
        resume: Some("Please resume and say so.".to_string()),
        ..base.clone()
    };
    let (driven, _) = drive(
        &cli,
        &binary,
        &wrap,
        "stop",
        false,
        &controls,
        "Please hang until you are stopped.",
    )
    .await;
    let stop = outcome(&driven, Capability::Stop);
    let resume = outcome(&driven, Capability::Resume);
    cells.insert(
        "stop".to_string(),
        json!({
            "demonstrated": stop.as_deref().is_some_and(|s| s.starts_with("done") && s.contains("the group was empty")),
            "evidence": stop,
        }),
    );
    cells.insert(
        "resume".to_string(),
        json!({
            "demonstrated": resume.as_deref().is_some_and(|s| s.starts_with("done"))
                && claims(&driven).iter().any(|c| c == "heard resume")
                && driven.transitions.iter().filter(|t| t.to == session::Phase::Running).count() == 2,
            "evidence": format!("{}; claims {:?}", resume.unwrap_or_else(|| "no resume".to_string()), claims(&driven)),
        }),
    );
    runs.push(run_record("stop-resume", &driven, started));

    // Steer: a message written into the running session.
    let started = Instant::now();
    let controls = Controls {
        steer: Some(Steer {
            when: Trigger::Claim {
                contains: "heard briefing".to_string(),
            },
            message: "Please steer toward the tests.".to_string(),
        }),
        ..base.clone()
    };
    let (driven, _) = drive(&cli, &binary, &wrap, "steer", true, &controls, "Say hello.").await;
    let steer = outcome(&driven, Capability::Steer);
    cells.insert(
        "steer".to_string(),
        json!({
            "demonstrated": steer.as_deref().is_some_and(|s| s.starts_with("done"))
                && claims(&driven).iter().any(|c| c == "heard steer"),
            "evidence": format!("{}; claims {:?}", steer.unwrap_or_else(|| "no steer".to_string()), claims(&driven)),
        }),
    );
    runs.push(run_record("steer", &driven, started));

    let (declared, _) = adapter::capabilities(agent);
    let agrees = Capability::ALL.iter().all(|capability| {
        cells[capability.word()]["demonstrated"].as_bool() == Some(declared.has(*capability))
    });
    let requests = server.requests().len();
    Ok(json!({
        "schema": DEMONSTRATION_SCHEMA,
        "adapter": agent.word(),
        "cli_version": version,
        "at": atif::document::iso(atif::now_ms()),
        "server": "the local model server in coder_one::capture, answering each call with a scripted turn",
        "credential": "a dummy token in a scratch home with a cleared environment",
        "model_requests": requests,
        "capabilities": cells,
        "agrees_with_matrix": agrees,
        "runs": runs,
    }))
}

async fn drive(
    cli: &Cli,
    binary: &Path,
    wrap: &crate::delegate::Wrap<'_>,
    name: &str,
    steerable: bool,
    controls: &Controls,
    text: &str,
) -> (Driven, Recorder) {
    let recorder = Recorder::default();
    let mut session = CliSession::new(cli, binary.to_path_buf(), wrap, name);
    session.steerable = steerable;
    let driven = session::drive(
        &mut session,
        &briefing(text),
        controls,
        &recorder,
        &mut session::virtual_time(),
    )
    .await;
    session.shutdown().await;
    (driven, recorder)
}

fn run_record(name: &str, driven: &Driven, started: Instant) -> Value {
    json!({
        "name": name,
        "milliseconds": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "record": driven.record(),
    })
}

/// The matrix as text rows.
#[must_use]
pub fn lines(document: &Value) -> Vec<String> {
    let mut lines = vec![
        "Executor capability matrix · a policy may use only what its adapter demonstrated"
            .to_string(),
        format!(
            "  {:<12} {:<7} {:<7} {:<7} {:<7} {:<7}  real CLI demonstration",
            "adapter", "start", "observe", "stop", "resume", "steer"
        ),
    ];
    for adapter in document["adapters"].as_array().into_iter().flatten() {
        let cell = |capability: &str| {
            let declared = adapter["capabilities"][capability]["demonstrated"].as_bool();
            let real =
                adapter["demonstration"]["capabilities"][capability]["demonstrated"].as_bool();
            match (declared, real) {
                (Some(true), Some(false)) => "yes!".to_string(),
                (Some(true), _) => "yes".to_string(),
                (Some(false), _) => "refused".to_string(),
                _ => "—".to_string(),
            }
        };
        let demonstration = &adapter["demonstration"];
        let real = if demonstration.is_null() {
            "none recorded".to_string()
        } else {
            format!(
                "{} at {}{}",
                demonstration["cli_version"]
                    .as_str()
                    .unwrap_or("unknown version"),
                demonstration["at"].as_str().unwrap_or("—"),
                if demonstration["agrees_with_matrix"] == json!(true) {
                    ", agrees"
                } else {
                    ", DISAGREES"
                }
            )
        };
        lines.push(format!(
            "  {:<12} {:<7} {:<7} {:<7} {:<7} {:<7}  {real}",
            adapter["adapter"].as_str().unwrap_or("?"),
            cell("start"),
            cell("observe"),
            cell("stop"),
            cell("resume"),
            cell("steer"),
        ));
    }
    lines.push(
        "  yes: demonstrated by tests; refused: the host refuses it; yes!: the real CLI did not show it"
            .to_string(),
    );
    lines
}

/// `coder-one capabilities` usage.
pub const USAGE: &str = "\
coder-one capabilities [--demonstrate] [--json] [--out PATH | --no-write]

Prints each executor adapter's capability matrix: start, observe, stop,
resume, and steer, with the tests that demonstrate each cell, and writes it
to ~/.openagents/coder-one/capabilities.json for the Gym.

  --demonstrate  also drive the installed Claude Code and Codex through each
                 capability against a local model server that answers every
                 call with a scripted turn: a scratch home, a dummy
                 credential, and no inference
  --json         print the matrix file instead of text
  --out PATH     write the matrix here instead
  --no-write     print only";

/// Runs `coder-one capabilities`.
///
/// # Errors
///
/// Returns a message for an unknown flag or a file that can't be written.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let mut demonstrate_real = false;
    let mut json_out = false;
    let mut write = true;
    let mut out = default_path();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--demonstrate" => demonstrate_real = true,
            "--json" => json_out = true,
            "--no-write" => write = false,
            "--out" => {
                index += 1;
                out = Some(PathBuf::from(args.get(index).ok_or("--out needs a path")?));
            }
            "help" | "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(0);
            }
            other => return Err(format!("unknown flag {other}\n\n{USAGE}")),
        }
        index += 1;
    }
    let mut demonstrations = out.as_deref().map(previous).unwrap_or_default();
    if demonstrate_real {
        let env = |name: &str| std::env::var(name).ok();
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            let Some(binary) = crate::delegate::binary(agent, env) else {
                eprintln!("capabilities ▸ no {} binary; skipped", agent.program());
                continue;
            };
            eprintln!(
                "capabilities ▸ demonstrating {} ({}) against the local model server",
                agent.word(),
                binary.display()
            );
            let record = demonstrate(agent, &binary).await?;
            demonstrations.retain(|d| d["adapter"] != json!(agent.word()));
            demonstrations.push(record);
        }
    }
    let document = document(&demonstrations);
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&document).map_err(|error| error.to_string())?
        );
    } else {
        for line in lines(&document) {
            println!("{line}");
        }
    }
    if write && let Some(path) = out {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        }
        crate::record::write_atomic(
            &path,
            serde_json::to_string_pretty(&document)
                .map_err(|error| error.to_string())?
                .as_bytes(),
        )?;
        eprintln!("capabilities ▸ wrote {}", path.display());
    }
    let disagrees = document["adapters"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|adapter| adapter["demonstration"]["agrees_with_matrix"] == json!(false));
    Ok(i32::from(disagrees))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_demonstrated_cell_names_its_tests_and_every_refused_cell_says_so() {
        let document = document(&[]);
        let adapters = document["adapters"].as_array().unwrap();
        assert_eq!(adapters.len(), 3);
        for adapter in adapters {
            let name = adapter["adapter"].as_str().unwrap();
            for capability in Capability::ALL {
                let cell = &adapter["capabilities"][capability.word()];
                assert!(
                    !cell["tests"].as_array().unwrap().is_empty(),
                    "{name} {} names no test",
                    capability.word()
                );
            }
        }
        let codex = adapters.iter().find(|a| a["adapter"] == "codex").unwrap();
        assert_eq!(codex["capabilities"]["steer"]["demonstrated"], false);
        let text = lines(&document).join("\n");
        assert!(text.contains("codex"));
        assert!(text.contains("refused"));
    }

    #[test]
    fn the_answering_server_echoes_the_last_request_word() {
        let body = json!({"stream": true, "messages": [
            {"role": "user", "content": "Please hang until stopped."},
            {"role": "assistant", "content": "…"},
            {"role": "user", "content": [{"type": "text", "text": "Please resume and say so."}]},
        ]});
        assert_eq!(crate::capture::heard(&body), "resume");
        let (delay, kind, reply) =
            crate::capture::answer("/v1/messages?beta=true", body.to_string().as_bytes());
        assert!(delay.is_zero());
        assert_eq!(kind, "text/event-stream");
        assert!(reply.contains("heard resume"));
        let hang = json!({"input": [{"role": "user", "content": [{"type": "input_text", "text": "Please hang."}]}]});
        let (delay, _, reply) =
            crate::capture::answer("/v1/responses", hang.to_string().as_bytes());
        assert_eq!(delay, crate::capture::HANG);
        assert!(reply.contains("response.completed"));
    }

    /// Drives the installed CLIs through every capability against the
    /// local model server. It needs the binaries, so it runs only when
    /// `CODER_ONE_REAL_CLI=1`; `coder-one capabilities --demonstrate` runs
    /// the same demonstration.
    #[tokio::test]
    async fn the_real_clis_show_their_matrices_against_the_local_model_server() {
        if std::env::var("CODER_ONE_REAL_CLI").as_deref() != Ok("1") {
            return;
        }
        let env = |name: &str| std::env::var(name).ok();
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            let Some(binary) = crate::delegate::binary(agent, env) else {
                continue;
            };
            let record = demonstrate(agent, &binary).await.unwrap();
            assert_eq!(
                record["agrees_with_matrix"],
                true,
                "{}",
                serde_json::to_string_pretty(&record).unwrap()
            );
        }
    }
}
