//! Memory on the account (#11182): the notes Coder keeps about the person
//! and their projects (`coder-new` `memory`), kept here while the person's
//! sync choice is on, shown and changed in Settings, Memory, and read by
//! the web chat as the person's standing context.
//!
//! | Route | What |
//! | --- | --- |
//! | `POST /coder/memory/sync` `{notes: [record]}` | Coder sends its notes and deletions; the account keeps the newer of each by id (a delete wins a tie) and answers with its whole list, `200 {notes: [record]}` |
//! | `GET /coder/memory` | The account's list, `200 {notes: [record]}`, for the apps |
//! | `PUT /coder/memory/{id}` `record` | Save one note (a new one, or a change). `200 {note}`; `422 secret`; `400 invalid` |
//! | `DELETE /coder/memory/{id}` | Delete one note: it is kept as a deletion so every computer forgets it. `200 {deleted}` |
//! | `GET /settings/memory` | The person's notes, each with Edit and Delete, and a form for a new one |
//! | `POST /settings/memory/save`, `POST /settings/memory/delete` | The page's forms |
//!
//! A record is `{id, scope, project, project_name, kind, name, description,
//! body, updated, deleted}`: `scope` is `user` (applies everywhere) or
//! `project` (one checkout, `project` its key and `project_name` its
//! folder's name), `kind` is `user`, `feedback`, `project`, or `reference`,
//! and `updated` is Unix seconds. A deletion carries only its id, scope,
//! project, and time. The `/coder/` routes take `Authorization: Bearer
//! sess_…` (Coder's or an app's own token). The list is private to the
//! account: one object beside its chats ([`Store::owner_key`]), never
//! listed or shared. A note that looks like it holds a credential is never
//! kept. Coder sends nothing while its sync choice is off.
//!
//! The web chat reads the notes that apply everywhere (scope `user`),
//! newest first ([`chat_notes`]); notes about one checkout stay with Coder.

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use openagents_chat::router::MemoryNote;
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::MarkdownRoot;
use openagents_ui::forms::{Field, Input, InputType, Select, Textarea};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::App;
use crate::chat_store::{Error, Store, account_owner, is_account_owner, now_unix};
use crate::cloud::byo::fresh_request;
use crate::cloud::protect;
use crate::cloud::session::{SessionError, Viewer};
use crate::coder_sync::{answer, line, refused, stored};
use crate::settings::{page, viewer};
use crate::ui_page::action_link;

pub(crate) const PAGE: &str = "/settings/memory";
const SAVE: &str = "/settings/memory/save";
const DELETE: &str = "/settings/memory/delete";
const API: &str = "/coder/memory";
const SYNC: &str = "/coder/memory/sync";
const SAVE_SCOPE: &str = "memory-save";
const DELETE_SCOPE: &str = "memory-delete";

/// Where the list lives, under the account's folder.
const KEY: &str = "memory/notes.json";
const SCHEMA: &str = "openagents.web.memory.v1";
/// The most notes an account keeps; a new note past it is not kept.
pub(crate) const MAX_NOTES: usize = 1000;
/// The most deletions an account remembers; the oldest go first.
const MAX_DELETIONS: usize = 1000;
/// The longest note body, as Coder keeps it.
const MAX_BODY_BYTES: usize = 8 * 1024;
const MAX_NAME_CHARS: usize = 80;
const MAX_DESCRIPTION_CHARS: usize = 300;
const MAX_PROJECT_NAME_CHARS: usize = 120;
/// The largest exchange Coder sends.
const MAX_UPLOAD: usize = 12 * 1024 * 1024;
/// How far ahead of this server's clock a note's time may be.
const FUTURE_SLACK: u64 = 24 * 60 * 60;

/// The kinds of note, as the word Coder keeps and what a person reads.
const KINDS: [(&str, &str); 4] = [
    ("user", "About you"),
    ("feedback", "How you like things done"),
    ("project", "About a project"),
    ("reference", "Where to look"),
];

/// One note, or the fact that one was deleted, as Coder's sync record has
/// it (`coder-new` `memory::SyncRecord`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Note {
    pub id: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default)]
    pub updated: u64,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Serialize, Deserialize)]
struct Record {
    schema: String,
    notes: Vec<Note>,
}

/// Coder's note ids: letters, numbers, `-`, and `_`, up to 64 bytes.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

/// Coder's project keys: one plain folder name.
fn valid_project(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 120
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn credential(text: &str) -> bool {
    secret_screen::credential_in(text).is_some()
}

/// Why a note isn't kept.
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Secret,
    Invalid(&'static str),
}

/// `note` as the account keeps it, or why not: a known id, scope, and
/// kind, one-line name and description within their bounds, a body, and
/// no text that looks like a credential. A deletion keeps only its id,
/// scope, project, and time.
fn checked(mut note: Note, now: u64) -> Result<Note, Refusal> {
    if !valid_id(&note.id) {
        return Err(Refusal::Invalid("A note has an invalid id."));
    }
    if note.updated == 0 || note.updated > now + FUTURE_SLACK {
        return Err(Refusal::Invalid("A note has an invalid time."));
    }
    match note.scope.as_str() {
        "user" => {
            note.project = None;
            note.project_name = None;
        }
        "project" => {
            if !note.project.as_deref().is_some_and(valid_project) {
                return Err(Refusal::Invalid("A project note has no project."));
            }
            note.project_name = note
                .project_name
                .as_deref()
                .map(|name| line(name, MAX_PROJECT_NAME_CHARS))
                .filter(|name| !name.is_empty() && !credential(name));
        }
        _ => return Err(Refusal::Invalid("A note's scope is user or project.")),
    }
    if note.deleted {
        note.kind = None;
        note.name = None;
        note.description = None;
        note.body = None;
        return Ok(note);
    }
    let kind = note
        .kind
        .as_deref()
        .filter(|kind| KINDS.iter().any(|(word, _)| word == kind))
        .ok_or(Refusal::Invalid("Pick what kind of note this is."))?
        .to_owned();
    let name = line(note.name.as_deref().unwrap_or_default(), MAX_NAME_CHARS);
    if name.is_empty() {
        return Err(Refusal::Invalid("Give the note a name."));
    }
    let body = note.body.as_deref().unwrap_or_default().trim().to_owned();
    if body.is_empty() {
        return Err(Refusal::Invalid("Write what to remember."));
    }
    if body.len() > MAX_BODY_BYTES {
        return Err(Refusal::Invalid("That note is longer than 8 KB."));
    }
    let description = line(
        note.description.as_deref().unwrap_or_default(),
        MAX_DESCRIPTION_CHARS,
    );
    if [&name, &description, &body]
        .iter()
        .any(|text| credential(text))
    {
        return Err(Refusal::Secret);
    }
    note.kind = Some(kind);
    note.name = Some(name);
    note.description = Some(description);
    note.body = Some(body);
    Ok(note)
}

/// Keep `incoming` when it is newer than the note with its id (a delete
/// wins a tie), or new and there is room. Returns whether anything
/// changed.
fn merge(notes: &mut Vec<Note>, incoming: Note) -> bool {
    match notes.iter_mut().find(|note| note.id == incoming.id) {
        Some(have) => {
            let newer = incoming.updated > have.updated
                || (incoming.updated == have.updated && incoming.deleted && !have.deleted);
            if !newer || *have == incoming {
                return false;
            }
            *have = incoming;
            true
        }
        None => {
            if !incoming.deleted && notes.iter().filter(|note| !note.deleted).count() >= MAX_NOTES {
                return false;
            }
            notes.push(incoming);
            true
        }
    }
}

/// Forget the oldest deletions past [`MAX_DELETIONS`].
fn prune(notes: &mut Vec<Note>) {
    let deletions = notes.iter().filter(|note| note.deleted).count();
    if deletions <= MAX_DELETIONS {
        return;
    }
    let mut times: Vec<u64> = notes
        .iter()
        .filter(|note| note.deleted)
        .map(|note| note.updated)
        .collect();
    times.sort_unstable();
    let mut over = deletions - MAX_DELETIONS;
    let cutoff = times[over - 1];
    notes.retain(|note| {
        if over > 0 && note.deleted && note.updated <= cutoff {
            over -= 1;
            false
        } else {
            true
        }
    });
}

/// Newest first.
fn newest_first(mut notes: Vec<Note>) -> Vec<Note> {
    notes.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.id.cmp(&b.id)));
    notes
}

/// Read the account's list and change it with `change` (which says
/// whether it changed anything and what to answer), retrying on a
/// concurrent write.
async fn update<T>(
    store: &Store,
    owner: &str,
    change: impl Fn(&mut Vec<Note>) -> (bool, T),
) -> Result<T, Error> {
    let key = Store::owner_key(owner, KEY)?;
    for _ in 0..4 {
        let (mut notes, generation) = match store.read_key(&key).await? {
            Some((bytes, generation)) => {
                let record: Record = serde_json::from_slice(&bytes)
                    .map_err(|_| Error::Corrupt("The memory list is invalid."))?;
                if record.schema != SCHEMA {
                    return Err(Error::Corrupt("The memory list is invalid."));
                }
                (record.notes, Some(generation))
            }
            None => (Vec::new(), None),
        };
        let (changed, result) = change(&mut notes);
        if !changed {
            return Ok(result);
        }
        prune(&mut notes);
        let bytes = serde_json::to_vec(&Record {
            schema: SCHEMA.to_owned(),
            notes,
        })
        .map_err(|_| Error::Invalid("The memory list is invalid."))?;
        match store.write_key(&key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(result),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// The account's notes and deletions, newest first.
pub(crate) async fn notes(store: &Store, owner: &str) -> Result<Vec<Note>, Error> {
    update(store, owner, |notes| (false, newest_first(notes.clone()))).await
}

/// Merge what Coder sent (records it can't keep are skipped) and answer
/// with the account's whole list, newest first.
pub(crate) async fn exchange(
    store: &Store,
    owner: &str,
    sent: &[Value],
) -> Result<Vec<Note>, Error> {
    let now = now_unix();
    let incoming: Vec<Note> = sent
        .iter()
        .filter_map(|value| serde_json::from_value::<Note>(value.clone()).ok())
        .filter_map(|note| checked(note, now).ok())
        .collect();
    update(store, owner, |notes| {
        let mut changed = false;
        for note in &incoming {
            changed |= merge(notes, note.clone());
        }
        (changed, ())
    })
    .await?;
    notes(store, owner).await
}

/// What a person changes about one note, on the page or from an app.
#[derive(Debug, Default)]
pub(crate) struct Edit {
    /// `None` makes a new note that applies everywhere.
    pub id: Option<String>,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub body: String,
}

/// The new id for a note made here, in Coder's shape.
fn new_id() -> String {
    format!("mem-{}", &fresh_request()[..16])
}

/// What became of a save.
#[derive(Debug, PartialEq, Eq)]
enum Saved {
    Saved(Note),
    Unknown,
    Full,
    Refused(Refusal),
}

/// Save `edit`: a change to an existing note keeps its scope and project
/// and is newer than what is kept; a new note applies everywhere.
async fn save(store: &Store, owner: &str, edit: &Edit) -> Result<Saved, Error> {
    update(store, owner, |notes| {
        let now = now_unix();
        let existing = edit
            .id
            .as_ref()
            .and_then(|id| notes.iter().find(|note| &note.id == id && !note.deleted));
        let note = match (&edit.id, existing) {
            (Some(_), None) => return (false, Saved::Unknown),
            (Some(id), Some(have)) => Note {
                id: id.clone(),
                scope: have.scope.clone(),
                project: have.project.clone(),
                project_name: have.project_name.clone(),
                kind: Some(edit.kind.clone()),
                name: Some(edit.name.clone()),
                description: Some(edit.description.clone()),
                body: Some(edit.body.clone()),
                updated: now.max(have.updated + 1),
                deleted: false,
            },
            (None, _) => Note {
                id: new_id(),
                scope: "user".into(),
                project: None,
                project_name: None,
                kind: Some(edit.kind.clone()),
                name: Some(edit.name.clone()),
                description: Some(edit.description.clone()),
                body: Some(edit.body.clone()),
                updated: now,
                deleted: false,
            },
        };
        let note = match checked(note, now) {
            Ok(note) => note,
            Err(refusal) => return (false, Saved::Refused(refusal)),
        };
        if merge(notes, note.clone()) {
            (true, Saved::Saved(note))
        } else if edit.id.is_none() {
            (false, Saved::Full)
        } else {
            (false, Saved::Saved(note))
        }
    })
    .await
}

/// Delete the note `id` everywhere: it is kept as a deletion newer than
/// the note. Returns false when there was no such note.
async fn forget(store: &Store, owner: &str, id: &str) -> Result<bool, Error> {
    update(store, owner, |notes| {
        let Some(have) = notes.iter().find(|note| note.id == id && !note.deleted) else {
            return (false, false);
        };
        let gone = Note {
            id: have.id.clone(),
            scope: have.scope.clone(),
            project: have.project.clone(),
            project_name: have.project_name.clone(),
            kind: None,
            name: None,
            description: None,
            body: None,
            updated: now_unix().max(have.updated),
            deleted: true,
        };
        (merge(notes, gone), true)
    })
    .await
}

/// The notes the web chat sends with a turn: the ones that apply
/// everywhere, newest first. None for a visitor who isn't signed in, or
/// when the list can't be read (the chat answers without them).
pub(crate) async fn chat_notes(store: &Store, owner: &str) -> Vec<MemoryNote> {
    if !is_account_owner(owner) {
        return Vec::new();
    }
    match notes(store, owner).await {
        Ok(notes) => notes
            .into_iter()
            .filter(|note| !note.deleted && note.scope == "user")
            .filter_map(|note| {
                Some(MemoryNote {
                    name: note.name?,
                    kind: note.kind?,
                    description: note.description.unwrap_or_default(),
                    body: note.body?,
                })
            })
            .take(openagents_chat::router::MAX_MEMORY_NOTES)
            .collect(),
        Err(error) => {
            eprintln!("openagents-web: memory: {error}");
            Vec::new()
        }
    }
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(API, get(api_list))
        .route(SYNC, post(api_sync))
        .route(
            &format!("{API}/{{id}}"),
            axum::routing::put(api_put).delete(api_delete),
        )
        .layer(DefaultBodyLimit::max(MAX_UPLOAD))
        .route(PAGE, get(memory_page))
        .route(SAVE, post(save_form))
        .route(DELETE, post(delete_form))
}

fn wire(notes: &[Note]) -> Value {
    json!({ "notes": notes })
}

async fn api_list(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match notes(&app.config.chat_store, &owner).await {
        Ok(notes) => answer(StatusCode::OK, wire(&notes)),
        Err(error) => stored(&error),
    }
}

async fn api_sync(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Some(sent) = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|body| body["notes"].as_array().cloned())
    else {
        return refused(StatusCode::BAD_REQUEST, "invalid", "Send {notes}.");
    };
    if sent.len() > MAX_NOTES + MAX_DELETIONS {
        return refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            "Send at most 2000 notes at once.",
        );
    }
    match exchange(&app.config.chat_store, &owner, &sent).await {
        Ok(notes) => answer(StatusCode::OK, wire(&notes)),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
struct Put {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    body: String,
}

async fn api_put(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Ok(sent) = serde_json::from_slice::<Put>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {kind, name, description, body}.",
        );
    };
    let edit = Edit {
        id: (id != "new").then_some(id),
        kind: sent.kind,
        name: sent.name,
        description: sent.description,
        body: sent.body,
    };
    match save(&app.config.chat_store, &owner, &edit).await {
        Ok(Saved::Saved(note)) => answer(StatusCode::OK, json!({ "note": note })),
        Ok(Saved::Unknown) => refused(StatusCode::NOT_FOUND, "unknown", "No such note."),
        Ok(Saved::Full) => refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "full",
            "Your memory has no room for more notes. Delete some first.",
        ),
        Ok(Saved::Refused(Refusal::Secret)) => refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "secret",
            "That note looks like it holds a password or key, so it wasn't saved.",
        ),
        Ok(Saved::Refused(Refusal::Invalid(message))) => {
            refused(StatusCode::BAD_REQUEST, "invalid", message)
        }
        Err(error) => stored(&error),
    }
}

async fn api_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match forget(&app.config.chat_store, &owner, &id).await {
        Ok(deleted) => answer(StatusCode::OK, json!({ "deleted": deleted })),
        Err(error) => stored(&error),
    }
}

fn problem(status: StatusCode, text: &str) -> Response {
    protect(crate::layout::problem(
        status,
        "Memory",
        text,
        (PAGE, "Memory"),
    ))
}

fn unavailable_page(error: &Error) -> Response {
    eprintln!("openagents-web: memory: {error}");
    problem(
        StatusCode::SERVICE_UNAVAILABLE,
        "Your memory can't be read right now. Try again in a minute.",
    )
}

fn target(viewer: &Viewer, request: &str) -> String {
    format!("{}:memory:{request}", viewer.account_id)
}

fn kind_label(kind: &str) -> &'static str {
    KINDS
        .iter()
        .find(|(word, _)| *word == kind)
        .map_or("Note", |(_, label)| *label)
}

async fn memory_page(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let notes = match notes(&app.config.chat_store, &owner).await {
        Ok(notes) => notes,
        Err(error) => return unavailable_page(&error),
    };
    let mut tickets = Vec::new();
    for scope in [SAVE_SCOPE, DELETE_SCOPE] {
        let request = fresh_request();
        match service.csrf(&headers, &viewer, scope, &target(&viewer, &request)) {
            Ok(csrf) => tickets.push((csrf, request)),
            Err(error) => return crate::cloud::refused(error),
        }
    }
    let live: Vec<&Note> = notes.iter().filter(|note| !note.deleted).collect();
    let body = memory_content(
        &live,
        (&tickets[0].0, &tickets[0].1),
        (&tickets[1].0, &tickets[1].1),
    );
    page(&headers, service, &viewer, "Memory", PAGE, body)
}

/// The note's fields, filled in for an edit or empty for a new one.
fn note_form(note: Option<&Note>, save: (&str, &str), prefix: &str) -> Markup {
    let name = Field::new(format!("{prefix}-name"), "Name");
    let kind = Field::new(format!("{prefix}-kind"), "Kind");
    let description =
        Field::new(format!("{prefix}-description"), "Summary").description("One line. Optional.");
    let body = Field::new(format!("{prefix}-body"), "What to remember");
    let mut select = Select::new("kind").aria(kind.aria());
    for (word, label) in KINDS {
        select = select.option(word, label);
    }
    select = select.selected(note.and_then(|n| n.kind.as_deref()).unwrap_or("user"));
    html! {
        form method="post" action=(SAVE) autocomplete="off" {
            input type="hidden" name="csrf" value=(save.0);
            input type="hidden" name="request" value=(save.1);
            input type="hidden" name="id" value=(note.map_or("", |n| n.id.as_str()));
            (name.clone().control(
                Input::new("name")
                    .input_type(InputType::Text)
                    .maxlength(MAX_NAME_CHARS as u32)
                    .value(note.and_then(|n| n.name.clone()).unwrap_or_default())
                    .required(true)
                    .aria(name.aria()),
            ))
            (kind.clone().control(select))
            (description.clone().control(
                Input::new("description")
                    .input_type(InputType::Text)
                    .maxlength(MAX_DESCRIPTION_CHARS as u32)
                    .value(note.and_then(|n| n.description.clone()).unwrap_or_default())
                    .aria(description.aria()),
            ))
            (body.clone().control(
                Textarea::new("body")
                    .value(note.and_then(|n| n.body.clone()).unwrap_or_default())
                    .rows(5)
                    .maxlength(MAX_BODY_BYTES as u32)
                    .required(true)
                    .aria(body.aria()),
            ))
            p { (Button::new(if note.is_some() { "Save" } else { "Add note" }).kind(ButtonType::Submit)) }
        }
    }
}

/// The page: what memory is, the notes by where they apply, and a form
/// for a new note.
fn memory_content(notes: &[&Note], save: (&str, &str), delete: (&str, &str)) -> Markup {
    let everywhere: Vec<&Note> = notes
        .iter()
        .copied()
        .filter(|note| note.scope == "user")
        .collect();
    let mut projects: Vec<(String, Vec<&Note>)> = Vec::new();
    for note in notes.iter().copied().filter(|note| note.scope == "project") {
        let label = note
            .project_name
            .clone()
            .unwrap_or_else(|| "A project".to_owned());
        match projects.iter_mut().find(|(name, _)| *name == label) {
            Some((_, list)) => list.push(note),
            None => projects.push((label, vec![note])),
        }
    }
    let row = |note: &Note| {
        html! {
            div class="oa-settings-row" {
                div class="oa-settings-text" {
                    span class="oa-settings-label" { (note.name.as_deref().unwrap_or_default()) }
                    span class="oa-settings-hint" {
                        (kind_label(note.kind.as_deref().unwrap_or_default()))
                        @if let Some(description) = note.description.as_deref().filter(|d| !d.is_empty()) {
                            " · " (description)
                        }
                    }
                    details {
                        summary { "Edit" }
                        (note_form(Some(note), save, &format!("memory-{}", note.id)))
                    }
                }
                div class="oa-settings-control" {
                    form method="post" action=(DELETE) {
                        input type="hidden" name="csrf" value=(delete.0);
                        input type="hidden" name="request" value=(delete.1);
                        input type="hidden" name="id" value=(note.id);
                        (Button::new("Delete")
                            .kind(ButtonType::Submit)
                            .variant(ButtonVariant::Soft)
                            .color(Color::Secondary))
                    }
                }
            }
        }
    };
    html! {
        p { (action_link("Settings", crate::settings::PAGE)) }
        (MarkdownRoot::new(html! {
            h1 { "Memory" }
            p {
                "What Coder remembers about you and your projects. Notes reach your account from computers where you turned sync on (type "
                code { "/sync" } " in Coder), and only you can see them."
            }
            p { "The chat here uses the notes that apply everywhere. Changes and deletions here reach Coder the next time it syncs." }
        }))
        @if notes.is_empty() {
            p { "You have no notes yet. Tell Coder \"remember …\", or add one below." }
        }
        @if !everywhere.is_empty() {
            section class="oa-settings-group" aria-labelledby="memory-everywhere" {
                h2 #memory-everywhere { "Everywhere" }
                @for note in &everywhere { (row(*note)) }
            }
        }
        @for (index, (name, list)) in projects.iter().enumerate() {
            @let heading = format!("memory-project-{index}");
            section class="oa-settings-group" aria-labelledby=(heading) {
                h2 id=(heading) { (name) }
                @for note in list { (row(*note)) }
            }
        }
        section class="oa-settings-group" aria-labelledby="memory-add" {
            h2 #memory-add { "Add a note" }
            (note_form(None, save, "memory-new"))
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveForm {
    csrf: String,
    request: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    body: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteForm {
    csrf: String,
    request: String,
    id: String,
}

async fn save_form(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<SaveForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        SAVE_SCOPE,
        &target(&viewer, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let owner = account_owner(&viewer.account_id);
    let edit = Edit {
        id: Some(form.id).filter(|id| !id.is_empty()),
        kind: form.kind,
        name: form.name,
        description: form.description,
        body: form.body,
    };
    match save(&app.config.chat_store, &owner, &edit).await {
        Ok(Saved::Saved(_)) => protect(Redirect::to(PAGE).into_response()),
        Ok(Saved::Unknown) => problem(
            StatusCode::NOT_FOUND,
            "That note was deleted, maybe on another computer.",
        ),
        Ok(Saved::Full) => problem(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Your memory has no room for more notes. Delete some first.",
        ),
        Ok(Saved::Refused(Refusal::Secret)) => problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "That note looks like it holds a password or key, so it wasn't saved.",
        ),
        Ok(Saved::Refused(Refusal::Invalid(message))) => problem(StatusCode::BAD_REQUEST, message),
        Err(error) => unavailable_page(&error),
    }
}

async fn delete_form(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<DeleteForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        DELETE_SCOPE,
        &target(&viewer, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let owner = account_owner(&viewer.account_id);
    match forget(&app.config.chat_store, &owner, &form.id).await {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(error) => unavailable_page(&error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> String {
        account_owner("acct_memory")
    }

    fn note(id: &str, body: &str, updated: u64) -> Value {
        json!({"id": id, "scope": "user", "kind": "feedback", "name": "Tabs",
               "description": "Indent with tabs.", "body": body, "updated": updated})
    }

    #[tokio::test]
    async fn coder_notes_merge_by_id_and_a_delete_wins_a_tie() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let listed = exchange(&store, &owner(), &[note("mem-1", "Use tabs.", 100)])
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].body.as_deref(), Some("Use tabs."));
        // An older change doesn't replace a newer one.
        let listed = exchange(&store, &owner(), &[note("mem-1", "Old.", 50)])
            .await
            .unwrap();
        assert_eq!(listed[0].body.as_deref(), Some("Use tabs."));
        // A delete at the same time wins.
        let gone = json!({"id": "mem-1", "scope": "user", "updated": 100, "deleted": true});
        let listed = exchange(&store, &owner(), &[gone]).await.unwrap();
        assert!(listed[0].deleted && listed[0].body.is_none());
        assert!(chat_notes(&store, &owner()).await.is_empty());
        // Another account sees none of it.
        assert!(
            notes(&store, &account_owner("acct_other"))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn records_that_cannot_be_kept_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        // Assembled at run time so no credential-shaped literal sits here.
        let key = format!("sk-ant-{}", "a1".repeat(20));
        let sent = [
            note("mem-ok", "Fine.", 10),
            note("mem-key", &format!("use {key}"), 10),
            note("../bad", "x", 10),
            json!({"id": "mem-p", "scope": "project", "kind": "project", "name": "P", "body": "x", "updated": 10}),
            json!({"id": "mem-q", "scope": "project", "project": "repo-ab12", "project_name": "repo",
                   "kind": "project", "name": "Builds", "body": "make all", "updated": 11}),
            note("mem-future", "x", now_unix() + FUTURE_SLACK + 60),
        ];
        let listed = exchange(&store, &owner(), &sent).await.unwrap();
        let ids: Vec<&str> = listed.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["mem-q", "mem-ok"]);
        // The chat reads only the notes that apply everywhere.
        let chat = chat_notes(&store, &owner()).await;
        assert_eq!(chat.len(), 1);
        assert_eq!(chat[0].name, "Tabs");
        // A visitor who isn't signed in has none.
        assert!(chat_notes(&store, "browser-owner").await.is_empty());
    }

    #[tokio::test]
    async fn the_page_edits_and_deletes_and_coder_hears_of_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        exchange(&store, &owner(), &[note("mem-1", "Use tabs.", 100)])
            .await
            .unwrap();
        let edit = Edit {
            id: Some("mem-1".into()),
            kind: "feedback".into(),
            name: "Tabs".into(),
            description: String::new(),
            body: "Use two spaces.".into(),
        };
        let Saved::Saved(saved) = save(&store, &owner(), &edit).await.unwrap() else {
            panic!("not saved");
        };
        assert!(saved.updated > 100);
        assert_eq!(saved.description.as_deref(), Some(""));
        // Coder's older copy doesn't undo the edit; the answer carries it.
        let listed = exchange(&store, &owner(), &[note("mem-1", "Use tabs.", 100)])
            .await
            .unwrap();
        assert_eq!(listed[0].body.as_deref(), Some("Use two spaces."));
        let new = Edit {
            kind: "user".into(),
            name: "Time zone".into(),
            body: "Central.".into(),
            ..Edit::default()
        };
        let Saved::Saved(made) = save(&store, &owner(), &new).await.unwrap() else {
            panic!("not saved");
        };
        assert!(made.id.starts_with("mem-") && made.scope == "user");
        assert!(forget(&store, &owner(), "mem-1").await.unwrap());
        assert!(!forget(&store, &owner(), "mem-1").await.unwrap());
        let listed = notes(&store, &owner()).await.unwrap();
        let gone = listed.iter().find(|n| n.id == "mem-1").unwrap();
        assert!(gone.deleted && gone.updated >= saved.updated);
        // An edit of a deleted note is refused, and a bad one says why.
        assert_eq!(save(&store, &owner(), &edit).await.unwrap(), Saved::Unknown);
        let empty = Edit {
            kind: "user".into(),
            name: "Empty".into(),
            ..Edit::default()
        };
        assert!(matches!(
            save(&store, &owner(), &empty).await.unwrap(),
            Saved::Refused(Refusal::Invalid(_))
        ));
        let chat = chat_notes(&store, &owner()).await;
        assert_eq!(chat.len(), 1);
        assert_eq!(chat[0].body, "Central.");
    }

    #[test]
    fn the_oldest_deletions_go_first() {
        let mut notes: Vec<Note> = (0..(MAX_DELETIONS as u64 + 5))
            .map(|n| Note {
                id: format!("mem-{n}"),
                scope: "user".into(),
                project: None,
                project_name: None,
                kind: None,
                name: None,
                description: None,
                body: None,
                updated: n + 1,
                deleted: true,
            })
            .collect();
        prune(&mut notes);
        assert_eq!(notes.len(), MAX_DELETIONS);
        assert!(notes.iter().all(|n| n.updated > 5));
    }

    #[test]
    fn the_page_shows_notes_by_where_they_apply() {
        let everywhere = checked(
            serde_json::from_value(note("mem-1", "Use tabs.", 10)).unwrap(),
            100,
        )
        .unwrap();
        let project = checked(
            serde_json::from_value(
                json!({"id": "mem-2", "scope": "project", "project": "repo-1",
                "project_name": "openagents", "kind": "project", "name": "Builds",
                "body": "cargo build", "updated": 9}),
            )
            .unwrap(),
            100,
        )
        .unwrap();
        let html = memory_content(&[&everywhere, &project], ("c", "r"), ("c2", "r2")).into_string();
        assert!(html.contains("Everywhere") && html.contains("openagents"));
        assert!(html.contains("How you like things done · Indent with tabs."));
        assert!(html.contains("cargo build") && html.contains("Add a note"));
    }
}
