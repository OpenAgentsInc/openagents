//! Fixed Rust data and patch preparation worker over the existing CJ service.
use crate::{Result, book::Coordinator};
use nostr::{contracts, domain::Event};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerState {
    v: String,
    service: nostr::execution::Service,
    roots: BTreeMap<String, Value>,
    events: BTreeMap<String, Event>,
    outputs: BTreeMap<String, Value>,
}
/// A fixed Rust worker. Data and proposed patches remain data until integration.
pub struct Worker {
    store: workspace_host::artifact::Artifacts,
    secret: secp256k1::SecretKey,
    signer: nostr::domain::RelaySigner,
}
impl Worker {
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
                    v: "openagents.coordination-worker.v1".into(),
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
        authority: &Coordinator,
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
        if state.v != "openagents.coordination-worker.v1"
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
        let intake = json!({"v":"openagents.coordination-native-intake.v1","requires":[],"request":event.id,"fingerprint":execute.fingerprint});
        let intake =
            workspace_host::host::request(&self.secret, authority.authority(), intake, now)?;
        let (payload, pins) = authority.intake(&intake, now)?;
        if payload != execute.payload || opened.principal != authority.authority() {
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
        if !matches!(
            target["binding_contract"]["operation"].as_str(),
            Some("openagents.coord-native-data.v1" | "openagents.coord-native-git-change.v1")
        ) {
            return Err("unsupported native worker target".into());
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
        let output = input;
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
