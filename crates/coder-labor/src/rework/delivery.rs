//! Deliver only retained task artifacts; checks run over those exact bytes.
use super::*;
use std::path::Path;

pub struct Publication {
    pub body: Value,
    pub artifacts: Bytes,
    pub events: Vec<Event>,
}
fn put(store: &Artifacts, bytes: &mut Bytes, value: Value) -> Result<Value> {
    let raw = contracts::jcs(&value).map_err(|e| e.to_string())?;
    let reference = store.put(&raw, "application/json", value["v"].as_str())?;
    bytes.insert(
        reference["digest"]
            .as_str()
            .ok_or("artifact digest")?
            .into(),
        raw,
    );
    Ok(reference)
}
impl Worker {
    pub(super) fn admit_run(
        &self,
        a: &Agreement,
        link: &Value,
        local_task: &str,
        index: usize,
        now: u64,
    ) -> Result<Vec<Value>> {
        let mut runs = if index == 0 {
            Vec::new()
        } else {
            self.state.attempts[index - 1]
                .submission
                .as_ref()
                .ok_or("previous submission")?["run_evidence"]
                .as_array()
                .ok_or("previous RUN evidence")?
                .clone()
        };
        if index == 0 {
            append_run(
                &self.store,
                &mut runs,
                self.signer.pubkey(),
                link,
                index,
                now,
                "created",
                json!({"owner":self.signer.pubkey(),"request":link["request"],"base":a.admission().input,"program":a.admission().target,
                    "lock":artifact_value(&a.labor().execution.lock),"context":link["context"],"policy":a.admission().requirements,"parent":null,"recipients":a.role_terms()?.rights.recipients}),
            )?;
        }
        append_run(
            &self.store,
            &mut runs,
            self.signer.pubkey(),
            link,
            index,
            now,
            "admitted",
            json!({"enforcement":a.admission().requirements,"reservations":[format!("{}:{}",a.order().ok_or("order")?.order_id,index+1)],
                "deadlines":{"delivery":self.roles.book().terms.due(index as u32)},"retention":a.market().retain_until,"input":a.admission().input}),
        )?;
        append_run(
            &self.store,
            &mut runs,
            self.signer.pubkey(),
            link,
            index,
            now,
            "dispatched",
            json!({"binding":a.admission().target,"attempt":index+1,"input":a.admission().input,"context":link["context"],"effect":format!("{local_task}:1:command"),"generation":0}),
        )?;
        Ok(runs)
    }
    /// Record an authenticated receive adapter's observation. The receiver,
    /// provider payee, order, price and artifact pins come from this admission.
    /// The caller must authenticate its adapter; this method moves no funds.
    pub fn record_funding(
        &mut self,
        a: &Agreement,
        receiver: &str,
        ledger: &mut pay_ledger::Ledger,
        receipt: &pay_ledger::markets::worker::FundingReceipt,
    ) -> Result<pay_ledger::Recorded> {
        if a.receiver() != Some(receiver)
            || json!({"order":order_value(a)?,"admission":a.admission(),"receiver":a.receiver()})
                != self.state.config
        {
            return Err("the receive adapter differs from the frozen agreement".into());
        }
        let pin = |v: &Value| -> Result<String> {
            Ok(v["digest"]
                .as_str()
                .ok_or("funding pin")?
                .trim_start_matches("sha256:")
                .to_string())
        };
        let context = a.blobs().get(&a.admission().context)?;
        let pins = BTreeMap::from([
            ("source", pin(&a.admission().input)?),
            ("checker", pin(&a.admission().checker["artifact"])?),
            ("disclosure", pin(&context["policy"])?),
            ("execution_requirements", pin(&a.admission().requirements)?),
        ]);
        self.roles.fund(ledger, &pins, receipt)
    }
    /// Collect named JSON deliverables from the task owner's sealed result,
    /// never from files that the worker can still edit. Retrying republishes
    /// the saved submission, including after a crash before transport.
    pub fn submit(
        &mut self,
        a: &Agreement,
        attempt: u32,
        paths: &BTreeMap<String, PathBuf>,
        now: u64,
    ) -> Result<Publication> {
        if json!({"order":order_value(a)?,"admission":a.admission(),"receiver":a.receiver()})
            != self.state.config
        {
            return Err("the submission agreement changed".into());
        }
        let observed = self.reconcile(attempt)?;
        let index = attempt.checked_sub(1).ok_or("attempt")? as usize;
        let mut bytes = self.roles.bytes().clone();
        if let Some(body) = &observed.submission {
            for reference in submission_artifacts(body) {
                let raw = self.store.get(&reference)?;
                bytes.insert(
                    reference["digest"]
                        .as_str()
                        .ok_or("artifact digest")?
                        .into(),
                    raw,
                );
            }
            return Ok(Publication {
                body: body.clone(),
                events: self.roles.issue(body, &bytes, now)?,
                artifacts: bytes,
            });
        }
        let task = observed
            .observation
            .as_ref()
            .ok_or("task outcome unavailable")?;
        let run = task.run.as_ref().ok_or("task run unavailable")?;
        let result = run.result.as_ref().ok_or("task result unavailable")?;
        if !Self::known_terminal(task) || observed.outcome != "finished" || result.output_incomplete
        {
            return Err(
                "only a finished task with complete retained output can be submitted".into(),
            );
        }
        if paths.len() != a.labor().deliverables.len() {
            return Err("the deliverable paths do not match the order".into());
        }
        let mut deliverables = Vec::new();
        for declaration in &a.labor().deliverables {
            let path = paths
                .get(&declaration.id)
                .ok_or("missing deliverable path")?;
            let raw = task::artifact::read(
                &self.store.root().join("tasks"),
                &observed.local_task,
                Path::new(path),
            )
            .map_err(|e| e.to_string())?;
            if raw.len() as u64 > declaration.max_bytes {
                return Err("a deliverable exceeds its accepted byte limit".into());
            }
            contracts::parse_strict_bounded(&raw, 1_048_576).map_err(|e| e.to_string())?;
            let reference = self.store.put(&raw, "application/json", None)?;
            bytes.insert(reference["digest"].as_str().unwrap().into(), raw);
            deliverables.push(json!({"id":declaration.id,"content":reference}));
        }
        let receipt = put(
            &self.store,
            &mut bytes,
            json!({"v":"openagents.labor-native-execution.v1","requires":[],
            "execute":{"id":observed.execute.id,"pubkey":observed.execute.pubkey,"kind":observed.execute.kind},"linkage":observed.linkage,
            "grant_digest":observed.grant_digest,"program_digest":run.admission.program_digest,
            "source_revision":run.admission.source_revision,"source_snapshot":run.admission.source_snapshot,
            "context":observed.context,"outcome":observed.outcome,"elapsed_ms":result.elapsed_ms,
            "exit_code":result.exit_code,"group_clear":result.group_clear,"trace_digest":result.trace_digest,
            "artifact_digest":result.artifact_digest,"candidate_snapshot":result.candidate_snapshot,
            "cost_usd":null,"limitations":"Local task-owner observation; no remote attestation or complete cost measurement."}),
        )?;
        let link_raw = self.store.get(&observed.linkage)?;
        bytes.insert(
            observed.linkage["digest"].as_str().unwrap().into(),
            link_raw.clone(),
        );
        let link: Value = serde_json::from_slice(&link_raw).map_err(|e| e.to_string())?;
        let mut runs = observed.run_evidence.clone();
        append_run(
            &self.store,
            &mut runs,
            self.signer.pubkey(),
            &link,
            index,
            now,
            "resolved",
            json!({"outcome":"completed","dispatched":true,"output":deliverables[0]["content"],"receipts":[receipt],"usage":{"cost_usd":null},"verification":"not_run","integration":"unconfirmed"}),
        )?;
        for reference in &runs {
            bytes.insert(
                reference["digest"].as_str().ok_or("RUN digest")?.into(),
                self.store.get(reference)?,
            );
        }
        let previous_submission = if index == 0 {
            Value::Null
        } else {
            self.state.attempts[index - 1]
                .submission
                .as_ref()
                .ok_or("previous collected submission")?
                .clone()
        };
        let previous_ref = if previous_submission.is_null() {
            Value::Null
        } else {
            put(&self.store, &mut bytes, previous_submission)?
        };
        let body = json!({"v":labor_roles::records::SUBMISSION_V,"requires":[],"issuer":self.signer.pubkey(),"order":order_value(a)?,
            "number":index,"previous":previous_ref,"rework":link["rework"],"executions":[observed.linkage],"deliverables":deliverables,
            "run_evidence":runs,"limitations":receipt});
        // Retain all direct references before saving the identity. Issuing
        // checks the pinned schema and chronology before it can publish.
        self.store.put_json(&body)?;
        self.state.attempts[index].submission = Some(body.clone());
        self.save()?;
        let events = self.roles.issue(&body, &bytes, now)?;
        Ok(Publication {
            body,
            artifacts: bytes,
            events,
        })
    }
}
fn append_run(
    store: &Artifacts,
    refs: &mut Vec<Value>,
    signer: &str,
    link: &Value,
    index: usize,
    now: u64,
    kind: &str,
    data: Value,
) -> Result<()> {
    let mut journal = nostr::run::Journal::default();
    for reference in refs.iter() {
        let value = store.json(reference)?;
        let record = nostr::run::parse_envelope(&value).map_err(|e| e.to_string())?;
        if record.run != link["run"].as_str().ok_or("RUN identity")?
            || nostr::run::ingest(&mut journal, record, signer).map_err(|e| e.to_string())?
                != nostr::run::Ingest::Applied
        {
            return Err("the saved RUN evidence has a gap or fork".into());
        }
    }
    let previous = journal
        .applied
        .last()
        .map(|r| json!(r.digest))
        .unwrap_or(Value::Null);
    let record = json!({"run":link["run"],"seq":journal.applied.len(),"previous":previous,"controller":signer,"generation":0,
        "type":kind,"subject":{"step":"command","iteration":index,"attempt":index+1},"time":now,"data":data});
    let digest = nostr::run::logical_digest(&record).map_err(|e| e.to_string())?;
    let envelope =
        json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"record":record,"digest":digest});
    let parsed = nostr::run::parse_envelope(&envelope).map_err(|e| e.to_string())?;
    if nostr::run::ingest(&mut journal, parsed, signer).map_err(|e| e.to_string())?
        != nostr::run::Ingest::Applied
    {
        return Err("the new RUN record does not extend its saved chain".into());
    }
    refs.push(store.put_json(&envelope)?);
    Ok(())
}
fn submission_artifacts(body: &Value) -> Vec<Value> {
    let mut refs = vec![body["limitations"].clone()];
    for field in ["executions", "run_evidence"] {
        refs.extend(body[field].as_array().into_iter().flatten().cloned());
    }
    refs.extend(
        body["deliverables"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|d| d["content"].clone()),
    );
    if !body["previous"].is_null() {
        refs.push(body["previous"].clone());
    }
    refs
}

/// The explicitly supported native checker compares the delivered JSON's
/// exact byte hash with the frozen expected hash. It executes no worker code
/// and has no acceptance, wallet, or task-control authority.
pub fn verify(
    a: &Agreement,
    reviewer: &mut wire_host::Host,
    submission: &Value,
    now: u64,
) -> Result<Publication> {
    if reviewer.pubkey() != a.labor().reviewer || reviewer.book().terms != a.role_terms()? {
        return Err("this participant is not the admitted independent reviewer".into());
    }
    let definition = a.blobs().get(&a.admission().checker["artifact"])?;
    if definition["binding_contract"]["operation"] != "coder.free-labor.byte-equality.v1"
        || a.policy().criteria != ["result"]
        || a.labor().deliverables.len() != 1
    {
        return Err("the native reviewer does not implement this checker profile".into());
    }
    let r = contracts::parse_artifact(submission).map_err(|e| e.to_string())?;
    let body = reviewer
        .book()
        .original(&r.digest)
        .ok_or("the reviewer has not received this submission")?
        .clone();
    contracts::check_artifact_bytes(&r, &contracts::jcs(&body).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if body["v"] != labor_roles::records::SUBMISSION_V {
        return Err("not a submission".into());
    }
    let content = &body["deliverables"][0]["content"];
    let raw = reviewer
        .bytes()
        .get(content["digest"].as_str().ok_or("content digest")?)
        .ok_or("deliverable unavailable")?;
    let content_ref = contracts::parse_artifact(content).map_err(|e| e.to_string())?;
    contracts::check_artifact_bytes(&content_ref, raw).map_err(|e| e.to_string())?;
    let expected = a.blobs().get(&a.admission().input)?["expected_output_digest"]
        .as_str()
        .ok_or("expected output hash")?
        .to_string();
    let actual = contracts::digest_bytes(raw);
    let verdict = if actual == expected {
        "passed"
    } else {
        "failed"
    };
    let receipt = json!({"v":"openagents.labor-native-checker.v1","requires":[],"submission":submission,"checker":a.admission().checker,
        "lock":artifact_value(&a.policy().lock),"input":a.admission().input,"deliverable":content,"expected":expected,"observed":actual,
        "verdict":verdict,"cost_usd":null,"limitations":"Exact byte equality only; no broader quality or remote operator claim."});
    let raw = contracts::jcs(&receipt).map_err(|e| e.to_string())?;
    let receipt_ref = workspace_host::artifact::reference(
        &raw,
        "application/json",
        Some("openagents.labor-native-checker.v1"),
    );
    let mut bytes = reviewer.bytes().clone();
    let policy_raw = contracts::jcs(a.blobs().resolve(&a.labor().acceptance_policy)?)
        .map_err(|e| e.to_string())?;
    bytes.insert(a.labor().acceptance_policy.digest.clone(), policy_raw);
    bytes.insert(receipt_ref["digest"].as_str().unwrap().into(), raw);
    let verification = json!({"v":labor_roles::records::VERIFICATION_V,"requires":[],"issuer":reviewer.pubkey(),"order":order_value(a)?,
        "submission":submission,"policy":artifact_value(&a.labor().acceptance_policy),"checker_receipts":[receipt_ref],
        "criteria":[{"id":"result","verdict":verdict,"evidence":[receipt_ref]}],"verdict":verdict,"limitations":receipt_ref});
    let events = reviewer.issue(&verification, &bytes, now)?;
    Ok(Publication {
        body: verification,
        artifacts: bytes,
        events,
    })
}
