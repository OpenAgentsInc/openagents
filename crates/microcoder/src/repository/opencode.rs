//! A repository turn on an OpenCode route: OpenCode over ACP.
//!
//! OpenCode is a whole coding agent, so, as on a Devin route ([`super::devin`]),
//! the Microcoder step loop does not run: the host starts `opencode acp` in
//! the admitted workspace, opens a session (or reattaches, with
//! `session/load`, the OpenCode session an earlier turn of the same task
//! used), and prompts it with the turn's message. The task owner keeps its
//! authority and evidence the same way:
//!
//! - **Effects**: starting the agent and each prompt are effect intents
//!   retained before dispatch, with their observations after.
//! - **Transcript**: OpenCode's streamed reply, reasoning, and completed
//!   tool calls are appended to the ATIF transcript as they arrive, bounded.
//! - **Model**: the route names OpenCode's own `provider/model`
//!   ([`acp_client::opencode::Model`]); it goes in OpenCode's inline
//!   configuration, and the model the session reports must be it.
//! - **Logins**: OpenCode reaches the provider with its own stored logins
//!   (`auth.json`) or the owner's configured providers. The process gets
//!   no variable named `*_API_KEY`, `*_TOKEN`, or `*_SECRET`.
//! - **Access**: full access allows every OpenCode tool
//!   ([`acp_client::opencode::Permission::Full`]). Under the boundary,
//!   OpenCode may read, search, and edit inside the workspace; every other
//!   tool asks ([`acp_client::opencode::Permission::Edits`]), and the host
//!   answers every ask with OpenCode's own reject option, so it runs no
//!   command and reaches no network through its tools.
//! - **Sessions**: an engine session is saved in the engine's own OpenCode
//!   database (`OPENCODE_DB`, [`coder_history::engine::OPENCODE_DATABASE`]),
//!   never the owner's `opencode.db`. When the turn ends the session is
//!   copied beside the task ([`coder_history::delegate`]), so the phone
//!   reads it inside the Coder chat.
//! - **Cancellation**: a cancelled task, or one at its wall deadline, sends
//!   `session/cancel`, waits a grace, and stops the agent's process group.
//! - **Capacity**: OpenCode's ACP refusal names only its error
//!   (`APIError`); the host reads the failed message's HTTP status and
//!   retry headers from the engine's database, and a 429 before any work is
//!   a rate-limit refusal for the capacity book
//!   ([`coder::task::capacity::Refusal::opencode`]), so the run fails over
//!   to the next admitted route.
//!
//! Cost is OpenCode's own list-price figure from its `usage_update`; tokens
//! are the `session/prompt` reply's totals.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use acp_client::opencode::{Model, Permission, RefusalData};
use acp_client::{ClientError, Opening, StopReason};
use atif::{Source, Step};
use coder::task::adapter::{Access, Host, Route as GrantRoute};
use coder::task::capacity::Refusal;
use serde_json::{Value, json};

use super::devin::{CANCEL_GRACE, Ended, Recorder, SILENCE, STOP_GRACE, Turn, environment};

/// The step extension that names the OpenCode session a turn used, which
/// the next turn of the task reattaches.
pub const SESSION_NOTE: &str = "opencode_session";
/// The engine an OpenCode turn records.
pub const ENGINE: &str = "opencode-acp";

/// The OpenCode binary for this host, or why there is none.
pub(crate) fn binary() -> Result<PathBuf, String> {
    acp_client::opencode::binary(&|name| std::env::var_os(name)).ok_or_else(|| {
        "no opencode binary in OPENCODE_BIN, PATH, ~/.opencode/bin, or ~/.local/bin".to_owned()
    })
}

/// The OpenCode process's environment: the owner's (see
/// [`environment`]), less any inline configuration or database of the
/// owner's, plus the engine's.
async fn opencode_environment(
    host: &Host,
    model: &Model,
    permission: Permission,
) -> Vec<(String, String)> {
    let mut variables = environment(host).await;
    variables.retain(|(key, _)| {
        key != acp_client::opencode::CONFIG_VAR && key != acp_client::opencode::DATABASE_VAR
    });
    variables.extend(acp_client::opencode::environment(
        model,
        permission,
        Path::new(coder_history::engine::OPENCODE_DATABASE),
    ));
    variables
}

/// The engine's OpenCode database, as OpenCode resolves the relative name
/// in its data directory under `variables`.
fn engine_database(variables: &[(String, String)]) -> Option<PathBuf> {
    let lookup = |name: &str| {
        variables
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| OsString::from(value))
    };
    acp_client::opencode::data_dir(&lookup)
        .map(|data| coder_history::engine::opencode_database(&data))
}

/// Run one turn on `route` with the OpenCode binary `program`.
pub(crate) async fn turn(host: &Host, route: &GrantRoute, program: PathBuf) -> Turn {
    let mut ended = Ended {
        engine: ENGINE,
        agent: "OpenCode",
        ..Ended::default()
    };
    let model = match Model::parse(&route.model) {
        Ok(model) => model,
        Err(why) => {
            ended.error = Some(why);
            return Turn::Ended(ended);
        }
    };
    let access = host.configuration().access;
    let permission = match access {
        Access::Full => Permission::Full,
        Access::Boundary | Access::Toolchains => Permission::Edits,
    };
    let variables = opencode_environment(host, &model, permission).await;
    let database = engine_database(&variables);
    let arguments = acp_client::opencode::arguments();
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
            environment: variables,
        },
        resume: resume.clone(),
        meta: Some(acp_client::devin::engine_meta(coder_history::engine::MARK)),
        mode: None,
    };
    let sequence = match host.effect(
        "opencode_session",
        json!({"program": program, "arguments": arguments, "cwd": host.workspace(),
            "permission": permission.config(), "resume": resume, "model": route.model,
            "database": coder_history::engine::OPENCODE_DATABASE}),
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
            let _ = host.result(sequence, "opencode_session", json!({"error": why}));
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
    if let Err(error) = host.result(sequence, "opencode_session", observed) {
        ended.error = Some(error.to_string());
        session.close(STOP_GRACE).await;
        return Turn::Ended(ended);
    }
    if let Err(error) = host.append(
        &Step::said(Source::System, "The OpenCode session this turn runs in.").noting(
            SESSION_NOTE,
            json!({"session": session.id(), "model": reported, "resumed": session.resumed}),
        ),
    ) {
        ended.error = Some(error.to_string());
        session.close(STOP_GRACE).await;
        return Turn::Ended(ended);
    }
    if !acp_client::opencode::admits(&model, reported.as_deref()) {
        host.fail("OpenCode reported a model different from the admitted model");
        ended.error = Some(format!(
            "Requested {model}, OpenCode reported {}; refusing the turn.",
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
        "opencode_prompt",
        json!({"session": session.id(), "prompt": prompt, "model": route.model}),
    ) {
        Ok(sequence) => sequence,
        Err(error) => {
            ended.error = Some(error.to_string());
            session.close(STOP_GRACE).await;
            return Turn::Ended(ended);
        }
    };
    let mut recorder = Recorder::new(host, "OpenCode", "opencode", route.model.clone(), access);
    let silence = SILENCE.min(std::time::Duration::from_secs(host.wall_seconds().max(1)));
    let result = session
        .prompt(&prompt, silence, &cancelled, CANCEL_GRACE, &mut recorder)
        .await;
    recorder.close();
    ended.reply = std::mem::take(&mut recorder.reply);
    ended.refused = recorder.refused.take();
    ended.tool_calls = recorder.tool_calls;
    ended.cost_usd = recorder.cost_usd;
    let stderr = session.stderr_tail();
    let session_id = session.id().to_owned();
    let group_clear = session.close(STOP_GRACE).await;
    if !group_clear {
        host.fail("the OpenCode process group did not stop");
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    super::devin::keep_delegate(
        host,
        coder_history::Harness::OpenCode,
        &session_id,
        database.as_deref(),
        coder_history::opencode::delegate,
    );
    let refusal = match &result {
        Ok(reply) => {
            ended.stop = Some(reply.stop_reason);
            if let Some(usage) = reply.usage {
                ended.input_tokens = usage.input_tokens.unwrap_or_default();
                ended.output_tokens = usage.output_tokens.unwrap_or_default();
            }
            None
        }
        Err(ClientError::Refused { error, .. }) => {
            // Only a provider's refusal before any work fails over; its
            // status and retry headers are on the failed message.
            let refusal = (RefusalData::parse(error.data.as_ref()).api()
                && ended.tool_calls == 0
                && ended.reply.is_empty())
            .then(|| last_error(database.as_deref(), &session_id))
            .flatten()
            .and_then(|saved| Refusal::opencode(&saved, coder::task::autostart::unix_now()));
            if refusal.is_none() {
                ended.error = Some(
                    result
                        .as_ref()
                        .err()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                );
            }
            refusal
        }
        Err(error) => {
            ended.error = Some(error.to_string());
            None
        }
    };
    let observation = json!({"stop_reason": ended.stop.map(StopReason::as_str),
        "error": ended.error, "refusal": refusal, "group_clear": group_clear,
        "input_tokens": ended.input_tokens, "output_tokens": ended.output_tokens,
        "tool_calls": ended.tool_calls, "cost_usd": ended.cost_usd,
        "stderr_tail": if ended.error.is_some() { json!(stderr) } else { Value::Null },
        "billing": "unknown"});
    if let Err(error) = host.result(prompted, "opencode_prompt", observation) {
        ended.error = Some(error.to_string());
    }
    match refusal {
        Some(refusal) => Turn::Refused(refusal),
        None => Turn::Ended(ended),
    }
}

/// The error OpenCode saved for `session`, where `coder-history` reads
/// OpenCode's database.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn last_error(database: Option<&Path>, session: &str) -> Option<serde_json::Value> {
    coder_history::opencode::last_error(database?, session)
}

/// OpenCode's database is read only on Linux and macOS.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn last_error(_database: Option<&Path>, _session: &str) -> Option<serde_json::Value> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::tests::fixture_with;
    use super::super::{AgentEngine, Stage, run_stages};
    use super::*;
    use acp_client::replay;
    use coder::task::adapter::Configuration;
    use coder::task::capacity::Provider;
    use coder::task::{self, Action, Command, Store};

    /// The model the recorded turn ran on.
    const MODEL: &str = "google/gemini-3.6-flash";
    const SESSION: &str = "ses_f160610b7ffepNCFaIAle5HIB0";
    /// The model the recorded refusal ran on.
    const REFUSED_MODEL: &str = "opencode/gpt-5-nano";

    fn opencode(configuration: &mut Configuration, access: Access) {
        configuration.provider = "opencode".into();
        configuration.model = MODEL.into();
        configuration.effort = None;
        configuration.generation_endpoint = coder::task::capacity::OPENCODE_ENDPOINT.into();
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
            provider: "opencode".into(),
            model: model.into(),
            effort: None,
            generation_endpoint: coder::task::capacity::OPENCODE_ENDPOINT.into(),
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
            vec![Stage::Agent(AgentEngine::OpenCode, route(model), agent)];
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

    /// The configuration OpenCode was started with.
    fn started_config(agent_dir: &Path) -> Value {
        let environment = std::fs::read_to_string(agent_dir.join("environment")).unwrap();
        let line = environment
            .lines()
            .find_map(|line| line.strip_prefix("OPENCODE_CONFIG_CONTENT="))
            .expect("the inline configuration");
        serde_json::from_str(line).unwrap()
    }

    /// The recorded agent, with its environment written beside it.
    fn agent(dir: &Path, blocks: &[Vec<Value>]) -> PathBuf {
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

    #[tokio::test]
    async fn an_opencode_route_runs_the_recorded_turn_under_full_access() {
        let (_root, store, grant) = fixture_with(MODEL, |c| opencode(c, Access::Full));
        let agent_dir = tempfile::tempdir().unwrap();
        let agent = agent(agent_dir.path(), &replay::blocks(replay::OPENCODE_TURN));
        let task = run_turn(&store, &grant, MODEL, agent).await;
        assert_eq!(task.execution, task::Execution::Finished);
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_eq!(result.ending, "model_finished");
        assert_eq!(result.exit_code, Some(0));
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        for expected in [
            "cat note.txt",
            &format!("\"session\":\"{SESSION}\""),
            "\"kind\":\"opencode_prompt\"",
            "\"opencode_tool\"",
            "\"cost_usd\":0.0202536",
            "\"input_tokens\":2086",
        ] {
            assert!(trace.contains(expected), "missing {expected}");
        }
        assert_eq!(replay::arguments(agent_dir.path()), vec!["acp"]);
        let config = started_config(agent_dir.path());
        assert_eq!(config["model"], MODEL);
        assert_eq!(config["permission"], "allow");
        let environment = std::fs::read_to_string(agent_dir.path().join("environment")).unwrap();
        assert!(environment.contains(&format!(
            "OPENCODE_DB={}",
            coder_history::engine::OPENCODE_DATABASE
        )));
        let sent = replay::received(agent_dir.path());
        assert_eq!(sent[1]["method"], "session/new");
        assert_eq!(
            sent[1]["params"]["_meta"][acp_client::devin::ENGINE_META_KEY],
            coder_history::engine::MARK
        );
        assert_eq!(sent[2]["method"], "session/prompt");
        assert_eq!(
            sent[2]["params"]["prompt"][0]["text"],
            "Write result.txt containing output."
        );
    }

    #[tokio::test]
    async fn the_boundary_lets_opencode_edit_and_refuses_what_it_asks() {
        let (_root, store, grant) = fixture_with(MODEL, |c| opencode(c, Access::Boundary));
        let agent_dir = tempfile::tempdir().unwrap();
        let mut blocks = replay::blocks(replay::OPENCODE_TURN);
        let ask = json!({"jsonrpc":"2.0","id":"ask-1","method":"session/request_permission",
            "params":{"sessionId":SESSION,"toolCall":{"toolCallId":"nW2sx9GXPPZUcJJx","kind":"execute","title":"bash"},
            "options":[{"optionId":"once","kind":"allow_once","name":"Allow once"},
                {"optionId":"always","kind":"allow_always","name":"Always allow"},
                {"optionId":"reject","kind":"reject_once","name":"Reject"}]}});
        blocks[2].insert(0, ask);
        let agent = agent(agent_dir.path(), &blocks);
        let task = run_turn(&store, &grant, MODEL, agent).await;
        assert_eq!(
            task.run.as_ref().unwrap().result.as_ref().unwrap().ending,
            "model_finished"
        );
        let config = started_config(agent_dir.path());
        assert_eq!(config["permission"]["*"], "ask");
        assert_eq!(config["permission"]["edit"], "allow");
        assert_eq!(config["permission"]["external_directory"], "deny");
        let environment = std::fs::read_to_string(agent_dir.path().join("environment")).unwrap();
        assert!(environment.lines().all(|line| {
            !line
                .split('=')
                .next()
                .unwrap_or_default()
                .ends_with("_API_KEY")
        }));
        let sent = replay::received(agent_dir.path());
        let answer = sent
            .iter()
            .find(|line| line["id"] == "ask-1")
            .expect("the permission answer");
        assert_eq!(answer["result"]["outcome"]["optionId"], "reject");
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(trace.contains("opencode_permission"));
    }

    #[tokio::test]
    async fn a_follow_up_reattaches_the_same_opencode_session() {
        let (_root, store, grant) = fixture_with(MODEL, |c| opencode(c, Access::Full));
        let first_dir = tempfile::tempdir().unwrap();
        let first = agent(first_dir.path(), &replay::blocks(replay::OPENCODE_TURN));
        let ended = run_turn(&store, &grant, MODEL, first).await;
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
        let mut blocks = replay::blocks(replay::OPENCODE_TURN);
        // session/load answers with the session's options and no new ID.
        let mut loaded = blocks[1].last().unwrap().clone();
        loaded["result"]
            .as_object_mut()
            .unwrap()
            .remove("sessionId");
        blocks[1] = vec![loaded];
        let second_dir = tempfile::tempdir().unwrap();
        let second = agent(second_dir.path(), &blocks);
        let task = run_turn(&store, &serde_json::to_vec(&next).unwrap(), MODEL, second).await;
        assert_eq!(task.turn(), 2);
        let sent = replay::received(second_dir.path());
        assert_eq!(sent[1]["method"], "session/load");
        assert_eq!(sent[1]["params"]["sessionId"], SESSION);
        // The reattached session remembers the conversation: only the new
        // message is sent.
        assert_eq!(
            sent[2]["params"]["prompt"][0]["text"],
            "Now delete note.txt."
        );
        let trace = std::fs::read_to_string(store.join("fixture.2.atif.jsonl")).unwrap();
        assert!(trace.contains("\"resumed\":true"));
    }

    /// Writes the engine's OpenCode database under `data` with one failed
    /// assistant message in `session`, as OpenCode saves it.
    fn saved_failure(data: &Path, session: &str, error: &Value) {
        let dir = data.join("opencode");
        std::fs::create_dir_all(&dir).unwrap();
        let db =
            rusqlite::Connection::open(coder_history::engine::opencode_database(&dir)).unwrap();
        db.execute_batch(
            "CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL, \
             time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);",
        )
        .unwrap();
        db.execute(
            "INSERT INTO message VALUES ('msg_1', ?1, 1, 1, ?2)",
            rusqlite::params![
                session,
                json!({"role":"assistant","error":error}).to_string()
            ],
        )
        .unwrap();
    }

    #[tokio::test]
    async fn a_rate_limit_is_recorded_from_the_saved_message_and_ends_without_capacity() {
        // Under the boundary OpenCode runs with this process's environment,
        // where the test names OpenCode's data directory.
        let (_root, store, grant) = fixture_with(MODEL, |c| opencode(c, Access::Boundary));
        let data = tempfile::tempdir().unwrap();
        let error = json!({"name":"APIError","data":{"message":"rate limited","statusCode":429,
            "isRetryable":true,"responseHeaders":{"retry-after":"120"}}});
        saved_failure(data.path(), SESSION, &error);
        let agent_dir = tempfile::tempdir().unwrap();
        let mut blocks = replay::blocks(replay::OPENCODE_TURN);
        blocks[2] = vec![json!({"jsonrpc":"2.0","id":3,"error":{"code":-32603,
            "message":"Internal error: rate limited","data":{"service":"session","errorName":"APIError"}}})];
        let agent = agent(agent_dir.path(), &blocks);
        // SAFETY: this test alone names XDG_DATA_HOME, and the value is
        // read back only by this process's own turn.
        unsafe { std::env::set_var("XDG_DATA_HOME", data.path()) };
        let task = run_turn(&store, &grant, MODEL, agent).await;
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
        assert_eq!(
            task.run.as_ref().unwrap().result.as_ref().unwrap().ending,
            coder::task::capacity::NO_CAPACITY_ENDING
        );
        let now = coder::task::autostart::unix_now();
        let book = coder::task::capacity::Book::load(&store);
        assert!(!book.has_capacity(Provider::OpenCode, now));
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(trace.contains("route_exhausted"));
    }

    #[tokio::test]
    async fn a_refusal_that_is_not_a_rate_limit_ends_the_turn_with_its_error() {
        let (_root, store, grant) = fixture_with(REFUSED_MODEL, |c| {
            opencode(c, Access::Full);
            c.model = REFUSED_MODEL.into();
        });
        let agent_dir = tempfile::tempdir().unwrap();
        let agent = agent(agent_dir.path(), &replay::blocks(replay::OPENCODE_REFUSED));
        let task = run_turn(&store, &grant, REFUSED_MODEL, agent).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_ne!(result.ending, coder::task::capacity::NO_CAPACITY_ENDING);
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(trace.contains("Model access is disabled"));
    }

    #[tokio::test]
    async fn a_model_other_than_the_admitted_one_is_refused() {
        let admitted = "anthropic/claude-sonnet-5";
        let (_root, store, grant) = fixture_with(admitted, |c| {
            opencode(c, Access::Full);
            c.model = admitted.into();
        });
        let agent_dir = tempfile::tempdir().unwrap();
        let agent = agent(agent_dir.path(), &replay::blocks(replay::OPENCODE_TURN));
        let task = run_turn(&store, &grant, admitted, agent).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_ne!(result.exit_code, Some(0));
        let sent = replay::received(agent_dir.path());
        assert!(sent.iter().all(|line| line["method"] != "session/prompt"));
    }

    /// A live repository turn through the installed OpenCode, on the model
    /// `OPENCODE_LIVE_MODEL` names (such as `google/gemini-3.6-flash`),
    /// with OpenCode's own login. The fixture task asks for `result.txt`.
    #[tokio::test]
    #[ignore = "runs the installed opencode against a real provider; set OPENCODE_LIVE_MODEL"]
    async fn a_live_opencode_turn_writes_the_file() {
        let model = std::env::var("OPENCODE_LIVE_MODEL").expect("OPENCODE_LIVE_MODEL");
        let (root, store, grant) = fixture_with(&model, |c| {
            opencode(c, Access::Full);
            c.model.clone_from(&model);
        });
        let program = binary().unwrap();
        let task = run_turn(&store, &grant, &model, program).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert_eq!(result.ending, "model_finished", "{trace}");
        let checkout = root.path().join("checkout");
        assert!(checkout.join("result.txt").is_file(), "{trace}");
        assert!(trace.contains("\"kind\":\"opencode_prompt\""));
        eprintln!("{}", serde_json::to_string_pretty(&result).unwrap());
    }

    #[test]
    fn an_opencode_route_is_closed() {
        let (_root, _store, grant) = fixture_with(MODEL, |c| opencode(c, Access::Full));
        let grant = task::owner::Grant::parse(&grant).unwrap();
        let configuration = grant.adapter_configuration.unwrap();
        configuration.validate().unwrap();
        let mut effort = configuration.clone();
        effort.effort = Some("medium".into());
        assert!(effort.validate().is_err());
        let mut endpoint = configuration.clone();
        endpoint.generation_endpoint = "https://opencode.ai/zen/v1".into();
        assert!(endpoint.validate().is_err());
        let mut bare = configuration.clone();
        bare.model = "sonnet".into();
        assert!(bare.validate().is_err());
        assert_eq!(
            configuration.capabilities()["steering"]["adapter"],
            "opencode-acp"
        );
        assert_eq!(
            configuration.capabilities()["cost_reporting"],
            "provider-reported-list-price"
        );
    }
}
