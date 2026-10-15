//! Finite Git-document checkpoints and separately admitted restoration runs.
use crate::{
    Result,
    artifact::{self},
    host::{ApprovalVerifier, Host, State},
};
use nostr::{contracts, domain::Event, workspace};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Restore {
    pub actor: String,
    pub fingerprint: String,
    pub admission: Value,
    pub plan: Value,
    pub steps: Vec<Value>,
    pub results: Vec<Value>,
    pub journal: Vec<Value>,
    pub status: String,
}
impl Host {
    fn checkpoint_locked(
        &self,
        state: &State,
        actor: &str,
        resources: &BTreeSet<String>,
        now: u64,
        retain_until: u64,
    ) -> Result<Value> {
        if resources.is_empty()
            || resources.len() > 16
            || retain_until <= now
            || retain_until > now + 7 * 86_400
        {
            return Err("checkpoint selection or retention exceeds its bound".into());
        }
        let mut documents = Vec::new();
        let mut sources = Vec::new();
        for id in resources {
            self.check(state, actor, Some(id), false, now)?;
            let resource = state.resources.get(id).ok_or("resource unavailable")?;
            let (snapshot, body) = self.capture(state, resource, now)?;
            documents.push(json!({"resource":resource.reference,"snapshot":snapshot}));
            sources.push(body["observation"].clone());
        }
        let exclusions = json!([
            "worktree_files",
            "staged",
            "untracked",
            "ignored",
            "provider_history",
            "credentials",
            "processes"
        ]);
        let manifest = json!({"v":"openagents.ws-git-checkpoint-manifest.v1","requires":[],"binding":state.binding_ref,"documents":documents,"exclusions":exclusions});
        let manifest = self.artifacts.put_json(&manifest)?;
        let schema = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["v","requires","binding","documents","exclusions"],"properties":{"v":{"const":"openagents.ws-git-checkpoint-manifest.v1"},"requires":{"type":"array","maxItems":0},"binding":{"type":"object"},"documents":{"type":"array","maxItems":16,"items":{"type":"object","additionalProperties":false,"required":["resource","snapshot"],"properties":{"resource":{"type":"object"},"snapshot":{"type":"object"}}}},"exclusions":{"const":exclusions}}});
        let manifest_schema = self.artifacts.put(
            &contracts::jcs(&schema).map_err(|e| e.to_string())?,
            "application/schema+json",
            None,
        )?;
        let body = json!({"v":"openagents.workspace-checkpoint.v1","requires":[],"checkpoint":artifact::token(),"binding":state.binding_ref,"sources":sources,"manifest":manifest,"manifest_schema":manifest_schema,"capture":"version_vector","coverage":"complete","omissions":[],"created_at":now,"retain_until":retain_until});
        workspace::validate(&body).map_err(|e| e.to_string())?;
        let reference = self.artifacts.put_json(&body)?;
        Ok(json!({"reference":reference,"body":body}))
    }
    pub fn checkpoint(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, request) =
            self.authenticated(event, now, "openagents.ws-checkpoint-create.v1")?;
        closed(&request, &["v", "requires", "resources", "retain_until"])?;
        let _guard = self.artifacts.lock()?;
        let state = self.state()?;
        let mut resources = BTreeSet::new();
        for r in request["resources"]
            .as_array()
            .filter(|a| a.len() <= 16)
            .ok_or("checkpoint resources")?
        {
            let r = workspace::resource(r).map_err(|e| e.to_string())?;
            let actual = state
                .resources
                .get(&r.resource)
                .ok_or("resource unavailable")?;
            if actual.reference != r || !resources.insert(r.resource) {
                return Err("checkpoint resource identity differs or repeats".into());
            }
        }
        self.checkpoint_locked(
            &state,
            &actor,
            &resources,
            now,
            request["retain_until"]
                .as_u64()
                .ok_or("checkpoint retain_until")?,
        )
    }
    /// The host creates an inert plan and safety checkpoint for review. The
    /// caller later signs a distinct admission that pins those exact bytes.
    pub fn plan_restore(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, request) = self.authenticated(event, now, "openagents.ws-restore-plan.v1")?;
        closed(&request, &["v", "requires", "checkpoint"])?;
        let _guard = self.artifacts.lock()?;
        let mut state = self.state()?;
        let grant = self.check(&state, &actor, None, true, now)?.clone();
        let checkpoint = self.artifacts.json(&request["checkpoint"])?;
        workspace::validate(&checkpoint).map_err(|e| e.to_string())?;
        if checkpoint["v"] != "openagents.workspace-checkpoint.v1"
            || checkpoint["coverage"] != "complete"
            || now
                > checkpoint["retain_until"]
                    .as_u64()
                    .ok_or("checkpoint retention")?
        {
            return Err("checkpoint is unavailable or incomplete".into());
        }
        let manifest = self.artifacts.json(&checkpoint["manifest"])?;
        closed(
            &manifest,
            &["v", "requires", "binding", "documents", "exclusions"],
        )?;
        if manifest["v"] != "openagents.ws-git-checkpoint-manifest.v1"
            || manifest["binding"] != checkpoint["binding"]
            || checkpoint["capture"] != "version_vector"
        {
            return Err("unsupported checkpoint reconstruction".into());
        }
        let docs = manifest["documents"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 16)
            .ok_or("checkpoint documents")?;
        let mut resources = BTreeSet::new();
        let mut targets = Vec::new();
        let mut admitted_sources = Vec::new();
        for row in docs {
            let r = workspace::resource(&row["resource"]).map_err(|e| e.to_string())?;
            self.check(&state, &actor, Some(&r.resource), true, now)?;
            let resource = state
                .resources
                .get(&r.resource)
                .filter(|v| v.reference == r)
                .ok_or("checkpoint source identity changed")?;
            if admitted_sources
                .iter()
                .any(|source: &crate::git::GitDocument| source.same_ref(&resource.source))
            {
                return Err(
                    "cannot_enforce: restore one document per Git ref in this profile".into(),
                );
            }
            admitted_sources.push(resource.source.clone());
            let snapshot = self.artifacts.json(&row["snapshot"])?;
            let observation = self.artifacts.json(&snapshot["observation"])?;
            let bytes = self.artifacts.get(&snapshot["content"])?;
            workspace::verify_document(&snapshot, &observation, Some(&bytes))
                .map_err(|e| e.to_string())?;
            if snapshot["resource"] != row["resource"]
                || observation["adapter"] != resource.adapter
                || !resources.insert(r.resource.clone())
            {
                return Err("checkpoint source identity differs or repeats".into());
            }
            targets.push((r, snapshot));
        }
        let safety = self.checkpoint_locked(&state, &actor, &resources, now, now + 3600)?;
        let mut expected = Vec::new();
        let mut steps = Vec::new();
        let mut impact = Vec::new();
        for (r, target) in targets {
            let resource = state.resources.get(&r.resource).ok_or("resource")?;
            let (snapshot, current) = self.capture(&state, resource, now)?;
            let input = json!({"v":"openagents.document-replacement.v1","requires":[],"snapshot":snapshot,"content":target["content"],"encoding":target["encoding"]});
            workspace::validate(&input).map_err(|e| e.to_string())?;
            steps.push(
                json!({"operation":resource.operation,"input":self.artifacts.put_json(&input)?}),
            );
            expected.push(current["observation"].clone());
            impact.push(json!({"resource":r,"before":current["observation"],"target":target["observation"],"working_tree":"unchanged","staged":"unchanged","untracked":"unchanged","ignored":"unchanged"}));
        }
        let impact=self.artifacts.put_json(&json!({"v":"openagents.ws-git-restore-impact.v1","requires":[],"changes":impact,"provider_history":"excluded","processes":"excluded"}))?;
        let plan = json!({"v":"openagents.workspace-restore.v1","requires":[],"command":artifact::token(),"checkpoint":request["checkpoint"],"binding":state.binding_ref,"expected":expected,"impact":impact,"steps":steps,"safety_checkpoint":safety["reference"],"authorization":grant.reference,"issued_at":now,"expires_at":now+300});
        workspace::validate(&plan).map_err(|e| e.to_string())?;
        if state.restore_plans.len() >= 256 {
            return Err("restore plan retention bound exceeded".into());
        }
        let reference = self.artifacts.put_json(&plan)?;
        state.restore_plans.insert(
            reference["digest"].as_str().ok_or("plan digest")?.into(),
            (actor, reference.clone()),
        );
        self.save(&state)?;
        Ok(
            json!({"reference":reference,"body":plan,"impact":self.artifacts.json(&impact)?,"safety_checkpoint":safety}),
        )
    }
    pub fn restore(
        &self,
        event: &Event,
        now: u64,
        approvals: Option<&dyn ApprovalVerifier>,
    ) -> Result<Value> {
        let started = std::time::Instant::now();
        let (actor, admission) =
            self.authenticated(event, now, "openagents.workspace-action-admission.v1")?;
        workspace::validate(&admission).map_err(|e| e.to_string())?;
        if admission["actor"] != actor {
            return Err("restoration signer differs".into());
        }
        let plan = self.artifacts.json(&admission["proposal"])?;
        workspace::validate(&plan).map_err(|e| e.to_string())?;
        if plan["v"] != "openagents.workspace-restore.v1" {
            return Err("unsupported restoration".into());
        }
        let command = plan["command"].as_str().ok_or("restoration command")?;
        let key = format!("{actor}:{command}");
        let fingerprint =
            contracts::digest_bytes(&contracts::jcs(&admission).map_err(|e| e.to_string())?);
        {
            let _guard = self.artifacts.lock()?;
            let mut state = self.state()?;
            let grant = self.check(&state, &actor, None, true, now)?.clone();
            if state
                .restore_plans
                .get(
                    admission["proposal"]["digest"]
                        .as_str()
                        .ok_or("plan digest")?,
                )
                .is_none_or(|(issuer, reference)| {
                    issuer != &actor || *reference != admission["proposal"]
                })
            {
                return Err(
                    "not_admitted: restoration plan was not prepared by this host for this actor"
                        .into(),
                );
            }
            if let Some(prior) = state.restores.get(&key).cloned() {
                if prior.fingerprint != fingerprint {
                    return Err("idempotency_conflict: restoration bytes differ".into());
                }
                for admission in &prior.steps {
                    let proposal = self.artifacts.json(&admission["proposal"])?;
                    let resource =
                        workspace::resource(&proposal["resource"]).map_err(|e| e.to_string())?;
                    self.check(&state, &actor, Some(&resource.resource), true, now)?;
                    let command_key = format!(
                        "{actor}:{}",
                        proposal["command"].as_str().ok_or("child command")?
                    );
                    if state
                        .commands
                        .get(&command_key)
                        .is_some_and(|c| c.result.is_none())
                    {
                        self.recover_locked(&mut state, &command_key, now)?;
                    }
                }
                if matches!(prior.status.as_str(), "running" | "admitted") {
                    state.restores.get_mut(&key).ok_or("restoration")?.status =
                        "interrupted".into();
                    self.save(&state)?;
                }
                return self.restore_status(&state, &key);
            }
            if state.restores.len() >= 128 {
                return Err("restoration retention limit exceeded".into());
            }
            if plan["binding"] != state.binding_ref
                || plan["authorization"] != grant.reference
                || now < plan["issued_at"].as_u64().ok_or("plan issued_at")?
                || now >= plan["expires_at"].as_u64().ok_or("plan expires_at")?
            {
                return Err("stale: restoration authority or plan changed".into());
            }
            let checkpoint = self.artifacts.json(&plan["safety_checkpoint"])?;
            workspace::validate(&checkpoint).map_err(|e| e.to_string())?;
            if checkpoint["binding"] != state.binding_ref || checkpoint["coverage"] != "complete" {
                return Err("safety checkpoint differs".into());
            }
            if grant.approval_required {
                let verifier =
                    approvals.ok_or("cannot_enforce: restoration requires an approval verifier")?;
                if admission["approval"].is_null() {
                    return Err("restoration requires an exact action approval".into());
                }
                verifier.approve(
                    &actor,
                    &admission["proposal"],
                    &self.artifacts.json(&admission["approval"])?,
                    now,
                )?;
            }
            let steps = plan["steps"]
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 16)
                .ok_or("restoration steps")?;
            let mut admissions = Vec::new();
            let mut resources = BTreeSet::new();
            for (index, step) in steps.iter().enumerate() {
                let replacement = self.artifacts.json(&step["input"])?;
                workspace::validate(&replacement).map_err(|e| e.to_string())?;
                let snapshot = self.artifacts.json(&replacement["snapshot"])?;
                workspace::validate(&snapshot).map_err(|e| e.to_string())?;
                let r = workspace::resource(&snapshot["resource"]).map_err(|e| e.to_string())?;
                self.check(&state, &actor, Some(&r.resource), true, now)?;
                let resource = state
                    .resources
                    .get(&r.resource)
                    .filter(|v| v.reference == r)
                    .ok_or("restoration source identity changed")?;
                if !resources.insert(r.resource.clone())
                    || resource.operation != step["operation"]
                    || snapshot["binding"] != state.binding_ref
                    || !plan["expected"]
                        .as_array()
                        .is_some_and(|a| a.contains(&snapshot["observation"]))
                {
                    return Err("restoration step is outside its declared impact".into());
                }
                let observation = self.artifacts.json(&snapshot["observation"])?;
                let capture = resource.source.capture()?;
                if observation["provider_revision"] != capture.position {
                    return Err("stale: restoration source changed during preflight".into());
                }
                let child_command =
                    contracts::digest_bytes(format!("{command}:{index}").as_bytes())[7..]
                        .to_string();
                let proposal = json!({"v":"openagents.resource-change.v1","requires":[],"command":child_command,"actor":actor,"resource":r,"binding":state.binding_ref,"expected":snapshot["observation"],"operation":step["operation"],"input":step["input"],"authorization":grant.reference,"issued_at":plan["issued_at"],"expires_at":plan["expires_at"]});
                workspace::validate(&proposal).map_err(|e| e.to_string())?;
                let proposal = self.artifacts.put_json(&proposal)?;
                admissions.push(json!({"v":"openagents.workspace-action-admission.v1","requires":[],"proposal":proposal,"actor":actor,"approval":null,"meta":{"restoration":admission["proposal"]}}));
            }
            if plan["expected"].as_array().map(Vec::len) != Some(resources.len()) {
                return Err("restoration expected-source vector differs".into());
            }
            let mut restore = Restore {
                actor: actor.clone(),
                fingerprint,
                admission: admission.clone(),
                plan: plan.clone(),
                steps: admissions,
                results: Vec::new(),
                journal: Vec::new(),
                status: "admitted".into(),
            };
            self.run_record(&mut restore,command,"created",None,json!({"owner":state.binding["owner"],"request":artifact::json_ref(&admission)?,"base":plan["binding"],"program":admission["proposal"],"lock":contracts::digest_bytes(&contracts::jcs(&plan).map_err(|e|e.to_string())?),"context":plan["safety_checkpoint"],"policy":state.binding["policy"],"parent":null,"recipients":[actor]}),now)?;
            self.run_record(&mut restore,command,"admitted",None,json!({"enforcement":plan["impact"],"reservations":[],"deadlines":{"expires_at":plan["expires_at"]},"retention":{"retain_until":now+3600},"input":admission["proposal"]}),now)?;
            state.restores.insert(key.clone(), restore);
            self.artifacts.put_json(&admission)?;
            self.save(&state)?;
        }
        // A fresh live admission performs the finite steps. Retransmission or
        // restart returns retained/reconciled state and never resumes dispatch.
        let steps = self.state()?.restores[&key].steps.clone();
        for (index, step) in steps.iter().enumerate() {
            let time = now.saturating_add(started.elapsed().as_secs());
            {
                let _guard = self.artifacts.lock()?;
                let mut state = self.state()?;
                let restore = state.restores.get_mut(&key).ok_or("restoration")?;
                restore.status = "running".into();
                self.run_record(restore,command,"dispatched",Some(index),json!({"binding":plan["binding"],"attempt":1,"input":step["proposal"],"context":plan["safety_checkpoint"],"effect":plan["impact"],"generation":0}),time)?;
                self.save(&state)?;
            }
            let result = self.change_admitted(
                actor.clone(),
                step.clone(),
                &BTreeMap::new(),
                time,
                None,
                Some(&plan),
            );
            let _guard = self.artifacts.lock()?;
            let mut state = self.state()?;
            let (outcome, reference, dispatched) = match result {
                Ok(body) => {
                    let reference = self.artifacts.put_json(&body)?;
                    (
                        body["outcome"].as_str().ok_or("step outcome")?.to_string(),
                        Some(reference),
                        body["dispatched"] == true,
                    )
                }
                Err(_) => {
                    let proposal = self.artifacts.json(&step["proposal"])?;
                    let command_key = format!(
                        "{actor}:{}",
                        proposal["command"].as_str().ok_or("step command")?
                    );
                    let dispatched = state
                        .commands
                        .get(&command_key)
                        .is_some_and(|c| c.dispatched);
                    (
                        if dispatched { "unknown" } else { "refused" }.into(),
                        None,
                        dispatched,
                    )
                }
            };
            let restore = state.restores.get_mut(&key).ok_or("restoration")?;
            restore.results.push(
                json!({"step":index,"outcome":outcome,"result":reference,"dispatched":dispatched}),
            );
            self.run_record(restore,command,"resolved",Some(index),json!({"outcome":outcome,"dispatched":dispatched,"output":reference,"receipts":reference.into_iter().collect::<Vec<_>>(),"usage":{"wall_ms":null,"spend_microunits":null},"verification":if outcome=="completed"{"passed"}else{"not_run"},"integration":if outcome=="completed"{"accepted"}else{"not_requested"}}),time)?;
            if outcome != "completed" {
                restore.status = if outcome == "unknown" {
                    "unknown"
                } else {
                    "partial"
                }
                .into();
                self.save(&state)?;
                break;
            }
            restore.status = if index + 1 == steps.len() {
                "completed"
            } else {
                "running"
            }
            .into();
            self.save(&state)?;
            #[cfg(test)]
            if let Some(hook) = &self.restore_hook {
                hook(index);
            }
        }
        {
            let _guard = self.artifacts.lock()?;
            let mut state = self.state()?;
            let restore = state.restores.get_mut(&key).ok_or("restoration")?;
            if matches!(restore.status.as_str(), "completed" | "partial") {
                let outcome = if restore.status == "completed" {
                    "completed"
                } else {
                    "failed"
                };
                let results = restore.results.clone();
                self.run_record(restore,command,"settled",None,json!({"outcome":outcome,"unknowns":[],"verification":if outcome=="completed"{"passed"}else{"failed"},"integration":if outcome=="completed"{"accepted"}else{"pending"},"results":results,"reservations":[]}),now.saturating_add(started.elapsed().as_secs()))?;
                self.save(&state)?;
            }
        }
        let state = self.state()?;
        self.restore_status(&state, &key)
    }
    fn run_record(
        &self,
        restore: &mut Restore,
        run: &str,
        kind: &str,
        step: Option<usize>,
        data: Value,
        now: u64,
    ) -> Result<()> {
        let previous = restore.journal.last().map(|e| e["digest"].clone());
        let record = json!({"run":run,"seq":restore.journal.len(),"previous":previous,"controller":self.signer.pubkey(),"generation":0,"type":kind,"subject":{"step":step.map(|n|format!("restore-{n}")),"iteration":null,"attempt":step.map(|_|1)},"time":now,"data":data});
        let digest = nostr::run::logical_digest(&record).map_err(|e| e.to_string())?;
        let envelope =
            json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"record":record,"digest":digest});
        let mut journal = nostr::run::Journal::default();
        for old in restore.journal.iter().chain(std::iter::once(&envelope)) {
            let record = nostr::run::parse_envelope(old).map_err(|e| e.to_string())?;
            nostr::run::ingest(&mut journal, record, self.signer.pubkey())
                .map_err(|e| e.to_string())?;
        }
        self.artifacts.put_json(&envelope)?;
        restore.journal.push(envelope);
        Ok(())
    }
    fn restore_status(&self, state: &State, key: &str) -> Result<Value> {
        let restore = state.restores.get(key).ok_or("restoration")?;
        let mut steps = Vec::new();
        for (index, admission) in restore.steps.iter().enumerate() {
            let proposal = self.artifacts.json(&admission["proposal"])?;
            let command = format!(
                "{}:{}",
                restore.actor,
                proposal["command"].as_str().ok_or("child command")?
            );
            let status = state.commands.get(&command);
            steps.push(json!({"step":index,"result":status.and_then(|c|c.result.clone()),"dispatched":status.is_some_and(|c|c.dispatched),"state":if status.is_some_and(|c|c.result.is_some()){"recorded"}else if status.is_some_and(|c|c.dispatched){"unknown"}else{"unattempted"}}));
        }
        Ok(
            json!({"v":"openagents.ws-local-restore-status.v1","requires":[],"admission":artifact::json_ref(&restore.admission)?,"plan":artifact::json_ref(&restore.plan)?,"status":restore.status,"steps":steps,"run_records":restore.journal.iter().map(artifact::json_ref).collect::<Result<Vec<_>>>()?,"working_tree":"unchanged","provider_history":"excluded"}),
        )
    }
}
fn closed(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or("closed checkpoint object required")?;
    if object.len() != keys.len()
        || object.keys().any(|k| !keys.contains(&k.as_str()))
        || value["requires"] != json!([])
    {
        return Err("unsupported checkpoint fields".into());
    }
    Ok(())
}
