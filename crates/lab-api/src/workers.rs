//! Outbound worker requests carry only a one-space Lab key. Each operation
//! rechecks the account service before looking up its space or current lease.
use crate::*;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    routing::{get, post},
};
use lab_jobs::{authority::Authority, coordinator::Coordinator, protocol};
use serde::Deserialize;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis().try_into().unwrap_or(u64::MAX))
}
pub(crate) fn routes() -> Router<Arc<Api>> {
    Router::new()
        .route("/v1/labs/workers", post(enroll))
        .route("/v1/labs/workers/{id}", axum::routing::delete(revoke))
        .route("/v1/labs/workers/poll", post(poll))
        .route("/v1/labs/workers/start", post(start))
        .route("/v1/labs/workers/heartbeat", post(heartbeat))
        .route("/v1/labs/workers/finish", post(finish))
        .route("/v1/labs/workers/inputs", post(inputs))
        .route("/v1/labs/workers/inputs/file", post(input_file))
        .route("/v1/labs/jobs/{id}/usage", get(usage))
        .layer(DefaultBodyLimit::max(12 * 1024 * 1024))
        .layer(axum::middleware::map_response(worker_no_store))
}
async fn worker_no_store(mut response: axum::response::Response) -> axum::response::Response {
    response.headers_mut().insert(
        "Cache-Control",
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    response
}
fn no_workers() -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        "invite_only",
        "Lab workers aren't available for this account yet.",
    )
}
fn no_store() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "worker_revoked",
        "This Lab worker key no longer works. Add the worker again.",
    )
}
fn coordinator(api: &Api) -> Coordinator {
    Coordinator {
        authority: Authority::new(api.backend.clone()),
    }
}
async fn current(
    api: &Api,
    headers: &HeaderMap,
) -> Result<(protocol::Access, Store, BTreeMap<String, KindManifest>), ApiError> {
    current_access(api, headers)
        .await
        .map(|(worker, _, store, kinds)| (worker, store, kinds))
}
async fn current_access(
    api: &Api,
    headers: &HeaderMap,
) -> Result<
    (
        protocol::Access,
        Access,
        Store,
        BTreeMap<String, KindManifest>,
    ),
    ApiError,
> {
    if headers.get_all("Authorization").iter().count() != 1
        || !headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("Bearer labw_"))
    {
        return Err(no_store());
    }
    let worker = api.authorizer.worker_access(headers).await?;
    let cad_only = worker
        .worker
        .capabilities
        .jobs
        .iter()
        .all(|j| j.starts_with("cad/"));
    if !worker.experimental && !(worker.cad && cad_only) {
        return Err(no_workers());
    }
    if worker.worker.revoked
        || worker.worker.expires_unix_ms <= now()
        || worker.worker.id != worker.worker.capabilities.worker
        || worker.worker.capabilities.validate().is_err()
    {
        return Err(no_store());
    }
    let access = Access {
        account: worker.worker.account.clone(),
        space: worker.worker.space.clone(),
        experimental: worker.experimental,
        cad: worker.cad,
    };
    let kinds = worker
        .worker
        .capabilities
        .jobs
        .iter()
        .filter_map(|job| job.split_once('/').map(|(kind, _)| kind))
        .filter_map(|kind| {
            api.registry
                .get(kind, &access)
                .ok()
                .map(|k| (kind.to_owned(), k.manifest()))
        })
        .collect();
    let store = Store::new(api.backend.clone(), &worker.worker.space)?;
    Ok((worker, access, store, kinds))
}
async fn enroll(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<protocol::Enroll>,
) -> Result<Json<lab_jobs::authority::Enrolled>, ApiError> {
    request
        .capabilities
        .validate()
        .map_err(lab_store::Error::from)?;
    let (a, _) = api.scoped(&headers, &request.space, Action::Edit).await?;
    let cad_only = request
        .capabilities
        .jobs
        .iter()
        .all(|j| j.starts_with("cad/"));
    if !a.experimental && !(a.cad && cad_only) {
        return Err(no_workers());
    }
    for job in &request.capabilities.jobs {
        let (kind, job) = job
            .split_once('/')
            .ok_or_else(|| ApiError::invalid("Choose a supported Lab job."))?;
        let k = api.registry.get(kind, &a)?;
        if !k.manifest().job_types.iter().any(|j| j == job) {
            return Err(ApiError::invalid("Choose a supported Lab job."));
        }
    }
    Ok(Json(
        api.authorizer.enroll_worker(&headers, &request).await?,
    ))
}
async fn revoke(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    api.authorizer.revoke_worker(&headers, &id).await?;
    Ok(Json(serde_json::json!({"revoked":true})))
}
async fn poll(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<protocol::Poll>,
) -> Result<Json<protocol::Available>, ApiError> {
    if request.wait_seconds > 25 {
        return Err(ApiError::invalid(
            "Wait for up to 25 seconds before checking again.",
        ));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(request.wait_seconds);
    loop {
        let (access, store, kinds) = current(&api, &headers).await?;
        let work = coordinator(&api)
            .poll(&access.worker, &store, &kinds, now())
            .await?;
        if work.is_some() || std::time::Instant::now() >= deadline {
            return Ok(Json(protocol::Available { work }));
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}
async fn start(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<protocol::Attempt>,
) -> Result<Json<protocol::Work>, ApiError> {
    let (a, s, k) = current(&api, &headers).await?;
    Ok(Json(
        coordinator(&api)
            .start(&a.worker, &s, &k, &request, now())
            .await?,
    ))
}
async fn heartbeat(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<protocol::Attempt>,
) -> Result<Json<protocol::Work>, ApiError> {
    let (a, s, k) = current(&api, &headers).await?;
    Ok(Json(
        coordinator(&api)
            .heartbeat(&a.worker, &s, &k, &request, now())
            .await?,
    ))
}
async fn finish(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<protocol::Finish>,
) -> Result<Json<JobState>, ApiError> {
    let (a, s, k) = current(&api, &headers).await?;
    Ok(Json(
        coordinator(&api)
            .finish(&a.worker, &s, &k, &request, now())
            .await?,
    ))
}
#[derive(Deserialize)]
struct Space {
    #[serde(default = "personal")]
    space: String,
}
fn personal() -> String {
    "me".into()
}
async fn usage(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(p): Query<Space>,
) -> Result<Json<Vec<lab_jobs::authority::Usage>>, ApiError> {
    let (access, store) = api.scoped(&headers, &p.space, Action::Read).await?;
    let job = store.job(&id).await?;
    api.registry.get(&job.spec.kind, &access)?;
    let coordinator = coordinator(&api);
    coordinator
        .observe_losses(&access.space, &store, now())
        .await?;
    Ok(Json(coordinator.authority.usage(&access.space, &id).await?))
}

/// At most this many versions are readable for one job.
const MAX_INPUT_VERSIONS: usize = 64;
/// A pinned reference scans at most this many versions of its document.
const MAX_PIN_SCAN: u64 = 1000;
/// The largest input file a worker reads in one response.
pub const MAX_INPUT_FILE: u64 = 64 * 1024 * 1024;

fn same_space(access: &Access, space: &str) -> bool {
    let target = if space == "me" {
        SpaceRef::Personal {
            account: access.account.clone(),
        }
    } else if let Some(project) = space.strip_prefix("project:") {
        SpaceRef::Project {
            project: project.into(),
        }
    } else {
        return false;
    };
    target == access.space
}

async fn pinned(store: &Store, pin: &crate::Pin) -> Result<Version, ApiError> {
    let mut after = 0;
    while after < MAX_PIN_SCAN {
        let page = store.versions(&pin.document, after, 100).await?;
        if let Some(v) = page
            .items
            .into_iter()
            .find(|v| v.content_sha256 == pin.content_sha256)
        {
            return Ok(v);
        }
        match page.next {
            Some(next) => after = next,
            None => break,
        }
    }
    Err(ApiError::missing())
}

/// `root` and every version it references in its own space, each once,
/// with every kind checked against `access`. References to other spaces
/// aren't followed: a job's computer reads only its own space.
pub(crate) async fn referenced(
    api: &Api,
    access: &Access,
    store: &Store,
    root: &Version,
) -> Result<Vec<Version>, ApiError> {
    let mut queue = std::collections::VecDeque::from([root.clone()]);
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    while let Some(v) = queue.pop_front() {
        if !seen.insert((v.document.clone(), v.number)) {
            continue;
        }
        if out.len() >= MAX_INPUT_VERSIONS {
            return Err(ApiError::invalid("This work uses too many saved items."));
        }
        let doc = store.document(&v.document).await?;
        let kind = api.registry.get(&doc.kind, access)?;
        for r in kind
            .references(&v.content)
            .map_err(lab_store::Error::from)?
        {
            if same_space(access, &r.space) {
                queue.push_back(
                    store
                        .version(&r.version.document, r.version.version)
                        .await?,
                );
            }
        }
        for pin in kind.pins(&v.content).map_err(lab_store::Error::from)? {
            queue.push_back(pinned(store, &pin).await?);
        }
        out.push(v);
    }
    Ok(out)
}

/// The started work for `attempt` and what it may read. The worker key,
/// the space's rights, and the lease are all checked again on every call.
async fn leased(
    api: &Api,
    headers: &HeaderMap,
    attempt: &protocol::Attempt,
) -> Result<(Store, protocol::Inputs), ApiError> {
    let (worker, access, store, kinds) = current_access(api, headers).await?;
    let work = coordinator(api)
        .leased(&worker.worker, &store, &kinds, attempt, now())
        .await?;
    let versions = referenced(api, &access, &store, &work.version).await?;
    if worker.worker.capabilities.placement != lab_jobs::Placement::Enrolled
        && versions.iter().any(Version::has_private_local)
    {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "private_local",
            "This work uses private files that stay on their own computer.",
        ));
    }
    let mut files = Vec::new();
    for v in &versions {
        for a in store.version_artifacts(&v.reference()).await? {
            let used = v
                .assets
                .iter()
                .any(|e| e.sha256 == a.sha256 && e.class != AssetClass::PrivateLocal);
            if a.class != AssetClass::PrivateLocal && (a.role == ArtifactRole::Source || used) {
                files.push(a);
            }
        }
    }
    if files.len() > 512 {
        return Err(ApiError::invalid("This work uses too many files."));
    }
    Ok((store, protocol::Inputs { versions, files }))
}
async fn inputs(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<protocol::Attempt>,
) -> Result<Json<protocol::Inputs>, ApiError> {
    Ok(Json(leased(&api, &headers, &request).await?.1))
}
async fn input_file(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Json(request): Json<protocol::InputFile>,
) -> Result<axum::response::Response, ApiError> {
    use axum::response::IntoResponse;
    let attempt = protocol::Attempt {
        job: request.job,
        attempt: request.attempt,
    };
    let (store, inputs) = leased(&api, &headers, &attempt).await?;
    let file = inputs
        .files
        .into_iter()
        .find(|a| a.id == request.artifact)
        .ok_or_else(ApiError::missing)?;
    if file.size > MAX_INPUT_FILE {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_limit",
            "This file is too large for a Lab computer to read.",
        ));
    }
    let bytes = store.artifact_bytes(&file.id, None).await?;
    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        bytes,
    )
        .into_response())
}
