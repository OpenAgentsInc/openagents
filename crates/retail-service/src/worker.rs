//! Bounded lifecycle advancement independent of HTTP connections.

use crate::store::{vault_read, vault_remove};
use crate::types::{Confirmation, STEP_MAX, WorkerReport};
use crate::{Backend, Error, Result, Service};
use openagents_wallet::LightningWallet;
use retail_cloud::{
    cancel, contract,
    dispatch::TaskStatus,
    material::{self, Credential, CustomerSecret},
    offer::{self, ConfirmedVia, FundedRequest},
    provision::{self, Provider, ProvisionState},
    recover::{self, State},
    retain, settle,
};
use route_contract::price_book::Ending;
use rusqlite::params;
use std::sync::Arc;
use std::time::Duration;

impl<B: Backend, W: LightningWallet + Send + Sync + 'static> Service<B, W> {
    /// Recover and advance at most sixteen confirmed executions. The cursor
    /// is durable so one blocked resource cannot starve later customers.
    /// Unknown observations preserve holds; errors do not stop other duties.
    pub fn tick(&self, now: i64) -> Result<WorkerReport> {
        if now < 0 {
            return Err(Error::Invalid("invalid worker time"));
        }
        let mut guard = self.lock()?;
        let store = &mut *guard;
        retail_cloud::topup::reconcile(&mut store.ledger, &*self.wallet, now)?;
        store.check()?;
        // Saved environments: settle known endings once against the
        // month's hours, and retire images kept past a subscription's end.
        let environments = retail_cloud::environment::recover(&mut store.journal, now);
        store.check()?;
        let mut expired = store.db.prepare(
            "SELECT id FROM offer WHERE confirmation IS NULL AND done=0 AND created_at<=? ORDER BY created_at,id LIMIT 16",
        )?;
        let expired_ids = expired
            .query_map([now.saturating_sub(offer::OFFER_TTL_SECS as i64)], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(expired);
        for id in expired_ids {
            vault_remove(&self.config.state, &id)?;
            store
                .db
                .execute("UPDATE offer SET done=1 WHERE id=?", [&id])?;
        }
        let cursor: String =
            store
                .db
                .query_row("SELECT cursor FROM worker WHERE id=1", [], |r| r.get(0))?;
        let mut query=store.db.prepare("SELECT id FROM offer WHERE confirmation IS NOT NULL AND done=0 ORDER BY CASE WHEN id>? THEN 0 ELSE 1 END,id LIMIT ?")?;
        let ids = query
            .query_map(params![cursor, STEP_MAX], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(query);
        let mut report = WorkerReport {
            visited: 0,
            pending: 0,
            failed: vec![],
        };
        if environments.is_err() {
            report.failed.push("environments".into());
        }
        for id in ids {
            report.visited += 1;
            match self.advance(store, &id, now) {
                Ok(done) => {
                    report.pending += usize::from(!done);
                    if done {
                        store
                            .db
                            .execute("UPDATE offer SET done=1 WHERE id=?", [&id])?;
                    }
                }
                Err(_) => {
                    report.pending += 1;
                    report.failed.push(id.clone());
                }
            }
            store.check()?;
            store
                .db
                .execute("UPDATE worker SET cursor=? WHERE id=1", [id])?;
        }
        // Expiration is part of the existing retention worker, not an HTTP
        // request. It also reconciles cleanup after the launching client left.
        let bindings = store
            .journal
            .all_funded()?
            .into_iter()
            .map(|f| (retail_cloud::dispatch::task_id(&f.execution), f.offer))
            .collect();
        let artifacts = SafeArtifacts {
            backend: &*self.backend,
            state: &self.config.state,
            bindings,
        };
        retain::worker_step(&mut store.journal, &*self.backend, &artifacts, now)?;
        store.check()?;
        Ok(report)
    }
    fn advance(&self, store: &mut crate::store::Store, id: &str, now: i64) -> Result<bool> {
        let (made, _, _, confirmation) = store
            .offer(id)?
            .ok_or(Error::Conflict("worker offer disappeared"))?;
        let confirmation =
            confirmation.ok_or(Error::Conflict("worker has no accepted confirmation"))?;
        if store.commercial(id)? != confirmation.commercial {
            return Err(Error::Denied);
        }
        if confirmation.custody != crate::custody_digest(&made, confirmation.commercial.as_ref()) {
            return Err(Error::Conflict("accepted custody policy changed"));
        }
        let custody_deadline = confirmation
            .at
            .saturating_add(made.quote.max_seconds)
            .saturating_add(2 * provision::READY_DEADLINE_SECS as u64)
            .saturating_add(900);
        if now as u64 >= custody_deadline {
            vault_remove(&self.config.state, id)?;
        }
        let capacity = self.capacity(store, Some(id))?;
        let funded = offer::confirm(
            &mut store.journal,
            &made,
            &made.offer.digest,
            ConfirmedVia::OfferControl,
            &contract::price_book(),
            capacity,
            confirmation.at,
        )?;
        let current = self.current(store, &funded, &confirmation)?;
        if store.ledger.hold(&funded.request)?.is_none() {
            let revoked = current.execute.as_ref().is_some_and(|g| g.revoked)
                || current.disclose.as_ref().is_some_and(|c| c.withdrawn);
            if revoked
                && store.journal.provisioning(&funded.execution)?.is_none()
                && store.journal.dispatch(&funded.execution)?.is_none()
            {
                // A crash before reservation creates no charge or release.
                // Revocation still removes custody and retires the admission.
                cancel::revoke(&mut store.journal, &funded, &current, now)?;
                let cleanup = cancel::advance_bounded(
                    &mut store.journal,
                    &store.ledger,
                    &*self.backend,
                    &*self.backend,
                    &*self.backend,
                    &funded.execution,
                    now,
                )?;
                if cleanup.provider_deleted {
                    vault_remove(&self.config.state, id)?;
                }
                recover::step_bounded(
                    &mut store.journal,
                    &mut store.ledger,
                    &*self.backend,
                    &*self.backend,
                    &funded,
                    now,
                )?;
                return Ok(cleanup.provider_deleted);
            }
            // Never reserve money after revocation. Accepted but unfunded
            // confirmations stay private and cannot provision a computer.
            retail_cloud::reserve::reserve(&mut store.ledger, &funded, &current, now)?;
        }
        let snapshot = recover::step_bounded(
            &mut store.journal,
            &mut store.ledger,
            &*self.backend,
            &*self.backend,
            &funded,
            now,
        )?;
        // Provider/task reconciliation can block. Do not carry an authority
        // read from before those calls into a later side effect.
        let current = self.current(store, &funded, &confirmation)?;
        if settle::observe(&store.journal, &funded, &Self::read_current(&funded))?.is_some() {
            return self.finish_cleanup(store, &funded, &confirmation, now);
        }
        let revoked = current.execute.as_ref().is_some_and(|g| g.revoked)
            || current.disclose.as_ref().is_some_and(|c| c.withdrawn);
        if revoked && !store.journal.cancellation_requested(&funded.execution)? {
            cancel::revoke(&mut store.journal, &funded, &current, now)?;
        }
        if let Some(usage) = store.journal.usage(&funded.execution)?
            && !usage.final_usage
        {
            retail_cloud::meter::poll_bounded(
                &mut store.journal,
                &*self.backend,
                &funded.execution,
                &format!("worker:{}", funded.execution),
                now,
            )?;
        }
        let end = match &snapshot.state {
            State::TaskObserved {
                status: TaskStatus::Ended { .. },
                ..
            } => Some(Ending::ExecutorEnded),
            State::NewOfferRequired { .. } => Some(Ending::ProviderLostAfterExecutor),
            State::Refused { .. } => None,
            _ => match store
                .journal
                .provisioning(&funded.execution)?
                .map(|p| p.state)
            {
                Some(ProvisionState::Refused { .. }) => Some(Ending::NotStarted),
                Some(ProvisionState::Unavailable) => Some(Ending::ProviderUnavailable),
                _ => None,
            },
        };
        if matches!(snapshot.state, State::Refused { .. })
            && !store.journal.cancellation_requested(&funded.execution)?
        {
            cancel::request(&mut store.journal, &funded, &current, now)?;
        }
        if let Some(ending) = end {
            store.set_ending(id, ending)?;
            retain::request(&mut store.journal, &funded, now)?;
        }
        let absolute_deadline = (funded.confirmed_at as i64)
            .saturating_add(funded.quote.max_seconds as i64)
            .saturating_add(2 * provision::READY_DEADLINE_SECS);
        let metered_stop = store
            .journal
            .usage(&funded.execution)?
            .is_some_and(|u| u.ceiling_reached || now >= u.deadline_at);
        if (now >= absolute_deadline || metered_stop)
            && store.ending(id)?.is_none()
            && !store.journal.cancellation_requested(&funded.execution)?
        {
            cancel::request(&mut store.journal, &funded, &current, now)?;
        }
        if store.journal.cancellation_requested(&funded.execution)?
            || store
                .journal
                .retention_receipt(&funded.execution, now)?
                .is_some()
        {
            return self.finish_cleanup(store, &funded, &confirmation, now);
        }
        if matches!(snapshot.state, State::Unknown { provider: true, .. })
            && store.journal.dispatch(&funded.execution)?.is_none()
            && let Some(provision) = store.journal.provisioning(&funded.execution)?
            && matches!(provision.state, ProvisionState::Creating)
            && store
                .ledger
                .hold(&funded.request)?
                .is_some_and(|h| h.state == pay_ledger::compute::HoldState::Held)
        {
            let current = self.current(store, &funded, &confirmation)?;
            retail_cloud::authority::check(
                retail_cloud::authority::Step::Provision,
                &funded.admission,
                &current,
            )
            .map_err(retail_cloud::Error::Denied)?;
            let spec = provision::spec(&funded, provision.attempt, &self.config.template);
            if let Some(resource) = self
                .backend
                .reconcile_creation(&spec, now)
                .map_err(|_| Error::Unavailable("original create reconciliation is unavailable"))?
                && (resource.account != funded.account
                    || resource.provisioning != spec.provisioning)
            {
                return Err(Error::Conflict("reconciled create has another identity"));
            }
        }
        match snapshot.state {
            State::AwaitingProvision | State::ProvisionObserved { .. } => {
                let current = self.current(store, &funded, &confirmation)?;
                provision::advance(
                    &mut store.journal,
                    &store.ledger,
                    &*self.backend,
                    &funded,
                    &current,
                    &self.config.template,
                    now,
                )?;
            }
            State::AwaitingMaterial { resource } => {
                let current = self.current(store, &funded, &confirmation)?;
                let mut key = vault_read(&self.config.state, id)?
                    .ok_or(Error::Unavailable("accepted customer credential is absent"))?;
                if retail_cloud::sha256_hex(key.as_str().as_bytes()) != confirmation.key_digest {
                    return Err(Error::Conflict("private credential changed"));
                }
                material::deliver(
                    &mut store.journal,
                    &*self.backend,
                    &funded,
                    &current,
                    &resource,
                    &funded.admission.source,
                    &Credential::ApiKey {
                        provider: "openai".into(),
                        secret: CustomerSecret::new(key.take()),
                    },
                    now,
                )?;
            }
            State::AwaitingDispatch { .. } => {
                // Re-read all current authorities immediately before the
                // required disclose/spend/execute checks in dispatch_metered.
                let current = self.current(store, &funded, &confirmation)?;
                retail_cloud::meter::dispatch_metered(
                    &mut store.journal,
                    &store.ledger,
                    &*self.backend,
                    &*self.backend,
                    &funded,
                    &current,
                    &contract::price_book(),
                    now,
                )?;
            }
            // Unknown task/provider observations are never a dispatch retry
            // or proof of absence. Recovery must find the exact task first.
            _ => {}
        }
        Ok(false)
    }
    fn finish_cleanup(
        &self,
        store: &mut crate::store::Store,
        funded: &FundedRequest,
        _confirmation: &Confirmation,
        now: i64,
    ) -> Result<bool> {
        // Remove the customer key before teardown when the resource still
        // answers. Failure does not postpone deletion or free the cost hold.
        let _ = material::remove_credentials(
            &mut store.journal,
            &*self.backend,
            &funded.execution,
            now,
        );
        let artifacts = SafeArtifacts {
            backend: &*self.backend,
            state: &self.config.state,
            bindings: std::collections::BTreeMap::from([(
                retail_cloud::dispatch::task_id(&funded.execution),
                funded.offer.clone(),
            )]),
        };
        let cancellation = if store.journal.cancellation_requested(&funded.execution)? {
            Some(cancel::advance_bounded(
                &mut store.journal,
                &store.ledger,
                &*self.backend,
                &*self.backend,
                &artifacts,
                &funded.execution,
                now,
            )?)
        } else {
            None
        };
        let retention = retain::advance(
            &mut store.journal,
            &*self.backend,
            &artifacts,
            &funded.execution,
            now,
        )?;
        if retention.deleted() {
            vault_remove(&self.config.state, &funded.offer)?;
        }
        if let Some(usage) = store.journal.usage(&funded.execution)?
            && !usage.final_usage
        {
            retail_cloud::meter::poll_bounded(
                &mut store.journal,
                &*self.backend,
                &funded.execution,
                &format!("worker:final:{}", funded.execution),
                now,
            )?;
        }
        store.check()?;
        // Preserve the owner's terminal/loss evidence before cleanup changes
        // the read-side projection. A lost stop acknowledgment remains unknown.
        let ending = store.ending(&funded.offer)?.or_else(|| {
            cancellation
                .as_ref()
                .and_then(|c| c.executor.as_ref())
                .map(|e| {
                    if !e.started {
                        Ending::NotStarted
                    } else if matches!(e.status, TaskStatus::Ended { .. }) {
                        Ending::ExecutorEnded
                    } else {
                        Ending::Cancelled
                    }
                })
        });
        if let Some(ending) = ending {
            store.set_ending(&funded.offer, ending)?;
            let receipt =
                settle::settle(&mut store.journal, &mut store.ledger, funded, ending, now)?;
            return Ok(retention.deleted() && receipt.held_msat == 0);
        }
        store.check()?;
        let _ = settle::settle(
            &mut store.journal,
            &mut store.ledger,
            funded,
            Ending::Unknown,
            now,
        )?;
        Ok(false)
    }
    /// Start a resident worker. HTTP client lifetime has no effect on it.
    /// The caller stops it only for service shutdown; its records resume on
    /// the next process's first pass.
    pub fn spawn_worker(self: &Arc<Self>, period: Duration) -> Worker {
        let service = Arc::clone(self);
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let signal = Arc::clone(&stop);
        let period = period.clamp(Duration::from_secs(1), Duration::from_secs(30));
        let thread = std::thread::spawn(move || {
            while !signal.load(std::sync::atomic::Ordering::Relaxed) {
                let now = crate::http::now();
                let result = service.tick(now);
                if let Some(ops) = &service.operations
                    && let Ok(mut status) = ops.last_worker.lock()
                {
                    *status = Some((now, result.as_ref().is_ok_and(|r| r.failed.is_empty())));
                }
                // Short waits let shutdown preserve and release state promptly.
                let deadline = std::time::Instant::now() + period;
                while std::time::Instant::now() < deadline
                    && !signal.load(std::sync::atomic::Ordering::Relaxed)
                {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
        Worker {
            stop,
            thread: Some(thread),
        }
    }
}
pub struct Worker {
    stop: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Refuse secret-bearing artifact bytes before they reach the retained
/// journal. Without the exact custody key, collection remains incomplete;
/// cleanup still proceeds. This also catches keys embedded inside a token.
struct SafeArtifacts<'a, B> {
    backend: &'a B,
    state: &'a std::path::Path,
    bindings: std::collections::BTreeMap<String, String>,
}
impl<B: retail_cloud::retain::Artifacts> retail_cloud::retain::Artifacts for SafeArtifacts<'_, B> {
    fn manifest(
        &self,
        resource: &str,
        task: &str,
    ) -> retail_cloud::Result<retail_cloud::retain::Manifest> {
        self.backend.manifest(resource, task)
    }
    fn read(
        &self,
        resource: &str,
        task: &str,
        name: &str,
        max_bytes: usize,
    ) -> retail_cloud::Result<Vec<u8>> {
        let offer = self.bindings.get(task).ok_or(retail_cloud::Error::Invalid(
            "artifact has no custody binding",
        ))?;
        let key = vault_read(self.state, offer)
            .map_err(|_| retail_cloud::Error::Invalid("artifact custody is unavailable"))?
            .ok_or(retail_cloud::Error::Invalid("artifact custody has ended"))?;
        let bytes = self.backend.read(resource, task, name, max_bytes)?;
        if !key.as_str().is_empty()
            && bytes
                .windows(key.as_str().len())
                .any(|w| w == key.as_str().as_bytes())
        {
            return Err(retail_cloud::Error::Invalid(
                "artifact contains private credential material",
            ));
        }
        Ok(bytes)
    }
}
