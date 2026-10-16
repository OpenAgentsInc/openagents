//! Read-only, bounded saved-chat discovery and revision-checked transcript pages.

use crate::projects::{api, shared_chats};
use crate::{
    App,
    chat_store::{Conversation, Error, account_owner},
    coder_sync::{answer, refused, stored},
};
use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/history", get(list))
        .route("/v1/history/projects", get(projects))
        .route("/v1/history/{id}/messages", get(messages))
        .route("/v1/history/{id}/files/{file}", get(file))
        .route("/v1/history/{id}/captures/{capture}", get(capture))
}

#[derive(Default, Deserialize)]
struct Page {
    after: Option<String>,
    limit: Option<usize>,
    project: Option<String>,
    revision: Option<u64>,
}
impl Page {
    fn limit(&self) -> Result<usize, Response> {
        let limit = self.limit.unwrap_or(100);
        if !(1..=200).contains(&limit) {
            return Err(invalid());
        }
        Ok(limit)
    }
}
fn invalid() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "invalid_cursor",
        "Start again from the first page.",
    )
}
fn missing() -> Response {
    refused(StatusCode::NOT_FOUND, "not_found", "No such chat or file.")
}

fn summary(chat: &Conversation) -> Value {
    json!({"id": chat.id, "title": chat.title, "revision": chat.revision,
        "updated_unix": chat.updated_unix, "pinned_unix": chat.pinned_unix,
        "archived_unix": chat.archived_unix, "project": chat.project,
        "source": match &chat.terminal { None => "web", Some(t) if crate::phone_api::phone_session(&t.session) => "phone", Some(_) => "terminal" },
        "messages": chat.messages.len()})
}

async fn list(State(app): State<App>, headers: HeaderMap, Query(page): Query<Page>) -> Response {
    let limit = match page.limit() {
        Ok(n) => n,
        Err(r) => return r,
    };
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if page
        .after
        .as_deref()
        .is_some_and(|a| !crate::chat_store::valid_id(a))
    {
        return invalid();
    }
    let result = if let Some(project) = &page.project {
        let access = match api::access(&app, &headers, project).await {
            Ok(a) => a,
            Err(r) => return r,
        };
        if !shared_chats::shared(&access.project) {
            return refused(
                StatusCode::BAD_REQUEST,
                "not_shared",
                "Read this project's chats from your account list.",
            );
        }
        shared_chats::history_page(
            &app,
            &access.project,
            &owner,
            page.after.as_deref(),
            limit,
            summary,
        )
        .await
    } else {
        app.config
            .chat_store
            .history_page(&owner, page.after.as_deref(), limit, summary)
            .await
    };
    match result {
        Ok((chats, next)) => answer(StatusCode::OK, json!({"data": chats, "next": next})),
        Err(e) => stored(&e),
    }
}

/// Project discovery without loading/counting every chat in the account.
async fn projects(
    State(app): State<App>,
    headers: HeaderMap,
    Query(page): Query<Page>,
) -> Response {
    let limit = match page.limit() {
        Ok(n) => n,
        Err(r) => return r,
    };
    let (service, token, _) = match api::caller(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let status = match api::status(service, &token).await {
        Ok(s) => s,
        Err(r) => return r,
    };
    let mut projects = status.projects;
    projects.sort_by(|a, b| a.id.cmp(&b.id));
    if page
        .after
        .as_deref()
        .is_some_and(|id| !oa_auth::repos::project_id(id))
    {
        return invalid();
    }
    let remaining: Vec<_> = projects
        .iter()
        .filter(|p| page.after.as_deref().is_none_or(|a| p.id.as_str() > a))
        .take(limit + 1)
        .collect();
    let next = (remaining.len() > limit).then(|| remaining[limit - 1].id.clone());
    let data: Vec<_> = remaining
        .into_iter()
        .take(limit)
        .map(|p| json!({"id":p.id,"name":p.name,"shared":shared_chats::shared(p)}))
        .collect();
    answer(StatusCode::OK, json!({"data":data,"next":next}))
}

/// Authorize each read; an unavailable account service or store never becomes an empty list.
async fn load(app: &App, headers: &HeaderMap, id: &str) -> Result<Conversation, Response> {
    let owner = crate::coder_sync::owner(app, headers).await?;
    if !crate::chat_store::valid_id(id) {
        return Err(missing());
    }
    match app.config.chat_store.load(&owner, id).await {
        Ok(Some(loaded)) if !loaded.conversation.deleted() => return Ok(loaded.conversation),
        Ok(_) => {}
        Err(e) => return Err(stored(&e)),
    }
    let (service, token, _) = api::caller(app, headers).await?;
    let status = api::status(service, &token).await?;
    shared_chats::find(app, &status.projects, &owner, id)
        .await
        .map_err(|e| stored(&e))?
        .map(|(loaded, _)| loaded.conversation)
        .ok_or_else(missing)
}

async fn messages(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(page): Query<Page>,
) -> Response {
    let limit = match page.limit() {
        Ok(n) => n,
        Err(r) => return r,
    };
    let chat = match load(&app, &headers, &id).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    if page.revision.is_some_and(|r| r != chat.revision) {
        return refused(
            StatusCode::CONFLICT,
            "changed",
            "This chat changed. Load it again.",
        );
    }
    let start = match page.after.as_deref() {
        None => 0,
        Some(after) => {
            if page.revision.is_none() {
                return invalid();
            }
            let Some(at) = after
                .strip_prefix("msg_")
                .and_then(|s| s.parse::<usize>().ok())
            else {
                return invalid();
            };
            if after != format!("msg_{at}") || at >= chat.messages.len() {
                return invalid();
            }
            at + 1
        }
    };
    let data: Vec<_> = chat
        .messages
        .iter()
        .enumerate()
        .skip(start)
        .take(limit)
        .map(|(at, m)| {
            let files: Vec<_> = crate::chat_files::of_message(&chat, m)
                .iter()
                .map(|f| {
                    json!({
                        "id": f.id, "name": f.name, "kind": f.kind, "size": f.size,
                    })
                })
                .collect();
            let tool = chat.terminal.as_ref().and_then(|t| t.steps.get(&at));
            json!({"id":format!("msg_{at}"),"role":m.role,"text":m.text,"files":files,"tool":tool,
            "generated_files":[], "artifacts":[]})
        })
        .collect();
    let next = (start + data.len() < chat.messages.len())
        .then(|| format!("msg_{}", start + data.len() - 1));
    answer(
        StatusCode::OK,
        json!({"thread":summary(&chat),"revision":chat.revision,"data":data,"next":next,"captures":captures(&chat)}),
    )
}

async fn file(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, file)): Path<(String, String)>,
) -> Response {
    let chat = match load(&app, &headers, &id).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    // Only saved references in this chat are readable, including through a shared project.
    if !chat
        .requests
        .iter()
        .any(|r| r.files.iter().any(|f| f.id == file))
    {
        return missing();
    }
    match crate::chat_files::read(&app.config.chat_store, &chat.owner, &id, &file).await {
        Ok(Some((found, bytes))) => crate::chat_files::file_response(&found, bytes),
        Ok(None) | Err(Error::Invalid(_)) => missing(),
        Err(e) => stored(&e),
    }
}

fn captures(chat: &Conversation) -> Vec<Value> {
    chat.terminal.as_ref().map(|t| t.asks.iter().map(|ask| {
        let (state, size, media) = match &ask.state {
            crate::chat_store::AskState::Saved {size,media} => ("saved",Some(*size),Some(media.as_str())),
            crate::chat_store::AskState::Failed {..} => ("unavailable",None,None),
            _ => ("not_saved",None,None),
        };
        json!({"id":ask.id,"name":match &ask.action {crate::chat_store::AskAction::Screenshot => "Screenshot".to_owned(), crate::chat_store::AskAction::Pull {path} => path.rsplit('/').next().unwrap_or("File").to_owned()},"state":state,"size":size,"media":media})
    }).collect()).unwrap_or_default()
}

async fn capture(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, capture)): Path<(String, String)>,
) -> Response {
    use axum::http::header;
    use axum::response::IntoResponse;
    let chat = match load(&app, &headers, &id).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    if !crate::chat_store::valid_id(&capture) {
        return missing();
    }
    if !chat.terminal.as_ref().is_some_and(|t| {
        t.asks.iter().any(|a| {
            a.id == capture && matches!(a.state, crate::chat_store::AskState::Saved { .. })
        })
    }) {
        return missing();
    }
    let key = match crate::chat_store::Store::capture_key(&chat.owner, &id, &capture) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    match app.config.chat_store.read_key(&key).await {
        Ok(Some((bytes, _))) => {
            let media = crate::coder_sync::capture_media(&bytes);
            (
                [
                    (header::CONTENT_TYPE, media),
                    (header::CACHE_CONTROL, "no-store"),
                    (header::CONTENT_DISPOSITION, "attachment"),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                ],
                bytes,
            )
                .into_response()
        }
        Ok(None) => missing(),
        Err(e) => stored(&e),
    }
}
