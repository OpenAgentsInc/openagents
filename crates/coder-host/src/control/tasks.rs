//! Same-user task broker through the portable clients and normal admission.
use std::sync::Arc;

use coder_access::{Code, client::Client, protocol::Operation};
use coder_connect::protocol::{Query, Route};
use openagents_connect::{control::Reply, keys::KeyName};
use secp256k1::SecretKey;

use crate::serve::{Shared, dispatch::Dispatcher};

#[derive(serde::Serialize, serde::Deserialize)]
struct Handoff {
    request: String,
    host: String,
    task: coder_access::protocol::TaskCreate,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Import {
    request: String,
    task: coder_access::protocol::TaskCreate,
}

pub(super) async fn import(
    shared: Arc<Shared>,
    request: String,
    chat: String,
    task: coder_access::protocol::TaskCreate,
) -> Reply {
    let _serial = shared.local_handoffs.lock().await;
    if chat.len() != 32
        || !chat
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || request.len() != 64
        || !request
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || (Operation::CreateTask { task: task.clone() })
            .validate()
            .is_err()
    {
        return super::refused("malformed", "Invalid continuation task");
    }
    let plan = Import {
        request: request.clone(),
        task: task.clone(),
    };
    let worker = shared.clone();
    let id = chat.clone();
    let prepared = tokio::task::spawn_blocking(move || {
        let control = worker
            .config
            .control
            .as_ref()
            .ok_or("Local conversation storage is unavailable")?;
        let cache =
            openagents_chat::cache::Cache::open(&control.root.join("imports"), &worker.secret)?;
        let old: Option<Import> = cache.read(&id)?;
        let existing = super::chat(
            &worker,
            openagents_chat::service::Command::Read {
                chat: id.clone(),
                before: None,
            },
        );
        if let Some(old) = old {
            if old != plan {
                return Err("This continuation ID is already bound to another request".into());
            }
        } else {
            if matches!(existing, Reply::Chat { .. }) {
                return Err("Choose a fresh conversation for this continuation".into());
            }
            // Persist exact retry bytes before creating the conversation or task.
            cache.write(&id, &plan)?;
        }
        let Reply::Chat { snapshot } = super::chat(
            &worker,
            openagents_chat::service::Command::Create { chat: id.clone() },
        ) else {
            return Err("The continuation conversation could not be saved".into());
        };
        if snapshot.storage_error.is_some() {
            return Err("The continuation conversation could not be saved".into());
        }
        if !matches!(existing, Reply::Chat { .. }) {
            let Reply::Chat { snapshot } = super::chat(
                &worker,
                openagents_chat::service::Command::Rename {
                    chat: id,
                    title: plan.task.title.clone(),
                },
            ) else {
                return Err("The continuation title could not be saved".into());
            };
            if snapshot.storage_error.is_some() {
                return Err("The continuation title could not be saved".into());
            }
        }
        Ok::<_, String>(())
    })
    .await;
    match prepared {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return super::refused("unavailable", error),
        Err(_) => return super::refused("unavailable", "Coder could not prepare the continuation"),
    }
    let result = call(
        shared.clone(),
        request,
        Operation::CreateTask { task: task.clone() },
    )
    .await;
    let Reply::Task {
        outcome: coder_access::protocol::Outcome::Dispatched { receipt },
    } = result
    else {
        return result;
    };
    let worker = shared.clone();
    tokio::task::spawn_blocking(move || {
        {
            let mut state = worker
                .chats
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(chats) = state.as_mut() else {
                return super::refused(
                    "unavailable",
                    "Coder accepted the task; refresh this conversation",
                );
            };
            if let Some(binding) = chats.get(&chat).and_then(|summary| summary.coder.as_ref()) {
                if binding.host != worker.host_key
                    || binding.task != receipt.reference
                    || binding.project.as_deref() != Some(&task.workspace)
                {
                    return super::refused("chat", "This conversation is bound to another task");
                }
            } else {
                chats.spawned_in(
                    &chat,
                    &worker.host_key,
                    &receipt.reference,
                    Some(&task.workspace),
                    crate::unix_time().unwrap_or_default(),
                );
            }
        }
        super::chat(
            &worker,
            openagents_chat::service::Command::Read { chat, before: None },
        )
    })
    .await
    .unwrap_or_else(|_| {
        super::refused(
            "unavailable",
            "Coder accepted the task; refresh this conversation",
        )
    })
}

pub(super) async fn handoff(shared: Arc<Shared>, chat: String) -> Reply {
    let _serial = shared.local_handoffs.lock().await;
    let worker = shared.clone();
    let id = chat.clone();
    let prepared = tokio::task::spawn_blocking(move || prepare_handoff(&worker, &id)).await;
    let plan = match prepared {
        Ok(Ok(Some(plan))) => plan,
        Ok(Ok(None)) => {
            let worker = shared.clone();
            return tokio::task::spawn_blocking(move || {
                super::chat(
                    &worker,
                    openagents_chat::service::Command::Read { chat, before: None },
                )
            })
            .await
            .unwrap_or_else(|_| super::refused("unavailable", "Coder could not read this chat"));
        }
        Ok(Err(message)) => return super::refused("chat", message),
        Err(_) => {
            return super::refused("unavailable", "Coder could not prepare this conversation");
        }
    };
    let result = call(
        shared.clone(),
        plan.request,
        Operation::CreateTask {
            task: plan.task.clone(),
        },
    )
    .await;
    let Reply::Task {
        outcome: coder_access::protocol::Outcome::Dispatched { receipt },
    } = result
    else {
        return result;
    };
    let worker = shared.clone();
    tokio::task::spawn_blocking(move || {
        let now = crate::unix_time().unwrap_or_default();
        {
            let mut state = worker
                .chats
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(chats) = state.as_mut() else {
                return super::refused("chat", "The conversation is unavailable");
            };
            chats.spawned_in(
                &chat,
                &plan.host,
                &receipt.reference,
                Some(&plan.task.workspace),
                now,
            );
        }
        super::chat(
            &worker,
            openagents_chat::service::Command::Read { chat, before: None },
        )
    })
    .await
    .unwrap_or_else(|_| super::refused("unavailable", "Coder accepted the task; refresh this chat"))
}

fn prepare_handoff(shared: &Shared, id: &str) -> Result<Option<Handoff>, String> {
    let Reply::Chat { snapshot } = super::chat(
        shared,
        openagents_chat::service::Command::Read {
            chat: id.into(),
            before: None,
        },
    ) else {
        return Err("This conversation could not be read.".into());
    };
    if snapshot
        .chats
        .iter()
        .any(|row| row.id == id && row.archived)
    {
        return Err("Restore this chat before running Coder.".into());
    }
    if snapshot.coder.is_some() {
        return Ok(None);
    }
    if snapshot.busy || snapshot.failure.is_some() || snapshot.storage_error.is_some() {
        return Err("Wait for a complete saved reply before running Coder.".into());
    }
    if shared.config.keys.is_none() || shared.config.workspaces.is_empty() {
        return Err("Choose a project in Settings before running Coder.".into());
    }
    let control = shared
        .config
        .control
        .as_ref()
        .ok_or("This computer has no local chat storage.")?;
    let cache =
        openagents_chat::cache::Cache::open(&control.root.join("handoffs"), &shared.secret)?;
    if let Some(plan) = cache.read::<Handoff>(id)? {
        if plan.host != shared.host_key
            || !shared.config.workspaces.contains_key(&plan.task.workspace)
        {
            return Err("The saved Coder project is unavailable. Restore it in Settings.".into());
        }
        return Ok(Some(plan));
    }
    let mut state = shared
        .chats
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let chats = state.as_mut().ok_or("This conversation is unavailable.")?;
    let summary = chats
        .get(id)
        .cloned()
        .ok_or("This conversation is unavailable.")?;
    if summary.archived {
        return Err("Restore this chat before running Coder.".into());
    }
    let turns = chats.turns(id).to_vec();
    if !turns.last().is_some_and(|turn| {
        turn.role == openagents_chat::basic_coder::Role::Assistant && !turn.stopped
    }) || !openagents_chat::delegation::offered(
        turns.last().and_then(|turn| turn.meta.as_ref()),
        snapshot.computer,
    ) {
        return Err("This reply has no current Coder offer.".into());
    }
    let listed: Vec<String> = shared.config.workspaces.keys().cloned().collect();
    let project = openagents_chat::delegation::project(&listed, |label| {
        chats
            .list()
            .iter()
            .filter_map(|row| {
                row.coder
                    .as_ref()
                    .filter(|coder| {
                        coder.host == shared.host_key && coder.project.as_deref() == Some(label)
                    })
                    .map(|coder| coder.at.unwrap_or(row.updated))
            })
            .max()
    })
    .ok_or("Choose a project in Settings.")?;
    let prompt = openagents_chat::delegation::prompt(&summary.title, &turns);
    use sha2::{Digest, Sha256};
    let request = Sha256::digest(
        format!("openagents.desktop.handoff.v1:{}:{id}", shared.host_key).as_bytes(),
    )
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect();
    let plan = Handoff {
        request,
        host: shared.host_key.clone(),
        task: coder_access::client::tasks::input(&prompt, &project),
    };
    cache.write(id, &plan)?;
    Ok(Some(plan))
}

fn error(error: coder_access::Error) -> Reply {
    super::access_refused(&error)
}

pub(super) async fn call(shared: Arc<Shared>, request: String, operation: Operation) -> Reply {
    let worker = shared.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut dispatcher = Dispatcher::new(worker.clone());
        let result = task(&worker, request, operation, &mut dispatcher);
        (result, dispatcher.changed)
    })
    .await;
    let Ok((result, changed)) = result else {
        return super::refused("unavailable", "Coder could not answer the task operation");
    };
    for task in &changed {
        crate::serve::summarize(&shared, task).await;
    }
    result
        .map(|outcome| Reply::Task { outcome })
        .unwrap_or_else(error)
}

fn task(
    shared: &Shared,
    request: String,
    operation: Operation,
    dispatcher: &mut Dispatcher,
) -> coder_access::Result<coder_access::protocol::Outcome> {
    let _serial = shared
        .local_tasks
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !matches!(
        operation,
        Operation::CreateTask { .. }
            | Operation::SteerTask { .. }
            | Operation::CancelTask { .. }
            | Operation::ArchiveTask { .. }
            | Operation::CommandTask { .. }
            | Operation::QueueTask { .. }
            | Operation::ListWorkspaces {}
    ) {
        return Err(coder_access::Error::new(
            Code::Forbidden,
            "the task broker admits task operations only",
        ));
    }
    let failed = || {
        coder_access::Error::new(
            Code::Unavailable,
            "the local Coder task request could not be kept",
        )
    };
    let keys = shared.config.keys.as_ref().ok_or_else(failed)?;
    // Load only. An existing host must never mint a different owner to answer.
    let bytes = keys
        .0
        .load(KeyName::Owner)
        .map_err(|_| failed())?
        .ok_or_else(failed)?;
    let owner = SecretKey::from_byte_array(*bytes.expose()).map_err(|_| failed())?;
    let relay = shared.config.primary().map_err(|_| failed())?;
    let client = Client::owner(&shared.host_key, relay, owner, shared.config.policy)?;
    let now = coder_access::unix_time()?;
    operation.validate()?;
    let prepared = client.prepare_with_id(operation.clone(), now, request.clone())?;
    let root = shared
        .config
        .control
        .as_ref()
        .ok_or_else(failed)?
        .root
        .join("task-calls");
    let cache = openagents_chat::cache::Cache::open(&root, &shared.secret).map_err(|_| failed())?;
    let pending = match cache
        .read::<coder_access::client::Pending>(&request)
        .map_err(|_| failed())?
    {
        Some(pending) => {
            if pending.request.op != operation
                || pending.event.pubkey != coder_reach::pubkey(&owner)
            {
                return Err(coder_access::Error::new(
                    Code::Conflict,
                    "task request identity reused",
                ));
            }
            if pending.request.expires_at >= now {
                pending
            } else {
                cache.write(&request, &prepared).map_err(|_| failed())?;
                prepared
            }
        }
        None => {
            cache.write(&request, &prepared).map_err(|_| failed())?;
            prepared
        }
    };
    let reply = shared.authority.handle(&pending.event, relay, dispatcher)?;
    client.verify_reply(&pending, &reply, coder_access::unix_time()?)
}

pub(super) fn history(shared: &Shared, query: Query) -> Reply {
    match read(shared, query) {
        Ok(observation) => Reply::TaskHistory { observation },
        Err(error) => {
            let code = serde_json::to_value(error.code)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "unavailable".into());
            super::refused(&code, "Coder history could not be read")
        }
    }
}

fn read(
    shared: &Shared,
    query: Query,
) -> coder_connect::Result<coder_connect::protocol::Observation> {
    let failed = || {
        coder_connect::Error::new(
            coder_connect::ErrorCode::Unavailable,
            "Coder history is unavailable",
        )
    };
    let chats = shared.config.chats.as_ref().ok_or_else(failed)?;
    let source = chats.sources.coder.clone().ok_or_else(failed)?;
    let relay = shared.config.primary().map_err(|_| failed())?;
    let now = coder_connect::unix_time()?;
    let observer =
        coder_connect::host::Host::new(&chats.observer, shared.config.policy).coder_only();
    let mut slot = shared
        .local_history
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if slot
        .as_ref()
        .is_none_or(|client| client.connection().expires_at <= now.saturating_add(60))
    {
        let secret = SecretKey::new(&mut secp256k1::rand::rng());
        let connection = observer.pair(
            &coder_reach::pubkey(&secret),
            relay,
            coder_history::Config {
                coder: Some(source),
                ..coder_history::Config::default()
            },
            now,
            now.saturating_add(24 * 60 * 60),
        )?;
        *slot = Some(coder_connect::client::Client::new_with_policy(
            connection,
            secret,
            shared.config.policy,
        )?);
    }
    let client = slot.as_ref().ok_or_else(failed)?;
    let pending = client.prepare_for(query, now, Route::Direct)?;
    let handled = observer.handle_direct(&pending.event)?;
    let now = coder_connect::unix_time()?;
    match handled.payload {
        Some(payload) => client.verify_detached(&pending, &handled.reply, &payload, now),
        None => client.verify_reply_via(&pending, &handled.reply, now, Route::Direct),
    }
}

pub(super) fn activity(shared: &Shared, id: &str) -> Reply {
    if id.len() != 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return super::refused("malformed", "The task identity is invalid.");
    }
    let Some(task) = shared
        .tasks
        .current()
        .into_iter()
        .find(|task| task.task == id)
    else {
        return super::refused("not_found", "This task is unavailable or archived.");
    };
    let Some(summary) = crate::serve::activity(
        &shared.host_key,
        &task,
        shared.tasks.note(id),
        crate::unix_time().unwrap_or_default(),
    ) else {
        return super::refused("unavailable", "The task state could not be read.");
    };
    Reply::TaskActivity { summary }
}
