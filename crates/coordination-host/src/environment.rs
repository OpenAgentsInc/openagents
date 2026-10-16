//! ENV admission reads the actual current claim, never caller-supplied claim bytes.
use crate::{Result, book::Coordinator};
use nostr::{contracts, execution};
use serde_json::Value;
impl Coordinator {
    /// Local authority capability for an operator-mounted ENV allocator.
    pub fn environment_claim(
        &self,
        claim: &Value,
        controller: &str,
        worker: &str,
        declaration: &Value,
        plaintext: &Value,
        reservation: &Value,
        now: u64,
    ) -> Result<()> {
        let _guard = self.store.lock()?;
        let state = self.state()?;
        let (scope, lease) = state
            .scopes
            .values()
            .find_map(|s| {
                s.claims
                    .values()
                    .find(|l| l.reference == *claim)
                    .map(|l| (s, l))
            })
            .ok_or("not_admitted: actual environment claim unavailable")?;
        self.current_lease(scope, lease, now)?;
        self.check_claim_controller(scope, lease, controller, now)?;
        self.resources_current(scope, &lease.body["resources"], now)?;
        let event = lease
            .execution
            .as_ref()
            .ok_or("not_admitted: prepare the exact environment execution first")?;
        if lease.actor != worker
            || lease.dispatched
            || lease.input.as_ref() != Some(plaintext)
            || plaintext["requires"] != serde_json::json!([nostr::environment::BINDING_FEATURE])
            || declaration["caller"] != self.authority()
            || declaration["event"]["id"] != event.id
            || declaration["event"]["pubkey"] != self.authority()
            || declaration["event"]["kind"] != execution::REQUEST_KIND
        {
            return Err(
                "identity_mismatch: environment execution differs from the actual claim".into(),
            );
        }
        let parent = contracts::parse_reservation(&self.store.json(&lease.body["reservation"])?)
            .map_err(|e| e.to_string())?;
        let child = contracts::parse_reservation(reservation).map_err(|e| e.to_string())?;
        contracts::admit_child(&parent, &child).map_err(|e| e.to_string())?;
        if child.parent.as_deref() != Some(parent.id.as_str())
            || child.currency != parent.currency
            || child.ceiling_microunits != Some(0)
            || child.unknown
        {
            return Err("cannot_enforce: native environment reservation must narrow the actual no-spend claim".into());
        }
        Ok(())
    }
}
