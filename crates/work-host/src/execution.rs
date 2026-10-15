//! Independently admitted, attenuated no-spend execution grants and exact links.
use crate::{
    Result,
    authority::{Authority, Scope, State, closed},
};
use nostr::{
    contracts,
    domain::Event,
    work::{self, WorkRef},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutionGrant {
    pub reference: Value,
    pub issuer: String,
    pub delegate: String,
    pub work: WorkRef,
    pub frame: Value,
    pub parent: Option<String>,
    pub recipients: BTreeSet<String>,
    pub reads: BTreeSet<String>,
    pub effects: BTreeSet<String>,
    pub calls: u64,
    pub wall_ms: u64,
    pub output_bytes: u64,
    pub until: u64,
    pub used: u64,
    pub revoked: bool,
    pub superseded: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Job {
    pub id: String,
    pub delegate: String,
    pub work: WorkRef,
    pub basis: Value,
    pub frame: Value,
    pub acceptance: Value,
    pub grant: Value,
    pub input: Value,
    pub fingerprint: String,
    pub status: String,
    pub confirmed: bool,
    pub result: Option<Value>,
    pub result_event: Option<Event>,
    pub request: Option<Event>,
    pub run: Option<Value>,
    pub execute: Value,
}
impl Authority {
    /// Separate owner control admits the actual execution envelope. Work
    /// assignment never calls this operation or manufactures a grant.
    fn current_execution(&self, scope: &Scope, grant: &ExecutionGrant, now: u64) -> Result<()> {
        let mut current = grant;
        let mut visited = BTreeSet::new();
        let mut first = true;
        loop {
            let digest = current.reference["digest"]
                .as_str()
                .ok_or("grant identity")?;
            if visited.len() >= 64
                || !visited.insert(digest.to_string())
                || current.revoked
                || first && current.superseded
                || now >= current.until
            {
                return Err("not_admitted: execution grant or ancestor is unavailable".into());
            }
            self.check(
                scope,
                &current.issuer,
                Some(&current.work.item),
                "assign",
                now,
            )?;
            let Some(parent) = &current.parent else {
                return Ok(());
            };
            first = false;
            current = scope
                .executions
                .get(parent)
                .ok_or("execution ancestor unavailable")?;
        }
    }
    pub fn execution_grant(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.work-execution-grant.v1"))?;
        closed(
            &body,
            &[
                "v",
                "requires",
                "issuer",
                "scope",
                "work",
                "delegate",
                "frame",
                "parent",
                "recipients",
                "reads",
                "effects",
                "calls",
                "wall_ms",
                "output_bytes",
                "until",
            ],
        )?;
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let w = work::work_ref(&body["work"]).map_err(|e| format!("{e:?}"))?;
        let scope = state
            .scopes
            .get_mut(&w.scope)
            .ok_or("work scope unavailable")?;
        if scope.reference != body["scope"]
            || scope.descriptor["authority"] != w.authority
            || scope.revoked
        {
            return Err("execution grant scope is unavailable".into());
        }
        let roles = self.check(scope, &actor, Some(&w.item), "assign", now)?;
        let _ = roles;
        let (_, _, item) = self.head(scope, &w.item)?.ok_or("work is unavailable")?;
        if item["kind"] != "work" || item["frame"] != body["frame"] {
            return Err("execution grant frame differs".into());
        }
        let delegate = body["delegate"].as_str().ok_or("delegate")?.to_string();
        delegate
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "invalid delegate")?;
        let recipients: BTreeSet<String> =
            serde_json::from_value(body["recipients"].clone()).map_err(|_| "grant recipients")?;
        let reads: BTreeSet<String> =
            serde_json::from_value(body["reads"].clone()).map_err(|_| "grant reads")?;
        let effects: BTreeSet<String> =
            serde_json::from_value(body["effects"].clone()).map_err(|_| "grant effects")?;
        let candidate_ref = workspace_host::artifact::json_ref(&body)?;
        if let Some(known) = scope
            .executions
            .get(candidate_ref["digest"].as_str().ok_or("grant digest")?)
        {
            return Ok(known.reference.clone());
        }
        if scope.executions.len() >= 256 {
            return Err("execution grant retention limit exceeded".into());
        }
        let calls = number(&body, "calls", 1, 64)?;
        let wall_ms = number(&body, "wall_ms", 1, 60_000)?;
        let output_bytes = number(&body, "output_bytes", 1, 1_048_576)?;
        let until = number(&body, "until", now + 1, 9_007_199_254_740_991)?;
        if recipients.is_empty()
            || recipients.len() > 64
            || reads.len() > 64
            || !recipients.contains(&delegate)
            || !effects.is_empty()
        {
            return Err(
                "unsupported execution authority; this profile admits bounded pure jobs only"
                    .into(),
            );
        }
        for key in &recipients {
            key.parse::<secp256k1::XOnlyPublicKey>()
                .map_err(|_| "invalid recipient")?;
        }
        let parent = if body["parent"].is_null() {
            None
        } else {
            Some(
                body["parent"]["digest"]
                    .as_str()
                    .ok_or("parent grant")?
                    .to_string(),
            )
        };
        if let Some(parent) = &parent {
            let prior = scope
                .executions
                .get(parent)
                .ok_or("parent execution grant is unavailable")?;
            self.current_execution(scope, prior, now)?;
            if prior.revoked
                || prior.until <= now
                || prior.delegate != actor
                || prior.work != w
                || prior.frame != body["frame"]
                || until > prior.until
                || !recipients.is_subset(&prior.recipients)
                || !reads.is_subset(&prior.reads)
                || !effects.is_subset(&prior.effects)
                || calls > prior.calls
                || wall_ms > prior.wall_ms
                || output_bytes > prior.output_bytes
            {
                return Err("execution delegation widens issuer authority".into());
            }
            let held = scope
                .executions
                .values()
                .filter(|g| g.parent.as_ref() == Some(parent))
                .map(|g| g.calls)
                .sum::<u64>();
            if held.saturating_add(prior.used).saturating_add(calls) > prior.calls {
                return Err("parent execution budget is already held".into());
            }
        } else {
            if scope.descriptor["owner"] != actor {
                return Err("only the scope owner can admit a root execution grant".into());
            }
            let held = scope
                .executions
                .values()
                .filter(|g| g.parent.is_none())
                .map(|g| g.calls)
                .sum::<u64>();
            if held.saturating_add(calls)
                > scope.policy["execution"]["calls"]
                    .as_u64()
                    .ok_or("scope execution budget")?
            {
                return Err("scope execution budget is already held".into());
            }
        }
        let reference = self.store.put_json(&body)?;
        let key = reference["digest"]
            .as_str()
            .ok_or("grant digest")?
            .to_string();
        if let Some(known) = scope.executions.get(&key) {
            return Ok(known.reference.clone());
        }
        scope.executions.insert(
            key,
            ExecutionGrant {
                reference: reference.clone(),
                issuer: actor,
                delegate,
                work: w,
                frame: body["frame"].clone(),
                parent,
                recipients,
                reads,
                effects,
                calls,
                wall_ms,
                output_bytes,
                until,
                used: 0,
                revoked: false,
                superseded: false,
            },
        );
        self.retain_declaration(&mut state, event, &body)?;
        self.save(&state)?;
        Ok(reference)
    }
    pub(crate) fn check_delegation(
        &self,
        state: &State,
        scope: &Scope,
        w: &WorkRef,
        prior: Option<&Value>,
        item: &Value,
        expected: Option<&Value>,
        now: u64,
    ) -> Result<()> {
        let changed = prior
            .is_none_or(|p| p["delegation"] != item["delegation"] || p["frame"] != item["frame"]);
        if item["delegation"].is_null() {
            return Ok(());
        }
        if !changed {
            return Ok(());
        }
        let (issuer, delegation) = self.original(state, &item["delegation"])?;
        work::validate(&delegation).map_err(|e| format!("{e:?}"))?;
        self.check(scope, &issuer, Some(&w.item), "assign", now)?;
        if delegation["v"] != "openagents.work-delegation.v1"
            || work::work_ref(&delegation["work"]).map_err(|e| format!("{e:?}"))? != *w
            || Some(&delegation["basis"]) != expected
            || delegation["frame"] != item["frame"]
            || now >= delegation["until"].as_u64().ok_or("delegation expiry")?
        {
            return Err("delegation basis, frame, or expiry differs".into());
        }
        if !delegation["order"].is_null() {
            self.market_link(state, scope, w, item, &delegation)?;
            self.market
                .as_ref()
                .ok_or("admitted LAB book required")?
                .execution_grant(
                    &delegation["grant"],
                    &delegation["order"],
                    delegation["until"].as_u64().ok_or("delegation expiry")?,
                )?;
            return Ok(());
        }
        let grant = scope
            .executions
            .get(
                delegation["grant"]["digest"]
                    .as_str()
                    .ok_or("execution grant")?,
            )
            .ok_or("independent execution grant is unavailable")?;
        self.current_execution(scope, grant, now)?;
        if grant.reference != delegation["grant"]
            || grant.revoked
            || grant.until <= now
            || grant.delegate != delegation["delegate"]
            || grant.work != *w
            || grant.frame != item["frame"]
            || delegation["until"]
                .as_u64()
                .is_none_or(|until| until > grant.until)
        {
            return Err(
                "delegation differs from current independently admitted execution authority".into(),
            );
        }
        if !delegation["order"].is_null() {
            self.market_link(state, scope, w, item, &delegation)?;
        }
        for link in delegation["executions"]
            .as_array()
            .ok_or("execution links")?
        {
            let matching = scope.jobs.values().find(|j| {
                j.work == *w
                    && j.frame == item["frame"]
                    && j.delegate == grant.delegate
                    && j.grant == delegation["grant"]
                    && if link["request"].is_null() {
                        !link["run"].is_null() && j.run.as_ref() == Some(&link["run"])
                    } else {
                        j.request.as_ref().is_some_and(|event| {
                            link["request"]
                                == json!({"id":event.id,"pubkey":event.pubkey,"kind":event.kind})
                        })
                    }
            });
            let job = matching
                .ok_or("execution reference has not been admitted under this work grant")?;
            if !link["run"].is_null() && job.run.as_ref() != Some(&link["run"]) {
                return Err("execution RUN root differs".into());
            }
            if !link["session"].is_null() || !link["proposal"].is_null() {
                return Err("unsupported session/COORD link without its owning adapter".into());
            }
        }
        Ok(())
    }
    pub(crate) fn reassign(
        &self,
        scope: &mut Scope,
        prior: Option<&Value>,
        item: &Value,
    ) -> Result<()> {
        let Some(prior) = prior else {
            return Ok(());
        };
        if !matches!(item["state"].as_str(), Some("cancelled" | "superseded"))
            && prior["frame"] == item["frame"]
        {
            if prior["delegation"] == item["delegation"] {
                return Ok(());
            }
            if !prior["delegation"].is_null() && !item["delegation"].is_null() {
                let old = self.store.json(&prior["delegation"])?;
                let next = self.store.json(&item["delegation"])?;
                if old["grant"] == next["grant"]
                    && old["delegate"] == next["delegate"]
                    && old["frame"] == next["frame"]
                {
                    return Ok(());
                }
            }
        }
        if !prior["delegation"].is_null() {
            let delegation = self.store.json(&prior["delegation"])?;
            if !delegation["order"].is_null() {
                self.market
                    .as_ref()
                    .ok_or("reassignment requires the owning LAB book")?
                    .reassign(&delegation["order"])?;
                return Ok(());
            }

            let old_digest = delegation["grant"]["digest"]
                .as_str()
                .ok_or("old grant identity")?;
            let mut retained_ancestor = false;
            if prior["frame"] == item["frame"]
                && !item["delegation"].is_null()
                && !matches!(item["state"].as_str(), Some("cancelled" | "superseded"))
            {
                let next = self.store.json(&item["delegation"])?;
                let mut cursor = scope.executions.get(
                    next["grant"]["digest"]
                        .as_str()
                        .ok_or("new grant identity")?,
                );
                for _ in 0..64 {
                    let Some(grant) = cursor else {
                        break;
                    };
                    let Some(parent) = &grant.parent else {
                        break;
                    };
                    if parent == old_digest {
                        retained_ancestor = true;
                        break;
                    }
                    cursor = scope.executions.get(parent);
                }
            }
            if let Some(grant) = scope.executions.get_mut(
                delegation["grant"]["digest"]
                    .as_str()
                    .ok_or("prior execution grant")?,
            ) {
                if retained_ancestor {
                    grant.superseded = true;
                } else {
                    grant.revoked = true;
                }
            }
        }
        // This pure-job profile never releases previous call reservations or
        // rewrites active/unknown attempts when a tracking pointer changes.
        Ok(())
    }
    pub(crate) fn market_link(
        &self,
        _state: &State,
        _scope: &Scope,
        _w: &WorkRef,
        item: &Value,
        delegation: &Value,
    ) -> Result<()> {
        self.market
            .as_ref()
            .ok_or("commercial linkage requires the admitted LAB book")?
            .delegation(
                &delegation["order"],
                &item["frame"],
                &item["acceptance"],
                delegation["delegate"].as_str().ok_or("worker")?,
                delegation["executions"]
                    .as_array()
                    .ok_or("execution links")?,
            )?;
        Ok(())
    }
}
fn number(body: &Value, key: &str, min: u64, max: u64) -> Result<u64> {
    body[key]
        .as_u64()
        .filter(|n| *n >= min && *n <= max)
        .ok_or_else(|| format!("execution bound is invalid: {key}"))
}

impl Authority {
    pub fn prepare_job(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.work-job-proposal.v1"))?;
        closed(
            &body,
            &[
                "v",
                "requires",
                "issuer",
                "job",
                "work",
                "basis",
                "delegation",
            ],
        )?;
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let w = work::work_ref(&body["work"]).map_err(|e| format!("{e:?}"))?;
        let scope = state
            .scopes
            .get(&w.scope)
            .filter(|s| s.descriptor["authority"] == w.authority)
            .ok_or("work scope unavailable")?;
        self.check(scope, &actor, Some(&w.item), "read", now)?;
        let id = body["job"]
            .as_str()
            .filter(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
            })
            .ok_or("job ID")?
            .to_string();
        let fingerprint =
            contracts::digest_bytes(&contracts::jcs(&body).map_err(|e| format!("{e:?}"))?);
        if let Some(job) = scope.jobs.get(&id) {
            if job.fingerprint != fingerprint {
                return Err("conflict: execution request ID has different bytes".into());
            }
            return Ok(json!({"job":id,"request":job.request,"input":job.input}));
        }
        if state
            .scopes
            .iter()
            .any(|(scope_id, scope)| scope_id != &w.scope && scope.jobs.contains_key(&id))
        {
            return Err("conflict: execution request identity is already retained".into());
        }
        let scope = state
            .scopes
            .get_mut(&w.scope)
            .ok_or("work scope unavailable")?;
        let (basis, _, item) = self.head(scope, &w.item)?.ok_or("work unavailable")?;
        if basis != body["basis"]
            || item["delegation"] != body["delegation"]
            || matches!(
                item["state"].as_str(),
                Some("resolved" | "cancelled" | "superseded" | "archived")
            )
        {
            return Err("stale: work revision or delegation changed".into());
        }
        let delegation = self.store.json(&body["delegation"])?;
        if !delegation["order"].is_null() {
            return Err(
                "commercial execution must use the separately admitted LAB executor".into(),
            );
        }
        let grant_key = delegation["grant"]["digest"]
            .as_str()
            .ok_or("execution grant")?
            .to_string();
        let grant = scope
            .executions
            .get(&grant_key)
            .ok_or("execution grant unavailable")?
            .clone();
        self.current_execution(scope, &grant, now)?;
        if grant.revoked
            || now >= grant.until
            || now >= delegation["until"].as_u64().ok_or("delegation expiry")?
            || grant.frame != item["frame"]
            || grant.work != w
            || actor != grant.issuer
        {
            return Err("not_admitted: current independent execution authority required".into());
        }
        if scope.jobs.len() >= 4096 {
            return Err("execution identity retention limit exceeded".into());
        }
        let held = scope
            .executions
            .values()
            .filter(|g| g.parent.as_ref() == Some(&grant_key))
            .map(|g| g.calls)
            .sum::<u64>();
        if grant.used.saturating_add(held) >= grant.calls {
            return Err("execution call budget is held or consumed".into());
        }
        let frame = self.store.json(&item["frame"])?;
        let objective_ref =
            contracts::parse_artifact(&frame["objective"]).map_err(|e| format!("{e:?}"))?;
        if !grant.reads.contains(&objective_ref.digest) {
            return Err("execution grant does not admit the objective bytes".into());
        }
        let objective = String::from_utf8(self.store.get(&frame["objective"])?)
            .map_err(|_| "pure work objective must be UTF-8")?;
        let input=self.store.put_json(&json!({"v":"openagents.work-frozen-input.v1","requires":[],"work_revision":basis,"frame":item["frame"],"acceptance":item["acceptance"],"delegation":body["delegation"],"grant":grant.reference,"task":frame["task"],"objective":objective,"job":id}))?;
        let target_artifact=self.store.put_json(&json!({"v":"openagents.work-pure-operation.v1","requires":[],"operation":"echo-frozen-objective","effects":[],"implementation":"rust-native"}))?;
        let target = json!({"id":format!("{}:work/echo-frozen-objective",self.signer.pubkey()),"artifact":target_artifact});
        let lock = json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":target["id"],"definition":target,"dependencies":[]}]});
        contracts::parse_lock(&lock).map_err(|e| format!("{e:?}"))?;
        let lock = self.store.put_json(&lock)?;
        let context = json!({"v":"openagents.context.v1","requires":[],"task":frame["task"],"recipient":grant.delegate,"policy":scope.policy_ref,"entries":[],"omissions":[],"coverage":"complete"});
        contracts::parse_context(&context).map_err(|e| format!("{e:?}"))?;
        let context = self.store.put_json(&context)?;
        let requirements=self.store.put_json(&json!({"v":"openagents.work-pure-requirements.v1","requires":[],"effects":[],"spend_microunits":0,"input":input,"task_frame":item["frame"]}))?;
        let deadline = now
            .saturating_add(grant.wall_ms.div_ceil(1000))
            .min(grant.until)
            .min(delegation["until"].as_u64().ok_or("delegation expiry")?);
        if deadline <= now {
            return Err("execution grant expired".into());
        }
        let payload = json!({"v":nostr::execution::SCHEMA,"requires":[],"type":"execute","request":id,"attempt":1,"run":id,"target":target,"lock":lock,"input":{"artifact":input},"context":context,"requirements":requirements,"bounds":{"wall_ms":grant.wall_ms,"output_bytes":grant.output_bytes,"jobs":1,"spend_microunits":0},"deadline":deadline,"retain_until":deadline+3600});
        let peer = grant
            .delegate
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "worker key")?;
        let conversation = nostr::nip44::conversation_key(&self.secret, &peer);
        let event = nostr::execution::Seal {
            signer: &self.signer,
            conversation,
            nonce: secp256k1::rand::random(),
            created_at: now,
        }
        .event(
            nostr::execution::REQUEST_KIND,
            vec![
                nostr::domain::Tag::new(vec!["p".into(), grant.delegate.clone()]),
                nostr::domain::Tag::new(vec!["expiration".into(), deadline.to_string()]),
            ],
            &payload,
        )
        .map_err(|e| format!("{e:?}"))?;
        scope
            .executions
            .get_mut(&grant_key)
            .ok_or("execution grant")?
            .used += 1;
        scope.jobs.insert(
            id.clone(),
            Job {
                id: id.clone(),
                delegate: grant.delegate,
                work: w,
                basis,
                frame: item["frame"].clone(),
                acceptance: item["acceptance"].clone(),
                grant: grant.reference,
                input: input.clone(),
                fingerprint,
                status: "prepared".into(),
                confirmed: false,
                result: None,
                result_event: None,
                request: Some(event.clone()),
                run: None,
                execute: payload,
            },
        );
        self.save(&state)?;
        Ok(json!({"job":id,"request":event,"input":input}))
    }
    pub(crate) fn worker_intake(
        &self,
        event: &Event,
        now: u64,
    ) -> Result<(String, Value, BTreeMap<String, Vec<u8>>)> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.work-worker-intake.v1"))?;
        closed(
            &body,
            &["v", "requires", "issuer", "job", "request", "fingerprint"],
        )?;
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let scope = state
            .scopes
            .values_mut()
            .find(|s| s.jobs.contains_key(body["job"].as_str().unwrap_or("")))
            .ok_or("not_admitted: job unavailable")?;
        let job = scope
            .jobs
            .get(body["job"].as_str().ok_or("job ID")?)
            .ok_or("job")?
            .clone();
        let grant = scope
            .executions
            .get(job.grant["digest"].as_str().ok_or("grant")?)
            .ok_or("execution grant unavailable")?;
        self.current_execution(scope, grant, now)?;
        if scope.revoked
            || grant.revoked
            || now >= grant.until
            || job.delegate != actor
            || job.request.as_ref().is_none_or(|e| body["request"] != e.id)
            || body["fingerprint"]
                != contracts::digest_bytes(
                    &contracts::jcs(&job.execute).map_err(|e| format!("{e:?}"))?,
                )
            || now >= job.execute["deadline"].as_u64().ok_or("job deadline")?
        {
            return Err("not_admitted: job grant or request is unavailable".into());
        }
        let (_basis, _, item) = self
            .head(scope, &job.work.item)?
            .ok_or("work unavailable")?;
        if !scope.items[&job.work.item].history.contains(&job.basis)
            || item["frame"] != job.frame
            || item["acceptance"] != job.acceptance
            || item["delegation"].is_null()
            || matches!(
                item["state"].as_str(),
                Some("resolved" | "cancelled" | "superseded" | "archived")
            )
        {
            return Err("stale: worker intake no longer matches the exact work revision".into());
        }
        let current = self.store.json(&item["delegation"])?;
        if current["grant"] != job.grant || current["delegate"] != job.delegate {
            return Err("stale: worker intake no longer has the current delegation".into());
        }
        let mut pins = BTreeMap::new();
        for r in [
            &job.execute["target"]["artifact"],
            &job.execute["lock"],
            &job.input,
            &job.execute["context"],
            &job.execute["requirements"],
        ] {
            let bytes = self.store.get(r)?;
            pins.insert(contracts::digest_bytes(&bytes), bytes);
        }
        scope.jobs.get_mut(&job.id).ok_or("job")?.status = "admitted".into();
        self.save(&state)?;
        Ok((job.id, job.execute, pins))
    }
    pub fn record_worker_result(
        &self,
        event: &Event,
        artifacts: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Value> {
        if artifacts.len() > 64 || artifacts.values().map(Vec::len).sum::<usize>() > 4 * 1_048_576 {
            return Err("worker result closure exceeds its bound".into());
        }
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let scope = state
            .scopes
            .values_mut()
            .find(|s| {
                s.jobs.values().any(|j| {
                    j.request
                        .as_ref()
                        .is_some_and(|r| event.tag_values("e").any(|e| e == r.id))
                })
            })
            .ok_or("worker request is unavailable")?;
        let key = scope
            .jobs
            .iter()
            .find(|(_, j)| {
                j.request
                    .as_ref()
                    .is_some_and(|r| event.tag_values("e").any(|e| e == r.id))
            })
            .map(|(key, _)| key.clone())
            .ok_or("worker request")?;
        let job = scope.jobs[&key].clone();
        let request = job.request.as_ref().ok_or("worker request")?;
        let payload = nostr::execution::bind_worker_event(
            event,
            &nostr::execution::Pending {
                execute_event: &request.id,
                worker: &job.delegate,
                customer: self.signer.pubkey(),
                request: &job.id,
                attempt: 1,
            },
            &self.secret,
        )
        .map_err(|e| format!("{e:?}"))?;
        if event.kind != nostr::execution::RESULT_KIND {
            return Err("terminal worker result required".into());
        }
        if let Some(previous) = &job.result {
            if *previous != payload {
                return Err("conflict: worker returned different terminal results".into());
            }
            return Ok(previous.clone());
        }
        for (digest, bytes) in artifacts {
            if contracts::digest_bytes(bytes) != *digest || bytes.len() > 1_048_576 {
                return Err("worker artifact digest or bound differs".into());
            }
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        if payload["run"] != job.id || payload["spend"] != 0 {
            return Err("worker result run or spend differs".into());
        }
        let output = payload["output"].clone();
        if payload["outcome"] == "completed" {
            let value = self.store.json(&output)?;
            closed(
                &value,
                &["v", "requires", "work_revision", "frame", "job", "value"],
            )?;
            if value["v"] != "openagents.work-pure-output.v1"
                || value["work_revision"] != job.basis
                || value["frame"] != job.frame
                || value["job"] != job.id
            {
                return Err("worker output differs from frozen work input".into());
            }
        }
        let roots = payload["artifacts"].as_array().ok_or("worker artifacts")?;
        let mut root = None;
        for reference in roots {
            if reference["schema"] == nostr::run::RECORD_SCHEMA {
                let body = self.store.json(reference)?;
                let record = nostr::run::parse_envelope(&body).map_err(|e| format!("{e:?}"))?;
                if record.seq != 0
                    || record.controller != job.delegate
                    || record.run != job.id
                    || body["record"]["data"]["request"] != job.id
                    || body["record"]["data"]["program"]
                        != job.execute["target"]["artifact"]["digest"]
                    || body["record"]["data"]["lock"] != job.execute["lock"]["digest"]
                    || body["record"]["data"]["context"] != job.execute["context"]["digest"]
                    || body["record"]["data"]["policy"] != job.execute["requirements"]["digest"]
                    || payload["record"]["digest"] != reference["digest"]
                    || root.is_some()
                {
                    return Err("worker RUN root identity differs".into());
                }
                root = Some(reference.clone());
            }
        }
        if root.is_none() {
            return Err("authenticated worker RUN root is unavailable".into());
        }
        let current = scope.jobs.get_mut(&key).ok_or("job")?;
        current.status = payload["outcome"].as_str().ok_or("worker outcome")?.into();
        current.confirmed = payload["outcome"] == "completed";
        current.result = Some(payload.clone());
        current.result_event = Some(event.clone());
        current.run = root;
        self.store.put_json(&payload)?;
        self.save(&state)?;
        let _ = now;
        Ok(payload)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerState {
    v: String,
    service: nostr::execution::Service,
    roots: BTreeMap<String, Value>,
    events: BTreeMap<String, Event>,
    outputs: BTreeMap<String, Value>,
}
/// A concrete pure Rust worker over the existing encrypted CJ and RUN owners.
/// Its sole registered operation echoes the exact frozen objective as data.
pub struct PureWorker {
    store: workspace_host::artifact::Artifacts,
    secret: secp256k1::SecretKey,
    signer: nostr::domain::RelaySigner,
}
impl PureWorker {
    pub fn open(root: std::path::PathBuf, secret: secp256k1::SecretKey, now: u64) -> Result<Self> {
        let store = workspace_host::artifact::Artifacts::new(root)?;
        let signer =
            nostr::domain::RelaySigner::from_secret_hex(&secret.display_secret().to_string())
                .map_err(|_| "worker key")?;
        let worker = Self {
            store,
            secret,
            signer,
        };
        let _lock = worker.store.lock()?;
        if !worker.store.root().join("state.json").exists() {
            worker.store.save(
                &WorkerState {
                    v: "openagents.work-pure-worker.v1".into(),
                    service: nostr::execution::Service::new(
                        worker.signer.pubkey(),
                        1,
                        now + 7 * 86_400,
                    ),
                    roots: BTreeMap::new(),
                    events: BTreeMap::new(),
                    outputs: BTreeMap::new(),
                },
                &worker.signer,
            )?;
        }
        drop(_lock);
        Ok(worker)
    }
    pub fn run(
        &self,
        authority: &Authority,
        event: &Event,
        now: u64,
    ) -> Result<(Event, BTreeMap<String, Vec<u8>>)> {
        let opened = nostr::execution::open_request(
            event,
            self.signer.pubkey(),
            &self.secret,
            event.created_at,
            nostr::execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let nostr::execution::Body::Execute(execute) = &opened.body else {
            return Err("pure work worker accepts explicit executes only".into());
        };
        let _lock = self.store.lock()?;
        let mut state: WorkerState = self.store.load(&self.signer)?;
        if state.v != "openagents.work-pure-worker.v1"
            || state.service.worker != self.signer.pubkey()
        {
            return Err("pure worker identity differs".into());
        }
        let key = state
            .service
            .key(&opened.principal, &execute.request, execute.attempt);
        if let Some(result) = state.events.get(&key) {
            let claim = state
                .service
                .claims
                .get(&key)
                .ok_or("retained worker claim")?;
            if claim.fingerprint != execute.fingerprint
                || claim.execute_event != event.id
                || now > claim.retain_until
            {
                return Err("conflict: changed execution request under a retained identity".into());
            }

            let mut closure = BTreeMap::new();
            for reference in state
                .outputs
                .get(&key)
                .and_then(Value::as_array)
                .ok_or("retained output references")?
            {
                closure.insert(
                    reference["digest"].as_str().ok_or("output digest")?.into(),
                    self.store.get(reference)?,
                );
            }
            return Ok((result.clone(), closure));
        }
        nostr::execution::open_request(
            event,
            self.signer.pubkey(),
            &self.secret,
            now,
            nostr::execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        if state.service.claims.contains_key(&key) {
            return Err(
                "unknown: an earlier worker attempt requires reconciliation; it was not replayed"
                    .into(),
            );
        }
        let intake = json!({"v":"openagents.work-worker-intake.v1","requires":[],"issuer":self.signer.pubkey(),"job":execute.request,"request":event.id,"fingerprint":execute.fingerprint});
        let intake =
            workspace_host::host::request(&self.secret, authority.signer.pubkey(), intake, now)?;
        let (_, payload, pins) = authority.worker_intake(&intake, now)?;
        if payload != execute.payload || opened.principal != authority.signer.pubkey() {
            return Err("worker request differs from authority's exact admission".into());
        }
        nostr::execution::require_pins(&execute, &pins).map_err(|e| format!("{e:?}"))?;
        let bytes = pins
            .get(
                &execute
                    .input_artifact
                    .as_ref()
                    .ok_or("frozen input artifact")?
                    .digest,
            )
            .ok_or("frozen input bytes")?;
        let input = contracts::parse_strict(bytes).map_err(|e| format!("{e:?}"))?;
        let target = contracts::parse_strict(
            pins.get(&execute.target.artifact.digest)
                .ok_or("target bytes")?,
        )
        .map_err(|e| format!("{e:?}"))?;
        if target
            != json!({"v":"openagents.work-pure-operation.v1","requires":[],"operation":"echo-frozen-objective","effects":[],"implementation":"rust-native"})
            || input["v"] != "openagents.work-frozen-input.v1"
        {
            return Err("unsupported pure worker target or input".into());
        }
        let (claim, root) = match state
            .service
            .prepare(&opened, now, &workspace_host::artifact::token())
            .map_err(|e| format!("{e:?}"))?
        {
            nostr::execution::Admission::Reserved { claim, root } => (claim, root),
            nostr::execution::Admission::Retransmission(_) => {
                return Err("unexpected duplicate worker admission".into());
            }
        };
        let root_ref =
            self.store
                .put(&root, "application/json", Some(nostr::run::RECORD_SCHEMA))?;
        state.roots.insert(key.clone(), root_ref.clone());
        self.store.save(&state, &self.signer)?;
        state
            .service
            .acknowledge(&claim.key, &root)
            .map_err(|e| format!("{e:?}"))?;
        state
            .service
            .intend(&claim.key)
            .map_err(|e| format!("{e:?}"))?;
        self.store.save(&state, &self.signer)?;
        let output = json!({"v":"openagents.work-pure-output.v1","requires":[],"work_revision":input["work_revision"],"frame":input["frame"],"job":input["job"],"value":input["objective"]});
        let bytes = contracts::jcs(&output).map_err(|e| format!("{e:?}"))?;
        if bytes.len() as u64 > execute.bounds.output_bytes.ok_or("output byte bound")? {
            return Err("pure worker output exceeds its declared bound".into());
        }
        let output_ref = self.store.put_json(&output)?;
        let result = state
            .service
            .observe(
                &claim.key,
                nostr::execution::Report {
                    outcome: "completed",
                    dispatched: true,
                    output: Some(output_ref.clone()),
                    artifacts: vec![root_ref.clone(), output_ref.clone()],
                    receipts: vec![],
                    spend: Some(0),
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: None,
                    message: None,
                },
            )
            .map_err(|e| format!("{e:?}"))?;
        let peer = opened
            .principal
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "caller key")?;
        let event = nostr::execution::Seal {
            signer: &self.signer,
            conversation: nostr::nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: now,
        }
        .event(
            nostr::execution::RESULT_KIND,
            vec![
                nostr::domain::Tag::new(vec!["e".into(), event.id.clone()]),
                nostr::domain::Tag::new(vec!["p".into(), opened.principal]),
            ],
            &result,
        )
        .map_err(|e| format!("{e:?}"))?;
        state.events.insert(key.clone(), event.clone());
        state.outputs.insert(key, json!([root_ref, output_ref]));
        self.store.save(&state, &self.signer)?;
        let closure = BTreeMap::from([
            (contracts::digest_bytes(&root), root),
            (contracts::digest_bytes(&bytes), bytes),
        ]);
        Ok((event, closure))
    }
}
