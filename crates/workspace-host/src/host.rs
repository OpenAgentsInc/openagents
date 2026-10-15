//! Persistent host admission, exact retries, and source-confirmed changes.
use crate::projection::Projection;
use crate::{
    Result,
    artifact::{self, Artifacts},
    git::{Capture, GitDocument},
};
use nostr::{
    contracts,
    domain::{Event, RelaySigner},
    workspace::{self, ResourceRef},
};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    str::FromStr,
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub actor: String,
    pub epoch: u64,
    pub all: bool,
    pub resources: BTreeSet<String>,
    pub write: bool,
    pub approval_required: bool,
    pub expires_at: u64,
    pub revoked: bool,
    pub reference: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Resource {
    pub reference: ResourceRef,
    pub source: GitDocument,
    pub name: String,
    pub adapter: Value,
    pub operation: Value,
    pub descriptor: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommandState {
    pub fingerprint: String,
    pub actor: String,
    pub resource: String,
    pub request: Value,
    pub proposal: Value,
    pub before: Value,
    pub candidate: Option<String>,
    pub result: Option<Value>,
    pub dispatched: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub v: String,
    pub binding: Value,
    pub binding_ref: Value,
    pub resources: BTreeMap<String, Resource>,
    pub grants: BTreeMap<String, Grant>,
    pub commands: BTreeMap<String, CommandState>,
    pub projections: BTreeMap<String, Projection>,
    pub restores: BTreeMap<String, crate::checkpoint::Restore>,
    pub restore_plans: BTreeMap<String, (String, Value)>,
}
pub struct Host {
    pub(crate) artifacts: Artifacts,
    pub(crate) signer: RelaySigner,
    pub(crate) secret: SecretKey,
    #[cfg(test)]
    pub(crate) fail_after_dispatch: bool,
    #[cfg(test)]
    pub(crate) restore_hook: Option<Box<dyn Fn(usize) + Send + Sync>>,
}
/// Existing POL owners can enforce exact action decisions through this boundary.
/// No verifier is installed by default; required approvals then refuse.
pub trait ApprovalVerifier {
    fn approve(&self, actor: &str, proposal: &Value, decision: &Value, now: u64) -> Result<()>;
}
impl Host {
    pub fn create(root: PathBuf, secret: SecretKey, now: u64) -> Result<Self> {
        let host = Self::open(root, secret)?;
        let _guard = host.artifacts.lock()?;
        if host.artifacts.root().join("state.json").exists() {
            return Err("workspace already exists".into());
        }
        let policy=host.artifacts.put_json(&json!({"v":"openagents.ws-local-policy.v1","requires":[],"effects":"git_ref_document_only","filesystem_save":"cannot_enforce","approval":"per_actor","retention":"bounded"}))?;
        let authorization=host.artifacts.put_json(&json!({"v":"openagents.ws-local-authority.v1","requires":[],"owner":host.signer.pubkey(),"authority":host.signer.pubkey()}))?;
        let binding = json!({"v":"openagents.workspace-binding.v1","requires":[],"workspace":artifact::token(),"owner":host.signer.pubkey(),"authority":host.signer.pubkey(),"generation":0,"revision":0,"environment":null,"projects":[],"roots":[],"policy":policy,"authorization":authorization,"created_at":now});
        workspace::validate(&binding).map_err(|e| e.to_string())?;
        let binding_ref = host.artifacts.put_json(&binding)?;
        let grant = host.make_grant(
            host.signer.pubkey(),
            0,
            true,
            BTreeSet::new(),
            true,
            false,
            9_007_199_254_740_991,
            false,
        )?;
        let state = State {
            v: "openagents.workspace-host.v1".into(),
            binding,
            binding_ref,
            resources: BTreeMap::new(),
            grants: BTreeMap::from([(host.signer.pubkey().into(), grant)]),
            commands: BTreeMap::new(),
            projections: BTreeMap::new(),
            restores: BTreeMap::new(),
            restore_plans: BTreeMap::new(),
        };
        host.save(&state)?;
        drop(_guard);
        Ok(host)
    }
    pub fn open(root: PathBuf, secret: SecretKey) -> Result<Self> {
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|_| "invalid workspace key")?;
        Ok(Self {
            artifacts: Artifacts::new(root)?,
            signer,
            secret,
            #[cfg(test)]
            fail_after_dispatch: false,
            #[cfg(test)]
            restore_hook: None,
        })
    }
    pub(crate) fn state(&self) -> Result<State> {
        let state: State = self.artifacts.load(&self.signer)?;
        if state.v != "openagents.workspace-host.v1"
            || state.binding["authority"] != self.signer.pubkey()
        {
            return Err("workspace state identity differs".into());
        }
        Ok(state)
    }
    pub(crate) fn save(&self, state: &State) -> Result<()> {
        self.artifacts.save(state, &self.signer)
    }
    pub fn binding(&self) -> Result<Value> {
        Ok(self.state()?.binding)
    }
    /// Organizational references stay in their existing project owners. This
    /// binding records association only and grants no project or file rights.
    pub fn associate_projects(&self, projects: Vec<Value>) -> Result<Value> {
        if projects.len() > 256 {
            return Err("project association bound exceeded".into());
        }
        for project in &projects {
            contracts::parse_artifact(project).map_err(|e| e.to_string())?;
        }
        let _guard = self.artifacts.lock()?;
        let mut state = self.state()?;
        state.binding["projects"] = json!(projects);
        state.binding["revision"] = json!(
            state.binding["revision"]
                .as_u64()
                .ok_or("binding revision")?
                .checked_add(1)
                .ok_or("binding revision exhausted")?
        );
        workspace::validate(&state.binding).map_err(|e| e.to_string())?;
        state.binding_ref = self.artifacts.put_json(&state.binding)?;
        self.save(&state)?;
        Ok(state.binding_ref)
    }
    fn make_grant(
        &self,
        actor: &str,
        epoch: u64,
        all: bool,
        resources: BTreeSet<String>,
        write: bool,
        approval_required: bool,
        expires_at: u64,
        revoked: bool,
    ) -> Result<Grant> {
        XOnlyPublicKey::from_str(actor).map_err(|_| "invalid actor key")?;
        let body = json!({"v":"openagents.ws-local-grant.v1","requires":[],"authority":self.signer.pubkey(),"actor":actor,"epoch":epoch,"all":all,"resources":resources,"write":write,"approval_required":approval_required,"expires_at":expires_at,"revoked":revoked});
        let reference = self.artifacts.put_json(&body)?;
        Ok(Grant {
            actor: actor.into(),
            epoch,
            all,
            resources,
            write,
            approval_required,
            expires_at,
            revoked,
            reference,
        })
    }
    /// A host-administration call: embedding transports must expose it only to
    /// the configured authority, never to a resource-change caller.
    pub fn grant(
        &self,
        actor: &str,
        resources: BTreeSet<String>,
        write: bool,
        approval_required: bool,
        expires_at: u64,
    ) -> Result<Value> {
        let _guard = self.artifacts.lock()?;
        let mut state = self.state()?;
        if resources.len() > 256 || resources.iter().any(|id| !state.resources.contains_key(id)) {
            return Err("grant names unavailable resources".into());
        }
        let epoch = state.grants.get(actor).map(|g| g.epoch + 1).unwrap_or(0);
        let grant = self.make_grant(
            actor,
            epoch,
            false,
            resources,
            write,
            approval_required,
            expires_at,
            false,
        )?;
        let reference = grant.reference.clone();
        state.grants.insert(actor.into(), grant);
        self.save(&state)?;
        Ok(reference)
    }
    pub fn revoke(&self, actor: &str) -> Result<()> {
        let _guard = self.artifacts.lock()?;
        let mut state = self.state()?;
        if actor == self.signer.pubkey() {
            return Err("cannot revoke the workspace authority".into());
        }
        let prior = state
            .grants
            .get(actor)
            .ok_or("actor is not enrolled")?
            .clone();
        let grant = self.make_grant(
            actor,
            prior.epoch + 1,
            false,
            prior.resources,
            false,
            true,
            prior.expires_at,
            true,
        )?;
        state.grants.insert(actor.into(), grant);
        self.save(&state)
    }
    pub(crate) fn check<'a>(
        &self,
        state: &'a State,
        actor: &str,
        resource: Option<&str>,
        write: bool,
        now: u64,
    ) -> Result<&'a Grant> {
        let grant = state
            .grants
            .get(actor)
            .filter(|g| !g.revoked && now < g.expires_at)
            .ok_or("not_admitted: workspace access is unavailable")?;
        if write && !grant.write
            || resource.is_some_and(|id| !grant.all && !grant.resources.contains(id))
        {
            return Err("not_admitted: resource access is unavailable".into());
        }
        Ok(grant)
    }
    pub fn register_git(&self, source: GitDocument, name: String, now: u64) -> Result<ResourceRef> {
        if name.len() > 1024 {
            return Err("resource name is too long".into());
        }
        let _guard = self.artifacts.lock()?;
        let mut state = self.state()?;
        if state.resources.len() >= 256 {
            return Err("resource bound exceeded".into());
        }
        let r = ResourceRef {
            authority: self.signer.pubkey().into(),
            workspace: state.binding["workspace"]
                .as_str()
                .ok_or("workspace ID")?
                .into(),
            resource: artifact::token(),
        };
        let adapter=self.definition("git-document",json!({"v":"openagents.ws-git-adapter.v1","requires":[],"encoding":"openagents.git-blob-utf8.v1","rename_identity":"new_resource","mutation":"git_update_ref_compare_and_swap","worktree_files":false}))?;
        let operation=self.definition("replace-git-document",json!({"v":"openagents.ws-git-replace-operation.v1","requires":[],"input_schema":"openagents.document-replacement.v1","effects":["git_ref_update"],"precondition":"exact_branch_commit","confirmation":"exact_commit_in_ref_or_retained_reflog","filesystem_mutation":false}))?;
        state.binding["roots"]
            .as_array_mut()
            .ok_or("roots")?
            .push(serde_json::to_value(&r).map_err(|e| e.to_string())?);
        state.binding["revision"] =
            json!(state.binding["revision"].as_u64().ok_or("revision")? + 1);
        state.binding["generation"] =
            json!(state.binding["generation"].as_u64().ok_or("generation")? + 1);
        state.binding_ref = self.artifacts.put_json(&state.binding)?;
        state.resources.insert(
            r.resource.clone(),
            Resource {
                reference: r.clone(),
                source,
                name,
                adapter,
                operation,
                descriptor: None,
            },
        );
        let resource = state.resources.get(&r.resource).ok_or("resource")?;
        self.capture(&state, resource, now)?;
        self.save(&state)?;
        Ok(r)
    }
    pub(crate) fn definition(&self, slug: &str, body: Value) -> Result<Value> {
        let artifact = self.artifacts.put_json(&body)?;
        Ok(json!({"id":format!("{}:workspace/{slug}",self.signer.pubkey()),"artifact":artifact}))
    }
    pub(crate) fn capture(
        &self,
        state: &State,
        resource: &Resource,
        now: u64,
    ) -> Result<(Value, Value)> {
        let capture = resource.source.capture()?;
        self.snapshot_from(state, resource, capture, now)
    }
    pub(crate) fn snapshot_from(
        &self,
        state: &State,
        resource: &Resource,
        capture: Capture,
        now: u64,
    ) -> Result<(Value, Value)> {
        let content = self.artifacts.put(&capture.bytes, "text/plain", None)?;
        let observation = json!({"v":"openagents.observation.v1","requires":[],"resource":{"scope":resource.reference.workspace,"id":resource.reference.resource},"adapter":resource.adapter,"captured_at":now,"provider_revision":capture.position,"content":content,"consistency":"conditional","valid_until":null});
        let observation_ref = self.artifacts.put_json(&observation)?;
        let snapshot = json!({"v":"openagents.document-snapshot.v1","requires":[],"resource":resource.reference,"binding":state.binding_ref,"observation":observation_ref,"content":content,"encoding":"openagents.git-blob-utf8.v1","derivation":null,"coverage":"complete","editable":true});
        workspace::verify_document(&snapshot, &observation, Some(&capture.bytes))
            .map_err(|e| e.to_string())?;
        let reference = self.artifacts.put_json(&snapshot)?;
        Ok((reference, snapshot))
    }
    /// Authenticate an actual private artifact. A caller-supplied actor string
    /// never establishes the principal.
    pub(crate) fn authenticated(
        &self,
        event: &Event,
        now: u64,
        schema: &str,
    ) -> Result<(String, Value)> {
        let envelope =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        if envelope.recipient() != self.signer.pubkey()
            || envelope.artifact().schema.as_deref() != Some(schema)
            || envelope.artifact().media_type != "application/json"
            || now > envelope.body().retain_until
            || envelope.body().issued_at > now.saturating_add(300)
        {
            return Err("not_admitted: invalid workspace request".into());
        }
        let bytes = envelope
            .inline_bytes()
            .ok_or("inline workspace request is required")?;
        let value = contracts::parse_strict(bytes).map_err(|e| e.to_string())?;
        Ok((envelope.signer().into(), value))
    }
    pub fn read_document(&self, event: &Event, now: u64) -> Result<Value> {
        let started = std::time::Instant::now();
        let (actor, request) = self.authenticated(event, now, "openagents.ws-resource-read.v1")?;
        closed_read(&request)?;
        let _guard = self.artifacts.lock()?;
        let state = self.state()?;
        let r = workspace::resource(&request["resource"]).map_err(|e| e.to_string())?;
        self.check(&state, &actor, Some(&r.resource), false, now)?;
        let resource = state
            .resources
            .get(&r.resource)
            .filter(|v| v.reference == r)
            .ok_or("not_admitted: resource unavailable")?;
        let (reference, snapshot) = self.capture(&state, resource, now)?;
        self.check(
            &state,
            &actor,
            Some(&r.resource),
            false,
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        Ok(
            json!({"snapshot":reference,"body":snapshot,"observation":self.artifacts.json(&snapshot["observation"])? ,"content_utf8":String::from_utf8(self.artifacts.get(&snapshot["content"])?).map_err(|_|"UTF-8 document")?,"authorization":state.grants[&actor].reference,"operation":resource.operation,"workbench":crate::bridge::artifact_view(self.signer.pubkey(),&snapshot)?}),
        )
    }
    /// Stage bounded attachments only after the caller's signed wrapper and
    /// the current grant admit its exact resource. Staging executes nothing.
    pub fn change(
        &self,
        event: &Event,
        attachments: &BTreeMap<String, Vec<u8>>,
        now: u64,
        approvals: Option<&dyn ApprovalVerifier>,
    ) -> Result<Value> {
        let (actor, admission) =
            self.authenticated(event, now, "openagents.workspace-action-admission.v1")?;
        self.change_admitted(actor, admission, attachments, now, approvals, None)
    }
    pub(crate) fn change_admitted(
        &self,
        actor: String,
        admission: Value,
        attachments: &BTreeMap<String, Vec<u8>>,
        now: u64,
        approvals: Option<&dyn ApprovalVerifier>,
        parent: Option<&Value>,
    ) -> Result<Value> {
        let started = std::time::Instant::now();
        workspace::validate(&admission).map_err(|e| e.to_string())?;
        if admission["actor"] != actor {
            return Err("not_admitted: action signer differs".into());
        }
        if attachments.len() > 64
            || attachments.values().map(Vec::len).sum::<usize>() > 4 * artifact::MAX_BYTES
        {
            return Err("action attachment bound exceeded".into());
        }
        let _guard = self.artifacts.lock()?;
        let mut state = self.state()?;
        let resolve = |reference: &Value| -> Result<Vec<u8>> {
            let parsed = contracts::parse_artifact(reference).map_err(|e| e.to_string())?;
            let bytes = if let Some(bytes) = attachments.get(&parsed.digest) {
                bytes.clone()
            } else {
                self.artifacts.get(reference)?
            };
            contracts::check_artifact_bytes(&parsed, &bytes).map_err(|e| e.to_string())?;
            Ok(bytes)
        };
        let proposal =
            workspace::parse(&resolve(&admission["proposal"])?).map_err(|e| e.to_string())?;
        if proposal["v"] != "openagents.resource-change.v1" || proposal["actor"] != actor {
            return Err("unsupported workspace action or signer".into());
        }
        let r = workspace::resource(&proposal["resource"]).map_err(|e| e.to_string())?;
        let grant = self
            .check(&state, &actor, Some(&r.resource), true, now)?
            .clone();
        let command = proposal["command"].as_str().ok_or("command")?;
        let key = format!("{actor}:{command}");
        let fingerprint =
            contracts::digest_bytes(&contracts::jcs(&admission).map_err(|e| e.to_string())?);
        if let Some(prior) = state.commands.get(&key) {
            if prior.fingerprint != fingerprint {
                return Err("idempotency_conflict: command bytes differ".into());
            }
            if let Some(result) = &prior.result {
                return Ok(result.clone());
            }
            return self.recover_locked(&mut state, &key, now);
        }
        if state.commands.len() >= 4096 {
            return Err("command retention bound exceeded; no keys were discarded".into());
        }
        if proposal["binding"] != state.binding_ref
            || proposal["authorization"] != grant.reference
            || r.authority != self.signer.pubkey()
            || r.workspace != state.binding["workspace"]
        {
            return Err("stale: resource binding or authority changed".into());
        }
        if now < proposal["issued_at"].as_u64().ok_or("issued_at")?
            || now >= proposal["expires_at"].as_u64().ok_or("expires_at")?
        {
            return Err("stale: workspace change expired".into());
        }
        let resource = state
            .resources
            .get(&r.resource)
            .filter(|v| v.reference == r)
            .ok_or("resource is unavailable")?
            .clone();
        if proposal["operation"] != resource.operation {
            return Err("unsupported workspace operation".into());
        }
        if let Some(parent) = parent {
            if proposal["binding"] != parent["binding"]
                || proposal["authorization"] != parent["authorization"]
                || !parent["expected"]
                    .as_array()
                    .is_some_and(|a| a.contains(&proposal["expected"]))
                || !parent["steps"].as_array().is_some_and(|steps| {
                    steps.iter().any(|s| {
                        s["operation"] == proposal["operation"] && s["input"] == proposal["input"]
                    })
                })
            {
                return Err("not_admitted: change is outside the approved restoration".into());
            }
        }
        if grant.approval_required && parent.is_none() {
            let verifier =
                approvals.ok_or("cannot_enforce: required approval verifier is unavailable")?;
            if admission["approval"].is_null() {
                return Err("not_admitted: exact action approval is required".into());
            }
            let decision = contracts::parse_strict(&resolve(&admission["approval"])?)
                .map_err(|e| e.to_string())?;
            verifier.approve(&actor, &admission["proposal"], &decision, now)?;
        }
        let replacement =
            workspace::parse(&resolve(&proposal["input"])?).map_err(|e| e.to_string())?;
        if replacement["v"] != "openagents.document-replacement.v1" {
            return Err("unsupported replacement profile".into());
        }
        let snapshot =
            workspace::parse(&resolve(&replacement["snapshot"])?).map_err(|e| e.to_string())?;
        let observation = contracts::parse_strict(&resolve(&snapshot["observation"])?)
            .map_err(|e| e.to_string())?;
        let old_bytes = resolve(&snapshot["content"])?;
        workspace::verify_document(&snapshot, &observation, Some(&old_bytes))
            .map_err(|e| e.to_string())?;
        if snapshot["editable"] != true
            || snapshot["resource"] != proposal["resource"]
            || snapshot["binding"] != proposal["binding"]
            || snapshot["observation"] != proposal["expected"]
            || snapshot["encoding"] != replacement["encoding"]
            || replacement["encoding"] != "openagents.git-blob-utf8.v1"
            || observation["adapter"] != resource.adapter
        {
            return Err("stale: replacement is not bound to the expected document".into());
        }
        let bytes = resolve(&replacement["content"])?;
        let expected = observation["provider_revision"]
            .as_str()
            .ok_or("conditional revision is unavailable")?;
        // Stage all needed immutable evidence before committing the admission.
        for value in [&admission, &proposal, &replacement, &snapshot, &observation] {
            self.artifacts.put_json(value)?;
        }
        self.artifacts.put(&old_bytes, "text/plain", None)?;
        self.artifacts.put(&bytes, "text/plain", None)?;
        if let Some(reference) = admission.get("approval").filter(|v| !v.is_null()) {
            self.artifacts.put_json(
                &contracts::parse_strict(&resolve(reference)?).map_err(|e| e.to_string())?,
            )?;
        }
        let entry = CommandState {
            fingerprint: fingerprint.clone(),
            actor: actor.clone(),
            resource: r.resource.clone(),
            request: artifact::json_ref(&admission)?,
            proposal: proposal.clone(),
            before: proposal["expected"].clone(),
            candidate: None,
            result: None,
            dispatched: false,
        };
        state.commands.insert(key.clone(), entry);
        self.save(&state)?;
        let candidate = match resource.source.prepare(expected, &bytes, &fingerprint) {
            Ok(commit) => commit,
            Err(error) => {
                return self.finish(
                    &mut state,
                    &key,
                    "refused",
                    None,
                    Some(if error.starts_with("stale:") {
                        "stale"
                    } else {
                        "cannot_enforce"
                    }),
                    now,
                );
            }
        };
        state.commands.get_mut(&key).ok_or("command")?.candidate = Some(candidate.clone());
        self.save(&state)?;
        let dispatch_now = now.saturating_add(started.elapsed().as_secs());
        if dispatch_now >= proposal["expires_at"].as_u64().ok_or("expires_at")?
            || self
                .check(&state, &actor, Some(&r.resource), true, dispatch_now)
                .is_err()
        {
            return self.finish(
                &mut state,
                &key,
                "refused",
                None,
                Some("stale"),
                dispatch_now,
            );
        }
        // Persist effect intent. A crash after this point remains unknown until
        // the source confirms the candidate; it never triggers blind dispatch.
        state.commands.get_mut(&key).ok_or("command")?.dispatched = true;
        self.save(&state)?;
        self.check(&state, &actor, Some(&r.resource), true, now)?;
        let _dispatch = resource.source.dispatch_checked(expected, &candidate, || {
            let actual_now = now.saturating_add(started.elapsed().as_secs());
            self.check(&state, &actor, Some(&r.resource), true, actual_now)?;
            if actual_now >= proposal["expires_at"].as_u64().ok_or("expires_at")? {
                return Err("stale: proposal expired before source commit".into());
            }
            Ok(())
        });
        #[cfg(test)]
        if self.fail_after_dispatch {
            return Err("fixture: lost response after source dispatch".into());
        }
        if resource.source.confirmed(&candidate).unwrap_or(false) {
            let after = self.confirmed_snapshot(&state, &resource, &candidate, &bytes, now)?;
            self.finish(
                &mut state,
                &key,
                "completed",
                Some(after["observation"].clone()),
                None,
                now,
            )
        } else {
            self.finish(&mut state, &key, "unknown", None, Some("conflict"), now)
        }
    }
    fn confirmed_snapshot(
        &self,
        state: &State,
        resource: &Resource,
        revision: &str,
        bytes: &[u8],
        now: u64,
    ) -> Result<Value> {
        let (_, snapshot) = self.snapshot_from(
            state,
            resource,
            Capture {
                revision: revision.into(),
                bytes: bytes.to_vec(),
                position: resource.source.position_for(revision)?,
            },
            now,
        )?;
        Ok(snapshot)
    }
    fn finish(
        &self,
        state: &mut State,
        key: &str,
        outcome: &str,
        after: Option<Value>,
        reason: Option<&str>,
        now: u64,
    ) -> Result<Value> {
        let command = state.commands.get(key).ok_or("command")?.clone();
        let resource = state.resources.get(&command.resource).ok_or("resource")?;
        let enforcement=self.artifacts.put_json(&json!({"v":"openagents.ws-cas-enforcement.v1","requires":[],"operation":resource.operation,"source":"git_update_ref","checked_at":now}))?;
        let receipt = json!({"v":"openagents.execution-receipt.v1","requires":[],"run":command.proposal["command"],"step":"workspace-change","attempt":1,"dispatched":command.dispatched,"component":resource.operation,"lock":contracts::digest_bytes(&contracts::jcs(&state.binding_ref).map_err(|e|e.to_string())?),"input":command.proposal["input"],"context":contracts::digest_bytes(&contracts::jcs(&command.before).map_err(|e|e.to_string())?),"authority":command.proposal["authorization"],"enforcement":enforcement,"recipient":command.actor,"usage":{"wall_ms":null,"spend_microunits":null},"output":after,"outcome":outcome,"verification":if outcome=="completed"{"passed"}else{"not_run"},"integration":if outcome=="completed"{"accepted"}else{"not_requested"}});
        contracts::parse_receipt(&receipt).map_err(|e| e.to_string())?;
        let receipt = self.artifacts.put_json(&receipt)?;
        let result = json!({"v":"openagents.resource-change-result.v1","requires":[],"request":command.request,"outcome":outcome,"dispatched":command.dispatched,"before":command.before,"after":after,"receipt":receipt,"verification":if outcome=="completed"{"passed"}else{"not_run"},"integration":if outcome=="completed"{"accepted"}else{"not_requested"},"reason":reason});
        workspace::validate(&result).map_err(|e| e.to_string())?;
        self.artifacts.put_json(&result)?;
        state.commands.get_mut(key).ok_or("command")?.result = Some(result.clone());
        self.save(state)?;
        Ok(result)
    }
    pub(crate) fn recover_locked(&self, state: &mut State, key: &str, now: u64) -> Result<Value> {
        let command = state.commands.get(key).ok_or("command")?.clone();
        let resource = state
            .resources
            .get(&command.resource)
            .ok_or("resource")?
            .clone();
        if command.dispatched
            && let Some(candidate) = &command.candidate
            && resource.source.confirmed(candidate).unwrap_or(false)
        {
            let replacement = self.artifacts.json(&command.proposal["input"])?;
            let bytes = self.artifacts.get(&replacement["content"])?;
            let after = self.confirmed_snapshot(state, &resource, candidate, &bytes, now)?;
            return self.finish(
                state,
                key,
                "completed",
                Some(after["observation"].clone()),
                None,
                now,
            );
        }
        self.finish(
            state,
            key,
            if command.dispatched {
                "unknown"
            } else {
                "refused"
            },
            None,
            Some("unavailable"),
            now,
        )
    }
    pub fn descriptor(&self, event: &Event, now: u64) -> Result<Value> {
        let started = std::time::Instant::now();
        let (actor, request) = self.authenticated(event, now, "openagents.ws-resource-read.v1")?;
        closed_read(&request)?;
        let _guard = self.artifacts.lock()?;
        let state = self.state()?;
        let r = workspace::resource(&request["resource"]).map_err(|e| e.to_string())?;
        self.check(&state, &actor, Some(&r.resource), false, now)?;
        let resource = state
            .resources
            .get(&r.resource)
            .filter(|v| v.reference == r)
            .ok_or("resource unavailable")?;
        let (_, snapshot) = self.capture(&state, resource, now)?;
        self.check(
            &state,
            &actor,
            Some(&r.resource),
            false,
            now.saturating_add(started.elapsed().as_secs()),
        )?;
        let descriptor = json!({"v":"openagents.workspace-resource.v1","requires":[],"ref":r,"binding":state.binding_ref,"kind":"document","parent":null,"name":resource.name,"adapter":resource.adapter,"operations":[resource.operation],"observation":snapshot["observation"],"lifecycle":"available"});
        workspace::validate(&descriptor).map_err(|e| e.to_string())?;
        Ok(descriptor)
    }
}
fn closed_read(value: &Value) -> Result<()> {
    let o = value.as_object().ok_or("resource read object")?;
    if o.len() != 3
        || o.keys()
            .any(|k| !matches!(k.as_str(), "v" | "requires" | "resource"))
        || value["v"] != "openagents.ws-resource-read.v1"
        || value["requires"] != json!([])
    {
        return Err("unsupported resource read".into());
    }
    Ok(())
}
/// Build a private request with caller-owned entropy, never an ambient key.
pub fn request(secret: &SecretKey, host: &str, value: Value, now: u64) -> Result<Event> {
    let bytes = contracts::jcs(&value).map_err(|e| e.to_string())?;
    let reference = artifact::reference(&bytes, "application/json", value["v"].as_str());
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":reference,"inline":value,"issued_at":now,"retain_until":now+3600});
    nostr::private_artifact::seal(
        &body,
        secret,
        &XOnlyPublicKey::from_str(host).map_err(|_| "invalid host key")?,
        &artifact::token(),
        now,
        secp256k1::rand::random(),
    )
    .map_err(|e| e.to_string())
}
