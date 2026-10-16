use crate::coordinator::{Bundle, Config, Coordinator, envelope};
use nostr::contracts::{digest_bytes, jcs};
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn pk(n: u8) -> String {
    secret(n).x_only_public_key(&Secp256k1::new()).0.to_string()
}
const COORD: u8 = 1;
const AUTHORITY: u8 = 2;
const AGENT: u8 = 3;
const HOST_A: u8 = 4;
const HOST_B: u8 = 5;
const CHILD: u8 = 6;
const STRANGER: u8 = 7;
const HOST_C: u8 = 8;

fn put(map: &mut BTreeMap<String, Vec<u8>>, bytes: &[u8], schema: Option<&str>) -> Value {
    map.insert(digest_bytes(bytes), bytes.to_vec());
    let mut v =
        json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"});
    if let Some(schema) = schema {
        v["schema"] = json!(schema);
    }
    v
}
fn put_json(map: &mut BTreeMap<String, Vec<u8>>, body: &Value) -> Value {
    put(map, &jcs(body).unwrap(), body["v"].as_str())
}
fn reference(body: &Value) -> Value {
    let bytes = jcs(body).unwrap();
    json!({"digest":digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":body["v"]})
}

struct Fx {
    _temp: tempfile::TempDir,
    c: Coordinator,
    artifacts: BTreeMap<String, Vec<u8>>,
    profile: Value,
    reservation: Value,
    now: u64,
    counter: u8,
}

impl Fx {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let mut artifacts = BTreeMap::new();
        let disclosure = put_json(
            &mut artifacts,
            &json!({"v":"openagents.sovereign-state-disclosure.v1","requires":[],"recipients":[pk(HOST_A),pk(HOST_B)]}),
        );
        let schema = put(&mut artifacts, &serde_json::to_vec(&json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","required":["goal"],"properties":{"goal":{"type":"string"}}})).unwrap(), None);
        let profile = put_json(
            &mut artifacts,
            &json!({"v":"openagents.sovereign-profile.v1","requires":[],"agent":pk(AGENT),
            "authority":pk(AUTHORITY),"revision":0,"previous":null,"disclosure":disclosure,"state_schema":schema}),
        );
        let reservation = put_json(
            &mut artifacts,
            &json!({"v":"openagents.reservation.v1","requires":[],"id":"ab".repeat(32),
            "parent":null,"currency":"SAT","ceiling_microunits":1000,"spent_microunits":0,"unknown":false}),
        );
        Coordinator::initialize(
            temp.path().join("coord"),
            secret(COORD),
            Config {
                authorities: BTreeMap::from([(pk(AGENT), pk(AUTHORITY))]),
                profiles: BTreeMap::from([(pk(AGENT), profile.clone())]),
                hosts: BTreeSet::from([pk(HOST_A), pk(HOST_B), pk(HOST_C)]),
            },
        )
        .unwrap();
        let c = Coordinator::open(temp.path().join("coord"), secret(COORD)).unwrap();
        Self {
            _temp: temp,
            c,
            artifacts,
            profile,
            reservation,
            now: crate::now(),
            counter: 0x30,
        }
    }
    fn next(&mut self) -> String {
        self.counter += 1;
        format!("{:02x}", self.counter).repeat(32)
    }
    fn environment(&mut self, executor: u8) -> Value {
        let art = |s: &str| json!({"digest":digest_bytes(s.as_bytes()),"size":s.len(),"media_type":"application/json","schema":s});
        let plain = |s: &str| json!({"digest":digest_bytes(s.as_bytes()),"size":s.len(),"media_type":"application/json"});
        let lease = json!({"v":"openagents.environment-lease.v1","requires":[],"lease":self.next(),
            "request":art("openagents.environment-request.v1"),"allocator":pk(9),"created_at":self.now,
            "not_after":self.now+3600,"retain_until":self.now+7200,"reservation":art("openagents.reservation.v1")});
        let lease_ref = put_json(&mut self.artifacts, &lease);
        let runtime = json!({"id":format!("{}:runtime/sov",pk(9)),"artifact":plain("runtime")});
        let materialization = json!({"v":"openagents.environment-materialization.v1","requires":[],"lease":lease_ref,
            "definition":art("openagents.environment-definition.v1"),"resource":art("openagents.environment-resource.v1"),
            "generation":self.next(),"components":[{"id":"base","role":"base","artifact":plain("base")}],"runtime":runtime,
            "configuration":plain("config"),"model":null,"workspace":null,
            "participants":[{"principal":pk(executor),"role":"executor","runtime":runtime,"model":null,"configuration":plain("config"),"assurance":plain("a")}],
            "bootstrap":null,"enforcement":art("openagents.environment-enforcement.v1"),"evidence":[],"observed_at":self.now});
        json!({"lease":lease_ref,"materialization":put_json(&mut self.artifacts,&materialization)})
    }
    fn request(&mut self, controller: u8, executor: u8, checkpoint: Value) -> Value {
        let environment = self.environment(executor);
        let nonce = self.next();
        let plan = put(&mut self.artifacts, nonce.as_bytes(), None);
        let claim = put(&mut self.artifacts, b"lifecycle claim", None);
        json!({"v":"openagents.sovereign-activation-request.v1","requires":[],"agent":pk(AGENT),"activation":self.next(),
            "profile":self.profile,"plan":plan,"controller":pk(controller),"claim":claim,"reservation":self.reservation,
            "environment":environment,"dispatchers":[pk(controller),pk(CHILD)],"checkpoint":checkpoint,"expires_at":self.now+3600})
    }
    fn send(&self, signer: u8, body: &Value) -> Value {
        self.c
            .submit(
                &Bundle {
                    event: envelope(&secret(signer), &pk(COORD), body, self.now).unwrap(),
                    artifacts: self.artifacts.clone(),
                },
                self.now,
            )
            .unwrap()
    }
    fn activate(&mut self) -> (String, Value) {
        let body = self.request(HOST_A, HOST_A, Value::Null);
        let result = self.send(AUTHORITY, &body);
        assert_eq!(result["status"], "accepted", "{result}");
        (
            body["activation"].as_str().unwrap().into(),
            result["activation"].clone(),
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn checkpoint(
        &mut self,
        activation: &str,
        reference: &Value,
        controller: u8,
        generation: u64,
        revision: u64,
        previous: Value,
        goal: Value,
    ) -> (Value, Value) {
        let state = put(
            &mut self.artifacts,
            &serde_json::to_vec(&json!({"goal":goal})).unwrap(),
            None,
        );
        let obligation = put(
            &mut self.artifacts,
            format!("claim disposition {revision}").as_bytes(),
            None,
        );
        let prior = if revision == 0 {
            Value::Null
        } else {
            json!(digest_bytes(format!("cut {}", revision - 1).as_bytes()))
        };
        let record = json!({"run":activation,"seq":revision,"previous":prior,"controller":pk(controller),"generation":generation,
            "type":"created","subject":{"step":null,"iteration":null,"attempt":null},"time":self.now,
            "data":{"owner":pk(controller),"request":null,"base":null,"program":"sov","lock":null,"context":null,
            "policy":null,"parent":null,"recipients":[],"state":state["digest"],"obligations":[obligation["digest"]]}});
        let digest = nostr::run::logical_digest(&record).unwrap();
        let envelope =
            json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"record":record,"digest":digest});
        let record_ref = put_json(&mut self.artifacts, &envelope);
        let body = json!({"v":"openagents.sovereign-checkpoint.v1","requires":[],"activation":reference,"revision":revision,
            "previous":previous,"controller":pk(controller),"generation":generation,"record":record_ref,"state":state,
            "obligations":[obligation]});
        let result = self.send(controller, &body);
        (result, reference_of(&body))
    }
    fn transfer(
        &mut self,
        signer: u8,
        from: &Value,
        checkpoint: &Value,
        to_controller: u8,
        executor: u8,
    ) -> (Value, Value) {
        let to = self.request(to_controller, executor, checkpoint.clone());
        let body = json!({"v":"openagents.sovereign-transfer.v1","requires":[],"agent":pk(AGENT),"from":from,
            "checkpoint":checkpoint,"to":to,"reason":"move host"});
        (self.send(signer, &body), body)
    }
    fn fence(
        &mut self,
        dispatcher: u8,
        activation: &Value,
        generation: u64,
        fenced: bool,
        unknown: Vec<Value>,
    ) -> Value {
        let body = json!({"v":"openagents.sovereign-fencing.v1","requires":[],"activation":activation,"generation":generation,
            "dispatcher":pk(dispatcher),"status":if fenced {"fenced"} else {"unreachable"},"unknown":unknown,"observed_at":self.now});
        self.send(dispatcher, &body)
    }
    fn accept(&mut self, transfer: &Value) -> Value {
        let body = json!({"v":"openagents.sovereign-transfer-acceptance.v1","requires":[],"transfer":reference(transfer),
            "controller":transfer["to"]["controller"],"environment":transfer["to"]["environment"]});
        self.send(HOST_B, &body)
    }
    fn control(&mut self, activation: &Value, action: &str) -> Value {
        let body = json!({"v":"openagents.sovereign-lifecycle-control.v1","requires":[],"agent":pk(AGENT),"activation":activation,
            "command":self.next(),"action":action,"reason":""});
        self.send(AUTHORITY, &body)
    }
}

fn reference_of(body: &Value) -> Value {
    reference(body)
}

#[test]
fn one_lifecycle_holds_the_agent_and_its_id_is_bound_forever() {
    let mut fx = Fx::new();
    let body = fx.request(HOST_A, HOST_A, Value::Null);
    assert_eq!(fx.send(STRANGER, &body)["reason"], "not_admitted");
    let unadmitted = fx.request(STRANGER, STRANGER, Value::Null);
    assert_eq!(fx.send(AUTHORITY, &unadmitted)["reason"], "not_admitted");
    let first = fx.send(AUTHORITY, &body);
    assert_eq!(first["status"], "accepted");
    assert_eq!(fx.send(AUTHORITY, &body)["status"], "duplicate");
    let mut changed = body.clone();
    changed["expires_at"] = json!(fx.now + 10);
    assert_eq!(
        fx.send(AUTHORITY, &changed)["reason"],
        "idempotency_conflict"
    );
    let competing = fx.request(HOST_B, HOST_B, Value::Null);
    assert_eq!(fx.send(AUTHORITY, &competing)["reason"], "conflict");
    let id = body["activation"].as_str().unwrap();
    fx.c.check_dispatch(&pk(AGENT), id, &pk(HOST_A), 1, fx.now)
        .unwrap();
    assert!(
        fx.c.check_dispatch(&pk(AGENT), id, &pk(HOST_B), 1, fx.now)
            .is_err()
    );
    // A stopped lifecycle releases the claim, but its ID never reopens.
    assert_eq!(
        fx.control(&first["activation"], "stop")["status"],
        "accepted"
    );
    assert!(
        fx.c.check_dispatch(&pk(AGENT), id, &pk(HOST_A), 1, fx.now)
            .is_err()
    );
    assert_eq!(
        fx.send(AUTHORITY, &changed)["reason"],
        "idempotency_conflict"
    );
}

#[test]
fn checkpoints_come_only_from_the_current_controller_with_bound_records() {
    let mut fx = Fx::new();
    let (id, activation) = fx.activate();
    let (ok, first) = fx.checkpoint(
        &id,
        &activation,
        HOST_A,
        1,
        0,
        Value::Null,
        json!("ship it"),
    );
    assert_eq!(ok["status"], "accepted", "{ok}");
    let (gap, _) = fx.checkpoint(&id, &activation, HOST_A, 1, 2, first.clone(), json!("x"));
    assert_eq!(gap["reason"], "conflict");
    let (bad, _) = fx.checkpoint(&id, &activation, HOST_A, 1, 1, first.clone(), json!(42));
    assert_eq!(bad["reason"], "malformed");
    let (stale, _) = fx.checkpoint(&id, &activation, HOST_A, 0, 1, first.clone(), json!("x"));
    assert_eq!(stale["reason"], "stale");
    let (other, _) = fx.checkpoint(&id, &activation, HOST_B, 1, 1, first, json!("x"));
    assert_eq!(other["reason"], "stale");
}

#[test]
fn a_two_host_transfer_needs_authority_acceptance_and_complete_fencing() {
    let mut fx = Fx::new();
    let (id, activation) = fx.activate();
    let (_, checkpoint) = fx.checkpoint(
        &id,
        &activation,
        HOST_A,
        1,
        0,
        Value::Null,
        json!("migrate"),
    );
    // A discovered host's own signature grants no transfer authority.
    assert_eq!(
        fx.transfer(HOST_B, &activation, &checkpoint, HOST_B, HOST_B)
            .0["reason"],
        "not_admitted"
    );
    // Private state is not disclosed to a host outside the disclosure policy.
    assert_eq!(
        fx.transfer(AUTHORITY, &activation, &checkpoint, HOST_C, HOST_C)
            .0["reason"],
        "not_admitted"
    );
    // A materialization for another runtime refuses.
    assert_eq!(
        fx.transfer(AUTHORITY, &activation, &checkpoint, HOST_B, HOST_A)
            .0["reason"],
        "incompatible"
    );
    let (pending, transfer) = fx.transfer(AUTHORITY, &activation, &checkpoint, HOST_B, HOST_B);
    assert_eq!(pending["status"], "pending", "{pending}");
    // The old controller cannot dispatch while fencing.
    assert!(
        fx.c.check_dispatch(&pk(AGENT), &id, &pk(HOST_A), 1, fx.now)
            .is_err()
    );
    assert_eq!(
        fx.transfer(AUTHORITY, &activation, &checkpoint, HOST_B, HOST_B)
            .0["reason"],
        "conflict"
    );
    assert_eq!(
        fx.fence(HOST_A, &activation, 1, true, vec![])["status"],
        "pending"
    );
    assert_eq!(fx.accept(&transfer)["status"], "pending");
    assert_eq!(
        fx.fence(STRANGER, &activation, 1, true, vec![])["reason"],
        "not_admitted"
    );
    let done = fx.fence(CHILD, &activation, 1, true, vec![]);
    assert_eq!(done["status"], "accepted", "{done}");
    let new_id = transfer["to"]["activation"].as_str().unwrap();
    let new_ref = done["activation"].clone();
    let new_body = fx.c.store().json(&new_ref).unwrap();
    assert_eq!(new_body["generation"], 2);
    assert_eq!(new_body["checkpoint"], checkpoint);
    fx.c.check_dispatch(&pk(AGENT), new_id, &pk(HOST_B), 2, fx.now)
        .unwrap();
    assert!(
        fx.c.check_dispatch(&pk(AGENT), &id, &pk(HOST_A), 1, fx.now)
            .is_err()
    );
    // A late result from the old host is retained as evidence only.
    let (late, _) = fx.checkpoint(
        &id,
        &activation,
        HOST_A,
        1,
        1,
        checkpoint.clone(),
        json!("late"),
    );
    assert_eq!(late["reason"], "stale");
    assert_eq!(
        fx.c.status(&id).unwrap()["late"].as_array().unwrap().len(),
        1
    );
    // The new lifecycle starts its own checkpoint revision zero.
    let (fresh, _) = fx.checkpoint(
        new_id,
        &new_ref,
        HOST_B,
        2,
        0,
        Value::Null,
        json!("resumed"),
    );
    assert_eq!(fresh["status"], "accepted", "{fresh}");
}

#[test]
fn unreachable_dispatchers_and_signer_outages_hold_the_transfer() {
    let mut fx = Fx::new();
    let (id, activation) = fx.activate();
    let (_, checkpoint) = fx.checkpoint(&id, &activation, HOST_A, 1, 0, Value::Null, json!("x"));
    let (_, transfer) = fx.transfer(AUTHORITY, &activation, &checkpoint, HOST_B, HOST_B);
    let effect = put(&mut fx.artifacts, b"payment attempt with no reply", None);
    fx.fence(HOST_A, &activation, 1, true, vec![]);
    assert_eq!(
        fx.fence(CHILD, &activation, 1, false, vec![effect.clone()])["status"],
        "pending"
    );
    assert_eq!(fx.accept(&transfer)["status"], "pending");
    let status = fx.c.status(&id).unwrap();
    assert_eq!(status["status"], "fencing");
    assert_eq!(status["unknown"], json!([effect]));
    // Without target acceptance (its signer is down) nothing moves either;
    // the authority may roll back to the same host under a new generation.
    assert_eq!(fx.control(&activation, "rollback")["status"], "accepted");
    assert!(
        fx.c.check_dispatch(&pk(AGENT), &id, &pk(HOST_A), 1, fx.now)
            .is_err()
    );
    fx.c.check_dispatch(&pk(AGENT), &id, &pk(HOST_A), 2, fx.now)
        .unwrap();
    // Unknown effects survive and block recovery after a stop.
    let (accepted, latest) = fx.checkpoint(&id, &activation, HOST_A, 2, 1, checkpoint, json!("y"));
    assert_eq!(accepted["status"], "accepted", "{accepted}");
    fx.control(&activation, "stop");
    let recovery = fx.request(HOST_B, HOST_B, latest);
    assert_eq!(fx.send(AUTHORITY, &recovery)["reason"], "conflict");
}

#[test]
fn recovery_needs_a_complete_chain_from_a_stopped_unrevoked_lifecycle() {
    let mut fx = Fx::new();
    let (id, activation) = fx.activate();
    let (_, first) = fx.checkpoint(&id, &activation, HOST_A, 1, 0, Value::Null, json!("a"));
    let (accepted, second) =
        fx.checkpoint(&id, &activation, HOST_A, 1, 1, first.clone(), json!("b"));
    assert_eq!(accepted["status"], "accepted", "{accepted}");
    let early = fx.request(HOST_B, HOST_B, second.clone());
    assert_eq!(fx.send(AUTHORITY, &early)["reason"], "conflict");
    fx.control(&activation, "stop");
    let stale = fx.request(HOST_B, HOST_B, first);
    let r = fx.send(AUTHORITY, &stale);
    assert_eq!(r["reason"], "stale", "{r}");
    let recovery = fx.request(HOST_B, HOST_B, second.clone());
    let recovered = fx.send(AUTHORITY, &recovery);
    assert_eq!(recovered["status"], "accepted", "{recovered}");
    assert_eq!(
        fx.c.store().json(&recovered["activation"]).unwrap()["generation"],
        2
    );
    // A revoked lifecycle cannot be recovered.
    fx.control(&recovered["activation"], "revoke");
    let mut fx2 = Fx::new();
    let (id2, activation2) = fx2.activate();
    let (_, cp) = fx2.checkpoint(&id2, &activation2, HOST_A, 1, 0, Value::Null, json!("z"));
    fx2.control(&activation2, "revoke");
    let blocked = fx2.request(HOST_B, HOST_B, cp);
    assert_eq!(fx2.send(AUTHORITY, &blocked)["reason"], "revoked");
}

#[test]
fn expiry_releases_the_claim_and_stops_dispatch() {
    let mut fx = Fx::new();
    let (id, _) = fx.activate();
    assert_eq!(fx.c.expire(fx.now + 7200).unwrap(), vec![id.clone()]);
    assert!(
        fx.c.check_dispatch(&pk(AGENT), &id, &pk(HOST_A), 1, fx.now)
            .is_err()
    );
    let next = fx.request(HOST_B, HOST_B, Value::Null);
    assert_eq!(fx.send(AUTHORITY, &next)["status"], "accepted");
}

#[test]
fn activation_ids_cannot_replace_another_agents_controller() {
    let mut fx = Fx::new();
    let agent_b = pk(10);
    let authority_b = pk(11);
    let mut profile_b: Value =
        serde_json::from_slice(&fx.artifacts[fx.profile["digest"].as_str().unwrap()]).unwrap();
    profile_b["agent"] = json!(agent_b);
    profile_b["authority"] = json!(authority_b);
    let profile_b = put_json(&mut fx.artifacts, &profile_b);
    let root = fx._temp.path().join("two-agents");
    Coordinator::initialize(
        root.clone(),
        secret(COORD),
        Config {
            authorities: BTreeMap::from([
                (pk(AGENT), pk(AUTHORITY)),
                (agent_b.clone(), authority_b),
            ]),
            profiles: BTreeMap::from([
                (pk(AGENT), fx.profile.clone()),
                (agent_b.clone(), profile_b.clone()),
            ]),
            hosts: BTreeSet::from([pk(HOST_A), pk(HOST_B)]),
        },
    )
    .unwrap();
    fx.c = Coordinator::open(root, secret(COORD)).unwrap();
    let first = fx.request(HOST_A, HOST_A, Value::Null);
    assert_eq!(fx.send(AUTHORITY, &first)["status"], "accepted");
    let id = first["activation"].as_str().unwrap();
    let mut other = fx.request(HOST_B, HOST_B, Value::Null);
    let next = other["activation"].clone();
    other["agent"] = json!(agent_b);
    other["profile"] = profile_b;
    other["activation"] = first["activation"].clone();
    let refused = fx.send(11, &other);
    assert_eq!(refused["reason"], "conflict", "{refused}");
    fx.c.check_dispatch(&pk(AGENT), id, &pk(HOST_A), 1, fx.now)
        .unwrap();
    assert!(
        fx.c.check_dispatch(&pk(AGENT), id, &pk(HOST_B), 1, fx.now)
            .is_err()
    );
    assert!(
        fx.c.check_dispatch(&pk(10), id, &pk(HOST_B), 1, fx.now)
            .is_err()
    );
    other["activation"] = next;
    assert_eq!(fx.send(11, &other)["status"], "accepted");
    fx.c.check_dispatch(&pk(AGENT), id, &pk(HOST_A), 1, fx.now)
        .unwrap();
}

#[test]
fn activation_placement_must_be_current_and_cover_its_entire_lifetime() {
    for case in [
        "expired-lease",
        "future-lease",
        "future-observation",
        "overlong-activation",
    ] {
        let mut fx = Fx::new();
        let mut request = fx.request(HOST_A, HOST_A, Value::Null);
        let mut lease: Value = serde_json::from_slice(
            &fx.artifacts[request["environment"]["lease"]["digest"].as_str().unwrap()],
        )
        .unwrap();
        let mut material: Value = serde_json::from_slice(
            &fx.artifacts[request["environment"]["materialization"]["digest"]
                .as_str()
                .unwrap()],
        )
        .unwrap();
        match case {
            "expired-lease" => {
                lease["created_at"] = json!(fx.now - 200);
                lease["not_after"] = json!(fx.now - 1);
            }
            "future-lease" => {
                lease["created_at"] = json!(fx.now + 10);
            }
            "future-observation" => {
                material["observed_at"] = json!(fx.now + 10);
            }
            "overlong-activation" => {
                request["expires_at"] = json!(fx.now + 3601);
            }
            _ => unreachable!(),
        }
        let lease = put_json(&mut fx.artifacts, &lease);
        material["lease"] = lease.clone();
        request["environment"]["lease"] = lease;
        request["environment"]["materialization"] = put_json(&mut fx.artifacts, &material);
        let result = fx.send(AUTHORITY, &request);
        assert_eq!(result["status"], "refused", "{case}: {result}");
        assert!(
            fx.c.check_dispatch(
                &pk(AGENT),
                request["activation"].as_str().unwrap(),
                &pk(HOST_A),
                1,
                fx.now
            )
            .is_err()
        );
    }
}

#[test]
fn exact_activation_replay_does_not_bypass_its_authority() {
    let mut fx = Fx::new();
    let request = fx.request(HOST_A, HOST_A, Value::Null);
    assert_eq!(fx.send(AUTHORITY, &request)["status"], "accepted");
    assert_eq!(fx.send(STRANGER, &request)["reason"], "not_admitted");
    assert_eq!(fx.send(AUTHORITY, &request)["status"], "duplicate");
}

#[test]
fn a_delayed_transfer_rechecks_placement_and_retains_fencing_evidence() {
    let mut fx = Fx::new();
    let (id, activation) = fx.activate();
    let (_, checkpoint) = fx.checkpoint(
        &id,
        &activation,
        HOST_A,
        1,
        0,
        Value::Null,
        json!("transfer"),
    );
    let (pending, transfer) = fx.transfer(AUTHORITY, &activation, &checkpoint, HOST_B, HOST_B);
    assert_eq!(pending["status"], "pending");
    assert_eq!(
        fx.fence(HOST_A, &activation, 1, true, vec![])["status"],
        "pending"
    );
    assert_eq!(fx.accept(&transfer)["status"], "pending");
    fx.now += 3601;
    let unresolved = put(&mut fx.artifacts, b"unresolved child effect", None);
    let refused = fx.fence(CHILD, &activation, 1, true, vec![unresolved.clone()]);
    assert_eq!(refused["reason"], "stale", "{refused}");
    assert!(
        fx.c.check_dispatch(&pk(AGENT), &id, &pk(HOST_A), 1, fx.now)
            .is_err()
    );
    assert!(
        fx.c.check_dispatch(
            &pk(AGENT),
            transfer["to"]["activation"].as_str().unwrap(),
            &pk(HOST_B),
            2,
            fx.now
        )
        .is_err()
    );
    let signer =
        nostr::domain::RelaySigner::from_secret_hex(&secret(COORD).display_secret().to_string())
            .unwrap();
    let saved: Value = fx.c.store().load(&signer).unwrap();
    assert_eq!(
        saved["activations"][&id]["fencing"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(saved["activations"][&id]["status"], "fencing");
    assert_eq!(saved["activations"][&id]["unknown"], json!([unresolved]));
}

#[test]
fn transferred_activation_replays_its_original_target_request() {
    let mut fx = Fx::new();
    let (id, activation) = fx.activate();
    let (_, checkpoint) = fx.checkpoint(
        &id,
        &activation,
        HOST_A,
        1,
        0,
        Value::Null,
        json!("transfer"),
    );
    let (_, transfer) = fx.transfer(AUTHORITY, &activation, &checkpoint, HOST_B, HOST_B);
    fx.fence(HOST_A, &activation, 1, true, vec![]);
    fx.accept(&transfer);
    let admitted = fx.fence(CHILD, &activation, 1, true, vec![]);
    assert_eq!(admitted["status"], "accepted");
    let replay = fx.send(AUTHORITY, &transfer["to"]);
    assert_eq!(replay["status"], "duplicate", "{replay}");
    assert_eq!(replay["activation"], admitted["activation"]);
}
