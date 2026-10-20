//! Explicit publication of an agent's runtime references and selected receipts.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use nostr::{agent_public, domain::Event};
use serde_json::json;

use super::{Refusal, key, load, owner};
use crate::{
    App,
    chat_store::Error,
    coder_sync::{answer, stored},
};

/// Change the NIP-ATT references carried by the next signed definition.
pub(super) async fn set_runtime(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<agent_public::Runtime>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let publisher = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(r) => return r,
    };
    let Ok(Json(runtime)) = body else {
        return Refusal::Invalid("Send the release or endpoint event ID.".into()).response();
    };
    if runtime.validate().is_err() {
        return Refusal::Invalid("Use a full event ID for the release or endpoint.".into())
            .response();
    }
    if !agent_public::valid_id(&id) {
        return Refusal::Missing.response();
    }
    let Some(agent) = owner::signer(&app, &id) else {
        return Refusal::Missing.response();
    };
    for _ in 0..16 {
        let (mut record, generation) = match load(&app.config.chat_store, &id).await {
            Ok(Some(found)) => found,
            Ok(None) => return Refusal::Missing.response(),
            Err(e) => return stored(&e),
        };
        if record.publisher != publisher {
            return Refusal::Denied.response();
        }
        let Some(owned) = record.owner.as_mut().filter(|o| !o.revoked) else {
            return Refusal::Invalid("Set this agent's owner first.".into()).response();
        };
        let at = crate::cloud::session::now().max(owned.signed_at.saturating_add(1));
        if agent_public::check_owner_tag(&agent, &nostr::domain::Tag::new(owned.tag.clone()), at)
            .is_err()
        {
            return Refusal::Invalid("Sign a new owner tag before changing this agent.".into())
                .response();
        }
        owned.runtime = runtime.clone();
        owned.signed_at = at;
        let owned = owned.clone();
        let Some(events) = owner::events(&app, &id, &record, &owned) else {
            return Refusal::Invalid("This agent could not be published.".into()).response();
        };
        let bytes = match serde_json::to_vec(&record) {
            Ok(bytes) => bytes,
            Err(_) => return Refusal::Invalid("This agent could not be saved.".into()).response(),
        };
        match app
            .config
            .chat_store
            .write_key(&key(&id), bytes, Some(&generation))
            .await
        {
            Ok(_) => {
                let published = owner::send(&app, &events).await;
                return answer(
                    StatusCode::OK,
                    json!({"runtime": runtime, "published": published, "definition": events.last()}),
                );
            }
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    stored(&Error::Conflict)
}

/// Forward one owner-signed receipt, unchanged, after checking its agent
/// and seal. The caller selects and signs it; this route never publishes
/// a receipt merely because an execution finished.
pub(super) async fn publish_receipt(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<Event>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let publisher = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(r) => return r,
    };
    if !agent_public::valid_id(&id) {
        return Refusal::Missing.response();
    }
    let Ok(Json(event)) = body else {
        return Refusal::Invalid("Send the receipt event your owner key signed.".into()).response();
    };
    let record = match load(&app.config.chat_store, &id).await {
        Ok(Some((record, _))) => record,
        Ok(None) => return Refusal::Missing.response(),
        Err(e) => return stored(&e),
    };
    if record.publisher != publisher {
        return Refusal::Denied.response();
    }
    let Some(owned) = record.owner.as_ref().filter(|o| !o.revoked) else {
        return Refusal::Invalid("Set this agent's owner first.".into()).response();
    };
    let definition = owner::events(&app, &id, &record, owned)
        .and_then(|events| events.last().cloned())
        .and_then(|event| agent_public::read_definition(&event).ok());
    let at = crate::cloud::session::now();
    if definition
        .as_ref()
        .is_none_or(|definition| receipts_nostr::parse(&event, definition).is_err())
        || event.created_at.abs_diff(at) > 120
    {
        return Refusal::Invalid(
            "That receipt does not check out for this agent and owner.".into(),
        )
        .response();
    }
    let published = owner::send(&app, &[event.clone()]).await;
    if published == 0 {
        return crate::coder_sync::refused(
            StatusCode::BAD_GATEWAY,
            "unavailable",
            "The relays did not accept the receipt. Try again.",
        );
    }
    answer(
        StatusCode::OK,
        json!({"id": event.id, "published": published}),
    )
}
