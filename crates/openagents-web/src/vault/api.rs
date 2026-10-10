//! The NIP-VAULT service API (tier `user`). Every route answers only the
//! signed-in account's vault; another account's ids are simply not there.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use oa_vault::index::Index;
use oa_vault::object::{self, Tier};
use oa_vault::slot::{self, Method, Slot};
use oa_vault::valid_id;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{MAX_INDEX_BYTES, MAX_OBJECT_BYTES, MAX_OBJECTS, MAX_SLOTS, store};
use crate::App;
use crate::chat_store::{Error, Store, account_owner, now_unix};

/// The form-token scope the page's requests carry.
pub(crate) const CSRF_SCOPE: &str = "vault";
/// The header the page sends its form token in.
pub(crate) const TOKEN_HEADER: &str = "x-openagents-csrf";
const SCHEMA: &str = "openagents.web.vault.v1";
/// The largest Fast answer request: up to four files of 2 MB and the words.
pub(crate) const MAX_ANSWER_BODY: usize = 12 * 1024 * 1024;
const MAX_ANSWER_FILE_BYTES: usize = 8 * 1024 * 1024;
const MAX_QUESTION_CHARS: usize = 4_000;
const MAX_ANSWER_FILES: usize = 4;
const MAX_TEXT_CHARS: usize = 200_000;

/// The account's vault: its id, its slots, and where the current index is.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub schema: String,
    pub vault: String,
    pub slots: Vec<Slot>,
    pub epoch: u32,
    /// The current index's key under the vault folder (`index/{epoch}-{nonce}`).
    pub index: String,
    pub created_unix: u64,
}

fn refusal(status: StatusCode, text: &str) -> Response {
    crate::chat_html::protect((status, Json(json!({ "error": text }))).into_response())
}

fn unavailable(error: &Error) -> Response {
    match error {
        Error::Conflict => refusal(
            StatusCode::CONFLICT,
            "Your vault changed on another device. Try again.",
        ),
        Error::Invalid(_) => refusal(StatusCode::BAD_REQUEST, "That request isn't valid."),
        _ => refusal(
            StatusCode::SERVICE_UNAVAILABLE,
            "Your vault can't be reached right now. Try again in a minute.",
        ),
    }
}

/// The account a request acts for. A browser proves the page sent it with
/// the form token (writes only); an app or Coder sends `Bearer sess_…`.
/// Returns the owner and, for a browser, a fresh form token.
pub(crate) async fn caller(
    app: &App,
    headers: &HeaderMap,
    write: bool,
) -> Result<(String, Option<String>), Response> {
    if headers.contains_key(header::AUTHORIZATION) {
        return crate::coder_sync::owner(app, headers)
            .await
            .map(|owner| (owner, None));
    }
    let service = crate::cloud::service(app).map_err(|_| {
        refusal(
            StatusCode::SERVICE_UNAVAILABLE,
            "This site doesn't offer accounts.",
        )
    })?;
    let viewer = service
        .authenticate(headers)
        .await
        .map_err(|_| refusal(StatusCode::UNAUTHORIZED, "Sign in again to use your vault."))?;
    if write {
        let token = headers
            .get(TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        service
            .verify_csrf(
                headers,
                Some(&viewer),
                CSRF_SCOPE,
                &viewer.account_id,
                token,
            )
            .map_err(|_| refusal(StatusCode::FORBIDDEN, "Reload this page and try again."))?;
    }
    let fresh = service
        .csrf(headers, &viewer, CSRF_SCOPE, &viewer.account_id)
        .ok();
    Ok((account_owner(&viewer.account_id), fresh))
}

fn key(owner: &str, rest: &str) -> Result<String, Error> {
    Store::owner_key(owner, &format!("vault/{rest}"))
}

fn object_key(owner: &str, id: &str) -> Result<String, Error> {
    key(owner, &format!("objects/{id}"))
}

/// The account's vault record and its generation.
pub(crate) async fn load(store: &Store, owner: &str) -> Result<Option<(Record, String)>, Error> {
    let Some((bytes, generation)) = store.read_key(&key(owner, "vault.json")?).await? else {
        return Ok(None);
    };
    let record: Record = serde_json::from_slice(&bytes)
        .map_err(|_| Error::Corrupt("The vault record is invalid."))?;
    if record.schema != SCHEMA || !valid_id(&record.vault) {
        return Err(Error::Corrupt("The vault record is invalid."));
    }
    Ok(Some((record, generation)))
}

async fn save(
    store: &Store,
    owner: &str,
    record: &Record,
    expected: Option<&str>,
) -> Result<String, Error> {
    let bytes =
        serde_json::to_vec(record).map_err(|_| Error::Invalid("The vault record is invalid."))?;
    store
        .write_key(&key(owner, "vault.json")?, bytes, expected)
        .await
}

/// Write a sealed index under a new unique key; returns that key.
async fn put_index(store: &Store, owner: &str, epoch: u32, blob: Vec<u8>) -> Result<String, Error> {
    let nonce: String = secp256k1::rand::random::<[u8; 8]>()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let name = format!("index/{epoch:010}-{nonce}");
    store.write_key(&key(owner, &name)?, blob, None).await?;
    Ok(name)
}

async fn delete(store: &Store, full_key: &str) -> Result<(), Error> {
    if let Some((_, generation)) = store.read_key(full_key).await? {
        store.delete_key(full_key, &generation).await?;
    }
    Ok(())
}

async fn object_ids(store: &Store, owner: &str) -> Result<Vec<String>, Error> {
    Ok(store
        .folder_keys(&key(owner, "objects")?)
        .await?
        .into_iter()
        .filter_map(|key| key.rsplit('/').next().map(str::to_owned))
        .filter(|id| valid_id(id))
        .collect())
}

fn decode(text: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(text).ok()
}

fn ok(status: StatusCode, body: Value, fresh: Option<String>) -> Response {
    let response = if status == StatusCode::NO_CONTENT {
        status.into_response()
    } else {
        (status, Json(body)).into_response()
    };
    let mut response = crate::chat_html::protect(response);
    if let Some(token) = fresh.and_then(|token| HeaderValue::from_str(&token).ok()) {
        response.headers_mut().insert(TOKEN_HEADER, token);
    }
    response
}

/// `GET /vault/api/state`.
pub(crate) async fn state(State(app): State<App>, headers: HeaderMap) -> Response {
    let (owner, fresh) = match caller(&app, &headers, false).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let store = store(&app);
    let (mut record, generation) = match load(store, &owner).await {
        Ok(Some(found)) => found,
        Ok(None) => return ok(StatusCode::OK, json!({ "vault": null }), fresh),
        Err(error) => return unavailable(&error),
    };
    // Expired pairing links go.
    let now = now_unix();
    if record.slots.iter().any(|slot| slot.expired(now)) {
        record.slots.retain(|slot| !slot.expired(now));
        let _ = save(store, &owner, &record, Some(&generation)).await;
    }
    let blob = match key(&owner, &record.index) {
        Ok(full) => match store.read_key(&full).await {
            Ok(Some((bytes, _))) => bytes,
            Ok(None) => {
                return refusal(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Your vault's file list is missing. Try again in a minute.",
                );
            }
            Err(error) => return unavailable(&error),
        },
        Err(error) => return unavailable(&error),
    };
    let objects = match object_ids(store, &owner).await {
        Ok(ids) => ids,
        Err(error) => return unavailable(&error),
    };
    ok(
        StatusCode::OK,
        json!({
            "vault": {
                "id": record.vault,
                "slots": record.slots,
                "index": {
                    "epoch": record.epoch,
                    "blob": base64::engine::general_purpose::STANDARD.encode(&blob),
                },
                "objects": objects,
            }
        }),
        fresh,
    )
}

fn check_slot(slot: &Slot, vault: &str) -> Result<(), &'static str> {
    slot.check().map_err(|_| "A key isn't valid.")?;
    if slot.vault != vault {
        return Err("A key belongs to another vault.");
    }
    if slot.expired(now_unix()) {
        return Err("That pairing link has expired.");
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateBody {
    vault: String,
    slots: Vec<Slot>,
    index: String,
}

/// `POST /vault/api/create`.
pub(crate) async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<CreateBody>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (owner, fresh) = match caller(&app, &headers, true).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(Json(body)) = body else {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    };
    if !valid_id(&body.vault) || body.slots.len() > MAX_SLOTS {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    }
    for slot in &body.slots {
        if let Err(text) = check_slot(slot, &body.vault) {
            return refusal(StatusCode::BAD_REQUEST, text);
        }
    }
    let mut ids: Vec<&str> = body.slots.iter().map(|slot| slot.slot.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() != body.slots.len() {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    }
    if !slot::enough(&body.slots) {
        return refusal(
            StatusCode::BAD_REQUEST,
            "A vault needs your recovery code and one more way to unlock it.",
        );
    }
    let Some(blob) = decode(&body.index).filter(|blob| blob.len() <= MAX_INDEX_BYTES) else {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    };
    if Index::epoch_of(&blob) != Ok(1) {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    }
    let store = store(&app);
    match load(store, &owner).await {
        Ok(Some(_)) => return refusal(StatusCode::CONFLICT, "You already have a vault."),
        Ok(None) => {}
        Err(error) => return unavailable(&error),
    }
    let name = match put_index(store, &owner, 1, blob).await {
        Ok(name) => name,
        Err(error) => return unavailable(&error),
    };
    let record = Record {
        schema: SCHEMA.to_owned(),
        vault: body.vault,
        slots: body.slots,
        epoch: 1,
        index: name.clone(),
        created_unix: now_unix(),
    };
    if let Err(error) = save(store, &owner, &record, None).await {
        if let Ok(full) = key(&owner, &name) {
            let _ = delete(store, &full).await;
        }
        return match error {
            Error::Conflict => refusal(StatusCode::CONFLICT, "You already have a vault."),
            other => unavailable(&other),
        };
    }
    eprintln!("vault: created");
    ok(StatusCode::CREATED, json!({ "vault": record.vault }), fresh)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SlotBody {
    slot: Slot,
}

/// `POST /vault/api/slots`.
pub(crate) async fn add_slot(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<SlotBody>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (owner, fresh) = match caller(&app, &headers, true).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(Json(SlotBody { slot })) = body else {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    };
    let store = store(&app);
    let (mut record, generation) = match load(store, &owner).await {
        Ok(Some(found)) => found,
        Ok(None) => return refusal(StatusCode::NOT_FOUND, "You don't have a vault yet."),
        Err(error) => return unavailable(&error),
    };
    if let Err(text) = check_slot(&slot, &record.vault) {
        return refusal(StatusCode::BAD_REQUEST, text);
    }
    if record.slots.iter().any(|kept| kept.slot == slot.slot) {
        return refusal(StatusCode::CONFLICT, "That key is already in your vault.");
    }
    if record.slots.len() >= MAX_SLOTS {
        return refusal(
            StatusCode::CONFLICT,
            "Your vault has as many devices as it can. Remove one first.",
        );
    }
    record.slots.push(slot);
    match save(store, &owner, &record, Some(&generation)).await {
        Ok(_) => ok(
            StatusCode::CREATED,
            json!({ "slots": record.slots.len() }),
            fresh,
        ),
        Err(error) => unavailable(&error),
    }
}

/// `POST /vault/api/slots/{slot}/delete`.
pub(crate) async fn delete_slot(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (owner, fresh) = match caller(&app, &headers, true).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let store = store(&app);
    let (mut record, generation) = match load(store, &owner).await {
        Ok(Some(found)) => found,
        Ok(None) => return refusal(StatusCode::NOT_FOUND, "You don't have a vault yet."),
        Err(error) => return unavailable(&error),
    };
    let Some(position) = record.slots.iter().position(|slot| slot.slot == id) else {
        return refusal(StatusCode::NOT_FOUND, "That device isn't in your vault.");
    };
    let removed = record.slots.remove(position);
    if removed.method != Method::Pairing && !slot::enough(&record.slots) {
        return refusal(
            StatusCode::CONFLICT,
            "Keep your recovery code and one more way to unlock your vault.",
        );
    }
    match save(store, &owner, &record, Some(&generation)).await {
        Ok(_) => ok(StatusCode::NO_CONTENT, json!({}), fresh),
        Err(error) => unavailable(&error),
    }
}

/// `PUT /vault/api/objects/{object}`: a sealed object, checked for shape.
pub(crate) async fn write_object(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let (owner, fresh) = match caller(&app, &headers, true).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if !valid_id(&id) {
        return refusal(StatusCode::NOT_FOUND, "That file isn't in your vault.");
    }
    if body.len() > MAX_OBJECT_BYTES {
        return refusal(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Files must be 10 MB or smaller.",
        );
    }
    let store = store(&app);
    let record = match load(store, &owner).await {
        Ok(Some((record, _))) => record,
        Ok(None) => return refusal(StatusCode::NOT_FOUND, "You don't have a vault yet."),
        Err(error) => return unavailable(&error),
    };
    let Ok((header, _)) = object::parse(&body) else {
        return refusal(StatusCode::BAD_REQUEST, "That isn't a locked vault file.");
    };
    if header.core.object != id
        || header.core.vault != record.vault
        || header.core.tier != Tier::User
        || !header.wraps.is_empty()
    {
        return refusal(StatusCode::BAD_REQUEST, "That isn't a locked vault file.");
    }
    match object_ids(store, &owner).await {
        Ok(ids) if ids.len() >= MAX_OBJECTS => {
            return refusal(
                StatusCode::CONFLICT,
                "Your vault holds as many files as it can. Delete some first.",
            );
        }
        Ok(_) => {}
        Err(error) => return unavailable(&error),
    }
    let full = match object_key(&owner, &id) {
        Ok(full) => full,
        Err(error) => return unavailable(&error),
    };
    let size = body.len();
    match store.write_key(&full, body.to_vec(), None).await {
        Ok(_) => ok(
            StatusCode::CREATED,
            json!({ "object": id, "size": size }),
            fresh,
        ),
        Err(error) => unavailable(&error),
    }
}

/// `GET /vault/api/objects/{object}`.
pub(crate) async fn read_object(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (owner, _) = match caller(&app, &headers, false).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if !valid_id(&id) {
        return refusal(StatusCode::NOT_FOUND, "That file isn't in your vault.");
    }
    let full = match object_key(&owner, &id) {
        Ok(full) => full,
        Err(error) => return unavailable(&error),
    };
    match store(&app).read_key(&full).await {
        Ok(Some((bytes, _))) => crate::chat_html::protect(
            ([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response(),
        ),
        Ok(None) => refusal(StatusCode::NOT_FOUND, "That file isn't in your vault."),
        Err(error) => unavailable(&error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndexBody {
    after: u32,
    blob: String,
    #[serde(default)]
    delete: Vec<String>,
}

/// `POST /vault/api/index`: the next epoch's index, compare-and-swap on the
/// current epoch. The old epoch and the objects named in `delete` are then
/// deleted, so their keys exist nowhere.
pub(crate) async fn write_index(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<IndexBody>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (owner, fresh) = match caller(&app, &headers, true).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(Json(body)) = body else {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    };
    if body.delete.len() > MAX_OBJECTS || !body.delete.iter().all(|id| valid_id(id)) {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    }
    let Some(blob) = decode(&body.blob).filter(|blob| blob.len() <= MAX_INDEX_BYTES) else {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    };
    let Some(epoch) = body.after.checked_add(1) else {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    };
    if Index::epoch_of(&blob) != Ok(epoch) {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    }
    let store = store(&app);
    let (mut record, generation) = match load(store, &owner).await {
        Ok(Some(found)) => found,
        Ok(None) => return refusal(StatusCode::NOT_FOUND, "You don't have a vault yet."),
        Err(error) => return unavailable(&error),
    };
    if record.epoch != body.after {
        return refusal(
            StatusCode::CONFLICT,
            "Your vault changed on another device. Try again.",
        );
    }
    let name = match put_index(store, &owner, epoch, blob).await {
        Ok(name) => name,
        Err(error) => return unavailable(&error),
    };
    let old = std::mem::replace(&mut record.index, name.clone());
    record.epoch = epoch;
    if let Err(error) = save(store, &owner, &record, Some(&generation)).await {
        if let Ok(full) = key(&owner, &name) {
            let _ = delete(store, &full).await;
        }
        return unavailable(&error);
    }
    // The new epoch is current: the old one, and the deleted files, go.
    let mut failed = false;
    if let Ok(full) = key(&owner, &old) {
        failed |= delete(store, &full).await.is_err();
    }
    for id in &body.delete {
        if let Ok(full) = object_key(&owner, id) {
            failed |= delete(store, &full).await.is_err();
        }
    }
    if failed {
        eprintln!("vault: index advanced; a stale object or index wasn't deleted");
    }
    ok(StatusCode::OK, json!({ "epoch": epoch }), fresh)
}

/// `POST /vault/api/delete`: every slot, index and object.
pub(crate) async fn delete_vault(State(app): State<App>, headers: HeaderMap) -> Response {
    let (owner, fresh) = match caller(&app, &headers, true).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let folder = match Store::owner_key(&owner, "vault") {
        Ok(folder) => folder,
        Err(error) => return unavailable(&error),
    };
    match store(&app).remove_folder(&folder).await {
        Ok(()) => {
            eprintln!("vault: deleted");
            ok(StatusCode::NO_CONTENT, json!({}), fresh)
        }
        Err(error) => unavailable(&error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnswerBody {
    question: String,
    files: Vec<AnswerFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnswerFile {
    name: String,
    #[serde(default)]
    media: String,
    data: String,
}

const INSTRUCTIONS: &str = "Answer the person's question from the files they shared in this message. \
If the files don't hold the answer, say so plainly. Keep the answer short and exact; quote figures as they appear.";

/// The Open Responses request for a Fast answer: the question, then each
/// file (text inline, images and PDFs as data).
pub(crate) fn answer_request(
    question: &str,
    files: &[(String, crate::chat_files::Kind, Vec<u8>)],
) -> Value {
    let mut content = vec![json!({ "type": "input_text", "text": question })];
    for (name, kind, bytes) in files {
        match kind {
            crate::chat_files::Kind::Text => {
                let text = String::from_utf8_lossy(bytes);
                let text: String = text.chars().take(MAX_TEXT_CHARS).collect();
                content.push(
                    json!({ "type": "input_text", "text": format!("File \"{name}\":\n{text}") }),
                );
            }
            other => {
                content.push(json!({ "type": "input_text", "text": format!("File \"{name}\":") }));
                content.push(json!({
                    "type": "input_image",
                    "image_url": format!(
                        "data:{};base64,{}",
                        other.mime(),
                        base64::engine::general_purpose::STANDARD.encode(bytes)
                    ),
                }));
            }
        }
    }
    json!({
        "model": crate::chat_vision::GEMINI,
        "instructions": INSTRUCTIONS,
        "input": [{ "type": "message", "role": "user", "content": content }],
        "stream": false,
        "store": false,
    })
}

/// `POST /vault/api/answer`: the Fast route. The files are the plaintext
/// the person's browser decrypted for this one answer. Nothing is stored,
/// and nothing of the question, the files or the answer is logged.
pub(crate) async fn answer(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<AnswerBody>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (_, fresh) = match caller(&app, &headers, true).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(Json(body)) = body else {
        return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
    };
    let question = body.question.trim();
    if question.is_empty() || question.chars().count() > MAX_QUESTION_CHARS {
        return refusal(
            StatusCode::BAD_REQUEST,
            "Ask a question of up to 4,000 characters.",
        );
    }
    if body.files.is_empty() || body.files.len() > MAX_ANSWER_FILES {
        return refusal(
            StatusCode::BAD_REQUEST,
            "Pick one to four files to ask about.",
        );
    }
    let mut files = Vec::new();
    let mut total = 0;
    for file in body.files {
        let Some(bytes) = decode(&file.data) else {
            return refusal(StatusCode::BAD_REQUEST, "That request isn't valid.");
        };
        total += bytes.len();
        if total > MAX_ANSWER_FILE_BYTES {
            return refusal(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Those files are too large to ask about together.",
            );
        }
        let Some(kind) = crate::chat_files::Kind::sniff(&bytes) else {
            return refusal(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Fast answers read text, PDFs and images.",
            );
        };
        let _ = file.media;
        let name: String = file
            .name
            .chars()
            .filter(|c| !c.is_control())
            .take(120)
            .collect();
        files.push((name, kind, bytes));
    }
    let Some(gemini) = crate::chat_vision::doors(&app).and_then(|doors| doors.gemini) else {
        return refusal(
            StatusCode::SERVICE_UNAVAILABLE,
            "Fast answers aren't available here right now.",
        );
    };
    match gemini.call(&answer_request(question, &files)).await {
        Ok((text, _)) => ok(
            StatusCode::OK,
            json!({ "answer": text, "model": "Google Gemini" }),
            fresh,
        ),
        Err(_) => {
            eprintln!("vault: fast answer failed");
            refusal(
                StatusCode::BAD_GATEWAY,
                "Google Gemini didn't answer. Try again in a minute.",
            )
        }
    }
}
