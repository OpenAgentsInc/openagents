//! A frozen source and one linked destination, using the chat store's existing CAS.
use crate::continuation_receipts as receipts;
use crate::{
    App,
    chat_store::{Conversation, Role},
    coder_sync::{account, answer, refused, stored},
};
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
};
use chat_home::{Continued, Home, PartRef, Seed, TransferClaim};

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/v1/threads/{id}/continue", post(claim))
}
pub(crate) fn home(chat: &Conversation) -> Home {
    match &chat.terminal {
        None => Home::Cloud,
        Some(_) => Home::Computer(format!("legacy-{}", chat.id)),
    }
}
pub(crate) fn seed(chat: &Conversation) -> Seed {
    Seed::new(
        &chat.title,
        chat.messages.iter().filter_map(|m| {
            let role = match m.role {
                Role::User => chat_home::Role::User,
                Role::Assistant => chat_home::Role::Assistant,
                _ => return None,
            };
            Some((role, m.text.as_str()))
        }),
    )
}
async fn claim(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: axum::body::Bytes,
) -> Response {
    let account = match account(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let caller = crate::chat_store::account_owner(&account);
    let request = match serde_json::from_slice::<TransferClaim>(&body) {
        Ok(v)
            if v.target.valid()
                && v.seed.as_ref().is_none_or(Seed::valid)
                && v.target.id != id
                && v.revision > 0 =>
        {
            v
        }
        _ => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Choose a computer and a new conversation.",
            );
        }
    };
    if !crate::chat_store::valid_id(&id) {
        return refused(StatusCode::NOT_FOUND, "not_found", "No such chat.");
    }
    for _ in 0..6 {
        let loaded = match app.config.chat_store.load(&caller, &id).await {
            Ok(Some(v)) if !v.conversation.deleted() => v,
            Ok(None) => return claim_computer(&app, &account, &caller, &id, &request).await,
            Ok(_) => return refused(StatusCode::NOT_FOUND, "not_found", "No such chat."),
            Err(e) => return stored(&e),
        };
        let chat = &loaded.conversation;
        if request.seed.is_some() || request.target.home == home(chat) {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Choose a different place to continue this chat.",
            );
        }
        if let Some(target) = &chat.links.continued_by {
            if target != &request.target {
                return refused(
                    StatusCode::CONFLICT,
                    "continued",
                    "This chat already continues elsewhere.",
                );
            }
            return finish(
                &app,
                &caller,
                Continued {
                    account: account.clone(),
                    source: PartRef {
                        id: id.clone(),
                        home: home(chat),
                    },
                    target: target.clone(),
                    seed: seed(chat),
                },
                chat.project.clone(),
            )
            .await;
        }
        if chat.working()
            || crate::pages::chat::work::running(chat)
            || chat
                .requests
                .iter()
                .any(|r| r.outcome == crate::chat_store::Outcome::Pending)
        {
            return refused(
                StatusCode::CONFLICT,
                "busy",
                "Stop the current answer before continuing elsewhere.",
            );
        }
        if chat.revision != request.revision {
            return refused(
                StatusCode::CONFLICT,
                "changed",
                "This chat changed. Open it again before continuing.",
            );
        }
        match app
            .config
            .chat_store
            .load(&caller, &request.target.id)
            .await
        {
            Ok(Some(_)) => {
                return refused(
                    StatusCode::CONFLICT,
                    "invalid",
                    "Choose a new conversation for this continuation.",
                );
            }
            Ok(None) => {}
            Err(e) => return stored(&e),
        }
        match crate::session_index::load(&app.config.chat_store, &caller).await {
            Ok(index) if index.rows.contains_key(&request.target.id) => {
                return refused(
                    StatusCode::CONFLICT,
                    "invalid",
                    "Choose a new conversation for this continuation.",
                );
            }
            Ok(_) => {}
            Err(error) => return stored(&error),
        }
        let mut next = chat.clone();
        let continued = match receipts::reserve(
            &app.config.chat_store,
            &caller,
            &Continued {
                account: account.clone(),
                source: PartRef {
                    id: id.clone(),
                    home: home(chat),
                },
                target: request.target.clone(),
                seed: seed(chat),
            },
        )
        .await
        {
            Ok(value) => value,
            Err(error) => return stored(&error),
        };
        if continued.seed != seed(chat) {
            return refused(
                StatusCode::CONFLICT,
                "changed",
                "This chat changed. Open it again before continuing.",
            );
        }
        next.links.continued_by = Some(request.target.clone());
        next.revision += 1;
        next.updated_unix = crate::chat_store::now_unix();
        match app.config.chat_store.compare_and_swap(&loaded, &next).await {
            Ok(_) => {
                return finish(&app, &caller, continued, chat.project.clone()).await;
            }
            Err(crate::chat_store::Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    refused(
        StatusCode::CONFLICT,
        "changed",
        "This chat changed. Try again.",
    )
}

async fn finish(app: &App, owner: &str, continued: Continued, project: Option<String>) -> Response {
    let continued = match receipts::reserve(&app.config.chat_store, owner, &continued).await {
        Ok(value) => value,
        Err(error) => return stored(&error),
    };
    if let Err(error) =
        receipts::ensure_cloud(&app.config.chat_store, owner, &continued, project).await
    {
        return stored(&error);
    }
    if let Err(error) = crate::session_moves::complete(
        &app.config.chat_store,
        owner,
        &continued.source.id,
        &continued.target,
    )
    .await
    {
        return stored(&error);
    }
    answer(
        StatusCode::OK,
        serde_json::to_value(continued).unwrap_or_default(),
    )
}

async fn claim_computer(
    app: &App,
    account: &str,
    owner: &str,
    id: &str,
    request: &TransferClaim,
) -> Response {
    let index = match crate::session_index::load_linked(&app.config.chat_store, owner).await {
        Ok(index) => index,
        Err(error) => return stored(&error),
    };
    let Some(row) = index.rows.get(id) else {
        return refused(StatusCode::NOT_FOUND, "not_found", "No such chat.");
    };
    let source = PartRef {
        id: id.into(),
        home: row.home(),
    };
    if request.target.home == source.home {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Choose a different place to continue this chat.",
        );
    }
    if let Some(target) = &row.continued_by {
        if target != &request.target {
            return refused(
                StatusCode::CONFLICT,
                "continued",
                "This chat already continues elsewhere.",
            );
        }
    } else {
        if !matches!(
            row.status,
            crate::session_index::Status::Idle | crate::session_index::Status::Errored
        ) {
            return refused(
                StatusCode::CONFLICT,
                "busy",
                "Stop the current answer before continuing elsewhere.",
            );
        }
        if request.revision != row.last_activity_unix.max(1) {
            return refused(
                StatusCode::CONFLICT,
                "changed",
                "This chat changed. Open it again before continuing.",
            );
        }
        if index.rows.contains_key(&request.target.id) {
            return refused(
                StatusCode::CONFLICT,
                "invalid",
                "Choose a new conversation for this continuation.",
            );
        }
        match app.config.chat_store.load(owner, &request.target.id).await {
            Ok(Some(_)) => {
                return refused(
                    StatusCode::CONFLICT,
                    "invalid",
                    "Choose a new conversation for this continuation.",
                );
            }
            Ok(None) => {}
            Err(error) => return stored(&error),
        }
    }
    let continued = match receipts::load(&app.config.chat_store, owner, &request.target.id).await {
        Ok(Some(old))
            if old.account == account
                && receipts::source_matches(&old, &source, &request.target) =>
        {
            old
        }
        Ok(Some(_)) => {
            return refused(
                StatusCode::CONFLICT,
                "invalid",
                "Choose a new conversation for this continuation.",
            );
        }
        Ok(None) => {
            let Some(seed) = &request.seed else {
                return refused(
                    StatusCode::CONFLICT,
                    "context_required",
                    "Open this chat on its computer before continuing elsewhere.",
                );
            };
            Continued {
                account: account.into(),
                source,
                target: request.target.clone(),
                seed: seed.with_recent(std::iter::empty()),
            }
        }
        Err(error) => return stored(&error),
    };
    let continued = match receipts::reserve(&app.config.chat_store, owner, &continued).await {
        Ok(value) => value,
        Err(error) => return stored(&error),
    };
    match crate::session_index::link_part(&app.config.chat_store, owner, id, &request.target).await
    {
        Ok(true) => finish(app, owner, continued, row.project.clone()).await,
        Ok(false) => refused(
            StatusCode::CONFLICT,
            "continued",
            "This chat already continues elsewhere.",
        ),
        Err(error) => stored(&error),
    }
}

pub(crate) fn check_update(
    previous: &Conversation,
    next: &Conversation,
) -> Result<(), crate::chat_store::Error> {
    fn content(chat: &Conversation) -> serde_json::Value {
        let mut value = serde_json::to_value(chat).unwrap_or_default();
        if let Some(fields) = value.as_object_mut() {
            for key in [
                "revision",
                "updated_unix",
                "title",
                "pinned_unix",
                "archived_unix",
                "project",
                "filed",
                "opened_unix",
            ] {
                fields.remove(key);
            }
        }
        value
    }
    if previous.links.continued_by.is_some()
        && (next.links != previous.links || (!next.deleted() && content(previous) != content(next)))
    {
        return Err(crate::chat_store::Error::Invalid(
            "This chat continues elsewhere.",
        ));
    }
    Ok(())
}

pub(crate) fn notice(chat: &Conversation) -> Option<maud::Markup> {
    if chat.links.continues.is_none() && chat.links.continued_by.is_none() {
        return None;
    }
    let view = chat_home::view::parts(
        &format!("parts-{}", chat.id),
        chat.revision.max(1),
        &chat.links,
    )
    .ok()?;
    let rust_native::Element::Stack { children, .. } = &view.view().root.element else {
        return None;
    };
    Some(maud::html! { p.oa-thread-notice {
        @if chat.links.continued_by.is_some() { "This chat continues elsewhere. " }
        @for node in children {
            @if let rust_native::Element::Button {label, intent: part, ..} = &node.element {
                a href=(part_href(part)) { (label) } " "
            }
        }
    } })
}

pub(crate) fn part_href(part: &PartRef) -> String {
    let path = if matches!(&part.home, Home::Computer(device) if device != &format!("legacy-{}", part.id))
    {
        "chat/sessions"
    } else {
        "chat"
    };
    format!("/{path}/{}", part.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::tests::{Browser, world};
    use axum::{
        body::{Body, to_bytes},
        http::{Request, header},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn claim_is_owner_bound_retry_safe_and_freezes_the_source() {
        let world = world().await;
        let mut alice = Browser::default();
        alice.sign_in(&world, "octo-local").await;
        let mut bob = Browser::default();
        bob.sign_in(&world, "quiet-local").await;
        let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
        let upload=serde_json::from_value(json!({"computer":"Fixture computer","title":"Earlier work","messages":[{"role":"user","text":"The fictional flower is coral orchid"},{"role":"tool","text":"never-replay-this-tool-output"},{"role":"assistant","text":"I remember the flower"}]})).unwrap();
        let session = "continued-terminal-test";
        crate::coder_sync::save(&world.store, &owner, session, &upload)
            .await
            .unwrap();
        let id = crate::coder_sync::chat_id(&owner, session);
        let loaded = world.store.load(&owner, &id).await.unwrap().unwrap();
        let mut next = loaded.conversation.clone();
        let synthetic = format!("sk-ant-{}", "synthetic-test-only-0000000000");
        next.messages.push(crate::chat_store::Message {
            role: Role::Assistant,
            text: synthetic.clone(),
            request_id: None,
        });
        next.revision += 1;
        world.store.compare_and_swap(&loaded, &next).await.unwrap();
        let revision = next.revision;
        let destination =
            json!({"id":"d75ef8a1-16ca-49b6-83be-000000000094","home":"computer:fixture-mac"});
        async fn post(
            world: &crate::projects::tests::World,
            token: &str,
            id: &str,
            body: Value,
        ) -> (u16, Value) {
            let response = world
                .site
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/v1/threads/{id}/continue"))
                        .header(header::HOST, &world.host)
                        .header(header::AUTHORIZATION, format!("Bearer {token}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status().as_u16();
            let body =
                serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                    .unwrap();
            (status, body)
        }
        let claim = json!({"revision":revision,"target":destination});
        assert_eq!(
            post(&world, &bob.0["oa_cloud_session"], &id, claim.clone())
                .await
                .0,
            404
        );
        assert_eq!(
            post(
                &world,
                &alice.0["oa_cloud_session"],
                &id,
                json!({"revision":1,"target":destination})
            )
            .await
            .0,
            409
        );
        let (status, response) =
            post(&world, &alice.0["oa_cloud_session"], &id, claim.clone()).await;
        assert_eq!(status, 200, "{response}");
        let continued: Continued = serde_json::from_value(response.clone()).unwrap();
        assert_eq!(continued.account, world.account_of("octo-local"));
        assert!(continued.seed.valid());
        let seed = serde_json::to_string(&continued.seed).unwrap();
        assert!(seed.contains("coral orchid"));
        assert!(!seed.contains("never-replay-this-tool-output") && !seed.contains(&synthetic));
        assert!(seed.contains("[redacted:"));
        let spec = crate::app_openapi::document();
        let schema =
            json!({"$ref":"#/components/schemas/ChatContinuation","components":spec["components"]});
        assert!(
            jsonschema::validator_for(&schema)
                .unwrap()
                .is_valid(&response)
        );
        assert_eq!(
            post(&world, &alice.0["oa_cloud_session"], &id, claim).await,
            (200, response)
        );
        assert_eq!(post(&world,&alice.0["oa_cloud_session"],&id,json!({"revision":revision,"target":{"id":"d75ef8a1-16ca-49b6-83be-000000000095","home":"computer:another"}})).await.0,409);
        let loaded = world.store.load(&owner, &id).await.unwrap().unwrap();
        assert_eq!(
            loaded.conversation.links.continued_by.as_ref().unwrap().id,
            continued.target.id
        );
        let mut changed = loaded.conversation.clone();
        changed.revision += 1;
        changed.messages.clear();
        assert!(
            world
                .store
                .compare_and_swap(&loaded, &changed)
                .await
                .is_err()
        );
        let changed_upload=serde_json::from_value(json!({"computer":"Fixture computer","title":"Earlier work","messages":[{"role":"user","text":"A new question after the move"}]})).unwrap();
        assert!(matches!(
            crate::coder_sync::save(&world.store, &owner, session, &changed_upload)
                .await
                .unwrap(),
            crate::coder_sync::Saved::Invalid(_)
        ));
    }
}
