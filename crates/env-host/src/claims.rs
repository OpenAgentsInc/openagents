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
        now: u64,
    ) -> Result<()>;
}
impl ClaimAuthority for coordination_host::book::Coordinator {
    fn check(
        &self,
        claim: &Value,
        controller: &str,
        worker: &str,
        declaration: &Value,
        plaintext: &Value,
        reservation: &Value,
        now: u64,
    ) -> Result<()> {
        self.environment_claim(
            claim,
            controller,
            worker,
            declaration,
            plaintext,
            reservation,
            now,
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
