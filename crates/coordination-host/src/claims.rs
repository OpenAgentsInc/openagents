//! Atomic all-or-none resource claims and a shared, nonrenewable call ledger.
use crate::{
    Result,
    book::{Coordinator, Lease, Request, Scope},
    operations::Limits,
};
use nostr::{contracts, coordination, domain::Event};
use serde_json::{Value, json};

fn active(lease: &Lease) -> bool {
    !matches!(lease.phase.as_str(), "released" | "cancelled")
}
fn charge(lease: &Lease) -> u64 {
    if lease.phase != "released" || lease.unknown {
        lease.reserved_calls
    } else {
        lease.spent_calls
    }
}
pub(crate) fn allocated(scope: &Scope, parent: Option<&str>) -> u64 {
    scope
        .claims
        .values()
        .filter(|l| l.parent.as_deref() == parent)
        .map(charge)
        .sum()
}
impl Coordinator {
    /// A controller can manage only descendants of a currently held parent.
    pub(crate) fn check_claim_controller(
        &self,
        scope: &Scope,
        lease: &Lease,
        actor: &str,
        now: u64,
    ) -> Result<()> {
        if actor == scope.owner || actor == lease.actor {
            return Ok(());
        }
        self.check(scope, actor, "control", now)?;
        let mut parent = lease.parent.as_deref();
        for _ in 0..64 {
            let Some(key) = parent else {
                break;
            };
            let ancestor = scope.claims.get(key).ok_or("parent claim unavailable")?;
            self.current_lease(scope, ancestor, now)?;
            if ancestor.actor == actor {
                return Ok(());
            }
            parent = ancestor.parent.as_deref();
        }
        Err("not_admitted: controller has no current ancestor claim".into())
    }
    pub fn claim(&self, event: &Event, now: u64) -> Result<Value> {
        let started = std::time::Instant::now();
        let (actor, body) = self.authenticate(event, now, Some("openagents.claim-request.v1"))?;
        coordination::validate(&body).map_err(|e| e.to_string())?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        self.current_invocation(&state, event)?;
        let id = state
            .scopes
            .iter()
            .find(|(_, s)| {
                s.proposals
                    .values()
                    .any(|p| p.reference == body["proposal"])
            })
            .map(|(id, _)| id.clone())
            .ok_or("not_admitted: proposal unavailable")?;
        let mut scope = state.scopes[&id].clone();
        let task = scope
            .proposals
            .iter()
            .find(|(_, p)| p.reference == body["proposal"])
            .map(|(id, _)| id.clone())
            .ok_or("proposal unavailable")?;
        let proposal = scope.proposals[&task].clone();
        let container = self.operation(&proposal.body["target"])?["binding_contract"]["operation"]
            == "coord-container";
        let worker = body["worker"].as_str().ok_or("worker key")?;
        self.check_executor(&proposal.body["target"], worker)?;
        let grant = self.check(&scope, &actor, "claim", now)?;
        let worker_grant = self.check(&scope, worker, "claim", now)?;
        if actor != worker && actor != scope.owner {
            self.check(&scope, &actor, "control", now)?;
            if proposal.body["parent"].is_null() {
                return Err("not_admitted: delegated controller requires its own parent".into());
            }
        }
        let fp = contracts::digest_value(&body).map_err(|e| e.to_string())?;
        let key = format!("{actor}:claim:{fp}");
        if let Some(previous) = scope.requests.get(&key) {
            return Ok(previous.result.clone());
        }
        if scope.requests.len() >= 4096 || scope.claims.len() >= 4096 {
            return Err("limit_exceeded: retained claim identities".into());
        }
        if body["expected_revision"] != scope.revision {
            return Err("stale: coordinator revision changed".into());
        }
        if proposal.claim.is_some() || proposal.reuse.is_some() || proposal.status != "proposed" {
            return Err("conflict: task already claimed or closed".into());
        }
        self.check_frame(&scope, &proposal.body)?;
        for dependency in proposal.body["dependencies"]
            .as_array()
            .ok_or("dependencies")?
        {
            if !matches!(
                scope.proposals[dependency.as_str().ok_or("dependency")?]
                    .status
                    .as_str(),
                "completed" | "integrated" | "reused"
            ) {
                return Err("conflict: dependency has no known successful result".into());
            }
        }
        let target = proposal.body["target"]["id"].as_str().ok_or("target")?;
        for rights in [grant, worker_grant] {
            if !rights.all && !rights.targets.contains(target) {
                return Err("not_admitted: target outside claim grant".into());
            }
            for entry in body["resources"].as_array().ok_or("resources")? {
                if !rights.all
                    && !rights
                        .resources
                        .contains(entry["id"].as_str().ok_or("resource")?)
                {
                    return Err("not_admitted: resource outside claim grant".into());
                }
            }
        }
        let limits = Limits::parse(&proposal.body["bounds"], &scope.policy)?;
        let until = body["requested_until"].as_u64().ok_or("claim expiry")?;
        if until <= now
            || until
                > now.saturating_add(if container {
                    scope
                        .policy
                        .max_container_seconds
                        .unwrap_or(scope.policy.max_claim_seconds)
                } else {
                    limits.wall_ms.div_ceil(1000)
                })
            || until > scope.policy.until
            || until > grant.until
            || until > worker_grant.until
        {
            return Err("not_admitted: claim lifetime exceeds current authority".into());
        }
        let domains = self.resources_current(&scope, &body["resources"], now)?;
        for lease in scope.claims.values().filter(|l| active(l) || l.unknown) {
            for (domain, mode) in &domains {
                if let Some(other) = lease.domains.get(domain) {
                    if mode == "write" || other == "write" {
                        return Err("conflict: source has an unreleased claim".into());
                    }
                }
            }
        }
        let effects =
            contracts::parse_effects(&proposal.body["effects"]).map_err(|e| e.to_string())?;
        if container && body["resources"] != json!([]) {
            return Err("container claims hold allowance rather than source locks".into());
        }
        for resource in effects
            .writes
            .iter()
            .chain(effects.reads.iter().filter(|r| r.as_str() != "artifact"))
        {
            if container {
                continue;
            }
            let write = effects.writes.contains(resource);
            if !body["resources"]
                .as_array()
                .ok_or("resources")?
                .iter()
                .any(|r| r["id"] == *resource && (!write || r["mode"] == "write"))
            {
                return Err("not_admitted: exact operation effects require resource claims".into());
            }
        }
        let parent = if proposal.body["parent"].is_null() {
            if body["reservation"] != scope.allowance {
                return Err("not_admitted: exact scope parent allowance required".into());
            }
            if allocated(&scope, None)
                .checked_add(limits.calls)
                .is_none_or(|v| v > scope.policy.calls)
            {
                return Err("limit_exceeded: aggregate call allowance is spent or held".into());
            }
            None
        } else {
            let parent_task = proposal.body["parent"].as_str().ok_or("parent task")?;
            let (parent_key, parent) = scope
                .claims
                .iter()
                .find(|(_, l)| {
                    l.body["proposal"] == scope.proposals[parent_task].reference
                        && l.body["reservation"] == body["reservation"]
                })
                .ok_or("not_admitted: exact admitted parent reservation required")?;
            self.current_lease(&scope, parent, now)?;
            if actor != scope.owner && actor != parent.actor {
                return Err(
                    "not_admitted: parent worker or owner must authorize child allowance".into(),
                );
            }
            if parent
                .own_calls
                .checked_add(allocated(&scope, Some(parent_key)))
                .and_then(|v| v.checked_add(limits.calls))
                .is_none_or(|v| v > parent.reserved_calls)
            {
                return Err("limit_exceeded: parent call allowance is spent or held".into());
            }
            if until > parent.body["expires_at"].as_u64().ok_or("parent expiry")? {
                return Err("not_admitted: child lease outlives parent".into());
            }
            Some(parent_key.clone())
        };
        let running = scope.claims.values().filter(|l| active(l)).count() as u64;
        if running >= scope.policy.concurrency || running >= limits.concurrency {
            return Err("limit_exceeded: coordinator concurrency".into());
        }
        let effective_now = now.saturating_add(started.elapsed().as_secs());
        if effective_now >= until || started.elapsed().as_millis() > u128::from(limits.wall_ms) {
            return Err("stale: claim admission exceeded its deadline".into());
        }
        self.check(&scope, &actor, "claim", effective_now)?;
        self.check(&scope, worker, "claim", effective_now)?;
        let parent_reservation =
            contracts::parse_reservation(&self.store.json(&body["reservation"])?)
                .map_err(|e| e.to_string())?;
        if parent_reservation.unknown {
            return Err("unknown: parent reservation requires reconciliation".into());
        }
        let reservation=self.store.put_json(&json!({"v":"openagents.reservation.v1","requires":[],"id":workspace_host::artifact::token(),"parent":parent_reservation.id,"currency":"USD","ceiling_microunits":0,"spent_microunits":0,"unknown":false,"meta":{"calls":limits.calls,"proposal":body["proposal"]}}))?;
        scope.token = scope.token.checked_add(1).ok_or("claim token exhausted")?;
        scope.revision += 1;
        let claim = json!({"v":"openagents.claim.v1","requires":[],"proposal":body["proposal"],"coordinator":scope.reference,"worker":worker,"resources":body["resources"],"token":scope.token,"epoch":scope.descriptor["epoch"],"expires_at":until,"reservation":reservation,"revision":scope.revision});
        coordination::validate(&claim).map_err(|e| e.to_string())?;
        let reference = self.declare(&mut state, &claim, worker, now)?;
        let reservation_key = reservation["digest"]
            .as_str()
            .ok_or("reservation digest")?
            .to_string();
        scope.claims.insert(
            reservation_key,
            Lease {
                reference: reference.clone(),
                body: claim,
                actor: worker.into(),
                domains,
                phase: "admitted".into(),
                dispatched: false,
                result: None,
                execution: None,
                input: None,
                parent,
                reserved_calls: limits.calls,
                spent_calls: 0,
                own_calls: 0,
                check: None,
                check_admission: None,
                integration: None,
                unknown: false,
            },
        );
        scope.proposals.get_mut(&task).ok_or("proposal")?.claim = Some(reference.clone());
        scope.proposals.get_mut(&task).ok_or("proposal")?.status = "claimed".into();
        let input = self.retain(&mut state, event, &body)?;
        let retain_until = scope.policy.until;
        let record=self.journal(&mut scope,"admitted",Some(&format!("claim:{}",scope_token(&reference))),json!({"enforcement":[],"reservations":[reservation],"deadlines":{"claim":until},"retention":retain_until,"input":input}),now)?;
        let result = self.result(
            &mut state, &scope, &input, &record, "accepted", reference, None, &actor, now,
        )?;
        scope.requests.insert(
            key,
            Request {
                fingerprint: fp,
                result: result.clone(),
            },
        );
        state.scopes.insert(id, scope);
        self.save(&state)?;
        Ok(result)
    }
    pub(crate) fn current_lease(&self, scope: &Scope, lease: &Lease, now: u64) -> Result<()> {
        let grant = self.check(scope, &lease.actor, "claim", now)?;
        if !active(lease)
            || lease.unknown
            || matches!(lease.phase.as_str(), "cancel_requested" | "unknown")
            || now >= lease.body["expires_at"].as_u64().ok_or("lease expiry")?
            || lease.body["epoch"] != scope.descriptor["epoch"]
        {
            return Err("not_admitted: claim expired, revoked, or unresolved".into());
        }
        let proposal = scope
            .proposals
            .values()
            .find(|p| p.reference == lease.body["proposal"])
            .ok_or("proposal")?;
        self.check_executor(&proposal.body["target"], &lease.actor)?;
        if proposal.claim.as_ref() != Some(&lease.reference) {
            return Err("stale: claim token was replaced".into());
        }
        if !grant.all
            && (!grant
                .targets
                .contains(proposal.body["target"]["id"].as_str().ok_or("target")?)
                || lease.body["resources"]
                    .as_array()
                    .ok_or("resources")?
                    .iter()
                    .any(|r| !grant.resources.contains(r["id"].as_str().unwrap_or(""))))
        {
            return Err("not_admitted: current grant narrowed".into());
        }
        self.check_frame(scope, &proposal.body)?;
        if let Some(parent) = &lease.parent {
            self.current_lease(scope, scope.claims.get(parent).ok_or("parent lease")?, now)?;
        }
        Ok(())
    }
}
fn scope_token(reference: &Value) -> String {
    reference["digest"].as_str().unwrap_or("").to_string()
}
