//! A bounded private RPC exposes only admitted product operations.
use crate::{Controller, Error, Result, now, private};
use pay_ledger::shared::{Envelope, Intent, Liability, Operation};
use receipts::funding_units::Unit;
use receipts::purchase::CommercialProduct;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
impl Controller {
    pub fn run(self: Arc<Self>, stop: Arc<AtomicBool>) -> Result<()> {
        let parent = self.config.socket.parent().ok_or(Error::Denied)?;
        let m = std::fs::symlink_metadata(parent)?;
        if !m.is_dir() || m.mode() & 0o077 != 0 || m.uid() != unsafe { libc::geteuid() } {
            return Err(Error::Denied);
        }
        if self.config.socket.exists() {
            private::socket(&self.config.socket)?;
            if UnixStream::connect(&self.config.socket).is_ok() {
                return Err(Error::Denied);
            }
            std::fs::remove_file(&self.config.socket)?;
        }
        let listener = UnixListener::bind(&self.config.socket)?;
        std::fs::set_permissions(&self.config.socket, std::fs::Permissions::from_mode(0o600))?;
        let socket_identity = std::fs::symlink_metadata(&self.config.socket)?;
        listener.set_nonblocking(true)?;
        let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        while !stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    if active.fetch_add(1, Ordering::SeqCst) >= 32 {
                        active.fetch_sub(1, Ordering::SeqCst);
                        drop(stream);
                        continue;
                    }
                    let controller = self.clone();
                    let running = active.clone();
                    std::thread::spawn(move || {
                        controller.answer(stream);
                        running.fetch_sub(1, Ordering::SeqCst);
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => return Err(e.into()),
            }
        }
        let end = std::fs::symlink_metadata(&self.config.socket)?;
        if end.dev() == socket_identity.dev() && end.ino() == socket_identity.ino() {
            std::fs::remove_file(&self.config.socket)?;
        }
        Ok(())
    }
    fn answer(&self, mut stream: UnixStream) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        let mut line = String::new();
        let result = (|| {
            BufReader::new(&stream)
                .take(pay_ledger::shared::BODY_MAX as u64 + 1)
                .read_line(&mut line)?;
            if line.len() > pay_ledger::shared::BODY_MAX || !line.ends_with('\n') {
                return Err(Error::Denied);
            }
            let value: Value = serde_json::from_str(&line)?;
            if let Some(permit) = value.get("custody_authorization") {
                if value.as_object().is_none_or(|o| o.len() != 1) {
                    return Err(Error::Denied);
                }
                self.custody_authorization(serde_json::from_value(permit.clone())?)
            } else {
                self.request(serde_json::from_value(value)?)
            }
        })();
        let reply = match result {
            Ok(value) => value,
            Err(error) => {
                let code = match error {
                    Error::Wallet(_) => "custodian_refused",
                    Error::Ledger(_) => "canonical_ledger_refused",
                    Error::Accounts(_) => "native_account_custody_changed",
                    Error::Json(_) => "invalid_shared_document",
                    Error::Io(_) => "shared_state_unavailable",
                    Error::Funds => "shared_funds_insufficient",
                    Error::Unknown => "original_reconciliation_required",
                    Error::Denied => "shared_authority_refused",
                };
                json!({"origin":self.config.origin,"error":code})
            }
        };
        let _ = writeln!(stream, "{reply}");
    }
    fn request(&self, envelope: Envelope) -> Result<Value> {
        let g = self.grant(&envelope.binding)?;
        if envelope.origin != self.config.origin
            || crate::digest(envelope.authorization.as_bytes()) != g.credential_digest
        {
            return Err(Error::Denied);
        }
        let product = g.binding.source.product;
        let value = match envelope.operation {
            Operation::Binding {} => {
                let _ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
                self.current(g, true)?;
                serde_json::to_value(&g.binding)?
            }
            Operation::Identity {} => {
                let _ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
                self.current(g, false)?;
                serde_json::to_value(&g.binding)?
            }
            Operation::DispatchPlugin { intent, wait_secs }
                if product == CommercialProduct::Plugin =>
            {
                if intent.binding.id != g.binding.id {
                    return Err(Error::Denied);
                }
                self.dispatch_plugin(intent, wait_secs)?
            }
            Operation::ReconcilePlugin { id } if product == CommercialProduct::Plugin => {
                self.reconcile_plugin(&id, &g.binding.id)?
            }
            Operation::Funding {
                purchase,
                amount_sats,
            } => self.funding(&g.binding.id, &purchase, amount_sats)?,
            Operation::ReverseFunding { review } => self.reverse_funding(&g.binding.id, &review)?,
            Operation::FundingStatus { purchase } => {
                self.funding_status(&g.binding.id, &purchase)?
            }
            Operation::Refund { review } => self.refund(&g.binding.id, &review)?,
            Operation::RefundStatus { review } => self.refund_status(&g.binding.id, &review)?,
            operation => {
                let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
                let new_effect = matches!(
                    operation,
                    Operation::Reserve { .. }
                        | Operation::RetailReserve { .. }
                        | Operation::Handoff { .. }
                );
                self.current(g, new_effect)?;
                match operation {
                    Operation::Reserve {
                        intent,
                        projection_head,
                        actor,
                    } if product == CommercialProduct::Gateway => {
                        if intent.binding != g.binding
                            || intent.invoice.is_some()
                            || !matches!(&intent.liability,Liability::NativeService{resource} if resource=="openagents.gateway.systemone.v1")
                        {
                            return Err(Error::Denied);
                        }
                        let proof = self.gateway_actor(g, &actor)?;
                        ledger.shared_seal_native_actor(&intent.id, &proof)?;
                        let result = serde_json::to_value(
                            ledger.shared_reserve_projected(&intent, projection_head)?,
                        )?;
                        if self.gateway_actor(g, &actor)? != proof {
                            return Err(Error::Unknown);
                        }
                        result
                    }
                    Operation::RetailReserve { request }
                        if product == CommercialProduct::Retail =>
                    {
                        if request.account != g.binding.source.account
                            || g.binding.conversion.source != Unit::Millisatoshis
                            || g.binding.conversion.numerator != g.binding.conversion.denominator
                            || request.amount_msat <= 0
                        {
                            return Err(Error::Denied);
                        }
                        let intent = Intent {
                            id: Intent::stable_id(&g.binding, &request.id),
                            binding: g.binding.clone(),
                            native_attempt: request.id.clone(),
                            quote: request.quote.clone(),
                            execution: request.execution.clone(),
                            terms: request.terms.clone(),
                            maximum_units: request.amount_msat as u64,
                            fee_cap_msat: 0,
                            invoice: None,
                            liability: Liability::NativeService {
                                resource: pay_ledger::compute::hold::RETAIL_RESOURCE.into(),
                            },
                            admitted_at: request.at.try_into().map_err(|_| Error::Denied)?,
                        };
                        let out = if let Some(old) = ledger.shared_outcome(&intent.id)? {
                            let original = pay_ledger::Ledger::shared_retail_hold(&old)?;
                            if original.request.account != request.account
                                || original.request.quote != request.quote
                                || original.request.execution != request.execution
                                || original.request.terms != request.terms
                                || original.request.amount_msat != request.amount_msat
                            {
                                return Err(Error::Denied);
                            }
                            old
                        } else {
                            ledger.shared_reserve(&intent)?
                        };
                        serde_json::to_value(pay_ledger::Ledger::shared_retail_hold(&out)?)?
                    }
                    Operation::Balance {} => {
                        serde_json::to_value(ledger.shared_balance(&g.binding.id)?)?
                    }
                    Operation::SourceOutcomes { after, through } => serde_json::to_value(
                        ledger.shared_source_outcomes(&g.binding, after, through)?,
                    )?,
                    Operation::Observe { id } => {
                        let o = ledger.shared_outcome(&id)?.ok_or(Error::Denied)?;
                        if !o.intent.binding.mode().same_native(&g.binding.mode()) {
                            return Err(Error::Denied);
                        }
                        serde_json::to_value(o)?
                    }
                    Operation::Handoff { id, actor } if product == CommercialProduct::Gateway => {
                        self.owned(&ledger, &id, &g.binding.id)?;
                        if ledger
                            .shared_outcome(&id)?
                            .is_none_or(|o| o.intent.binding != g.binding)
                        {
                            return Err(Error::Denied);
                        }
                        let proof = self.gateway_actor(g, &actor)?;
                        if ledger.shared_native_actor(&id)?.as_ref() != Some(&proof) {
                            return Err(Error::Denied);
                        }
                        let result = serde_json::to_value(ledger.shared_handoff(&id)?)?;
                        if self.gateway_actor(g, &actor)? != proof {
                            return Err(Error::Unknown);
                        }
                        result
                    }
                    Operation::Settle {
                        id,
                        units,
                        evidence,
                    } if product == CommercialProduct::Gateway => {
                        self.owned(&ledger, &id, &g.binding.id)?;
                        serde_json::to_value(ledger.shared_settle(
                            &id,
                            units,
                            0,
                            &evidence,
                            None,
                            now() as i64,
                        )?)?
                    }
                    Operation::ReleaseUndispatched { id, evidence }
                        if product != CommercialProduct::Plugin =>
                    {
                        self.owned(&ledger, &id, &g.binding.id)?;
                        serde_json::to_value(ledger.shared_release_undispatched(
                            &id,
                            &evidence,
                            now() as i64,
                        )?)?
                    }
                    Operation::Unknown { id } if product != CommercialProduct::Plugin => {
                        self.owned(&ledger, &id, &g.binding.id)?;
                        serde_json::to_value(ledger.shared_unknown(&id)?)?
                    }
                    Operation::RetailUnknown { id } if product == CommercialProduct::Retail => {
                        let shared = Intent::stable_id(&g.binding, &id);
                        self.owned(&ledger, &shared, &g.binding.id)?;
                        serde_json::to_value(pay_ledger::Ledger::shared_retail_hold(
                            &ledger.shared_unknown(&shared)?,
                        )?)?
                    }
                    Operation::RetailHandoff { id } if product == CommercialProduct::Retail => {
                        let shared = Intent::stable_id(&g.binding, &id);
                        self.owned(&ledger, &shared, &g.binding.id)?;
                        let original = ledger.shared_outcome(&shared)?.ok_or(Error::Denied)?;
                        let out = if original.state == "held" {
                            if original.intent.binding != g.binding {
                                return Err(Error::Denied);
                            }
                            self.current(g, true)?;
                            ledger.shared_handoff(&shared)?
                        } else {
                            original
                        };
                        serde_json::to_value(out)?
                    }
                    Operation::RetailSettle {
                        id,
                        charge_msat,
                        at,
                    } if product == CommercialProduct::Retail => {
                        let shared = Intent::stable_id(&g.binding, &id);
                        self.owned(&ledger, &shared, &g.binding.id)?;
                        let units = charge_msat.try_into().map_err(|_| Error::Denied)?;
                        let out = ledger.shared_settle(
                            &shared,
                            units,
                            0,
                            &format!("native-retail:{id}"),
                            None,
                            at,
                        )?;
                        serde_json::to_value(pay_ledger::Ledger::shared_retail_hold(&out)?)?
                    }
                    _ => return Err(Error::Denied),
                }
            }
        };
        // Wallet IPC can outlast native revocation. Retain booked facts while
        // requiring current read authority before returning their evidence.
        self.current(g, false)?;
        Ok(json!({"origin":self.config.origin,"result":value}))
    }
    fn owned(&self, ledger: &pay_ledger::Ledger, id: &str, binding: &str) -> Result<()> {
        if ledger.shared_outcome(id)?.is_none_or(|o| {
            self.grant(binding).map_or(true, |g| {
                !o.intent.binding.mode().same_native(&g.binding.mode())
            })
        }) {
            return Err(Error::Denied);
        }
        Ok(())
    }
}
