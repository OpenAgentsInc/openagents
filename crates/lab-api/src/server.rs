//! Existing bounded server adapters share placement, usage and cancellation
//! with outbound workers. No account credential is sent to the child process.
use crate::{ApiError, Authorizer, Kind};
use axum::http::HeaderMap;
use lab_jobs::protocol;
use lab_jobs::{
    Capabilities,
    authority::{Authority, Worker},
    coordinator::Coordinator,
};
use lab_kit::{JobState, JobStatus, SpaceRef};
use lab_store::{LabStore, Store, backend::Backend};
use std::sync::Arc;

pub struct ServerExecution {
    pub worker: Worker,
    pub authority: Authority,
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |v| v.as_millis().try_into().unwrap_or(u64::MAX))
}
pub(crate) async fn run(
    backend: Arc<dyn Backend>,
    authorizer: Arc<dyn Authorizer>,
    headers: HeaderMap,
    alias: String,
    account: String,
    space: SpaceRef,
    kind: Arc<dyn Kind>,
    capabilities: Capabilities,
    store: Store,
    job: JobState,
) -> Result<(), ApiError> {
    let authority = Authority::new(backend);
    let lifetime_ms = capabilities
        .max_seconds
        .saturating_add(60)
        .saturating_mul(1000)
        .clamp(60_000, 86_400_000);
    let issued = authorizer
        .enroll_worker(
            &headers,
            &protocol::Enroll {
                space: alias,
                capabilities,
                lifetime_ms,
            },
        )
        .await?;
    drop(headers);
    if issued.worker.account != account || issued.worker.space != space {
        return Err(ApiError::missing());
    }
    let worker = issued.worker;
    let mut scoped = HeaderMap::new();
    scoped.insert(
        "Authorization",
        format!("Bearer {}", issued.token)
            .parse()
            .map_err(|_| ApiError::invalid("Add this worker again."))?,
    );
    authority.observe(&worker, now()).await?;
    let context = ServerExecution {
        worker: worker.clone(),
        authority: authority.clone(),
    };
    {
        let run = kind.run_server_job(store.clone(), job.clone(), context);
        tokio::pin!(run);
        let mut check = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            tokio::select! {
                _ = &mut run => break,
                _ = check.tick() => {
                    let access=authorizer.worker_access(&scoped).await;
                    let allowed=access.is_ok_and(|a|a.worker==worker && (if kind.manifest().kind=="cad" {a.cad} else {a.experimental}));
                    let active=store.job(&job.id).await.is_ok_and(|j|matches!(j.status,JobStatus::Queued|JobStatus::Running));
                    if !allowed || !active {let _=store.cancel(&job.id,now()).await;break;}
                }
            }
        }
        // Dropping the adapter future stops its bounded child before permission is
        // withdrawn from its internal worker. Lost work stays uncertain in Store.
    }
    let _ = Coordinator {
        authority: authority.clone(),
    }
    .observe_losses(&space, &store, now())
    .await;
    let _ = authorizer.revoke_worker(&scoped, &worker.id).await;
    authority.revoke(&account, &worker.id).await?;
    Ok(())
}
