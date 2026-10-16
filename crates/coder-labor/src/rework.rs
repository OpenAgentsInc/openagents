//! Native repair attempts through the existing protected task owner.
//! An accepted market, execution grant, and current authority are distinct.
use crate::{Result, agreement::Agreement, artifact_value, execution::prepare_frozen};
use coder::task::{self, Action, Command, Task};
use labor_roles::{records::Record, wire::Bytes, wire_host};
use nostr::{
    contracts::{self, Assurance, Bound},
    domain::{Event, RelaySigner},
    execution, private_artifact,
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use workspace_host::artifact::{Artifacts, Lock};
mod delivery;
pub use delivery::{Publication, verify};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub execute: Event,
    pub linkage: Value,
    pub context: Value,
    pub local_task: String,
    pub grant_digest: String,
    pub wall_ms: u64,
    pub output_bytes: u64,
    pub observation: Option<Task>,
    pub outcome: String,
    pub submission: Option<Value>,
    pub run_evidence: Vec<Value>,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    config: Value,
    attempts: Vec<Attempt>,
    context_hash: Option<String>,
}
pub struct Worker {
    store: Artifacts,
    signer: RelaySigner,
    secret: SecretKey,
    roles: wire_host::Host,
    state: State,
    _owner: Lock,
}
fn order_value(a: &Agreement) -> Result<Value> {
    let o = a.order().ok_or("the market order is not confirmed")?;
    Ok(
        json!({"market":o.market,"order_id":o.order_id,"buyer":o.buyer,"provider":o.provider,
        "order":artifact_value(&o.order),"confirmation":artifact_value(&o.confirmation)}),
    )
}
fn read(reference: &Value, bytes: &Bytes, a: &Agreement) -> Result<Vec<u8>> {
    let r = contracts::parse_artifact(reference).map_err(|e| e.to_string())?;
    let raw = if let Some(raw) = bytes.get(&r.digest) {
        raw.clone()
    } else {
        contracts::jcs(a.blobs().resolve(&r)?).map_err(|e| e.to_string())?
    };
    contracts::check_artifact_bytes(&r, &raw).map_err(|e| e.to_string())?;
    Ok(raw)
}
impl Worker {
    pub fn open(root: PathBuf, secret: SecretKey, a: &Agreement, now: u64) -> Result<Self> {
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        if signer.pubkey() != a.market().worker {
            return Err("this key is not the admitted worker".into());
        }
        let store = Artifacts::new(root)?;
        let owner = store.lock()?;
        let config =
            json!({"order":order_value(a)?,"admission":a.admission(),"receiver":a.receiver()});
        let marker = store.root().join("worker.initialized");
        if marker.exists() && !store.root().join("state.json").exists() {
            return Err("the initialized execution journal is missing".into());
        }
        let state: State = if store.root().join("state.json").exists() {
            store.load(&signer)?
        } else {
            State {
                config: config.clone(),
                ..Default::default()
            }
        };
        if state.config != config {
            return Err("the execution journal belongs to another frozen agreement".into());
        }
        let (terms, wire) = a.wire_admission(now)?;
        let roles = wire_host::Host::open(store.root().join("roles"), secret, terms, wire, now)?;
        let worker = Self {
            store,
            signer,
            secret,
            roles,
            state,
            _owner: owner,
        };
        worker.store.save(&worker.state, &worker.signer)?;
        workspace_host::artifact::atomic(&marker, b"openagents.labor-native-worker.v1")?;
        Ok(worker)
    }
    pub fn roles(&self) -> &wire_host::Host {
        &self.roles
    }
    pub fn roles_mut(&mut self) -> &mut wire_host::Host {
        &mut self.roles
    }
    pub fn attempts(&self) -> &[Attempt] {
        &self.state.attempts
    }
    fn save(&self) -> Result<()> {
        self.store.save(&self.state, &self.signer)
    }
    fn bound(a: &Agreement, bound: Bound) -> Result<u64> {
        a.labor()
            .execution
            .bounds
            .iter()
            .find(|b| b.bound == bound && b.assurance == Assurance::Host)
            .map(|b| b.ceiling)
            .ok_or("the native worker cannot enforce an unspecified whole-order bound".into())
    }
    fn known_terminal(task: &Task) -> bool {
        matches!(
            task.execution,
            task::Execution::Finished | task::Execution::Failed | task::Execution::Stopped
        ) && task
            .run
            .as_ref()
            .and_then(|r| r.result.as_ref())
            .is_some_and(|r| r.group_clear)
    }
    /// Observe one exact local task. Missing or not-started work stays unknown;
    /// no recovery path calls execute or creates a replacement task.
    pub fn reconcile(&mut self, attempt: u32) -> Result<Attempt> {
        let index = attempt.checked_sub(1).ok_or("invalid attempt")? as usize;
        let prior = self
            .state
            .attempts
            .get_mut(index)
            .ok_or("unknown attempt")?;
        let directory = self.store.root().join("tasks");
        match task::owner::recover(&directory, &prior.local_task) {
            Ok(task) => {
                if task
                    .run
                    .as_ref()
                    .is_some_and(|r| r.admission.grant_digest != prior.grant_digest)
                {
                    return Err("the recovered task has a different grant".into());
                }
                prior.outcome = if Self::known_terminal(&task) {
                    format!("{:?}", task.execution).to_ascii_lowercase()
                } else {
                    "unknown".into()
                };
                prior.observation = Some(task);
            }
            Err(task::Error::NotFound) => {
                prior.outcome = "unknown".into();
                prior.observation = None;
            }
            Err(e) => return Err(e.to_string()),
        }
        self.save()?;
        Ok(self.state.attempts[index].clone())
    }
    /// The host polls original control records and current local authority
    /// while the protected owner runs. Returning an error requests cancellation;
    /// neither cancellation nor an unknown result releases a later attempt.
    pub async fn dispatch(
        &mut self,
        a: &mut Agreement,
        link_event: &Event,
        execute_event: &Event,
        artifacts: &Bytes,
        grant_bytes: &[u8],
        now: u64,
        poll: &mut dyn FnMut(&mut Agreement, &mut wire_host::Host, u64) -> Result<()>,
    ) -> Result<Attempt> {
        let received = Instant::now();
        if json!({"order":order_value(a)?,"admission":a.admission(),"receiver":a.receiver()})
            != self.state.config
        {
            return Err("the execution agreement changed".into());
        }
        let opened = private_artifact::open(link_event, &self.secret).map_err(|e| e.to_string())?;
        if opened.artifact().schema.as_deref() != Some(crate::records::LINK)
            || opened.artifact().media_type != "application/json"
            || opened.signer() != a.market().buyer
            || opened.recipient() != self.signer.pubkey()
            || opened.body().issued_at > now.saturating_add(300)
            || opened.body().retain_until < now
        {
            return Err("the linkage has a different buyer, worker, or lifetime".into());
        }
        let link = contracts::parse_strict_bounded(
            opened.inline_bytes().ok_or("linkage bytes")?,
            1_048_576,
        )
        .map_err(|e| e.to_string())?;
        crate::exact(
            &link,
            crate::records::LINK,
            &[
                "issuer",
                "order",
                "request",
                "attempt",
                "run",
                "execute",
                "execute_body",
                "rework",
                "context",
                "previous",
            ],
        )?;
        let request = execution::open_labor_request(
            execute_event,
            &a.market().worker,
            &self.secret,
            now,
            execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let execution::Body::Execute(execute) = request.body else {
            return Err("the linked request is not execute".into());
        };
        let index = execute.attempt.checked_sub(1).ok_or("invalid attempt")? as usize;
        let link_ref = workspace_host::artifact::reference(
            opened.inline_bytes().unwrap(),
            "application/json",
            Some(crate::records::LINK),
        );
        let grant_digest = contracts::digest_bytes(grant_bytes);
        if let Some(prior) = self.state.attempts.get(index) {
            if prior.execute.id != execute_event.id
                || prior.linkage != link_ref
                || prior.grant_digest != grant_digest
            {
                return Err("the attempt is already bound to different bytes".into());
            }
            return self.reconcile(execute.attempt);
        }
        let expected_order = order_value(a)?;
        if request.principal != a.market().buyer
            || link["issuer"] != a.market().buyer
            || link["order"] != expected_order
            || link["request"] != execute.request
            || link["attempt"] != execute.attempt
            || link["run"] != execute.run
            || link["execute"]
                != json!({"id":execute_event.id,"pubkey":execute_event.pubkey,"kind":execute_event.kind})
            || read(&link["execute_body"], artifacts, a)?
                != contracts::jcs(&execute.payload).map_err(|e| e.to_string())?
            || execute.target != a.labor().execution.target
            || execute.lock != a.labor().execution.lock
            || execute.input_artifact.as_ref() != Some(&a.labor().execution.input)
            || execute.requirements != a.labor().execution.requirements
            || link["context"] != artifact_value(&execute.context)
            || execute.retain_until < a.market().retain_until
            || execute.attempt
                > u32::try_from(a.labor().max_reworks + 1).map_err(|_| "attempt count")?
            || index != self.state.attempts.len()
            || execute.deadline > self.roles.book().terms.due(index as u32)
            || execute.bounds.jobs != Some(1)
            || execute.bounds.spend_microunits.is_some()
        {
            return Err("the request differs from the admitted labor linkage".into());
        }
        let initial = a.blobs().get(&a.admission().context)?;
        let manifest =
            contracts::parse_strict_bounded(&read(&link["context"], artifacts, a)?, 1_048_576)
                .map_err(|e| e.to_string())?;
        let mut bytes = self.roles.bytes().clone();
        for (digest, raw) in artifacts {
            if contracts::digest_bytes(raw) != *digest {
                return Err("context attachment digest differs".into());
            }
            bytes.insert(digest.clone(), raw.clone());
        }
        if index == 0 {
            if !link["previous"].is_null()
                || !link["rework"].is_null()
                || execute.context != a.labor().execution.context
                || &manifest != initial
            {
                return Err("the initial context changed".into());
            }
        } else {
            let previous = self.reconcile(execute.attempt - 1)?;
            if previous
                .observation
                .as_ref()
                .is_none_or(|t| !Self::known_terminal(t))
                || link["previous"] != previous.linkage
            {
                return Err("the preceding attempt is not reconciled".into());
            }
            let previous_body =
                contracts::parse_strict_bounded(&self.store.get(&previous.linkage)?, 1_048_576)
                    .map_err(|e| e.to_string())?;
            if previous_body["request"] != execute.request || previous_body["run"] != execute.run {
                return Err("the repair changes its logical request or run".into());
            }
            let review = contracts::parse_artifact(&link["rework"]).map_err(|e| e.to_string())?;
            let mut evidence = BTreeMap::new();
            for raw in bytes.values() {
                if let Ok(value) = contracts::parse_strict_bounded(raw, 1_048_576) {
                    if value["v"] == "openagents.evidence.v1" {
                        if let Some(id) = value["id"].as_str() {
                            evidence.insert(id.to_string(), value);
                        }
                    }
                }
            }
            labor_roles::context::admit_rework(
                self.roles.book(),
                index as u32,
                &review.digest,
                initial,
                &manifest,
                &evidence,
                &bytes,
                now,
            )?;
            let previous_submission = self
                .roles
                .book()
                .entries()
                .iter()
                .find_map(|e| match &e.record {
                    Record::Submission(s) if s.number + 1 == index as u32 => Some(s),
                    _ => None,
                })
                .ok_or("the previous submission is unavailable")?;
            if previous.submission.as_ref().is_none_or(|s| contracts::digest_bytes(&contracts::jcs(s).unwrap_or_default()) != self.roles.book().entries().iter().find(|e| matches!(&e.record, Record::Submission(s) if s.number + 1 == index as u32)).map(|e|e.digest.as_str()).unwrap_or("")) {
                return Err("the preceding submission was not collected from this task".into());
            }
            if previous_submission.executions
                != [previous.linkage["digest"]
                    .as_str()
                    .ok_or("previous linkage")?
                    .to_string()]
            {
                return Err("the repair refers to another execution".into());
            }
        }
        let current = now.saturating_add(received.elapsed().as_secs());
        poll(a, &mut self.roles, current)?;
        if order_value(a)? != self.state.config["order"] {
            return Err("the market agreement changed".into());
        }
        if self
            .roles
            .book()
            .entries()
            .iter()
            .any(|e| matches!(e.record, Record::Cancellation(_) | Record::Acceptance(_)))
        {
            return Err("the order is stopped or final".into());
        }
        let concurrency = Self::bound(a, Bound::Concurrency)?;
        let wall = execute.bounds.wall_ms.ok_or("attempt wall bound")?;
        let output = execute.bounds.output_bytes.ok_or("attempt output bound")?;
        if concurrency != 1
            || self
                .state
                .attempts
                .iter()
                .map(|a| a.wall_ms)
                .fold(0u64, u64::saturating_add)
                .saturating_add(wall)
                > Self::bound(a, Bound::WallMs)?
            || self
                .state
                .attempts
                .iter()
                .map(|a| a.output_bytes)
                .fold(0u64, u64::saturating_add)
                .saturating_add(output)
                > Self::bound(a, Bound::OutputBytes)?
        {
            return Err("the attempt exceeds remaining whole-order bounds".into());
        }
        let input = a.blobs().get(&a.admission().input)?;
        let requirements = a.blobs().get(&a.admission().requirements)?;
        let local_task = format!("labor-{}-a{}", execute.request, execute.attempt);
        let (command, _grant) = prepare_frozen(
            input,
            requirements,
            &execute,
            &local_task,
            grant_bytes,
            current,
        )?;
        let target = a.blobs().get(&a.admission().target["artifact"])?;
        let context_path = PathBuf::from(
            target["meta"]["context_file"]
                .as_str()
                .ok_or("native context file")?,
        );
        let workspace = PathBuf::from(
            input["intent"]["workspace"]["path"]
                .as_str()
                .ok_or("native workspace")?,
        );
        let workspace = std::fs::canonicalize(workspace).map_err(|e| e.to_string())?;
        let context_parent = std::fs::canonicalize(context_path.parent().ok_or("context parent")?)
            .map_err(|e| e.to_string())?;
        if context_parent.starts_with(&workspace)
            || context_path.starts_with(&workspace)
            || !context_path.is_absolute()
            || context_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err("the context file must be outside workspace writes".into());
        }
        let parsed = contracts::parse_context(&manifest).map_err(|e| e.to_string())?;
        let mut representations = Vec::new();
        for entry in parsed.entries {
            let raw = read(&artifact_value(&entry.representation), &bytes, a)?;
            let content = match contracts::parse_strict_bounded(&raw, 1_048_576) {
                Ok(value) => json!({"encoding":"json","value":value}),
                Err(_) => json!({"encoding":"bytes","value":raw}),
            };
            representations.push(json!({"evidence":entry.evidence,"representation":artifact_value(&entry.representation),"content":content}));
        }
        let context_bytes =
            contracts::jcs(&json!({"manifest":manifest,"representations":representations,"deadline":execute.deadline}))
                .map_err(|e| e.to_string())?;
        if context_bytes.len() > 2 * 1_048_576 {
            return Err("the native context exceeds its byte bound".into());
        }
        if context_path.exists() {
            let existing = workspace_host::artifact::read(&context_path, 2 * 1_048_576)?;
            if self.state.context_hash.as_deref()
                != Some(contracts::digest_bytes(&existing).as_str())
            {
                return Err("the context file is not owned by this execution journal".into());
            }
        }
        let tasks =
            task::Store::open(&self.store.root().join("tasks")).map_err(|e| e.to_string())?;
        match tasks.show(&local_task) {
            Ok(_) => return Err("the local task exists without this linkage".into()),
            Err(task::Error::NotFound) => (),
            Err(e) => return Err(e.to_string()),
        }
        drop(tasks);
        if received.elapsed() >= Duration::from_secs(execute.deadline.saturating_sub(now)) {
            return Err("the execution deadline has passed".into());
        }
        self.store.put(
            opened.inline_bytes().unwrap(),
            "application/json",
            Some(crate::records::LINK),
        )?;
        let run_evidence = self.admit_run(a, &link, &local_task, index, current)?;
        self.state.attempts.push(Attempt {
            execute: execute_event.clone(),
            linkage: link_ref,
            context: artifact_value(&execute.context),
            local_task: local_task.clone(),
            grant_digest,
            wall_ms: wall,
            output_bytes: output,
            observation: None,
            outcome: "unknown".into(),
            submission: None,
            run_evidence,
        });
        self.state.context_hash = Some(contracts::digest_bytes(&context_bytes));
        self.save()?;
        workspace_host::artifact::atomic(&context_path, &context_bytes)?;
        let directory = self.store.root().join("tasks");
        {
            let mut tasks = task::Store::open(&directory).map_err(|e| e.to_string())?;
            tasks
                .apply(&serde_json::to_vec(&command).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
        let context_digest = contracts::digest_bytes(&context_bytes);
        let run = task::owner::execute_with_readonly_input(
            &directory,
            grant_bytes,
            &context_path,
            &context_digest,
        );
        tokio::pin!(run);
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        let began = received;
        let mut cancelled = false;
        loop {
            tokio::select! {
                _result=&mut run=>break,
                _=interval.tick()=>{
                    let current=now.saturating_add(began.elapsed().as_secs());
                    let stop=current>=execute.deadline || poll(a,&mut self.roles,current).is_err()
                        || a.order().is_none() || self.roles.book().entries().iter().any(|e|matches!(e.record,Record::Cancellation(_)|Record::Acceptance(_)));
                    if stop && !cancelled {
                        let mut tasks=task::Store::open(&directory).map_err(|e|e.to_string())?;
                        let cancel=Command {schema:task::COMMAND_SCHEMA.into(),command_id:format!("stop-{local_task}"),task_id:local_task.clone(),expected_revision:Some(tasks.show(&local_task).map_err(|e|e.to_string())?.revision),
                            action:Action::Cancel {reason:"The labor order was stopped.".into()}};
                        tasks.apply(&serde_json::to_vec(&cancel).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
                        cancelled=true;
                    }
                }
            }
        }
        self.reconcile(execute.attempt)
    }
}
