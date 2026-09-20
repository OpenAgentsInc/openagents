use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

use crate::{
    domain::{Event, Filter, IdentityArchiveRequest, RelaySigner},
    store::{
        AdmissionOutcome, IdentityStatus, ManagementRequest, MediaDeleteOutcome, MediaRecord,
        MediaUploadOutcome, Store, StoreError, StoredEvent,
    },
};

use super::GatewayError;

#[derive(Clone)]
pub struct DbPool {
    inner: Arc<DbPoolInner>,
}

struct DbPoolInner {
    workers: Vec<mpsc::Sender<DbRequest>>,
    next: AtomicUsize,
}

#[derive(Clone)]
pub(super) struct DbProtocolConfig {
    pub relay_signer: Option<RelaySigner>,
}

enum DbRequest {
    Admit {
        event: Event,
        now: u64,
        virtual_owner: Option<String>,
        response: oneshot::Sender<Result<AdmissionOutcome, StoreError>>,
    },
    IdentityStatus {
        pubkey: String,
        response: oneshot::Sender<Result<IdentityStatus, StoreError>>,
    },
    MaterializeAgentOwner {
        agent_pubkey: String,
        owner_pubkey: String,
        require_owner_member: bool,
        response: oneshot::Sender<Result<bool, StoreError>>,
    },
    IsAgentOwner {
        agent_pubkey: String,
        owner_pubkey: String,
        response: oneshot::Sender<Result<bool, StoreError>>,
    },
    WorkspaceIcon {
        response: oneshot::Sender<Result<Option<String>, StoreError>>,
    },
    SetWorkspaceIcon {
        event: Event,
        icon: String,
        response: oneshot::Sender<Result<bool, StoreError>>,
    },
    IdentityArchive {
        event: Event,
        request: IdentityArchiveRequest,
        consent: String,
        now: u64,
        response: oneshot::Sender<Result<bool, StoreError>>,
    },
    DmVisibility {
        event: Event,
        channel: String,
        hidden: bool,
        now: u64,
        response: oneshot::Sender<Result<bool, StoreError>>,
    },
    History {
        filters: Vec<Filter>,
        now: u64,
        max_results: usize,
        cancel: watch::Receiver<bool>,
        read_pubkeys: Vec<String>,
        response: oneshot::Sender<Result<HistoryResult, StoreError>>,
    },
    Count {
        filters: Vec<Filter>,
        now: u64,
        max_count: usize,
        read_pubkeys: Vec<String>,
        response: oneshot::Sender<Result<Option<usize>, StoreError>>,
    },
    Manage {
        authorization_id: String,
        authorization_pubkey: String,
        request: ManagementRequest,
        now: u64,
        response: oneshot::Sender<Result<serde_json::Value, StoreError>>,
    },
    MediaLookup {
        sha256: String,
        response: oneshot::Sender<Result<Option<MediaRecord>, StoreError>>,
    },
    MediaUpload {
        authorization_id: String,
        authorization_pubkey: String,
        sha256: String,
        size: u64,
        media_type: String,
        uploaded_at: u64,
        max_bytes_per_pubkey: u64,
        response: oneshot::Sender<Result<MediaUploadOutcome, StoreError>>,
    },
    MediaDelete {
        authorization_id: String,
        authorization_pubkey: String,
        sha256: String,
        response: oneshot::Sender<Result<MediaDeleteOutcome, StoreError>>,
    },
    MediaFinalize {
        sha256: String,
        response: oneshot::Sender<Result<(), StoreError>>,
    },
    MediaAbandon {
        authorization_pubkey: String,
        sha256: String,
        owned_before: bool,
        response: oneshot::Sender<Result<(), StoreError>>,
    },
    CatchUp {
        after: i64,
        through: i64,
        now: u64,
        limit: usize,
        response: oneshot::Sender<Result<CatchUpResult, StoreError>>,
    },
}

#[derive(Debug)]
pub struct HistoryResult {
    pub high_water: i64,
    pub events: Vec<StoredEvent>,
}

#[derive(Debug)]
pub struct CatchUpResult {
    pub latest: i64,
    pub events: Vec<StoredEvent>,
}

impl DbPool {
    pub async fn start(
        database_url: &str,
        workers: usize,
        queue_capacity: usize,
        shutdown: watch::Sender<bool>,
        shutdown_receiver: watch::Receiver<bool>,
        current: Arc<AtomicBool>,
        protocol: DbProtocolConfig,
    ) -> Result<(Self, Vec<JoinHandle<()>>), GatewayError> {
        let mut stores = Vec::with_capacity(workers);
        for _ in 0..workers {
            stores.push(Store::connect_verified(database_url).await?);
        }

        let mut senders = Vec::with_capacity(workers);
        let mut tasks = Vec::with_capacity(workers);
        for mut store in stores {
            let (sender, mut receiver) = mpsc::channel(queue_capacity.max(1));
            senders.push(sender);
            let mut worker_shutdown = shutdown_receiver.clone();
            let failure_shutdown = shutdown.clone();
            let worker_current = Arc::clone(&current);
            let protocol = protocol.clone();
            tasks.push(tokio::spawn(async move {
                loop {
                    tokio::select! {
                        changed = worker_shutdown.changed() => {
                            if changed.is_err() || *worker_shutdown.borrow() {
                                break;
                            }
                        }
                        request = receiver.recv() => {
                            let Some(request) = request else { break };
                            let fatal = handle_request(
                                &mut store,
                                request,
                                protocol.relay_signer.as_ref(),
                            ).await;
                            if fatal || !store.is_current() {
                                worker_current.store(false, Ordering::Release);
                                let _ = failure_shutdown.send(true);
                                break;
                            }
                        }
                    }
                }
            }));
        }
        Ok((
            Self {
                inner: Arc::new(DbPoolInner {
                    workers: senders,
                    next: AtomicUsize::new(0),
                }),
            },
            tasks,
        ))
    }

    pub async fn admit(
        &self,
        event: Event,
        now: u64,
        virtual_owner: Option<String>,
    ) -> Result<AdmissionOutcome, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::Admit {
            event,
            now,
            virtual_owner,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn identity_status(&self, pubkey: String) -> Result<IdentityStatus, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::IdentityStatus { pubkey, response })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn materialize_agent_owner(
        &self,
        agent_pubkey: String,
        owner_pubkey: String,
        require_owner_member: bool,
    ) -> Result<bool, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::MaterializeAgentOwner {
            agent_pubkey,
            owner_pubkey,
            require_owner_member,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn is_agent_owner(
        &self,
        agent_pubkey: String,
        owner_pubkey: String,
    ) -> Result<bool, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::IsAgentOwner {
            agent_pubkey,
            owner_pubkey,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn workspace_icon(&self) -> Result<Option<String>, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::WorkspaceIcon { response })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn set_workspace_icon(&self, event: Event, icon: String) -> Result<bool, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::SetWorkspaceIcon {
            event,
            icon,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn process_identity_archive(
        &self,
        event: Event,
        request: IdentityArchiveRequest,
        consent: String,
        now: u64,
    ) -> Result<bool, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::IdentityArchive {
            event,
            request,
            consent,
            now,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn process_dm_visibility(
        &self,
        event: Event,
        channel: String,
        hidden: bool,
        now: u64,
    ) -> Result<bool, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::DmVisibility {
            event,
            channel,
            hidden,
            now,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn history(
        &self,
        filters: Vec<Filter>,
        now: u64,
        max_results: usize,
        cancel: watch::Receiver<bool>,
        read_pubkeys: Vec<String>,
    ) -> Result<HistoryResult, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::History {
            filters,
            now,
            max_results,
            cancel,
            read_pubkeys,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn count(
        &self,
        filters: Vec<Filter>,
        now: u64,
        max_count: usize,
        read_pubkeys: Vec<String>,
    ) -> Result<Option<usize>, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::Count {
            filters,
            now,
            max_count,
            read_pubkeys,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn manage(
        &self,
        authorization_id: String,
        authorization_pubkey: String,
        request: ManagementRequest,
        now: u64,
    ) -> Result<serde_json::Value, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::Manage {
            authorization_id,
            authorization_pubkey,
            request,
            now,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn media_blob(&self, sha256: String) -> Result<Option<MediaRecord>, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::MediaLookup { sha256, response })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn register_media(
        &self,
        authorization_id: String,
        authorization_pubkey: String,
        sha256: String,
        size: u64,
        media_type: String,
        uploaded_at: u64,
        max_bytes_per_pubkey: u64,
    ) -> Result<MediaUploadOutcome, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::MediaUpload {
            authorization_id,
            authorization_pubkey,
            sha256,
            size,
            media_type,
            uploaded_at,
            max_bytes_per_pubkey,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn delete_media(
        &self,
        authorization_id: String,
        authorization_pubkey: String,
        sha256: String,
    ) -> Result<MediaDeleteOutcome, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::MediaDelete {
            authorization_id,
            authorization_pubkey,
            sha256,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn finalize_media(&self, sha256: String) -> Result<(), StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::MediaFinalize { sha256, response })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn abandon_media(
        &self,
        authorization_pubkey: String,
        sha256: String,
        owned_before: bool,
    ) -> Result<(), StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::MediaAbandon {
            authorization_pubkey,
            sha256,
            owned_before,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    pub async fn catch_up(
        &self,
        after: i64,
        through: i64,
        now: u64,
        limit: usize,
    ) -> Result<CatchUpResult, StoreError> {
        let (response, result) = oneshot::channel();
        self.send(DbRequest::CatchUp {
            after,
            through,
            now,
            limit,
            response,
        })?;
        result.await.map_err(|_| StoreError::ConnectionClosed)?
    }

    fn send(&self, request: DbRequest) -> Result<(), StoreError> {
        let start = self.inner.next.fetch_add(1, Ordering::Relaxed) % self.inner.workers.len();
        let mut request = request;
        for offset in 0..self.inner.workers.len() {
            let index = (start + offset) % self.inner.workers.len();
            match self.inner.workers[index].try_send(request) {
                Ok(()) => return Ok(()),
                Err(mpsc::error::TrySendError::Full(returned)) => request = returned,
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    return Err(StoreError::ConnectionClosed);
                }
            }
        }
        Err(StoreError::WorkQueueFull)
    }
}

async fn handle_request(
    store: &mut Store,
    request: DbRequest,
    relay_signer: Option<&RelaySigner>,
) -> bool {
    match request {
        DbRequest::Admit {
            event,
            now,
            virtual_owner,
            response,
        } => {
            let result = store
                .admit_with_identity(&event, now, relay_signer, virtual_owner.as_deref())
                .await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::IdentityStatus { pubkey, response } => {
            let result = store.identity_status(&pubkey).await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::MaterializeAgentOwner {
            agent_pubkey,
            owner_pubkey,
            require_owner_member,
            response,
        } => {
            let result = store
                .materialize_agent_owner(&agent_pubkey, &owner_pubkey, require_owner_member)
                .await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::IsAgentOwner {
            agent_pubkey,
            owner_pubkey,
            response,
        } => {
            let result = store.is_agent_owner(&agent_pubkey, &owner_pubkey).await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::WorkspaceIcon { response } => {
            let result = store.workspace_icon().await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::SetWorkspaceIcon {
            event,
            icon,
            response,
        } => {
            let result = store.set_workspace_icon(&event, &icon).await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::IdentityArchive {
            event,
            request,
            consent,
            now,
            response,
        } => {
            let result = match relay_signer {
                Some(signer) => {
                    store
                        .process_identity_archive(&event, &request, &consent, now, signer)
                        .await
                }
                None => Err(StoreError::Management(
                    "relay signing key is required for identity archival".into(),
                )),
            };
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::DmVisibility {
            event,
            channel,
            hidden,
            now,
            response,
        } => {
            let result = match relay_signer {
                Some(signer) => {
                    store
                        .process_dm_visibility(&event, &channel, hidden, now, signer)
                        .await
                }
                None => Err(StoreError::Management(
                    "relay signing key is required for DM visibility".into(),
                )),
            };
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::History {
            filters,
            now,
            max_results,
            cancel,
            read_pubkeys,
            response,
        } => {
            let result =
                query_history(store, filters, now, max_results, cancel, read_pubkeys).await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::Count {
            filters,
            now,
            max_count,
            read_pubkeys,
            response,
        } => {
            let result = store
                .count_filters(&filters, now, max_count, &read_pubkeys)
                .await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::Manage {
            authorization_id,
            authorization_pubkey,
            request,
            now,
            response,
        } => {
            let result = store
                .manage(
                    &authorization_id,
                    &authorization_pubkey,
                    request,
                    now,
                    relay_signer,
                )
                .await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::MediaLookup { sha256, response } => {
            let result = store.media_blob(&sha256).await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::MediaUpload {
            authorization_id,
            authorization_pubkey,
            sha256,
            size,
            media_type,
            uploaded_at,
            max_bytes_per_pubkey,
            response,
        } => {
            let result = store
                .register_media(
                    &authorization_id,
                    &authorization_pubkey,
                    &sha256,
                    size,
                    &media_type,
                    uploaded_at,
                    max_bytes_per_pubkey,
                )
                .await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::MediaDelete {
            authorization_id,
            authorization_pubkey,
            sha256,
            response,
        } => {
            let result = store
                .delete_media(&authorization_id, &authorization_pubkey, &sha256)
                .await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::MediaFinalize { sha256, response } => {
            let result = store.finalize_media(&sha256).await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::MediaAbandon {
            authorization_pubkey,
            sha256,
            owned_before,
            response,
        } => {
            let result = store
                .abandon_media(&authorization_pubkey, &sha256, owned_before)
                .await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
        DbRequest::CatchUp {
            after,
            through,
            now,
            limit,
            response,
        } => {
            let result = catch_up(store, after, through, now, limit).await;
            let fatal = result.as_ref().is_err_and(is_fatal);
            let _ = response.send(result);
            fatal
        }
    }
}

async fn catch_up(
    store: &Store,
    after: i64,
    through: i64,
    now: u64,
    limit: usize,
) -> Result<CatchUpResult, StoreError> {
    let latest = store.latest_ingest_seq().await?;
    let events = store.events_after(after, through, now, limit).await?;
    Ok(CatchUpResult { latest, events })
}

async fn query_history(
    store: &Store,
    filters: Vec<Filter>,
    now: u64,
    max_results: usize,
    cancel: watch::Receiver<bool>,
    read_pubkeys: Vec<String>,
) -> Result<HistoryResult, StoreError> {
    if *cancel.borrow() {
        return Err(StoreError::QueryCancelled);
    }
    let high_water = store.latest_ingest_seq().await?;
    let search_order = filters.iter().any(|filter| filter.search.is_some());
    let mut events = HashMap::new();
    let mut order = Vec::new();
    let per_filter = max_results.div_ceil(filters.len().max(1));
    for filter in filters {
        let rows = store
            .query_filter_for(
                &filter,
                now,
                per_filter,
                high_water,
                cancel.clone(),
                &read_pubkeys,
            )
            .await?;
        for stored in rows {
            let id = stored.event.id.clone();
            if let std::collections::hash_map::Entry::Vacant(entry) = events.entry(id.clone()) {
                order.push(id);
                entry.insert(stored);
            }
        }
    }
    let mut events = if search_order {
        order
            .into_iter()
            .filter_map(|id| events.remove(&id))
            .collect::<Vec<_>>()
    } else {
        events.into_values().collect::<Vec<_>>()
    };
    if !search_order {
        events.sort_by(|left, right| {
            right
                .event
                .created_at
                .cmp(&left.event.created_at)
                .then_with(|| left.event.id.cmp(&right.event.id))
        });
    }
    events.truncate(max_results);
    Ok(HistoryResult { high_water, events })
}

fn is_fatal(error: &StoreError) -> bool {
    !matches!(
        error,
        StoreError::Domain(_)
            | StoreError::QueryCancelled
            | StoreError::WorkQueueFull
            | StoreError::TimestampOutOfRange { .. }
            | StoreError::InvalidLimit(_)
            | StoreError::Serialization(_)
            | StoreError::EphemeralTooLarge(_)
            | StoreError::Management(_)
            | StoreError::Media(_)
    )
}
