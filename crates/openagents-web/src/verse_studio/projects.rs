//! Saved world content uses the existing project membership and private project-file store.
use super::*;
use aethyrforge_host::{
    ProjectCheckpoint, ProjectDocument, ProjectFault, ProjectHistory, SavedRevision, WorldNamespace,
};
use axum::extract::Path;
use oa_auth::repos::Project as AccountProject;
use serde::de::DeserializeOwned;
use serde_json::Value;
mod history;
mod playable_validation;
mod publication;
mod resources;
mod tutorial;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .merge(resources::routes())
        .merge(publication::routes())
        .merge(history::routes())
        .merge(tutorial::routes())
        .route("/v1/verse/studio/projects", get(list).post(create))
        .route(
            "/v1/verse/studio/projects/{id}/{namespace}/{branch}",
            get(open).post(commit),
        )
        .route(
            "/v1/verse/studio/projects/{id}/{namespace}/{branch}/undo",
            post(undo),
        )
        .route(
            "/v1/verse/studio/projects/{id}/{namespace}/{branch}/checkpoint",
            get(checkpoint).post(keep_checkpoint),
        )
}
fn no_project() -> Response {
    refused(
        StatusCode::NOT_FOUND,
        "not_found",
        "This project isn't available to you.",
    )
}
fn invalid() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "invalid_project",
        "Check the project, source files, and save request.",
    )
}
fn fault(error: ProjectFault) -> Response {
    let (status, code, message) = match error {
        ProjectFault::Invalid => (
            StatusCode::BAD_REQUEST,
            "invalid_project",
            "Check the project and source files.",
        ),
        ProjectFault::Stale => (
            StatusCode::CONFLICT,
            "newer_version",
            "Someone saved a newer version. Export your draft or open the latest version.",
        ),
        ProjectFault::ReusedCommand => (
            StatusCode::CONFLICT,
            "save_reused",
            "That save was already used for a different change.",
        ),
        ProjectFault::Full => (
            StatusCode::INSUFFICIENT_STORAGE,
            "history_full",
            "Project history is full. Export a copy before saving more.",
        ),
        ProjectFault::ChangedField => (
            StatusCode::CONFLICT,
            "changed_field",
            "Someone changed the same part. Open the latest version and choose a new change.",
        ),
        ProjectFault::NothingToUndo => (
            StatusCode::CONFLICT,
            "nothing_to_undo",
            "You have no earlier saved change to undo.",
        ),
    };
    refused(status, code, message)
}
fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}
async fn account_projects(
    app: &App,
    headers: &HeaderMap,
    actor: &Actor,
) -> Result<oa_auth::repos::Status, Response> {
    let service = app.config.cloud.as_deref().ok_or_else(denied)?;
    let result = if actor.cookie.is_some() {
        service.github_status(headers).await
    } else {
        service
            .github_status_as(bearer(headers).ok_or_else(denied)?)
            .await
    };
    result.map_err(crate::projects::api::failed)
}
struct Access {
    actor: Actor,
    project: AccountProject,
}
impl Access {
    fn owner(&self) -> String {
        account_owner(crate::projects::api::home_of(
            &self.project,
            &self.actor.account_id,
        ))
    }
    fn edits(&self) -> Result<(), Response> {
        if crate::projects::api::role_of(&self.project).edits() {
            Ok(())
        } else {
            Err(crate::projects::api::role_refused())
        }
    }
}
async fn access(app: &App, headers: &HeaderMap, id: &str) -> Result<Access, Response> {
    let actor = viewer(app, headers).await?;
    let status = account_projects(app, headers, &actor).await?;
    let project = status
        .projects
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(no_project)?;
    let access = Access { actor, project };
    let protection =
        crate::projects::files::protection(&app.config.chat_store, &access.owner(), id)
            .await
            .map_err(|e| stored(&e))?;
    if protection != crate::projects::files::Protection::Standard {
        return Err(refused(
            StatusCode::CONFLICT,
            "local_project",
            "Choose a project that stores its files here to save this world.",
        ));
    }
    Ok(access)
}
fn location(namespace: &str, branch: &str) -> Result<WorldNamespace, Response> {
    if !valid_id(branch) {
        return Err(invalid());
    }
    serde_json::from_value(Value::String(namespace.into())).map_err(|_| invalid())
}
fn key(access: &Access, namespace: WorldNamespace, branch: &str) -> Result<String, Error> {
    // Existing project deletion removes this entire folder, including worlds and their blobs.
    Store::owner_key(
        &access.owner(),
        &format!(
            "project-files/projects/{}/verse/{}/{branch}/history.json",
            access.project.id,
            namespace.key()
        ),
    )
}
async fn read<T: DeserializeOwned>(store: &Store, key: &str) -> Result<Option<(T, String)>, Error> {
    store
        .read_key(key)
        .await?
        .map(|(bytes, generation)| {
            serde_json::from_slice(&bytes)
                .map(|value| (value, generation))
                .map_err(|_| Error::Corrupt("This saved world could not be read."))
        })
        .transpose()
}
async fn load(
    store: &Store,
    key: &str,
    namespace: WorldNamespace,
) -> Result<(ProjectHistory, Option<String>), Error> {
    let (history, generation) = match read(store, key).await? {
        Some((history, generation)) => (history, Some(generation)),
        None => (ProjectHistory::new(namespace), None),
    };
    if history.namespace != namespace || history.validate().is_err() {
        return Err(Error::Corrupt("This saved world could not be read."));
    }
    Ok((history, generation))
}
async fn list(State(app): State<App>, headers: HeaderMap) -> Response {
    let actor = match viewer(&app, &headers).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    match account_projects(&app, &headers, &actor).await {
        Ok(status) => answer(
            StatusCode::OK,
            json!({"data":status.projects.iter().map(|p|json!({"id":p.id,"name":p.name,"role":crate::projects::api::role_of(p)})).collect::<Vec<_>>() }),
        ),
        Err(r) => r,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    csrf: String,
    name: String,
}
async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<Create>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let actor = match viewer(&app, &headers).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let Ok(Json(sent)) = body else {
        return invalid();
    };
    if !csrf(&app, &headers, &actor, &sent.csrf) {
        return denied();
    }
    let Some(new) = oa_auth::repos::new_project(&json!({"name":sent.name,"repositories":[]}))
    else {
        return invalid();
    };
    let Some(service) = app.config.cloud.as_deref() else {
        return denied();
    };
    let result = if actor.cookie.is_some() {
        service.create_project(&headers, &new).await
    } else {
        let Some(token) = bearer(&headers) else {
            return denied();
        };
        service.create_project_as(token, &new).await
    };
    match result {
        Ok(p) => answer(
            StatusCode::CREATED,
            json!({"id":p.id,"name":p.name,"role":crate::projects::api::role_of(&p)}),
        ),
        Err(e) => crate::projects::api::failed(e),
    }
}
type At = Path<(String, String, String)>;
async fn open(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch)): At,
) -> Response {
    let ns = match location(&namespace, &branch) {
        Ok(n) => n,
        Err(r) => return r,
    };
    let access = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let key = match key(&access, ns, &branch) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    match load(&app.config.chat_store, &key, ns).await {
        Ok((book, _)) => answer(
            StatusCode::OK,
            json!({"project":id,"namespace":ns,"branch":branch,"head":book.head(),"history":book.revisions.iter().map(|r|json!({"version":r.version,"actor":r.actor,"created_unix":r.created_unix,"summary":r.summary,"document_sha256":r.document_sha256,"undo_of":r.undo_of})).collect::<Vec<_>>()}),
        ),
        Err(e) => stored(&e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    csrf: String,
    command_id: String,
    expected_version: u64,
    document: ProjectDocument,
    summary: String,
}
async fn commit(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch)): At,
    body: Result<Json<Save>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(sent)) = body else {
        return invalid();
    };
    let ns = match location(&namespace, &branch) {
        Ok(n) => n,
        Err(r) => return r,
    };
    if sent.document.namespace != ns || sent.document.validate().is_err() {
        return invalid();
    }
    let access = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    if let Err(r) = access.edits() {
        return r;
    }
    if !csrf(&app, &headers, &access.actor, &sent.csrf) {
        return denied();
    }
    if let Err(r) = resources::present(&app, &access, ns, &branch, &sent.document).await {
        return r;
    }
    let encoded = serde_json::to_string(&sent.document).unwrap_or_default();
    if secret_screen::credential_in(&encoded).is_some() {
        return refused(
            StatusCode::BAD_REQUEST,
            "looks_like_a_secret",
            "A source file looks like it contains a password or key. Remove it before saving.",
        );
    }
    store_change(&app, &access, ns, &branch, |book| {
        book.save(
            &access.actor.account_id,
            now(),
            &sent.command_id,
            sent.expected_version,
            sent.document.clone(),
            &sent.summary,
        )
    })
    .await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Undo {
    csrf: String,
    command_id: String,
    expected_version: u64,
}
async fn undo(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch)): At,
    body: Result<Json<Undo>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(sent)) = body else {
        return invalid();
    };
    let ns = match location(&namespace, &branch) {
        Ok(n) => n,
        Err(r) => return r,
    };
    let access = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    if let Err(r) = access.edits() {
        return r;
    }
    if !csrf(&app, &headers, &access.actor, &sent.csrf) {
        return denied();
    }
    store_change(&app, &access, ns, &branch, |book| {
        book.undo_own(
            &access.actor.account_id,
            now(),
            &sent.command_id,
            sent.expected_version,
        )
    })
    .await
}
async fn store_change(
    app: &App,
    access: &Access,
    namespace: WorldNamespace,
    branch: &str,
    mut change: impl FnMut(&mut ProjectHistory) -> Result<SavedRevision, ProjectFault>,
) -> Response {
    let key = match key(access, namespace, branch) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    for _ in 0..4 {
        let (mut book, generation) = match load(&app.config.chat_store, &key, namespace).await {
            Ok(b) => b,
            Err(e) => return stored(&e),
        };
        let revision = match change(&mut book) {
            Ok(r) => r,
            Err(e) => return fault(e),
        };
        let bytes = match serde_json::to_vec(&book) {
            Ok(b) => b,
            Err(_) => return invalid(),
        };
        match app
            .config
            .chat_store
            .write_key(&key, bytes, generation.as_deref())
            .await
        {
            Ok(_) => {
                return answer(
                    StatusCode::OK,
                    json!({"project":access.project.id,"namespace":namespace,"branch":branch,"saved":revision}),
                );
            }
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    fault(ProjectFault::Stale)
}
fn checkpoint_key(
    access: &Access,
    namespace: WorldNamespace,
    branch: &str,
) -> Result<String, Error> {
    Store::owner_key(
        &account_owner(&access.actor.account_id),
        &format!(
            "verse-views/{}/{}/{branch}.json",
            access.project.id,
            namespace.key()
        ),
    )
}
async fn checkpoint(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch)): At,
) -> Response {
    let ns = match location(&namespace, &branch) {
        Ok(n) => n,
        Err(r) => return r,
    };
    let access = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let key = match checkpoint_key(&access, ns, &branch) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    match read::<ProjectCheckpoint>(&app.config.chat_store, &key).await {
        Ok(value) => answer(StatusCode::OK, json!({"checkpoint":value.map(|(v,_)|v)})),
        Err(e) => stored(&e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeepCheckpoint {
    csrf: String,
    checkpoint: ProjectCheckpoint,
}
async fn keep_checkpoint(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch)): At,
    body: Result<Json<KeepCheckpoint>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(mut sent)) = body else {
        return invalid();
    };
    let ns = match location(&namespace, &branch) {
        Ok(n) => n,
        Err(r) => return r,
    };
    if sent.checkpoint.project != id
        || sent.checkpoint.namespace != ns
        || sent.checkpoint.branch != branch
        || sent.checkpoint.validate().is_err()
    {
        return invalid();
    }
    let access = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    if !csrf(&app, &headers, &access.actor, &sent.csrf) {
        return denied();
    }
    sent.checkpoint.visit.owner = Some(access.actor.account_id.clone());
    let key = match checkpoint_key(&access, ns, &branch) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    for _ in 0..4 {
        let generation = match app.config.chat_store.read_key(&key).await {
            Ok(v) => v.map(|(_, g)| g),
            Err(e) => return stored(&e),
        };
        match app
            .config
            .chat_store
            .write_key(
                &key,
                serde_json::to_vec(&sent.checkpoint).unwrap(),
                generation.as_deref(),
            )
            .await
        {
            Ok(_) => return answer(StatusCode::OK, json!({"saved":true})),
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    fault(ProjectFault::Stale)
}

#[cfg(test)]
mod tests;
