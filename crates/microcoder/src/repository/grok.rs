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
//! - **Access**: full access passes `--always-approve`. Under toolchains
//!   the flag is omitted and the whole Grok Build process runs inside the
//!   host's own operating-system boundary, the one the loop's commands get
//!   (`Host::engine_boundary`): it and every tool it runs write only the
//!   workspace and a private scratch, read only the workspace, the system,
//!   this computer's toolchains, and the `grok` program, with the network.
//!   `HOME` and the Grok home are in that scratch, and the Grok home holds
//!   a copy of the login. The host allows what Grok Build asks, since the
//!   boundary holds it, but refuses a file-writing tool that names a path
//!   outside the workspace. Under the boundary (no network) Grok Build
//!   cannot reach xAI, so the turn is refused before it starts.
//! - **Cancellation**: a cancelled task, or one at its wall deadline, sends
//!   `session/cancel`, waits a grace, and stops the agent's process group.

use std::path::PathBuf;

use acp_client::{ClientError, Opening, StopReason};
use atif::{Source, Step};
use coder::task::adapter::{Access, Host, Route as GrantRoute};
use coder::task::capacity::Provider;
use serde_json::{Value, json};

use super::devin::{Answering, CANCEL_GRACE, Ended, Recorder, SILENCE, STOP_GRACE, Turn};

/// How long the copied sign-in must still last, besides the margin, for a
/// turn to start. A turn has no time limit; this is the half hour a long
/// turn is expected to take (the time limit turns had before #10103), and
/// a turn that runs past an expired copy fails its next request and says
/// so.
const TURN_LOGIN_SECONDS: i64 = 1800;

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

/// A Grok Build process held in the host's boundary: the boundary (held
/// until the process is reaped), the wrapped program and arguments, the
/// process's environment, and what the evidence records.
struct Contained {
    boundary: coder_boundary::Boundary,
    program: PathBuf,
    arguments: Vec<String>,
    environment: Vec<(String, String)>,
    record: Value,
}

/// Put Grok Build (`program arguments`) inside this run's boundary, with a
/// private Grok home in the boundary's scratch that holds a copy of the
/// login, or `XAI_API_KEY` when this process has it.
fn contain(
    host: &Host,
    program: &std::path::Path,
    arguments: &[String],
) -> Result<Contained, String> {
    if host.configuration().access == Access::Boundary {
        return Err(
            "Grok Build reaches xAI from its own process, and this run's access \
            (boundary) allows no network; run it with this computer's toolchains or full access"
                .to_owned(),
        );
    }
    let program = program
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", program.display()))?;
    let boundary = host
        .engine_boundary(std::slice::from_ref(&program))
        .map_err(|error| error.to_string())?;
    let scratch = boundary
        .scratch()
        .ok_or("the engine boundary has no scratch")?
        .to_path_buf();
    let home = scratch.join(".grok");
    std::fs::create_dir(&home).map_err(|error| format!("cannot make the Grok home: {error}"))?;
    let variable = login_variable;
    let key = variable(acp_client::grok::API_KEY_VAR)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.into_string().ok());
    let login = if key.is_some() {
        "XAI_API_KEY"
    } else {
        let source = acp_client::grok::auth_path(&variable)
            .filter(|path| {
                path.metadata()
                    .is_ok_and(|meta| meta.is_file() && meta.len() > 0)
            })
            .ok_or("Grok Build is not signed in on this computer (run grok and log in)")?;
        let bytes = std::fs::read(&source)
            .map_err(|error| format!("cannot read the Grok Build login: {error}"))?;
        let now = i64::try_from(coder::task::autostart::unix_now()).unwrap_or(i64::MAX);
        let needed = TURN_LOGIN_SECONDS.saturating_add(acp_client::grok::LOGIN_MARGIN_SECONDS);
        if let Some(left) = acp_client::grok::login_seconds_left(&bytes, now)
            && left < needed
        {
            return Err(format!(
                "Grok Build's sign-in expires in {} minutes, within the half hour a turn may take, and a \
                 sandboxed turn uses a copy it must not refresh; run grok once to refresh the \
                 sign-in, then try again",
                left.max(0) / 60
            ));
        }
        write_private(&home.join("auth.json"), &bytes)
            .map_err(|error| format!("cannot copy the Grok Build login: {error}"))?;
        "copy"
    };
    let mut environment: Vec<(String, String)> = host
        .bounded_environment(&boundary)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .collect();
    environment.push((
        acp_client::grok::HOME_VAR.to_owned(),
        home.to_string_lossy().into_owned(),
    ));
    if let Some(key) = key {
        environment.push((acp_client::grok::API_KEY_VAR.to_owned(), key));
    }
    let wrapped = boundary
        .command(&program, arguments)
        .map_err(|error| error.to_string())?;
    let wrapped_program = PathBuf::from(wrapped.get_program());
    let wrapped_arguments = wrapped
        .get_args()
        .map(|argument| {
            argument
                .to_str()
                .map(str::to_owned)
                .ok_or("a boundary argument is not UTF-8")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let record = json!({"backend": boundary.backend(), "checkout": boundary.checkout(),
        "writable": boundary.writable(), "reads": boundary.readable().len(),
        "offline": boundary.offline(), "grok_home": home, "login": login, "program": program});
    Ok(Contained {
        boundary,
        program: wrapped_program,
        arguments: wrapped_arguments,
        environment,
        record,
    })
}

#[cfg(test)]
thread_local! {
    /// The variables a test's login lookup reads, in place of this
    /// process's: tests never read the real home.
    static LOGIN_VARIABLES: std::cell::RefCell<Vec<(String, std::ffi::OsString)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// A variable the login lookup reads: `HOME`, `GROK_HOME`, `XAI_API_KEY`.
fn login_variable(name: &str) -> Option<std::ffi::OsString> {
    #[cfg(test)]
    return LOGIN_VARIABLES.with(|variables| {
        variables
            .borrow()
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    });
    #[cfg(not(test))]
    std::env::var_os(name)
}

/// Write `bytes` to a new file only this user can read.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

/// Run one turn on `route` with the Grok Build binary `program`.
pub(crate) async fn turn(
    host: &Host,
    route: &GrantRoute,
    program: PathBuf,
    mut recipe: Option<&mut super::recipe::Recipe>,
) -> Turn {
    let mut ended = Ended {
        engine: ENGINE,
        agent: "Grok Build",
        ..Ended::default()
    };
    if let Err(why) = acp_client::grok::parse_model(&route.model) {
        ended.error = Some(why);
        return Turn::Ended(ended);
    }
    let access = host.configuration().access;
    let approve = access == Access::Full;
    // The delegate recipe's effort for the task's class (#10208); without
    // a recipe, Grok Build's own.
    let effort = recipe
        .as_deref()
        .and_then(|recipe| recipe.effort("grok", route.effort.as_deref()));
    // Under full access Grok Build gets the owner's login environment;
    // inside the boundary its private home holds the key when this
    // process has one, else a copy of the stored login.
    let environment = if approve {
        Some(grok_environment(host).await)
    } else {
        None
    };
    let api_login = match &environment {
        Some(environment) => acp_client::grok::api_key_login(&|name| {
            environment
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into())
        }),
        None => {
            login_variable(acp_client::grok::API_KEY_VAR).is_some_and(|value| !value.is_empty())
        }
    };
    // The API login's own default model fakes tool results (#10275): a
    // route that keeps Grok Build's default runs a capable one instead.
    let model = acp_client::grok::session_model(&route.model, api_login).to_owned();
    if let Some(why) = acp_client::grok::refusal(&model) {
        ended.error = Some(why);
        return Turn::Ended(ended);
    }
    let arguments = acp_client::grok::arguments_with_effort(&model, approve, effort.as_deref());
    let earlier = host.earlier_note(SESSION_NOTE).and_then(|note| {
        note.get("session")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    // Outside full access the whole process runs in the host's boundary,
    // with a Grok home of its own that lasts one turn, so an earlier turn's
    // session is not there to reattach: the new session is told the
    // earlier turns instead.
    let contained = if approve {
        None
    } else {
        match contain(host, &program, &arguments) {
            Ok(contained) => Some(contained),
            Err(why) => {
                ended.error = Some(why);
                return Turn::Ended(ended);
            }
        }
    };
    let resume = if contained.is_none() { earlier } else { None };
    let spec = match &contained {
        Some(contained) => acp_client::process::Spec {
            program: contained.program.clone(),
            arguments: contained.arguments.clone(),
            cwd: host.workspace().to_path_buf(),
            environment: contained.environment.clone(),
        },
        None => super::private_spec(
            host,
            acp_client::process::Spec {
                program: program.clone(),
                arguments: arguments.clone(),
                cwd: host.workspace().to_path_buf(),
                environment: environment.unwrap_or_default(),
            },
        ),
    };
    let opening = Opening {
        spec,
        resume: resume.clone(),
        meta: Some(acp_client::devin::engine_meta(coder_history::engine::MARK)),
        mode: None,
        authenticate: None,
    };
    let sequence = match host.effect(
        "grok_session",
        json!({"program": program, "arguments": arguments, "cwd": host.workspace(),
            "approve": approve, "resume": resume, "model": model, "route_model": route.model,
            "boundary": contained.as_ref().map(|contained| &contained.record)}),
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
    if !acp_client::grok::admits(&model, reported.as_deref()) {
        host.fail("Grok Build reported a model different from the admitted model");
        ended.error = Some(format!(
            "Requested {model}, Grok Build reported {}; refusing the turn. This Grok Build \
             login may not offer {model} (`grok models` lists the ones it does).",
            reported.as_deref().unwrap_or("no model")
        ));
        session.close(STOP_GRACE).await;
        return Turn::Ended(ended);
    }
    if let Some(why) = reported.as_deref().and_then(acp_client::grok::refusal) {
        host.fail("the Grok Build model is known to report edits it never made");
        ended.error = Some(why);
        session.close(STOP_GRACE).await;
        return Turn::Ended(ended);
    }
    // A reattached session remembers the conversation; a new one is told it.
    // With the delegate recipe, the briefing comes first (#10208).
    let prompt = super::recipe::agent_prompt(recipe.as_deref(), host, session.resumed);
    let prompted = match host.effect(
        "grok_prompt",
        json!({"session": session.id(), "prompt": prompt, "model": model}),
    ) {
        Ok(sequence) => sequence,
        Err(error) => {
            ended.error = Some(error.to_string());
            session.close(STOP_GRACE).await;
            return Turn::Ended(ended);
        }
    };
    let mut recorder = Recorder::new(host, "Grok Build", "grok", model.clone(), access);
    if let Some(contained) = &contained {
        let roots = contained
            .boundary
            .checkout()
            .into_iter()
            .chain(contained.boundary.writable().iter().map(PathBuf::as_path))
            .map(std::path::Path::to_path_buf)
            .collect();
        recorder = recorder.answering(Answering::Contained {
            base: host.workspace().to_path_buf(),
            roots,
        });
    }
    let silence = SILENCE;
    let (result, checks_passed) = super::recipe::prompt_watched(
        &mut session,
        &prompt,
        host,
        recipe.as_deref_mut(),
        silence,
        CANCEL_GRACE,
        &mut recorder,
    )
    .await;
    ended.checks_passed = checks_passed;
    recorder.close();
    ended.reply = std::mem::take(&mut recorder.reply);
    ended.refused = recorder.refused.take();
    ended.tool_calls = recorder.tool_calls;
    ended.cost_usd = recorder.cost_usd;
    ended.input_tokens = recorder.input_tokens;
    ended.output_tokens = recorder.output_tokens;
    let stderr = session.stderr_tail();
    let group_clear = session.close(STOP_GRACE).await;
    if !group_clear {
        host.fail("the Grok Build process group did not stop");
    }
    let refusal = match &result {
        Ok(reply) => {
            ended.stop = Some(reply.stop_reason);
            if let Some(usage) = reply.usage {
                ended.input_tokens = usage.input_tokens.unwrap_or_default();
                ended.output_tokens = usage.output_tokens.unwrap_or_default();
            }
            None
        }
        // A limit before any work fails over to the next route (#10765).
        Err(ClientError::Refused { error, .. })
            if ended.tool_calls == 0 && ended.reply.is_empty() && error.limited() =>
        {
            coder::task::capacity::acp_refusal(
                Provider::Grok,
                error,
                coder::task::autostart::unix_now(),
            )
        }
        Err(error) => {
            ended.error = Some(error.to_string());
            super::devin::book_limit(host, Provider::Grok, error);
            None
        }
    };
    let observation = json!({"stop_reason": ended.stop.map(StopReason::as_str),
        "error": ended.error, "refusal": refusal, "group_clear": group_clear,
        "input_tokens": ended.input_tokens, "output_tokens": ended.output_tokens,
        "tool_calls": ended.tool_calls, "cost_usd": ended.cost_usd,
        "stderr_tail": if ended.error.is_some() { json!(stderr) } else { Value::Null },
        "billing": "unknown"});
    if let Err(error) = host.result(prompted, "grok_prompt", observation) {
        ended.error = Some(error.to_string());
    }
    // The boundary's profile and scratch outlive the process group.
    drop(contained);
    match refusal {
        Some(refusal) => Turn::Refused(refusal),
        None => Turn::Ended(ended),
    }
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
    async fn a_model_that_fakes_tool_results_is_refused_before_it_starts() {
        let faking = acp_client::grok::FAKES_TOOL_RESULTS[0];
        let (_root, store, grant) = fixture_with(faking, |c| grok_route(c, Access::Full, faking));
        let agent_dir = tempfile::tempdir().unwrap();
        let agent = agent(agent_dir.path(), &replay::blocks(replay::GROK_TURN));
        let task = run_turn(&store, &grant, faking, agent).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_eq!(result.ending, "engine_incomplete");
        assert!(replay::arguments(agent_dir.path()).is_empty());
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(trace.contains("reports edits it never made"), "{trace}");
    }

    #[tokio::test]
    async fn the_boundary_refuses_a_grok_turn_before_it_starts() {
        let (_root, store, grant) = fixture_with(acp_client::grok::DEFAULT_MODEL, |c| {
            grok_route(c, Access::Boundary, acp_client::grok::DEFAULT_MODEL)
        });
        let agent_dir = tempfile::tempdir().unwrap();
        let agent = agent(agent_dir.path(), &replay::blocks(replay::GROK_TURN));
        let task = run_turn(&store, &grant, acp_client::grok::DEFAULT_MODEL, agent).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_eq!(result.ending, "engine_incomplete");
        assert!(replay::arguments(agent_dir.path()).is_empty());
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(trace.contains("allows no network"), "{trace}");
        assert!(trace.contains("\"stopped\":\"Grok Build could not finish the turn"));
    }

    /// A Grok Build login in a temporary Grok home, for the turn's lookup,
    /// valid for a day after now.
    fn signed_in(dir: &std::path::Path) -> String {
        let expires = std::time::SystemTime::now() + std::time::Duration::from_secs(86_400);
        let at = expires
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let (days, seconds) = (at / 86_400, at % 86_400);
        // Civil date from days (Howard Hinnant's algorithm).
        let z = days as i64 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        let login = format!(
            r#"{{"https://auth.x.ai":{{"key":"fixture-login","expires_at":"{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z"}}}}"#,
            seconds / 3600,
            seconds % 3600 / 60,
            seconds % 60
        );
        std::fs::write(dir.join("auth.json"), &login).unwrap();
        LOGIN_VARIABLES.with(|variables| {
            *variables.borrow_mut() = vec![(
                acp_client::grok::HOME_VAR.to_owned(),
                dir.as_os_str().to_owned(),
            )];
        });
        login
    }

    fn ask(id: &str, kind: &str, title: &str, path: Option<&str>) -> Value {
        let mut tool = json!({"toolCallId": format!("tool-{id}"), "kind": kind, "title": title});
        if let Some(path) = path {
            tool["locations"] = json!([{"path": path}]);
            tool["rawInput"] = json!({"path": path, "content": "x"});
        }
        json!({"jsonrpc":"2.0","id":id,"method":"session/request_permission",
            "params":{"sessionId":SESSION,"toolCall":tool,
            "options":[{"optionId":"always","kind":"allow_always","name":"Always"},
                {"optionId":"allow","kind":"allow_once","name":"Allow"},
                {"optionId":"reject","kind":"reject_once","name":"Reject"}]}})
    }

    /// At this computer's toolchains (a person's default local access,
    /// #10092), the whole Grok Build process runs inside the host's own
    /// boundary: it cannot write outside the workspace, its `HOME` and Grok
    /// home are the boundary's scratch with a copy of the login, and the
    /// host allows the commands it asks for but refuses a file write that
    /// names a path outside the workspace.
    #[tokio::test]
    async fn toolchains_runs_grok_inside_the_boundary_and_allows_what_it_holds() {
        let (root, store, grant) = fixture_with(acp_client::grok::DEFAULT_MODEL, |c| {
            grok_route(c, Access::Toolchains, acp_client::grok::DEFAULT_MODEL)
        });
        let login_dir = tempfile::tempdir().unwrap();
        let login = signed_in(login_dir.path());
        let elsewhere = tempfile::tempdir_in("/var/tmp").unwrap();
        let outside = elsewhere.path().join("outside.txt");
        // The stand-in keeps its records in the workspace, the one place
        // it may write.
        let agent_dir = root.path().join("checkout/.agent");
        std::fs::create_dir(&agent_dir).unwrap();
        let mut blocks = replay::blocks(replay::GROK_TURN);
        blocks[2].insert(0, ask("ask-run", "execute", "Run ls && git status", None));
        blocks[2].insert(
            1,
            ask(
                "ask-out",
                "edit",
                "Write outside",
                Some(outside.to_str().unwrap()),
            ),
        );
        blocks[2].insert(
            2,
            ask("ask-in", "edit", "Write notes", Some("notes/plan.md")),
        );
        let agent = agent(&agent_dir, &blocks);
        let body = std::fs::read_to_string(&agent).unwrap().replacen(
            "#!/bin/sh\n",
            &format!(
                "#!/bin/sh\ncp \"$GROK_HOME/auth.json\" '{dir}/login'\n\
                 if printf x > '{out}' 2>/dev/null; then echo written > '{dir}/outside'; else echo denied > '{dir}/outside'; fi\n",
                dir = agent_dir.display(),
                out = outside.display()
            ),
            1,
        );
        std::fs::write(&agent, body).unwrap();
        let task = run_turn(&store, &grant, acp_client::grok::DEFAULT_MODEL, agent).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert_eq!(result.ending, "model_finished", "{trace}");
        // The OS boundary held the agent process itself.
        assert!(!outside.exists());
        assert_eq!(
            std::fs::read_to_string(agent_dir.join("outside")).unwrap(),
            "denied\n"
        );
        assert_eq!(
            replay::arguments(&agent_dir),
            vec!["agent", "--no-leader", "stdio"]
        );
        // A copy of the login, in a Grok home inside the scratch.
        assert_eq!(
            std::fs::read_to_string(agent_dir.join("login")).unwrap(),
            login
        );
        let environment = std::fs::read_to_string(agent_dir.join("environment")).unwrap();
        let value = |name: &str| {
            environment
                .lines()
                .find_map(|line| line.strip_prefix(&format!("{name}=")))
                .unwrap_or_default()
                .to_owned()
        };
        let home = value("HOME");
        assert!(!home.is_empty());
        assert_eq!(value("GROK_HOME"), format!("{home}/.grok"));
        assert_ne!(home, std::env::var("HOME").unwrap_or_default());
        assert!(
            !std::path::Path::new(&home).exists(),
            "the scratch is removed"
        );
        let sent = replay::received(&agent_dir);
        let answer = |id: &str| {
            sent.iter()
                .find(|line| line["id"] == id)
                .unwrap_or_else(|| panic!("no answer to {id}"))["result"]["outcome"]["optionId"]
                .clone()
        };
        assert_eq!(answer("ask-run"), "allow");
        assert_eq!(answer("ask-out"), "reject");
        assert_eq!(answer("ask-in"), "allow");
        assert!(trace.contains("outside the workspace"), "{trace}");
        assert!(trace.contains("\"login\":\"copy\""), "{trace}");
        assert!(leaked_credential_names(&agent_dir).is_empty());
    }

    /// A Grok Build turn the agent ended itself after the host refused a
    /// tool says so, never that the task was stopped.
    #[tokio::test]
    async fn a_turn_grok_ended_after_a_refusal_says_so() {
        let (root, store, grant) = fixture_with(acp_client::grok::DEFAULT_MODEL, |c| {
            grok_route(c, Access::Toolchains, acp_client::grok::DEFAULT_MODEL)
        });
        let login_dir = tempfile::tempdir().unwrap();
        signed_in(login_dir.path());
        let agent_dir = root.path().join("checkout/.agent");
        std::fs::create_dir(&agent_dir).unwrap();
        let mut blocks = replay::blocks(replay::GROK_TURN);
        blocks[2] = vec![
            ask("ask-out", "edit", "Write /etc/hosts", Some("/etc/hosts")),
            json!({"jsonrpc":"2.0","id":3,"result":{"stopReason":"cancelled"}}),
        ];
        let agent = agent(&agent_dir, &blocks);
        let task = run_turn(&store, &grant, acp_client::grok::DEFAULT_MODEL, agent).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        assert_eq!(result.ending, "engine_stopped_after_refusal");
        assert!(!result.stop_requested);
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert!(
            trace.contains(
                "Grok Build stopped after the host refused a tool it asked to run (Write /etc/hosts: it would write /etc/hosts, outside the workspace)."
            ),
            "{trace}"
        );
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
        assert_eq!(result.ending, coder::task::adapter::HOST_FAULT);
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

    /// The live boundary check (#10092): the installed Grok Build CLI, at
    /// this computer's toolchains, runs a shell command that writes outside
    /// the workspace; the host allows the command, the operating-system
    /// boundary refuses the write, and the turn still finishes. The login
    /// is only read and copied into the turn's scratch. Run with
    /// `cargo test -p microcoder --lib live_grok -- --ignored`.
    #[tokio::test]
    #[ignore = "runs the installed Grok Build CLI with the owner's login and spends a model request"]
    async fn live_grok_at_toolchains_writes_only_the_workspace() {
        let agent = binary().expect("grok binary");
        LOGIN_VARIABLES.with(|variables| {
            *variables.borrow_mut() = [
                "HOME",
                acp_client::grok::HOME_VAR,
                acp_client::grok::API_KEY_VAR,
            ]
            .into_iter()
            .filter_map(|name| Some((name.to_owned(), std::env::var_os(name)?)))
            .collect();
        });
        let elsewhere = tempfile::tempdir_in("/var/tmp").unwrap();
        let outside = elsewhere.path().join("grok-outside.txt");
        let prompt = format!(
            "Run exactly this shell command once: printf x > {} ; then write result.txt \
             in the repository containing the word output, and say whether the first \
             command succeeded.",
            outside.display()
        );
        let (root, store, grant) = super::super::tests::fixture_images(
            acp_client::grok::DEFAULT_MODEL,
            |c| grok_route(c, Access::Toolchains, acp_client::grok::DEFAULT_MODEL),
            &[],
            &prompt,
        );
        let mut grant: task::owner::Grant = serde_json::from_slice(&grant).unwrap();
        grant.wall_seconds = 300;
        let grant = serde_json::to_vec(&grant).unwrap();
        let task = run_turn(&store, &grant, acp_client::grok::DEFAULT_MODEL, agent).await;
        let result = task.run.as_ref().unwrap().result.as_ref().unwrap();
        let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
        assert_eq!(result.ending, "model_finished", "{result:?}\n{trace}");
        assert!(!outside.exists(), "the write outside the workspace landed");
        let written = std::fs::read_to_string(root.path().join("checkout/result.txt")).unwrap();
        assert!(written.contains("output"), "{written}");
        assert!(trace.contains("\"login\":"), "{trace}");
        assert!(trace.contains("grok_permission"), "{trace}");
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
