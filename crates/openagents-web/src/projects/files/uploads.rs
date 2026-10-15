//! Resumable project files. Each stored object and request has a bounded size.
use super::*;
use axum::{
    Json,
    body::Body,
    routing::{post, put},
};
use std::collections::BTreeMap;

pub const PART_BYTES: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub max_file_bytes: Option<u64>,
    pub admin_max_file_bytes: Option<u64>,
    pub account_bytes: Option<u64>,
    pub upload_seconds: u64,
    pub max_files: usize,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            max_file_bytes: Some(MAX_BYTES as u64),
            admin_max_file_bytes: None,
            account_bytes: Some(20 * 1024 * 1024 * 1024),
            upload_seconds: 7 * 86400,
            max_files: 10_000,
        }
    }
}
impl Policy {
    pub fn read(path: &std::path::Path) -> Result<Self, String> {
        let meta = std::fs::metadata(path)
            .map_err(|_| "The project upload configuration couldn't be opened")?;
        if !meta.is_file() || meta.len() > 65536 {
            return Err("Choose a small project upload configuration file".into());
        }
        let policy: Self = serde_json::from_slice(
            &std::fs::read(path).map_err(|_| "The upload configuration couldn't be read")?,
        )
        .map_err(|_| "Check the project upload configuration")?;
        policy.validate()?;
        Ok(policy)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.max_file_bytes == Some(0)
            || self.admin_max_file_bytes == Some(0)
            || self.account_bytes == Some(0)
            || !(60..=30 * 86400).contains(&self.upload_seconds)
            || self.max_files == 0
            || self.max_files > 10_000
        {
            return Err(
                "Choose positive upload limits and an upload lifetime from one minute to 30 days"
                    .into(),
            );
        }
        Ok(())
    }
    fn limit(&self, admin: bool) -> Option<u64> {
        if admin {
            self.admin_max_file_bytes
        } else {
            self.max_file_bytes
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Chunks {
    upload: String,
    parts: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Upload {
    id: String,
    creator: String,
    project: String,
    filename: String,
    size: u64,
    sha256: String,
    file: String,
    part_bytes: usize,
    offset: u64,
    next: u64,
    expires: u64,
    #[serde(default)]
    committed: bool,
    #[serde(default)]
    finalizing: bool,
    #[serde(default)]
    aborted: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewUpload {
    request_id: String,
    project: String,
    filename: String,
    size_bytes: u64,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
struct Part {
    bytes: u64,
    sha256: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Reservation {
    size: u64,
    expires: Option<u64>,
    upload: Option<String>,
}
#[derive(Default, Serialize, Deserialize)]
struct Usage {
    entries: BTreeMap<String, Reservation>,
}

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(
            "/v1/files/uploads",
            post(start).layer(DefaultBodyLimit::max(8192)),
        )
        .route("/v1/files/uploads/{id}", get(status).delete(abort))
        .route(
            "/v1/files/uploads/{id}/parts/{part}",
            put(part).layer(DefaultBodyLimit::max(PART_BYTES)),
        )
        .route("/v1/files/uploads/{id}/complete", post(complete))
}
fn bad(code: &'static str, message: &str) -> Response {
    refused(StatusCode::BAD_REQUEST, code, message)
}
fn conflict(message: &str) -> Response {
    refused(StatusCode::CONFLICT, "upload_conflict", message)
}
fn valid_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn base(owner: &str, id: &str) -> Result<String, Error> {
    if !valid_prefixed(id, "upl_") {
        return Err(Error::Invalid("The upload name is invalid."));
    }
    Store::owner_key(owner, &format!("{ROOT}/uploads/{id}"))
}
fn session_key(owner: &str, id: &str) -> Result<String, Error> {
    Ok(format!("{}/session.json", base(owner, id)?))
}
fn part_key(owner: &str, id: &str, index: u64, suffix: &str) -> Result<String, Error> {
    Ok(format!(
        "{}/parts/{}/{index}{suffix}",
        base(owner, id)?,
        index / 128
    ))
}
fn usage_key(owner: &str) -> Result<String, Error> {
    Store::owner_key(owner, &format!("{ROOT}/usage.json"))
}
async fn caller(app: &App, headers: &HeaderMap) -> Result<(String, bool), Response> {
    let service = app.config.cloud.as_ref().ok_or_else(|| {
        refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "This site doesn't offer accounts.",
        )
    })?;
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    let viewer = service.app_viewer(token).await.map_err(|_| {
        refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Log in again with coder login.",
        )
    })?;
    Ok((viewer.account_id, viewer.admin))
}
pub(super) async fn direct_limit(app: &App, headers: &HeaderMap) -> Result<u64, Response> {
    let (_, admin) = caller(app, headers).await?;
    Ok(app
        .config
        .project_uploads
        .limit(admin)
        .unwrap_or(u64::MAX)
        .min(16 * 1024 * 1024))
}
async fn allowed(
    app: &App,
    headers: &HeaderMap,
    project: &str,
) -> Result<(String, String, String, bool), Response> {
    let (account, admin) = caller(app, headers).await?;
    let found = access(app, headers, project).await?;
    found.edits()?;
    let owner = found.home_owner();
    let project = found.project.id.clone();
    if protection(&app.config.chat_store, &owner, &project)
        .await
        .map_err(|e| stored(&e))?
        == Protection::OnlyMe
    {
        return Err(only_me_refusal());
    }
    Ok((account, owner, project, admin))
}
async fn load(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    project: &str,
    allow_expired: bool,
) -> Result<(String, Upload, String), Response> {
    if !valid_prefixed(id, "upl_") {
        return Err(no_file());
    }
    let (account, owner, project, admin) = allowed(app, headers, project).await?;
    let key = session_key(&owner, id).map_err(|e| stored(&e))?;
    let (upload, generation) = read_json::<Upload>(&app.config.chat_store, &key)
        .await
        .map_err(|e| stored(&e))?
        .ok_or_else(no_file)?;
    if upload.id != id || upload.creator != account || upload.project != project {
        return Err(no_file());
    }
    if upload.aborted {
        return Err(refused(
            StatusCode::GONE,
            "upload_expired",
            "This upload was cancelled. Start the upload again.",
        ));
    }
    if !allow_expired && !upload.committed && !upload.finalizing && upload.expires <= now_unix() {
        return Err(refused(
            StatusCode::GONE,
            "upload_expired",
            "This upload expired. Start the upload again.",
        ));
    }
    check_limit(&app.config.project_uploads, admin, upload.size)?;
    Ok((owner, upload, generation))
}
fn check_limit(policy: &Policy, admin: bool, size: u64) -> Result<(), Response> {
    if size == 0 {
        return Err(bad("empty", "This file is empty."));
    }
    if let Some(max) = policy.limit(admin) {
        if size > max {
            return Err(refused(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                &format!("Your account accepts files up to {max} bytes."),
            ));
        }
    }
    Ok(())
}
fn shown(upload: &Upload) -> Value {
    json!({"id":upload.id,"object":"file.upload","file":upload.file,"project":upload.project,"size_bytes":upload.size,"sha256":upload.sha256,"part_bytes":upload.part_bytes,"uploaded_bytes":upload.offset,"next_part":upload.next,"expires_at":upload.expires,"complete":upload.committed})
}
#[derive(Default, Deserialize)]
struct InProject {
    project: String,
}
async fn status(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<InProject>,
) -> Response {
    match load(&app, &headers, &id, &query.project, false).await {
        Ok((_, u, _)) => answer(StatusCode::OK, shown(&u)),
        Err(e) => e,
    }
}
async fn usage(store: &Store, owner: &str) -> Result<(Usage, Option<String>), Error> {
    if let Some((book, generation)) = read_json::<Usage>(store, &usage_key(owner)?).await? {
        return Ok((book, Some(generation)));
    }
    let folder = Store::owner_key(owner, &format!("{ROOT}/files"))?;
    let mut book = Usage::default();
    for key in store.folder_keys_limit(&folder, 20_000).await? {
        if key.ends_with(".json") {
            if let Some((info, _)) = read_json::<FileInfo>(store, &key).await? {
                book.entries.insert(
                    info.id,
                    Reservation {
                        size: info.size_bytes,
                        expires: None,
                        upload: None,
                    },
                );
            }
        }
    }
    Ok((book, None))
}
async fn reserve(
    store: &Store,
    owner: &str,
    file: &str,
    reservation: Reservation,
    policy: &Policy,
) -> Result<(), Response> {
    for _ in 0..16 {
        let (mut book, generation) = usage(store, owner).await.map_err(|e| stored(&e))?;
        let expired = book
            .entries
            .iter()
            .filter(|(_, entry)| entry.expires.is_some_and(|time| time <= now_unix()))
            .map(|(key, entry)| (key.clone(), entry.upload.clone()))
            .collect::<Vec<_>>();
        let mut cleanup = Vec::new();
        for (file, upload) in expired {
            if let Some(id) = upload {
                let key = session_key(owner, &id).map_err(|e| stored(&e))?;
                if let Some((mut session, generation)) = read_json::<Upload>(store, &key)
                    .await
                    .map_err(|e| stored(&e))?
                {
                    if session.finalizing || session.committed {
                        if let Some(entry) = book.entries.get_mut(&file) {
                            entry.expires = None;
                        }
                        continue;
                    }
                    if !session.aborted {
                        session.aborted = true;
                        match write_json(store, &key, &session, Some(&generation)).await {
                            Ok(_) => (),
                            Err(Error::Conflict) => continue,
                            Err(e) => return Err(stored(&e)),
                        }
                    }
                }
                cleanup.push(id);
            }
            book.entries.remove(&file);
        }
        if let Some(existing) = book.entries.get(file) {
            if existing.size == reservation.size {
                return Ok(());
            }
            return Err(conflict("This upload names a different file."));
        }
        let used = book
            .entries
            .values()
            .try_fold(0u64, |sum, r| sum.checked_add(r.size))
            .ok_or_else(|| conflict("The account storage limit was reached."))?;
        if book.entries.len() >= policy.max_files
            || policy.account_bytes.is_some_and(|quota| {
                used.checked_add(reservation.size)
                    .is_none_or(|total| total > quota)
            })
        {
            return Err(refused(
                StatusCode::PAYLOAD_TOO_LARGE,
                "storage_full",
                "This account's project file storage is full. Remove a file or ask for more storage.",
            ));
        }
        book.entries.insert(file.into(), reservation.clone());
        match write_json(
            store,
            &usage_key(owner).map_err(|e| stored(&e))?,
            &book,
            generation.as_deref(),
        )
        .await
        {
            Ok(_) => {
                for id in cleanup {
                    store
                        .remove_folder(&base(owner, &id).map_err(|e| stored(&e))?)
                        .await
                        .map_err(|e| stored(&e))?;
                }
                return Ok(());
            }
            Err(Error::Conflict) => continue,
            Err(e) => return Err(stored(&e)),
        }
    }
    Err(conflict(
        "Another upload changed the account's storage. Try again.",
    ))
}
async fn change_reservation(
    store: &Store,
    owner: &str,
    file: &str,
    finish: bool,
) -> Result<(), Error> {
    for _ in 0..16 {
        let (mut book, generation) = usage(store, owner).await?;
        if finish {
            let entry = book.entries.get_mut(file).ok_or(Error::Conflict)?;
            entry.expires = None;
        } else {
            book.entries.remove(file);
        }
        match write_json(store, &usage_key(owner)?, &book, generation.as_deref()).await {
            Ok(_) => return Ok(()),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}
pub(super) async fn release(store: &Store, owner: &str, file: &str) -> Result<(), Error> {
    change_reservation(store, owner, file, false).await
}
async fn start(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<NewUpload>,
) -> Response {
    let (account, owner, project, admin) = match allowed(&app, &headers, &body.project).await {
        Ok(v) => v,
        Err(e) => return e,
    };
    if let Err(e) = check_limit(&app.config.project_uploads, admin, body.size_bytes) {
        return e;
    }
    if !valid_sha(&body.sha256) {
        return bad("invalid_request", "Send the complete file's SHA-256.");
    }
    if !valid_sha(&body.request_id) {
        return bad(
            "invalid_request",
            "Choose a 64-character upload request ID.",
        );
    }
    let identity = hash(format!("{}:{}:{}", account, project, body.request_id).as_bytes());
    let id = format!("upl_{}", &identity[..24]);
    let file = format!("file_{}", &identity[..24]);
    let store = &app.config.chat_store;
    let key = match session_key(&owner, &id) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    let filename = clean_name(&body.filename);
    match read_json::<Upload>(store, &key).await {
        Ok(Some((mut existing, generation)))
            if existing.creator == account
                && existing.project == project
                && existing.filename == filename
                && existing.size == body.size_bytes
                && existing.sha256 == body.sha256 =>
        {
            if !existing.aborted
                && (existing.expires > now_unix() || existing.finalizing || existing.committed)
            {
                return answer(StatusCode::OK, shown(&existing));
            }
            existing.aborted = true;
            if let Err(e) = write_json(store, &key, &existing, Some(&generation)).await {
                return stored(&e);
            }
            if let Err(e) = store
                .remove_folder(&base(&owner, &existing.id).unwrap())
                .await
            {
                return stored(&e);
            }
            if let Err(e) = release(store, &owner, &existing.file).await {
                return stored(&e);
            }
        }
        Ok(Some(_)) => return conflict("That upload request ID was used for a different file."),
        Err(e) => return stored(&e),
        _ => (),
    }
    let upload = Upload {
        id: id.clone(),
        creator: account,
        project,
        filename,
        size: body.size_bytes,
        sha256: body.sha256,
        file: file.clone(),
        part_bytes: PART_BYTES,
        offset: 0,
        next: 0,
        expires: now_unix() + app.config.project_uploads.upload_seconds,
        committed: false,
        finalizing: false,
        aborted: false,
    };
    if let Err(e) = reserve(
        store,
        &owner,
        &file,
        Reservation {
            size: upload.size,
            expires: Some(upload.expires),
            upload: Some(id),
        },
        &app.config.project_uploads,
    )
    .await
    {
        return e;
    }
    match write_json(store, &key, &upload, None).await {
        Ok(_) => answer(StatusCode::CREATED, shown(&upload)),
        Err(Error::Conflict) => conflict("The upload was created. Read its status to continue."),
        Err(e) => stored(&e),
    }
}
async fn part(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, index)): Path<(String, u64)>,
    Query(query): Query<InProject>,
    body: Bytes,
) -> Response {
    let (owner, mut upload, generation) =
        match load(&app, &headers, &id, &query.project, false).await {
            Ok(v) => v,
            Err(e) => return e,
        };
    if upload.committed || upload.finalizing {
        return conflict("This upload is finished or is being checked.");
    }
    let digest = headers
        .get("x-part-sha256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !valid_sha(digest) || hash(&body) != digest {
        return bad(
            "digest_mismatch",
            "This file part failed its SHA-256 check. Send it again.",
        );
    }
    let count = upload.size.div_ceil(upload.part_bytes as u64);
    if index >= count
        || index > upload.next
        || body.len() as u64
            != (upload.size - index * upload.part_bytes as u64).min(upload.part_bytes as u64)
    {
        return conflict("Send the next complete file part shown in the upload status.");
    }
    let store = &app.config.chat_store;
    let key = match part_key(&owner, &id, index, "") {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    match store.write_key(&key, body.to_vec(), None).await {
        Ok(_) => (),
        Err(Error::Conflict) => match store.read_key(&key).await {
            Ok(Some((bytes, _))) if hash(&bytes) == digest => (),
            _ => return conflict("That file part was already uploaded with different bytes."),
        },
        Err(e) => return stored(&e),
    }
    let meta = Part {
        bytes: body.len() as u64,
        sha256: digest.into(),
    };
    let meta_key = format!("{key}.json");
    if let Err(e) = write_json(store, &meta_key, &meta, None).await {
        if !matches!(e, Error::Conflict) {
            return stored(&e);
        }
    }
    if index < upload.next {
        return answer(StatusCode::OK, shown(&upload));
    }
    upload.offset += body.len() as u64;
    upload.next += 1;
    match write_json(
        store,
        &session_key(&owner, &id).unwrap(),
        &upload,
        Some(&generation),
    )
    .await
    {
        Ok(_) => answer(StatusCode::OK, shown(&upload)),
        Err(Error::Conflict) => conflict("Read the upload status to continue."),
        Err(e) => stored(&e),
    }
}
async fn read_part(
    store: &Store,
    owner: &str,
    chunks: &Chunks,
    index: u64,
) -> Result<Vec<u8>, Error> {
    let key = part_key(owner, &chunks.upload, index, "")?;
    let meta = read_json::<Part>(store, &format!("{key}.json"))
        .await?
        .ok_or(Error::Invalid("A file part is missing."))?
        .0;
    let bytes = store
        .read_key(&key)
        .await?
        .ok_or(Error::Invalid("A file part is missing."))?
        .0;
    if bytes.len() as u64 != meta.bytes || hash(&bytes) != meta.sha256 {
        return Err(Error::Invalid("A file part failed its SHA-256 check."));
    }
    Ok(bytes)
}
async fn complete(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<InProject>,
) -> Response {
    let (owner, mut upload, mut generation) =
        match load(&app, &headers, &id, &query.project, false).await {
            Ok(v) => v,
            Err(e) => return e,
        };
    let store = &app.config.chat_store;
    if upload.committed {
        return match read_json::<FileInfo>(store, &file_key(&owner, &upload.file, ".json").unwrap())
            .await
        {
            Ok(Some((info, _))) => answer(StatusCode::OK, info.shown()),
            Ok(None) => no_file(),
            Err(e) => stored(&e),
        };
    }
    if upload.offset != upload.size {
        return conflict("Upload every file part before finishing.");
    }
    let chunks = Chunks {
        upload: id.clone(),
        parts: upload.next,
    };
    let mut sha = Sha256::new();
    let mut size = 0u64;
    let mut mime = None;
    let mut carry = Vec::new();
    for index in 0..chunks.parts {
        let bytes = match read_part(store, &owner, &chunks, index).await {
            Ok(b) => b,
            Err(e) => return stored(&e),
        };
        let content_type = mime.get_or_insert_with(|| mime_type(&bytes, &upload.filename));
        if content_type.starts_with("text/")
            || content_type == "application/json"
            || content_type == "application/pdf"
        {
            carry.extend_from_slice(&bytes);
            if holds_credential(&String::from_utf8_lossy(&carry)) {
                return credential_refusal();
            }
            if carry.len() > 65536 {
                carry.drain(..carry.len() - 65536);
            }
        }
        sha.update(&bytes);
        size += bytes.len() as u64;
    }
    if size != upload.size || format!("{:x}", sha.finalize()) != upload.sha256 {
        return bad(
            "digest_mismatch",
            "The complete file failed its SHA-256 check. Start a new upload.",
        );
    }
    let existing = match held(store, &owner, &upload.project).await {
        Ok(v) => v,
        Err(e) => return stored(&e),
    };
    if existing.protection == Protection::OnlyMe {
        return only_me_refusal();
    }
    if existing.count() >= MAX_ATTACHMENTS && !existing.files.iter().any(|f| f.id == upload.file) {
        return conflict("This project holds 200 files and documents. Remove one to add another.");
    }
    if !upload.finalizing {
        upload.finalizing = true;
        generation = match write_json(
            store,
            &session_key(&owner, &id).unwrap(),
            &upload,
            Some(&generation),
        )
        .await
        {
            Ok(g) => g,
            Err(e) => return stored(&e),
        };
    }
    if let Err(e) = change_reservation(store, &owner, &upload.file, true).await {
        return stored(&e);
    }
    let mut info = FileInfo {
        schema: FILE_SCHEMA.into(),
        id: upload.file.clone(),
        filename: upload.filename.clone(),
        mime_type: mime.unwrap_or_else(|| "application/octet-stream".into()),
        size_bytes: upload.size,
        sha256: upload.sha256.clone(),
        created_unix: now_unix(),
        project: Some(upload.project.clone()),
        chunks: Some(chunks),
    };
    let saved = async {
        let file = file_key(&owner, &info.id, ".json")?;
        match read_json::<FileInfo>(store, &file).await? {
            Some((existing, _))
                if existing.sha256 == info.sha256 && existing.project == info.project =>
            {
                info = existing;
            }
            Some(_) => return Err(Error::Conflict),
            None => {
                write_json(store, &file, &info, None).await?;
            }
        }
        let attachment = project_key(&owner, &upload.project, &format!("f-{}.json", info.id))?;
        if read_json::<FileInfo>(store, &attachment).await?.is_none() {
            write_json(store, &attachment, &info, None).await?;
        }
        change_reservation(store, &owner, &info.id, true).await?;
        upload.committed = true;
        upload.finalizing = false;
        write_json(
            store,
            &session_key(&owner, &id)?,
            &upload,
            Some(&generation),
        )
        .await?;
        Ok::<_, Error>(())
    }
    .await;
    match saved {
        Ok(()) => answer(StatusCode::CREATED, info.shown()),
        Err(e) => stored(&e),
    }
}
pub(super) async fn remove_chunks(
    store: &Store,
    owner: &str,
    chunks: &Chunks,
) -> Result<(), Error> {
    store.remove_folder(&base(owner, &chunks.upload)?).await
}
async fn abort(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<InProject>,
) -> Response {
    let (owner, mut upload, generation) =
        match load(&app, &headers, &id, &query.project, true).await {
            Ok(v) => v,
            Err(e) => return e,
        };
    if upload.committed || upload.finalizing {
        return conflict("This upload is finished. Delete the file instead.");
    }
    let store = &app.config.chat_store;
    upload.aborted = true;
    if let Err(e) = write_json(
        store,
        &session_key(&owner, &id).unwrap(),
        &upload,
        Some(&generation),
    )
    .await
    {
        return stored(&e);
    }
    if let Err(e) = store.remove_folder(&base(&owner, &id).unwrap()).await {
        return stored(&e);
    }
    match release(store, &owner, &upload.file).await {
        Ok(()) => answer(
            StatusCode::OK,
            json!({"id":id,"object":"file.upload.deleted"}),
        ),
        Err(e) => stored(&e),
    }
}
pub(super) async fn content(app: &App, owner: &str, info: &FileInfo) -> Response {
    let chunks = info.chunks.clone().unwrap();
    let store = app.config.chat_store.clone();
    let owner = owner.to_owned();
    let stream = futures_util::stream::try_unfold(
        (store, owner, chunks, 0u64),
        |(store, owner, chunks, index)| async move {
            if index == chunks.parts {
                return Ok::<_, std::io::Error>(None);
            }
            let bytes = read_part(&store, &owner, &chunks, index)
                .await
                .map_err(|_| std::io::Error::other("This file part couldn't be read"))?;
            Ok(Some((
                Bytes::from(bytes),
                (store, owner, chunks, index + 1),
            )))
        },
    );
    let mut response = Body::from_stream(stream).into_response();
    let headers = response.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&info.mime_type) {
        headers.insert(header::CONTENT_TYPE, v);
    }
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(info.size_bytes));
    let encoded = info
        .filename
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"._-".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect::<String>();
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename*=utf-8''{encoded}")) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }
    if let Ok(v) = HeaderValue::from_str(&format!("sha-256={}", info.sha256)) {
        headers.insert("digest", v);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    protect(response)
}

pub(super) async fn reserve_legacy(
    app: &App,
    owner: &str,
    id: &str,
    size: u64,
) -> Result<(), Response> {
    reserve(
        &app.config.chat_store,
        owner,
        id,
        Reservation {
            size,
            expires: Some(now_unix() + app.config.project_uploads.upload_seconds),
            upload: None,
        },
        &app.config.project_uploads,
    )
    .await
}
pub(super) async fn finish_legacy(store: &Store, owner: &str, id: &str) -> Result<(), Error> {
    change_reservation(store, owner, id, true).await
}
