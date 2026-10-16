//! The operator mounts a live authority; neither a hash nor a relay supplies it.
use crate::Result;
use serde_json::Value;
pub trait ClaimAuthority {
    fn check(
        &self,
        claim: &Value,
        controller: &str,
        worker: &str,
        declaration: &Value,
        plaintext: &Value,
        reservation: &Value,
        frame: &Value,
        now: u64,
    ) -> Result<()>;
    fn with_current(
        &self,
        _claim: &Value,
        _controller: &str,
        _worker: &str,
        _declaration: &Value,
        _plaintext: &Value,
        _reservation: &Value,
        _frame: &Value,
        _now: u64,
        _effect: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        Err("cannot_enforce: fenced runtime admission is not configured".into())
    }
    fn with_intake(
        &self,
        _claim: &Value,
        _controller: &str,
        _worker: &str,
        _declaration: &Value,
        _plaintext: &Value,
        _reservation: &Value,
        _frame: &Value,
        _intake: &nostr::domain::Event,
        _now: u64,
        _effect: &mut dyn FnMut(&coordination_host::worker::Intake, u64) -> Result<Value>,
    ) -> Result<Value> {
        Err("cannot_enforce: fenced runtime intake is not configured".into())
    }
    fn bootstrap_receipt(&self, _: &nostr::domain::Event, _: &str, _: u64) -> Result<Value> {
        Err("unavailable: bootstrap accounting cannot be observed".into())
    }
    fn with_continuation(
        &self,
        _claim: &Value,
        _controller: &str,
        _worker: &str,
        _declaration: &Value,
        _plaintext: &Value,
        _reservation: &Value,
        _frame: &Value,
        _now: u64,
        _effect: &mut dyn FnMut(u64) -> Result<Value>,
    ) -> Result<Value> {
        Err("cannot_enforce: fenced runtime continuation is not configured".into())
    }
    fn bootstrap(
        &self,
        _: &nostr::domain::Event,
        _: &str,
        _: &Value,
        _: &Value,
        _: &str,
        _: &str,
        _: &Value,
        _: u64,
        _: u64,
        _: &mut dyn FnMut() -> Result<Value>,
    ) -> Result<(Value, Value)> {
        Err("unsupported_feature: bootstrap accounting is not configured".into())
    }
}
impl ClaimAuthority for coordination_host::book::Coordinator {
    fn with_continuation(
        &self,
        claim: &Value,
        controller: &str,
        worker: &str,
        declaration: &Value,
        plaintext: &Value,
        reservation: &Value,
        frame: &Value,
        now: u64,
        effect: &mut dyn FnMut(u64) -> Result<Value>,
    ) -> Result<Value> {
        self.with_environment_continuation(
            claim,
            controller,
            worker,
            declaration,
            plaintext,
            reservation,
            frame,
            now,
            effect,
        )
    }
    fn check(
        &self,
        claim: &Value,
        controller: &str,
        worker: &str,
        declaration: &Value,
        plaintext: &Value,
        reservation: &Value,
        frame: &Value,
        now: u64,
    ) -> Result<()> {
        self.environment_claim(
            claim,
            controller,
            worker,
            declaration,
            plaintext,
            reservation,
            frame,
            now,
        )
    }
    fn with_current(
        &self,
        claim: &Value,
        controller: &str,
        worker: &str,
        declaration: &Value,
        plaintext: &Value,
        reservation: &Value,
        frame: &Value,
        now: u64,
        effect: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        self.with_environment_claim(
            claim,
            controller,
            worker,
            declaration,
            plaintext,
            reservation,
            frame,
            now,
            effect,
        )
    }
    fn with_intake(
        &self,
        claim: &Value,
        controller: &str,
        worker: &str,
        declaration: &Value,
        plaintext: &Value,
        reservation: &Value,
        frame: &Value,
        intake: &nostr::domain::Event,
        now: u64,
        effect: &mut dyn FnMut(&coordination_host::worker::Intake, u64) -> Result<Value>,
    ) -> Result<Value> {
        self.with_environment_intake(
            claim,
            controller,
            worker,
            declaration,
            plaintext,
            reservation,
            frame,
            intake,
            now,
            effect,
        )
    }
    fn bootstrap_receipt(
        &self,
        event: &nostr::domain::Event,
        owner: &str,
        now: u64,
    ) -> Result<Value> {
        self.environment_bootstrap_receipt(event, owner, now)
    }
    fn bootstrap(
        &self,
        admission: &nostr::domain::Event,
        owner: &str,
        claim: &Value,
        reservation: &Value,
        request: &str,
        fingerprint: &str,
        frame: &Value,
        deadline: u64,
        now: u64,
        effect: &mut dyn FnMut() -> Result<Value>,
    ) -> Result<(Value, Value)> {
        self.environment_bootstrap(
            admission,
            owner,
            claim,
            reservation,
            request,
            fingerprint,
            frame,
            deadline,
            now,
            effect,
        )
    }
}

impl coordination_host::worker::DispatchGate for crate::allocator::Allocator {
    fn intake(
        &self,
        proof: &coordination_host::worker::VerifiedIntake,
        now: u64,
        dispatch: &mut dyn FnMut() -> Result<coordination_host::worker::Intake>,
    ) -> Result<coordination_host::worker::Intake> {
        self.with_worker_intake(proof, now, dispatch)
    }
}
