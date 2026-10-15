//! A separately keyed Rust checker compares actual frozen inputs and outputs.
use crate::{
    Result,
    authority::{Authority, closed},
};
use nostr::{
    contracts,
    domain::{Event, RelaySigner},
    work,
};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn component(check: &str) -> Value {
    json!({"v":"openagents.work-native-check.v1","requires":[],"check":check,"operation":"exact-output","implementation":"rust-native"})
}
impl Authority {
    fn verification_inputs(&self, event: &Event, now: u64) -> Result<Value> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.work-check-input.v1"))?;
        closed(&body, &["v", "requires", "issuer", "job"])?;
        let _lock = self.store.lock()?;
        let state = self.state()?;
        let (scope, job) = state
            .scopes
            .values()
            .find_map(|s| {
                s.jobs
                    .get(body["job"].as_str().unwrap_or(""))
                    .map(|j| (s, j))
            })
            .ok_or("not_admitted: work execution unavailable")?;
        self.check(scope, &actor, Some(&job.work.item), "read", now)?;
        self.check(scope, &actor, Some(&job.work.item), "evidence", now)?;
        if actor == job.delegate
            || !job.confirmed
            || !scope.policy["verifiers"]
                .as_array()
                .is_some_and(|keys| keys.contains(&json!(actor)))
        {
            return Err("not_admitted: independent verifier required".into());
        }
        let output = job.result.as_ref().ok_or("job result")?["output"].clone();
        let policy = self.store.json(&job.acceptance)?;
        self.acceptance_policy(&policy)?;
        Ok(
            json!({"work":job.work,"basis":job.basis,"frame":job.frame,"acceptance":job.acceptance,"policy":policy,"job":job.id,"input":job.input,"input_body":self.store.json(&job.input)?,"output":output,"output_body":self.store.json(&output)?,"grant":self.check(scope,&actor,Some(&job.work.item),"evidence",now)?.reference}),
        )
    }
    pub fn submit_check(
        &self,
        event: &Event,
        attachments: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<Value> {
        let (actor, body) =
            self.authenticate(event, now, Some("openagents.work-check-report.v1"))?;
        let w = work::work_ref(&body["work"]).map_err(|e| e.to_string())?;
        if attachments.len() > 128
            || attachments.values().map(Vec::len).sum::<usize>() > 4 * 1_048_576
        {
            return Err("check report attachment limit exceeded".into());
        }
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let scope = state
            .scopes
            .get(&w.scope)
            .ok_or("work scope unavailable")?
            .clone();
        self.check(&scope, &actor, Some(&w.item), "evidence", now)?;
        self.check(&scope, &actor, Some(&w.item), "read", now)?;
        for (digest, bytes) in attachments {
            if contracts::digest_bytes(bytes) != *digest || bytes.len() > 1_048_576 {
                return Err("check artifact identity differs".into());
            }
            self.store.put(bytes, "application/octet-stream", None)?;
        }
        let reference = self.retain_declaration(&mut state, event, &body)?;
        let (_, _, item) = self.head(&scope, &w.item)?.ok_or("work unavailable")?;
        self.verification(
            &state,
            &scope,
            &w,
            &item,
            &json!({"artifact":reference,"basis":body["basis"]}),
        )?;
        self.save(&state)?;
        Ok(reference)
    }
    pub(crate) fn check_receipt_identity(
        &self,
        report: &Value,
        result: &Value,
        job: &crate::execution::Job,
        scope: &crate::authority::Scope,
    ) -> Result<()> {
        let receipt = self.store.json(&result["receipt"])?;
        let expected = json!({"v":"openagents.work-check-data.v1","requires":[],"input":job.input,"output":job.result.as_ref().ok_or("job result")?["output"],"frame":job.frame,"acceptance":job.acceptance,"check":result["id"]});
        let lock = json!({"v":"openagents.lock.v1","requires":[],"root":receipt["component"],"entries":[{"id":receipt["component"]["id"],"definition":receipt["component"],"dependencies":[]}]});
        let expected_id = format!(
            "{}:work/check-{}",
            report["issuer"].as_str().ok_or("check issuer")?,
            result["id"].as_str().ok_or("check ID")?
        );
        if receipt["component"]["id"] != expected_id
            || receipt["lock"]
                != contracts::digest_bytes(&contracts::jcs(&lock).map_err(|e| e.to_string())?)
            || receipt["context"] != job.input["digest"]
            || receipt["authority"]
                != scope
                    .grants
                    .get(report["issuer"].as_str().ok_or("check issuer")?)
                    .ok_or("check grant")?
                    .reference
            || receipt["dispatched"] != true
            || receipt["attempt"] != 1
            || receipt["step"] != result["id"]
            || receipt["usage"]["spend_microunits"] != 0
        {
            return Err(
                "check execution identity or grant differs from its admitted operation".into(),
            );
        }
        if self.store.json(&receipt["input"])? != expected
            || self.store.json(&receipt["component"]["artifact"])?
                != component(result["id"].as_str().ok_or("check ID")?)
            || receipt["outcome"] != "completed"
            || receipt["recipient"] != report["issuer"]
            || receipt["run"] != job.id
            || receipt["enforcement"] != job.acceptance
        {
            return Err(
                "check receipt does not bind this execution's exact inputs, output, and policy"
                    .into(),
            );
        }
        Ok(())
    }
}
/// Produce an independently signed result. No verdict is inferred from a job's
/// terminal status, and a failed comparison remains failed.
pub fn verify_job(
    host: &Authority,
    secret: &SecretKey,
    job: &str,
    now: u64,
) -> Result<(Event, BTreeMap<String, Vec<u8>>)> {
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|_| "verifier key")?;
    let event = workspace_host::host::request(
        secret,
        host.signer.pubkey(),
        json!({"v":"openagents.work-check-input.v1","requires":[],"issuer":signer.pubkey(),"job":job}),
        now,
    )?;
    let data = host.verification_inputs(&event, now)?;
    let ids = host.acceptance_policy(&data["policy"])?;
    let mut files = BTreeMap::new();
    let mut put = |value: &Value| -> Result<Value> {
        let bytes = contracts::jcs(value).map_err(|e| e.to_string())?;
        let reference = workspace_host::artifact::json_ref(value)?;
        files.insert(contracts::digest_bytes(&bytes), bytes);
        Ok(reference)
    };
    let mut checks = Vec::new();
    for id in ids {
        if id != "exact-output" {
            return Err("unsupported native work check".into());
        }
        let passed = data["output_body"]["value"] == data["input_body"]["objective"];
        let check_input = put(
            &json!({"v":"openagents.work-check-data.v1","requires":[],"input":data["input"],"output":data["output"],"frame":data["frame"],"acceptance":data["acceptance"],"check":id}),
        )?;
        let definition = json!({"id":format!("{}:work/check-{id}",signer.pubkey()),"artifact":put(&component(&id))?});
        let lock = put(
            &json!({"v":"openagents.lock.v1","requires":[],"root":definition,"entries":[{"id":definition["id"],"definition":definition,"dependencies":[]}]}),
        )?;
        let receipt = json!({"v":"openagents.execution-receipt.v1","requires":[],"run":job,"step":id,"attempt":1,"dispatched":true,"component":definition,"lock":lock["digest"],"input":check_input,"context":data["input"]["digest"],"authority":data["grant"],"enforcement":data["acceptance"],"recipient":signer.pubkey(),"usage":{"wall_ms":0,"spend_microunits":0},"output":null,"outcome":"completed","verification":if passed{"passed"}else{"failed"},"integration":"not_requested"});
        contracts::parse_receipt(&receipt).map_err(|e| e.to_string())?;
        checks.push(json!({"id":id,"passed":passed,"receipt":put(&receipt)?}));
    }
    let report = json!({"v":"openagents.work-check-report.v1","requires":[],"issuer":signer.pubkey(),"work":data["work"],"basis":data["basis"],"frame":data["frame"],"acceptance":data["acceptance"],"checks":checks,"execution":job});
    let event = workspace_host::host::request(secret, host.signer.pubkey(), report, now)?;
    Ok((event, files))
}
