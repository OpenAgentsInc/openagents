//! Source polls reserve allowance before crossing the source receiver boundary.
use crate::{
    Result,
    host::{Automation, Record, State},
    source::{Reply, SourceGate, SourceIntake},
};
use nostr::{
    automation, contracts,
    domain::{Event, Tag},
    execution,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Automation {
    pub(crate) fn prepare_poll(
        &self,
        state: &mut State,
        record: &mut Record,
        phase: &str,
        slot: Option<u64>,
        occurrence: Option<Value>,
        now: u64,
    ) -> Result<Event> {
        if record.polls.len() as u64
            >= record.body["max_observations"]
                .as_u64()
                .ok_or("poll limit")?
        {
            return Err("limit_exceeded: source observation allowance exhausted".into());
        }
        if record.pending.is_some() {
            return Err("unknown: reconcile the earlier pending request".into());
        }
        let source = self.source.as_ref().ok_or("source not mounted")?;
        let bundle = source.binding()?;
        let units = crate::plans::units(&record.body["trigger"]["bounds"])?;
        let poll = workspace_host::artifact::token();
        let request = workspace_host::artifact::token();
        let run = workspace_host::artifact::token();
        let mut deadline = now
            .checked_add(units["wall_ms"].div_ceil(1000))
            .ok_or("poll deadline")?
            .min(record.body["expires_at"].as_u64().ok_or("expiry")?);
        if phase == "revalidate" {
            deadline =
                deadline.min(
                    record.occurrences[record.active.as_ref().ok_or("active occurrence")?]["body"]
                        ["deadline"]
                        .as_u64()
                        .ok_or("occurrence deadline")?,
                );
        }
        if deadline <= now {
            return Err("stale: no source execution time remains".into());
        }
        let body = json!({"v":"openagents.automation-poll.v1","requires":[],"plan":record.reference,"poll":poll,"phase":phase,"slot":slot,"occurrence":occurrence,"request":request,"run":run,"deadline":deadline});
        automation::validate(&body).map_err(|e| e.to_string())?;
        let reference = self.declare(
            state,
            &body,
            record.body["owner"].as_str().ok_or("owner")?,
            now,
        )?;
        // Revalidation is already covered by the active occurrence's full hold.
        if phase == "observe" {
            record.accounting.reserve(&poll, units.clone())?;
        }
        let (parent, _) = self.coordinator.automation_parent(
            &record.body,
            &state.policy.worker,
            &state.policy.checker,
            &BTreeMap::new(),
            now,
        )?;
        let admission = workspace_host::host::request(
            &self.secret,
            self.coordinator.authority(),
            json!({"v":"openagents.automation-native-source-input.v1","requires":[],"coordinator":record.body["coordinator"],"claim":parent["claim"],"source":self.coordinator.automation_source_operation(record.body["trigger"]["resource"].as_str().ok_or("source resource")?)?,"resource":record.body["trigger"]["resource"],"request":request,"phase":phase,"poll":reference,"plan":record.reference,"executor":source.authority()}),
            now,
        )?;
        let payload = json!({"v":execution::SCHEMA,"requires":[],"type":"execute","request":request,"attempt":1,"run":run,"target":bundle["target"],"lock":bundle["lock"],"input":{"v":"openagents.automation-native-source-call.v1","requires":[],"config":bundle["config"],"admission":admission},"context":bundle["context"],"requirements":bundle["requirements"],"bounds":{"wall_ms":units["wall_ms"].min((deadline-now).saturating_mul(1000)),"output_bytes":units["output_bytes"],"jobs":1,"spend_microunits":0},"deadline":deadline,"retain_until":record.body["retain_until"]});
        let peer = source
            .authority()
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "source key")?;
        let event = execution::Seal {
            signer: &self.signer,
            conversation: nostr::nip44::conversation_key(&self.secret, &peer),
            nonce: secp256k1::rand::random(),
            created_at: now,
        }
        .event(
            execution::REQUEST_KIND,
            vec![
                Tag::new(vec!["p".into(), source.authority().into()]),
                Tag::new(vec!["expiration".into(), deadline.to_string()]),
            ],
            &payload,
        )
        .map_err(|e| format!("{e:?}"))?;
        record.polls.insert(poll.clone(),json!({"reference":reference,"body":body,"event":event,"payload":payload,"admission":admission,"status":"pending","observation":null,"result":null}));
        record.pending = Some(json!({"phase":"source","poll":poll,"event":event}));
        Ok(event)
    }
    pub(crate) fn check_source_intake(
        &self,
        state: &State,
        record: &Record,
        proof: &SourceIntake,
        now: u64,
    ) -> Result<()> {
        let source = self.source.as_ref().ok_or("native source not mounted")?;
        let pending = record
            .pending
            .as_ref()
            .ok_or("no admitted pending source call")?;
        let poll = record
            .polls
            .get(pending["poll"].as_str().ok_or("poll")?)
            .ok_or("admitted poll")?;
        if record.state["status"] != "active"
            || now >= record.body["expires_at"].as_u64().ok_or("expiry")?
            || now >= poll["body"]["deadline"].as_u64().ok_or("poll deadline")?
            || proof.authority() != source.authority()
            || proof.principal() != self.authority()
            || pending["phase"] != "source"
            || pending["event"]["id"] != proof.event()
            || poll["payload"] != *proof.payload()
            || contracts::digest_value(proof.payload()).map_err(|e| e.to_string())?
                != proof.fingerprint()
            || proof.admission()["poll"] != poll["reference"]
            || proof.admission()["plan"] != record.reference
            || proof.admission()["resource"] != record.body["trigger"]["resource"]
        {
            return Err("not_admitted: exact current source intake fence required".into());
        }
        self.coordinator.automation_parent(
            &record.body,
            &state.policy.worker,
            &state.policy.checker,
            &BTreeMap::new(),
            now,
        )?;
        Ok(())
    }
    pub(crate) fn accept_poll(
        &self,
        state: &mut State,
        record: &mut Record,
        response: &Event,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Value> {
        let source = self.source.as_ref().ok_or("source not mounted")?;
        let pending = record.pending.as_ref().ok_or("pending poll")?;
        let poll_id = pending["poll"].as_str().ok_or("poll ID")?.to_string();
        let poll = record.polls[&poll_id].clone();
        let request: Event =
            serde_json::from_value(poll["event"].clone()).map_err(|_| "poll request")?;
        let result = execution::bind_worker_event(
            response,
            &execution::Pending {
                worker: source.authority(),
                customer: self.authority(),
                request: poll["body"]["request"].as_str().ok_or("request")?,
                attempt: 1,
                execute_event: &request.id,
            },
            &self.secret,
        )
        .map_err(|e| format!("{e:?}"))?;
        if response.kind != execution::RESULT_KIND
            || result["run"] != poll["body"]["run"]
            || result["outcome"] != "completed"
            || result["spend"] != 0
            || files.len() > 256
            || files.values().map(Vec::len).sum::<usize>() > 1048576
        {
            return Err("unknown: complete bounded source result required".into());
        }
        for (digest, bytes) in files {
            if contracts::digest_bytes(bytes) != *digest {
                return Err("identity_mismatch: source attachment digest differs".into());
            }
        }
        let root_ref = &result["record"];
        let root_bytes = files
            .get(root_ref["digest"].as_str().ok_or("root")?)
            .ok_or("source RUN root")?;
        contracts::check_artifact_bytes(
            &contracts::parse_artifact(root_ref).map_err(|e| e.to_string())?,
            root_bytes,
        )
        .map_err(|e| e.to_string())?;
        let root = contracts::parse_strict(root_bytes).map_err(|e| e.to_string())?;
        let parsed = nostr::run::parse_envelope(&root).map_err(|e| e.to_string())?;
        if parsed.seq != 0
            || parsed.controller != source.authority()
            || root["record"]["run"] != poll["body"]["run"]
            || root["record"]["data"]["request"] != poll["body"]["request"]
            || root["record"]["data"]["program"] != poll["payload"]["target"]["artifact"]["digest"]
            || root["record"]["data"]["lock"] != poll["payload"]["lock"]["digest"]
            || root["record"]["data"]["context"] != poll["payload"]["context"]["digest"]
            || root["record"]["data"]["policy"] != poll["payload"]["requirements"]["digest"]
        {
            return Err("identity_mismatch: source RUN admission differs".into());
        }
        let output_ref = &result["output"];
        let bytes = files
            .get(output_ref["digest"].as_str().ok_or("observation")?)
            .ok_or("observation bytes")?;
        contracts::check_artifact_bytes(
            &contracts::parse_artifact(output_ref).map_err(|e| e.to_string())?,
            bytes,
        )
        .map_err(|e| e.to_string())?;
        let observation = automation::parse(bytes).map_err(|e| e.to_string())?;
        if observation["v"] != "openagents.automation-observation.v1"
            || observation["source"] != record.body["trigger"]["source"]
            || observation["resource"] != record.body["trigger"]["resource"]
            || observation["observed_at"]
                .as_u64()
                .ok_or("observation time")?
                > now
        {
            return Err("identity_mismatch: source identity/resource/time differs".into());
        }
        let mut statement = None;
        let mut owner_statement = None;
        for file in files.values() {
            if let Ok(event) = serde_json::from_slice::<Event>(file) {
                if let Ok(opened) = nostr::private_artifact::open(&event, &self.secret) {
                    if let Some(original) = opened.inline_bytes() {
                        if original == bytes.as_slice() && opened.signer() == source.authority() {
                            statement = Some(event.clone());
                        }
                        if opened.signer() == self.coordinator.authority() {
                            if let Ok(body) = automation::parse(original) {
                                if body["v"] == "openagents.automation-observation.v1"
                                    && body["resource"] == observation["resource"]
                                    && body["revision"] == observation["revision"]
                                    && body["observed_at"] == observation["observed_at"]
                                    && body["available"] == observation["available"]
                                    && body["evidence"] == observation["evidence"]
                                    && body["source"]
                                        == self.coordinator.automation_source_operation(
                                            record.body["trigger"]["resource"]
                                                .as_str()
                                                .ok_or("resource")?,
                                        )?
                                {
                                    owner_statement = Some((event.clone(), body));
                                }
                            }
                        }
                    }
                }
            }
        }
        let statement = statement.ok_or("not_admitted: original source declaration required")?;
        let (owner_event, owner_body) =
            owner_statement.ok_or("not_admitted: original native reader declaration required")?;
        for bytes in files.values() {
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        self.retain(state, &owner_event, &owner_body)?;
        let observation_ref = self.retain(state, &statement, &observation)?;
        if observation["available"] == true {
            let snapshot = self.store.json(&observation["evidence"])?;
            let native = self.store.json(&snapshot["observation"])?;
            let content = self.store.get(&snapshot["content"])?;
            nostr::workspace::verify_document(&snapshot, &native, Some(&content))
                .map_err(|e| e.to_string())?;
            if native["provider_revision"] != observation["revision"]
                || native["captured_at"] != observation["observed_at"]
            {
                return Err(
                    "identity_mismatch: original native source revision/time differs".into(),
                );
            }
        }
        if poll["body"]["phase"] == "observe" {
            let reserved = record.accounting.requests[&poll_id].reserved.clone();
            record.accounting.settle(&poll_id, Some(reserved))?;
        }
        let item = record.polls.get_mut(&poll_id).ok_or("poll")?;
        item["status"] = json!("complete");
        item["observation"] = observation_ref.clone();
        item["result"] = self.store.put_json(&result)?;
        record.pending = None;
        Ok(observation_ref)
    }
    fn source_occurrence(
        &self,
        state: &mut State,
        record: &mut Record,
        observation_ref: &Value,
        slot: u64,
        now: u64,
    ) -> Result<()> {
        if record.state["status"] != "active"
            || now >= record.body["expires_at"].as_u64().ok_or("expiry")?
        {
            return Ok(());
        }
        if self
            .coordinator
            .automation_parent(
                &record.body,
                &state.policy.worker,
                &state.policy.checker,
                &BTreeMap::new(),
                now,
            )
            .is_err()
        {
            record.state["status"] = json!("blocked");
            return Ok(());
        }
        let observation = self.store.json(observation_ref)?;
        if observation["available"] != true {
            return Ok(());
        }
        let observed = observation["observed_at"]
            .as_u64()
            .ok_or("observation time")?;
        if now.saturating_sub(observed)
            > record.body["trigger"]["max_age_seconds"]
                .as_u64()
                .ok_or("source age")?
        {
            return Ok(());
        }
        let key = json!({"kind":"source","source":record.body["trigger"]["source"],"resource":observation["resource"],"revision":observation["revision"]});
        let identity = contracts::digest_value(&json!({"plan":record.reference,"key":key}))
            .map_err(|e| e.to_string())?;
        if record.source_revisions.contains(&identity) {
            return Ok(());
        }
        if record.active.is_some() {
            return Err("unknown: previous occurrence is still active".into());
        }
        if record.occurrences.len() as u64
            >= record.body["max_occurrences"]
                .as_u64()
                .ok_or("occurrence cap")?
            || record.polls.len() as u64
                >= record.body["max_observations"].as_u64().ok_or("poll cap")?
        {
            record.state["status"] = json!("exhausted");
            return Ok(());
        }
        let occurrence = workspace_host::artifact::token();
        let allowances = crate::plans::units(&record.body["per_occurrence"])?;
        if record
            .accounting
            .reserve(&occurrence, allowances.clone())
            .is_err()
        {
            record.state["status"] = json!("exhausted");
            return Ok(());
        }
        let prior = record
            .outcomes
            .values()
            .max_by_key(|v| v["ordinal"].as_u64())
            .map(|v| v["reference"].clone());
        let body = json!({"v":"openagents.automation-occurrence.v1","requires":[],"plan":record.reference,"occurrence":occurrence,"ordinal":record.occurrences.len()+1,"key":key,"due_at":Self::due(record,slot)?,"deadline":now.checked_add(allowances["wall_ms"].div_ceil(1000)).ok_or("deadline")?.min(record.body["expires_at"].as_u64().ok_or("expiry")?),"observation":observation_ref,"prior":prior,"request":workspace_host::artifact::token(),"run":workspace_host::artifact::token(),"task":workspace_host::artifact::token()});
        automation::validate(&body).map_err(|e| e.to_string())?;
        let reference = self.declare(
            state,
            &body,
            record.body["owner"].as_str().ok_or("owner")?,
            now,
        )?;
        let wrapper = json!({"v":"openagents.automation-input.v1","requires":[],"plan":record.reference,"occurrence":reference,"frame":record.body["frame"],"payload":record.body["payload"],"context":record.body["context"],"prior":prior});
        automation::validate(&wrapper).map_err(|e| e.to_string())?;
        let input = self.store.put_json(&wrapper)?;
        record.occurrences.insert(occurrence.clone(),json!({"reference":reference,"body":body,"input":input,"phase":"prepare","started_at":now,"proposal":null,"claim":null,"execute":null,"result":null,"check":null,"revalidation":null}));
        record.source_revisions.insert(identity);
        record.active = Some(occurrence);
        Ok(())
    }
    pub(crate) fn revalidate_source(
        &self,
        state: &mut State,
        record: &mut Record,
        now: u64,
    ) -> Result<()> {
        let started = std::time::Instant::now();
        let active = record.active.clone().ok_or("active source occurrence")?;
        let occurrence = record.occurrences[&active].clone();
        let id = record.body["automation"]
            .as_str()
            .ok_or("plan ID")?
            .to_string();
        let reference = if occurrence["revalidation"].is_object() {
            occurrence["revalidation"].clone()
        } else {
            let request = if let Some(pending) = &record.pending {
                let poll = record
                    .polls
                    .get(pending["poll"].as_str().ok_or("pending poll")?)
                    .ok_or("pending revalidation")?;
                if pending["phase"] != "source"
                    || poll["body"]["phase"] != "revalidate"
                    || poll["body"]["occurrence"] != occurrence["reference"]
                {
                    return Err("unknown: another source call is pending".into());
                }
                serde_json::from_value::<Event>(pending["event"].clone())
                    .map_err(|_| "source event")?
            } else {
                let event = self.prepare_poll(
                    state,
                    record,
                    "revalidate",
                    None,
                    Some(occurrence["reference"].clone()),
                    now,
                )?;
                self.transition(state, record, "active", vec![], now)?;
                state.plans.insert(id.clone(), record.clone());
                self.save(state)?;
                event
            };
            let (response, files) = {
                let gate = HeldSourceGate {
                    host: self,
                    state,
                    record,
                };
                self.source.as_ref().ok_or("source mount")?.run(
                    &self.coordinator,
                    &request,
                    now,
                    &gate,
                )?
            };
            let reference = self.accept_poll(
                state,
                record,
                &response,
                &files,
                now.saturating_add(started.elapsed().as_secs()),
            )?;
            record.occurrences.get_mut(&active).ok_or("active")?["revalidation"] =
                reference.clone();
            self.transition(
                state,
                record,
                "active",
                vec![reference.clone()],
                now.saturating_add(started.elapsed().as_secs()),
            )?;
            state.plans.insert(id.clone(), record.clone());
            self.save(state)?;
            reference
        };
        let observation = self.store.json(&reference)?;
        let original = self.store.json(&occurrence["body"]["observation"])?;
        let effective_now = now.saturating_add(started.elapsed().as_secs());
        if observation["available"] != true
            || observation["revision"] != original["revision"]
            || effective_now
                .saturating_sub(observation["observed_at"].as_u64().ok_or("source time")?)
                > record.body["trigger"]["max_age_seconds"]
                    .as_u64()
                    .ok_or("source age")?
        {
            self.transition(state, record, "blocked", vec![reference], effective_now)?;
            state.plans.insert(id, record.clone());
            self.save(state)?;
            return Err(
                "stale: source changed or could not be freshly verified before worker intake"
                    .into(),
            );
        }
        Ok(())
    }
    fn consume_expired_source(
        &self,
        state: &mut State,
        record: &mut Record,
        now: u64,
    ) -> Result<()> {
        let trigger = &record.body["trigger"];
        let interval = standing_jobs::Interval {
            first: trigger["first_at"].as_u64().ok_or("first poll")?,
            every: trigger["every_seconds"].as_u64().ok_or("interval")?,
            expires: record.body["expires_at"].as_u64().ok_or("expiry")?,
        };
        if let Some((last, _)) = interval.latest(now) {
            self.skip(state, record, last, "expired", now)?;
        }
        Ok(())
    }
    pub fn tick_source(&self, reference: &Value, now: u64) -> Result<Value> {
        let started = std::time::Instant::now();
        let request = {
            let _guard = self.store.lock()?;
            let now = now.saturating_add(started.elapsed().as_secs());
            let mut state = self.state()?;
            let id = Self::plan_key(&state, reference)?;
            let mut record = state.plans[&id].clone();
            if record.body["trigger"]["kind"] != "source" {
                return Err("source plan required".into());
            }
            let status = record.state["status"].as_str().ok_or("status")?.to_string();
            if matches!(status.as_str(), "cancelled" | "complete" | "exhausted") {
                return Ok(record.state);
            }
            if now >= record.body["expires_at"].as_u64().ok_or("expiry")? {
                self.consume_expired_source(&mut state, &mut record, now)?;
                self.transition(&mut state, &mut record, "expired", vec![], now)?;
                let result = record.state.clone();
                state.plans.insert(id, record);
                self.save(&state)?;
                return Ok(result);
            }
            if status == "paused" {
                self.consume_paused(&mut state, &mut record, now)?;
                self.transition(&mut state, &mut record, "paused", vec![], now)?;
                let result = record.state.clone();
                state.plans.insert(id, record);
                self.save(&state)?;
                return Ok(result);
            }
            if status != "active" {
                return Ok(record.state);
            }
            if let Some(pending) = &record.pending {
                if pending["phase"] != "source" {
                    return Err("unknown: earlier admission requires reconciliation".into());
                }
                serde_json::from_value::<Event>(pending["event"].clone())
                    .map_err(|_| "source request")?
            } else {
                let trigger = &record.body["trigger"];
                let interval = standing_jobs::Interval {
                    first: trigger["first_at"].as_u64().ok_or("first poll")?,
                    every: trigger["every_seconds"].as_u64().ok_or("poll interval")?,
                    expires: record.body["expires_at"].as_u64().ok_or("expiry")?,
                };
                let Some((slot, _)) = interval.latest(now) else {
                    return Ok(record.state);
                };
                if slot < record.next_slot {
                    return Ok(record.state);
                }
                if record.active.is_some() {
                    if slot > record.next_slot {
                        self.skip(&mut state, &mut record, slot - 1, "occupied", now)?;
                        self.transition(&mut state, &mut record, "active", vec![], now)?;
                    }
                    let result = record.state.clone();
                    state.plans.insert(id, record);
                    self.save(&state)?;
                    return Ok(result);
                }
                if record.polls.len() as u64
                    >= record.body["max_observations"].as_u64().ok_or("poll cap")?
                {
                    self.skip(&mut state, &mut record, slot, "bound", now)?;
                    self.transition(&mut state, &mut record, "exhausted", vec![], now)?;
                    let result = record.state.clone();
                    state.plans.insert(id, record);
                    self.save(&state)?;
                    return Ok(result);
                }
                if slot > record.next_slot {
                    self.skip(&mut state, &mut record, slot - 1, "missed", now)?;
                }
                let event =
                    self.prepare_poll(&mut state, &mut record, "observe", Some(slot), None, now)?;
                record.next_slot = slot + 1;
                self.transition(&mut state, &mut record, "active", vec![], now)?;
                state.plans.insert(id, record);
                self.save(&state)?;
                event
            }
        };
        let (response, files) = self.source.as_ref().ok_or("source not mounted")?.run(
            &self.coordinator,
            &request,
            now.saturating_add(started.elapsed().as_secs()),
            self,
        )?;
        let _guard = self.store.lock()?;
        let now = now.saturating_add(started.elapsed().as_secs());
        let mut state = self.state()?;
        let id = Self::plan_key(&state, reference)?;
        let mut record = state.plans[&id].clone();
        let poll_id = record.pending.as_ref().ok_or("pending poll")?["poll"]
            .as_str()
            .ok_or("poll")?
            .to_string();
        let slot = record.polls[&poll_id]["body"]["slot"]
            .as_u64()
            .ok_or("observation slot")?;
        let observation_ref = self.accept_poll(&mut state, &mut record, &response, &files, now)?;
        self.source_occurrence(&mut state, &mut record, &observation_ref, slot, now)?;
        let status = record.state["status"].as_str().ok_or("status")?.to_string();
        self.transition(&mut state, &mut record, &status, vec![observation_ref], now)?;
        let result = record.state.clone();
        state.plans.insert(id, record);
        self.save(&state)?;
        Ok(result)
    }
}
impl SourceGate for Automation {
    fn with_fence(
        &self,
        proof: &SourceIntake,
        now: u64,
        call: &mut dyn FnMut() -> Result<Reply>,
    ) -> Result<Reply> {
        let started = std::time::Instant::now();
        let _guard = self.store.lock()?;
        let state = self.state()?;
        let record = state
            .plans
            .values()
            .find(|r| {
                r.pending
                    .as_ref()
                    .is_some_and(|p| p["event"]["id"] == proof.event())
            })
            .ok_or("not_admitted: pending source call unavailable")?;
        self.check_source_intake(
            &state,
            record,
            proof,
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        call()
    }
}
pub(crate) struct HeldSourceGate<'a> {
    pub host: &'a Automation,
    pub state: &'a State,
    pub record: &'a Record,
}
impl SourceGate for HeldSourceGate<'_> {
    fn with_fence(
        &self,
        proof: &SourceIntake,
        now: u64,
        call: &mut dyn FnMut() -> Result<Reply>,
    ) -> Result<Reply> {
        self.host
            .check_source_intake(self.state, self.record, proof, now)?;
        call()
    }
}
