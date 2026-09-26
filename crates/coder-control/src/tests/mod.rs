use crate::*;
use nostr::control;
use secp256k1::SecretKey;
use tempfile::TempDir;

mod relay;
mod roundtrip;

pub(super) const NOW: u64 = 1_800_000_000;
pub(super) struct Fixture {
    pub directory: TempDir,
    pub tasks: PathBuf,
    pub host_directory: PathBuf,
    pub setup: Setup,
    pub authority: SecretKey,
    pub client: SecretKey,
    pub host: Host,
}
impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let tasks = directory.path().join("tasks");
        let repository = directory.path().join("repository");
        let checkout = directory.path().join("checkout");
        std::fs::create_dir(&repository).unwrap();
        for args in [
            vec!["init", "-q"],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-qm",
                "Fixture",
            ],
        ] {
            assert!(
                std::process::Command::new("git")
                    .args(args)
                    .current_dir(&repository)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        assert!(
            std::process::Command::new("git")
                .args(["worktree", "add", "--detach", "-q"])
                .arg(&checkout)
                .current_dir(&repository)
                .status()
                .unwrap()
                .success()
        );
        let authority = SecretKey::from_byte_array([1; 32]).unwrap();
        let client = SecretKey::from_byte_array([2; 32]).unwrap();
        let command = coder::task::Command {
            schema: coder::task::COMMAND_SCHEMA.into(),
            command_id: "submit".into(),
            task_id: "synthetic".into(),
            expected_revision: None,
            action: coder::task::Action::Submit {
                intent: coder::task::TaskIntent {
                    title: "Control fixture".into(),
                    prompt: "Retain an exact synthetic task".into(),
                    workspace: coder::task::Workspace {
                        path: checkout.canonicalize().unwrap().to_string_lossy().into(),
                        source_revision: None,
                    },
                    configuration: coder::task::RequestedConfiguration {
                        adapter: "bounded-command".into(),
                        model: None,
                    },
                },
            },
        };
        coder::task::Store::open(&tasks)
            .unwrap()
            .apply(&serde_json::to_vec(&command).unwrap())
            .unwrap();
        let setup = Setup::for_task(
            &tasks,
            "synthetic",
            &client::pubkey(&authority),
            &client::pubkey(&authority),
            NOW,
            NOW + 172800,
        )
        .unwrap();
        let host_directory = directory.path().join("control");
        let host = Host::open(&host_directory, setup.clone(), authority).unwrap();
        Self {
            directory,
            tasks,
            host_directory,
            setup,
            authority,
            client,
            host,
        }
    }
    pub fn input(&self, body: &Value, secret: &SecretKey) -> (Event, Value) {
        let event = client::envelope(
            body,
            secret,
            &self.setup.authority,
            NOW,
            self.setup.retain_until,
        )
        .unwrap();
        let r = reference(
            &nostr::contracts::jcs(body).unwrap(),
            "application/json",
            body["v"].as_str().unwrap(),
        );
        (event, r)
    }
    pub fn execute(
        &mut self,
        role: &str,
        body: &Value,
        secret: &SecretKey,
        blobs: &Blobs,
    ) -> (Reply, Value) {
        let (input, r) = self.input(body, secret);
        let request_id = client::random_id();
        let request = client::request(
            secret,
            &self.setup.authority,
            &self.setup.operations[role],
            &input,
            &r,
            NOW,
            NOW + 100,
            self.setup.retain_until,
            &request_id,
        )
        .unwrap();
        let reply = self.host.handle(&request, &input, blobs, NOW + 1).unwrap();
        let result = client::result(
            &reply.result,
            &request,
            &self.setup.authority,
            &request_id,
            secret,
        )
        .unwrap();
        let answer_ref = &result["output"]["artifact"];
        let answer = reply
            .artifacts
            .iter()
            .find_map(|event| {
                client::open_artifact(event, &self.setup.authority, answer_ref, secret).ok()
            })
            .unwrap_or_else(|| panic!("control answer missing: {result}"));
        (reply, answer)
    }
    pub fn pair(&mut self, rights: &[&str]) -> Value {
        let invitation = self
            .host
            .invite(
                &client::pubkey(&self.client),
                &rights.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                NOW,
                NOW + 60,
                NOW + 1000,
            )
            .unwrap();
        let open = nostr::private_artifact::open(&invitation, &self.client).unwrap();
        let body: Value = serde_json::from_slice(open.inline_bytes().unwrap()).unwrap();
        let invitation_ref = reference(
            open.inline_bytes().unwrap(),
            "application/json",
            control::INVITATION,
        );
        let pairing = json!({"v":control::PAIRING,"requires":[],"invitation":invitation_ref,"client":client::pubkey(&self.client),"challenge":body["challenge"],"rights":rights,"accepted":true});
        let secret = self.client;
        let (_, answer) = self.execute("pair", &pairing, &secret, &Blobs::default());
        assert_eq!(answer["status"], "granted");
        answer["access"].clone()
    }
    pub fn read(&self, grant: &Value) -> Value {
        json!({"v":control::READ,"requires":[],"request":client::random_id(),"grant":grant,"epoch":0,"scope":self.setup.scope,"view":"state","after":null,"max_items":2,"max_bytes":32768})
    }
    pub fn command(&self, grant: &Value, action: &str, payload: Value, revision: u64) -> Value {
        json!({"v":control::COMMAND,"requires":[],"command":client::random_id(),"grant":grant,"epoch":0,"scope":self.setup.scope,"expected_revision":revision,"issued_at":NOW,"expires_at":NOW+90,"action":action,"payload":payload})
    }
}

#[test]
fn scoped_observation_correction_cancellation_and_restart() {
    let mut f = Fixture::new();
    let grant = f.pair(&["observe", "steer", "cancel"]);
    let secret = f.client;
    let request = f.read(&grant);
    let (reply, view) = f.execute("read", &request, &secret, &Blobs::default());
    assert_eq!(view["coverage"], "partial");
    assert!(!view["items"].as_array().unwrap().is_empty());
    assert!(
        reply
            .artifacts
            .iter()
            .all(|event| event.pubkey == f.setup.authority)
    );
    let mut blobs = Blobs::default();
    let message = blobs
        .insert(
            b"Use the corrected instruction".to_vec(),
            "text/plain",
            "openagents.control-message.v1",
        )
        .unwrap();
    let correction = f.command(&grant, "steer", json!({"message":message,"replaces":[]}), 1);
    let (_, answer) = f.execute("command", &correction, &secret, &blobs);
    assert_eq!(answer["status"], "accepted");
    let (_, again) = f.execute("command", &correction, &secret, &blobs);
    assert_eq!(answer, again);
    assert_eq!(
        coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap()
            .revision,
        2
    );
    drop(f.host);
    f.host = Host::open(&f.host_directory, f.setup.clone(), f.authority).unwrap();
    let cancel = f.command(
        &grant,
        "cancel",
        json!({"reason":"fixture cancellation"}),
        2,
    );
    let (_, answer) = f.execute("command", &cancel, &secret, &Blobs::default());
    assert_eq!(answer["status"], "accepted");
    assert_eq!(
        coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap()
            .status,
        coder::task::Status::Cancelled
    );
    let (_, view) = f.execute("read", &f.read(&grant), &secret, &Blobs::default());
    assert_eq!(view["coverage"], "partial");
}

#[test]
fn cancel_only_does_not_admit_read_or_steer_and_revocation_survives_restart() {
    let mut f = Fixture::new();
    let grant = f.pair(&["cancel"]);
    let secret = f.client;
    let read = f.read(&grant);
    let (input, r) = f.input(&read, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["read"],
        &input,
        &r,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        "read-denied",
    )
    .unwrap();
    assert!(
        f.host
            .handle(&cj, &input, &Blobs::default(), NOW + 1)
            .is_err()
    );
    let revoke = json!({"v":control::REVOKE,"requires":[],"request":client::random_id(),"grant":grant,"reason":"Withdraw access"});
    let owner = f.authority;
    let (_, answer) = f.execute("revoke", &revoke, &owner, &Blobs::default());
    assert_eq!(answer["status"], "revoked");
    drop(f.host);
    f.host = Host::open(&f.host_directory, f.setup.clone(), f.authority).unwrap();
    let command = f.command(&grant, "cancel", json!({"reason":"late"}), 1);
    let (input, r) = f.input(&command, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["command"],
        &input,
        &r,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        "cancel-denied",
    )
    .unwrap();
    assert!(
        f.host
            .handle(&cj, &input, &Blobs::default(), NOW + 1)
            .is_err()
    );
    assert_eq!(
        coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap()
            .revision,
        1
    );
}

#[test]
fn command_conflicts_and_substituted_envelopes_do_not_mutate_tasks() {
    let mut f = Fixture::new();
    let grant = f.pair(&["cancel"]);
    let secret = f.client;
    let command = f.command(&grant, "cancel", json!({"reason":"stop"}), 1);
    let (input, r) = f.input(&command, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["command"],
        &input,
        &r,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        "cancel",
    )
    .unwrap();
    let (mutated, _) = f.input(&command, &secret);
    assert!(
        f.host
            .handle(&cj, &mutated, &Blobs::default(), NOW + 1)
            .is_err()
    );
    let reply = f
        .host
        .handle(&cj, &input, &Blobs::default(), NOW + 1)
        .unwrap();
    let repeat = f
        .host
        .handle(&cj, &input, &Blobs::default(), NOW + 2)
        .unwrap();
    assert_eq!(
        reply.artifacts.last().unwrap().id,
        repeat.artifacts.last().unwrap().id
    );
    let mut conflict = command.clone();
    conflict["payload"]["reason"] = json!("changed");
    let (_, answer) = f.execute("command", &conflict, &secret, &Blobs::default());
    assert_eq!(answer["status"], "conflict");
    assert_eq!(
        coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap()
            .revision,
        2
    );
}

#[test]
fn setup_changes_and_stale_scope_refuse_without_state_disclosure() {
    let mut f = Fixture::new();
    let grant = f.pair(&["observe"]);
    let secret = f.client;
    let mut read = f.read(&grant);
    read["scope"]["generation"] = json!(1);
    let (input, r) = f.input(&read, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["read"],
        &input,
        &r,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        "scope",
    )
    .unwrap();
    assert!(
        f.host
            .handle(&cj, &input, &Blobs::default(), NOW + 1)
            .is_err()
    );
    drop(f.host);
    let mut changed = f.setup.clone();
    changed.scope["generation"] = json!(1);
    assert!(Host::open(&f.host_directory, changed, f.authority).is_err());
    assert!(f.directory.path().exists());
}

#[test]
fn uncertain_command_reconciles_original_local_id_after_restart() {
    let mut f = Fixture::new();
    let grant = f.pair(&["cancel"]);
    let secret = f.client;
    let command = f.command(&grant, "cancel", json!({"reason":"one durable effect"}), 1);
    let (input, r) = f.input(&command, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["command"],
        &input,
        &r,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        "recover-original",
    )
    .unwrap();
    host::FAIL_AFTER_LOCAL_APPLY.with(|fault| fault.set(true));
    let uncertain = f
        .host
        .handle(&cj, &input, &Blobs::default(), NOW + 1)
        .unwrap();
    let result = client::result(
        &uncertain.result,
        &cj,
        &f.setup.authority,
        "recover-original",
        &secret,
    )
    .unwrap();
    assert_eq!(result["outcome"], "unknown");
    assert!(result["dispatched"].is_null());
    assert_eq!(
        coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap()
            .revision,
        2
    );
    drop(f.host);
    f.host = Host::open(&f.host_directory, f.setup.clone(), f.authority).unwrap();
    let recovered = f
        .host
        .handle(&cj, &input, &Blobs::default(), NOW + 2)
        .unwrap();
    let result = client::result(
        &recovered.result,
        &cj,
        &f.setup.authority,
        "recover-original",
        &secret,
    )
    .unwrap();
    assert_eq!(result["outcome"], "completed");
    assert_eq!(
        coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap()
            .revision,
        2
    );
}

#[test]
fn pairing_consumption_rights_and_expiry_are_current_host_decisions() {
    let mut f = Fixture::new();
    let secret = f.client;
    let invitation = f
        .host
        .invite(
            &client::pubkey(&secret),
            &["observe".into()],
            NOW,
            NOW + 60,
            NOW + 1000,
        )
        .unwrap();
    let open = nostr::private_artifact::open(&invitation, &secret).unwrap();
    let body: Value = serde_json::from_slice(open.inline_bytes().unwrap()).unwrap();
    let r = reference(
        open.inline_bytes().unwrap(),
        "application/json",
        control::INVITATION,
    );
    let mut pairing = json!({"v":control::PAIRING,"requires":[],"invitation":r,"client":client::pubkey(&secret),"challenge":body["challenge"],"rights":["observe","cancel"],"accepted":true});
    let (input, r) = f.input(&pairing, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["pair"],
        &input,
        &r,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        "widen",
    )
    .unwrap();
    assert!(
        f.host
            .handle(&cj, &input, &Blobs::default(), NOW + 1)
            .is_err()
    );
    pairing["rights"] = json!(["observe"]);
    let (_, access) = f.execute("pair", &pairing, &secret, &Blobs::default());
    let grant = access["access"].clone();
    pairing["accepted"] = json!(false);
    let (_, conflict) = f.execute("pair", &pairing, &secret, &Blobs::default());
    assert_eq!(conflict["reason"], "idempotency_conflict");
    let read = f.read(&grant);
    let (input, r) = f.input(&read, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["read"],
        &input,
        &r,
        NOW + 1001,
        NOW + 1100,
        f.setup.retain_until,
        "expired",
    )
    .unwrap();
    assert!(
        f.host
            .handle(&cj, &input, &Blobs::default(), NOW + 1001)
            .is_err()
    );
}

#[test]
fn unsupported_reads_do_not_poison_capacity_or_disclose_a_partial_view() {
    let mut f = Fixture::new();
    let grant = f.pair(&["observe"]);
    let secret = f.client;
    let mut read = f.read(&grant);
    read["max_bytes"] = json!(1048576);
    let (input, r) = f.input(&read, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["read"],
        &input,
        &r,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        "large-read",
    )
    .unwrap();
    let reply = f
        .host
        .handle(&cj, &input, &Blobs::default(), NOW + 1)
        .unwrap();
    let result = client::result(
        &reply.result,
        &cj,
        &f.setup.authority,
        "large-read",
        &secret,
    )
    .unwrap();
    assert_eq!(result["outcome"], "refused");
    assert!(reply.artifacts.is_empty());
    let (_, view) = f.execute("read", &f.read(&grant), &secret, &Blobs::default());
    assert_eq!(view["v"], control::VIEW);
}

#[test]
fn text_carrier_keeps_exact_original_signatures_and_refuses_substitution() {
    let f = Fixture::new();
    let text = client::text(
        "Exact instruction.",
        &f.client,
        &f.setup.authority,
        NOW,
        f.setup.retain_until,
    )
    .unwrap();
    assert_eq!(
        text.verify(&text.reference, &client::pubkey(&f.client), &f.authority)
            .unwrap(),
        b"Exact instruction."
    );
    let mut changed = text.clone();
    changed.carrier = client::text(
        "Other instruction.",
        &f.client,
        &f.setup.authority,
        NOW,
        f.setup.retain_until,
    )
    .unwrap()
    .carrier;
    assert!(
        changed
            .verify(&text.reference, &client::pubkey(&f.client), &f.authority)
            .is_err()
    );
    assert!(
        text.verify(&text.reference, &f.setup.authority, &f.authority)
            .is_err()
    );
    let mut changed_ref = text.reference.clone();
    changed_ref["sources"] = json!([{"url":"https://example.invalid/instruction"}]);
    assert!(
        text.verify(&changed_ref, &client::pubkey(&f.client), &f.authority)
            .is_err()
    );
}

#[test]
fn current_grant_can_retrieve_original_command_result_after_command_expiry() {
    let mut f = Fixture::new();
    let grant = f.pair(&["cancel"]);
    let secret = f.client;
    let command = f.command(&grant, "cancel", json!({"reason":"once"}), 1);
    let (input, r) = f.input(&command, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["command"],
        &input,
        &r,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        "expiry-replay",
    )
    .unwrap();
    let original = f
        .host
        .handle(&cj, &input, &Blobs::default(), NOW + 1)
        .unwrap();
    let retry = f
        .host
        .handle(&cj, &input, &Blobs::default(), NOW + 110)
        .unwrap();
    assert_eq!(
        original.artifacts.last().unwrap().id,
        retry.artifacts.last().unwrap().id
    );
    assert_eq!(
        coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap()
            .revision,
        2
    );
}

#[test]
fn poisoned_final_reply_never_escapes_from_the_same_host() {
    let mut f = Fixture::new();
    let grant = f.pair(&["cancel"]);
    let secret = f.client;
    let command = f.command(
        &grant,
        "cancel",
        json!({"reason":"durable answer required"}),
        1,
    );
    let (input, r) = f.input(&command, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["command"],
        &input,
        &r,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        "failed-save",
    )
    .unwrap();
    host::FAIL_FINAL_REPLY_SAVE.with(|fault| fault.set(true));
    assert!(
        f.host
            .handle(&cj, &input, &Blobs::default(), NOW + 1)
            .is_err()
    );
    assert!(
        f.host
            .handle(&cj, &input, &Blobs::default(), NOW + 2)
            .is_err()
    );
    assert!(f.host.required_text(&cj, &input, NOW + 2).is_err());
    drop(f.host);
    f.host = Host::open(&f.host_directory, f.setup.clone(), f.authority).unwrap();
    let recovered = f
        .host
        .handle(&cj, &input, &Blobs::default(), NOW + 3)
        .unwrap();
    assert_eq!(
        client::result(
            &recovered.result,
            &cj,
            &f.setup.authority,
            "failed-save",
            &secret
        )
        .unwrap()["outcome"],
        "completed"
    );
    assert_eq!(
        coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap()
            .revision,
        2
    );
}

#[test]
fn retained_intent_does_not_dispatch_after_its_cj_deadline() {
    let mut f = Fixture::new();
    let grant = f.pair(&["cancel"]);
    let secret = f.client;
    let command = f.command(
        &grant,
        "cancel",
        json!({"reason":"deadline stays binding"}),
        1,
    );
    let (input, r) = f.input(&command, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["command"],
        &input,
        &r,
        NOW,
        NOW + 10,
        f.setup.retain_until,
        "pending-expired",
    )
    .unwrap();
    host::FAIL_BEFORE_LOCAL_APPLY.with(|fault| fault.set(true));
    let pending = f
        .host
        .handle(&cj, &input, &Blobs::default(), NOW + 1)
        .unwrap();
    assert_eq!(
        client::result(
            &pending.result,
            &cj,
            &f.setup.authority,
            "pending-expired",
            &secret
        )
        .unwrap()["outcome"],
        "unknown"
    );
    drop(f.host);
    f.host = Host::open(&f.host_directory, f.setup.clone(), f.authority).unwrap();
    assert!(
        f.host
            .handle(&cj, &input, &Blobs::default(), NOW + 11)
            .is_err()
    );
    assert_eq!(
        coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap()
            .revision,
        1
    );
}

#[test]
fn text_fetch_preflight_does_not_freeze_command_authority() {
    let mut f = Fixture::new();
    let grant = f.pair(&["steer"]);
    let secret = f.client;
    let text = client::text(
        "Do not apply after the command expires.",
        &secret,
        &f.setup.authority,
        NOW,
        f.setup.retain_until,
    )
    .unwrap();
    let command = f.command(
        &grant,
        "steer",
        json!({"message":text.reference,"replaces":[]}),
        1,
    );
    let (input, r) = f.input(&command, &secret);
    let cj = client::request(
        &secret,
        &f.setup.authority,
        &f.setup.operations["command"],
        &input,
        &r,
        NOW,
        NOW + 100,
        f.setup.retain_until,
        "fetch-expired",
    )
    .unwrap();
    assert!(
        f.host
            .required_text(&cj, &input, NOW + 1)
            .unwrap()
            .is_some()
    );
    assert!(f.host.handle_text(&cj, &input, &text, NOW + 91).is_err());
    assert_eq!(
        coder::task::Store::open(&f.tasks)
            .unwrap()
            .show("synthetic")
            .unwrap()
            .revision,
        1
    );
}

#[test]
fn reopen_refuses_grant_body_corruption_against_original_signature() {
    let mut f = Fixture::new();
    f.pair(&["observe"]);
    drop(f.host);
    let path = f.host_directory.join("control.json");
    let mut state: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for access in state["grants"].as_object_mut().unwrap().values_mut() {
        access["body"]["rights"] = json!(["observe", "cancel"]);
    }
    std::fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    assert!(Host::open(&f.host_directory, f.setup, f.authority).is_err());
}
