//! Web chat's public-client requests and observations. Recovery reads the same
//! saved request; credentials are retained only in memory for active observers.

use crate::{App, chat_store::Store};
use axum::http::HeaderMap;
use openagents_sdk::{
    CancellationToken, Client, Deployment, Session,
    computers::{CommandRequest, CreateRequest, OutputReader},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(crate) struct Context {
    pub client: Client,
    pub session: Session,
    pub workspace: Option<(String, u64)>,
}
static OBSERVERS: OnceLock<Mutex<HashMap<String, (Context, Instant)>>> = OnceLock::new();
const OBSERVER_LIFETIME: Duration = Duration::from_secs(4500);

/// The switch affects new runs only. Existing public runs always remain public.
pub(crate) fn enabled() -> bool {
    std::env::var("OPENAGENTS_WEB_PUBLIC_COMPUTERS").as_deref() != Ok("legacy")
}

pub(crate) async fn authorize(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
) -> Result<Context, String> {
    let cloud = app
        .config
        .cloud
        .as_deref()
        .ok_or("Log in to use this computer.")?;
    if crate::agent_work::scope(app, headers).await.is_none() {
        return Err("Cloud computers aren't available for this account yet.".into());
    }
    let base = app
        .config
        .api_host
        .as_ref()
        .map(|host| format!("https://{host}/v1"))
        .unwrap_or_else(|| format!("{}/v1", cloud.origin()));
    let deployment = Deployment::new(&base, cloud.origin()).map_err(|e| e.to_string())?;
    let client = Client::with_timeouts(deployment, Duration::from_secs(5), Duration::from_secs(65))
        .map_err(|e| e.to_string())?;
    let session = cloud
        .computer_session(headers, &client)
        .await
        .map_err(|e| e.to_string())?;
    if crate::chat_store::account_owner(&session.account().id) != owner {
        return Err("Start a chat of your own to use this computer.".into());
    }
    let viewer = cloud
        .authenticate(headers)
        .await
        .map_err(|e| e.to_string())?;
    let workspace = viewer
        .workspace
        .map(|workspace| (workspace.id, workspace.members_epoch));
    let context = Context {
        client,
        session,
        workspace,
    };
    let mut observers = OBSERVERS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    observers.retain(|_, (_, until)| *until > Instant::now());
    if observers.len() >= 1024 && !observers.contains_key(owner) {
        return Err("Too many tasks are running. Try again later.".into());
    }
    observers.insert(
        owner.into(),
        (context.clone(), Instant::now() + OBSERVER_LIFETIME),
    );
    Ok(context)
}
pub(crate) fn observer(owner: &str) -> Option<Context> {
    let observers = OBSERVERS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    observers
        .get(owner)
        .filter(|(_, until)| *until > Instant::now())
        .map(|(context, _)| context.clone())
}

pub(crate) fn forget(owner: &str) {
    OBSERVERS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(owner);
}

/// Retain a little live progress and the complete final JSON line while
/// draining everything the agent writes. No transcript file is created.
pub(crate) fn bounded_agent_command(command: &str) -> String {
    format!(
        r#"umask 077
mkdir -p "$HOME/.oa-chat" || exit 1
d=$(mktemp -d "$HOME/.oa-chat/output.XXXXXX") || exit 1
trap 'rm -rf "$d"' 0
mkfifo "$d/events" || exit 1
LC_ALL=C awk '{{ if(length($0)) {{ last=$0; long=(length(last)>131072); if(long) last=""; }} if(sent<32768) {{ text=substr($0,1,32768-sent); print text; sent+=length(text)+1; fflush(); }} }} END {{ if(long) {{ print "The task answer is too long. Open its changes to review it." > "/dev/stderr"; exit 1; }} print last; fflush(); }}' < "$d/events" &
reader=$!
(
{command}
) > "$d/events"
code=$?
wait "$reader" || code=1
exit "$code"
"#
    )
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Run {
    pub v: String,
    pub account: String,
    pub workspace: Option<(String, u64)>,
    pub chat: String,
    pub request: String,
    pub create: CreateRequest,
    pub input_digest: String,
    pub environment: String,
    pub repository: String,
    pub version: u64,
    pub title: String,
    pub computer: Option<String>,
    pub command: Option<CommandRequest>,
    pub credentials_prepared: bool,
    pub command_id: Option<String>,
    pub reader: Option<OutputReader>,
    pub uploads: Vec<openagents_sdk::computers::WriteIntent>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub gap: bool,
}
fn key(owner: &str, chat: &str, request: &str) -> Result<String, crate::chat_store::Error> {
    if !crate::chat_store::valid_id(chat) || !crate::chat_store::valid_id(request) {
        return Err(crate::chat_store::Error::Invalid("The request is invalid."));
    }
    Store::owner_key(
        owner,
        &format!("files/{chat}/computer-run-{request}.record"),
    )
}
pub(crate) async fn read(
    store: &Store,
    owner: &str,
    chat: &str,
    request: &str,
) -> Result<Option<(Run, String)>, String> {
    let key = key(owner, chat, request).map_err(|e| e.to_string())?;
    let Some((bytes, generation)) = store.read_key(&key).await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let run: Run = serde_json::from_slice(&bytes).map_err(|_| "We couldn't read this task.")?;
    if run.v != "openagents.web.computer-run.v1"
        || run.chat != chat
        || run.request != request
        || crate::chat_store::account_owner(&run.account) != owner
    {
        return Err("We couldn't read this task.".into());
    }
    Ok(Some((run, generation)))
}
pub(crate) async fn save(
    store: &Store,
    owner: &str,
    run: &Run,
    expected: Option<&str>,
) -> Result<String, String> {
    if crate::chat_store::account_owner(&run.account) != owner {
        return Err("This task belongs to another account.".into());
    }
    let key = key(owner, &run.chat, &run.request).map_err(|e| e.to_string())?;
    store
        .write_key(
            &key,
            serde_json::to_vec(run).map_err(|_| "We couldn't save this task.")?,
            expected,
        )
        .await
        .map_err(|e| e.to_string())
}

/// An index keeps the old `cmp_.trn_` task ID while recording which transport
/// owns it. A rollback never tries the legacy turn endpoint for a public run.
pub(crate) async fn index(
    store: &Store,
    owner: &str,
    task: &str,
    request: &str,
    chat: &str,
) -> Result<(), String> {
    let name = format!(
        "computer-run-index/{:x}.json",
        Sha256::digest(task.as_bytes())
    );
    let key = Store::owner_key(owner, &name).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&(chat, request)).unwrap();
    if let Some((old, _)) = store.read_key(&key).await.map_err(|e| e.to_string())? {
        return if old == bytes {
            Ok(())
        } else {
            Err("This task changed. Reload the chat.".into())
        };
    }
    store
        .write_key(&key, bytes, None)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}
pub(crate) async fn for_task(
    store: &Store,
    owner: &str,
    task: &str,
) -> Result<Option<(Run, String)>, String> {
    let key = Store::owner_key(
        owner,
        &format!(
            "computer-run-index/{:x}.json",
            Sha256::digest(task.as_bytes())
        ),
    )
    .map_err(|e| e.to_string())?;
    let Some((bytes, _)) = store.read_key(&key).await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let (chat, request): (String, String) =
        serde_json::from_slice(&bytes).map_err(|_| "We couldn't read this task.")?;
    read(store, owner, &chat, &request).await
}

pub(crate) async fn cancel(app: &App, owner: &str, task: &str) -> Result<bool, String> {
    let Some((run, _)) = for_task(&app.config.chat_store, owner, task).await? else {
        return Ok(false);
    };
    let context = observer(owner).ok_or("Open the chat again to stop this task.")?;
    let computers = context
        .client
        .computers(&context.session)
        .map_err(|e| e.to_string())?;
    let computer = computers
        .reference(
            run.computer
                .as_deref()
                .ok_or("This task hasn't started yet.")?,
        )
        .map_err(|e| e.to_string())?;
    let command = computers
        .command_reference(
            &computer,
            run.command_id
                .as_deref()
                .ok_or("This task hasn't started yet.")?,
        )
        .map_err(|e| e.to_string())?;
    computers
        .cancel_command(&command, &CancellationToken::new())
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

/// Save both identities before either request leaves this process. A retry
/// imports these exact SDK requests, including after a process restart.
pub(crate) async fn start(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    chat: &str,
    request: &str,
    turn: &coder_environment_operator::studio::ChatTurn,
    title: &str,
    repository: &str,
    files: &[coder_environment_operator::studio::claude::Attachment],
) -> Result<String, String> {
    use openagents_sdk::computers::{CommandOptions, CreateOptions, RequestKey};
    let context = authorize(app, headers, owner).await?;
    let computers = context
        .client
        .computers(&context.session)
        .map_err(|e| e.to_string())?;
    let cancel = CancellationToken::new();
    let store = &app.config.chat_store;
    let file_digests: Vec<_> = files
        .iter()
        .map(|file| (&file.name, format!("{:x}", Sha256::digest(&file.bytes))))
        .collect();
    let input_digest = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                &file_digests,
                turn.environment.as_str(),
                turn.version_id.as_str(),
                turn.command.as_str(),
                turn.env
                    .iter()
                    .find(|(name, _)| name == "OA_TURN_TASK")
                    .map(|(_, value)| value.as_str())
            ))
            .unwrap()
        )
    );
    let (mut run, mut generation) = match read(store, owner, chat, request).await? {
        Some((run, generation)) => (run, Some(generation)),
        None => {
            let create = computers
                .create_request(CreateOptions {
                    chat: Some(chat.into()),
                    environment: Some(turn.environment.clone()),
                    version: Some(turn.version.to_string()),
                    size: Some(turn.size.clone()),
                    idempotency_key: Some(format!("chat-{chat}")),
                    ..Default::default()
                })
                .map_err(|e| e.to_string())?;
            let run = Run {
                v: "openagents.web.computer-run.v1".into(),
                account: context.session.account().id.clone(),
                workspace: context.workspace.clone(),
                chat: chat.into(),
                request: request.into(),
                create,
                input_digest: input_digest.clone(),
                environment: turn.environment.clone(),
                repository: repository.into(),
                version: turn.version,
                title: title.into(),
                computer: None,
                command: None,
                credentials_prepared: false,
                command_id: None,
                reader: None,
                uploads: Vec::new(),
                stdout: Vec::new(),
                stderr: Vec::new(),
                gap: false,
            };
            match save(store, owner, &run, None).await {
                Ok(generation) => (run, Some(generation)),
                Err(_) => read(store, owner, chat, request)
                    .await?
                    .map(|(r, g)| (r, Some(g)))
                    .ok_or("We couldn't save this task. Try again.")?,
            }
        }
    };
    if run.input_digest != input_digest {
        return Err("This task changed. Send it as a new message.".into());
    }
    if run.account != context.session.account().id || run.workspace != context.workspace {
        return Err("This task belongs to another account.".into());
    }
    if run.computer.is_none() {
        let computer = computers
            .create(&run.create, &cancel)
            .await
            .map_err(|error| match error.code() {
                Some("idempotency_conflict") => {
                    "Start a new chat to use a different saved environment.".into()
                }
                Some("quota_exhausted") => {
                    "Delete an unused cloud computer before starting another chat.".into()
                }
                _ => error.to_string(),
            })?;
        run.computer = Some(computer.reference().id().into());
        generation = Some(save(store, owner, &run, generation.as_deref()).await?);
    }
    let computer = computers
        .reference(run.computer.as_deref().unwrap())
        .map_err(|e| e.to_string())?;
    if run.command_id.is_none() && !files.is_empty() {
        if run.uploads.is_empty() {
            run.uploads = files
                .iter()
                .enumerate()
                .map(|(i, file)| {
                    computers
                        .write_intent(&computer, &upload_path(request, i, &file.name), &file.bytes)
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            generation = Some(save(store, owner, &run, generation.as_deref()).await?);
        }
        if run.uploads.len() != files.len() {
            return Err("The files changed. Send them as a new message.".into());
        }
        let current = computers
            .get(&computer, &cancel)
            .await
            .map_err(|e| e.to_string())?;
        if current.stopped() {
            computers
                .wake(&computer, &cancel)
                .await
                .map_err(|e| e.to_string())?;
        }
        computers
            .wait(
                &computer,
                openagents_sdk::computers::WaitFor::Awake,
                Duration::from_secs(240),
                &cancel,
            )
            .await
            .map_err(|e| e.to_string())?;
        for (intent, file) in run.uploads.iter().zip(files) {
            computers
                .write_file(intent, &file.bytes, &cancel)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    if run.command.is_none() {
        let reference = format!(
            "cred_{}",
            RequestKey::generate().map_err(|e| e.to_string())?.as_str()
        );
        let mut options = CommandOptions {
            command: turn.command.clone(),
            timeout_seconds: Some(3600),
            idempotency_key: Some(format!("web-{request}")),
            ..Default::default()
        };
        options.credential_ref = Some(reference);
        run.command = Some(
            computers
                .command_request(&computer, options)
                .map_err(|e| e.to_string())?,
        );
        generation = Some(save(store, owner, &run, generation.as_deref()).await?);
    }
    let command_request = run.command.as_ref().unwrap();
    if run.command_id.is_none() {
        // Only the server-created reference crosses the public client. Values
        // go through the API/service boundary and are never part of this record.
        let saved =
            serde_json::to_value(command_request).map_err(|_| "We couldn't read this task.")?;
        let reference = saved["body"]["credential_ref"]
            .as_str()
            .ok_or("We couldn't authorize this task.")?;
        if saved["body"]["command"].as_str() != Some(&turn.command) {
            return Err("This task changed. Send it as a new message.".into());
        }
        if !run.credentials_prepared {
            crate::computers_api::commands::prepare_web_credentials(
                owner,
                computer.id(),
                reference,
                command_request.key().as_str(),
                &turn.command,
                &turn.env,
            )
            .await?;
            run.credentials_prepared = true;
            generation = Some(save(store, owner, &run, generation.as_deref()).await?);
        }
        let command = computers
            .execute(command_request, &cancel)
            .await
            .map_err(|e| e.to_string())?;
        run.command_id = Some(command.reference().id().into());
        run.reader = Some(
            computers
                .output(command.reference())
                .map_err(|e| e.to_string())?,
        );
        save(store, owner, &run, generation.as_deref()).await?;
    }
    let task = crate::computers::task_id(computer.id(), run.command_id.as_deref().unwrap());
    index(store, owner, &task, request, chat).await?;
    // Commands can queue while the computer boots; only file transfer waits.
    Ok(task)
}

pub(crate) async fn observe(
    app: &App,
    owner: &str,
    task: &str,
) -> Result<Option<crate::computers::Turn>, String> {
    let Some((mut run, generation)) = for_task(&app.config.chat_store, owner, task).await? else {
        return Ok(None);
    };
    let context = observer(owner).ok_or("Open the chat again to read this task.")?;
    let computers = context
        .client
        .computers(&context.session)
        .map_err(|e| e.to_string())?;
    let computer = computers
        .reference(
            run.computer
                .as_deref()
                .ok_or("This task hasn't started yet.")?,
        )
        .map_err(|e| e.to_string())?;
    let reference = computers
        .command_reference(
            &computer,
            run.command_id
                .as_deref()
                .ok_or("This task hasn't started yet.")?,
        )
        .map_err(|e| e.to_string())?;
    let cancel = CancellationToken::new();
    let mut command =
        match computers.get_command(&reference, &cancel).await {
            Ok(command) => command,
            Err(error) if error.status() == Some(404) => return Ok(Some(crate::computers::Turn {
                state: "failed".into(),
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(
                    "This task is no longer available. Check its changes before starting it again."
                        .into(),
                ),
            })),
            Err(error) => return Err(error.to_string()),
        };
    let mut expired = false;
    if let Some(reader) = run.reader.as_mut() {
        // A poll is bounded; persisted cursors resume after a server restart.
        for _ in 0..8 {
            let page = match reader.next_page(&computers, &cancel).await {
                Ok(Some(page)) => page,
                Ok(None) => break,
                Err(error) if error.status() == Some(410) => {
                    expired = true;
                    run.gap = true;
                    break;
                }
                Err(error) => return Err(error.to_string()),
            };
            run.gap |= page.stdout_gap || page.stderr_gap;
            run.stdout.extend(page.stdout);
            run.stderr.extend(page.stderr);
            command = page.command;
            if !page.has_more {
                break;
            }
        }
    }
    if expired {
        run.reader = None;
    }
    let keep_tail = |bytes: &mut Vec<u8>, max: usize| {
        if bytes.len() > max {
            bytes.drain(..bytes.len() - max);
        }
    };
    keep_tail(&mut run.stdout, 262144);
    keep_tail(&mut run.stderr, 16384);
    run.gap |= command.data().stdout_truncated || command.data().stderr_truncated;
    let outcome = command.outcome();
    save(&app.config.chat_store, owner, &run, Some(&generation)).await?;
    use openagents_sdk::computers::CommandOutcome;
    let stdout = String::from_utf8_lossy(&run.stdout);
    let complete_reply = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .is_some_and(|value| {
            value["reply"]
                .as_str()
                .or_else(|| value["result"]["reply"].as_str())
                .or_else(|| value["result"].as_str())
                .is_some_and(|reply| !reply.trim().is_empty())
        });
    let (state,error) = match outcome {
        Some(CommandOutcome::Succeeded) if !run.gap && complete_reply => ("done",None),
        Some(CommandOutcome::Succeeded) if !complete_reply && !run.gap => ("failed",Some("The task ended without an answer. Check its changes before starting it again.".into())),
        Some(CommandOutcome::Unknown) => ("failed",Some("We couldn't confirm how this task ended. Check its changes before starting it again.".into())),
        Some(CommandOutcome::Cancelled) => ("failed",Some("Stopped.".into())),
        Some(CommandOutcome::TimedOut) => ("failed",Some("This task ran out of time.".into())),
        Some(CommandOutcome::Succeeded) => ("failed",Some("The output is incomplete. Check this task's changes before starting it again.".into())),
        Some(CommandOutcome::Failed) => ("failed",Some("This task couldn't finish. Check its output and try again.".into())),
        None => ("running",None),
    };
    Ok(Some(crate::computers::Turn {
        state: state.into(),
        exit_code: command.data().exit_code,
        stdout: String::from_utf8_lossy(&run.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&run.stderr).into_owned(),
        error,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::get};
    use serde_json::json;

    async fn context() -> (Context, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new().route(
            "/v1/account",
            get(|| async {
                Json(json!({"account":{"id":"acct_fixture","label":"Test"},"workspaces":[]}))
            }),
        );
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client =
            Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
        let session = client
            .authenticate(&format!("sess_{}", "a".repeat(64)))
            .await
            .unwrap();
        (
            Context {
                client,
                session,
                workspace: None,
            },
            task,
        )
    }

    #[tokio::test]
    async fn saved_requests_survive_restart_without_credentials_or_cross_account_access() {
        use openagents_sdk::computers::{CommandOptions, CreateOptions};
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().join("chats"));
        let (context, task) = context().await;
        let computers = context.client.computers(&context.session).unwrap();
        let create = computers
            .create_request(CreateOptions {
                idempotency_key: Some("create-one".into()),
                ..Default::default()
            })
            .unwrap();
        let computer = computers.reference("cmp_0123456789abcdef0123").unwrap();
        let command = computers
            .command_request(
                &computer,
                CommandOptions {
                    command: "echo test".into(),
                    idempotency_key: Some("command-one".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let run = Run {
            v: "openagents.web.computer-run.v1".into(),
            account: context.session.account().id.clone(),
            workspace: None,
            chat: "11111111-1111-4111-8111-111111111111".into(),
            request: "22222222-2222-4222-8222-222222222222".into(),
            create,
            input_digest: "input-one".into(),
            environment: "environment-one".into(),
            repository: "example/repository".into(),
            version: 1,
            title: "Test".into(),
            computer: Some(computer.id().into()),
            command: Some(command),
            credentials_prepared: false,
            command_id: None,
            reader: None,
            uploads: Vec::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            gap: false,
        };
        let owner = crate::chat_store::account_owner(&run.account);
        let generation = save(&store, &owner, &run, None).await.unwrap();
        drop(store);
        let store = Store::local(dir.path().join("chats"));
        let (restored, read_generation) = read(&store, &owner, &run.chat, &run.request)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read_generation, generation);
        assert_eq!(restored.create.key().as_str(), "create-one");
        assert_eq!(
            restored.command.as_ref().unwrap().key().as_str(),
            "command-one"
        );
        assert!(save(&store, &owner, &restored, None).await.is_err());
        let other = crate::chat_store::account_owner("acct_other");
        assert!(
            read(&store, &other, &run.chat, &run.request)
                .await
                .unwrap()
                .is_none()
        );
        assert!(save(&store, &other, &run, None).await.is_err());
        assert!(!serde_json::to_string(&restored).unwrap().contains("sess_"));
        task.abort();
    }

    #[tokio::test]
    async fn long_agent_progress_keeps_the_complete_final_answer_and_exit_status() {
        let dir = tempfile::tempdir().unwrap();
        let script = bounded_agent_command(
            "i=0; while [ $i -lt 12000 ]; do printf 'progress %s\\n' \"$i\"; i=$((i+1)); done; printf '%s\\n' '{\"reply\":\"finished fixture\"}'",
        );
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(script)
            .env("HOME", dir.path())
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
        assert!(output.stdout.len() < 65536);
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            stdout.lines().last(),
            Some("{\"reply\":\"finished fixture\"}")
        );
        let failed = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(bounded_agent_command("echo 'failed fixture'; exit 7"))
            .env("HOME", dir.path())
            .output()
            .await
            .unwrap();
        assert_eq!(failed.status.code(), Some(7));
        assert_eq!(
            std::fs::read_dir(dir.path().join(".oa-chat"))
                .unwrap()
                .count(),
            0
        );
    }

    #[tokio::test]
    async fn lost_command_reply_recovers_one_execution_and_deletes_its_saved_records() {
        use axum::{
            extract::State,
            http::{HeaderMap, StatusCode},
            response::IntoResponse,
            routing::post,
        };
        use openagents_sdk::computers::{CommandOptions, CreateOptions, RetryPolicy};
        #[derive(Default)]
        struct Wire {
            keys: Vec<String>,
            effects: usize,
        }
        async fn execute(
            State(wire): State<std::sync::Arc<Mutex<Wire>>>,
            headers: HeaderMap,
        ) -> axum::response::Response {
            let key = headers["idempotency-key"].to_str().unwrap().to_owned();
            let mut wire = wire.lock().unwrap();
            if !wire.keys.contains(&key) {
                wire.effects += 1;
            }
            wire.keys.push(key);
            if wire.keys.len() == 1 {
                return StatusCode::BAD_GATEWAY.into_response();
            }
            Json(json!({"id":"trn_0123456789abcdef0123","computer":"cmp_0123456789abcdef0123","state":"running","exit_code":null,"digest":"a".repeat(64),"cancel_requested":false,"output_limit_bytes":262144,"output_expires_unix":2000000000,"stdout_truncated":false,"stderr_truncated":false})).into_response()
        }
        fn unknown() -> serde_json::Value {
            json!({"id":"trn_0123456789abcdef0123","computer":"cmp_0123456789abcdef0123","state":"unknown","exit_code":null,"digest":"a".repeat(64),"cancel_requested":false,"output_limit_bytes":262144,"output_expires_unix":2000000000,"stdout_truncated":false,"stderr_truncated":false})
        }
        let wire = std::sync::Arc::new(Mutex::new(Wire::default()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new()
            .route(
                "/v1/account",
                get(|| async {
                    Json(json!({"account":{"id":"acct_recovery","label":"Test"},"workspaces":[]}))
                }),
            )
            .route("/v1/computers/{id}/commands", post(execute))
            .route("/v1/computers/{id}/commands/{command}",get(|| async { Json(unknown()) }))
            .route("/v1/computers/{id}/commands/{command}/output",get(|| async { Json(json!({"command":unknown(),"encoding":"base64","stdout":"","stderr":"","next_cursor":{"stdout":0,"stderr":0},"gaps":{"stdout":false,"stderr":false},"terminal":true,"has_more":false})) }))
            .with_state(wire.clone());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let deployment = Deployment::new(&format!("{origin}/v1"), &origin).unwrap();
        let client = Client::new(deployment.clone()).unwrap();
        let session = client
            .authenticate(&format!("sess_{}", "a".repeat(64)))
            .await
            .unwrap();
        let computers = client
            .computers(&session)
            .unwrap()
            .with_retries(RetryPolicy::none())
            .unwrap();
        let computer = computers.reference("cmp_0123456789abcdef0123").unwrap();
        let request = computers
            .command_request(
                &computer,
                CommandOptions {
                    command: "echo effect".into(),
                    idempotency_key: Some("one-logical-command".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let run = Run {
            v: "openagents.web.computer-run.v1".into(),
            account: session.account().id.clone(),
            workspace: None,
            chat: "33333333-3333-4333-8333-333333333333".into(),
            request: "44444444-4444-4444-8444-444444444444".into(),
            create: computers.create_request(CreateOptions::default()).unwrap(),
            input_digest: "fixture".into(),
            environment: "environment-fixture".into(),
            repository: "example/fixture".into(),
            version: 1,
            title: "Test recovery".into(),
            computer: Some(computer.id().into()),
            command: Some(request.clone()),
            credentials_prepared: true,
            command_id: None,
            reader: None,
            uploads: Vec::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            gap: false,
        };
        let owner = crate::chat_store::account_owner(&run.account);
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().join("chats"));
        let chat: crate::chat_store::Conversation = serde_json::from_value(json!({"id":run.chat,"owner":owner,"revision":1,"title":"Test recovery","messages":[],"pending":null,"requests":[],"updated_unix":crate::chat_store::now_unix()})).unwrap();
        store.create(&chat).await.unwrap();
        save(&store, &owner, &run, None).await.unwrap();
        assert!(
            computers
                .execute(&request, &CancellationToken::new())
                .await
                .is_err()
        );
        drop(store);
        drop(client);
        let mut config = crate::Config::development(dir.path().join("tasks"));
        config.chat_store = std::sync::Arc::new(Store::local(dir.path().join("chats")));
        let app = App(std::sync::Arc::new(crate::Inner { config }));
        let client = Client::new(deployment).unwrap();
        let session = client
            .authenticate(&format!("sess_{}", "a".repeat(64)))
            .await
            .unwrap();
        OBSERVERS
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .insert(
                owner.clone(),
                (
                    Context {
                        client,
                        session,
                        workspace: None,
                    },
                    Instant::now() + OBSERVER_LIFETIME,
                ),
            );
        recover(&app, &owner, &run.chat).await.unwrap();
        recover(&app, &owner, &run.chat).await.unwrap();
        let loaded = app
            .config
            .chat_store
            .load(&owner, &run.chat)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.conversation.tasks.len(), 1);
        let task = &loaded.conversation.tasks[0].id;
        assert!(
            for_task(&app.config.chat_store, &owner, task)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(wire.lock().unwrap().effects, 1);
        assert_eq!(
            wire.lock().unwrap().keys,
            vec!["one-logical-command", "one-logical-command"]
        );
        let observed = observe(&app, &owner, task).await.unwrap().unwrap();
        assert_eq!(observed.state, "failed");
        assert!(
            observed
                .error
                .as_deref()
                .unwrap()
                .contains("couldn't confirm")
        );
        app.config.chat_store.remove(&loaded).await.unwrap();
        assert!(
            for_task(&app.config.chat_store, &owner, task)
                .await
                .unwrap()
                .is_none()
        );
        OBSERVERS.get().unwrap().lock().unwrap().remove(&owner);
        server.abort();
    }
}

/// Recover a command whose HTTP reply or chat write was interrupted. Only a
/// saved keyed command may be resent, under freshly reauthorized account access.
pub(crate) async fn recover(app: &App, owner: &str, chat: &str) -> Result<(), String> {
    let Some(context) = observer(owner) else {
        return Ok(());
    };
    let store = &app.config.chat_store;
    let Some(loaded) = store.load(owner, chat).await.map_err(|e| e.to_string())? else {
        return Ok(());
    };
    if loaded.conversation.deleted() || loaded.conversation.archived_unix.is_some() {
        return Ok(());
    }
    let folder = Store::owner_key(owner, &format!("files/{chat}")).map_err(|e| e.to_string())?;
    let keys = store
        .folder_keys_limit(&folder, crate::chat_store::MAX_FOLDER)
        .await
        .map_err(|e| e.to_string())?;
    let computers = context
        .client
        .computers(&context.session)
        .map_err(|e| e.to_string())?;
    for key in keys.into_iter().filter(|key| {
        key.rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("computer-run-") && name.ends_with(".record"))
    }) {
        let Some((bytes, generation)) = store.read_key(&key).await.map_err(|e| e.to_string())?
        else {
            continue;
        };
        let mut run: Run =
            serde_json::from_slice(&bytes).map_err(|_| "We couldn't read this task.")?;
        if run.account != context.session.account().id || run.chat != chat {
            return Err("This task belongs to another account.".into());
        }
        if run.workspace != context.workspace {
            continue;
        }
        let Some(request) = run.command.as_ref() else {
            continue;
        };
        if !run.credentials_prepared {
            continue;
        }
        if run.command_id.is_none() {
            let command = computers
                .execute(request, &CancellationToken::new())
                .await
                .map_err(|e| e.to_string())?;
            run.command_id = Some(command.reference().id().into());
            run.reader = Some(
                computers
                    .output(command.reference())
                    .map_err(|e| e.to_string())?,
            );
            save(store, owner, &run, Some(&generation)).await?;
        }
        let id = crate::computers::task_id(
            run.computer
                .as_deref()
                .ok_or("We couldn't read this task.")?,
            run.command_id.as_deref().unwrap(),
        );
        index(store, owner, &id, &run.request, chat).await?;
        for _ in 0..4 {
            let Some(loaded) = store.load(owner, chat).await.map_err(|e| e.to_string())? else {
                break;
            };
            if loaded.conversation.tasks.iter().any(|task| task.id == id) {
                break;
            }
            if loaded.conversation.deleted() || loaded.conversation.archived_unix.is_some() {
                let _ = cancel(app, owner, &id).await;
                break;
            }
            let mut next = loaded.conversation.clone();
            next.revision += 1;
            next.updated_unix = crate::chat_store::now_unix();
            next.environment = Some(crate::chat_store::ChatEnvironment {
                id: run.environment.clone(),
                repository: run.repository.clone(),
                version: Some(run.version),
                removed: false,
            });
            let at = next.messages.len();
            next.tasks.push(crate::chat_store::ChatTask {
                id: id.clone(),
                kind: crate::chat_store::TaskKind::Computer,
                environment: run.environment.clone(),
                title: run.title.clone(),
                state: crate::chat_store::TaskState::Working,
                started_unix: next.updated_unix,
                after_message: at,
                version: Some(run.version),
                finished_unix: None,
                agent: None,
                steps: Vec::new(),
            });
            if store.compare_and_swap(&loaded, &next).await.is_ok() {
                break;
            }
        }
    }
    Ok(())
}

pub(crate) async fn abandon(app: &App, owner: &str, task: &str) {
    if let Ok(Some((run, _))) = for_task(&app.config.chat_store, owner, task).await {
        if let Ok(Some(loaded)) = app.config.chat_store.load(owner, &run.chat).await {
            if !loaded.conversation.deleted() && loaded.conversation.archived_unix.is_none() {
                let _ = recover(app, owner, &run.chat).await;
                return;
            }
        }
    }
    match cancel(app, owner, task).await {
        Ok(false) => {
            if let Some((computer, turn)) = crate::computers::split_task_id(task)
                && let Some(client) = crate::computers::client()
            {
                let _ = crate::computers::stop_turn(client, owner, computer, turn).await;
            }
        }
        _ => {}
    }
}

pub(crate) fn upload_path(request: &str, index: usize, name: &str) -> String {
    let name: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .take(80)
        .collect();
    format!(
        "chat-{request}-{index}-{}",
        if name.is_empty() { "file" } else { &name }
    )
}

pub(crate) async fn archive(
    app: &App,
    headers: &HeaderMap,
    owner: &str,
    chat: &str,
) -> Result<(), String> {
    let context = authorize(app, headers, owner).await?;
    let computers = context
        .client
        .computers(&context.session)
        .map_err(|e| e.to_string())?;
    let cancel = CancellationToken::new();
    let mut after = None;
    loop {
        let page = computers
            .list(200, after.as_deref(), &cancel)
            .await
            .map_err(|e| e.to_string())?;
        for computer in page.computers {
            if computer.data().chat.as_deref() == Some(chat) {
                computers
                    .delete(computer.reference(), &cancel)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        match page.next_cursor {
            Some(next) => after = Some(next),
            None => return Ok(()),
        }
    }
}

pub(crate) async fn purge(store: &Store, owner: &str, chat: &str) -> Result<(), String> {
    let folder = Store::owner_key(owner, &format!("files/{chat}")).map_err(|e| e.to_string())?;
    for key in store
        .folder_keys_limit(&folder, crate::chat_store::MAX_FOLDER)
        .await
        .map_err(|e| e.to_string())?
    {
        if !key
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("computer-run-") && name.ends_with(".record"))
        {
            continue;
        }
        let Some((bytes, generation)) = store.read_key(&key).await.map_err(|e| e.to_string())?
        else {
            continue;
        };
        let run: Run = serde_json::from_slice(&bytes).map_err(|_| "We couldn't read this task.")?;
        if run.chat != chat || crate::chat_store::account_owner(&run.account) != owner {
            return Err("This task belongs to another account.".into());
        }
        store
            .delete_key(&key, &generation)
            .await
            .map_err(|e| e.to_string())?;
        if let (Some(computer), Some(command)) = (run.computer, run.command_id) {
            let task = crate::computers::task_id(&computer, &command);
            let index = Store::owner_key(
                owner,
                &format!(
                    "computer-run-index/{:x}.json",
                    Sha256::digest(task.as_bytes())
                ),
            )
            .map_err(|e| e.to_string())?;
            if let Some((_, generation)) =
                store.read_key(&index).await.map_err(|e| e.to_string())?
            {
                store
                    .delete_key(&index, &generation)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
