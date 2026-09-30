//! Same-user task broker through the portable clients and normal admission.
use std::sync::Arc;

use coder_access::{Code, client::Client, protocol::Operation};
use coder_connect::protocol::{Query, Route};
use openagents_connect::{control::Reply, keys::KeyName};
use secp256k1::SecretKey;

use crate::serve::{Shared, dispatch::Dispatcher};

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
