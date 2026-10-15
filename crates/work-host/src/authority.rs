//! Work mutations serialize against current rights, policy, and revisions.
use crate::Result;
use nostr::{
    contracts,
    domain::{Event, RelaySigner},
    work::{self, WorkRef},
};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    str::FromStr,
};
use workspace_host::{
    artifact::{self, Artifacts},
    projection::Projection,
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub actor: String,
    pub roles: BTreeSet<String>,
    pub all: bool,
    pub items: BTreeSet<String>,
    pub until: u64,
    pub epoch: u64,
    pub reference: Value,
    pub revoked: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Item {
    pub history: Vec<Value>,
    pub frozen: bool,
    pub forks: Vec<Value>,
    pub artifacts: BTreeSet<String>,
    pub commercial: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Command {
    pub fingerprint: String,
    pub issuer: String,
    pub work: WorkRef,
    pub command: Value,
    pub result: Option<Value>,
    pub run: Vec<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Scope {
    pub descriptor: Value,
    pub reference: Value,
    pub policy: Value,
    pub policy_ref: Value,
    pub epoch: u64,
    pub revoked: bool,
    pub grants: BTreeMap<String, Grant>,
    pub used_items: BTreeSet<String>,
    pub items: BTreeMap<String, Item>,
    pub commands: BTreeMap<String, Command>,
    pub imports: BTreeMap<String, String>,
    pub executions: BTreeMap<String, crate::execution::ExecutionGrant>,
    pub jobs: BTreeMap<String, crate::execution::Job>,
    pub attention: BTreeMap<String, Value>,
    pub dismissed: BTreeSet<String>,
    pub views: BTreeMap<String, Projection>,
    pub stale_views: BTreeSet<String>,
    pub attention_overflow: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub v: String,
    pub authority: String,
    pub scopes: BTreeMap<String, Scope>,
    pub declarations: BTreeMap<String, Vec<Event>>,
    pub frames: BTreeMap<String, String>,
}
pub struct Authority {
    pub(crate) market: Option<Box<dyn crate::market::Market>>,
    pub(crate) store: Artifacts,
    pub(crate) signer: RelaySigner,
    pub(crate) secret: SecretKey,
    #[cfg(test)]
    pub(crate) crash_after_admission: bool,
}
pub const ROLES: &[&str] = &[
    "read",
    "create",
    "revise",
    "assign",
    "requirements",
    "evidence",
    "dispose",
    "integrate",
];
impl Authority {
    pub fn initialize(root: PathBuf, secret: SecretKey) -> Result<Self> {
        let authority = Self::open(root, secret)?;
        let _lock = authority.store.lock()?;
        if authority.store.root().join("state.json").exists() {
            return Err("work authority already exists".into());
        }
        authority.save(&State {
            v: "openagents.work-host.v1".into(),
            authority: authority.signer.pubkey().into(),
            scopes: BTreeMap::new(),
            declarations: BTreeMap::new(),
            frames: BTreeMap::new(),
        })?;
        drop(_lock);
        Ok(authority)
    }
    pub fn open(root: PathBuf, secret: SecretKey) -> Result<Self> {
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|_| "invalid work authority key")?;
        Ok(Self {
            market: None,
            store: Artifacts::new(root)?,
            signer,
            secret,
            #[cfg(test)]
            crash_after_admission: false,
        })
    }
    pub fn with_market(mut self, market: Box<dyn crate::market::Market>) -> Self {
        self.market = Some(market);
        self
    }
    pub(crate) fn state(&self) -> Result<State> {
        let state: State = self.store.load(&self.signer)?;
        if state.v != "openagents.work-host.v1" || state.authority != self.signer.pubkey() {
            return Err("work authority identity or version differs".into());
        }
        Ok(state)
    }
    pub(crate) fn save(&self, state: &State) -> Result<()> {
        self.store.save(state, &self.signer)
    }
    /// Native host administration, explicitly creating a fresh local scope.
    /// Owner and authority are the local key in this default profile.
    pub fn create_scope(&self, verifiers: BTreeSet<String>, now: u64) -> Result<Value> {
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        if state.scopes.len() >= 16 || verifiers.len() > 64 {
            return Err("work scope or verifier bound exceeded".into());
        }
        for key in &verifiers {
            XOnlyPublicKey::from_str(key).map_err(|_| "invalid verifier key")?;
        }
        let scope_id = artifact::token();
        let policy = json!({"v":"openagents.work-native-policy.v1","requires":[],"authority":self.signer.pubkey(),"owner":self.signer.pubkey(),"max_items":256,"max_history":128,"imports":"one_time_copy","external_writes":false,"verifiers":verifiers,"execution":{"effects":[],"calls":64,"wall_ms":60000,"output_bytes":1048576,"spend_msat":0}});
        let policy_ref = self.store.put_json(&policy)?;
        let descriptor = json!({"v":"openagents.work-scope.v1","requires":[],"issuer":self.signer.pubkey(),"owner":self.signer.pubkey(),"scope":scope_id,"authority":self.signer.pubkey(),"policy":policy_ref,"resources":null,"created_at":now});
        work::validate(&descriptor).map_err(|e| e.to_string())?;
        let reference = self.declare(&mut state, &descriptor, self.signer.pubkey(), now)?;
        let grant = self.make_grant(
            self.signer.pubkey(),
            ROLES.iter().map(|s| s.to_string()).collect(),
            true,
            BTreeSet::new(),
            9_007_199_254_740_991,
            0,
            false,
        )?;
        state.scopes.insert(
            scope_id,
            Scope {
                descriptor,
                reference: reference.clone(),
                policy,
                policy_ref,
                epoch: 0,
                revoked: false,
                grants: BTreeMap::from([(self.signer.pubkey().into(), grant)]),
                used_items: BTreeSet::new(),
                items: BTreeMap::new(),
                commands: BTreeMap::new(),
                imports: BTreeMap::new(),
                executions: BTreeMap::new(),
                jobs: BTreeMap::new(),
                attention: BTreeMap::new(),
                dismissed: BTreeSet::new(),
                views: BTreeMap::new(),
                stale_views: BTreeSet::new(),
                attention_overflow: false,
            },
        );
        self.save(&state)?;
        Ok(reference)
    }
    fn make_grant(
        &self,
        actor: &str,
        roles: BTreeSet<String>,
        all: bool,
        items: BTreeSet<String>,
        until: u64,
        epoch: u64,
        revoked: bool,
    ) -> Result<Grant> {
        XOnlyPublicKey::from_str(actor).map_err(|_| "invalid work principal")?;
        if roles.iter().any(|r| !ROLES.contains(&r.as_str())) || items.len() > 256 {
            return Err("unsupported work grant".into());
        }
        let reference=self.store.put_json(&json!({"v":"openagents.work-native-grant.v1","requires":[],"authority":self.signer.pubkey(),"actor":actor,"roles":roles,"all":all,"items":items,"until":until,"epoch":epoch,"revoked":revoked}))?;
        Ok(Grant {
            actor: actor.into(),
            roles,
            all,
            items,
            until,
            epoch,
            reference,
            revoked,
        })
    }
    /// Only the scope owner's independently authenticated control can change
    /// grants. A work item or revision can never invoke this path.
    pub fn control(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.work-native-control.v1"))?;
        closed(
            &body,
            &[
                "v",
                "requires",
                "issuer",
                "scope",
                "actor",
                "roles",
                "all",
                "items",
                "until",
                "revoke_scope",
            ],
        )?;
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let key = body["scope"]["digest"]
            .as_str()
            .ok_or("scope reference")?
            .to_string();
        let scope = state
            .scopes
            .values_mut()
            .find(|s| s.reference["digest"] == key)
            .ok_or("not_admitted: work scope unavailable")?;
        if scope.reference != body["scope"] || scope.descriptor["owner"] != actor {
            return Err("not_admitted: scope owner control required".into());
        }
        if body["revoke_scope"] == true {
            scope.revoked = true;
            scope.epoch = scope.epoch.checked_add(1).ok_or("scope epoch exhausted")?;
        } else {
            let principal = body["actor"].as_str().ok_or("grant actor")?;
            let roles: BTreeSet<String> =
                serde_json::from_value(body["roles"].clone()).map_err(|_| "grant roles")?;
            let items: BTreeSet<String> = serde_json::from_value(body["items"].clone())
                .map_err(|_| "grant item selection")?;
            if items.iter().any(|id| !scope.items.contains_key(id)) {
                return Err("grant references unavailable work".into());
            }
            let all = body["all"].as_bool().ok_or("grant all flag")?;
            let epoch = scope
                .grants
                .get(principal)
                .map(|g| g.epoch + 1)
                .unwrap_or(0);
            let grant = self.make_grant(
                principal,
                roles,
                all,
                items,
                body["until"].as_u64().ok_or("grant expiry")?,
                epoch,
                false,
            )?;
            scope.grants.insert(principal.into(), grant);
        }
        let reference = self.store.put_json(&body)?;
        self.retain_declaration(&mut state, event, &body)?;
        self.save(&state)?;
        Ok(reference)
    }
    pub(crate) fn check<'a>(
        &self,
        scope: &'a Scope,
        actor: &str,
        item: Option<&str>,
        role: &str,
        now: u64,
    ) -> Result<&'a Grant> {
        if scope.revoked {
            return Err("revoked: work scope is unavailable".into());
        }
        let grant = scope
            .grants
            .get(actor)
            .filter(|g| !g.revoked && now < g.until && g.roles.contains(role))
            .ok_or("not_admitted: work access is unavailable")?;
        if item.is_some_and(|item| !grant.all && !grant.items.contains(item)) {
            return Err("not_admitted: work access is unavailable".into());
        }
        Ok(grant)
    }
    pub(crate) fn authenticate(
        &self,
        event: &Event,
        now: u64,
        schema: Option<&str>,
    ) -> Result<(String, Value)> {
        let envelope =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        if envelope.recipient() != self.signer.pubkey()
            || envelope.artifact().media_type != "application/json"
            || schema.is_some_and(|s| envelope.artifact().schema.as_deref() != Some(s))
            || envelope.body().issued_at > now.saturating_add(300)
            || now > envelope.body().retain_until
        {
            return Err("not_admitted: invalid private work declaration".into());
        }
        let body = contracts::parse_strict(
            envelope
                .inline_bytes()
                .ok_or("inline work declaration required")?,
        )
        .map_err(|e| e.to_string())?;
        if body
            .get("issuer")
            .is_some_and(|issuer| *issuer != envelope.signer())
        {
            return Err("not_admitted: work issuer differs from signer".into());
        }
        Ok((envelope.signer().into(), body))
    }
    pub(crate) fn retain_declaration(
        &self,
        state: &mut State,
        event: &Event,
        body: &Value,
    ) -> Result<Value> {
        let reference = self.store.put_json(body)?;
        let key = reference["digest"]
            .as_str()
            .ok_or("declaration digest")?
            .to_string();
        let declarations = state.declarations.entry(key).or_default();
        if !declarations.iter().any(|e| e.id == event.id) {
            if declarations.len() >= 64 {
                return Err("declaration audience bound exceeded".into());
            }
            declarations.push(event.clone());
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
        if body
            .get("issuer")
            .is_some_and(|issuer| *issuer != self.signer.pubkey())
        {
            return Err("cannot sign another work author's declaration".into());
        }
        let event = workspace_host::host::request(&self.secret, recipient, body.clone(), now)?;
        self.retain_declaration(state, &event, body)
    }
    pub fn submit_declaration(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) = self.authenticate(event, now, None)?;
        work::validate(&body).map_err(|e| e.to_string())?;
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let w = work::work_ref(&body["work"]).map_err(|e| e.to_string())?;
        let scope = state
            .scopes
            .get(&w.scope)
            .filter(|s| s.descriptor["authority"] == w.authority)
            .ok_or("not_admitted: work scope unavailable")?;
        let role = match body["v"].as_str() {
            Some("openagents.work-delegation.v1") => "assign",
            Some("openagents.work-disposition.v1") => "dispose",
            Some("openagents.work-evidence.v1") => "evidence",
            _ => return Err("unsupported independent work declaration".into()),
        };
        self.check(scope, &actor, Some(&w.item), role, now)?;
        let reference = self.retain_declaration(&mut state, event, &body)?;
        self.save(&state)?;
        Ok(reference)
    }
    pub(crate) fn original(&self, state: &State, reference: &Value) -> Result<(String, Value)> {
        let body = self.store.json(reference)?;
        let issuer = body["issuer"].as_str().ok_or("record issuer")?;
        let list = state
            .declarations
            .get(reference["digest"].as_str().ok_or("record digest")?)
            .ok_or("original work declaration is unavailable")?;
        for event in list {
            if event.pubkey != issuer {
                continue;
            }
            if let Ok((actor, value)) = self.authenticate(event, event.created_at, None)
                && value == body
            {
                return Ok((actor, body));
            }
        }
        Err("original work declaration could not be authenticated".into())
    }
}
pub(crate) fn closed(value: &Value, fields: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or("closed work object required")?;
    if object.len() != fields.len()
        || object.keys().any(|k| !fields.contains(&k.as_str()))
        || value["requires"] != json!([])
    {
        return Err("unsupported work fields".into());
    }
    Ok(())
}

impl Authority {
    pub fn command(
        &self,
        event: &Event,
        attachments: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Value> {
        let (actor, command) = self.authenticate(event, now, Some("openagents.work-command.v1"))?;
        work::validate(&command).map_err(|e| e.to_string())?;
        if attachments.len() > 128
            || attachments.values().map(Vec::len).sum::<usize>()
                > 8 * workspace_host::artifact::MAX_BYTES
        {
            return Err("work input closure exceeds its bound".into());
        }
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let w = work::work_ref(&command["work"]).map_err(|e| e.to_string())?;
        let mut scope = state
            .scopes
            .get(&w.scope)
            .filter(|s| s.descriptor["authority"] == w.authority && s.reference == command["scope"])
            .ok_or("not_admitted: work scope unavailable")?
            .clone();
        self.check(&scope, &actor, Some(&w.item), "read", now)?;
        let id = command["command_id"].as_str().ok_or("command ID")?;
        let key = format!("{actor}:{id}");
        let fingerprint =
            contracts::digest_bytes(&contracts::jcs(&command).map_err(|e| e.to_string())?);
        if let Some(prior) = scope.commands.get(&key) {
            if prior.fingerprint != fingerprint {
                return self.refusal(&mut state, &scope, &command, "conflict", "conflict", now);
            }
            if let Some(result) = &prior.result {
                return Ok(result.clone());
            }
            return self.refusal(&mut state, &scope, &command, "unknown", "unavailable", now);
        }
        if scope.commands.len() >= 4096 {
            return Err(
                "work command retention bound exceeded; no identities were discarded".into(),
            );
        }
        if now >= command["expires_at"].as_u64().ok_or("command deadline")? {
            return self.refusal(&mut state, &scope, &command, "refused", "stale", now);
        }
        if command["action"] == "create" {
            self.check(&scope, &actor, None, "create", now)?;
        } else {
            self.check(&scope, &actor, Some(&w.item), "read", now)?;
        }
        // Staged bytes remain inert and private. Only the exact validated
        // snapshot and its references can enter the accepted item history.
        for (digest, bytes) in attachments {
            if bytes.len() > workspace_host::artifact::MAX_BYTES
                || contracts::digest_bytes(bytes) != *digest
            {
                return Err("work attachment identity differs".into());
            }
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        let closure =
            self.admit_artifacts(&state, &scope, &w, &actor, &command, attachments, now)?;
        let item = self.snapshot(&command["snapshot"])?;
        let prior = self.head(&scope, &w.item)?;
        if scope.items.get(&w.item).is_some_and(|i| i.frozen) {
            return self.refusal(&mut state, &scope, &command, "conflict", "conflict", now);
        }
        if command["action"] == "create" {
            if prior.is_some() || scope.used_items.contains(&w.item) {
                return self.refusal(&mut state, &scope, &command, "conflict", "conflict", now);
            }
            if scope.used_items.len() >= 256 {
                return Err("work item bound exceeded".into());
            }
        } else if prior
            .as_ref()
            .is_none_or(|(reference, _, _)| *reference != command["expected"])
        {
            return self.refusal(&mut state, &scope, &command, "conflict", "stale", now);
        }
        if scope
            .items
            .get(&w.item)
            .is_some_and(|i| i.history.len() >= 128)
        {
            return Err("work revision retention limit exceeded".into());
        }
        self.changed_rights(&scope, &actor, &w, prior.as_ref().map(|p| &p.2), &item, now)?;
        self.validate_frame(&mut state, &scope, &item)?;
        self.relations(&state, &scope, &w, &item, &actor, now)?;
        self.check_delegation(
            &state,
            &scope,
            &w,
            prior.as_ref().map(|p| &p.2),
            &item,
            prior.as_ref().map(|p| &p.0),
            now,
        )?;
        self.evidence(
            &state,
            &scope,
            &w,
            &item,
            prior.as_ref().map(|p| &p.2),
            &actor,
            now,
        )?;
        self.disposition(
            &state,
            &scope,
            &w,
            prior.as_ref().map(|p| (&p.0, &p.2)),
            &item,
            &actor,
            now,
        )?;
        self.import_identity(
            &state,
            &mut scope,
            &w,
            &command,
            prior.as_ref().map(|p| &p.0),
            &item,
            &actor,
            now,
        )?;
        let command_ref = self.retain_declaration(&mut state, event, &command)?;
        let grant = self
            .check(&scope, &actor, Some(&w.item), "read", now)?
            .reference
            .clone();
        let operation=self.store.put_json(&json!({"v":"openagents.work-native-operation.v1","requires":[],"kind":"conditional_work_revision","external_effects":[]}))?;
        let mut record = Command {
            fingerprint,
            issuer: actor.clone(),
            work: w.clone(),
            command: command_ref.clone(),
            result: None,
            run: Vec::new(),
        };
        let run_id = contracts::digest_bytes(format!("{}:{actor}:{id}", w.scope).as_bytes())[7..]
            .to_string();
        self.run_record(&mut record,&run_id,"created",json!({"owner":scope.descriptor["owner"],"request":command_ref,"base":command["expected"],"program":operation,"lock":contracts::digest_bytes(&contracts::jcs(&command).map_err(|e|e.to_string())?),"context":command["snapshot"],"policy":scope.policy_ref,"parent":null,"recipients":[actor]}),now)?;
        let admission=self.run_record(&mut record,&run_id,"admitted",json!({"enforcement":{"policy":scope.policy_ref,"grant":grant,"conditional":command["expected"]},"reservations":[],"deadlines":{"expires_at":command["expires_at"]},"retention":{"command_identity":"retained"},"input":command_ref}),now)?;
        scope.used_items.insert(w.item.clone());
        scope.commands.insert(key.clone(), record);
        state.scopes.insert(w.scope.clone(), scope.clone());
        self.save(&state)?;
        #[cfg(test)]
        if self.crash_after_admission {
            return Err("fixture: stopped after durable work admission".into());
        }
        let revision_number = prior
            .as_ref()
            .map(|p| p.0["revision"].as_u64().unwrap_or(0) + 1)
            .unwrap_or(1);
        let revision = json!({"v":"openagents.work-revision.v1","requires":[],"issuer":self.signer.pubkey(),"scope":scope.reference,"work":w,"revision":revision_number,"previous":prior.as_ref().map(|p|p.0.clone()),"command":command_ref,"snapshot":command["snapshot"],"admission":admission,"recorded_at":now});
        work::validate(&revision).map_err(|e| e.to_string())?;
        let revision_artifact = self.declare(&mut state, &revision, &actor, now)?;
        let reference = json!({"work":w,"revision":revision_number,"artifact":revision_artifact});
        let history = scope.items.entry(w.item.clone()).or_insert_with(|| Item {
            history: Vec::new(),
            frozen: false,
            forks: Vec::new(),
            artifacts: BTreeSet::new(),
            commercial: false,
        });
        if history.history.len() >= 128 {
            return Err("work revision retention bound exceeded".into());
        }
        history.history.push(reference.clone());
        history.artifacts.extend(closure);
        history.commercial |=
            item["acceptance"]["schema"] == "openagents.labor-acceptance-policy.v1";
        self.reassign(&mut scope, prior.as_ref().map(|p| &p.2), &item)?;
        let record = scope.commands.get_mut(&key).ok_or("work command")?;
        let result_record=self.run_record(record,&run_id,"resolved",json!({"outcome":"completed","dispatched":true,"output":reference["artifact"],"receipts":[],"usage":{"wall_ms":null,"spend_microunits":0},"verification":"passed","integration":"accepted"}),now)?;
        let result = json!({"v":"openagents.work-operation-result.v1","requires":[],"issuer":self.signer.pubkey(),"command":command_ref,"status":"accepted","revision":reference,"record":result_record,"reason":null});
        work::validate(&result).map_err(|e| e.to_string())?;
        record.result = Some(result.clone());
        self.declare(&mut state, &result, &actor, now)?;
        self.update_attention(
            &mut scope,
            &w,
            prior.as_ref().map(|p| &p.2),
            &item,
            &reference,
            &result_record,
            now,
        )?;
        let retained = scope.commands.get(&key).ok_or("command")?;
        let mut evidence_refs = vec![
            command_ref.clone(),
            reference["artifact"].clone(),
            workspace_host::artifact::json_ref(&result)?,
        ];
        for body in &retained.run {
            evidence_refs.push(workspace_host::artifact::json_ref(body)?);
        }
        let history = scope.items.get_mut(&w.item).ok_or("item history")?;
        for reference in evidence_refs {
            history.artifacts.insert(
                reference["digest"]
                    .as_str()
                    .ok_or("evidence digest")?
                    .into(),
            );
        }
        self.advance_views(&mut scope, &w, &reference, now)?;
        state.scopes.insert(w.scope, scope);
        self.save(&state)?;
        Ok(result)
    }
    pub(crate) fn run_record(
        &self,
        command: &mut Command,
        run: &str,
        kind: &str,
        data: Value,
        now: u64,
    ) -> Result<Value> {
        let previous = command.run.last().map(|body| body["digest"].clone());
        let record = json!({"run":run,"seq":command.run.len(),"previous":previous,"controller":self.signer.pubkey(),"generation":0,"type":kind,"subject":{"step":null,"iteration":null,"attempt":null},"time":now,"data":data});
        let digest = nostr::run::logical_digest(&record).map_err(|e| e.to_string())?;
        let body =
            json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"record":record,"digest":digest});
        let mut journal = nostr::run::Journal::default();
        for body in command.run.iter().chain(std::iter::once(&body)) {
            let record = nostr::run::parse_envelope(body).map_err(|e| e.to_string())?;
            if nostr::run::ingest(&mut journal, record, self.signer.pubkey())
                .map_err(|e| e.to_string())?
                != nostr::run::Ingest::Applied
            {
                return Err("work RUN admission did not extend its exact history".into());
            }
        }
        let reference = self.store.put_json(&body)?;
        command.run.push(body);
        Ok(reference)
    }
    fn refusal(
        &self,
        state: &mut State,
        scope: &Scope,
        command: &Value,
        status: &str,
        reason: &str,
        now: u64,
    ) -> Result<Value> {
        let reference = self.store.put_json(command)?;
        let w = work::work_ref(&command["work"]).map_err(|e| e.to_string())?;
        let issuer = command["issuer"].as_str().ok_or("command issuer")?;
        let fingerprint =
            contracts::digest_bytes(&contracts::jcs(command).map_err(|e| e.to_string())?);
        let key = format!(
            "{issuer}:{}",
            command["command_id"].as_str().ok_or("command ID")?
        );
        let existing = scope.commands.get(&key);
        let same = existing.is_some_and(|c| c.fingerprint == fingerprint);
        let mut run = if same {
            existing.cloned().ok_or("prior command")?
        } else {
            Command {
                fingerprint,
                issuer: issuer.into(),
                work: w.clone(),
                command: reference.clone(),
                result: None,
                run: Vec::new(),
            }
        };
        let id = run
            .run
            .first()
            .and_then(|r| r["record"]["run"].as_str())
            .map(str::to_string)
            .unwrap_or_else(artifact::token);
        let dispatched = !run.run.is_empty() && status == "unknown";
        if run.run.is_empty() {
            self.run_record(&mut run,&id,"created",json!({"owner":scope.descriptor["owner"],"request":reference,"base":command["expected"],"program":scope.policy_ref,"lock":contracts::digest_bytes(&contracts::jcs(command).map_err(|e|e.to_string())?),"context":command["snapshot"],"policy":scope.policy_ref,"parent":null,"recipients":[issuer]}),now)?;
        }
        let record=self.run_record(&mut run,&id,"resolved",json!({"outcome":if status=="unknown"{"unknown"}else{"refused"},"dispatched":dispatched,"output":null,"receipts":[],"usage":{"wall_ms":null,"spend_microunits":0},"verification":"not_run","integration":"not_requested"}),now)?;
        let revision = if status == "unknown" {
            None
        } else {
            self.head(scope, &w.item)?.map(|p| p.0)
        };
        let result = json!({"v":"openagents.work-operation-result.v1","requires":[],"issuer":self.signer.pubkey(),"command":reference,"status":status,"revision":revision,"record":record,"reason":reason});
        work::validate(&result).map_err(|e| e.to_string())?;
        self.declare(state, &result, issuer, now)?;
        if existing.is_none() || same {
            run.result = Some(result.clone());
            let mut updated = scope.clone();
            updated.commands.insert(key, run);
            state.scopes.insert(w.scope, updated);
        }
        self.save(state)?;
        Ok(result)
    }
}
