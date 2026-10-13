//! The Agent Client Protocol as Coder speaks it to a local coding agent.
//!
//! ACP is newline-delimited JSON-RPC 2.0 over an agent's standard input and
//! output, at protocol version 1. This crate is what Coder's adapters share
//! for an agent that speaks it:
//!
//! - [`wire`]: the method names, frame classification, and typed payloads:
//!   `initialize`, `session/new`, `session/prompt`, every `session/update`
//!   this crate renders, usage (including Devin's `cognition.ai/*` keys),
//!   and permission requests.
//! - [`client`]: one request at a time over an agent's streams, with the
//!   agent's updates, permission requests, and reverse requests handled on
//!   the way past, a silence limit, and cancellation that sends
//!   `session/cancel` and waits out a grace.
//! - [`process`]: the agent as a process group of its own, started with an
//!   explicit environment and stopped with an acknowledgment that the group
//!   is empty.
//! - [`session`]: start, hand-shake, open or reattach, set the mode, prompt,
//!   and stop.
//! - [`cursor`]: the Cursor CLI's specifics (`cursor-agent acp`).
//! - [`devin`]: the Devin CLI's specifics (`devin acp`).
//! - [`opencode`]: OpenCode's specifics (`opencode acp`).
//! - [`grok`]: Grok Build's specifics (`grok agent stdio`).
//! - [`replay`]: a stand-in agent that replays a recorded conversation, for
//!   tests.
//!
//! The design is carried over from the ACP client in the owner's earlier
//! Coder repository and reimplemented here with typed payloads.

pub mod client;
pub mod cursor;
pub mod devin;
pub mod grok;
pub mod opencode;
pub mod process;
pub mod replay;
pub mod session;
pub mod wire;

pub use client::{Client, ClientError, Handler, OnCancel, PendingAnswer, Wait};
pub use session::{Failure, Opening, Session};
pub use wire::{PermissionAnswer, PermissionRequest, StopReason, Update, Usage};

#[cfg(test)]
// The session cases run a `/bin/sh` stand-in, so on Windows some helpers go
// unused.
#[cfg_attr(windows, allow(unused_imports, dead_code))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[derive(Default)]
    struct Kept {
        text: String,
        tools: Vec<String>,
        usage: Vec<Usage>,
        stats: Vec<devin::TurnStats>,
    }

    impl Handler for Kept {
        fn update(&mut self, update: Update) {
            match update {
                Update::AgentText(text) => self.text.push_str(&text),
                Update::ToolCall { title, .. } => self.tools.push(title),
                Update::Usage(usage) => self.usage.push(usage),
                _ => {}
            }
        }
        fn notification(&mut self, method: &str, params: &serde_json::Value) {
            if method == devin::TURN_STATS {
                self.stats.extend(devin::TurnStats::parse(params));
            }
        }
    }

    fn opening(program: PathBuf, cwd: PathBuf) -> Opening {
        Opening {
            spec: process::Spec {
                program,
                arguments: devin::arguments(devin::DEFAULT_MODEL),
                cwd,
                environment: vec![("PATH".into(), "/bin:/usr/bin".into())],
            },
            resume: None,
            meta: Some(devin::engine_meta("openagents-coder-engine")),
            mode: Some(devin::Permission::Bypass.mode_id().into()),
            authenticate: None,
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn the_recorded_devin_turn_replays_through_a_session() {
        let dir = tempfile::tempdir().unwrap();
        let agent = replay::script(dir.path(), &replay::blocks(replay::DEVIN_TURN));
        let mut session = Session::open(&opening(agent, dir.path().into()), &|| false)
            .await
            .unwrap();
        assert_eq!(session.id(), "cheddar-cashew");
        assert_eq!(session.opened.model(), Some("swe-2-high"));
        assert!(session.initialized.agent_capabilities.load_session);
        let mut kept = Kept::default();
        let reply = session
            .prompt(
                "Create hello.txt, then run ls.",
                Duration::from_secs(5),
                &|| false,
                Duration::from_secs(1),
                &mut kept,
            )
            .await
            .unwrap();
        assert_eq!(reply.stop_reason, StopReason::EndTurn);
        assert_eq!(reply.usage.unwrap().output_tokens, Some(33));
        assert!(kept.text.starts_with("Created `hello.txt`"));
        assert_eq!(kept.tools, vec!["Ran echo, ls"]);
        assert_eq!(kept.usage.iter().filter(|u| !u.subagent).count(), 2);
        assert_eq!(kept.stats[0].number("output_tokens"), Some(95.0));
        assert!(session.close(Duration::from_millis(500)).await);
        let sent = replay::received(dir.path());
        assert_eq!(sent[1]["method"], "session/new");
        assert_eq!(
            sent[1]["params"]["_meta"][devin::ENGINE_META_KEY],
            "openagents-coder-engine"
        );
        assert_eq!(sent[2]["params"]["modeId"], "bypass");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_cancelled_turn_sends_session_cancel_and_ends_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let mut blocks = replay::blocks(replay::DEVIN_TURN);
        // The prompt's block, without its reply: the turn stays open.
        let prompt = blocks.pop().unwrap();
        blocks.push(prompt[..prompt.len() - 1].to_vec());
        let agent = replay::script(dir.path(), &blocks);
        let mut session = Session::open(&opening(agent, dir.path().into()), &|| false)
            .await
            .unwrap();
        let started = std::time::Instant::now();
        let reply = session
            .prompt(
                "Run forever.",
                Duration::from_secs(30),
                &|| started.elapsed() > Duration::from_millis(300),
                Duration::from_secs(5),
                &mut Kept::default(),
            )
            .await
            .unwrap();
        assert_eq!(reply.stop_reason, StopReason::Cancelled);
        assert!(session.close(Duration::from_millis(500)).await);
        let sent = replay::received(dir.path());
        assert_eq!(sent.last().unwrap()["method"], "session/cancel");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_refused_reattachment_falls_back_to_a_new_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut blocks = replay::blocks(replay::DEVIN_TURN);
        let refusal = serde_json::json!({"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"session not found"}});
        blocks.insert(1, vec![refusal]);
        // The recorded session/new and set_mode replies now answer requests
        // 3 and 4.
        for (index, block) in blocks.iter_mut().enumerate().skip(2) {
            if let Some(last) = block.last_mut() {
                last["id"] = serde_json::json!(index + 1);
            }
        }
        let agent = replay::script(dir.path(), &blocks);
        let mut opening = opening(agent, dir.path().into());
        opening.resume = Some("gone-session".into());
        let session = Session::open(&opening, &|| false).await.unwrap();
        assert!(!session.resumed);
        assert_eq!(session.resume_refused.as_deref(), Some("session not found"));
        assert_eq!(session.id(), "cheddar-cashew");
        session.close(Duration::from_millis(500)).await;
        let sent = replay::received(dir.path());
        assert_eq!(sent[1]["method"], "session/load");
        assert_eq!(sent[1]["params"]["sessionId"], "gone-session");
    }
    /// How each gated agent is started so its tools reach Coder's hooks
    /// first (#11508).
    #[test]
    fn the_gated_agents_are_started_with_coders_hook_or_asking() {
        let mut grok = grok::arguments(grok::DEFAULT_MODEL, false);
        grok::add_plugin_dir(&mut grok, std::path::Path::new("/t/plugin"));
        assert_eq!(
            grok,
            ["agent", "--no-leader", "--plugin-dir", "/t/plugin", "stdio"]
        );
        let plugin = grok::hook_plugin("coder coder-hook --format acp", 600);
        assert_eq!(
            plugin["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "coder coder-hook --format acp"
        );
        let dir = tempfile::tempdir().unwrap();
        grok::write_hook_plugin(dir.path(), "x", 5).unwrap();
        assert!(dir.path().join("hooks/hooks.json").is_file());

        let mut devin = devin::arguments(devin::DEFAULT_MODEL);
        devin::add_config(&mut devin, std::path::Path::new("/t/devin.json"));
        assert_eq!(devin, ["--config", "/t/devin.json", "acp"]);
        // The person's settings stay, and their own hooks run too.
        let person = br#"{"devin":{"org_id":"o"},"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"mine"}]}]}}"#;
        let config = devin::hooked_config(Some(person), "coder", 600);
        assert_eq!(config["devin"]["org_id"], "o");
        assert_eq!(config["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        assert_eq!(
            devin::hooked_config(Some(b"not json"), "c", 1)["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "c"
        );
        let home = |name: &str| (name == "HOME").then(|| std::ffi::OsString::from("/h"));
        assert_eq!(
            devin::config_path(&home),
            Some(PathBuf::from("/h/.config/devin/config.json"))
        );

        let asking: serde_json::Value = serde_json::from_str(&opencode::asking_config(Some(
            r#"{"model":"opencode/big-pickle","permission":"allow"}"#,
        )))
        .unwrap();
        assert_eq!(asking["model"], "opencode/big-pickle");
        assert_eq!(asking["permission"]["*"], "ask");
        assert_eq!(asking["permission"]["question"], "deny");
    }

    /// A handler that answers later: the request waits for its answer.
    #[tokio::test]
    async fn a_permission_answered_later_is_awaited() {
        struct Later;
        impl Handler for Later {
            fn update(&mut self, _update: Update) {}
            fn permission_later(&mut self, request: &PermissionRequest) -> Option<PendingAnswer> {
                let reject = request.reject().unwrap_or_default().to_owned();
                Some(Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    PermissionAnswer::Selected(reject)
                }))
            }
        }
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (client_in, mut agent_out) = tokio::io::duplex(8192);
        let (agent_in, client_out) = tokio::io::duplex(8192);
        let agent = tokio::spawn(async move {
            let mut lines = BufReader::new(agent_in).lines();
            let request: serde_json::Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let ask = wire::request(
                9,
                wire::method::REQUEST_PERMISSION,
                &serde_json::json!({"sessionId":"s","toolCall":{"kind":"execute"},"options":[{"optionId":"y","kind":"allow_once"},{"optionId":"n","kind":"reject_once"}]}),
            );
            agent_out
                .write_all(format!("{ask}\n").as_bytes())
                .await
                .unwrap();
            let answer: serde_json::Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let reply = wire::reply(
                &request["id"],
                Ok(serde_json::json!({"stopReason":"end_turn"})),
            );
            agent_out
                .write_all(format!("{reply}\n").as_bytes())
                .await
                .unwrap();
            answer
        });
        let mut client = Client::new(client_in, client_out);
        client
            .prompt("s", "go", Wait::plain(Duration::from_secs(5)), &mut Later)
            .await
            .unwrap();
        assert_eq!(agent.await.unwrap()["result"]["outcome"]["optionId"], "n");
    }
}
