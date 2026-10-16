//! Original private declarations, checked RUN extensions, and immutable states.
use crate::{
    Result,
    host::{Automation, Record, State},
};
use nostr::{automation, domain::Event};
use serde_json::{Value, json};
impl Automation {
    pub(crate) fn authenticate(&self, event: &Event, now: u64) -> Result<(String, Value)> {
        if event.kind == nostr::execution::REQUEST_KIND {
            return self.authenticated_cj(event, now);
        }
        let opened =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        if opened.recipient() != self.authority()
            || opened.body().issued_at > now.saturating_add(300)
            || now > opened.body().retain_until
        {
            return Err("not_admitted: private automation declaration unavailable".into());
        }
        let body = automation::parse(
            opened
                .inline_bytes()
                .ok_or("inline automation declaration required")?,
        )
        .map_err(|e| e.to_string())?;
        if body["v"] != opened.artifact().schema.as_deref().unwrap_or("") {
            return Err("automation envelope schema differs".into());
        }
        Ok((opened.signer().into(), body))
    }
    pub(crate) fn retain(&self, state: &mut State, event: &Event, body: &Value) -> Result<Value> {
        let reference = self.store.put_json(body)?;
        let events = state
            .originals
            .entry(reference["digest"].as_str().ok_or("digest")?.into())
            .or_default();
        if !events.iter().any(|e| e.id == event.id) {
            if events.len() >= 64 {
                return Err("automation declaration audience bound".into());
            }
            events.push(event.clone());
        }
        Ok(reference)
    }
    pub(crate) fn declare(
        &self,
        state: &mut State,
        body: &Value,
        recipient: &str,
        now: u64,
    ) -> Result<Value> {
        let event = workspace_host::host::request(&self.secret, recipient, body.clone(), now)?;
        self.retain(state, &event, body)
    }
    pub(crate) fn transition(
        &self,
        state: &mut State,
        record: &mut Record,
        status: &str,
        evidence: Vec<Value>,
        now: u64,
    ) -> Result<Value> {
        if record.journal.len() >= 8192 {
            return Err("automation journal bound".into());
        }
        let plan = &record.body;
        if record.journal.is_empty() {
            let root = json!({"run":plan["run"],"seq":0,"previous":null,"controller":self.authority(),"generation":0,"type":"created","subject":{"step":null,"iteration":null,"attempt":null},"time":now,"data":{"owner":self.authority(),"request":record.reference,"base":plan["frame"],"program":plan["target"],"lock":plan["lock"],"context":plan["context"],"policy":plan["policy"],"parent":plan["reservation"],"recipients":[plan["owner"]]}});
            let body = json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"digest":nostr::run::logical_digest(&root).map_err(|e|e.to_string())?,"record":root});
            record.journal.push(body);
        }
        let entry = json!({"run":plan["run"],"seq":record.journal.len(),"previous":record.journal.last().ok_or("RUN root")?["digest"],"controller":self.authority(),"generation":0,"type":"observed","subject":{"step":null,"iteration":null,"attempt":null},"time":now,"data":{"evidence":evidence,"sources":[],"complete":!record.accounting.unresolved(),"provenance":{"operation":"automation-transition","status":status,"occurrences":record.occurrences.len(),"observations":record.polls.len(),"next_slot":record.next_slot}}});
        let body = json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"digest":nostr::run::logical_digest(&entry).map_err(|e|e.to_string())?,"record":entry});
        let mut journal = nostr::run::Journal::default();
        for b in record.journal.iter().chain(std::iter::once(&body)) {
            let parsed = nostr::run::parse_envelope(b).map_err(|e| e.to_string())?;
            if nostr::run::ingest(&mut journal, parsed, self.authority())
                .map_err(|e| e.to_string())?
                != nostr::run::Ingest::Applied
            {
                return Err("automation RUN does not extend history".into());
            }
        }
        let run_ref = self.store.put_json(&body)?;
        record.journal.push(body);
        let previous = record.history.last().cloned().unwrap_or(Value::Null);
        let accounting=self.store.put_json(&json!({"v":"openagents.automation-native-accounting.v1","requires":[],"plan":record.reference,"ledger":record.accounting}))?;
        let active = record
            .active
            .as_ref()
            .and_then(|id| record.occurrences.get(id))
            .map(|r| r["reference"].clone());
        let last_outcome = record
            .outcomes
            .values()
            .max_by_key(|v| v["ordinal"].as_u64())
            .map(|v| v["reference"].clone());
        let evaluation = record
            .evaluations
            .values()
            .last()
            .map(|v| v["reference"].clone());
        let next = json!({"v":"openagents.automation-state.v1","requires":[],"plan":record.reference,"revision":record.history.len(),"previous":previous,"controller":self.authority(),"generation":0,"status":status,"occurrences":record.occurrences.len(),"observations":record.polls.len(),"active":active,"last_outcome":last_outcome,"evaluation":evaluation,"accounting":accounting,"record":run_ref});
        automation::validate(&next).map_err(|e| e.to_string())?;
        let reference = self.declare(state, &next, plan["owner"].as_str().ok_or("owner")?, now)?;
        record.state = next;
        record.history.push(reference.clone());
        Ok(reference)
    }
    pub(crate) fn result(
        &self,
        state: &mut State,
        input: Value,
        record: &Record,
        status: &str,
        now: u64,
    ) -> Result<Value> {
        let body = json!({"v":"openagents.automation-result.v1","requires":[],"input":input,"status":status,"state":record.history.last(),"reason":null});
        automation::validate(&body).map_err(|e| e.to_string())?;
        self.declare(
            state,
            &body,
            record.body["owner"].as_str().ok_or("owner")?,
            now,
        )?;
        Ok(body)
    }
    pub(crate) fn plan_key(state: &State, reference: &Value) -> Result<String> {
        state
            .plans
            .iter()
            .find(|(_, p)| p.reference == *reference)
            .map(|(id, _)| id.clone())
            .ok_or_else(|| "not_admitted: automation plan unavailable".into())
    }
    /// Local owner inspection. Signed remote status uses `control`.
    pub fn inspect(&self, reference: &Value) -> Result<Value> {
        let _guard = self.store.lock()?;
        let state = self.state()?;
        let id = Self::plan_key(&state, reference)?;
        let p = &state.plans[&id];
        Ok(
            json!({"plan":p.reference,"state":p.state,"history":p.history,"journal":p.journal,"occurrences":p.occurrences,"outcomes":p.outcomes,"polls":p.polls,"evaluations":p.evaluations,"skips":p.skips,"pending":p.pending,"accounting":p.accounting}),
        )
    }
}
