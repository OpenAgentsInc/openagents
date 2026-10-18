//! Host-side issue work for the public SDK, shared by app tools and other
//! clients. Execution belongs to the host and survives the calling turn.
mod gce;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use openagents_sdk::work::{self as api, Error, PoolStatus, WorkEvents, WorkRequest, WorkRun};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::{Mode, Placement, Record, Spec, Store};

/// Execution and pool control, replaceable without real cloud credentials.
pub trait Runner: Send + Sync + 'static {
    /// Check the saved agent budget and placement before saving a new run.
    fn admit(&self, request: &WorkRequest) -> Result<(), Error>;
    /// Run or recover one saved job under its durable writer lock.
    fn execute(&self, store: &Store, id: &str) -> Result<(), Error>;
    /// Read or manage the existing pool with an explicit budget actor.
    fn pool(&self, method: &str, agent: &str, hosts: u32) -> Result<PoolStatus, Error>;
}

/// Host-owned issue execution shared by SDK connections and app tools.
#[derive(Clone)]
pub struct Service {
    store: Store,
    runner: Arc<dyn Runner>,
    active: Arc<Mutex<BTreeSet<String>>>,
    pool_mutations: Arc<Mutex<()>>,
}

fn refused(message: impl Into<String>) -> Error {
    Error::refused(message)
}

impl Service {
    /// A host explicitly opts into its existing pool and local credential
    /// sources. The SDK client itself never reads these sources.
    pub fn local(root: &Path) -> Self {
        Self::new(root, Arc::new(gce::System))
    }
    /// Use an explicit job store and execution adapter, including test adapters.
    pub fn new(root: &Path, runner: Arc<dyn Runner>) -> Self {
        let service = Self {
            store: Store::under(root),
            runner,
            active: Arc::default(),
            pool_mutations: Arc::default(),
        };
        service.resume();
        service
    }

    fn resume(&self) {
        if let Ok(mut records) = self.store.list() {
            records.sort_by_key(|record| record.created_ms);
            for record in records {
                if !record.state.terminal() || !record.cleanup_complete {
                    self.kick(&record.id);
                }
            }
        }
    }

    fn kick(&self, id: &str) {
        let Ok(mut active) = self.active.lock() else {
            return;
        };
        if active.len() >= 32 || !active.insert(id.into()) {
            return;
        }
        let service = self.clone();
        let id = id.to_owned();
        std::thread::spawn(move || {
            loop {
                let result = service.runner.execute(&service.store, &id);
                if let Err(error) = result {
                    // A lost dispatch response stays resumable under the
                    // durable driver's process identity and writer lock.
                    if let Ok(lease) = service.store.lease(&id) {
                        if let Ok(mut record) = lease.read(&id) {
                            record.error = Some(error.to_string());
                            let _ = lease.save(&record);
                        }
                    }
                }
                let Ok(record) = service.store.read(&id) else {
                    break;
                };
                if record.state.terminal() && record.cleanup_complete {
                    break;
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if now.saturating_sub(record.created_ms / 1000) >= record.spec.timeout_seconds {
                    let _ = service.store.cancel(&id);
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            if let Ok(mut active) = service.active.lock() {
                active.remove(&id);
            }
            service.resume();
        });
    }

    fn run(&self, record: Record) -> Result<WorkRun, Error> {
        let request = serde_json::from_str(&record.spec.task)
            .map_err(|_| refused("This work run could not be read."))?;
        Ok(WorkRun {
            id: record.id,
            request,
            state: serde_json::to_value(record.state)
                .unwrap()
                .as_str()
                .unwrap()
                .into(),
            created_unix: record.created_ms / 1000,
            updated_unix: record.updated_ms / 1000,
            host: record.resource,
            result: record.result,
            error: record.error,
            usage: record.usage,
        })
    }

    fn start(&self, request_id: &str, request: WorkRequest) -> Result<WorkRun, Error> {
        request.validate()?;
        if !api::identifier(request_id) {
            return Err(Error::invalid("Choose a request ID."));
        }
        let id = format!(
            "work-{:x}",
            Sha256::digest(format!("{}\0{request_id}", request.agent))
        );
        let body = serde_json::to_string(&request)
            .map_err(|_| Error::invalid("The work request could not be read."))?;
        let record = if let Ok(record) = self.store.read(&id) {
            if record.spec.task != body {
                return Err(Error { code: "request_changed".into(), message: "That request ID already belongs to different work. Reuse its original request or choose a new ID.".into(), owner_command: None });
            }
            record
        } else {
            self.runner.admit(&request)?;
            let lease = self.store.lease(&id).map_err(refused)?;
            if lease.exists() {
                let record = lease.read(&id).map_err(refused)?;
                if record.spec.task != body {
                    return Err(Error::invalid(
                        "That request ID already belongs to different work.",
                    ));
                }
                record
            } else {
                let record = Record::new(
                    &id,
                    Spec {
                        placement: Placement::Gce,
                        mode: Mode::Coder,
                        agent: request.agent.clone(),
                        task: body,
                        model: None,
                        reasoning: None,
                        cwd: self.store.root().into(),
                        timeout_seconds: 12 * 3600,
                        size: "default".into(),
                        template: None,
                        credential_names: vec![],
                    },
                )
                .map_err(refused)?;
                lease.save(&record).map_err(refused)?;
                record
            }
        };
        if !record.state.terminal() || !record.cleanup_complete {
            self.kick(&id);
        }
        self.run(record)
    }

    fn handle(&self, method: &str, params: Value) -> Result<Value, Error> {
        let encode = |value| {
            serde_json::to_value(value).map_err(|_| refused("The work response could not be read."))
        };
        match method {
            api::START => {
                let id = params["request_id"]
                    .as_str()
                    .ok_or_else(|| Error::invalid("Choose a request ID."))?;
                let request = serde_json::from_value(params["request"].clone())
                    .map_err(|_| Error::invalid("Check the work request."))?;
                encode(self.start(id, request)?)
            }
            api::LIST => {
                let records = self.store.list().map_err(refused)?;
                let mut runs = vec![];
                for record in records {
                    if !record.state.terminal() || !record.cleanup_complete {
                        self.kick(&record.id);
                    }
                    runs.push(self.run(record)?);
                }
                serde_json::to_value(runs).map_err(|_| refused("The work list could not be read."))
            }
            api::EVENTS | api::CANCEL => {
                let id = params["id"]
                    .as_str()
                    .filter(|id| api::identifier(id))
                    .ok_or_else(|| Error::invalid("Choose a work run."))?;
                let record = self.store.read(id).map_err(refused)?;
                if method == api::CANCEL {
                    self.store.cancel(id).map_err(refused)?;
                    if !record.state.terminal() || !record.cleanup_complete {
                        self.kick(id);
                    }
                    return Ok(Value::Null);
                }
                let after = params["after"]
                    .as_u64()
                    .ok_or_else(|| Error::invalid("Choose a progress cursor."))?;
                let start = usize::try_from(after)
                    .map_err(|_| Error::invalid("Choose a progress cursor."))?;
                if start > record.events.len() {
                    return Err(Error::invalid("Start from a saved progress cursor."));
                }
                let end = start.saturating_add(128).min(record.events.len());
                let events = record.events[start..end].to_vec();
                let complete = record.state.terminal() && end == record.events.len();
                if !record.state.terminal() || !record.cleanup_complete {
                    self.kick(id);
                }
                serde_json::to_value(WorkEvents {
                    run: self.run(record)?,
                    events,
                    next: end as u64,
                    complete,
                })
                .map_err(|_| refused("The work progress could not be read."))
            }
            api::POOL_STATUS | api::POOL_UP | api::POOL_DOWN => {
                let agent = params["agent"]
                    .as_str()
                    .filter(|id| api::identifier(id))
                    .ok_or_else(|| Error::invalid("Choose a saved agent ID."))?;
                let hosts = params["hosts"].as_u64().unwrap_or(0);
                if method == api::POOL_UP && !(1..=32).contains(&hosts) {
                    return Err(Error::invalid("Choose 1 to 32 pool hosts."));
                }
                let _mutation = if method != api::POOL_STATUS {
                    Some(
                        self.pool_mutations
                            .lock()
                            .map_err(|_| refused("The pool connection stopped."))?,
                    )
                } else {
                    None
                };
                let status = self.runner.pool(method, agent, hosts as u32)?;
                if method == api::POOL_DOWN {
                    Ok(Value::Null)
                } else {
                    serde_json::to_value(status)
                        .map_err(|_| refused("The pool response could not be read."))
                }
            }
            _ => Err(Error::invalid("Choose a Work or Pool operation.")),
        }
    }
}

impl api::Transport for Service {
    fn request<'a>(
        &'a self,
        method: &'static str,
        params: Value,
        cancel: &'a openagents_sdk::CancellationToken,
    ) -> api::Reply<'a> {
        let service = self.clone();
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Err(Error::cancelled());
            }
            tokio::task::spawn_blocking(move || service.handle(method, params))
                .await
                .map_err(|_| refused("The work connection stopped."))?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::State;
    use openagents_sdk::{
        CancellationToken,
        work::{LandMode, Pool, Work},
    };
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct Fake {
        admitted: Mutex<Vec<String>>,
        starts: AtomicUsize,
        refuse: AtomicBool,
        lose_response: AtomicBool,
    }
    impl Runner for Fake {
        fn admit(&self, request: &WorkRequest) -> Result<(), Error> {
            if self.refuse.load(Ordering::SeqCst) {
                return Err(Error {
                    code: "refused".into(),
                    message: "Ask the owner for a compute grant.".into(),
                    owner_command: Some(format!(
                        "openagents cloud budget --agent {} --daily 10",
                        request.agent
                    )),
                });
            }
            self.admitted.lock().unwrap().push(request.agent.clone());
            Ok(())
        }
        fn execute(&self, store: &Store, id: &str) -> Result<(), Error> {
            let lease = store.lease(id).map_err(refused)?;
            let mut record = lease.read(id).map_err(refused)?;
            if record.state == State::Created {
                self.starts.fetch_add(1, Ordering::SeqCst);
                record.state = State::Running;
                record.events = (0..260).map(|i| json!({"sequence":i})).collect();
                lease.save(&record).map_err(refused)?;
                if self.lose_response.swap(false, Ordering::SeqCst) {
                    return Err(refused("The response was lost."));
                }
            }
            let until = Instant::now() + Duration::from_secs(10);
            while !lease.cancelled() {
                if Instant::now() >= until {
                    return Err(refused("test deadline"));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            record.state = State::Cancelled;
            record.cleanup_complete = true;
            lease.save(&record).map_err(refused)
        }
        fn pool(&self, _: &str, agent: &str, _: u32) -> Result<PoolStatus, Error> {
            if self.refuse.load(Ordering::SeqCst) {
                return Err(Error {
                    code: "refused".into(),
                    message: "Ask the owner to start the pool.".into(),
                    owner_command: Some(format!("openagents cloud up --hosts 1 --agent {agent}")),
                });
            }
            Ok(PoolStatus {
                granted: true,
                slots: 2,
                ..PoolStatus::default()
            })
        }
    }
    fn request(agent: &str) -> WorkRequest {
        WorkRequest {
            repo: "example/project".into(),
            issue: 12,
            agent: agent.into(),
            land: LandMode::Queue,
            engine: Some("briefed:codex".into()),
            on: Some(api::Placement::Gce),
        }
    }
    async fn until(service: &Service, id: &str, predicate: impl Fn(&Record) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if predicate(&service.store.read(id).unwrap()) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "work did not reach the expected state"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn work_outlives_a_read_retries_once_and_pages_progress_before_cancellation() {
        let root = tempfile::tempdir().unwrap();
        let runner = Arc::new(Fake::default());
        runner.lose_response.store(true, Ordering::SeqCst);
        let service = Service::new(root.path(), runner.clone());
        let work = Work::new(&service);
        let cancel = CancellationToken::new();
        let first = work
            .start("request-12", request("agt_builder"), &cancel)
            .await
            .unwrap();
        until(&service, &first.id, |record| record.state == State::Running).await;
        let again = work
            .start("request-12", request("agt_builder"), &cancel)
            .await
            .unwrap();
        assert_eq!(first.id, again.id);
        let mut changed = request("agt_builder");
        changed.issue = 13;
        assert_eq!(
            work.start("request-12", changed, &cancel)
                .await
                .unwrap_err()
                .code,
            "request_changed"
        );
        let page = work.events(&first.id, 0, &cancel).await.unwrap();
        assert_eq!(page.events.len(), 128);
        let next = work.events(&first.id, page.next, &cancel).await.unwrap();
        assert_eq!(next.events[0]["sequence"], 128);
        assert!(!next.complete);
        assert_eq!(runner.starts.load(Ordering::SeqCst), 1);
        let read_cancel = CancellationToken::new();
        read_cancel.cancel();
        assert_eq!(
            work.events(&first.id, 0, &read_cancel)
                .await
                .unwrap_err()
                .code,
            "cancelled"
        );
        work.cancel(&first.id, &cancel).await.unwrap();
        until(&service, &first.id, |record| {
            record.state == State::Cancelled
        })
        .await;
        assert_eq!(
            runner.starts.load(Ordering::SeqCst),
            1,
            "lost response must recover without dispatching twice"
        );
    }
    #[tokio::test]
    async fn missing_grants_return_owner_commands_without_a_job_or_host_start() {
        let root = tempfile::tempdir().unwrap();
        let runner = Arc::new(Fake::default());
        runner.refuse.store(true, Ordering::SeqCst);
        let service = Service::new(root.path(), runner.clone());
        let cancel = CancellationToken::new();
        let error = Work::new(&service)
            .start("id", request("agt_reviewer"), &cancel)
            .await
            .unwrap_err();
        assert!(
            error
                .owner_command
                .unwrap()
                .contains("--agent agt_reviewer")
        );
        assert!(service.store.list().unwrap().is_empty());
        assert_eq!(runner.starts.load(Ordering::SeqCst), 0);
        assert!(
            Pool::new(&service, "agt_reviewer")
                .unwrap()
                .up(1, &cancel)
                .await
                .unwrap_err()
                .owner_command
                .unwrap()
                .starts_with("openagents cloud up")
        );
    }
    #[tokio::test]
    async fn request_identity_and_budget_actor_are_independent_for_any_two_saved_agents() {
        let root = tempfile::tempdir().unwrap();
        let runner = Arc::new(Fake::default());
        let service = Service::new(root.path(), runner.clone());
        let cancel = CancellationToken::new();
        let work = Work::new(&service);
        let a = work
            .start("same-request", request("agt_builder"), &cancel)
            .await
            .unwrap();
        let b = work
            .start("same-request", request("agt_reviewer"), &cancel)
            .await
            .unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(
            *runner.admitted.lock().unwrap(),
            ["agt_builder", "agt_reviewer"]
        );
        for id in [&a.id, &b.id] {
            work.cancel(id, &cancel).await.unwrap();
            until(&service, id, |record| record.state == State::Cancelled).await;
        }
    }
}
