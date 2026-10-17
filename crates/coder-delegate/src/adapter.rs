//! Terminal session policy and retained streams around the shared app harness.
//! The harness owns native engine protocols; [`CliSession`] keeps the terminal's
//! borrowed boundary wrapper, deadlines, accounting, and process cleanup.

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::delegate::Agent;
pub use crate::native::CliSession;
use crate::session::Capabilities;

/// The bytes one stream file keeps: its first and last halves past this.
pub const STREAM_KEEP: usize = 8 * 1024 * 1024;

/// What each CLI adapter has demonstrated, and the tests that show it.
#[must_use]
pub fn capabilities(agent: Agent) -> (Capabilities, &'static str) {
    match agent {
        Agent::ClaudeCode => (
            Capabilities::all(),
            "Demonstrated by coder_one::adapter's tests against a stand-in CLI and, with `coder-one capabilities --demonstrate`, against Claude Code 2.1.280 and a local model server: start with --session-id, observe the stream as it arrives, stop with an empty process group, resume with --resume, and steer through --input-format stream-json.",
        ),
        Agent::Microluna => (
            Capabilities::start_only(),
            "Microluna runs in this process: coder_one::micro starts each short session and records its events as they happen, and the host doesn't stop, resume, or steer it; its own bounds end it.",
        ),
        Agent::Codex => (
            Capabilities::all(),
            "The shared app-server driver starts and resumes threads, streams events, confirms turn/steer, and stops the supervised process group.",
        ),
        Agent::OpenCode => (
            Capabilities {
                steer: false,
                ..Capabilities::all()
            },
            "The shared HTTP driver starts and resumes sessions, streams events, and stops the supervised server. Running turns accept queued follow-ups; the terminal exposes stop and resume for mid-turn control.",
        ),
    }
}

/// How each CLI adapter takes a steer and what confirms it; see
/// [`crate::steering`]. Mid-turn steering is exactly the demonstrated
/// `steer` capability.
#[must_use]
pub fn steering(agent: Agent) -> crate::steering::Steering {
    match agent {
        Agent::ClaudeCode => crate::steering::CLAUDE_CODE,
        Agent::Codex => crate::steering::CODEX_APP_SERVER,
        Agent::Microluna => crate::steering::MICROLUNA,
        Agent::OpenCode => crate::steering::OPENCODE_SERVER,
    }
}

/// A new session ID in UUID version 4 form, which Claude Code's
/// `--session-id` requires.
#[must_use]
pub fn new_session_id() -> String {
    let mut bytes = [0u8; 16];
    let random = std::fs::File::open("/dev/urandom")
        .and_then(|mut file| std::io::Read::read_exact(&mut file, &mut bytes));
    if random.is_err() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seed = format!(
            "{}-{}-{}",
            std::process::id(),
            atif::now_ms(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        );
        bytes.copy_from_slice(&Sha256::digest(seed.as_bytes())[..16]);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// One Claude Code stream-json user message.
#[must_use]
pub fn user_message(text: &str) -> String {
    let mut line = json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": text }] },
    })
    .to_string();
    line.push('\n');
    line
}

/// A native stream kept on disk while it arrives, bounded.
pub struct Retainer {
    path: PathBuf,
    recorded: String,
    file: Option<std::fs::File>,
    written: usize,
    keep: usize,
    tail: VecDeque<u8>,
    total: u64,
    gaps: Vec<supervise::Gap>,
}

impl Retainer {
    /// Starts `path`, which the record names `recorded`, keeping `keep`
    /// bytes.
    #[must_use]
    pub fn create(path: &Path, recorded: &str, keep: usize) -> Self {
        Retainer {
            path: path.to_path_buf(),
            recorded: recorded.to_string(),
            file: std::fs::File::create(path).ok(),
            written: 0,
            keep: keep.max(2),
            tail: VecDeque::new(),
            total: 0,
            gaps: Vec::new(),
        }
    }

    /// Keeps a chunk.
    pub fn push(&mut self, bytes: &[u8]) {
        self.total += bytes.len() as u64;
        let room = self.keep.saturating_sub(self.written).min(bytes.len());
        if room > 0
            && let Some(file) = &mut self.file
            && file.write_all(&bytes[..room]).is_ok()
        {
            self.written += room;
        }
        let half = self.keep / 2;
        self.tail.extend(&bytes[bytes.len().saturating_sub(half)..]);
        while self.tail.len() > half {
            self.tail.pop_front();
        }
    }

    /// Notes a span the supervisor dropped.
    pub fn gap(&mut self, gap: supervise::Gap) {
        self.total += gap.bytes;
        self.gaps.push(gap);
        // The tail can't straddle a hole: what it held came before it.
        self.tail.clear();
    }

    /// Finishes the file and returns its record.
    pub fn finish(mut self) -> Option<Value> {
        let file = self.file.take()?;
        drop(file);
        let truncated = self.total > self.keep as u64;
        if truncated {
            let head = std::fs::read(&self.path).ok()?;
            let half = self.keep / 2;
            let head = &head[..half.min(head.len())];
            let head_end = head
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(head.len(), |i| i + 1);
            let tail: Vec<u8> = self.tail.iter().copied().collect();
            let tail_start = tail
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(0, |i| i + 1);
            let omitted = self.total - head_end as u64 - (tail.len() - tail_start) as u64;
            let marker = json!({
                "type": "openagents_truncated",
                "omitted_bytes": omitted,
                "total_bytes": self.total,
            });
            let mut kept = head[..head_end].to_vec();
            kept.extend_from_slice(marker.to_string().as_bytes());
            kept.push(b'\n');
            kept.extend_from_slice(&tail[tail_start..]);
            crate::record::write_atomic(&self.path, &kept).ok()?;
        }
        let kept = std::fs::read(&self.path).ok()?;
        Some(json!({
            "path": self.recorded,
            "bytes": self.total,
            "kept_bytes": kept.len(),
            "truncated": truncated,
            "sha256": Sha256::digest(&kept).iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "gaps": self.gaps.iter().map(|gap| json!({ "offset": gap.offset, "bytes": gap.bytes })).collect::<Vec<_>>(),
            "retention": format!("first {} bytes written as they arrive; past that, the first and last halves", self.keep),
        }))
    }
}

/// Stand-ins for the two CLIs, for tests: shell scripts that speak each
/// CLI's arguments and stream format closely enough to exercise every
/// capability, and nothing else.
pub mod standin {
    use std::path::{Path, PathBuf};

    /// Claude Code's print mode: stream-json out; text or stream-json in;
    /// `--session-id` and `--resume` name the session. Each user message
    /// is a turn that answers `heard <word>`, where the word is `steer`,
    /// `resume`, or `briefing` by what the message says.
    /// `STANDIN_DELAY` sleeps between a turn's events; `STANDIN_HANG`
    /// names a marker a background child writes after two seconds while
    /// the turn hangs, after saying `hanging`.
    pub const CLAUDE: &str = r#"#!/bin/sh
id=""; json_in=0
while [ $# -gt 0 ]; do
  case "$1" in
    --session-id|--resume) id=$2; shift ;;
    --resume=*) id=${1#--resume=} ;;
    --input-format) [ "$2" = stream-json ] && json_in=1; shift ;;
  esac
  shift
done
n=0
turn() {
  n=$((n+1))
  echo "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"$id\",\"model\":\"stand-in\",\"claude_code_version\":\"stand-in\"}"
  sleep "${STANDIN_DELAY:-0}"
  if [ -n "$STANDIN_HANG" ] && [ "$1" = briefing ]; then
    echo '{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"hanging"}}}'
    echo "{\"type\":\"assistant\",\"message\":{\"id\":\"msg_hang\",\"content\":[{\"type\":\"text\",\"text\":\"hanging\"}],\"usage\":{\"input_tokens\":1}}}"
    (sleep 2; printf harmless > "$STANDIN_HANG") &
    wait
  fi
  echo "{\"type\":\"assistant\",\"message\":{\"id\":\"msg_$1_$n\",\"content\":[{\"type\":\"tool_use\",\"id\":\"t$n\",\"name\":\"Write\",\"input\":{\"file_path\":\"out-$1.txt\",\"content\":\"x\"}}],\"usage\":{\"input_tokens\":10}}}"
  echo "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"heard $1\"}}}"
  echo "{\"type\":\"assistant\",\"message\":{\"id\":\"msg_$1_$n\",\"content\":[{\"type\":\"text\",\"text\":\"heard $1\"}],\"usage\":{\"input_tokens\":10}}}"
  sleep "${STANDIN_DELAY:-0}"
  echo "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":$n,\"result\":\"heard $1\",\"session_id\":\"$id\",\"total_cost_usd\":0.01}"
}
word() {
  case "$1" in *steer*) echo steer ;; *resume*) echo resume ;; *) echo briefing ;; esac
}
if [ "$json_in" = 1 ]; then
  while IFS= read -r line; do
    case "$line" in *'"type":"control_request"'*) continue ;; esac
    turn "$(word "$line")"
  done
else
  turn "$(word "$(cat)")"
fi
"#;

    /// A local Codex app-server for session control tests.
    pub const CODEX: &str = include_str!("../fixtures/native-codex.py");

    /// A local OpenCode server for session control tests.
    pub const OPENCODE: &str = include_str!("../fixtures/native-opencode.py");

    /// Writes `script` as an executable `name` under `dir`.
    ///
    /// # Panics
    ///
    /// Panics when the file can't be written; it is for tests.
    #[must_use]
    pub fn install(dir: &Path, name: &str, script: &str) -> PathBuf {
        std::fs::create_dir_all(dir).expect("stand-in directory");
        let path = dir.join(name);
        std::fs::write(&path, script).expect("stand-in script");
        // The stand-ins are `/bin/sh` scripts, which only Unix runs.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("stand-in mode");
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delegate::{self, Briefing, BriefingInputs, Cli, Credential, Status};
    use crate::record::Recorder;
    use crate::session::{self, Capability, Controls, Driven, Phase, Steer, Trigger};
    use crate::stream::Kind;
    use std::time::Duration;

    fn briefing() -> Briefing {
        Briefing::build(
            &BriefingInputs {
                instruction: "Write the file.".to_string(),
                requirements: vec![],
                files: vec![],
                spans: vec![],
                commands: vec![],
                last_output: None,
                conclusion: String::new(),
                directions: "Go.".to_string(),
            },
            2_000,
        )
    }

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "coder-one-adapter-{label}-{}-{}",
            std::process::id(),
            atif::now_ms()
        ));
        std::fs::create_dir_all(dir.join("artifacts")).unwrap();
        dir
    }

    fn cli(agent: Agent, dir: &Path, env: &[(&str, &str)]) -> Cli {
        let (name, script) = match agent {
            Agent::ClaudeCode => ("claude", standin::CLAUDE),
            Agent::Codex | Agent::Microluna => ("codex", standin::CODEX),
            Agent::OpenCode => ("opencode", standin::OPENCODE),
        };
        let binary = standin::install(&dir.join("bin"), name, script);
        Cli {
            agent,
            binary: Some(binary),
            model: "stand-in".to_string(),
            deadline: Duration::from_secs(30),
            workdir: dir.to_path_buf(),
            artifacts: dir.join("artifacts"),
            artifacts_label: "artifacts".to_string(),
            env: env
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            credential: Credential::OauthToken,
            effort: None,
            tools: None,
            prompt_cache_ttl: None,
            codex_config: Vec::new(),
            hooks: None,
            system: None,
            episode: crate::deadline::Deadline::unbounded(),
            gate: None,
            granted: None,
            runs: 1,
            control: crate::delegate::Control::default(),
        }
    }

    async fn run(cli: &Cli, controls: &Controls, steerable: bool) -> (Driven, Recorder) {
        let recorder = Recorder::default();
        let wrap = Ok;
        let mut session = CliSession::new(cli, cli.binary.clone().unwrap(), &wrap, "delegate-1");
        session.steerable = steerable;
        let driven = session::drive(
            &mut session,
            &briefing(),
            controls,
            &recorder,
            &mut session::virtual_time(),
        )
        .await;
        session.shutdown().await;
        (driven, recorder)
    }

    fn controls() -> Controls {
        Controls {
            deadline_ms: 20_000,
            tick_ms: 20,
            ..Controls::default()
        }
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

    fn action(driven: &Driven, capability: Capability) -> Option<&session::Action> {
        driven
            .actions
            .iter()
            .find(|action| action.capability == capability)
    }

    #[test]
    fn every_claude_code_launch_keeps_the_accounts_connectors_out() {
        let dir = scratch("claude-connectors");
        let connectors = |command: &std::process::Command| {
            command
                .get_envs()
                .find(|(name, _)| *name == "ENABLE_CLAUDEAI_MCP_SERVERS")
                .and_then(|(_, value)| value)
                .map(|value| value.to_string_lossy().into_owned())
        };
        let claude = cli(Agent::ClaudeCode, &dir, &[]);
        let mut command = std::process::Command::new("claude");
        claude.configure_command(&mut command);
        assert_eq!(connectors(&command).as_deref(), Some("false"));
        let codex = cli(Agent::Codex, &dir, &[]);
        let mut command = std::process::Command::new("codex");
        codex.configure_command(&mut command);
        assert_eq!(connectors(&command), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn every_launch_marks_its_session_as_the_engines() {
        let dir = scratch("engine-mark");
        let env = |command: &std::process::Command, name: &str| {
            command
                .get_envs()
                .find(|(key, _)| *key == name)
                .and_then(|(_, value)| value)
                .map(|value| value.to_string_lossy().into_owned())
        };
        for (agent, variable, value) in [
            (
                Agent::ClaudeCode,
                "CLAUDE_CODE_ENTRYPOINT",
                coder_history::engine::MARK,
            ),
            (
                Agent::Codex,
                "CODEX_INTERNAL_ORIGINATOR_OVERRIDE",
                coder_history::engine::MARK,
            ),
            // OpenCode records no caller: the engine's sessions are saved
            // in the engine's own database, never the one the host mirrors.
            (
                Agent::OpenCode,
                "OPENCODE_DB",
                coder_history::engine::OPENCODE_DATABASE,
            ),
        ] {
            let cli = cli(agent, &dir, &[]);
            let mut command = std::process::Command::new("agent");
            cli.configure_command(&mut command);
            assert_eq!(env(&command, variable).as_deref(), Some(value), "{agent:?}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn claude_code_starts_under_the_hosts_session_id_and_is_observed_as_it_runs() {
        let dir = scratch("claude-observe");
        let cli = cli(Agent::ClaudeCode, &dir, &[("STANDIN_DELAY", "0.4")]);
        let (driven, recorder) = run(&cli, &controls(), false).await;
        assert_eq!(
            driven.report.status,
            Status::Answered,
            "{:?}",
            driven.report
        );
        let id = driven.session_id.clone().unwrap();
        assert_eq!(id.len(), 36, "a UUID the host chose: {id}");
        assert!(matches!(
            &driven.events[0].kind,
            Kind::SessionStarted { session_id: Some(started) } if *started == id
        ));
        assert_eq!(claims(&driven), vec!["heard briefing"]);
        // The first event was recorded while the session still ran.
        let steps: Vec<Value> = recorder
            .steps()
            .iter()
            .filter_map(|step| step.extensions.get(session::EVENT_KEY).cloned())
            .collect();
        let first = steps.first().unwrap()["at_ms"].as_u64().unwrap();
        let last = steps.last().unwrap()["at_ms"].as_u64().unwrap();
        assert!(last >= first + 600, "observed at {first} and {last} ms");
        assert!(steps.iter().all(|s| s["session_id"] == json!(id)));
        assert!(steps.iter().all(|s| s["event"]["offset"].is_u64()));
        // The artifact change moved the workspace revision.
        let revisions: Vec<u64> = steps
            .iter()
            .map(|s| s["revision"].as_u64().unwrap())
            .collect();
        assert_eq!(revisions.first(), Some(&0));
        assert_eq!(revisions.last(), Some(&1));
        let stream = driven.report.stream.clone().unwrap();
        assert_eq!(stream["path"], "artifacts/delegate-1.stream.jsonl");
        assert_eq!(stream["truncated"], false);
        let kept = std::fs::read_to_string(dir.join("artifacts/delegate-1.stream.jsonl")).unwrap();
        assert_eq!(stream["bytes"].as_u64(), Some(kept.len() as u64));
        assert_eq!(driven.report.summary.total_cost_usd, Some(0.01));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn claude_code_is_steered_through_its_stream_json_input() {
        let dir = scratch("claude-steer");
        let cli = cli(Agent::ClaudeCode, &dir, &[]);
        let controls = Controls {
            steer: Some(Steer {
                when: Trigger::Claim {
                    contains: "heard briefing".to_string(),
                },
                message: "Please steer toward the tests.".to_string(),
            }),
            ..controls()
        };
        let (driven, _) = run(&cli, &controls, true).await;
        assert_eq!(action(&driven, Capability::Steer).unwrap().outcome, "done");
        assert_eq!(claims(&driven), vec!["heard briefing", "heard steer"]);
        assert_eq!(driven.report.status, Status::Answered);
        assert_eq!(driven.report.summary.num_turns, Some(2));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_stop_ends_the_process_group_and_a_resume_continues_the_same_session() {
        for agent in [Agent::ClaudeCode, Agent::Codex, Agent::OpenCode] {
            let dir = scratch(&format!("{}-stop", agent.word()));
            let marker = dir.join("marker");
            let marker_text = marker.to_string_lossy().into_owned();
            let cli = cli(agent, &dir, &[("STANDIN_HANG", &marker_text)]);
            // Codex and OpenCode name their session only in their stream,
            // so a stop on the clock can land before a loaded machine's
            // first line and leave nothing to resume. The stop waits for
            // the stand-in's word that it hangs, which follows its ID.
            let controls = Controls {
                stop_when: Some(Trigger::Claim {
                    contains: "hanging".to_string(),
                }),
                resume: Some("Please resume and finish.".to_string()),
                ..controls()
            };
            let (driven, _) = run(&cli, &controls, false).await;
            let stop = action(&driven, Capability::Stop).unwrap();
            assert_eq!(stop.outcome, "done", "{}: {stop:?}", agent.word());
            assert!(
                stop.detail.contains("empty after cleanup"),
                "{}",
                stop.detail
            );
            assert_eq!(
                driven.stops.len(),
                1,
                "{}: {:?}; claims: {:?}",
                agent.word(),
                driven.stops,
                claims(&driven)
            );
            let resume = action(&driven, Capability::Resume).unwrap();
            assert_eq!(resume.outcome, "done", "{}: {resume:?}", agent.word());
            // The resumed process answered under the same session ID.
            let id = driven.session_id.clone().unwrap();
            assert!(resume.detail.contains(&id));
            assert_eq!(claims(&driven).last().unwrap(), "heard resume");
            let generations: Vec<(Phase, u32)> = driven
                .transitions
                .iter()
                .map(|t| (t.to, t.generation))
                .collect();
            assert_eq!(
                generations,
                vec![
                    (Phase::Running, 1),
                    (Phase::Stopped, 1),
                    (Phase::Running, 2),
                    (Phase::Ended, 2),
                ],
                "{}",
                agent.word()
            );
            assert_eq!(driven.report.status, Status::Answered, "{}", agent.word());
            let stream = driven.report.stream.clone().unwrap();
            assert_eq!(
                stream["resumed"][0]["path"],
                "artifacts/delegate-1.resume-1.stream.jsonl"
            );
            tokio::time::sleep(Duration::from_millis(2_500)).await;
            assert!(
                !marker.exists(),
                "{}: a descendant outlived an acknowledged stop",
                agent.word()
            );
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[tokio::test]
    async fn codex_steers_through_the_shared_app_server() {
        let dir = scratch("codex-steer");
        let cli = cli(Agent::Codex, &dir, &[("STANDIN_DELAY", "0.3")]);
        let controls = Controls {
            steer: Some(Steer {
                when: Trigger::CommandFailed,
                message: "Please steer after the failed command.".to_string(),
            }),
            ..controls()
        };
        let (driven, _) = run(&cli, &controls, true).await;
        let steer = action(&driven, Capability::Steer).unwrap();
        assert_eq!(steer.outcome, "done", "{steer:?}");
        assert_eq!(
            driven.report.status,
            Status::Answered,
            "{:?}",
            driven.report
        );
        assert!(claims(&driven).iter().any(|claim| claim == "heard steer"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn the_host_deadline_is_a_timeout_with_an_acknowledged_stop() {
        let dir = scratch("claude-deadline");
        let marker = dir.join("marker");
        let marker_text = marker.to_string_lossy().into_owned();
        let cli = cli(Agent::ClaudeCode, &dir, &[("STANDIN_HANG", &marker_text)]);
        let controls = Controls {
            deadline_ms: 300,
            ..controls()
        };
        let (driven, _) = run(&cli, &controls, false).await;
        assert_eq!(driven.report.status, Status::TimedOut);
        assert_eq!(driven.stops.len(), 1);
        assert!(driven.stops[0].cleanup.contains("empty"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_codex_session_that_only_reports_connection_errors_ends_as_a_transport_failure() {
        let dir = scratch("codex-offline");
        let cli = cli(Agent::Codex, &dir, &[("STANDIN_OFFLINE", "1")]);
        let recorder = Recorder::default();
        let wrap = Ok;
        let mut session = CliSession::new(&cli, cli.binary.clone().unwrap(), &wrap, "delegate-1");
        session.transport_bound = Duration::from_millis(400);
        let driven = session::drive(
            &mut session,
            &briefing(),
            &controls(),
            &recorder,
            &mut session::virtual_time(),
        )
        .await;
        session.shutdown().await;
        let Status::Transport { detail, reached } = &driven.report.status else {
            panic!("not a transport failure: {:?}", driven.report.status);
        };
        assert!(detail.contains("only connection errors"), "{detail}");
        assert!(detail.contains("waiting for network"), "{detail}");
        assert!(!reached);
        assert_eq!(driven.report.status.word(), "transport");
        assert!(
            driven.elapsed_ms < 10_000,
            "ended at {} ms",
            driven.elapsed_ms
        );
        assert!(driven.stops.is_empty(), "the host deadline didn't act");
        assert_eq!(
            delegate::charge(&driven.report),
            ("zero", "the executor never reached its provider")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn an_unanswered_native_steer_cannot_hold_the_host_past_its_deadline() {
        let dir = scratch("native-steer-timeout");
        let mut cli = cli(
            Agent::Codex,
            &dir,
            &[("STANDIN_STEER_SILENT", "1"), ("STANDIN_DELAY", "0.05")],
        );
        cli.granted = Some(Duration::from_millis(1500));
        let controls = Controls {
            deadline_ms: 1500,
            steer: Some(Steer {
                when: Trigger::CommandFailed,
                message: "Please steer".into(),
            }),
            ..controls()
        };
        let (driven, _) = tokio::time::timeout(Duration::from_secs(5), run(&cli, &controls, true))
            .await
            .expect("the host deadline must stop an unanswered steer");
        assert_eq!(driven.report.status, Status::TimedOut);
        assert_eq!(driven.stops.len(), 1);
        assert!(driven.stops[0].cleanup.contains("empty after cleanup"));
        assert_eq!(
            action(&driven, Capability::Steer).unwrap().outcome,
            "failed"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn shared_native_errors_keep_terminal_refusals_and_retry_times() {
        for (code, expected) in [
            ("401", "not_logged_in"),
            ("403", "model_unavailable"),
            ("429", "usage_limit"),
        ] {
            let dir = scratch("native-refusals");
            let cli = cli(Agent::OpenCode, &dir, &[("STANDIN_REFUSAL", code)]);
            let (driven, _) = run(&cli, &controls(), false).await;
            assert_eq!(
                driven.report.status,
                Status::Refused(expected.into()),
                "{:?}",
                driven.report
            );
            if code == "429" {
                assert_eq!(
                    driven
                        .report
                        .summary
                        .limit
                        .as_ref()
                        .unwrap()
                        .reset_source
                        .as_deref(),
                    Some("retry_after")
                );
            }
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[tokio::test]
    async fn terminal_launch_keeps_the_borrowed_host_wrapper_and_refuses_before_spawn() {
        use crate::session::Session;
        let dir = scratch("native-wrapper");
        let cli = cli(Agent::ClaudeCode, &dir, &[]);
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let wrap = |command: std::process::Command| {
            calls.set(calls.get() + 1);
            assert_eq!(command.get_current_dir(), Some(dir.as_path()));
            assert!(command.get_args().any(|arg| arg == "--input-format"));
            Err("fixture host refusal".into())
        };
        let mut session = CliSession::new(&cli, cli.binary.clone().unwrap(), &wrap, "delegate-1");
        assert!(
            session
                .start(&briefing())
                .await
                .unwrap_err()
                .contains("fixture host refusal")
        );
        session.shutdown().await;
        assert_eq!(calls.get(), 1);
        assert!(matches!(session.report().status, Status::Harness(_)));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn opencode_is_observed_as_it_runs_and_reports_its_cost() {
        let dir = scratch("opencode-observe");
        let cli = cli(Agent::OpenCode, &dir, &[("STANDIN_DELAY", "0.3")]);
        let (driven, _) = run(&cli, &controls(), false).await;
        assert_eq!(
            driven.report.status,
            Status::Answered,
            "{:?}",
            driven.report
        );
        assert_eq!(
            driven.session_id.as_deref(),
            Some("ses_0standin00000000000000001")
        );
        assert!(driven.events.iter().any(|event| matches!(
            event.kind,
            Kind::CommandCompleted {
                exit_code: Some(1),
                ..
            }
        )));
        assert_eq!(claims(&driven), vec!["heard briefing"]);
        let summary = &driven.report.summary;
        assert_eq!(summary.num_turns, Some(1));
        assert_eq!(summary.api_calls, Some(1));
        assert!((summary.total_cost_usd.unwrap() - 0.003).abs() < 1e-9);
        assert_eq!(summary.cost_provenance, Some("cli_list_price"));
        assert_eq!(summary.tokens("cache_read_input_tokens"), Some(4));
        assert_eq!(summary.result.as_deref(), Some("heard briefing"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A session with the host's hook bridge (#11509) installs it as the
    /// CLI's own `PreToolUse` command hook: Claude Code's `--settings`,
    /// Codex's `-c hooks.PreToolUse` without a trust prompt, and the
    /// host's socket in the environment.
    #[test]
    fn the_hook_bridge_is_each_clis_own_pretooluse_hook() {
        let dir = scratch("hook-bridge");
        let bridge = crate::delegate::HookBridge {
            program: PathBuf::from("/opt/coder one/coder-one"),
            args: vec!["coder-hook".into()],
            socket: dir.join("hook.sock"),
        };
        let mut claude = cli(Agent::ClaudeCode, &dir, &[]);
        claude.hooks = Some(bridge.clone());
        let settings: Value = serde_json::from_str(&bridge.claude_settings()).unwrap();
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "'/opt/coder one/coder-one' coder-hook --format claude"
        );
        let mut command = std::process::Command::new("claude");
        claude.configure_command(&mut command);
        assert!(
            command
                .get_envs()
                .any(|(name, value)| name == crate::delegate::HOOK_SOCKET_VAR
                    && value == Some(bridge.socket.as_os_str()))
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn opencode_refuses_a_steer_it_has_not_demonstrated_and_still_runs() {
        let dir = scratch("opencode-steer");
        let cli = cli(Agent::OpenCode, &dir, &[("STANDIN_DELAY", "0.3")]);
        let controls = Controls {
            steer: Some(Steer {
                when: Trigger::CommandFailed,
                message: "Please steer after the failed command.".to_string(),
            }),
            ..controls()
        };
        let (driven, _) = run(&cli, &controls, true).await;
        let steer = action(&driven, Capability::Steer).unwrap();
        assert_eq!(steer.outcome, "refused");
        assert_eq!(driven.report.status, Status::Answered);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_matrix_is_what_the_tests_demonstrate() {
        let (claude, _) = capabilities(Agent::ClaudeCode);
        assert!(Capability::ALL.iter().all(|c| claude.has(*c)));
        let (codex, note) = capabilities(Agent::Codex);
        assert!(codex.steer);
        assert!(codex.start && codex.observe && codex.stop && codex.resume);
        assert!(note.contains("app-server"));
    }

    #[test]
    fn each_adapter_reports_a_steering_mode_that_matches_what_it_demonstrated() {
        use crate::steering::{Native, Plan, Request, Turn};
        for agent in [
            Agent::ClaudeCode,
            Agent::Codex,
            Agent::Microluna,
            Agent::OpenCode,
        ] {
            let (demonstrated, _) = capabilities(agent);
            let steering = steering(agent);
            assert_eq!(steering.native == Native::MidTurn, demonstrated.steer);
            // Emulation stops and resumes, so it needs both.
            assert_eq!(
                steering.emulation.is_some(),
                demonstrated.stop && demonstrated.resume
            );
        }
        // Codex refuses a native steer, as its session does, and runs the
        // emulated one only when chosen.
        let codex = steering(Agent::Codex);
        assert_eq!(
            codex.admit(Turn::Running, Request::Native),
            Ok(Plan::Deliver)
        );
        assert_eq!(
            codex.admit(Turn::Running, Request::Emulated),
            Ok(Plan::CancelAndContinue)
        );
    }

    #[test]
    fn a_session_id_is_a_version_4_uuid() {
        let id = new_session_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(parts[2].starts_with('4'));
        assert!(matches!(
            parts[3].chars().next(),
            Some('8' | '9' | 'a' | 'b')
        ));
        assert_ne!(id, new_session_id());
    }

    #[test]
    fn a_long_stream_keeps_its_first_and_last_halves_as_keep_ends_does() {
        let dir = scratch("retain");
        let path = dir.join("stream.jsonl");
        let raw: String = (0..400).map(|i| format!("{{\"n\":{i}}}\n")).collect();
        let mut retainer = Retainer::create(&path, "artifacts/stream.jsonl", 1_000);
        for chunk in raw.as_bytes().chunks(37) {
            retainer.push(chunk);
        }
        let record = retainer.finish().unwrap();
        let kept = std::fs::read(&path).unwrap();
        assert_eq!(kept, delegate::keep_ends(raw.as_bytes(), 1_000));
        assert_eq!(record["truncated"], true);
        assert_eq!(record["bytes"].as_u64(), Some(raw.len() as u64));
        let _ = std::fs::remove_dir_all(dir);
    }
}
