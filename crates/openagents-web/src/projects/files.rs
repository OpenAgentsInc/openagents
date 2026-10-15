//! A project's private data (#11471, `docs/api/projects.md`): files,
//! text documents, the Drive sources connected on the project's Sources
//! page, and files locked in the person's vault, listed together as the
//! project's **attachments**.
//!
//! | Route | What |
//! | --- | --- |
//! | `POST /v1/files?purpose=project&filename=NAME` (the file's bytes) | `201 File` |
//! | `GET /v1/files/{id}` | The `File` |
//! | `GET /v1/files/{id}/content` | Its bytes, as a download |
//! | `DELETE /v1/files/{id}` | `{id, object: "file.deleted"}` |
//! | `GET /v1/projects/{id}/attachments` | `{data: [Attachment], next}`, oldest first (`?limit`, `?after`) |
//! | `POST /v1/projects/{id}/attachments` | `{"file": "file_…"}`, `{"document": {"title", "text"}}` or `{"vault": "<object id>"}`: `201 Attachment` |
//! | `GET /v1/projects/{id}/documents/{doc}` | A document with its text |
//! | `DELETE /v1/projects/{id}/attachments/{att}` | `{id, object: "attachment.deleted"}`; a file goes with it |
//! | `PATCH /v1/projects/{id}` `{"protection"}` | The project's default for new files: `standard` or `only_me` |
//!
//! - **`standard`** files and documents are kept in the account's own
//!   folder of the private chat store (the chat bucket in production,
//!   encrypted at rest by Google), under `project-files/`. Only the
//!   account's own sign-in reads them; another account's ids answer `404`.
//!   A text file, PDF, or document whose words look like a password or key
//!   is refused and nothing is kept.
//! - **`only_me`** files are locked on the person's computer under
//!   NIP-VAULT (`openagents projects files add`, which runs `openagents
//!   vault add --project`) and stored as vault objects. This server only
//!   records that object id `X` belongs to project `P`, with its size; the
//!   name, type, and key stay in the person's sealed index. An `only_me`
//!   project takes no `standard` file or document.
//!
//! Storage (under the account's folder): `project-files/files/{file}` and
//! `{file}.json`; per project `project-files/projects/{project}/` holds
//! `settings.json`, `f-{file}.json` (an attached file), `d-{doc}.json` (a
//! document) and `v-{object}.json` (a vault file).

pub(crate) mod uploads;

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::api::{Access, Page, access, paged, rfc3339};
use crate::App;
use crate::chat_files::Kind;
use crate::chat_store::{Error, Store, account_owner, now_unix};
use crate::cloud::protect;
use crate::coder_sync::{answer, refused, stored};

/// The largest file.
pub(crate) const MAX_BYTES: usize = 10 * 1024 * 1024;
/// The most attachments (files, documents, vault files) one project holds.
pub(crate) const MAX_ATTACHMENTS: usize = 200;
/// The longest document text.
pub(crate) const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAX_NAME_CHARS: usize = 120;
const MAX_TITLE_CHARS: usize = 200;
const FILE_SCHEMA: &str = "openagents.web.project-file.v2";
const DOCUMENT_SCHEMA: &str = "openagents.web.project-document.v1";
const VAULT_SCHEMA: &str = "openagents.web.project-vault-file.v1";
const SETTINGS_SCHEMA: &str = "openagents.web.project-settings.v1";
const ROOT: &str = "project-files";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .merge(uploads::routes())
        .route(
            "/v1/files",
            axum::routing::post(upload).layer(DefaultBodyLimit::max(16 * 1024 * 1024)),
        )
        .route("/v1/files/{id}", get(file_info).delete(file_delete))
        .route("/v1/files/{id}/content", get(file_content))
        .route(
            "/v1/projects/{id}/attachments",
            get(attachments)
                .post(attach)
                .layer(DefaultBodyLimit::max(MAX_DOCUMENT_BYTES * 2)),
        )
        .route(
            "/v1/projects/{id}/attachments/{attachment}",
            axum::routing::delete(detach),
        )
        .route("/v1/projects/{id}/documents/{doc}", get(document))
}

/// Whether `path` is one of the `/v1/files` routes (the `/v1/projects`
/// ones are [`super::api::owns`]).
pub(crate) fn owns(path: &str) -> bool {
    path.strip_prefix("/v1/files")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

// ------------------------------------------------------------ protection

/// Who can open a project's new files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Protection {
    /// Kept by the service, readable by the account's sign-in.
    #[default]
    Standard,
    /// Locked on the person's devices (NIP-VAULT); the service holds
    /// ciphertext only.
    OnlyMe,
}

impl Protection {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::OnlyMe => "only_me",
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Settings {
    schema: String,
    protection: Protection,
}

// --------------------------------------------------------------- objects

/// An uploaded file, as kept beside its bytes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct FileInfo {
    schema: String,
    id: String,
    filename: String,
    mime_type: String,
    size_bytes: u64,
    sha256: String,
    created_unix: u64,
    /// The project it is attached to.
    #[serde(default)]
    project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    chunks: Option<uploads::Chunks>,
}

impl FileInfo {
    fn shown(&self) -> Value {
        json!({
            "id": self.id,
            "object": "file",
            "filename": self.filename,
            "mime_type": self.mime_type,
            "size_bytes": self.size_bytes,
            "sha256": self.sha256,
            "created_at": rfc3339(self.created_unix),
            "stored_as": "original",
            "project": self.project,
        })
    }

    fn attachment(&self) -> Value {
        json!({
            "id": self.id,
            "object": "attachment",
            "type": "file",
            "protection": "standard",
            "filename": self.filename,
            "mime_type": self.mime_type,
            "size_bytes": self.size_bytes,
            "sha256": self.sha256,
            "created_at": rfc3339(self.created_unix),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DocumentInfo {
    schema: String,
    id: String,
    title: String,
    text: String,
    created_unix: u64,
}

impl DocumentInfo {
    fn attachment(&self) -> Value {
        json!({
            "id": self.id,
            "object": "attachment",
            "type": "document",
            "protection": "standard",
            "title": self.title,
            "size_bytes": self.text.len(),
            "created_at": rfc3339(self.created_unix),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct VaultInfo {
    schema: String,
    id: String,
    size_bytes: u64,
    created_unix: u64,
}

impl VaultInfo {
    fn attachment(&self) -> Value {
        json!({
            "id": self.id,
            "object": "attachment",
            "type": "vault",
            "protection": "only_me",
            "size_bytes": self.size_bytes,
            "created_at": rfc3339(self.created_unix),
        })
    }
}

// ----------------------------------------------------------------- keys

fn random_id(prefix: &str) -> String {
    let hex: String = secp256k1::rand::random::<[u8; 12]>()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{prefix}{hex}")
}

fn valid_prefixed(id: &str, prefix: &str) -> bool {
    id.strip_prefix(prefix).is_some_and(|rest| {
        rest.len() == 24
            && rest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn valid_project(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

fn file_key(owner: &str, id: &str, suffix: &str) -> Result<String, Error> {
    Store::owner_key(owner, &format!("{ROOT}/files/{id}{suffix}"))
}

fn project_folder(owner: &str, project: &str) -> Result<String, Error> {
    if !valid_project(project) {
        return Err(Error::Invalid("The project id is invalid."));
    }
    Store::owner_key(owner, &format!("{ROOT}/projects/{project}"))
}

fn project_key(owner: &str, project: &str, name: &str) -> Result<String, Error> {
    Ok(format!("{}/{name}", project_folder(owner, project)?))
}

async fn read_json<T: serde::de::DeserializeOwned>(
    store: &Store,
    key: &str,
) -> Result<Option<(T, String)>, Error> {
    let Some((bytes, generation)) = store.read_key(key).await? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(|value| Some((value, generation)))
        .map_err(|_| Error::Corrupt("A project file record is invalid."))
}

async fn write_json<T: Serialize>(
    store: &Store,
    key: &str,
    value: &T,
    expected: Option<&str>,
) -> Result<String, Error> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| Error::Invalid("The record could not be saved."))?;
    store.write_key(key, bytes, expected).await
}

async fn remove_key(store: &Store, key: &str) -> Result<bool, Error> {
    match store.read_key(key).await? {
        Some((_, generation)) => store.delete_key(key, &generation).await,
        None => Ok(false),
    }
}

// --------------------------------------------------------------- reading

/// What a project holds, for its counts and its protection.
#[derive(Default)]
pub(crate) struct Summary {
    pub files: usize,
    pub documents: usize,
    pub protection: Protection,
}

/// The project's attachments kept here (not its Drive sources).
struct Held {
    files: Vec<FileInfo>,
    documents: Vec<DocumentInfo>,
    vault: Vec<VaultInfo>,
    protection: Protection,
}

impl Held {
    fn count(&self) -> usize {
        self.files.len() + self.documents.len() + self.vault.len()
    }
}

async fn held(store: &Store, owner: &str, project: &str) -> Result<Held, Error> {
    let mut found = Held {
        files: Vec::new(),
        documents: Vec::new(),
        vault: Vec::new(),
        protection: Protection::Standard,
    };
    for key in store.folder_keys(&project_folder(owner, project)?).await? {
        let name = key.rsplit('/').next().unwrap_or_default();
        if name == "settings.json" {
            if let Some((settings, _)) = read_json::<Settings>(store, &key).await? {
                found.protection = settings.protection;
            }
        } else if name.starts_with("f-") {
            if let Some((file, _)) = read_json::<FileInfo>(store, &key).await? {
                found.files.push(file);
            }
        } else if name.starts_with("d-") {
            if let Some((doc, _)) = read_json::<DocumentInfo>(store, &key).await? {
                found.documents.push(doc);
            }
        } else if name.starts_with("v-")
            && let Some((vault, _)) = read_json::<VaultInfo>(store, &key).await?
        {
            found.vault.push(vault);
        }
    }
    Ok(found)
}

/// The counts and protection `GET /v1/projects` shows for one project.
pub(crate) async fn summary(app: &App, account: &str, project: &str) -> Result<Summary, Error> {
    let held = held(&app.config.chat_store, &account_owner(account), project).await?;
    Ok(Summary {
        files: held.files.len() + held.vault.len(),
        documents: held.documents.len(),
        protection: held.protection,
    })
}

/// The project's protection as its settings keep it: `standard` unless
/// set. A chat in an `only_me` project is never shared
/// ([`crate::pages::chat::share`]).
pub(crate) async fn protection(
    store: &Store,
    owner: &str,
    project: &str,
) -> Result<Protection, Error> {
    let Ok(key) = project_key(owner, project, "settings.json") else {
        return Ok(Protection::Standard);
    };
    Ok(read_json::<Settings>(store, &key)
        .await?
        .map_or(Protection::Standard, |(settings, _)| settings.protection))
}

/// Forget everything a removed project held here. Vault files stay in the
/// person's vault (their index still names them), only the project's
/// record of them goes.
pub(crate) async fn forget_project(app: &App, account: &str, project: &str) -> Result<(), Error> {
    let store = &app.config.chat_store;
    let owner = account_owner(account);
    let held = held(store, &owner, project).await?;
    for file in &held.files {
        delete_file(store, &owner, file).await?;
    }
    store.remove_folder(&project_folder(&owner, project)?).await
}

// ------------------------------------------------------------- the files

/// The file's name without folders or control characters.
pub(crate) fn clean_name(raw: &str) -> String {
    let last = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let name: String = last
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    let name = name.trim();
    if name.is_empty() {
        "File".to_owned()
    } else {
        name.to_owned()
    }
}

/// The type a file is served as: from its first bytes, else its name.
fn mime_type(bytes: &[u8], name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let by_name = |ext: &str| lower.ends_with(ext);
    match Kind::sniff(bytes) {
        Some(Kind::Text) if by_name(".csv") => "text/csv".to_owned(),
        Some(Kind::Text) if by_name(".md") => "text/markdown".to_owned(),
        Some(Kind::Text) if by_name(".json") => "application/json".to_owned(),
        Some(kind) => kind.mime().to_owned(),
        None if by_name(".xlsx") => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_owned()
        }
        None if by_name(".docx") => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_owned()
        }
        None => "application/octet-stream".to_owned(),
    }
}

/// Why `text` can't be kept as a `standard` file, if its words look like a
/// password or key.
fn holds_credential(text: &str) -> bool {
    secret_screen::credential_in(text).is_some()
}

fn credential_refusal() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "looks_like_a_secret",
        "This looks like it holds a password or key, so it wasn't added. Keep it in your vault instead: openagents projects files add PROJECT FILE --protection only_me",
    )
}

#[derive(Deserialize, Default)]
struct UploadQuery {
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// `POST /v1/files?purpose=project&filename=NAME`, the bytes as the body.
async fn upload(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<UploadQuery>,
    body: Bytes,
) -> Response {
    let account = match crate::coder_sync::account(&app, &headers).await {
        Ok(account) => account,
        Err(response) => return response,
    };
    if query.purpose.as_deref().unwrap_or("project") != "project" {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Files here are for projects: send ?purpose=project.",
        );
    }
    if body.is_empty() {
        return refused(StatusCode::BAD_REQUEST, "empty", "This file is empty.");
    }
    let limit = match uploads::direct_limit(&app, &headers).await {
        Ok(limit) => limit,
        Err(response) => return response,
    };
    if body.len() as u64 > limit {
        return refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            &format!(
                "This upload accepts at most {limit} bytes. Use a resumable project upload for larger files."
            ),
        );
    }
    let filename = clean_name(
        query
            .filename
            .as_deref()
            .or(query.name.as_deref())
            .unwrap_or_default(),
    );
    let mime = mime_type(&body, &filename);
    if (mime.starts_with("text/") || mime == "application/json" || mime == "application/pdf")
        && holds_credential(&String::from_utf8_lossy(&body))
    {
        return credential_refusal();
    }
    let info = FileInfo {
        schema: FILE_SCHEMA.to_owned(),
        id: random_id("file_"),
        filename,
        mime_type: mime,
        size_bytes: body.len() as u64,
        sha256: Sha256::digest(&body)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        created_unix: now_unix(),
        project: None,
        chunks: None,
    };
    let store = &app.config.chat_store;
    let owner = account_owner(&account);
    if let Err(response) = uploads::reserve_legacy(&app, &owner, &info.id, info.size_bytes).await {
        return response;
    }
    let saved = async {
        store
            .write_key(&file_key(&owner, &info.id, "")?, body.to_vec(), None)
            .await?;
        write_json(store, &file_key(&owner, &info.id, ".json")?, &info, None).await?;
        uploads::finish_legacy(store, &owner, &info.id).await
    }
    .await;
    match saved {
        Ok(_) => answer(StatusCode::CREATED, info.shown()),
        Err(error) => stored(&error),
    }
}

fn no_file() -> Response {
    refused(StatusCode::NOT_FOUND, "not_found", "There's no such file.")
}

/// Which project a file is read through: a member of a shared project
/// reads its files with `?project=ID` (#11585).
#[derive(Default, Deserialize)]
struct FileQuery {
    #[serde(default)]
    project: Option<String>,
}

/// The caller's file `id` (or, with `project`, that project's file kept
/// in its home's folder), the folder's owner, and its record's
/// generation. With `edit`, the caller must be able to change it.
async fn own_file(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    project: Option<&str>,
    edit: bool,
) -> Result<(String, FileInfo, String), Response> {
    let owner = match project.filter(|p| !p.is_empty()) {
        Some(reference) => {
            let found = access(app, headers, reference).await?;
            if edit {
                found.edits()?;
            }
            Some((found.home_owner(), found.project.id))
        }
        None => None,
    };
    let (owner, within) = match owner {
        Some((owner, project)) => (owner, Some(project)),
        None => (
            account_owner(&crate::coder_sync::account(app, headers).await?),
            None,
        ),
    };
    if !valid_prefixed(id, "file_") {
        return Err(no_file());
    }
    let key = file_key(&owner, id, ".json").map_err(|_| no_file())?;
    match read_json::<FileInfo>(&app.config.chat_store, &key).await {
        Ok(Some((info, generation)))
            if info.id == id
                && within
                    .as_ref()
                    .is_none_or(|p| info.project.as_ref() == Some(p)) =>
        {
            Ok((owner, info, generation))
        }
        Ok(_) => Err(no_file()),
        Err(error) => Err(stored(&error)),
    }
}

/// `GET /v1/files/{id}[?project=ID]`.
async fn file_info(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<FileQuery>,
) -> Response {
    match own_file(&app, &headers, &id, query.project.as_deref(), false).await {
        Ok((_, info, _)) => answer(StatusCode::OK, info.shown()),
        Err(response) => response,
    }
}

/// `GET /v1/files/{id}/content[?project=ID]`.
async fn file_content(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<FileQuery>,
) -> Response {
    let (owner, info, _) =
        match own_file(&app, &headers, &id, query.project.as_deref(), false).await {
            Ok(found) => found,
            Err(response) => return response,
        };
    if info.chunks.is_some() {
        return uploads::content(&app, &owner, &info).await;
    }
    let bytes = match file_key(&owner, &id, "") {
        Ok(key) => match app.config.chat_store.read_key(&key).await {
            Ok(Some((bytes, _))) => bytes,
            Ok(None) => return no_file(),
            Err(error) => return stored(&error),
        },
        Err(_) => return no_file(),
    };
    let encoded: String = info
        .filename
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let mut response = (StatusCode::OK, bytes).into_response();
    let out = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&info.mime_type) {
        out.insert(header::CONTENT_TYPE, value);
    }
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename*=utf-8''{encoded}")) {
        out.insert(header::CONTENT_DISPOSITION, value);
    }
    if let Ok(value) = HeaderValue::from_str(&format!("sha-256={}", info.sha256)) {
        out.insert("digest", value);
    }
    out.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    protect(response)
}

async fn delete_file(store: &Store, owner: &str, info: &FileInfo) -> Result<(), Error> {
    if let Some(project) = &info.project {
        remove_key(
            store,
            &project_key(owner, project, &format!("f-{}.json", info.id))?,
        )
        .await?;
    }
    if let Some(chunks) = &info.chunks {
        uploads::remove_chunks(store, owner, chunks).await?;
    }
    remove_key(store, &file_key(owner, &info.id, "")?).await?;
    remove_key(store, &file_key(owner, &info.id, ".json")?).await?;
    uploads::release(store, owner, &info.id).await?;
    Ok(())
}

/// `DELETE /v1/files/{id}[?project=ID]`: the file, from its project too.
async fn file_delete(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<FileQuery>,
) -> Response {
    let (owner, info, _) = match own_file(&app, &headers, &id, query.project.as_deref(), true).await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    match delete_file(&app.config.chat_store, &owner, &info).await {
        Ok(()) => answer(StatusCode::OK, json!({"id": id, "object": "file.deleted"})),
        Err(error) => stored(&error),
    }
}

// ----------------------------------------------------------- attachments

/// The caller's access to the project `reference` names (#11585).
async fn own_project(app: &App, headers: &HeaderMap, reference: &str) -> Result<Access, Response> {
    access(app, headers, reference).await
}

fn source_attachment(source: &crate::cloud::connections::Source) -> Value {
    json!({
        "id": source.id,
        "object": "attachment",
        "type": "source",
        "integration": source.integration,
        "kind": source.kind,
        "name": source.name,
        "link": source.link(),
        "created_at": rfc3339(source.added_at),
    })
}

fn sources(app: &App, account: &str, project: &str) -> Vec<crate::cloud::connections::Source> {
    app.config
        .connections
        .as_deref()
        .and_then(|store| store.load(&account_owner(account)).ok())
        .map(|kept| kept.sources_of(project).into_iter().cloned().collect())
        .unwrap_or_default()
}

/// `GET /v1/projects/{id}/attachments`.
async fn attachments(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(page): Query<Page>,
) -> Response {
    let found = match own_project(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let project = found.project.id.clone();
    let owner = found.home_owner();
    let held = match held(&app.config.chat_store, &owner, &project).await {
        Ok(held) => held,
        Err(error) => return stored(&error),
    };
    // A vault file deleted from the vault itself (`openagents vault rm`)
    // is no longer listed.
    let in_vault = if held.vault.is_empty() {
        Vec::new()
    } else {
        match crate::vault::held_objects(&app, &owner).await {
            Ok(ids) => ids,
            Err(error) => return stored(&error),
        }
    };
    let mut rows: Vec<(u64, String, Value)> = Vec::new();
    for file in &held.files {
        rows.push((file.created_unix, file.id.clone(), file.attachment()));
    }
    for doc in &held.documents {
        rows.push((doc.created_unix, doc.id.clone(), doc.attachment()));
    }
    for vault in held.vault.iter().filter(|v| in_vault.contains(&v.id)) {
        rows.push((vault.created_unix, vault.id.clone(), vault.attachment()));
    }
    for source in sources(&app, found.home(), &project) {
        rows.push((
            source.added_at,
            source.id.clone(),
            source_attachment(&source),
        ));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let rows: Vec<Value> = rows.into_iter().map(|(_, _, row)| row).collect();
    match paged(rows, &page) {
        Ok(mut body) => {
            body["protection"] = json!(held.protection.word());
            answer(StatusCode::OK, body)
        }
        Err(response) => response,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachBody {
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    document: Option<NewDocument>,
    #[serde(default)]
    vault: Option<String>,
    #[serde(default)]
    source: Option<Value>,
}

#[derive(Deserialize)]
struct NewDocument {
    #[serde(default)]
    title: String,
    #[serde(default)]
    text: String,
}

fn only_me_refusal() -> Response {
    refused(
        StatusCode::CONFLICT,
        "only_me_project",
        "Only you can open this project's files, so they're locked on your computer before they're sent. Add them with: openagents projects files add PROJECT FILE",
    )
}

/// `POST /v1/projects/{id}/attachments`.
async fn attach(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<axum::Json<AttachBody>, JsonRejection>,
) -> Response {
    let found = match own_project(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if let Err(response) = found.edits() {
        return response;
    }
    let project = found.project.id.clone();
    let Ok(axum::Json(body)) = body else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send one of {\"file\": \"file_…\"}, {\"document\": {\"title\", \"text\"}}, or {\"vault\": \"<object id>\"} as JSON.",
        );
    };
    let asked = usize::from(body.file.is_some())
        + usize::from(body.document.is_some())
        + usize::from(body.vault.is_some())
        + usize::from(body.source.is_some());
    if asked != 1 {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Attach one thing at a time: a file, a document, or a vault file.",
        );
    }
    if body.source.is_some() {
        return refused(
            StatusCode::BAD_REQUEST,
            "use_the_sources_page",
            &format!(
                "Attach Drive folders and files with POST /v1/projects/{project}/sources, or run: openagents projects sources PROJECT add LINK"
            ),
        );
    }
    let store = &app.config.chat_store;
    let owner = found.home_owner();
    let held = match held(store, &owner, &project).await {
        Ok(held) => held,
        Err(error) => return stored(&error),
    };
    if held.count() >= MAX_ATTACHMENTS {
        return refused(
            StatusCode::CONFLICT,
            "full",
            "This project holds 200 files and documents. Remove one to add another.",
        );
    }
    let now = now_unix();
    if let Some(object) = body.vault {
        // Vault files open only on the home account's devices.
        if found.home() != found.account {
            return super::api::role_refused();
        }
        let size = match crate::vault::object_size(&app, &owner, &object).await {
            Ok(Some(size)) => size,
            Ok(None) => {
                return refused(
                    StatusCode::NOT_FOUND,
                    "not_in_vault",
                    "That file isn't in your vault. Add it with: openagents projects files add PROJECT FILE",
                );
            }
            Err(error) => return stored(&error),
        };
        let info = VaultInfo {
            schema: VAULT_SCHEMA.to_owned(),
            id: object,
            size_bytes: size,
            created_unix: now,
        };
        let key = match project_key(&owner, &project, &format!("v-{}.json", info.id)) {
            Ok(key) => key,
            Err(error) => return stored(&error),
        };
        if let Some(kept) = held.vault.iter().find(|v| v.id == info.id) {
            return answer(StatusCode::OK, kept.attachment());
        }
        return match write_json(store, &key, &info, None).await {
            Ok(_) => answer(StatusCode::CREATED, info.attachment()),
            Err(error) => stored(&error),
        };
    }
    if held.protection == Protection::OnlyMe {
        return only_me_refusal();
    }
    if let Some(doc) = body.document {
        let title: String = doc
            .title
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_TITLE_CHARS)
            .collect();
        let title = title.trim().to_owned();
        if title.is_empty() || doc.text.trim().is_empty() {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "A document needs a title and some text.",
            );
        }
        if doc.text.len() > MAX_DOCUMENT_BYTES {
            return refused(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                "A document's text must be 1 MB or smaller.",
            );
        }
        if holds_credential(&doc.text) || holds_credential(&title) {
            return credential_refusal();
        }
        let info = DocumentInfo {
            schema: DOCUMENT_SCHEMA.to_owned(),
            id: random_id("pdoc_"),
            title,
            text: doc.text,
            created_unix: now,
        };
        let key = match project_key(&owner, &project, &format!("d-{}.json", info.id)) {
            Ok(key) => key,
            Err(error) => return stored(&error),
        };
        return match write_json(store, &key, &info, None).await {
            Ok(_) => answer(StatusCode::CREATED, info.attachment()),
            Err(error) => stored(&error),
        };
    }
    let file = body.file.unwrap_or_default();
    let (uploader, mut info, generation) = match own_file(&app, &headers, &file, None, false).await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    match info.project.as_deref() {
        Some(already) if already == project => {
            return answer(StatusCode::OK, info.attachment());
        }
        Some(_) => {
            return refused(
                StatusCode::CONFLICT,
                "in_another_project",
                "That file is already in another project. Upload it again to add it here.",
            );
        }
        None => {}
    }
    if uploader != owner {
        if let Err(response) =
            uploads::reserve_legacy(&app, &owner, &info.id, info.size_bytes).await
        {
            return response;
        }
    }
    info.project = Some(project.clone());
    let key = match project_key(&owner, &project, &format!("f-{}.json", info.id)) {
        Ok(key) => key,
        Err(error) => return stored(&error),
    };
    let saved = async {
        if uploader == owner {
            write_json(
                store,
                &file_key(&owner, &info.id, ".json")?,
                &info,
                Some(&generation),
            )
            .await?;
        } else {
            // A member's upload moves into the project's home folder, so
            // every member reads it there (#11585).
            let (bytes, _) = store
                .read_key(&file_key(&uploader, &info.id, "")?)
                .await?
                .ok_or(Error::Invalid("The file is gone."))?;
            store
                .write_key(&file_key(&owner, &info.id, "")?, bytes, None)
                .await?;
            write_json(store, &file_key(&owner, &info.id, ".json")?, &info, None).await?;
            remove_key(store, &file_key(&uploader, &info.id, ".json")?).await?;
            remove_key(store, &file_key(&uploader, &info.id, "")?).await?;
        }
        write_json(store, &key, &info, None).await?;
        uploads::finish_legacy(store, &owner, &info.id).await?;
        if uploader != owner {
            uploads::release(store, &uploader, &info.id).await?;
        }
        Ok::<_, Error>(())
    }
    .await;
    match saved {
        Ok(_) => answer(StatusCode::CREATED, info.attachment()),
        Err(error) => stored(&error),
    }
}

fn no_attachment() -> Response {
    refused(
        StatusCode::NOT_FOUND,
        "not_found",
        "There's no such file or document in this project.",
    )
}

/// `DELETE /v1/projects/{id}/attachments/{attachment}`.
async fn detach(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, attachment)): Path<(String, String)>,
) -> Response {
    let found = match own_project(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if let Err(response) = found.edits() {
        return response;
    }
    let project = found.project.id.clone();
    let store = &app.config.chat_store;
    let owner = found.home_owner();
    let removed = if valid_prefixed(&attachment, "file_") {
        let key = match project_key(&owner, &project, &format!("f-{attachment}.json")) {
            Ok(key) => key,
            Err(error) => return stored(&error),
        };
        match read_json::<FileInfo>(store, &key).await {
            Ok(Some((info, _))) => delete_file(store, &owner, &info).await.map(|()| true),
            Ok(None) => Ok(false),
            Err(error) => Err(error),
        }
    } else if valid_prefixed(&attachment, "pdoc_") {
        match project_key(&owner, &project, &format!("d-{attachment}.json")) {
            Ok(key) => remove_key(store, &key).await,
            Err(error) => Err(error),
        }
    } else if oa_vault::valid_id(&attachment) {
        match project_key(&owner, &project, &format!("v-{attachment}.json")) {
            Ok(key) => remove_key(store, &key).await,
            Err(error) => Err(error),
        }
    } else if sources(&app, found.home(), &project)
        .iter()
        .any(|s| s.id == attachment)
    {
        return refused(
            StatusCode::BAD_REQUEST,
            "use_the_sources_page",
            &format!(
                "Remove a Drive source with DELETE /v1/projects/{project}/sources/{attachment}, or run: openagents projects sources PROJECT remove {attachment}"
            ),
        );
    } else {
        Ok(false)
    };
    match removed {
        Ok(true) => answer(
            StatusCode::OK,
            json!({"id": attachment, "object": "attachment.deleted"}),
        ),
        Ok(false) => no_attachment(),
        Err(error) => stored(&error),
    }
}

/// `GET /v1/projects/{id}/documents/{doc}`.
async fn document(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, doc)): Path<(String, String)>,
) -> Response {
    let found = match own_project(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let project = found.project.id.clone();
    if !valid_prefixed(&doc, "pdoc_") {
        return no_attachment();
    }
    let owner = found.home_owner();
    let key = match project_key(&owner, &project, &format!("d-{doc}.json")) {
        Ok(key) => key,
        Err(error) => return stored(&error),
    };
    match read_json::<DocumentInfo>(&app.config.chat_store, &key).await {
        Ok(Some((info, _))) => {
            let mut shown = info.attachment();
            shown["object"] = json!("document");
            shown["project"] = json!(project);
            shown["text"] = json!(info.text);
            answer(StatusCode::OK, shown)
        }
        Ok(None) => no_attachment(),
        Err(error) => stored(&error),
    }
}

// ---------------------------------------------------------------- edits

/// Apply `protection` from `PATCH /v1/projects/{id}` to the project's
/// settings kept here (the account service keeps its name, instructions,
/// and repositories).
pub(crate) async fn edit(
    app: &App,
    account: &str,
    project: &str,
    protection: &str,
) -> Result<(), Response> {
    let protection = match Some(protection) {
        Some("standard") => Protection::Standard,
        Some("only_me") => Protection::OnlyMe,
        Some("sealed") => {
            return Err(refused(
                StatusCode::BAD_REQUEST,
                "not_available",
                "Sealed projects aren't available yet. Use only_me to keep files only you can open.",
            ));
        }
        Some(_) | None => {
            return Err(refused(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Send {\"protection\": \"standard\"} or {\"protection\": \"only_me\"}.",
            ));
        }
    };
    let store = &app.config.chat_store;
    let owner = account_owner(account);
    let key = project_key(&owner, project, "settings.json").map_err(|e| stored(&e))?;
    let settings = Settings {
        schema: SETTINGS_SCHEMA.to_owned(),
        protection,
    };
    for _ in 0..3 {
        let current = read_json::<Settings>(store, &key)
            .await
            .map_err(|e| stored(&e))?
            .map(|(_, generation)| generation);
        match write_json(store, &key, &settings, current.as_deref()).await {
            Ok(_) => return Ok(()),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(stored(&error)),
        }
    }
    Err(stored(&Error::Conflict))
}
