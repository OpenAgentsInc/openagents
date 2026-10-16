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
    bootstraps: usize,
    placements: usize,
    lose_bootstrap: bool,
    fail_bootstrap: bool,
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
        self.faults.borrow_mut().placements += 1;
        let drop = self.faults.borrow().drop_component.clone();
        let kept: Vec<_> = components
            .iter()
            .filter(|(id, _)| Some(id) != drop.as_ref())
            .cloned()
            .collect();
        self.inner.materialize(resource, &kept)
    }
    fn manifest_bootstrap(&self) -> bool {
        self.inner.manifest_bootstrap()
    }
    fn observe_components(
        &mut self,
        r: &Realized,
        ids: &[String],
    ) -> crate::Result<Vec<(String, String)>> {
        self.inner.observe_components(r, ids)
    }
    fn bootstrap(&mut self, r: &Realized, job: &str, input: &Value) -> crate::Result<Vec<u8>> {
        self.faults.borrow_mut().bootstraps += 1;
        if self.faults.borrow().fail_bootstrap {
            return Err("unknown: provider did not return an initialization result".into());
        }
        let bytes = self.inner.bootstrap(r, job, input)?;
        if self.faults.borrow().lose_bootstrap {
            return Err("unknown: initialization reply lost".into());
        }
        Ok(bytes)
    }
    fn bootstrap_result(&mut self, r: &Realized, job: &str) -> crate::Result<Option<Vec<u8>>> {
        self.inner.bootstrap_result(r, job)
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
    frame: Value,
    now: u64,
}

fn put(map: &mut BTreeMap<String, Vec<u8>>, bytes: &[u8], schema: Option<&str>) -> Value {
    map.insert(digest_bytes(bytes), bytes.to_vec());
    let mut value = json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":if schema.is_some(){"application/json"}else{"application/octet-stream"}});
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
        Self::with_owners(BTreeSet::from([pk(OWNER)]))
    }
    fn with_owners(owners: BTreeSet<String>) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("state");
        let resources = temp.path().join("resources");
        let policy = Policy {
            owners,
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
        let frame = put(&mut artifacts, b"frame", None);
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
            frame,
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
            "frame":self.frame,"policy":put(&mut artifacts,b"policy",None),
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
            bootstrap: None,
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
            panic!("unguarded remove must never be called")
        }
        fn remove_exact(&mut self, resource: &Realized) -> Result<Removal, String> {
            if self.host.ownership == Ownership::External {
                return Ok(Removal::Detached { pid: self.host.pid });
            }
            if resource.generation != format!("{}:{}", self.host.version, self.host.pid) {
                return Err("identity_mismatch: replacement host keeps running".into());
            }
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
        let mut replaced = realized.clone();
        replaced.generation = "another-generation".into();
        assert!(matches!(managed.destroy(&replaced), Teardown::Unknown(_)));
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
    let (h, scope, claim, event, payload, _) = native_environment_job_with_parent(fx, false);
    (h, scope, claim, event, payload)
}
fn native_environment_job_with_parent(
    fx: &mut Fixture,
    with_parent: bool,
) -> (
    coordination_host::book::Coordinator,
    Value,
    Value,
    Event,
    Value,
    Value,
) {
    let h = coordination_host::book::Coordinator::initialize(
        fx._temp.path().join("coord"),
        secret(OWNER),
    )
    .unwrap();
    let target = h.data_operation().unwrap();
    let effects = json!({"reads":["artifact"],"writes":[],"network":[],"process":false,"delegates":false,"spend":false});
    let mut ceiling = effects.clone();
    ceiling["delegates"] = json!(true);
    let container = h.container_operation(ceiling.clone()).unwrap();
    let now = fx.now;
    let scope = h
        .create_scope(
            coordination_host::book::Policy {
                recipients: BTreeSet::from([pk(OWNER), pk(WORKER), pk(INFERENCE)]),
                checkers: BTreeSet::from([pk(INFERENCE)]),
                calls: 4,
                concurrency: 2,
                max_claim_seconds: 300,
                max_container_seconds: Some(3600),
                max_depth: 8,
                until: now + 7200,
                effects: ceiling.clone(),
                parent: None,
            },
            vec![],
            vec![target.clone(), container.clone()],
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
    let mut parent_task = Value::Null;
    let mut budget = Value::Null;
    let mut parent_reservation = h.scope(&scope).unwrap()["allowance"].clone();
    if with_parent {
        parent_task = json!(workspace_host::artifact::token());
        let parent_lock=h.put_local_json(&json!({"v":"openagents.lock.v1","requires":[],"root":container,"entries":[{"id":container["id"],"definition":container,"dependencies":[]}]})).unwrap();
        let parent_context=h.put_local_json(&json!({"v":"openagents.context.v1","requires":[],"task":task,"recipient":pk(OWNER),"policy":descriptor["policy"],"entries":[],"omissions":[],"coverage":"complete"})).unwrap();
        let proposal=h.propose(&send(json!({"v":"openagents.task-proposal.v1","requires":[],"task":parent_task,"coordinator":scope,"frame":frame,"parent":null,"target":container,"lock":parent_lock,"input":input,"context":parent_context,"dependencies":[],"effects":ceiling,"bounds":[{"bound":"calls","ceiling":4,"assurance":"host","mechanism":"native-ledger"}],"acceptance":acceptance,"reuse":"never"})),&BTreeMap::new(),now).unwrap()["value"].clone();
        budget=h.claim(&send(json!({"v":"openagents.claim-request.v1","requires":[],"proposal":proposal,"worker":pk(OWNER),"resources":[],"expected_revision":h.scope(&scope).unwrap()["revision"],"requested_until":now+1800,"reservation":parent_reservation})),now).unwrap()["value"].clone();
        let parent_body: Value =
            serde_json::from_slice(&h.get_local_bytes(&budget).unwrap()).unwrap();
        parent_reservation = parent_body["reservation"].clone();
    }
    let proposal=h.propose(&send(json!({"v":"openagents.task-proposal.v1","requires":[],"task":workspace_host::artifact::token(),"coordinator":scope,"frame":frame,"parent":parent_task,"target":target,"lock":lock,"input":input,"context":context,"dependencies":[],"effects":effects,"bounds":[{"bound":"calls","ceiling":2,"assurance":"host","mechanism":"native-ledger"}],"acceptance":acceptance,"reuse":"never"})),&BTreeMap::new(),now).unwrap()["value"].clone();
    let claim=h.claim(&send(json!({"v":"openagents.claim-request.v1","requires":[],"proposal":proposal,"worker":pk(WORKER),"resources":[],"expected_revision":h.scope(&scope).unwrap()["revision"],"requested_until":now+300,"reservation":parent_reservation})),now).unwrap()["value"].clone();
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
    fx.frame = frame.clone();
    let mut queue = vec![frame];
    let mut seen = BTreeSet::new();
    while let Some(reference) = queue.pop() {
        let digest = reference["digest"].as_str().unwrap().to_string();
        if !seen.insert(digest.clone()) {
            continue;
        }
        let bytes = h.get_local_bytes(&reference).unwrap();
        fx.artifacts.insert(digest, bytes.clone());
        if let Ok(body) = nostr::contracts::parse_strict(&bytes) {
            let mut stack = vec![&body];
            while let Some(value) = stack.pop() {
                if nostr::contracts::parse_artifact(value).is_ok() {
                    queue.push(value.clone());
                    continue;
                }
                match value {
                    Value::Object(o) => stack.extend(o.values()),
                    Value::Array(a) => stack.extend(a),
                    _ => {}
                }
            }
        }
    }
    (h, scope, claim, event, execute.payload, budget)
}
#[test]
fn actual_coordinator_claim_and_environment_attachment_gate_the_native_worker() {
    for mode in ["run", "revoke", "drain", "expired-claim", "different-frame"] {
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
        if mode == "different-frame" {
            request["frame"] =
                workspace_host::artifact::reference(b"frame", "application/octet-stream", None);
        }
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
        if mode == "different-frame" {
            assert_eq!(result["status"], "refused");
            assert_eq!(result["reason"], "identity_mismatch");
            assert_eq!(coordinator.scope(&scope).unwrap()["calls"], 0);
            continue;
        }
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

fn bootstrap_fixture(
    fx: &mut Fixture,
    bad_allowance: bool,
) -> (
    Allocator,
    coordination_host::book::Coordinator,
    Value,
    String,
    MaterializeSpec,
) {
    let (coordinator, scope, claim, _event, _payload, budget) =
        native_environment_job_with_parent(fx, true);
    let mounted =
        coordination_host::book::Coordinator::open(fx._temp.path().join("coord"), secret(OWNER))
            .unwrap();
    let allocator = fx.allocator().with_claims(Box::new(mounted));
    let create_id = format!("{:02x}", 96).repeat(32);
    let request = workspace_host::artifact::token();
    let mut components = fx.definition["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| json!({"id":c["id"],"artifact":c["artifact"]}))
        .collect::<Vec<_>>();
    components.push(json!({"id":"workspace","artifact":fx.definition["workspace"]}));
    components.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let input = json!({"v":"openagents.environment-native-bootstrap-input.v1","requires":[],"owner":pk(OWNER),"request":create_id,"components":components});
    let input_ref = put_json(&mut fx.artifacts, &input);
    let fingerprint = if bad_allowance {
        digest_bytes(b"different input")
    } else {
        digest_value(&input).unwrap()
    };
    let admission=workspace_host::host::request(&secret(OWNER),&pk(OWNER),json!({"v":"openagents.coordination-native-auxiliary.v1","requires":[],"coordinator":scope,"claim":budget,"command":workspace_host::artifact::token(),"purpose":"prepare","request":request,"fingerprint":fingerprint}),fx.now).unwrap();
    let binding = allocator
        .bootstrap_binding(&claim, &admission, &request)
        .unwrap();
    let card: Value = serde_json::from_slice(
        &allocator
            .store()
            .get(&binding["target"]["artifact"])
            .unwrap(),
    )
    .unwrap();
    nostr::cap::parse_definition(&card).unwrap();
    let bounds = json!([
        {"bound":"calls","ceiling":1,"assurance":"host","mechanism":"native-ledger"},
        {"bound":"wall_ms","ceiling":30000,"assurance":"host","mechanism":"native-response-deadline"},
        {"bound":"input_bytes","ceiling":1048576,"assurance":"host","mechanism":"native-bytes"},
        {"bound":"output_bytes","ceiling":1048576,"assurance":"host","mechanism":"native-bytes"}
    ]);
    fx.definition["bootstrap"] = json!({"target":binding["target"],"lock":binding["lock"],"input":input_ref,"context":binding["context"],"requirements":binding["requirements"],"effects":binding["effects"],"bounds":bounds});
    let mut request_body = fx.request_body(96, 0, 1800);
    request_body["bounds"][1]["currency"] = json!("USD");
    let result = allocator
        .submit(&fx.bundle(OWNER, &request_body), fx.now)
        .unwrap();
    assert_eq!(result["status"], "accepted", "{result}");
    let lease_id = allocator.store().json(&result["lease"]).unwrap()["lease"]
        .as_str()
        .unwrap()
        .to_string();
    let deadline = fx.now + 200;
    let payload = json!({"v":execution::SCHEMA,"requires":[],"type":"execute","request":request,"attempt":1,"run":workspace_host::artifact::token(),"target":binding["target"],"lock":binding["lock"],"input":{"artifact":input_ref},"context":binding["context"],"requirements":binding["requirements"],"bounds":{"wall_ms":30000,"output_bytes":1048576,"jobs":1,"spend_microunits":0},"deadline":deadline,"retain_until":deadline+3600});
    let signer = RelaySigner::from_secret_hex(&secret(OWNER).display_secret().to_string()).unwrap();
    let event = execution::Seal {
        signer: &signer,
        conversation: nostr::nip44::conversation_key(
            &secret(OWNER),
            &pk(ALLOCATOR).parse().unwrap(),
        ),
        nonce: [8; 32],
        created_at: fx.now,
    }
    .event(
        execution::REQUEST_KIND,
        vec![
            Tag::new(vec!["p".into(), pk(ALLOCATOR)]),
            Tag::new(vec!["expiration".into(), deadline.to_string()]),
        ],
        &payload,
    )
    .unwrap();
    let mut spec = fx.spec();
    spec.bootstrap = Some(event);
    (allocator, coordinator, scope, lease_id, spec)
}
#[test]
fn a_separately_signed_bootstrap_initializes_real_files_and_charges_the_actual_parent_once() {
    let mut fx = Fixture::new();
    let (allocator, coordinator, scope, lease_id, spec) = bootstrap_fixture(&mut fx, false);
    assert_eq!(coordinator.scope(&scope).unwrap()["calls"], 0);
    let result = allocator.materialize(&lease_id, &spec, fx.now).unwrap();
    let materialization = allocator.store().json(&result["materialization"]).unwrap();
    assert_eq!(
        materialization["bootstrap"]["schema"],
        nostr::run::RECORD_SCHEMA
    );
    let terminal = allocator
        .store()
        .json(&materialization["bootstrap"])
        .unwrap();
    assert_eq!(terminal["record"]["type"], "settled");
    assert_eq!(terminal["record"]["data"]["outcome"], "completed");
    let resource = allocator
        .store()
        .json(&materialization["resource"])
        .unwrap();
    let directory = fx.resources.join(resource["resource"].as_str().unwrap());
    assert_eq!(
        std::fs::read_dir(&directory)
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".initialized-"))
            .count(),
        1
    );
    allocator.materialize(&lease_id, &spec, fx.now + 1).unwrap();
    assert_eq!(fx.faults.borrow().bootstraps, 1);
    assert_eq!(fx.faults.borrow().placements, 1);
    assert_eq!(coordinator.scope(&scope).unwrap()["calls"], 1);
    assert!(
        allocator
            .recover(spec.bootstrap.as_ref().unwrap(), fx.now + 1)
            .unwrap_err()
            .contains("management endpoint")
    );
}
#[test]
fn lost_bootstrap_result_recovers_only_original_provider_receipt_without_reinitializing() {
    let mut fx = Fixture::new();
    let (allocator, coordinator, scope, lease_id, spec) = bootstrap_fixture(&mut fx, false);
    fx.faults.borrow_mut().lose_bootstrap = true;
    assert!(allocator.materialize(&lease_id, &spec, fx.now).is_err());
    let status = allocator.status(&lease_id).unwrap();
    assert_eq!(status["status"], "provisioning");
    assert!(allocator.store().json(&status["state"]).unwrap()["materialization"].is_null());
    assert_eq!(fx.faults.borrow().bootstraps, 1);
    drop(allocator);
    let allocator = fx.allocator().with_claims(Box::new(
        coordination_host::book::Coordinator::open(fx._temp.path().join("coord"), secret(OWNER))
            .unwrap(),
    ));
    allocator.materialize(&lease_id, &spec, fx.now + 1).unwrap();
    assert_eq!(allocator.status(&lease_id).unwrap()["status"], "ready");
    assert_eq!(fx.faults.borrow().bootstraps, 1);
    assert_eq!(fx.faults.borrow().placements, 1);
    assert_eq!(coordinator.scope(&scope).unwrap()["calls"], 1);
}
#[test]
fn missing_bootstrap_receipt_stays_unknown_and_a_refused_budget_never_initializes() {
    for bad_allowance in [false, true] {
        let mut fx = Fixture::new();
        let (allocator, coordinator, scope, lease_id, spec) =
            bootstrap_fixture(&mut fx, bad_allowance);
        fx.faults.borrow_mut().fail_bootstrap = true;
        assert!(allocator.materialize(&lease_id, &spec, fx.now).is_err());
        assert!(allocator.materialize(&lease_id, &spec, fx.now + 1).is_err());
        assert_ne!(allocator.status(&lease_id).unwrap()["status"], "ready");
        assert_eq!(fx.faults.borrow().bootstraps, usize::from(!bad_allowance));
        assert_eq!(
            coordinator.scope(&scope).unwrap()["calls"],
            u64::from(!bad_allowance)
        );
    }
}
#[test]
fn another_owners_cached_component_is_not_read_permission_for_a_new_environment() {
    let mut fx = Fixture::with_owners(BTreeSet::from([pk(OWNER), pk(7)]));
    let allocator = fx.allocator();
    let mut other_definition = fx.definition.clone();
    let secret_bytes = b"private component for the other owner";
    let secret_ref=reference(&json!({"v":"openagents.private-fixture.v1","requires":[],"secret":String::from_utf8_lossy(secret_bytes)})).unwrap();
    let secret_body = json!({"v":"openagents.private-fixture.v1","requires":[],"secret":String::from_utf8_lossy(secret_bytes)});
    other_definition["components"][0]["artifact"] = secret_ref.clone();
    let mut other = fx.request_body(97, 4000, 1800);
    other["owner"] = json!(pk(7));
    let mut bundle = fx.bundle(7, &other);
    let definition = put_json(&mut bundle.artifacts, &other_definition);
    other["definition"] = definition;
    bundle.event = envelope(&secret(7), allocator.pubkey(), &other, fx.now).unwrap();
    put_json(&mut bundle.artifacts, &secret_body);
    let accepted = allocator.submit(&bundle, fx.now).unwrap();
    assert_eq!(accepted["status"], "accepted", "{accepted}");
    assert_eq!(fx.faults.borrow().creates, 1);
    fx.definition["components"][0]["artifact"] = secret_ref;
    let request = fx.request_body(98, 4000, 1800);
    let refused = allocator
        .submit(&fx.bundle(OWNER, &request), fx.now)
        .unwrap();
    assert_eq!(refused["status"], "refused");
    assert_eq!(refused["reason"], "not_admitted");
    assert_eq!(fx.faults.borrow().creates, 1);
}
#[test]
fn refused_cross_owner_status_does_not_disclose_the_other_lease_state() {
    let mut fx = Fixture::with_owners(BTreeSet::from([pk(OWNER), pk(7)]));
    let allocator = fx.allocator();
    let mut request = fx.request_body(99, 4000, 1800);
    request["owner"] = json!(pk(7));
    let result = allocator.submit(&fx.bundle(7, &request), fx.now).unwrap();
    assert_eq!(result["status"], "accepted");
    let lease_id = allocator.store().json(&result["lease"]).unwrap()["lease"]
        .as_str()
        .unwrap()
        .to_string();
    let status = allocator.status(&lease_id).unwrap();
    let command = json!({"v":"openagents.environment-control.v1","requires":[],"lease":result["lease"],"command":workspace_host::artifact::token(),"expected_revision":status["revision"],"action":"status","requested_until":null,"evidence":[]});
    let refused = allocator
        .submit(&fx.bundle(OWNER, &command), fx.now)
        .unwrap();
    assert_eq!(refused["status"], "refused");
    assert!(refused["lease"].is_null());
    assert!(refused["state"].is_null());
    let event = crate::transport::invoke(
        &secret(OWNER),
        allocator.pubkey(),
        &allocator.transport().unwrap(),
        command,
        workspace_host::artifact::token(),
        fx.now,
    )
    .unwrap();
    let (_, files) = allocator
        .serve(&event, &BTreeMap::new(), &[], fx.now)
        .unwrap();
    assert!(!files.contains_key(status["state"]["digest"].as_str().unwrap()));
    assert!(!files.contains_key(result["lease"]["digest"].as_str().unwrap()));
}
#[test]
fn observing_components_enforces_the_total_read_bound_after_files_change() {
    let temp = tempfile::tempdir().unwrap();
    let mut provider = LocalDirectories::new(temp.path().to_path_buf()).unwrap();
    let Create::Created(resource, _) = provider.create("local", &workspace_host::artifact::token())
    else {
        panic!()
    };
    provider
        .materialize(
            &resource,
            &[("a".into(), vec![1; 262144]), ("b".into(), vec![2; 262144])],
        )
        .unwrap();
    let path = provider.directory(&resource).unwrap().join("components");
    std::fs::write(path.join("a"), vec![1; 700000]).unwrap();
    std::fs::write(path.join("b"), vec![2; 700000]).unwrap();
    assert!(
        provider
            .observe_components(&resource, &["a".into(), "b".into()])
            .is_err()
    );
}
