//! A read-only WORK port over this owner's independently admitted LAB book.
use crate::{Result, artifact_value, book::Book, store::Store};
use nostr::market_contracts;
use serde_json::{Value, json};
use work_host::market::Market;
/// Keep the LAB store lock for the whole native WORK admission.
pub struct Adapter(pub Store);
impl Adapter {
    pub fn new(mut store: Store) -> Result<Self> {
        if store.document.dispatch.is_some() {
            store.reconcile()?;
        }
        Ok(Self(store))
    }
}
impl Market for Adapter {
    fn disclosure(&self, actor: &str, now: u64) -> Result<()> {
        self.0.book.disclosure(actor, now)
    }
    fn evidence(&self, reference: &Value, frame: &Value, acceptance: &Value) -> Result<()> {
        self.0.book.evidence(reference, frame, acceptance)
    }
    fn execution_grant(&self, grant: &Value, order: &Value, until: u64) -> Result<()> {
        if self.0.book.order()
            != Some(&market_contracts::parse_order_ref(order).map_err(|e| e.to_string())?)
            || until > self.0.book.market().delivery_due_at
        {
            return Err(
                "commercial execution grant differs from this LAB order or its deadline".into(),
            );
        }
        if self.0.book.paid_halted() {
            self.reassign(order)?;
        }
        let dispatch = self
            .0
            .document
            .dispatch
            .as_ref()
            .ok_or("LAB execution has not been independently admitted by its task owner")?;
        let reference = nostr::contracts::parse_artifact(grant).map_err(|e| e.to_string())?;
        if reference.digest != dispatch.grant_digest {
            return Err(
                "WORK grant differs from LAB's independently admitted execution authority".into(),
            );
        }
        Ok(())
    }
    fn reassign(&self, order: &Value) -> Result<()> {
        if self.0.book.order()
            != Some(&market_contracts::parse_order_ref(order).map_err(|e| e.to_string())?)
        {
            return Err("reassignment order differs".into());
        }
        let dispatch = self
            .0
            .document
            .dispatch
            .as_ref()
            .ok_or("LAB dispatch status is unavailable")?;
        let task = dispatch
            .observation
            .as_ref()
            .ok_or("LAB execution outcome is unknown; its authority remains held")?;
        if !matches!(
            task.execution,
            coder::task::Execution::Finished
                | coder::task::Execution::Failed
                | coder::task::Execution::Stopped
        ) {
            return Err("LAB execution is active or unknown; reconcile it through its owning controls before reassignment".into());
        }
        Ok(())
    }
    fn frame(&self, frame: &Value, acceptance: &Value) -> Result<()> {
        self.0.book.frame(frame, acceptance)
    }
    fn delegation(
        &self,
        order: &Value,
        frame: &Value,
        acceptance: &Value,
        worker: &str,
        links: &[Value],
    ) -> Result<Value> {
        self.0
            .book
            .delegation(order, frame, acceptance, worker, links)
    }
    fn acceptance(
        &self,
        order: &Value,
        frame: &Value,
        acceptance: &Value,
        worker: &str,
        evidence: &Value,
    ) -> Result<Value> {
        self.0
            .book
            .acceptance(order, frame, acceptance, worker, evidence)
    }
}
impl Market for Book {
    fn disclosure(&self, actor: &str, now: u64) -> Result<()> {
        let rights = self.blobs.resolve(&self.labor().rights)?;
        if now > self.market().retain_until
            || !rights["recipients"]
                .as_array()
                .is_some_and(|keys| keys.iter().any(|key| *key == actor))
        {
            return Err(
                "not_admitted: LAB source bytes are outside this recipient's retained rights"
                    .into(),
            );
        }
        Ok(())
    }
    fn evidence(&self, reference: &Value, frame: &Value, acceptance: &Value) -> Result<()> {
        self.frame(frame, acceptance)?;
        self.records.resolve(reference)?;
        Ok(())
    }
    fn execution_grant(&self, _grant: &Value, _order: &Value, _until: u64) -> Result<()> {
        Err("a commercial agreement does not supply task execution authority; mount its admitted LAB store".into())
    }
    fn reassign(&self, _order: &Value) -> Result<()> {
        Err("reassignment requires the LAB execution store's current task outcome".into())
    }
    fn frame(&self, frame: &Value, acceptance: &Value) -> Result<()> {
        if *frame != artifact_value(&self.labor().task_frame)
            || *acceptance != artifact_value(&self.labor().acceptance_policy)
        {
            return Err(
                "WORK frame or acceptance differs from the independently admitted LAB terms".into(),
            );
        }
        Ok(())
    }
    fn delegation(
        &self,
        order: &Value,
        frame: &Value,
        acceptance: &Value,
        worker: &str,
        links: &[Value],
    ) -> Result<Value> {
        self.frame(frame, acceptance)?;
        if self.records.conflict
            || self.order()
                != Some(&market_contracts::parse_order_ref(order).map_err(|e| e.to_string())?)
            || self.market().worker != worker
        {
            return Err(
                "WORK commercial delegation differs from the current admitted LAB order or worker"
                    .into(),
            );
        }
        for link in links {
            let admitted = self
                .records
                .link
                .as_ref()
                .ok_or("LAB has no admitted execution linkage")?;
            let record = self.records.resolve(admitted)?;
            let execute: &Value = self.blobs.get(&record["execute_body"])?;
            if link["request"] != record["execute"]
                || execute["request"] != record["request"]
                || execute["run"] != record["run"]
                || execute["target"]
                    != json!({"id":self.labor().execution.target.id,"artifact":artifact_value(&self.labor().execution.target.artifact)})
                || execute["input"]["artifact"] != artifact_value(&self.labor().execution.input)
            {
                return Err("WORK execution does not match LAB's exact admitted CJ request and frozen input".into());
            }
            if !link["run"].is_null() {
                let body = self.blobs.get(&link["run"])?;
                let root = nostr::run::parse_envelope(body).map_err(|e| e.to_string())?;
                if root.seq != 0 || root.controller != worker || root.run != record["run"] {
                    return Err(
                        "WORK commercial RUN root differs from the LAB worker and attempt".into(),
                    );
                }
            }
            if !link["session"].is_null() || !link["proposal"].is_null() {
                return Err("commercial SESS/COORD linkage requires its owning adapter".into());
            }
        }
        Ok(
            json!({"order":order,"task_frame":frame,"acceptance_policy":acceptance,"execution":self.records.link,"submission":self.records.submission,"verification":self.records.verification,"acceptance":self.records.acceptance,"commercial_notices":self.paid_notices}),
        )
    }
    fn acceptance(
        &self,
        order: &Value,
        frame: &Value,
        acceptance: &Value,
        worker: &str,
        evidence: &Value,
    ) -> Result<Value> {
        let records = self.delegation(order, frame, acceptance, worker, &[])?;
        if !self.records.disputes.is_empty()
            || self.records.acceptance.as_ref() != Some(evidence)
            || self.records.resolve(evidence)?["outcome"] != "accepted"
        {
            return Err(
                "WORK commercial evidence is not the exact independently verified LAB acceptance"
                    .into(),
            );
        }
        Ok(records)
    }
}
