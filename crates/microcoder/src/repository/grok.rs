//! A repository turn on a Grok Build route: Grok Build over ACP.
//!
//! Grok Build is a whole coding agent, so, as on a Devin route
//! ([`super::devin`]), the Microcoder step loop does not run: the host
//! starts `grok agent stdio` in the admitted workspace, opens a session
//! (or reattaches, with `session/load`, the session an earlier turn of the
//! same task used), and prompts it with the turn's message. The task owner
//! keeps its authority and evidence the same way:
//!
//! - **Effects**: starting the agent and each prompt are effect intents
//!   retained before dispatch, with their observations after.
//! - **Transcript**: the streamed reply, reasoning, and completed tool
//!   calls are appended to the ATIF transcript as they arrive, bounded.
//!   That transcript is the chat. This slice keeps no separate session store.
//! - **Model**: `grok:default` leaves Grok Build's own model. `grok:MODEL`
//!   passes `--model`, and the model the session reports must be it.
//! - **Login**: Grok Build uses its stored `auth.json` or `XAI_API_KEY`.
//!   The process gets no other variable named `*_API_KEY`, `*_TOKEN`, or
//!   `*_SECRET`.
//! - **Access**: full access passes `--always-approve`. Under the boundary
//!   and under toolchains that flag is omitted, and the host answers every
//!   permission request with the agent's reject option.
//! - **Cancellation**: a cancelled task, or one at its wall deadline, sends
//!   `session/cancel`, waits a grace, and stops the agent's process group.

use std::path::PathBuf;

use acp_client::{Opening, StopReason};
use atif::{Source, Step};
use coder::task::adapter::{Access, Host, Route as GrantRoute};
use serde_json::{Value, json};

use super::devin::{CANCEL_GRACE, Ended, Recorder, SILENCE, STOP_GRACE, Turn};

/// The step extension that names the Grok Build session a turn used, which
/// the next turn of the task reattaches.
pub const SESSION_NOTE: &str = "grok_session";
/// The engine a Grok Build turn records.
pub const ENGINE: &str = "grok-acp";

/// The Grok Build binary for this host, or why there is none.
pub(crate) fn binary() -> Result<PathBuf, String> {
    acp_client::grok::binary(&|name| std::env::var_os(name))
        .ok_or_else(|| "no grok binary in GROK_BIN, PATH, ~/.local/bin, or ~/.grok/bin".to_owned())
}

/// The Grok Build process's environment: the owner's login environment
/// under full access, else this process's, less every credential variable.
/// `XAI_API_KEY` is put back when that source had a non-empty value. That
/// variable is Grok Build's own login. Its value is never logged.
async fn grok_environment(host: &Host) -> Vec<(String, String)> {
    let (raw, grok_key) = match host.login_environment().await {
        Some(login) => (login.variables.clone(), login.grok_key().cloned()),
        None => (
            std::env::vars_os().collect(),
            std::env::var_os(acp_client::grok::API_KEY_VAR),
        ),
    };
    let mut variables: Vec<(String, String)> = raw
        .into_iter()
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(key, _)| !acp_client::process::is_credential_name(key))
        .collect();
    if let Some(value) = grok_key.filter(|value| !value.is_empty())
        && let Ok(value) = value.into_string()
    {
        variables.push((acp_client::grok::API_KEY_VAR.to_owned(), value));
    }
    variables
}

/// Run one turn on `route` with the Grok Build binary `program`.
pub(crate) async fn turn(host: &Host, route: &GrantRoute, program: PathBuf) -> Turn {
    let mut ended = Ended {
        engine: ENGINE,
        ..Ended::default()
    };
    if let Err(why) = acp_client::grok::parse_model(&route.model) {
        ended.error = Some(why);
        return Turn::Ended(ended);
    }
    let access = host.configuration().access;
    let approve = access == Access::Full;
    let arguments = acp_client::grok::arguments(&route.model, approve);
    let resume = host.earlier_note(SESSION_NOTE).and_then(|note| {
        note.get("session")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let opening = Opening {
        spec: acp_client::process::Spec {
            program: program.clone(),
            arguments: arguments.clone(),
            cwd: host.workspace().to_path_buf(),
            environment: grok_environment(host).await,
        },
        resume: resume.clone(),
        meta: Some(acp_client::devin::engine_meta(coder_history::engine::MARK)),
        mode: None,
    };
    let sequence = match host.effect(
        "grok_session",
        json!({"program": program, "arguments": arguments, "cwd": host.workspace(),
            "approve": approve, "resume": resume, "model": route.model}),
    ) {
        Ok(sequence) => sequence,
        Err(error) => {
            ended.error = Some(error.to_string());
            return Turn::Ended(ended);
        }
    };
    let cancelled = || host.cancelled();
    let mut session = match acp_client::Session::open(&opening, &cancelled).await {
        Ok(session) => session,
        Err(failure) => {
            let why = failure.to_string();
            let _ = host.result(sequence, "grok_session", json!({"error": why}));
            ended.error = Some(why);
            return Turn::Ended(ended);
        }
    };
    let reported = session.opened.model().map(str::to_owned);
    ended.session = Some(session.id().to_owned());
    ended.resumed = session.resumed;
    ended.model.clone_from(&reported);
    let observed = json!({"session": session.id(), "pid": session.pid(), "resumed": session.resumed,
        "resume_refused": session.resume_refused, "model": reported,
        "agent": session.initialized.agent_info});
    if let Err(error) = host.result(sequence, "grok_session", observed) {
        ended.error = Some(error.to_string());
        session.close(STOP_GRACE).await;
        return Turn::Ended(ended);
    }
    if let Err(error) = host.append(
        &Step::said(Source::System, "The Grok Build session this turn runs in.").noting(
            SESSION_NOTE,
            json!({"session": session.id(), "model": reported, "resumed": session.resumed}),
        ),
    ) {
        ended.error = Some(error.to_string());
        session.close(STOP_GRACE).await;
        return Turn::Ended(ended);
    }
    if !acp_client::grok::admits(&route.model, reported.as_deref()) {
        host.fail("Grok Build reported a model different from the admitted model");
        ended.error = Some(format!(
            "Requested {}, Grok Build reported {}; refusing the turn.",
            route.model,
            reported.as_deref().unwrap_or("no model")
        ));
        session.close(STOP_GRACE).await;
        return Turn::Ended(ended);
    }
    // A reattached session remembers the conversation; a new one is told it.
    let prompt = if session.resumed {
        host.prompt().to_owned()
    } else {
        host.engine_prompt()
    };
    let prompted = match host.effect(
        "grok_prompt",
        json!({"session": session.id(), "prompt": prompt, "model": route.model}),
    ) {
        Ok(sequence) => sequence,
        Err(error) => {
            ended.error = Some(error.to_string());
            session.close(STOP_GRACE).await;
            return Turn::Ended(ended);
        }
    };
    let mut recorder = Recorder::new(host, "Grok Build", "grok", route.model.clone(), access);
    let silence = SILENCE.min(std::time::Duration::from_secs(host.wall_seconds().max(1)));
    let result = session
        .prompt(&prompt, silence, &cancelled, CANCEL_GRACE, &mut recorder)
        .await;
    recorder.close();
    ended.reply = std::mem::take(&mut recorder.reply);
    ended.tool_calls = recorder.tool_calls;
    ended.cost_usd = recorder.cost_usd;
    ended.input_tokens = recorder.input_tokens;
    ended.output_tokens = recorder.output_tokens;
    let stderr = session.stderr_tail();
    let group_clear = session.close(STOP_GRACE).await;
    if !group_clear {
        host.fail("the Grok Build process group did not stop");
    }
    match &result {
        Ok(reply) => {
            ended.stop = Some(reply.stop_reason);
            if let Some(usage) = reply.usage {
                ended.input_tokens = usage.input_tokens.unwrap_or_default();
                ended.output_tokens = usage.output_tokens.unwrap_or_default();
            }
        }
        Err(error) => {
            ended.error = Some(error.to_string());
        }
    }
    let observation = json!({"stop_reason": ended.stop.map(StopReason::as_str),
        "error": ended.error, "group_clear": group_clear,
        "input_tokens": ended.input_tokens, "output_tokens": ended.output_tokens,
        "tool_calls": ended.tool_calls, "cost_usd": ended.cost_usd,
        "stderr_tail": if ended.error.is_some() { json!(stderr) } else { Value::Null },
        "billing": "unknown"});
    if let Err(error) = host.result(prompted, "grok_prompt", observation) {
        ended.error = Some(error.to_string());
    }
    Turn::Ended(ended)
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::tests::fixture_with;
    use super::super::{AgentEngine, Stage, run_stages};
    use super::*;
    use acp_client::replay;
    use coder::task::adapter::Configuration;
    use coder::task::{self, Action, Command, Store};

    /// The model the synthetic turn reports.
    const MODEL: &str = "grok-4.6";
    const SESSION: &str = "grok-session-1";

    fn grok_route(configuration: &mut Configuration, access: Access, model: &str) {
        configuration.provider = "grok".into();
        configuration.model = model.into();
        configuration.effort = None;
        configuration.generation_endpoint = coder::task::capacity::GROK_ENDPOINT.into();
        configuration.decision_endpoint = "https://decision.example.invalid".into();
        configuration.access = access;
    }

    fn jev() -> jev::Client {
        jev::Client::new(
            jev::Config::default()
                .api_key("unused-fixture-key")
                .base_url("https://decision.example.invalid")
                .default_model("fixture-judge"),
        )
        .unwrap()
    }

    fn route(model: &str) -> GrantRoute {
        GrantRoute {
            provider: "grok".into(),
            model: model.into(),
            effort: None,
            generation_endpoint: coder::task::capacity::GROK_ENDPOINT.into(),
        }
    }

    async fn run_turn(
        store: &std::path::Path,
        grant: &[u8],
        model: &str,
        agent: PathBuf,
    ) -> task::Task {
        let host = Host::admit(store, grant).await.unwrap();
        let stages: Vec<Stage<codex_transport::codex::CodexTransport>> =
            vec![Stage::Agent(AgentEngine::Grok, route(model), agent)];
        run_stages(
            host,
            store.to_path_buf(),
            stages,
            Ok(jev()),
            "fixture-session",
            &[],
        )
        .await
        .unwrap()
    }

    /// The recorded agent, with its environment written beside it.
    fn agent(dir: &std::path::Path, blocks: &[Vec<Value>]) -> PathBuf {
        let script = replay::script(dir, blocks);
        let body = std::fs::read_to_string(&script).unwrap();
        let body = body.replacen(
            "#!/bin/sh\n",
            &format!("#!/bin/sh\nenv > '{}/environment'\n", dir.display()),
            1,
        );
        std::fs::write(&script, body).unwrap();
        script
    }

    /// Credential variable names in the stand-in's environment, other than
    /// Grok Build's own `XAI_API_KEY`. Values are not returned.
    fn leaked_credential_names(dir: &std::path::Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("environment"))
            .unwrap()
            .lines()
            .filter_map(|line| line.split_once('=').map(|(name, _)| name.to_owned()))
            .filter(|name| {
                acp_client::process::is_credential_name(name)
                    && name != acp_client::grok::API_KEY_VAR
            })
            .collect()
    }

    #[tokio::test]
    async fn a_grok_route_runs_the_recorded_turn_under_full_access() {
        let (_root, store, grant) = fixture_with(acp_client::grok::DEFAULT_MODEL, |c| {
            grok_route(c, Access::Full, acp_client::grok::DEFAULT_MODEL)
        });
        let agent_dir = tempfile::tempdir().unwrap();
        let agent = agent(agent_dir.path(), &replay::blocks(replay::GROK_TURN));
        let task = run_turn(&store, &grant, acp_client::grok::DEFAULT_MODEL, agent).await;
        assert_eq!(task.execution, task::Execution::Finished);
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_eq!(result.ending, "model_finished");
        assert_eq!(result.exit_code, Some(0));
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        for expected in [
            "read note.txt",
            &format!("\"session\":\"{SESSION}\""),
            "\"kind\":\"grok_prompt\"",
            "\"grok_tool\"",
            "\"input_tokens\":120",
        ] {
            assert!(trace.contains(expected), "missing {expected}");
        }
        assert_eq!(
            replay::arguments(agent_dir.path()),
            vec!["agent", "--always-approve", "--no-leader", "stdio"]
        );
        let sent = replay::received(agent_dir.path());
        assert_eq!(sent[1]["method"], "session/new");
        assert_eq!(
            sent[1]["params"]["_meta"][acp_client::devin::ENGINE_META_KEY],
            coder_history::engine::MARK
        );
        assert_eq!(sent[2]["method"], "session/prompt");
        assert!(
            sent[2]["params"]["prompt"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Write result.txt containing output.")
        );
        assert!(leaked_credential_names(agent_dir.path()).is_empty());
    }

    #[tokio::test]
    async fn a_named_model_is_passed_and_must_match() {
        let (_root, store, grant) = fixture_with(MODEL, |c| grok_route(c, Access::Full, MODEL));
        let agent_dir = tempfile::tempdir().unwrap();
        let agent = agent(agent_dir.path(), &replay::blocks(replay::GROK_TURN));
        let task = run_turn(&store, &grant, MODEL, agent).await;
        assert_eq!(
            task.run.as_ref().unwrap().result.as_ref().unwrap().ending,
            "model_finished"
        );
        assert_eq!(
            replay::arguments(agent_dir.path()),
            vec![
                "agent",
                "--always-approve",
                "--model",
                MODEL,
                "--no-leader",
                "stdio"
            ]
        );
    }

    #[tokio::test]
    async fn the_boundary_omits_always_approve_and_refuses_what_it_asks() {
        let (_root, store, grant) = fixture_with(acp_client::grok::DEFAULT_MODEL, |c| {
            grok_route(c, Access::Boundary, acp_client::grok::DEFAULT_MODEL)
        });
        let agent_dir = tempfile::tempdir().unwrap();
        let mut blocks = replay::blocks(replay::GROK_TURN);
        let ask = json!({"jsonrpc":"2.0","id":"ask-1","method":"session/request_permission",
            "params":{"sessionId":SESSION,"toolCall":{"toolCallId":"tool-ask","kind":"execute","title":"bash"},
            "options":[{"optionId":"allow","kind":"allow_once","name":"Allow"},
                {"optionId":"reject","kind":"reject_once","name":"Reject"}]}});
        blocks[2].insert(0, ask);
        let agent = agent(agent_dir.path(), &blocks);
        let task = run_turn(&store, &grant, acp_client::grok::DEFAULT_MODEL, agent).await;
        assert_eq!(
            task.run.as_ref().unwrap().result.as_ref().unwrap().ending,
            "model_finished"
        );
        assert_eq!(
            replay::arguments(agent_dir.path()),
            vec!["agent", "--no-leader", "stdio"]
        );
        let sent = replay::received(agent_dir.path());
        let answer = sent
            .iter()
            .find(|line| line["id"] == "ask-1")
            .expect("the permission answer");
        assert_eq!(answer["result"]["outcome"]["optionId"], "reject");
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(trace.contains("grok_permission"));
        let leaked = leaked_credential_names(agent_dir.path());
        assert!(leaked.is_empty(), "{leaked:?}");
    }

    #[tokio::test]
    async fn a_follow_up_reattaches_the_same_grok_session() {
        let (_root, store, grant) = fixture_with(acp_client::grok::DEFAULT_MODEL, |c| {
            grok_route(c, Access::Full, acp_client::grok::DEFAULT_MODEL)
        });
        let first_dir = tempfile::tempdir().unwrap();
        let first = agent(first_dir.path(), &replay::blocks(replay::GROK_TURN));
        let ended = run_turn(&store, &grant, acp_client::grok::DEFAULT_MODEL, first).await;
        let follow_up = Command {
            schema: task::COMMAND_SCHEMA.into(),
            command_id: "follow-up-fixture".into(),
            task_id: "fixture".into(),
            expected_revision: Some(ended.revision),
            action: Action::Continue {
                prompt: "Now delete note.txt.".into(),
            },
        };
        let receipt = Store::open(&store)
            .unwrap()
            .apply(&serde_json::to_vec(&follow_up).unwrap())
            .unwrap();
        let mut next: task::owner::Grant = serde_json::from_slice(&grant).unwrap();
        next.expected_revision = receipt.revision;
        let mut blocks = replay::blocks(replay::GROK_TURN);
        let mut loaded = blocks[1].last().unwrap().clone();
        loaded["result"]
            .as_object_mut()
            .unwrap()
            .remove("sessionId");
        blocks[1] = vec![loaded];
        let second_dir = tempfile::tempdir().unwrap();
        let second = agent(second_dir.path(), &blocks);
        let task = run_turn(
            &store,
            &serde_json::to_vec(&next).unwrap(),
            acp_client::grok::DEFAULT_MODEL,
            second,
        )
        .await;
        assert_eq!(task.turn(), 2);
        let sent = replay::received(second_dir.path());
        assert_eq!(sent[1]["method"], "session/load");
        assert_eq!(sent[1]["params"]["sessionId"], SESSION);
        assert_eq!(
            sent[2]["params"]["prompt"][0]["text"],
            "Now delete note.txt."
        );
        let trace = std::fs::read_to_string(store.join("fixture.2.atif.jsonl")).unwrap();
        assert!(trace.contains("\"resumed\":true"));
    }

    #[tokio::test]
    async fn a_model_other_than_the_admitted_one_is_refused() {
        let admitted = "grok-4.5";
        let (_root, store, grant) =
            fixture_with(admitted, |c| grok_route(c, Access::Full, admitted));
        let agent_dir = tempfile::tempdir().unwrap();
        let agent = agent(agent_dir.path(), &replay::blocks(replay::GROK_TURN));
        let task = run_turn(&store, &grant, admitted, agent).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_eq!(result.ending, "cancelled_or_host_refusal");
        assert_ne!(result.exit_code, Some(0));
        let sent = replay::received(agent_dir.path());
        assert!(sent.iter().all(|line| line["method"] != "session/prompt"));
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(trace.contains("refusing the turn"));
    }

    /// The live smoke: the installed Grok Build CLI, with the owner's login,
    /// runs the fixture's turn under full access. The task store is temporary.
    /// Run with `cargo test -p microcoder --lib live_grok -- --ignored`.
    #[tokio::test]
    #[ignore = "runs the installed Grok Build CLI with the owner's login and spends a model request"]
    async fn live_grok_cli_runs_a_repository_turn() {
        let agent = binary().expect("grok binary");
        let (root, store, grant) = fixture_with(acp_client::grok::DEFAULT_MODEL, |c| {
            grok_route(c, Access::Full, acp_client::grok::DEFAULT_MODEL)
        });
        // A real turn takes longer than the fixture's eight seconds.
        let mut grant: task::owner::Grant = serde_json::from_slice(&grant).unwrap();
        grant.wall_seconds = 300;
        let grant = serde_json::to_vec(&grant).unwrap();
        let task = run_turn(&store, &grant, acp_client::grok::DEFAULT_MODEL, agent).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert_eq!(result.ending, "model_finished", "{result:?}\n{trace}");
        let written = std::fs::read_to_string(root.path().join("checkout/result.txt")).unwrap();
        assert!(written.contains("output"), "{written}");
        assert!(trace.contains("\"kind\":\"grok_prompt\""));
    }

    #[test]
    fn a_grok_route_is_closed() {
        let (_root, _store, grant) = fixture_with(acp_client::grok::DEFAULT_MODEL, |c| {
            grok_route(c, Access::Full, acp_client::grok::DEFAULT_MODEL)
        });
        let grant = task::owner::Grant::parse(&grant).unwrap();
        let configuration = grant.adapter_configuration.unwrap();
        configuration.validate().unwrap();
        let mut effort = configuration.clone();
        effort.effort = Some("medium".into());
        assert!(effort.validate().is_err());
        let mut endpoint = configuration.clone();
        endpoint.generation_endpoint = "https://api.x.ai".into();
        assert!(endpoint.validate().is_err());
        let mut slashed = configuration.clone();
        slashed.model = "grok/4".into();
        assert!(slashed.validate().is_err());
        assert_eq!(
            configuration.capabilities()["steering"]["adapter"],
            "grok-acp"
        );
        assert_eq!(
            configuration.capabilities()["cost_reporting"],
            "provider-reported-tokens"
        );
    }
}
