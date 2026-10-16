use crate::{
    allocator::{Allocator, Bundle, MaterializeSpec, Policy, envelope, reference},
    local::LocalDirectories,
    participant::{Admit, Participant, intake},
    provider::{Cost, Create, Lookup, Provider, Realized, Teardown},
};
use nostr::{
    contracts::{digest_bytes, digest_value, jcs},
    domain::{Event, RelaySigner, Tag},
    execution,
};
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    rc::Rc,
};

fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn pk(n: u8) -> String {
    secret(n).x_only_public_key(&Secp256k1::new()).0.to_string()
}
const OWNER: u8 = 1;
const CONTROLLER: u8 = 2;
const WORKER: u8 = 3;
const INFERENCE: u8 = 4;
const READER: u8 = 6;
const ALLOCATOR: u8 = 9;

#[derive(Default)]
struct Faults {
    lose_create: bool,
    destroy_unknown: bool,
    fail_collect: bool,
    drop_component: Option<String>,
    creates: usize,
    destroys: usize,
}

struct Faulty {
    inner: LocalDirectories,
    faults: Rc<RefCell<Faults>>,
}

impl Provider for Faulty {
    fn binding(&self) -> String {
        self.inner.binding()
    }
    fn create(&mut self, account: &str, key: &str) -> Create {
        self.faults.borrow_mut().creates += 1;
        let created = self.inner.create(account, key);
        if self.faults.borrow().lose_create {
            return Create::Unknown("response lost".into());
        }
        created
    }
    fn lookup(&mut self, account: &str, key: &str) -> Lookup {
        self.inner.lookup(account, key)
    }
    fn adopt(&mut self, a: &str, r: &str, g: &str, o: &[u8]) -> crate::Result<Realized> {
        self.inner.adopt(a, r, g, o)
    }
    fn materialize(
        &mut self,
        resource: &Realized,
        components: &[(String, Vec<u8>)],
    ) -> crate::Result<Vec<(String, String)>> {
        let drop = self.faults.borrow().drop_component.clone();
        let kept: Vec<_> = components
            .iter()
            .filter(|(id, _)| Some(id) != drop.as_ref())
            .cloned()
            .collect();
        self.inner.materialize(resource, &kept)
    }
    fn collect(&mut self, resource: &Realized) -> crate::Result<Vec<(String, Vec<u8>)>> {
        if self.faults.borrow().fail_collect {
            return Err("unavailable: retention store offline".into());
        }
        self.inner.collect(resource)
    }
    fn destroy(&mut self, resource: &Realized) -> Teardown {
        self.faults.borrow_mut().destroys += 1;
        if self.faults.borrow().destroy_unknown {
            return Teardown::Unknown("provider timeout".into());
        }
        self.inner.destroy(resource)
    }
    fn reconcile_teardown(&mut self, resource: &Realized, action: &str) -> Teardown {
        self.inner.reconcile_teardown(resource, action)
    }
    fn detach(&mut self, resource: &Realized) -> Teardown {
        self.inner.detach(resource)
    }
}

// Explicit test authority for isolated ENV state-machine fixtures. Actual
// COORD integration fixtures use the real authority and reject opaque claims.
struct FixtureClaims;
impl crate::claims::ClaimAuthority for FixtureClaims {
    fn check(
        &self,
        _: &Value,
        _: &str,
        _: &str,
        _: &Value,
        _: &Value,
        _: &Value,
        _: u64,
    ) -> crate::Result<()> {
        Ok(())
    }
}
struct Fixture {
    _temp: tempfile::TempDir,
    state: PathBuf,
    resources: PathBuf,
    faults: Rc<RefCell<Faults>>,
    artifacts: BTreeMap<String, Vec<u8>>,
    definition: Value,
    reservation: Value,
    now: u64,
}

fn put(map: &mut BTreeMap<String, Vec<u8>>, bytes: &[u8], schema: Option<&str>) -> Value {
    map.insert(digest_bytes(bytes), bytes.to_vec());
    let mut value =
        json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"});
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}
fn put_json(map: &mut BTreeMap<String, Vec<u8>>, body: &Value) -> Value {
    put(map, &jcs(body).unwrap(), body["v"].as_str())
}
fn bound(name: &str, ceiling: u64) -> Value {
    let mut value =
        json!({"bound":name,"ceiling":ceiling,"assurance":"host","mechanism":"env-host"});
    if name == "spend_microunits" {
        value["currency"] = json!("SAT");
    }
    value
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("state");
        let resources = temp.path().join("resources");
        let policy = Policy {
            owners: BTreeSet::from([pk(OWNER)]),
            controllers: BTreeSet::from([pk(CONTROLLER)]),
            readers: BTreeSet::from([pk(READER)]),
            account: "local".into(),
            max_ttl_seconds: 3600,
            max_lifetime_seconds: 86_400,
        };
        Allocator::initialize(state.clone(), secret(ALLOCATOR), policy).unwrap();
        let mut artifacts = BTreeMap::new();
        let requirements = put_json(
            &mut artifacts,
            &json!({"v":"openagents.environment-requirements.v1","requires":[],
                "recipients":[pk(WORKER),pk(INFERENCE)],"assurance":"host","opaque_models":false,
                "bounds":["wall_ms"]}),
        );
        let weights = put(&mut artifacts, b"fake weights", None);
        let model = put_json(
            &mut artifacts,
            &json!({"v":"openagents.environment-model.v1","requires":[],"name":"fixture-model",
                "identity":"verified","weights":weights}),
        );
        let base = put(&mut artifacts, b"base image bytes", None);
        let toolchain = put(&mut artifacts, b"toolchain bytes", None);
        let runtime = json!({"id":format!("{}:runtime/coder",pk(OWNER)),"artifact":put(&mut artifacts,b"runtime",None)});
        let configuration = put(&mut artifacts, b"config", None);
        let workspace = put(&mut artifacts, b"workspace snapshot", None);
        let definition = json!({"v":"openagents.environment-definition.v1","requires":[],
            "components":[{"id":"base","role":"base","artifact":base},{"id":"tools","role":"toolchain","artifact":toolchain}],
            "workspace":workspace,"bootstrap":null,"runtime":runtime,"configuration":configuration,
            "model":model,"requirements":requirements});
        let reservation = json!({"v":"openagents.reservation.v1","requires":[],"id":"aa".repeat(32),
            "parent":null,"currency":"SAT","ceiling_microunits":10_000,"spent_microunits":0,"unknown":false});
        put_json(&mut artifacts, &definition);
        put_json(&mut artifacts, &reservation);
        put(&mut artifacts, b"frame", None);
        put(&mut artifacts, b"policy", None);
        put(&mut artifacts, b"claim", None);
        put(&mut artifacts, b"assurance", None);
        Self {
            _temp: temp,
            state,
            resources,
            faults: Rc::default(),
            artifacts,
            definition,
            reservation,
            now: crate::now(),
        }
    }
    fn allocator(&self) -> Allocator {
        let provider = Faulty {
            inner: LocalDirectories::new(self.resources.clone()).unwrap(),
            faults: self.faults.clone(),
        };
        Allocator::open(self.state.clone(), secret(ALLOCATOR), Box::new(provider))
            .unwrap()
            .with_claims(Box::new(FixtureClaims))
    }
    fn request_body(&mut self, id: u8, spend: u64, ttl: u64) -> Value {
        let mut artifacts = std::mem::take(&mut self.artifacts);
        let body = json!({"v":"openagents.environment-request.v1","requires":[],
            "request":format!("{:02x}",id).repeat(32),"run":format!("{:02x}",id.wrapping_add(100)).repeat(32),
            "owner":pk(OWNER),"allocator":pk(ALLOCATOR),
            "definition":put_json(&mut artifacts,&self.definition),
            "frame":put(&mut artifacts,b"frame",None),"policy":put(&mut artifacts,b"policy",None),
            "reservation":put_json(&mut artifacts,&self.reservation),"mode":"create","resource":null,
            "effects":{"reads":[],"writes":["workspace"],"network":[],"process":true,"delegates":false,"spend":false},
            "bounds":[bound("wall_ms",600_000),bound("spend_microunits",spend)],
            "ttl_seconds":ttl,"idle_seconds":ttl,"not_after":self.now+7200,"retain_until":self.now+90_000,
            "cleanup":"destroy"});
        self.artifacts = artifacts;
        body
    }
    fn bundle(&self, signer: u8, body: &Value) -> Bundle {
        Bundle {
            event: envelope(&secret(signer), &pk(ALLOCATOR), body, self.now).unwrap(),
            artifacts: self.artifacts.clone(),
            declarations: Vec::new(),
        }
    }
    fn participants(&mut self) -> Vec<Value> {
        let mut artifacts = std::mem::take(&mut self.artifacts);
        let assurance = put(&mut artifacts, b"assurance", None);
        let tool = json!({"id":format!("{}:runtime/inference",pk(OWNER)),"artifact":put(&mut artifacts,b"inference runtime",None)});
        self.artifacts = artifacts;
        vec![
            json!({"principal":pk(WORKER),"role":"executor","runtime":self.definition["runtime"],
                "model":self.definition["model"],"configuration":self.definition["configuration"],"assurance":assurance}),
            json!({"principal":pk(INFERENCE),"role":"inference","runtime":tool,
                "model":self.definition["model"],"configuration":self.definition["configuration"],"assurance":assurance}),
        ]
    }
    fn spec(&mut self) -> MaterializeSpec {
        MaterializeSpec {
            participants: self.participants(),
            enforcement: json!([bound("wall_ms", 600_000)]),
            evidence: Vec::new(),
        }
    }
    /// Create a lease and materialize it; returns (lease ID, lease ref).
    fn ready(&mut self, allocator: &Allocator, id: u8) -> (String, Value) {
        let body = self.request_body(id, 4000, 1800);
        let result = allocator
            .submit(&self.bundle(OWNER, &body), self.now)
            .unwrap();
        assert_eq!(result["status"], "accepted", "{result}");
        let lease = allocator.store().json(&result["lease"]).unwrap();
        let lease_id = lease["lease"].as_str().unwrap().to_string();
        let spec = self.spec();
        allocator.materialize(&lease_id, &spec, self.now).unwrap();
        (lease_id, result["lease"].clone())
    }
}

/// The caller's signed CJ execute, its declaration, and the attach request.
struct Job {
    event: Event,
    payload: Value,
    declaration_event: Event,
    attach: Value,
}

fn job(
    fx: &mut Fixture,
    allocator: &Allocator,
    lease_id: &str,
    request: &str,
    claim: &[u8],
) -> Job {
    let status = allocator.status(lease_id).unwrap();
    let state = allocator.store().json(&status["state"]).unwrap();
    let deadline = fx.now + 600;
    let payload = json!({"v":execution::SCHEMA,"requires":[nostr::environment::BINDING_FEATURE],
        "type":"execute","request":request,"attempt":1,"run":request,
        "target":{"id":format!("{}:pkg/op",pk(OWNER)),"artifact":{"digest":digest_bytes(b"target"),"size":6,"media_type":"application/octet-stream"}},
        "lock":{"digest":digest_bytes(b"lock"),"size":4,"media_type":"application/octet-stream"},
        "input":{"task":"repair"},
        "context":{"digest":digest_bytes(b"context"),"size":7,"media_type":"application/octet-stream"},
        "requirements":{"digest":digest_bytes(b"requirements"),"size":12,"media_type":"application/octet-stream"},
        "bounds":{"output_bytes":1024},"deadline":deadline,"retain_until":deadline+3600});
    let signer =
        RelaySigner::from_secret_hex(&secret(CONTROLLER).display_secret().to_string()).unwrap();
    let event = execution::Seal {
        signer: &signer,
        conversation: nostr::nip44::conversation_key(
            &secret(CONTROLLER),
            &pk(WORKER).parse().unwrap(),
        ),
        nonce: [7; 32],
        created_at: fx.now,
    }
    .event(
        execution::REQUEST_KIND,
        vec![
            Tag::new(vec!["p".into(), pk(WORKER)]),
            Tag::new(vec!["expiration".into(), deadline.to_string()]),
        ],
        &payload,
    )
    .unwrap();
    let body_ref = put_json(&mut fx.artifacts, &payload);
    let declaration = json!({"v":"openagents.environment-execute.v1","requires":[],"caller":pk(CONTROLLER),
        "event":{"id":event.id,"pubkey":event.pubkey,"kind":event.kind},"body":body_ref});
    let declaration_ref = reference(&declaration).unwrap();
    let declaration_event =
        envelope(&secret(CONTROLLER), &pk(ALLOCATOR), &declaration, fx.now).unwrap();
    let claim = put(&mut fx.artifacts, claim, None);
    let reservation = put_json(
        &mut fx.artifacts,
        &allocator
            .store()
            .json(&allocator.store().json(&status["lease"]).unwrap()["reservation"])
            .unwrap(),
    );
    let attach = json!({"v":"openagents.environment-attach.v1","requires":[],"lease":status["lease"],
        "expected_revision":state["revision"],"materialization":state["materialization"],
        "worker":pk(WORKER),"controller":pk(CONTROLLER),"generation":1,"execute":declaration_ref,
        "input_digest":digest_value(&payload["input"]).unwrap(),"claim":claim,"reservation":reservation});
    Job {
        event,
        payload,
        declaration_event,
        attach,
    }
}

fn attach(fx: &Fixture, allocator: &Allocator, job: &Job) -> Value {
    let mut bundle = fx.bundle(CONTROLLER, &job.attach);
    bundle.declarations.push(job.declaration_event.clone());
    allocator.submit(&bundle, fx.now).unwrap()
}

fn admit_all(fx: &mut Fixture, allocator: &Allocator, job: &Job, grant_ref: &Value) {
    let grant = allocator.store().json(grant_ref).unwrap();
    let materialization = allocator.store().json(&grant["materialization"]).unwrap();
    for (n, projection) in [
        (WORKER, b"worker view".as_slice()),
        (INFERENCE, b"model view"),
    ] {
        let projection_ref = put(&mut fx.artifacts, projection, None);
        let input = json!({"v":"openagents.environment-participant-input.v1","requires":[],
            "attachment":grant_ref,"participant":pk(n),"projection":projection_ref});
        let input_ref = put_json(&mut fx.artifacts, &input);
        let enforcement = materialization["enforcement"].clone();
        let participant = Participant::new(secret(n));
        let event = participant
            .admit(
                &pk(ALLOCATOR),
                &Admit {
                    grant: &grant,
                    grant_ref,
                    attach: &job.attach,
                    input: &input,
                    input_ref: &input_ref,
                    materialization: &materialization,
                    materialization_ref: &grant["materialization"],
                    projection,
                    enforcement: &enforcement,
                    execute: (n == WORKER).then_some((&job.event, &job.payload)),
                },
                fx.now,
            )
            .unwrap();
        let result = allocator
            .submit(
                &Bundle {
                    event,
                    artifacts: fx.artifacts.clone(),
                    declarations: Vec::new(),
                },
                fx.now,
            )
            .unwrap();
        assert_eq!(result["status"], "accepted", "{result}");
    }
}

fn attachment_control(
    fx: &Fixture,
    allocator: &Allocator,
    grant_ref: &Value,
    action: &str,
    command: u8,
    signer: u8,
) -> Value {
    let lease = allocator.store().json(grant_ref).unwrap()["lease"].clone();
    let id = allocator
        .leases()
        .unwrap()
        .into_iter()
        .find(|id| allocator.status(id).unwrap()["lease"] == lease)
        .unwrap();
    let revision = allocator.status(&id).unwrap()["revision"].clone();
    let body = json!({"v":"openagents.environment-attachment-control.v1","requires":[],
        "attachment":grant_ref,"command":format!("{command:02x}").repeat(32),
        "expected_revision":revision,"action":action,"evidence":[]});
    allocator.submit(&fx.bundle(signer, &body), fx.now).unwrap()
}

fn control(
    fx: &Fixture,
    allocator: &Allocator,
    lease_id: &str,
    action: &str,
    until: Option<u64>,
    command: u8,
    signer: u8,
) -> Value {
    let status = allocator.status(lease_id).unwrap();
    let body = json!({"v":"openagents.environment-control.v1","requires":[],"lease":status["lease"],
        "command":format!("{command:02x}").repeat(32),"expected_revision":status["revision"],
        "action":action,"requested_until":until,"evidence":[]});
    allocator.submit(&fx.bundle(signer, &body), fx.now).unwrap()
}

fn fence(fx: &Fixture, allocator: &Allocator, grant_ref: &Value, n: u8, fenced: bool) -> Value {
    let event = Participant::new(secret(n))
        .fence(&pk(ALLOCATOR), grant_ref, fenced, fx.now)
        .unwrap();
    allocator
        .submit(
            &Bundle {
                event,
                artifacts: BTreeMap::new(),
                declarations: Vec::new(),
            },
            fx.now,
        )
        .unwrap()
}

#[test]
fn an_execute_dispatches_only_under_an_active_admitted_attachment() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let (lease_id, _) = fx.ready(&allocator, 1);
    assert_eq!(allocator.status(&lease_id).unwrap()["status"], "ready");
    let job = job(&mut fx, &allocator, &lease_id, "job-1", b"claim");
    // A signed execute arriving before its attachment is not dispatched.
    let fingerprint = digest_value(&job.payload).unwrap();
    assert!(
        intake(&job.event, &secret(WORKER), "none", fx.now, |a, f| {
            allocator.check_dispatch(a, f, fx.now)
        })
        .is_err()
    );
    // A generic worker refuses the required environment binding outright.
    assert!(
        execution::open_request(
            &job.event,
            &pk(WORKER),
            &secret(WORKER),
            fx.now,
            execution::Window::DEFAULT
        )
        .is_err()
    );

    let reserved = attach(&fx, &allocator, &job);
    assert_eq!(reserved["status"], "accepted", "{reserved}");
    let grant_ref = reserved["value"].clone();
    let grant = allocator.store().json(&grant_ref).unwrap();
    let attachment_id = grant["attachment"].as_str().unwrap().to_string();
    // An exact retry returns the original grant; changed bytes conflict.
    assert_eq!(attach(&fx, &allocator, &job), reserved);
    let mut changed = Job {
        attach: job.attach.clone(),
        ..job
    };
    changed.attach["claim"] = put(&mut fx.artifacts, b"other claim", None);
    assert_eq!(
        attach(&fx, &allocator, &changed)["reason"],
        "idempotency_conflict"
    );
    let job = changed;
    let job = Job {
        attach: reserved_request(&allocator, &grant),
        ..job
    };

    // Activation needs every participant's admission first.
    assert_eq!(
        attachment_control(&fx, &allocator, &grant_ref, "activate", 0x31, CONTROLLER)["reason"],
        "not_admitted"
    );
    assert!(
        allocator
            .check_dispatch(&attachment_id, &fingerprint, fx.now)
            .is_err()
    );
    admit_all(&mut fx, &allocator, &job, &grant_ref);
    let active = attachment_control(&fx, &allocator, &grant_ref, "activate", 0x32, CONTROLLER);
    assert_eq!(active["status"], "accepted", "{active}");
    let execute = intake(
        &job.event,
        &secret(WORKER),
        &attachment_id,
        fx.now,
        |a, f| allocator.check_dispatch(a, f, fx.now),
    )
    .unwrap();
    assert_eq!(execute.fingerprint, fingerprint);

    // The state survives a restart with the same authority.
    drop(allocator);
    let allocator = fx.allocator();
    allocator
        .check_dispatch(&attachment_id, &fingerprint, fx.now)
        .unwrap();
    assert!(
        allocator
            .check_dispatch(&attachment_id, &fingerprint, fx.now + 3600)
            .is_err()
    );

    // Draining blocks dispatch; only full fencing frees the slot.
    let drained = attachment_control(&fx, &allocator, &grant_ref, "drain", 0x33, CONTROLLER);
    assert_eq!(drained["status"], "accepted");
    assert!(
        allocator
            .check_dispatch(&attachment_id, &fingerprint, fx.now)
            .is_err()
    );
    fence(&fx, &allocator, &grant_ref, WORKER, true);
    assert_eq!(allocator.status(&lease_id).unwrap()["status"], "draining");
    fence(&fx, &allocator, &grant_ref, INFERENCE, true);
    assert_eq!(allocator.status(&lease_id).unwrap()["status"], "ready");

    // Cleanup retains outputs outside the resource before destroying it.
    let status = allocator.status(&lease_id).unwrap();
    let resource = allocator
        .store()
        .json(&allocator.store().json(&status["state"]).unwrap()["materialization"])
        .unwrap()["resource"]
        .clone();
    let resource = allocator.store().json(&resource).unwrap();
    let dir = fx.resources.join(resource["resource"].as_str().unwrap());
    assert!(dir.join("components/base").is_file());
    std::fs::create_dir_all(dir.join("out")).unwrap();
    std::fs::write(dir.join("out/patch.diff"), b"unpushed output").unwrap();
    let released = control(&fx, &allocator, &lease_id, "release", None, 0x41, OWNER);
    assert_eq!(released["status"], "accepted", "{released}");
    let status = allocator.status(&lease_id).unwrap();
    assert_eq!(status["status"], "released");
    assert_eq!(status["held"], 0);
    assert!(!dir.exists());
    let cleanup = allocator.store().json(&status["cleanup"]).unwrap();
    assert_eq!(cleanup["status"], "confirmed");
    assert!(
        cleanup["retained"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |r| allocator.store().get(r).ok().as_deref() == Some(b"unpushed output".as_slice())
            )
    );
    // Every allocator artifact resolves to a signed declaration.
    let declaration = allocator.declaration(&status["state"]).unwrap();
    assert_eq!(declaration.pubkey, pk(ALLOCATOR));
}

fn reserved_request(allocator: &Allocator, grant: &Value) -> Value {
    allocator.store().json(&grant["request"]).unwrap()
}

#[test]
fn two_attachments_race_for_one_slot() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let (lease_id, _) = fx.ready(&allocator, 2);
    let first = job(&mut fx, &allocator, &lease_id, "job-a", b"claim-a");
    let second = job(&mut fx, &allocator, &lease_id, "job-b", b"claim-b");
    assert_eq!(attach(&fx, &allocator, &first)["status"], "accepted");
    assert_eq!(attach(&fx, &allocator, &second)["status"], "conflict");
}

#[test]
fn a_lost_create_is_reconciled_never_repeated() {
    let mut fx = Fixture::new();
    fx.faults.borrow_mut().lose_create = true;
    let allocator = fx.allocator();
    let body = fx.request_body(3, 1000, 600);
    let result = allocator.submit(&fx.bundle(OWNER, &body), fx.now).unwrap();
    assert_eq!(result["status"], "unknown");
    // A transport retry returns the retained result; nothing is created again.
    assert_eq!(
        allocator.submit(&fx.bundle(OWNER, &body), fx.now).unwrap(),
        result
    );
    assert_eq!(fx.faults.borrow().creates, 1);
    let lease_id = allocator.leases().unwrap()[0].clone();
    assert!(allocator.cleanup(&lease_id, fx.now).is_err());
    drop(allocator);
    let allocator = fx.allocator();
    let reconciled = allocator.reconcile(&lease_id, fx.now).unwrap();
    assert_eq!(reconciled["status"], "provisioning");
    let state = allocator.store().json(&reconciled["state"]).unwrap();
    let accounting = allocator.store().json(&state["accounting"]).unwrap();
    assert!(
        accounting["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["phase"] == "allocate" && e["kind"] == "unknown")
    );
    assert!(
        !accounting["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["phase"] == "allocate" && e["kind"] == "measured")
    );
    assert_eq!(fx.faults.borrow().creates, 1);
    assert_eq!(std::fs::read_dir(&fx.resources).unwrap().count(), 1);
    // A changed request under the same identity conflicts.
    let mut changed = body.clone();
    changed["ttl_seconds"] = json!(601);
    changed["idle_seconds"] = json!(601);
    assert_eq!(
        allocator
            .submit(&fx.bundle(OWNER, &changed), fx.now)
            .unwrap()["status"],
        "conflict"
    );
}

#[test]
fn expiry_drains_and_cannot_be_renewed_or_attached() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let (lease_id, _) = fx.ready(&allocator, 4);
    let job = job(&mut fx, &allocator, &lease_id, "job-late", b"claim");
    // Stale revision and reader authority cannot renew.
    let mut stale = control(&fx, &allocator, &lease_id, "status", None, 0x50, READER);
    assert_eq!(stale["status"], "accepted");
    assert_eq!(
        control(
            &fx,
            &allocator,
            &lease_id,
            "renew",
            Some(fx.now + 100),
            0x51,
            READER
        )["reason"],
        "not_admitted"
    );
    let renewed = control(
        &fx,
        &allocator,
        &lease_id,
        "renew",
        Some(fx.now + 900),
        0x52,
        CONTROLLER,
    );
    assert_eq!(renewed["status"], "accepted");
    assert_eq!(
        control(
            &fx,
            &allocator,
            &lease_id,
            "renew",
            Some(fx.now + 99_999),
            0x53,
            CONTROLLER
        )["reason"],
        "limit_exceeded"
    );
    let later = fx.now + 2000;
    assert_eq!(allocator.expire(later).unwrap(), vec![lease_id.clone()]);
    assert_eq!(allocator.status(&lease_id).unwrap()["status"], "draining");
    fx.now = later;
    stale = control(
        &fx,
        &allocator,
        &lease_id,
        "renew",
        Some(later + 10),
        0x54,
        OWNER,
    );
    assert_eq!(stale["status"], "refused");
    assert_eq!(attach(&fx, &allocator, &job)["status"], "conflict");
}

#[test]
fn forged_adoption_refuses_and_release_only_detaches() {
    let mut fx = Fixture::new();
    let mut provider = LocalDirectories::new(fx.resources.clone()).unwrap();
    let Create::Created(existing, _) = provider.create("local", &"77".repeat(32)) else {
        panic!("create")
    };
    let allocator = fx.allocator();
    let mut artifacts = std::mem::take(&mut fx.artifacts);
    let forged = put(&mut artifacts, b"{\"forged\":true}", None);
    let real = put(&mut artifacts, &existing.ownership, None);
    let binding = json!({"id":format!("{}:environment/local-directory",pk(ALLOCATOR)),"artifact":put(&mut artifacts,b"environment/local-directory",None)});
    let descriptor = |ownership: &Value| {
        json!({"v":"openagents.environment-resource.v1","requires":[],
        "owner":pk(OWNER),"allocator":pk(ALLOCATOR),"provider":binding,"account":"local",
        "resource":existing.resource,"generation":existing.generation,"ownership":ownership})
    };
    let forged = put_json(&mut artifacts, &descriptor(&forged));
    let real = put_json(&mut artifacts, &descriptor(&real));
    fx.artifacts = artifacts;
    let mut body = fx.request_body(5, 1000, 600);
    body["mode"] = json!("adopt");
    body["cleanup"] = json!("detach");
    body["resource"] = forged;
    assert_eq!(
        allocator.submit(&fx.bundle(OWNER, &body), fx.now).unwrap()["reason"],
        "identity_mismatch"
    );
    body["resource"] = real;
    body["request"] = json!("56".repeat(32));
    let adopted = allocator.submit(&fx.bundle(OWNER, &body), fx.now).unwrap();
    assert_eq!(adopted["status"], "accepted", "{adopted}");
    let lease_id = allocator.leases().unwrap()[0].clone();
    let released = control(&fx, &allocator, &lease_id, "release", None, 0x60, OWNER);
    assert_eq!(released["status"], "accepted", "{released}");
    let status = allocator.status(&lease_id).unwrap();
    assert_eq!(
        allocator.store().json(&status["cleanup"]).unwrap()["action"],
        "detach"
    );
    assert!(
        fx.resources
            .join(&existing.resource)
            .join(".openagents-env.json")
            .exists()
    );
}

#[test]
fn readiness_refuses_substitutions_and_unenforceable_plans() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let body = fx.request_body(6, 1000, 600);
    let result = allocator.submit(&fx.bundle(OWNER, &body), fx.now).unwrap();
    let lease_id = allocator.store().json(&result["lease"]).unwrap()["lease"]
        .as_str()
        .unwrap()
        .to_string();
    let mut spec = fx.spec();
    spec.participants[1]["principal"] = json!(pk(8));
    assert!(
        allocator
            .materialize(&lease_id, &spec, fx.now)
            .unwrap_err()
            .starts_with("not_admitted")
    );
    let mut spec = fx.spec();
    spec.enforcement = json!([bound("output_bytes", 10)]);
    assert!(
        allocator
            .materialize(&lease_id, &spec, fx.now)
            .unwrap_err()
            .starts_with("cannot_enforce")
    );
    let mut spec = fx.spec();
    spec.participants[0]["configuration"] = put(&mut fx.artifacts, b"assurance", None);
    assert!(
        allocator
            .materialize(&lease_id, &spec, fx.now)
            .unwrap_err()
            .starts_with("incompatible")
    );
    let mut spec = fx.spec();
    spec.enforcement = json!([bound("wall_ms", 900_000)]);
    assert!(
        allocator
            .materialize(&lease_id, &spec, fx.now)
            .unwrap_err()
            .starts_with("limit_exceeded")
    );
    fx.faults.borrow_mut().drop_component = Some("tools".into());
    let spec = fx.spec();
    assert!(
        allocator
            .materialize(&lease_id, &spec, fx.now)
            .unwrap_err()
            .starts_with("incompatible")
    );
    fx.faults.borrow_mut().drop_component = None;
    assert_eq!(
        allocator.status(&lease_id).unwrap()["status"],
        "provisioning"
    );
    allocator.materialize(&lease_id, &spec, fx.now).unwrap();
    assert_eq!(allocator.status(&lease_id).unwrap()["status"], "ready");
}

#[test]
fn opaque_models_refuse_when_requirements_need_evidence() {
    let mut fx = Fixture::new();
    let opaque = put_json(
        &mut fx.artifacts,
        &json!({"v":"openagents.environment-model.v1","requires":[],"name":"hosted","identity":"opaque","weights":null}),
    );
    fx.definition["model"] = opaque;
    let allocator = fx.allocator();
    let body = fx.request_body(7, 1000, 600);
    let result = allocator.submit(&fx.bundle(OWNER, &body), fx.now).unwrap();
    let lease_id = allocator.store().json(&result["lease"]).unwrap()["lease"]
        .as_str()
        .unwrap()
        .to_string();
    let spec = fx.spec();
    assert!(
        allocator
            .materialize(&lease_id, &spec, fx.now)
            .unwrap_err()
            .contains("opaque")
    );
}

#[test]
fn unknown_cleanup_is_not_released_and_keeps_its_hold() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let (lease_id, _) = fx.ready(&allocator, 8);
    fx.faults.borrow_mut().destroy_unknown = true;
    let released = control(&fx, &allocator, &lease_id, "release", None, 0x70, OWNER);
    assert_eq!(released["status"], "unknown");
    let status = allocator.status(&lease_id).unwrap();
    assert_eq!(status["status"], "unknown");
    assert_eq!(status["held"], 4000);
    fx.faults.borrow_mut().destroy_unknown = false;
    drop(allocator);
    let allocator = fx.allocator();
    let retried = allocator.reconcile(&lease_id, fx.now).unwrap();
    assert_eq!(retried["status"], "unknown");
    assert_eq!(fx.faults.borrow().destroys, 1);
    // Complete the original provider operation, then observe its absence.
    let cleanup = allocator.store().json(&status["cleanup"]).unwrap();
    let resource = allocator.store().json(&cleanup["resource"]).unwrap();
    let realized = Realized {
        resource: resource["resource"].as_str().unwrap().into(),
        generation: resource["generation"].as_str().unwrap().into(),
        account: resource["account"].as_str().unwrap().into(),
        ownership: allocator.store().get(&resource["ownership"]).unwrap(),
    };
    let mut provider = LocalDirectories::new(fx.resources.clone()).unwrap();
    assert!(matches!(
        provider.destroy(&realized),
        Teardown::Confirmed(_, _)
    ));
    assert_eq!(
        allocator.reconcile(&lease_id, fx.now).unwrap()["status"],
        "released"
    );
    assert_eq!(fx.faults.borrow().destroys, 1);
    let status = allocator.status(&lease_id).unwrap();
    assert_eq!(
        allocator.store().json(&status["cleanup"]).unwrap()["status"],
        "confirmed"
    );
    // The earlier unknown charge is not zero, so the hold stays.
    assert_eq!(status["held"], 4000);
}

#[test]
fn failed_retention_never_deletes_the_resource() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let (lease_id, _) = fx.ready(&allocator, 9);
    fx.faults.borrow_mut().fail_collect = true;
    control(&fx, &allocator, &lease_id, "release", None, 0x71, OWNER);
    let status = allocator.status(&lease_id).unwrap();
    assert_eq!(status["status"], "failed");
    assert_eq!(std::fs::read_dir(&fx.resources).unwrap().count(), 1);
}

#[test]
fn an_unreachable_dispatcher_keeps_the_slot_and_cleanup_pending() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let (lease_id, _) = fx.ready(&allocator, 10);
    let job = job(&mut fx, &allocator, &lease_id, "job-u", b"claim");
    let grant_ref = attach(&fx, &allocator, &job)["value"].clone();
    admit_all(&mut fx, &allocator, &job, &grant_ref);
    attachment_control(&fx, &allocator, &grant_ref, "activate", 0x80, CONTROLLER);
    let released = control(&fx, &allocator, &lease_id, "release", None, 0x81, OWNER);
    assert_eq!(released["status"], "accepted");
    fence(&fx, &allocator, &grant_ref, WORKER, true);
    fence(&fx, &allocator, &grant_ref, INFERENCE, false);
    let status = allocator.status(&lease_id).unwrap();
    assert_eq!(status["status"], "draining");
    assert!(status["attachment"].is_object());
    allocator.cleanup(&lease_id, fx.now).unwrap();
    let status = allocator.status(&lease_id).unwrap();
    assert_eq!(
        allocator.store().json(&status["cleanup"]).unwrap()["status"],
        "pending"
    );
    assert_eq!(std::fs::read_dir(&fx.resources).unwrap().count(), 1);
}

#[test]
fn the_parent_allowance_cannot_be_spent_twice() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let first = fx.request_body(11, 6000, 600);
    assert_eq!(
        allocator.submit(&fx.bundle(OWNER, &first), fx.now).unwrap()["status"],
        "accepted"
    );
    let second = fx.request_body(12, 6000, 600);
    assert_eq!(
        allocator
            .submit(&fx.bundle(OWNER, &second), fx.now)
            .unwrap()["reason"],
        "limit_exceeded"
    );
    // Requests from an unauthorized owner refuse before any effect.
    let mut foreign = fx.request_body(13, 100, 600);
    foreign["owner"] = json!(pk(CONTROLLER));
    assert_eq!(
        allocator
            .submit(&fx.bundle(CONTROLLER, &foreign), fx.now)
            .unwrap()["reason"],
        "not_admitted"
    );
    // Spend inside an environment needs a separately admitted profile.
    let mut spend = fx.request_body(14, 100, 600);
    spend["effects"]["spend"] = json!(true);
    assert_eq!(
        allocator.submit(&fx.bundle(OWNER, &spend), fx.now).unwrap()["reason"],
        "unsupported_feature"
    );
    assert_eq!(fx.faults.borrow().creates, 1);
}

#[cfg(unix)]
mod ssh {
    use super::*;
    use crate::ssh::{SshEnvironments, SshHosts};
    use coder_ssh::{Arch, Host, Install, Os, Ownership, Removal, Start};

    struct Fake {
        host: Host,
        removal: Removal,
    }
    impl SshHosts for Fake {
        fn up(&mut self) -> Result<Host, String> {
            Ok(self.host.clone())
        }
        fn remove(&mut self) -> Result<Removal, String> {
            Ok(self.removal)
        }
    }
    fn host(ownership: Ownership, start: Start) -> Host {
        Host {
            os: Os::Linux,
            arch: Arch::X86_64,
            install: Install::Reused,
            version: digest_bytes(b"release")[7..].to_string(),
            start,
            ownership,
            pid: 4242,
            port: 7000,
            reclaimed_lock: false,
        }
    }

    #[test]
    fn managed_hosts_map_to_create_and_external_hosts_are_never_relabeled() {
        let mut managed = SshEnvironments::new(
            Fake {
                host: host(Ownership::Managed, Start::Started),
                removal: Removal::Stopped { pid: 4242 },
            },
            "dev@box",
        );
        let Create::Created(realized, Cost::Unknown) = managed.create("dev@box", &"11".repeat(32))
        else {
            panic!("managed create")
        };
        let observed = managed
            .materialize(
                &realized,
                &[
                    ("release".into(), b"release".to_vec()),
                    ("other".into(), b"x".to_vec()),
                ],
            )
            .unwrap();
        assert_eq!(
            observed,
            vec![("release".to_string(), digest_bytes(b"release"))]
        );
        assert!(matches!(
            managed.destroy(&realized),
            Teardown::Confirmed(..)
        ));
        assert!(matches!(managed.lookup("dev@box", "k"), Lookup::Unknown(_)));
        let mut external = SshEnvironments::new(
            Fake {
                host: host(Ownership::External, Start::Adopted),
                removal: Removal::Detached { pid: 4242 },
            },
            "dev@box",
        );
        assert!(matches!(
            external.create("dev@box", "k"),
            Create::Refused(_)
        ));
        assert!(matches!(external.destroy(&realized), Teardown::Failed(_)));
        assert!(matches!(
            external.detach(&realized),
            Teardown::Confirmed(..)
        ));
        let mut gone = SshEnvironments::new(
            Fake {
                host: host(Ownership::Managed, Start::Reused),
                removal: Removal::Absent,
            },
            "dev@box",
        );
        assert!(matches!(gone.destroy(&realized), Teardown::Unknown(_)));
        assert!(matches!(gone.create("other@box", "k"), Create::Refused(_)));
    }
}

#[test]
fn a_replayed_body_from_another_signer_cannot_block_the_owner() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let body = fx.request_body(15, 100, 600);
    let replay = allocator
        .submit(&fx.bundle(CONTROLLER, &body), fx.now)
        .unwrap();
    assert_eq!(replay["reason"], "not_admitted");
    let owner = allocator.submit(&fx.bundle(OWNER, &body), fx.now).unwrap();
    assert_eq!(owner["status"], "accepted", "{owner}");
}

#[test]
fn management_cj_pins_real_allocation_terminal_proof_and_restart_exact_reply() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let binding = allocator.transport().unwrap();
    let card: Value = serde_json::from_slice(
        &allocator
            .store()
            .get(&binding["target"]["artifact"])
            .unwrap(),
    )
    .unwrap();
    nostr::cap::parse_definition(&card).unwrap();
    let body = fx.request_body(81, 4000, 1800);
    let mut bad = binding.clone();
    bad["context"] = binding["requirements"].clone();
    let wrong = crate::transport::invoke(
        &secret(OWNER),
        allocator.pubkey(),
        &bad,
        body.clone(),
        workspace_host::artifact::token(),
        fx.now,
    )
    .unwrap();
    assert!(allocator.serve(&wrong, &fx.artifacts, &[], fx.now).is_err());
    assert_eq!(fx.faults.borrow().creates, 0);
    let request = crate::transport::invoke(
        &secret(OWNER),
        allocator.pubkey(),
        &binding,
        body,
        workspace_host::artifact::token(),
        fx.now,
    )
    .unwrap();
    let (response, files) = allocator
        .serve(&request, &fx.artifacts, &[], fx.now)
        .unwrap();
    let opened = execution::open_request(
        &request,
        &pk(ALLOCATOR),
        &secret(ALLOCATOR),
        fx.now,
        execution::Window::DEFAULT,
    )
    .unwrap();
    let execution::Body::Execute(execute) = opened.body else {
        panic!()
    };
    let result = execution::bind_worker_event(
        &response,
        &execution::Pending {
            worker: &pk(ALLOCATOR),
            customer: &pk(OWNER),
            request: &execute.request,
            attempt: 1,
            execute_event: &request.id,
        },
        &secret(OWNER),
    )
    .unwrap();
    nostr::run::terminal_from_result(&result, &files, &pk(ALLOCATOR)).unwrap();
    let output: Value =
        serde_json::from_slice(&files[result["output"]["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(output["status"], "accepted");
    assert_eq!(fx.faults.borrow().creates, 1);
    drop(allocator);
    let allocator = fx.allocator();
    assert_eq!(
        allocator
            .serve(&request, &fx.artifacts, &[], fx.now + 1)
            .unwrap(),
        (response.clone(), files.clone())
    );
    assert_eq!(
        allocator.recover(&request, fx.now + 1000).unwrap(),
        (response, files)
    );
    assert_eq!(fx.faults.borrow().creates, 1);
}
#[test]
fn a_local_unmarked_partial_create_is_never_adopted_by_retrying_create() {
    let temp = tempfile::tempdir().unwrap();
    let mut provider = LocalDirectories::new(temp.path().to_path_buf()).unwrap();
    let Create::Created(resource, _) = provider.create("local", "key") else {
        panic!()
    };
    let directory = provider.directory(&resource).unwrap();
    std::fs::remove_file(directory.join(".openagents-env.json")).unwrap();
    assert!(matches!(
        provider.create("local", "key"),
        Create::Unknown(_)
    ));
    assert!(matches!(
        provider.lookup("local", "key"),
        Lookup::Unknown(_)
    ));
    assert!(!directory.join(".openagents-env.json").exists());
}

#[test]
fn interrupted_cj_recovery_fences_a_late_handler_without_starting_the_provider() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let body = fx.request_body(82, 4000, 1800);
    let request = crate::transport::invoke(
        &secret(OWNER),
        allocator.pubkey(),
        &allocator.transport().unwrap(),
        body,
        workspace_host::artifact::token(),
        fx.now,
    )
    .unwrap();
    let wrong = BTreeMap::from([("sha256:invalid".to_string(), vec![1])]);
    assert!(allocator.serve(&request, &wrong, &[], fx.now).is_err());
    assert_eq!(fx.faults.borrow().creates, 0);
    let (response, files) = allocator.recover(&request, fx.now + 1).unwrap();
    let opened = execution::open_request(
        &request,
        &pk(ALLOCATOR),
        &secret(ALLOCATOR),
        fx.now,
        execution::Window::DEFAULT,
    )
    .unwrap();
    let execution::Body::Execute(execute) = opened.body else {
        panic!()
    };
    let result = execution::bind_worker_event(
        &response,
        &execution::Pending {
            worker: &pk(ALLOCATOR),
            customer: &pk(OWNER),
            request: &execute.request,
            attempt: 1,
            execute_event: &request.id,
        },
        &secret(OWNER),
    )
    .unwrap();
    let output: Value =
        serde_json::from_slice(&files[result["output"]["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(output["status"], "unknown");
    assert!(
        allocator
            .submit(
                &Bundle {
                    event: request.clone(),
                    artifacts: fx.artifacts.clone(),
                    declarations: vec![]
                },
                fx.now + 2
            )
            .unwrap_err()
            .contains("fenced")
    );
    assert_eq!(
        allocator
            .serve(&request, &fx.artifacts, &[], fx.now + 2)
            .unwrap(),
        (response, files)
    );
    assert!(allocator.leases().unwrap().is_empty());
    assert_eq!(fx.faults.borrow().creates, 0);
}

fn native_environment_job(
    fx: &mut Fixture,
) -> (
    coordination_host::book::Coordinator,
    Value,
    Value,
    Event,
    Value,
) {
    let h = coordination_host::book::Coordinator::initialize(
        fx._temp.path().join("coord"),
        secret(OWNER),
    )
    .unwrap();
    let target = h.data_operation().unwrap();
    let effects = json!({"reads":["artifact"],"writes":[],"network":[],"process":false,"delegates":false,"spend":false});
    let now = fx.now;
    let scope = h
        .create_scope(
            coordination_host::book::Policy {
                recipients: BTreeSet::from([pk(OWNER), pk(WORKER), pk(INFERENCE)]),
                checkers: BTreeSet::from([pk(INFERENCE)]),
                calls: 2,
                concurrency: 1,
                max_claim_seconds: 300,
                max_container_seconds: None,
                max_depth: 8,
                until: now + 7200,
                effects: effects.clone(),
                parent: None,
            },
            vec![],
            vec![target.clone()],
            now,
        )
        .unwrap();
    let send =
        |body: Value| workspace_host::host::request(&secret(OWNER), &pk(OWNER), body, now).unwrap();
    h.control_grants(&send(json!({"v":"openagents.coordination-native-control.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":h.scope(&scope).unwrap()["revision"],"actor":pk(WORKER),"roles":["read","claim"],"resources":[],"targets":[],"all":true,"until":now+7200,"revoke":false,"scope_revoke":false})),now).unwrap();
    let acceptance=h.put_local_json(&json!({"v":"openagents.coordination-native-check-policy.v1","requires":[],"check":"exact-admitted-output"})).unwrap();
    let task = workspace_host::artifact::token();
    let text = h
        .put_local_bytes(b"Bounded environment data", "text/plain", None)
        .unwrap();
    let snapshot=h.put_local_json(&json!({"v":"openagents.snapshot.v1","requires":[],"scope":"env-integration","captured_at":now,"sources":[],"coverage":"complete"})).unwrap();
    let precedence = h
        .put_local_json(
            &json!({"v":"openagents.coordination-empty-instruction-policy.v1","requires":[]}),
        )
        .unwrap();
    let instructions=h.put_local_json(&json!({"v":"openagents.instructions.v1","requires":[],"task":task,"revision":0,"resolver":{"id":format!("{}:coord/empty-instructions",pk(OWNER)),"artifact":precedence},"precedence":precedence,"snapshot":snapshot,"entries":[]})).unwrap();
    let frame=h.put_local_json(&json!({"v":"openagents.task-frame.v1","requires":[],"task":task,"owner":pk(OWNER),"controller":pk(OWNER),"revision":0,"previous":null,"objective":text,"origin":"user","constraints":[],"acceptance":[acceptance],"snapshot":snapshot,"instructions":instructions,"variables":[],"attempts":[],"unresolved":[],"corrections":[]})).unwrap();
    h.frame(&send(json!({"v":"openagents.coordination-native-frame.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":h.scope(&scope).unwrap()["revision"],"frame":frame})),&h.get_local_bytes(&frame).unwrap(),now).unwrap();
    let input=h.put_local_json(&json!({"v":"openagents.fixture-input.v1","requires":[],"trial":workspace_host::artifact::token(),"text":"native environment data"})).unwrap();
    let lock=h.put_local_json(&json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":target["id"],"definition":target,"dependencies":[]}]})).unwrap();
    let descriptor: Value = serde_json::from_slice(&h.get_local_bytes(&scope).unwrap()).unwrap();
    let context=h.put_local_json(&json!({"v":"openagents.context.v1","requires":[],"task":task,"recipient":pk(WORKER),"policy":descriptor["policy"],"entries":[],"omissions":[],"coverage":"complete"})).unwrap();
    let proposal=h.propose(&send(json!({"v":"openagents.task-proposal.v1","requires":[],"task":workspace_host::artifact::token(),"coordinator":scope,"frame":frame,"parent":null,"target":target,"lock":lock,"input":input,"context":context,"dependencies":[],"effects":effects,"bounds":[{"bound":"calls","ceiling":2,"assurance":"host","mechanism":"native-ledger"}],"acceptance":acceptance,"reuse":"never"})),&BTreeMap::new(),now).unwrap()["value"].clone();
    let claim=h.claim(&send(json!({"v":"openagents.claim-request.v1","requires":[],"proposal":proposal,"worker":pk(WORKER),"resources":[],"expected_revision":h.scope(&scope).unwrap()["revision"],"requested_until":now+300,"reservation":h.scope(&scope).unwrap()["allowance"]})),now).unwrap()["value"].clone();
    let dispatch = send(
        json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":scope,"claim":claim}),
    );
    let event = h.prepare_environment(&dispatch, now).unwrap();
    assert!(h.prepare(&dispatch, now).unwrap_err().contains("profile"));
    let opened = execution::open_environment_request(
        &event,
        &pk(WORKER),
        &secret(WORKER),
        now,
        execution::Window::DEFAULT,
    )
    .unwrap();
    let execution::Body::Execute(execute) = opened.body else {
        panic!()
    };
    let claim_body: Value = serde_json::from_slice(&h.get_local_bytes(&claim).unwrap()).unwrap();
    fx.reservation =
        serde_json::from_slice(&h.get_local_bytes(&claim_body["reservation"]).unwrap()).unwrap();
    put(
        &mut fx.artifacts,
        &h.get_local_bytes(&claim).unwrap(),
        Some("openagents.claim.v1"),
    );
    (h, scope, claim, event, execute.payload)
}
#[test]
fn actual_coordinator_claim_and_environment_attachment_gate_the_native_worker() {
    for mode in ["run", "revoke", "drain", "expired-claim"] {
        let mut fx = Fixture::new();
        let (coordinator, scope, claim, event, payload) = native_environment_job(&mut fx);
        let mounted = coordination_host::book::Coordinator::open(
            fx._temp.path().join("coord"),
            secret(OWNER),
        )
        .unwrap();
        let allocator = fx.allocator().with_claims(Box::new(mounted));
        let mut request = fx.request_body(91, 0, 1800);
        request["bounds"][1]["currency"] = json!("USD");
        let result = allocator
            .submit(&fx.bundle(OWNER, &request), fx.now)
            .unwrap();
        assert_eq!(result["status"], "accepted", "{result}");
        let lease_id = allocator.store().json(&result["lease"]).unwrap()["lease"]
            .as_str()
            .unwrap()
            .to_string();
        let spec = fx.spec();
        allocator.materialize(&lease_id, &spec, fx.now).unwrap();
        let mut job = job(&mut fx, &allocator, &lease_id, "unused", b"opaque claim");
        job.event = event;
        job.payload = payload;
        let body = put_json(&mut fx.artifacts, &job.payload);
        let declaration = json!({"v":"openagents.environment-execute.v1","requires":[],"caller":pk(OWNER),"event":{"id":job.event.id,"pubkey":job.event.pubkey,"kind":job.event.kind},"body":body});
        job.attach["controller"] = json!(pk(OWNER));
        job.attach["execute"] = put_json(&mut fx.artifacts, &declaration);
        job.attach["input_digest"] = json!(digest_value(&job.payload["input"]).unwrap());
        job.attach["claim"] = claim;
        job.declaration_event =
            envelope(&secret(OWNER), allocator.pubkey(), &declaration, fx.now).unwrap();
        let mut bundle = fx.bundle(OWNER, &job.attach);
        bundle.declarations.push(job.declaration_event.clone());
        let result = allocator.submit(&bundle, fx.now).unwrap();
        assert_eq!(result["status"], "accepted", "{result}");
        let grant = result["value"].clone();
        admit_all(&mut fx, &allocator, &job, &grant);
        assert_eq!(
            attachment_control(&fx, &allocator, &grant, "activate", 92, OWNER)["status"],
            "accepted"
        );
        if mode == "revoke" {
            let body = json!({"v":"openagents.coordination-native-control.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":coordinator.scope(&scope).unwrap()["revision"],"actor":pk(WORKER),"roles":[],"resources":[],"targets":[],"all":true,"until":fx.now+7200,"revoke":true,"scope_revoke":false});
            coordinator
                .control_grants(
                    &workspace_host::host::request(&secret(OWNER), &pk(OWNER), body, fx.now)
                        .unwrap(),
                    fx.now,
                )
                .unwrap();
        }
        let worker = coordination_host::worker::Worker::open(
            fx._temp.path().join("real-worker"),
            secret(WORKER),
            fx.now,
        )
        .unwrap();
        assert!(worker.run(&coordinator, &job.event, fx.now).is_err());
        if mode == "drain" {
            assert_eq!(
                attachment_control(&fx, &allocator, &grant, "drain", 94, OWNER)["status"],
                "accepted"
            );
        }
        let at = if mode == "expired-claim" {
            fx.now + 301
        } else {
            fx.now
        };
        let result = worker.run_in_environment(&coordinator, &job.event, at, &allocator);
        if mode != "run" {
            assert!(result.is_err());
            assert_eq!(coordinator.scope(&scope).unwrap()["calls"], 0);
        } else {
            let (event, files) = result.unwrap();
            assert_eq!(
                coordinator.record_result(&event, &files, fx.now).unwrap()["outcome"],
                "completed"
            );
            assert_eq!(coordinator.scope(&scope).unwrap()["calls"], 1);
            assert_eq!(
                worker
                    .retained_result(&job.event, fx.now + 1000)
                    .unwrap()
                    .unwrap()
                    .0,
                event
            );
        }
    }
}
#[test]
fn attachment_without_a_current_claim_authority_refuses_opaque_claim_bytes() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let (lease_id, _) = fx.ready(&allocator, 93);
    let job = job(
        &mut fx,
        &allocator,
        &lease_id,
        "unverified",
        b"claim bytes do not prove authority",
    );
    drop(allocator);
    let allocator = Allocator::open(
        fx.state.clone(),
        secret(ALLOCATOR),
        Box::new(LocalDirectories::new(fx.resources.clone()).unwrap()),
    )
    .unwrap();
    let result = attach(&fx, &allocator, &job);
    assert_eq!(result["status"], "refused");
    assert_eq!(result["reason"], "not_admitted");
    assert!(allocator.status(&lease_id).unwrap()["attachment"].is_null());
}
#[test]
fn an_execute_deadline_cannot_outlive_the_attachment_lease() {
    let mut fx = Fixture::new();
    let allocator = fx.allocator();
    let request = fx.request_body(95, 4000, 120);
    let result = allocator
        .submit(&fx.bundle(OWNER, &request), fx.now)
        .unwrap();
    let lease_id = allocator.store().json(&result["lease"]).unwrap()["lease"]
        .as_str()
        .unwrap()
        .to_string();
    let spec = fx.spec();
    allocator.materialize(&lease_id, &spec, fx.now).unwrap();
    let job = job(&mut fx, &allocator, &lease_id, "too-long", b"fixture claim");
    let result = attach(&fx, &allocator, &job);
    assert_eq!(result["status"], "refused");
    assert_eq!(result["reason"], "cannot_enforce");
}
