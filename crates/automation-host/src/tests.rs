use super::*;
use host::{Automation, Policy};
use nostr::domain::{Event, RelaySigner};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn key(n: u8) -> String {
    RelaySigner::from_secret_hex(&secret(n).display_secret().to_string())
        .unwrap()
        .pubkey()
        .into()
}
fn event(n: u8, to: u8, body: Value, now: u64) -> Event {
    workspace_host::host::request(&secret(n), &key(to), body, now).unwrap()
}
fn effects() -> Value {
    json!({"reads":["artifact"],"writes":[],"network":[],"process":false,"delegates":false,"spend":false})
}
fn bounds(calls: u64, wall: u64, bytes: u64) -> Value {
    json!([
 {"bound":"calls","ceiling":calls,"assurance":"host","mechanism":"native-ledger"},
 {"bound":"wall_ms","ceiling":wall,"assurance":"host","mechanism":"native-deadline"},
 {"bound":"input_bytes","ceiling":bytes,"assurance":"host","mechanism":"native-bytes"},
 {"bound":"output_bytes","ceiling":bytes,"assurance":"host","mechanism":"native-bytes"}])
}
fn setup(trigger: Value) -> (tempfile::TempDir, Automation, Value) {
    let t = tempfile::tempdir().unwrap();
    let coor =
        coordination_host::book::Coordinator::initialize(t.path().join("coor"), secret(1)).unwrap();
    let target = coor.automation_operation().unwrap();
    let mut ceiling = effects();
    ceiling["delegates"] = json!(true);
    let container = coor.container_operation(ceiling.clone()).unwrap();
    let scope = coor
        .create_scope(
            coordination_host::book::Policy {
                recipients: BTreeSet::from([key(1), key(2), key(3), key(5)]),
                checkers: BTreeSet::from([key(3)]),
                calls: 24,
                concurrency: 8,
                max_claim_seconds: 100,
                max_container_seconds: Some(1000),
                max_depth: 8,
                until: 2000,
                effects: ceiling.clone(),
                parent: None,
            },
            vec![],
            vec![target.clone(), container.clone()],
            100,
        )
        .unwrap();
    for n in [2, 3, 5] {
        let ctl = json!({"v":"openagents.coordination-native-control.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":coor.scope(&scope).unwrap()["revision"],"actor":key(n),"roles":["read","propose","claim","control","finding"],"resources":[],"targets":[],"all":true,"until":2000,"revoke":false,"scope_revoke":false});
        coor.control_grants(&event(1, 1, ctl, 100), 100).unwrap();
    }
    let acceptance=coor.put_local_json(&json!({"v":"openagents.coordination-native-check-policy.v1","requires":[],"check":"exact-admitted-output"})).unwrap();
    let text = coor
        .put_local_bytes(b"Objective", "text/plain", None)
        .unwrap();
    let snapshot=coor.put_local_json(&json!({"v":"openagents.snapshot.v1","requires":[],"scope":"auto-tests","captured_at":100,"sources":[],"coverage":"complete"})).unwrap();
    let task = workspace_host::artifact::token();
    let precedence = coor
        .put_local_json(
            &json!({"v":"openagents.coordination-empty-instruction-policy.v1","requires":[]}),
        )
        .unwrap();
    let instructions=coor.put_local_json(&json!({"v":"openagents.instructions.v1","requires":[],"task":task,"revision":0,"resolver":{"id":format!("{}:coord/empty-instructions",key(1)),"artifact":precedence},"precedence":precedence,"snapshot":snapshot,"entries":[]})).unwrap();
    let frame=coor.put_local_json(&json!({"v":"openagents.task-frame.v1","requires":[],"task":task,"owner":key(1),"controller":key(1),"revision":0,"previous":null,"objective":text,"origin":"user","constraints":[],"acceptance":[acceptance],"snapshot":snapshot,"instructions":instructions,"variables":[],"attempts":[],"unresolved":[],"corrections":[]})).unwrap();
    coor.frame(&event(1,1,json!({"v":"openagents.coordination-native-frame.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":coor.scope(&scope).unwrap()["revision"],"frame":frame}),100),&coor.get_local_bytes(&frame).unwrap(),100).unwrap();
    let descriptor: Value = serde_json::from_slice(&coor.get_local_bytes(&scope).unwrap()).unwrap();
    let context = |recipient: u8| {
        coor.put_local_json(&json!({"v":"openagents.context.v1","requires":[],"task":task,"recipient":key(recipient),"policy":descriptor["policy"],"entries":[],"omissions":[],"coverage":"complete"})).unwrap()
    };
    let lock = |target: &Value| {
        coor.put_local_json(&json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":target["id"],"definition":target,"dependencies":[]}]})).unwrap()
    };
    let parent_task = workspace_host::artifact::token();
    let payload = coor
        .put_local_json(&json!({"objective":"bounded data"}))
        .unwrap();
    let parent = json!({"v":"openagents.task-proposal.v1","requires":[],"task":parent_task,"coordinator":scope,"frame":frame,"parent":null,"target":container,"lock":lock(&container),"input":payload,"context":context(5),"dependencies":[],"effects":ceiling,"bounds":bounds(24,100000,1048576),"acceptance":acceptance,"reuse":"never"});
    let proposal = coor
        .propose(&event(1, 1, parent, 100), &BTreeMap::new(), 100)
        .unwrap()["value"]
        .clone();
    let claim=coor.claim(&event(1,1,json!({"v":"openagents.claim-request.v1","requires":[],"proposal":proposal,"worker":key(5),"resources":[],"expected_revision":coor.scope(&scope).unwrap()["revision"],"requested_until":1000,"reservation":coor.scope(&scope).unwrap()["allowance"]}),100),100).unwrap()["value"].clone();
    let claim_body: Value = serde_json::from_slice(&coor.get_local_bytes(&claim).unwrap()).unwrap();
    let target_lock = lock(&target);
    let target_context = context(2);
    let requirements=coor.put_local_json(&json!({"v":"openagents.automation-native-requirements.v1","requires":[],"spend_microunits":0})).unwrap();
    let auto = Automation::initialize(
        t.path().join("auto"),
        secret(5),
        Policy {
            owners: BTreeSet::from([key(1)]),
            coordinator: scope.clone(),
            worker: key(2),
            checker: key(3),
            max_plans: 4,
            until: 1000,
        },
        coor,
    )
    .unwrap();
    let plan = json!({"v":"openagents.automation-plan.v1","requires":[],"automation":workspace_host::artifact::token(),"run":workspace_host::artifact::token(),"owner":key(1),"controller":key(5),"coordinator":scope,"frame":frame,"policy":auto.policy().unwrap(),"reservation":claim_body["reservation"],"task":parent_task,"target":target,"lock":target_lock,"payload":payload,"context":target_context,"requirements":requirements,"acceptance":acceptance,"effects":effects(),"per_occurrence":bounds(3,100000,65536),"aggregate":bounds(24,800000,524288),"max_occurrences":8,"max_observations":16,"starts_at":100,"expires_at":900,"retain_until":2000,"trigger":trigger,"completion":null});
    (t, auto, plan)
}
#[test]
fn once_uses_actual_worker_independent_check_and_nonrenewable_parent() {
    let (t, auto, plan) = setup(json!({"kind":"once","at":110}));
    let admitted = auto
        .admit(&event(1, 5, plan.clone(), 100), &BTreeMap::new(), 100)
        .unwrap();
    let reference = admitted["input"].clone();
    auto.tick(&reference, 110).unwrap();
    let request = auto.advance(&reference, 110).unwrap().unwrap();
    let worker =
        coordination_host::worker::Worker::open(t.path().join("worker"), secret(2), 110).unwrap();
    assert!(worker.run(&auto.coordinator, &request, 110).is_err());
    let (result, files) = worker
        .run_gated(&auto.coordinator, &request, 110, &auto)
        .unwrap();
    auto.record_result(&reference, &result, &files, 111)
        .unwrap();
    auto.check(
        &reference,
        &coordination_host::checks::Checker::new(secret(3)),
        112,
    )
    .unwrap();
    let inspected = auto.inspect(&reference).unwrap();
    assert_eq!(inspected["state"]["status"], "complete");
    assert_eq!(inspected["state"]["occurrences"], 1);
    assert_eq!(inspected["accounting"]["spent"]["calls"], 3);
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        3
    );
    assert_eq!(
        auto.admit(&event(1, 5, plan, 120), &BTreeMap::new(), 120)
            .unwrap()["status"],
        "duplicate"
    );
}
fn admitted(auto: &Automation, plan: &Value) -> Value {
    auto.admit(&event(1, 5, plan.clone(), 100), &BTreeMap::new(), 100)
        .unwrap()["input"]
        .clone()
}
fn control(auto: &Automation, reference: &Value, action: &str, _now: u64) -> Value {
    let revision = auto.inspect(reference).unwrap()["state"]["revision"].clone();
    json!({"v":"openagents.automation-control.v1","requires":[],"plan":reference,"command":workspace_host::artifact::token(),"expected_revision":revision,"action":action,"reason":"owner request"})
}
#[test]
fn pause_cancel_and_restart_fence_queued_intake_and_commands_are_exact() {
    let (t, auto, plan) =
        setup(json!({"kind":"interval","first_at":110,"every_seconds":60,"missed":"latest"}));
    let reference = admitted(&auto, &plan);
    auto.tick(&reference, 110).unwrap();
    let request = auto.advance(&reference, 110).unwrap().unwrap();
    let pause = control(&auto, &reference, "pause", 111);
    let result = auto.control(&event(1, 5, pause.clone(), 111), 111).unwrap();
    assert_eq!(
        auto.control(&event(1, 5, pause.clone(), 112), 112).unwrap(),
        result
    );
    let mut changed = pause;
    changed["action"] = json!("cancel");
    assert!(
        auto.control(&event(1, 5, changed, 112), 112)
            .unwrap_err()
            .contains("idempotency_conflict")
    );
    let worker =
        coordination_host::worker::Worker::open(t.path().join("worker"), secret(2), 112).unwrap();
    assert!(
        worker
            .run_gated(&auto.coordinator, &request, 112, &auto)
            .is_err()
    );
    let resume = control(&auto, &reference, "resume", 113);
    auto.control(&event(1, 5, resume, 113), 113).unwrap();
    let cancel = control(&auto, &reference, "cancel", 114);
    auto.control(&event(1, 5, cancel, 114), 114).unwrap();
    drop(auto);
    let coor =
        coordination_host::book::Coordinator::open(t.path().join("coor"), secret(1)).unwrap();
    let auto = Automation::open(t.path().join("auto"), secret(5), coor).unwrap();
    assert!(
        worker
            .run_gated(&auto.coordinator, &request, 115, &auto)
            .is_err()
    );
    assert_eq!(
        auto.inspect(&reference).unwrap()["accounting"]["requests"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        auto.inspect(&reference).unwrap()["state"]["status"],
        "cancelled"
    );
}
#[test]
fn downtime_coalesces_exact_skipped_ranges_and_an_unknown_child_blocks_another() {
    let (t, auto, plan) =
        setup(json!({"kind":"interval","first_at":110,"every_seconds":60,"missed":"latest"}));
    let reference = admitted(&auto, &plan);
    auto.tick(&reference, 299).unwrap();
    let first = auto.inspect(&reference).unwrap();
    assert_eq!(first["state"]["occurrences"], 1);
    let skip: Value = serde_json::from_slice(&auto.store.get(&first["skips"][0]).unwrap()).unwrap();
    assert_eq!(skip["first_slot"], 0);
    assert_eq!(skip["last_slot"], 2);
    let request = auto.advance(&reference, 299).unwrap().unwrap();
    let worker =
        coordination_host::worker::Worker::open(t.path().join("worker"), secret(2), 299).unwrap();
    let _lost = worker
        .run_gated(&auto.coordinator, &request, 299, &auto)
        .unwrap();
    auto.tick(&reference, 600).unwrap();
    assert_eq!(auto.inspect(&reference).unwrap()["state"]["occurrences"], 1);
    let pause = control(&auto, &reference, "pause", 600);
    auto.control(&event(1, 5, pause, 600), 600).unwrap();
    let resume = control(&auto, &reference, "resume", 601);
    assert!(
        auto.control(&event(1, 5, resume, 601), 601)
            .unwrap_err()
            .contains("unknown")
    );
    assert_eq!(
        auto.inspect(&reference).unwrap()["accounting"]["spent"],
        json!({})
    );
}
#[test]
fn owner_identity_pins_and_parent_reuse_refuse_before_activation() {
    let (_t, auto, plan) = setup(json!({"kind":"once","at":110}));
    assert!(
        auto.admit(&event(2, 5, plan.clone(), 100), &BTreeMap::new(), 100)
            .is_err()
    );
    let mut bad = plan.clone();
    bad["target"] = auto.coordinator.data_operation().unwrap();
    assert!(
        auto.admit(&event(1, 5, bad, 100), &BTreeMap::new(), 100)
            .is_err()
    );
    admitted(&auto, &plan);
    let mut other = plan.clone();
    other["automation"] = json!(workspace_host::artifact::token());
    other["run"] = json!(workspace_host::artifact::token());
    assert!(
        auto.admit(&event(1, 5, other, 100), &BTreeMap::new(), 100)
            .unwrap_err()
            .contains("reservation")
    );
    let mut changed = plan;
    changed["max_occurrences"] = json!(1);
    assert!(
        auto.admit(&event(1, 5, changed, 100), &BTreeMap::new(), 100)
            .unwrap_err()
            .contains("immutable")
    );
}
#[test]
fn controller_cj_pins_exact_retries_and_independent_plan_run_are_enforced() {
    let (t, auto, plan) = setup(json!({"kind":"once","at":110}));
    let bundle = auto.transport().unwrap();
    let card: Value =
        serde_json::from_slice(&auto.store.get(&bundle["target"]["artifact"]).unwrap()).unwrap();
    nostr::cap::parse_definition(&card).unwrap();
    let request = coordination_host::transport::invoke(
        &secret(1),
        auto.authority(),
        &bundle,
        plan.clone(),
        workspace_host::artifact::token(),
        100,
    )
    .unwrap();
    assert!(auto.admit(&request, &BTreeMap::new(), 100).is_err());
    let (response, files) = auto.serve(&request, &BTreeMap::new(), 100).unwrap();
    let execute = nostr::execution::open_request(
        &request,
        &key(5),
        &secret(5),
        100,
        nostr::execution::Window::DEFAULT,
    )
    .unwrap();
    let nostr::execution::Body::Execute(execute) = execute.body else {
        panic!()
    };
    let result = nostr::execution::bind_worker_event(
        &response,
        &nostr::execution::Pending {
            worker: &key(5),
            customer: &key(1),
            request: &execute.request,
            attempt: 1,
            execute_event: &request.id,
        },
        &secret(1),
    )
    .unwrap();
    assert_eq!(result["outcome"], "completed");
    let output: Value =
        serde_json::from_slice(&files[result["output"]["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(output["status"], "accepted");
    assert!(files.values().map(Vec::len).sum::<usize>() <= 1048576);
    let reference = output["input"].clone();
    assert_eq!(auto.inspect(&reference).unwrap()["state"]["occurrences"], 0);
    assert_eq!(
        auto.serve(&request, &BTreeMap::new(), 120).unwrap().0,
        response
    );
    drop(auto);
    let coor =
        coordination_host::book::Coordinator::open(t.path().join("coor"), secret(1)).unwrap();
    let auto = Automation::open(t.path().join("auto"), secret(5), coor).unwrap();
    assert_eq!(auto.recover_controller(&request, 120).unwrap().0, response);
    let mut bad_bundle = bundle;
    bad_bundle["requirements"] = bad_bundle["context"].clone();
    let bad = coordination_host::transport::invoke(
        &secret(1),
        auto.authority(),
        &bad_bundle,
        plan,
        workspace_host::artifact::token(),
        120,
    )
    .unwrap();
    assert!(
        auto.serve(&bad, &BTreeMap::new(), 120)
            .unwrap_err()
            .contains("pins")
    );
}
#[test]
fn interrupted_controller_recovers_exact_domain_result_or_fenced_absence() {
    for complete_domain in [false, true] {
        let (_t, auto, plan) = setup(json!({"kind":"once","at":110}));
        let request = coordination_host::transport::invoke(
            &secret(1),
            auto.authority(),
            &auto.transport().unwrap(),
            plan.clone(),
            workspace_host::artifact::token(),
            100,
        )
        .unwrap();
        let opened = nostr::execution::open_request(
            &request,
            auto.authority(),
            &secret(5),
            100,
            nostr::execution::Window::DEFAULT,
        )
        .unwrap();
        {
            let _guard = auto.store.lock().unwrap();
            let mut state = auto.state().unwrap();
            let nostr::execution::Admission::Reserved { claim, root } = state
                .service
                .prepare(&opened, 100, &workspace_host::artifact::token())
                .unwrap()
            else {
                panic!()
            };
            let reference = auto
                .store
                .put(&root, "application/json", Some(nostr::run::RECORD_SCHEMA))
                .unwrap();
            state
                .reply_artifacts
                .insert(claim.key.clone(), vec![reference]);
            state.service.acknowledge(&claim.key, &root).unwrap();
            state.service.intend(&claim.key).unwrap();
            auto.save(&state).unwrap();
        }
        let result = if complete_domain {
            Some(auto.admit(&request, &BTreeMap::new(), 100).unwrap())
        } else {
            None
        };
        assert!(
            auto.serve(&request, &BTreeMap::new(), 101)
                .unwrap_err()
                .contains("no replay")
        );
        let (reply, files) = auto.recover_controller(&request, 101).unwrap();
        let nostr::execution::Body::Execute(execute) = opened.body else {
            panic!()
        };
        let body = nostr::execution::bind_worker_event(
            &reply,
            &nostr::execution::Pending {
                worker: &key(5),
                customer: &key(1),
                request: &execute.request,
                attempt: 1,
                execute_event: &request.id,
            },
            &secret(1),
        )
        .unwrap();
        assert_eq!(body["dispatched"], complete_domain);
        let output: Value =
            serde_json::from_slice(&files[body["output"]["digest"].as_str().unwrap()]).unwrap();
        assert_eq!(
            output["status"],
            if complete_domain {
                "accepted"
            } else {
                "refused"
            }
        );
        if let Some(result) = result {
            assert_eq!(output, result);
        }
        assert!(
            auto.admit(&request, &BTreeMap::new(), 101)
                .unwrap_err()
                .contains("revoked")
        );
        assert_eq!(
            auto.state().unwrap().plans.len(),
            usize::from(complete_domain)
        );
    }
}
#[test]
fn cj_byte_bound_and_foreign_owner_refuse_before_creating_service_history() {
    let (_t, auto, plan) = setup(json!({"kind":"once","at":110}));
    let good = coordination_host::transport::invoke(
        &secret(1),
        auto.authority(),
        &auto.transport().unwrap(),
        plan.clone(),
        workspace_host::artifact::token(),
        100,
    )
    .unwrap();
    let opened = nostr::execution::open_request(
        &good,
        auto.authority(),
        &secret(5),
        100,
        nostr::execution::Window::DEFAULT,
    )
    .unwrap();
    let nostr::execution::Body::Execute(execute) = opened.body else {
        panic!()
    };
    let mut payload = execute.payload;
    payload["bounds"]["output_bytes"] = json!(1);
    let signer = RelaySigner::from_secret_hex(&secret(1).display_secret().to_string()).unwrap();
    let peer = key(5).parse::<secp256k1::XOnlyPublicKey>().unwrap();
    let bad = nostr::execution::Seal {
        signer: &signer,
        conversation: nostr::nip44::conversation_key(&secret(1), &peer),
        nonce: secp256k1::rand::random(),
        created_at: 100,
    }
    .event(nostr::execution::REQUEST_KIND, good.tags.clone(), &payload)
    .unwrap();
    assert!(
        auto.serve(&bad, &BTreeMap::new(), 100)
            .unwrap_err()
            .contains("bounds")
    );
    let foreign = coordination_host::transport::invoke(
        &secret(2),
        auto.authority(),
        &auto.transport().unwrap(),
        plan,
        workspace_host::artifact::token(),
        100,
    )
    .unwrap();
    assert!(auto.serve(&foreign, &BTreeMap::new(), 100).is_err());
    assert!(auto.state().unwrap().service.claims.is_empty());
    assert!(auto.state().unwrap().plans.is_empty());
}
