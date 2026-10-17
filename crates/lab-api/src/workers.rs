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
        .route("/v1/labs/jobs/{id}/usage", get(usage))
        .layer(DefaultBodyLimit::max(12 * 1024 * 1024))
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
    if headers.get_all("Authorization").iter().count() != 1
        || !headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("Bearer labw_"))
    {
        return Err(no_store());
    }
    let worker = api.authorizer.worker_access(headers).await?;
    if !worker.experimental {
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
    Ok((worker, store, kinds))
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
    if !a.experimental {
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
    Ok(Json(
        coordinator(&api)
            .authority
            .usage(&access.space, &id)
            .await?,
    ))
}
