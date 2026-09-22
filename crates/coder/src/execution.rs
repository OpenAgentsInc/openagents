//! NIP-CJ execution jobs.
//!
//! [`Store::answer`] admits a kind-`25920` event and persists the claim
//! before it returns `accepted`. [`Store::dispatch_program`] then runs the
//! pinned program through [`crate::runtime::Runtime`], the same runtime
//! the terminal and `coder --print` use for a program, and seals a
//! [`receipts::ExecutionReceipt`] in the local store. A relay `OK` and a
//! closed socket do not accept or cancel a claim.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use nostr::domain::{Event, RelaySigner, Tag};
use nostr::execution::{self, Admission, Body, Error, Report, Service, Window, refusal_result};
use nostr::nip44;
use receipts::ExecutionReceipt;
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};

use crate::delegate::Task;
use crate::program::Program;
use crate::program_authority::Grant;
use crate::runtime::{Inputs, Runtime};

/// One opened execution request and the pins the host already holds.
pub struct Intake<'a> {
    /// The kind-`25920` event.
    pub event: &'a Event,
    /// Worker secret.
    pub secret: &'a SecretKey,
    /// Worker signer.
    pub signer: &'a RelaySigner,
    /// Unix seconds.
    pub now: u64,
    /// Freshness window.
    pub window: Window,
    /// Artifact digest to exact bytes.
    pub bytes: &'a BTreeMap<String, Vec<u8>>,
    /// Quoted spend, when the host knows it.
    pub quoted_spend: Option<u64>,
    /// Remaining budget, when the host knows it.
    pub remaining: Option<u64>,
    /// Nonce for the reply.
    pub nonce: [u8; 32],
    /// Mailbox for a new claim. 64 hexadecimal characters.
    pub mailbox: &'a str,
}

/// The program run [`Store::dispatch_program`] hands to the shared runtime.
pub struct Dispatch<'a> {
    /// Idempotency key.
    pub key: &'a str,
    /// Worker signer.
    pub signer: &'a RelaySigner,
    /// Worker secret.
    pub secret: &'a SecretKey,
    /// Reply nonce.
    pub nonce: [u8; 32],
    /// Program runtime.
    pub runtime: &'a Runtime,
    /// Program to run.
    pub program: &'a Program,
    /// Run inputs.
    pub inputs: &'a Inputs,
    /// Operator grant.
    pub grant: &'a Grant,
}

/// Durable execution ledger for one worker.
pub struct Store {
    dir: PathBuf,
    service: Service,
    /// Quoted spend applied when an intake does not override it.
    pub quoted_spend: Option<u64>,
    /// Remaining budget applied when an intake does not override it.
    pub remaining: Option<u64>,
}

impl Store {
    /// Load `dir`, or create an empty ledger for `worker`.
    ///
    /// # Errors
    ///
    /// Returns a message when `dir` cannot be created or the ledger is not
    /// the stored shape.
    pub fn open(dir: &Path, worker: &str, capacity: u32, horizon: u64) -> Result<Self, String> {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        let path = dir.join("service.json");
        let service = if path.exists() {
            let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            serde_json::from_str(&text).map_err(|error| error.to_string())?
        } else {
            Service::new(worker, capacity, horizon)
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            service,
            quoted_spend: None,
            remaining: None,
        })
    }

    /// A relay `OK` does not accept a claim.
    #[must_use]
    pub const fn note_relay_ok(&self) -> bool {
        false
    }

    /// A closed socket does not cancel a claim.
    #[must_use]
    pub const fn note_socket_closed(&self) -> bool {
        false
    }

    /// Apply a crash boundary and keep the reservation on disk.
    ///
    /// # Errors
    ///
    /// Returns a message when `key` is unknown or the ledger cannot be saved.
    pub fn crash(&mut self, key: &str, boundary: nostr::run::Boundary) -> Result<(), String> {
        self.service.crash(key, boundary).map_err(show)?;
        self.save()
    }

    /// Admit `intake` and return the events the worker should publish.
    ///
    /// The NIP-RUN root is on disk before an `accepted` event is built.
    ///
    /// # Errors
    ///
    /// Returns a message when the ledger cannot be saved. A typed refusal
    /// is an event, not an error.
    pub fn answer(&mut self, intake: &Intake<'_>) -> Result<Vec<Event>, String> {
        let opened = match execution::open_request(
            intake.event,
            &self.service.worker,
            intake.secret,
            intake.now,
            intake.window,
        ) {
            Ok(opened) => opened,
            Err(error) => return self.drop_or_refuse(intake, &error),
        };
        match &opened.body {
            Body::Execute(execute) => {
                if let Err(error) = execution::require_pins(execute, intake.bytes) {
                    return self.refuse_execute(intake, &opened.principal, execute, &error);
                }
                if let Err(error) =
                    execution::reserve_spend(&execute.bounds, intake.quoted_spend, intake.remaining)
                {
                    return self.refuse_execute(intake, &opened.principal, execute, &error);
                }
                match self.service.prepare(&opened, intake.now, intake.mailbox) {
                    Ok(Admission::Reserved { claim, root }) => {
                        let path = self.dir.join(format!("root-{}.json", claim.record_digest));
                        fs::write(&path, &root).map_err(|error| error.to_string())?;
                        let read = fs::read(&path).map_err(|error| error.to_string())?;
                        let accepted = self.service.acknowledge(&claim.key, &read).map_err(show)?;
                        self.save()?;
                        Ok(vec![self.reply(
                            intake,
                            &opened.principal,
                            execution::FEEDBACK_KIND,
                            &accepted,
                        )?])
                    }
                    Ok(Admission::Retransmission(claim)) => {
                        self.save()?;
                        if claim.tombstone {
                            let payload = refusal_result(
                                &claim.request,
                                claim.attempt,
                                &claim.run,
                                "content_unavailable",
                                "retention for this attempt has expired",
                            );
                            return Ok(vec![self.reply(
                                intake,
                                &claim.principal,
                                execution::RESULT_KIND,
                                &payload,
                            )?]);
                        }
                        let mut events = Vec::new();
                        if claim.acknowledged {
                            let accepted = self
                                .service
                                .acknowledge(
                                    &claim.key,
                                    &fs::read(self.root_path(&claim.record_digest))
                                        .unwrap_or_default(),
                                )
                                .unwrap_or_else(|_| json!(null));
                            if accepted["type"] == "accepted" {
                                events.push(self.reply(
                                    intake,
                                    &claim.principal,
                                    execution::FEEDBACK_KIND,
                                    &accepted,
                                )?);
                            }
                        }
                        if let Some(result) = claim.result.clone() {
                            events.push(self.reply(
                                intake,
                                &claim.principal,
                                execution::RESULT_KIND,
                                &result,
                            )?);
                        }
                        Ok(events)
                    }
                    Err(error) => self.refuse_execute(intake, &opened.principal, execute, &error),
                }
            }
            Body::Control { .. } => {
                let payload = match self.service.control(&opened) {
                    Ok(payload) => payload,
                    Err(error) => {
                        let (request, attempt, run) = control_ids(&opened.body);
                        refusal_result(
                            &request,
                            attempt,
                            &run,
                            error.code().unwrap_or("malformed"),
                            &detail(&error),
                        )
                    }
                };
                self.save()?;
                Ok(vec![self.reply(
                    intake,
                    &opened.principal,
                    execution::RESULT_KIND,
                    &payload,
                )?])
            }
        }
    }

    /// Record dispatch intent, run `dispatch.program`, and persist the result.
    ///
    /// # Errors
    ///
    /// Returns a message when the claim cannot move or the ledger cannot be
    /// saved. The runtime's refusal is a result event.
    pub async fn dispatch_program(&mut self, dispatch: Dispatch<'_>) -> Result<Event, String> {
        self.service.intend(dispatch.key).map_err(show)?;
        self.save()?;
        let run = dispatch
            .runtime
            .run(dispatch.program, dispatch.inputs, dispatch.grant, None)
            .await;
        let claim = self
            .service
            .claims
            .get(dispatch.key)
            .ok_or_else(|| "the claim is gone".to_string())?
            .clone();
        let dispatched = !run.steps.is_empty();
        let (outcome, output, code) = match &run.stopped {
            Some(refused) => (
                "refused",
                Some(json!({
                    "code": refused.code,
                    "step": refused.step
                })),
                Some(refused.code.clone()),
            ),
            None => {
                let text = run
                    .steps
                    .last()
                    .map(|step| step.output.clone())
                    .unwrap_or_default();
                ("completed", Some(json!(text)), None)
            }
        };
        let mut receipt = ExecutionReceipt::for_attempt(
            "relay",
            claim.request.clone(),
            claim.attempt,
            claim.fingerprint.clone(),
        );
        receipt.attempt_id = claim.execute_event.clone();
        receipt.outcome = match outcome {
            "completed" => receipts::Outcome::Answered,
            "refused" => receipts::Outcome::Refused,
            _ => receipts::Outcome::Unknown,
        };
        receipt.cause = code.clone();
        receipt.usage = None;
        receipt.seal();
        let receipt_bytes = receipt.to_json().into_bytes();
        let receipt_digest = nostr::contracts::digest_bytes(&receipt_bytes);
        fs::write(
            self.dir.join(format!("receipt-{receipt_digest}.json")),
            &receipt_bytes,
        )
        .map_err(|error| error.to_string())?;
        let output_bytes = serde_json::to_vec(&output).unwrap_or_default();
        let output_digest = nostr::contracts::digest_bytes(&output_bytes);
        let payload = self
            .service
            .observe(
                dispatch.key,
                Report {
                    outcome,
                    dispatched,
                    output,
                    artifacts: vec![json!({
                        "digest": output_digest,
                        "size": output_bytes.len() as u64,
                        "media_type": "application/json"
                    })],
                    receipts: vec![json!({
                        "digest": receipt_digest,
                        "size": receipt_bytes.len() as u64,
                        "media_type": "application/json"
                    })],
                    spend: None,
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code,
                    message: None,
                },
            )
            .map_err(show)?;
        self.save()?;
        self.reply_to(&claim.principal, dispatch, execution::RESULT_KIND, &payload)
    }

    fn refuse_execute(
        &mut self,
        intake: &Intake<'_>,
        principal: &str,
        execute: &execution::Execute,
        error: &Error,
    ) -> Result<Vec<Event>, String> {
        let Some(code) = error.code() else {
            return Ok(Vec::new());
        };
        let payload = refusal_result(
            &execute.request,
            execute.attempt,
            &execute.run,
            code,
            &detail(error),
        );
        self.save()?;
        Ok(vec![self.reply(
            intake,
            principal,
            execution::RESULT_KIND,
            &payload,
        )?])
    }

    fn drop_or_refuse(&mut self, intake: &Intake<'_>, error: &Error) -> Result<Vec<Event>, String> {
        let _ = (intake, error);
        Ok(Vec::new())
    }

    fn reply(
        &self,
        intake: &Intake<'_>,
        principal: &str,
        kind: u16,
        payload: &Value,
    ) -> Result<Event, String> {
        let peer = principal
            .parse()
            .map_err(|_| "the caller pubkey does not parse".to_string())?;
        let seal = execution::Seal {
            signer: intake.signer,
            conversation: nip44::conversation_key(intake.secret, &peer),
            nonce: intake.nonce,
            created_at: intake.now,
        };
        seal.event(
            kind,
            vec![
                Tag::new(vec!["e".into(), intake.event.id.clone()]),
                Tag::new(vec!["p".into(), principal.to_string()]),
            ],
            payload,
        )
        .map_err(show)
    }

    fn reply_to(
        &self,
        principal: &str,
        dispatch: Dispatch<'_>,
        kind: u16,
        payload: &Value,
    ) -> Result<Event, String> {
        let claim = self
            .service
            .claims
            .get(dispatch.key)
            .ok_or_else(|| "the claim is gone".to_string())?;
        let peer = principal
            .parse()
            .map_err(|_| "the caller pubkey does not parse".to_string())?;
        let seal = execution::Seal {
            signer: dispatch.signer,
            conversation: nip44::conversation_key(dispatch.secret, &peer),
            nonce: dispatch.nonce,
            created_at: claim.deadline,
        };
        seal.event(
            kind,
            vec![
                Tag::new(vec!["e".into(), claim.execute_event.clone()]),
                Tag::new(vec!["p".into(), principal.to_string()]),
            ],
            payload,
        )
        .map_err(show)
    }

    fn root_path(&self, digest: &str) -> PathBuf {
        self.dir.join(format!("root-{digest}.json"))
    }

    fn save(&self) -> Result<(), String> {
        let path = self.dir.join("service.json");
        let text =
            serde_json::to_string_pretty(&self.service).map_err(|error| error.to_string())?;
        fs::write(path, text).map_err(|error| error.to_string())
    }
}

fn control_ids(body: &Body) -> (String, u32, String) {
    match body {
        Body::Control {
            request,
            attempt,
            run,
            ..
        } => (request.clone(), *attempt, run.clone()),
        Body::Execute(execute) => (
            execute.request.clone(),
            execute.attempt,
            execute.run.clone(),
        ),
    }
}

fn detail(error: &Error) -> String {
    match error {
        Error::Malformed { detail }
        | Error::UnsupportedFeature { detail }
        | Error::CannotEnforce { detail } => detail.clone(),
        other => other.code().unwrap_or("malformed").to_string(),
    }
}

fn show(error: Error) -> String {
    detail(&error)
}

/// A one-task input list for a query program.
#[must_use]
pub fn one_task(prompt: &str) -> Inputs {
    Inputs {
        request: prompt.to_string(),
        tasks: vec![Task::asking(prompt)],
        executor: String::new(),
    }
}

/// The worker pubkey the process secret names.
#[must_use]
pub fn worker_pubkey(secret: &SecretKey) -> String {
    secret.x_only_public_key(&Secp256k1::new()).0.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::contracts::digest_bytes;
    use nostr::domain::RelaySigner;
    use nostr::execution::{SCHEMA, Watch};
    use secp256k1::SecretKey;
    use serde_json::json;

    const NOW: u64 = 1_700_000_000;
    const DEADLINE: u64 = 1_700_003_600;
    const RETAIN: u64 = 1_700_010_000;

    struct Keys {
        signer: RelaySigner,
        secret: SecretKey,
        pubkey: String,
    }

    fn keys(byte: u8) -> Keys {
        let raw = [byte; 32];
        let secret = SecretKey::from_byte_array(raw).unwrap();
        let hex: String = raw.iter().map(|item| format!("{item:02x}")).collect();
        let signer = RelaySigner::from_secret_hex(&hex).unwrap();
        let pubkey = signer.pubkey().to_string();
        Keys {
            signer,
            secret,
            pubkey,
        }
    }

    fn artifact(bytes: &[u8]) -> Value {
        json!({
            "digest": digest_bytes(bytes),
            "size": bytes.len() as u64,
            "media_type": "application/json"
        })
    }

    fn payload() -> Value {
        let publisher = "b".repeat(64);
        json!({
            "v": SCHEMA,
            "requires": [],
            "type": "execute",
            "request": "req-9",
            "attempt": 1,
            "run": "run-9",
            "target": {"id": format!("{publisher}:pkg/op"), "artifact": artifact(b"target")},
            "lock": artifact(b"lock"),
            "input": {"task": "count"},
            "context": artifact(b"context"),
            "requirements": artifact(b"requirements"),
            "bounds": {},
            "deadline": DEADLINE,
            "retain_until": RETAIN
        })
    }

    fn stored_bytes() -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        for bytes in [b"target".as_slice(), b"lock", b"context", b"requirements"] {
            out.insert(digest_bytes(bytes), bytes.to_vec());
        }
        out
    }

    fn event(from: &Keys, to: &Keys, body: &Value) -> Event {
        let peer = to.pubkey.parse().unwrap();
        execution::Seal {
            signer: &from.signer,
            conversation: nip44::conversation_key(&from.secret, &peer),
            nonce: [4; 32],
            created_at: NOW,
        }
        .event(
            execution::REQUEST_KIND,
            vec![
                Tag::new(vec!["p".into(), to.pubkey.clone()]),
                Tag::new(vec!["expiration".into(), DEADLINE.to_string()]),
            ],
            body,
        )
        .unwrap()
    }

    #[test]
    fn acceptance_is_persisted_before_it_is_published_and_survives_restart() {
        let caller = keys(4);
        let worker = keys(5);
        let dir = tempfile::tempdir().unwrap();
        let bytes = stored_bytes();
        let request = event(&caller, &worker, &payload());
        let mut store = Store::open(dir.path(), &worker.pubkey, 2, RETAIN).unwrap();
        let intake = Intake {
            event: &request,
            secret: &worker.secret,
            signer: &worker.signer,
            now: NOW,
            window: Window::DEFAULT,
            bytes: &bytes,
            quoted_spend: None,
            remaining: None,
            nonce: [8; 32],
            mailbox: &"cd".repeat(32),
        };
        let published = store.answer(&intake).unwrap();
        assert_eq!(published.len(), 1);
        assert!(dir.path().join("service.json").is_file());
        let root = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().starts_with("root-"))
            .unwrap();
        let root_bytes = fs::read(root.path()).unwrap();
        let accepted = execution::bind_worker_event(
            &published[0],
            &execution::Pending {
                execute_event: &request.id,
                worker: &worker.pubkey,
                customer: &caller.pubkey,
                request: "req-9",
                attempt: 1,
            },
            &caller.secret,
        )
        .unwrap();
        assert_eq!(accepted["type"], "accepted");
        assert_eq!(accepted["record"]["digest"], digest_bytes(&root_bytes));
        assert!(!store.note_relay_ok());
        assert!(!store.note_socket_closed());
        let mut watch = Watch::new();
        watch.note_relay_ok();
        watch.note_socket_closed();
        assert!(watch.accepted.is_none());

        let reopened = Store::open(dir.path(), &worker.pubkey, 2, RETAIN).unwrap();
        assert_eq!(reopened.service.active, 1);
        assert_eq!(reopened.service.claims.len(), 1);
    }

    #[test]
    fn a_missing_pin_refuses_before_a_claim_exists() {
        let caller = keys(4);
        let worker = keys(5);
        let dir = tempfile::tempdir().unwrap();
        let bytes = BTreeMap::new();
        let request = event(&caller, &worker, &payload());
        let mut store = Store::open(dir.path(), &worker.pubkey, 2, RETAIN).unwrap();
        let intake = Intake {
            event: &request,
            secret: &worker.secret,
            signer: &worker.signer,
            now: NOW,
            window: Window::DEFAULT,
            bytes: &bytes,
            quoted_spend: None,
            remaining: None,
            nonce: [8; 32],
            mailbox: &"cd".repeat(32),
        };
        let published = store.answer(&intake).unwrap();
        let payload = execution::bind_worker_event(
            &published[0],
            &execution::Pending {
                execute_event: &request.id,
                worker: &worker.pubkey,
                customer: &caller.pubkey,
                request: "req-9",
                attempt: 1,
            },
            &caller.secret,
        )
        .unwrap();
        assert_eq!(payload["outcome"], "refused");
        assert_eq!(payload["code"], "content_unavailable");
        assert_eq!(payload["dispatched"], false);
        assert!(payload["spend"].is_null());
        assert_eq!(store.service.claims.len(), 0);
    }

    #[tokio::test]
    async fn dispatch_uses_the_program_runtime_and_seals_a_receipt() {
        let caller = keys(4);
        let worker = keys(5);
        let dir = tempfile::tempdir().unwrap();
        let bytes = stored_bytes();
        let request = event(&caller, &worker, &payload());
        let mut store = Store::open(dir.path(), &worker.pubkey, 2, RETAIN).unwrap();
        let intake = Intake {
            event: &request,
            secret: &worker.secret,
            signer: &worker.signer,
            now: NOW,
            window: Window::DEFAULT,
            bytes: &bytes,
            quoted_spend: None,
            remaining: None,
            nonce: [8; 32],
            mailbox: &"cd".repeat(32),
        };
        store.answer(&intake).unwrap();
        let key = store.service.claims.keys().next().unwrap().clone();
        let program: Program = serde_json::from_value(json!({
            "v": 1,
            "slug": "list",
            "steps": [{"name": "select", "kind": "query", "bounds": {"max_results": 4}}]
        }))
        .unwrap();
        let inputs = one_task("count the lines");
        let runtime = Runtime::over(
            crate::survey::Survey {
                capabilities: Vec::new(),
                programs: crate::program::Registry::open(&[]),
                sources: crate::source::Registry::open(&[]),
                workspace: dir.path().to_path_buf(),
            },
            crate::questions::Registry::open(&[]),
            crate::runtime::Host::without_repository(),
        )
        .with_runstate(dir.path().join("runstate"));
        let result = store
            .dispatch_program(Dispatch {
                key: &key,
                signer: &worker.signer,
                secret: &worker.secret,
                nonce: [9; 32],
                runtime: &runtime,
                program: &program,
                inputs: &inputs,
                grant: &Grant::all(),
            })
            .await
            .unwrap();
        let payload = execution::bind_worker_event(
            &result,
            &execution::Pending {
                execute_event: &request.id,
                worker: &worker.pubkey,
                customer: &caller.pubkey,
                request: "req-9",
                attempt: 1,
            },
            &caller.secret,
        )
        .unwrap();
        assert_eq!(payload["outcome"], "completed");
        assert_eq!(payload["dispatched"], true);
        assert!(payload["spend"].is_null());
        assert_eq!(payload["verification"], "not_run");
        assert!(!payload["artifacts"].as_array().unwrap().is_empty());
        let receipt_ref = &payload["receipts"][0];
        let receipt_path = dir.path().join(format!(
            "receipt-{}.json",
            receipt_ref["digest"].as_str().unwrap()
        ));
        let receipt = ExecutionReceipt::parse(&fs::read_to_string(receipt_path).unwrap()).unwrap();
        assert!(receipt.usage.is_none());
        assert_eq!(receipt.transport, "relay");
        store.crash(&key, nostr::run::Boundary::Effect).unwrap();
        let crashed = store.service.claims[&key].clone();
        assert_eq!(crashed.outcome.as_deref(), Some("unknown"));
        assert!(crashed.spend.is_none());
        assert_eq!(store.service.active, 1);
    }
}
