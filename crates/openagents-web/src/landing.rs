//! Public access to the repository's existing landing queue (#12038).

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use coder::task::land_queue::{self, Entry, Heartbeat, Record, State as EntryState};
use openagents_sdk::land::LandSubmit;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::App;
use crate::chat_store::{Error, Store, digest};
use crate::coder_sync::{answer, refused, stored};

/// A queue belongs to one repository, just like the integrator's checkout.
/// Its objects have the same paths and JSON as `openagents land`.
pub struct LandingQueue {
    store: Arc<Store>,
    repository: String,
    location: String,
    client: reqwest::Client,
    github: url::Url,
}

impl LandingQueue {
    /// Open the CLI's queue using the server's cloud identity, without gcloud.
    pub fn from_env() -> Result<Self, Error> {
        let address = std::env::var(land_queue::QUEUE_ENV)
            .unwrap_or_else(|_| land_queue::DEFAULT_QUEUE.into());
        let repository = std::env::var("OPENAGENTS_LAND_REPO")
            .unwrap_or_else(|_| crate::github_refs::DEFAULT_REPOSITORY.into());
        Self::new(&repository, &address)
    }

    /// Use a `gs://bucket/prefix` queue or a local fixture directory.
    pub fn new(repository: &str, address: &str) -> Result<Self, Error> {
        if !crate::work_runs::valid_repo(repository) {
            return Err(Error::Invalid("The landing repository is invalid."));
        }
        let store = if let Some(rest) = address.strip_prefix("gs://") {
            let (bucket, prefix) =
                rest.trim_end_matches('/')
                    .split_once('/')
                    .ok_or(Error::Invalid(
                        "The landing queue needs a bucket and folder.",
                    ))?;
            Store::gcs(bucket.into(), prefix.into())?
        } else {
            let folder = address.strip_prefix("file://").unwrap_or(address);
            if folder.is_empty() {
                return Err(Error::Invalid("The landing queue needs a folder."));
            }
            Store::local(folder.into())
        };
        Ok(Self {
            store: Arc::new(store),
            repository: repository.into(),
            location: address.into(),
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("OpenAgents landing queue")
                .build()
                .map_err(|_| Error::Unavailable("The landing queue could not start."))?,
            github: url::Url::parse("https://api.github.com/").expect("fixed GitHub address"),
        })
    }

    async fn read<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Failure> {
        self.store
            .read_key(key)
            .await?
            .map(|(bytes, _)| {
                serde_json::from_slice(&bytes).map_err(|_| {
                    Failure::Store(Error::Unavailable("The landing record could not be read."))
                })
            })
            .transpose()
    }

    async fn branch_head(&self, branch: &str) -> Result<String, Failure> {
        let mut address = self.github.clone();
        // Repository and branch slashes are path separators, never query text.
        address.set_path(&format!("repos/{}/git/ref/heads/{branch}", self.repository));
        let response = self
            .client
            .get(address)
            .send()
            .await
            .map_err(|_| Failure::Store(Error::Unavailable("GitHub could not be reached.")))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Err(Failure::Branch);
        }
        if !response.status().is_success() {
            return Err(Failure::Store(Error::Unavailable(
                "GitHub could not check the branch.",
            )));
        }
        let value: Value = response.json().await.map_err(|_| {
            Failure::Store(Error::Unavailable("GitHub could not check the branch."))
        })?;
        let head = value["object"]["sha"].as_str().ok_or(Failure::Branch)?;
        if head.len() != 40 || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Failure::Branch);
        }
        Ok(head.into())
    }

    async fn submit(
        &self,
        account: &str,
        request_id: &str,
        body: LandSubmit,
    ) -> Result<Value, Failure> {
        if !valid_id(request_id)
            || !valid_branch(&body.branch)
            || body.repo != self.repository
            || body.issue == Some(0)
            || body
                .summary
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > 1024 || s.chars().any(char::is_control))
        {
            return Err(Failure::Invalid);
        }
        let fingerprint = digest(&serde_json::to_vec(&body).map_err(|_| Failure::Invalid)?);
        let request_key = digest(format!("{account}\0{request_id}").as_bytes());
        let receipt_key = format!("requests/{request_key}.json");
        let prepared = match self.read::<Prepared>(&receipt_key).await? {
            Some(prepared) => prepared,
            None => {
                let at = land_queue::now();
                let head = self.branch_head(&body.branch).await?;
                let prepared = Prepared {
                    fingerprint: fingerprint.clone(),
                    account: account.into(),
                    entry: Entry {
                        id: format!("{}-{}", land_queue::new_id(at, "api"), &request_key[..16]),
                        branch: body.branch.clone(),
                        target: "main".into(),
                        issue: body.issue,
                        close: true,
                        author: account.into(),
                        machine: "api".into(),
                        summary: body.summary.unwrap_or(body.branch),
                        head,
                        submitted_at: at,
                        updated_at: at,
                        ..Entry::default()
                    },
                };
                let bytes = serde_json::to_vec(&prepared).map_err(|_| Failure::Invalid)?;
                match self.store.write_key(&receipt_key, bytes, None).await {
                    Ok(_) => prepared,
                    Err(Error::Conflict) => {
                        self.read(&receipt_key).await?.ok_or(Failure::Conflict)?
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };
        if prepared.fingerprint != fingerprint || prepared.account != account {
            return Err(Failure::Conflict);
        }
        let mut value = serde_json::to_value(&prepared.entry).map_err(|_| Failure::Invalid)?;
        value["submitted_by"] = json!(account);
        value["request_fingerprint"] = json!(fingerprint);
        let key = entry_key(&prepared.entry.id)?;
        match self
            .store
            .write_key(
                &key,
                serde_json::to_vec(&value).map_err(|_| Failure::Invalid)?,
                None,
            )
            .await
        {
            Ok(_) => Ok(value),
            Err(Error::Conflict) => {
                let existing: Value = self.read(&key).await?.ok_or(Failure::Conflict)?;
                let entry: Entry =
                    serde_json::from_value(existing.clone()).map_err(|_| Failure::Conflict)?;
                if entry.id != prepared.entry.id
                    || entry.branch != prepared.entry.branch
                    || entry.head != prepared.entry.head
                    || entry.author != prepared.entry.author
                    || entry.submitted_at != prepared.entry.submitted_at
                {
                    return Err(Failure::Conflict);
                }
                Ok(existing)
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn status(&self) -> Result<Value, Failure> {
        let mut waiting = Vec::new();
        let mut landing = Vec::new();
        let mut failed = Vec::new();
        for key in self.store.folder_keys_limit("entries", 10_000).await? {
            if !key.ends_with(".json") {
                continue;
            }
            if let Some(entry) = self.read::<Entry>(&key).await? {
                match entry.state {
                    EntryState::Queued => waiting.push(entry),
                    EntryState::Landing => landing.push(entry),
                    EntryState::Bounced => failed.push(entry),
                    _ => {}
                }
            }
        }
        for group in [&mut waiting, &mut landing, &mut failed] {
            group.sort_by(|a, b| a.id.cmp(&b.id));
        }
        let heartbeat = self.read::<Heartbeat>("worker.json").await?;
        let running = heartbeat.as_ref().is_some_and(|h| {
            land_queue::now().saturating_sub(h.at) < land_queue::WORKER_STALE_SECS
        });
        Ok(
            json!({"queue": self.location, "integrator": {"running": running, "heartbeat": heartbeat}, "waiting": waiting, "landing": landing, "failed": failed}),
        )
    }

    async fn show(&self, id: &str) -> Result<Value, Failure> {
        let entry: Entry = self.read(&entry_key(id)?).await?.ok_or(Failure::Missing)?;
        let mut records = Vec::new();
        for key in self.store.folder_keys(&format!("attempts/{id}")).await? {
            if key.ends_with(".json") {
                if let Some(record) = self.read::<Record>(&key).await? {
                    records.push(record);
                }
            }
        }
        records.sort_by_key(|r| r.number);
        let mut value = serde_json::to_value(entry).map_err(|_| Failure::Invalid)?;
        value["records"] = json!(records);
        Ok(value)
    }

    async fn withdraw(&self, account: &str, admin: bool, id: &str) -> Result<(), Failure> {
        let key = entry_key(id)?;
        let (bytes, generation) = self.store.read_key(&key).await?.ok_or(Failure::Missing)?;
        let mut value: Value = serde_json::from_slice(&bytes).map_err(|_| Failure::Invalid)?;
        // An integrator rewrites Entry and drops the API metadata. By then
        // it is no longer queued and cannot be withdrawn.
        if !admin && value["submitted_by"] != account && value["author"] != account {
            return Err(Failure::Denied);
        }
        if value["state"] == "withdrawn" {
            return Ok(());
        }
        if value["state"] != "queued" {
            return Err(Failure::Conflict);
        }
        value["state"] = json!("withdrawn");
        value["updated_at"] = json!(land_queue::now());
        self.store
            .write_key(
                &key,
                serde_json::to_vec(&value).map_err(|_| Failure::Invalid)?,
                Some(&generation),
            )
            .await?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct Prepared {
    fingerprint: String,
    account: String,
    entry: Entry,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn valid_branch(branch: &str) -> bool {
    !branch.is_empty()
        && branch.len() <= 255
        && !branch.starts_with('-')
        && !branch.contains("..")
        && !branch.contains("@{")
        && !branch.ends_with(['/', '.'])
        && branch
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
        && branch
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
}

fn entry_key(id: &str) -> Result<String, Failure> {
    valid_id(id)
        .then(|| format!("entries/{id}.json"))
        .ok_or(Failure::Invalid)
}

enum Failure {
    Store(Error),
    Invalid,
    Missing,
    Conflict,
    Denied,
    Branch,
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        match error {
            Error::Conflict => Self::Conflict,
            error => Self::Store(error),
        }
    }
}

impl Failure {
    fn response(self) -> Response {
        match self {
            Self::Store(error) => stored(&error),
            Self::Invalid => refused(
                StatusCode::BAD_REQUEST,
                "invalid_land",
                "Check the repository, branch, issue, and request ID.",
            ),
            Self::Missing => refused(
                StatusCode::NOT_FOUND,
                "not_found",
                "That change is no longer in the landing queue.",
            ),
            Self::Conflict => refused(
                StatusCode::CONFLICT,
                "land_conflict",
                "The request or change has changed. Check its status.",
            ),
            Self::Denied => refused(
                StatusCode::FORBIDDEN,
                "not_allowed",
                "You can only withdraw changes you submitted.",
            ),
            Self::Branch => refused(
                StatusCode::BAD_REQUEST,
                "branch_missing",
                "Push the branch to GitHub before submitting it.",
            ),
        }
    }
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/land", get(status).post(submit))
        .route("/v1/land/{id}", get(show).delete(withdraw))
        .route("/api/v1/land", get(status).post(submit))
        .route("/api/v1/land/{id}", get(show).delete(withdraw))
        .layer(DefaultBodyLimit::max(4096))
}

pub(crate) fn owns(path: &str) -> bool {
    ["/v1/land", "/api/v1/land"].iter().any(|prefix| {
        path.strip_prefix(prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
}

async fn authorized(
    app: &App,
    headers: &HeaderMap,
) -> Result<(String, bool, Arc<LandingQueue>), Response> {
    // These are app APIs. Cookies alone cannot authorize a mutation.
    if !headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.starts_with("Bearer "))
    {
        return Err(refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Log in to use the landing queue.",
        ));
    }
    // Bearers also reach the API host. The cookie-based account reader
    // expects the website host, even though this request has no cookie.
    let mut authentication = headers.clone();
    if let Some(service) = &app.config.cloud {
        if let Ok(origin) = service.origin().parse::<axum::http::Uri>() {
            if let Some(authority) = origin.authority() {
                if let Ok(host) = axum::http::HeaderValue::from_str(authority.as_str()) {
                    authentication.insert(header::HOST, host);
                }
            }
        }
    }
    let viewer = crate::work_runs::agent_viewer(app, &authentication).await?;
    let queue = app.config.landing.clone().ok_or_else(|| {
        refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "The landing queue isn't available here.",
        )
    })?;
    Ok((viewer.account_id, viewer.admin, queue))
}

async fn submit(State(app): State<App>, headers: HeaderMap, bytes: Bytes) -> Response {
    let (account, _, queue) = match authorized(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(request_id) = headers.get("idempotency-key").and_then(|v| v.to_str().ok()) else {
        return Failure::Invalid.response();
    };
    let Ok(body) = serde_json::from_slice(&bytes) else {
        return Failure::Invalid.response();
    };
    match queue.submit(&account, request_id, body).await {
        Ok(value) => answer(StatusCode::CREATED, value),
        Err(e) => e.response(),
    }
}

async fn status(State(app): State<App>, headers: HeaderMap) -> Response {
    let (_, _, queue) = match authorized(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match queue.status().await {
        Ok(v) => answer(StatusCode::OK, v),
        Err(e) => e.response(),
    }
}

async fn show(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (_, _, queue) = match authorized(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match queue.show(&id).await {
        Ok(v) => answer(StatusCode::OK, v),
        Err(e) => e.response(),
    }
}

async fn withdraw(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (account, admin, queue) = match authorized(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match queue.withdraw(&account, admin, &id).await {
        Ok(()) => crate::cloud::protect(StatusCode::NO_CONTENT.into_response()),
        Err(e) => e.response(),
    }
}

pub(crate) fn openapi(doc: &mut Value) {
    use openagents_sdk::land::{LandEntry, LandStatus};
    fn schema<T: schemars::JsonSchema>() -> Value {
        let settings = schemars::generate::SchemaSettings::draft2020_12().with(|s| {
            s.inline_subschemas = true;
            s.meta_schema = None;
        });
        serde_json::to_value(settings.into_generator().into_root_schema_for::<T>())
            .expect("landing schema")
    }
    for (name, schema) in [
        ("LandSubmit", schema::<LandSubmit>()),
        ("LandEntry", schema::<LandEntry>()),
        ("LandStatus", schema::<LandStatus>()),
    ] {
        doc["components"]["schemas"][name] = schema;
    }
    for (path, method, op, result) in [
        ("/v1/land", "post", "submitLanding", "LandEntry"),
        ("/v1/land", "get", "landingStatus", "LandStatus"),
        ("/v1/land/{id}", "get", "showLanding", "LandEntry"),
        ("/v1/land/{id}", "delete", "withdrawLanding", ""),
    ] {
        let mut parameters = vec![];
        if path.contains("{id}") {
            parameters
                .push(json!({"name":"id","in":"path","required":true,"schema":{"type":"string"}}));
        }
        if method == "post" {
            parameters.push(json!({"name":"Idempotency-Key","in":"header","required":true,"schema":{"type":"string","minLength":1,"maxLength":128}}));
        }
        let mut operation = json!({"operationId":op,"tags":["Landing queue"],"security":[{"appBearer":[]}],"parameters":parameters,
            "description":"Uses the configured repository's CLI landing queue. Requires an account allowed to run agent work. Reuse the request ID and exact inputs on retry. Only the submitter or an administrator may withdraw a queued change.","responses":{}});
        let status = if method == "post" {
            "201"
        } else if method == "delete" {
            "204"
        } else {
            "200"
        };
        operation["responses"][status] = json!({"description":"Landing queue result."});
        if !result.is_empty() {
            operation["responses"][status]["content"] = json!({"application/json":{"schema":{"$ref":format!("#/components/schemas/{result}")}}});
        }
        if method == "post" {
            operation["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/LandSubmit"}}}});
        }
        for status in ["400", "401", "403", "404", "409", "413", "503"] {
            operation["responses"][status] = json!({"description":"Invalid request, signed out, denied, missing, conflict, too large, or unavailable.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        if doc["paths"][path].is_null() {
            doc["paths"][path] = json!({});
        }
        doc["paths"][path][method] = operation;
    }
}

#[cfg(test)]
mod tests;
