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
        frame: &Value,
        now: u64,
    ) -> Result<()> {
        let _guard = self.store.lock()?;
        let state = self.state()?;
        self.environment_claim_locked(
            &state,
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
    fn environment_claim_locked(
        &self,
        state: &crate::book::State,
        claim: &Value,
        controller: &str,
        worker: &str,
        declaration: &Value,
        plaintext: &Value,
        reservation: &Value,
        frame: &Value,
        now: u64,
    ) -> Result<()> {
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
        if scope
            .proposals
            .values()
            .find(|p| p.reference == lease.body["proposal"])
            .is_none_or(|p| p.body["frame"] != *frame)
        {
            return Err(
                "identity_mismatch: environment frame differs from the claimed task".into(),
            );
        }
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

impl Coordinator {
    /// Keep the actual current claim fenced through runtime input disclosure.
    /// The runtime host still owns operation admission and budget accounting.
    pub fn with_environment_claim(
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
        let started = std::time::Instant::now();
        let _g = self.store.lock()?;
        let state = self.state()?;
        let now = now.saturating_add(started.elapsed().as_secs());
        self.environment_claim_locked(
            &state,
            claim,
            controller,
            worker,
            declaration,
            plaintext,
            reservation,
            frame,
            now,
        )?;
        effect()
    }
    /// Admit one registered external operation under the ENV fence. Persist
    /// the actual call charge and unknown hold before disclosing inputs, and
    /// keep the authority lock through the runtime's dispatch callback.
    /// A callback return is not a terminal worker result or release of holds.
    pub fn with_environment_intake(
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
        effect: &mut dyn FnMut(&crate::worker::Intake, u64) -> Result<Value>,
    ) -> Result<Value> {
        let started = std::time::Instant::now();
        let (actor, body) = self.authenticate(
            intake,
            now,
            Some("openagents.coordination-native-intake.v1"),
        )?;
        crate::admin::closed(&body, &["v", "requires", "request", "fingerprint"])?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let current = now.saturating_add(started.elapsed().as_secs());
        self.environment_claim_locked(
            &state,
            claim,
            controller,
            worker,
            declaration,
            plaintext,
            reservation,
            frame,
            current,
        )?;
        if actor != worker
            || self.external_operation(&plaintext["target"])?.worker != worker
            || body["request"] != declaration["event"]["id"]
            || body["fingerprint"]
                != contracts::digest_value(plaintext).map_err(|e| e.to_string())?
        {
            return Err("identity_mismatch: external runtime intake differs from its exact environment execution".into());
        }
        let admitted = self.intake_locked(&mut state, intake, &actor, &body, now, &started)?;
        let current = now.saturating_add(started.elapsed().as_secs());
        if admitted.0 != *plaintext || plaintext["deadline"].as_u64().is_none_or(|v| v <= current) {
            return Err("stale: execution expired before runtime disclosure".into());
        }
        effect(&admitted, current)
    }

    /// Charge one owner-admitted bootstrap against the actual container shared
    /// with this environment's named worker. This grants no bootstrap effects.
    pub fn environment_bootstrap(
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
        let origin_now = now;
        let started = std::time::Instant::now();
        let (actor, body) = self.auxiliary_request(admission, now)?;
        if actor != owner
            || body["purpose"] != "prepare"
            || body["request"] != request
            || body["fingerprint"] != fingerprint
        {
            return Err("not_admitted: original bootstrap allowance differs".into());
        }
        let _guard = self.store.lock()?;
        let now = now.saturating_add(started.elapsed().as_secs());
        let mut state = self.state()?;
        {
            let id = self.scope_key(&state, &body["coordinator"])?;
            let scope = &state.scopes[&id];
            let (key, worker) = scope
                .claims
                .iter()
                .find(|(_, l)| l.reference == *claim)
                .ok_or("not_admitted: actual bootstrap worker claim required")?;
            self.current_lease(scope, worker, now)?;
            if scope
                .proposals
                .values()
                .find(|p| p.reference == worker.body["proposal"])
                .is_none_or(|p| p.body["frame"] != *frame)
            {
                return Err(
                    "identity_mismatch: bootstrap frame differs from the claimed task".into(),
                );
            }
            if scope.owner != owner
                || worker.body["reservation"] != *reservation
                || worker.dispatched
                || deadline > worker.body["expires_at"].as_u64().ok_or("claim expiry")?
            {
                return Err("not_admitted: bootstrap exceeds its exact worker claim".into());
            }
            let mut cursor = Some(key.as_str());
            let mut admitted = false;
            for _ in 0..64 {
                let Some(k) = cursor else { break };
                let lease = scope.claims.get(k).ok_or("ancestor")?;
                if lease.reference == body["claim"] {
                    admitted = true;
                    break;
                }
                cursor = lease.parent.as_deref();
            }
            if !admitted {
                return Err(
                    "not_admitted: bootstrap and worker do not share the admitted container".into(),
                );
            }
        }
        let receipt = self.charge_auxiliary_locked(&mut state, admission, &actor, &body, now)?;
        if origin_now.saturating_add(started.elapsed().as_secs()) >= deadline {
            return Err("stale: bootstrap deadline passed before initialization".into());
        }
        let output = effect()?;
        Ok((receipt, output))
    }
}

impl Coordinator {
    /// Owner-only historical lookup. It neither revalidates nor repeats effects.
    pub fn environment_bootstrap_receipt(
        &self,
        event: &nostr::domain::Event,
        owner: &str,
        now: u64,
    ) -> Result<Value> {
        let (actor, body) = self.auxiliary_request(event, now)?;
        let _guard = self.store.lock()?;
        let state = self.state()?;
        let id = self.scope_key(&state, &body["coordinator"])?;
        let scope = &state.scopes[&id];
        if actor != owner || scope.owner != owner {
            return Err("not_admitted: original bootstrap owner required".into());
        }
        let key = format!("{actor}:aux:{}", body["request"].as_str().ok_or("request")?);
        let previous = scope
            .requests
            .get(&key)
            .ok_or("unknown: original bootstrap charge is not recorded")?;
        if previous.fingerprint != contracts::digest_value(&body).map_err(|e| e.to_string())? {
            return Err("idempotency_conflict: bootstrap charge identity differs".into());
        }
        Ok(previous.result.clone())
    }
}
