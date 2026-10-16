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
            source: None,
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
fn git(repo: &std::path::Path, args: &[&str], input: Option<&[u8]>) -> String {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_AUTHOR_NAME", "AUTO fixture")
        .env("GIT_AUTHOR_EMAIL", "auto@example.test")
        .env("GIT_COMMITTER_NAME", "AUTO fixture")
        .env("GIT_COMMITTER_EMAIL", "auto@example.test")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(bytes) = input {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    } else {
        drop(child.stdin.take());
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn source_fixture() -> (
    tempfile::TempDir,
    coordination_host::book::Coordinator,
    source::Source,
    Value,
    Value,
    std::path::PathBuf,
    Value,
) {
    let (t, auto, base) = setup(json!({"kind":"once","at":110}));
    let repo = t.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "--bare"], None);
    git(&repo, &["config", "core.logAllRefUpdates", "always"], None);
    let blob = git(
        &repo,
        &["hash-object", "-w", "--stdin"],
        Some(b"source content\n"),
    );
    let tree = git(
        &repo,
        &["mktree"],
        Some(format!("100644 blob {blob}\tdoc.txt\n").as_bytes()),
    );
    let commit = git(&repo, &["commit-tree", &tree], Some(b"initial\n"));
    git(&repo, &["update-ref", "refs/heads/docs", &commit], None);
    let ws = workspace_host::host::Host::create(t.path().join("ws"), secret(1), 100).unwrap();
    let doc = ws
        .register_git(
            workspace_host::git::GitDocument::admit(&repo, "refs/heads/docs", "doc.txt").unwrap(),
            "Source".into(),
            100,
        )
        .unwrap();
    let coor = auto.coordinator.mount_source("docs".into(), ws).unwrap();
    let resource = coor
        .resource("doc".into(), "docs".into(), json!(doc), 100)
        .unwrap();
    let reader = coor.automation_source_operation("doc").unwrap();
    let mut ceiling = effects();
    ceiling["reads"] = json!(["artifact", "doc"]);
    ceiling["delegates"] = json!(true);
    let container = coor.container_operation(ceiling.clone()).unwrap();
    let scope = coor
        .create_scope(
            coordination_host::book::Policy {
                recipients: BTreeSet::from([key(1), key(2), key(3), key(4), key(5)]),
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
            vec![resource],
            vec![reader.clone(), container.clone(), base["target"].clone()],
            100,
        )
        .unwrap();
    for n in [2, 3, 4, 5] {
        coor.control_grants(&event(1,1,json!({"v":"openagents.coordination-native-control.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":coor.scope(&scope).unwrap()["revision"],"actor":key(n),"roles":if n==4{json!(["read"])}else{json!(["read","propose","claim","control","finding"])},"resources":[],"targets":[],"all":true,"until":2000,"revoke":false,"scope_revoke":false}),100),100).unwrap();
    }
    coor.frame(&event(1,1,json!({"v":"openagents.coordination-native-frame.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":coor.scope(&scope).unwrap()["revision"],"frame":base["frame"]}),100),&coor.get_local_bytes(&base["frame"]).unwrap(),100).unwrap();
    let lock=coor.put_local_json(&json!({"v":"openagents.lock.v1","requires":[],"root":container,"entries":[{"id":container["id"],"definition":container,"dependencies":[]}]})).unwrap();
    let descriptor: Value = serde_json::from_slice(&coor.get_local_bytes(&scope).unwrap()).unwrap();
    let frame: Value =
        serde_json::from_slice(&coor.get_local_bytes(&base["frame"]).unwrap()).unwrap();
    let context=coor.put_local_json(&json!({"v":"openagents.context.v1","requires":[],"task":frame["task"],"recipient":key(5),"policy":descriptor["policy"],"entries":[],"omissions":[],"coverage":"complete"})).unwrap();
    let task = workspace_host::artifact::token();
    let parent = json!({"v":"openagents.task-proposal.v1","requires":[],"task":task,"coordinator":scope,"frame":base["frame"],"parent":null,"target":container,"lock":lock,"input":base["payload"],"context":context,"dependencies":[],"effects":ceiling,"bounds":bounds(24,100000,1048576),"acceptance":base["acceptance"],"reuse":"never"});
    let pr = coor
        .propose(&event(1, 1, parent, 100), &BTreeMap::new(), 100)
        .unwrap()["value"]
        .clone();
    let claim=coor.claim(&event(1,1,json!({"v":"openagents.claim-request.v1","requires":[],"proposal":pr,"worker":key(5),"resources":[],"expected_revision":coor.scope(&scope).unwrap()["revision"],"requested_until":1000,"reservation":coor.scope(&scope).unwrap()["allowance"]}),100),100).unwrap()["value"].clone();
    let source = source::Source::initialize(
        t.path().join("source4"),
        secret(4),
        source::Policy {
            controller: key(5),
            coordinator: key(1),
            scope: scope.clone(),
            resource: "doc".into(),
            until: 1000,
        },
    )
    .unwrap();
    (t, coor, source, scope, claim, repo, base)
}
struct NativeSourceGate;
impl source::SourceGate for NativeSourceGate {
    fn with_fence(
        &self,
        p: &source::SourceIntake,
        _now: u64,
        call: &mut dyn FnMut() -> Result<source::Reply>,
    ) -> Result<source::Reply> {
        assert_eq!(p.authority(), key(4));
        assert_eq!(p.principal(), key(5));
        assert_eq!(p.admission()["resource"], "doc");
        call()
    }
}
#[test]
fn source_cj_preserves_original_signers_and_reads_actual_workspace_versions() {
    let (_t, coor, source, scope, claim, _repo, _base) = source_fixture();
    let bundle = source.binding().unwrap();
    let marker = coor
        .put_local_json(&json!({"fixture":"poll identity"}))
        .unwrap();
    let request = workspace_host::artifact::token();
    let admission = event(
        5,
        1,
        json!({"v":"openagents.automation-native-source-input.v1","requires":[],"coordinator":scope,"claim":claim,"source":coor.automation_source_operation("doc").unwrap(),"resource":"doc","request":request,"phase":"observe","poll":marker,"plan":marker,"executor":key(4)}),
        110,
    );
    let input = json!({"v":"openagents.automation-native-source-call.v1","requires":[],"config":bundle["config"],"admission":admission});
    let execute = coordination_host::transport::invoke(
        &secret(5),
        source.authority(),
        &bundle,
        input,
        request.clone(),
        110,
    )
    .unwrap();
    let (response, files) = source.run(&coor, &execute, 110, &NativeSourceGate).unwrap();
    let result = nostr::execution::bind_worker_event(
        &response,
        &nostr::execution::Pending {
            worker: &key(4),
            customer: &key(5),
            request: &request,
            attempt: 1,
            execute_event: &execute.id,
        },
        &secret(5),
    )
    .unwrap();
    assert_eq!(result["outcome"], "completed");
    let output =
        nostr::automation::parse(&files[result["output"]["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(output["source"], bundle["target"]);
    assert_eq!(output["available"], true);
    assert_eq!(output["observed_at"], 110);
    let mut signers = BTreeSet::new();
    for bytes in files.values() {
        if let Ok(e) = serde_json::from_slice::<Event>(bytes) {
            if let Ok(opened) = nostr::private_artifact::open(&e, &secret(5)) {
                signers.insert(opened.signer().to_string());
            }
        }
    }
    assert_eq!(signers, BTreeSet::from([key(1), key(4)]));
    assert_eq!(coor.scope(&scope).unwrap()["calls"], 1);
    assert_eq!(
        source
            .run(&coor, &execute, 150, &NativeSourceGate)
            .unwrap()
            .0,
        response
    );
}
fn watched() -> (
    tempfile::TempDir,
    Automation,
    Value,
    Value,
    std::path::PathBuf,
) {
    watched_with(16)
}
fn watched_with(
    max_observations: u64,
) -> (
    tempfile::TempDir,
    Automation,
    Value,
    Value,
    std::path::PathBuf,
) {
    let (t, coor, source, scope, claim, repo, mut plan) = source_fixture();
    let bundle = source.binding().unwrap();
    let scope_state = coor.scope(&scope).unwrap();
    let claim_body: Value = serde_json::from_slice(&coor.get_local_bytes(&claim).unwrap()).unwrap();
    let parent = scope_state["proposals"]
        .as_object()
        .unwrap()
        .values()
        .find(|p| p["reference"] == claim_body["proposal"])
        .unwrap();
    let frame: Value =
        serde_json::from_slice(&coor.get_local_bytes(&plan["frame"]).unwrap()).unwrap();
    let descriptor: Value = serde_json::from_slice(&coor.get_local_bytes(&scope).unwrap()).unwrap();
    plan["context"]=coor.put_local_json(&json!({"v":"openagents.context.v1","requires":[],"task":frame["task"],"recipient":key(2),"policy":descriptor["policy"],"entries":[],"omissions":[],"coverage":"complete"})).unwrap();
    plan["coordinator"] = scope.clone();
    plan["reservation"] = claim_body["reservation"].clone();
    plan["task"] = parent["body"]["task"].clone();
    plan["per_occurrence"] = bounds(4, 100000, 1048576);
    plan["aggregate"] = bounds(24, 2400000, 25165824);
    plan["effects"]["reads"] = json!(["artifact", "doc"]);
    plan["trigger"] = json!({"kind":"source","first_at":110,"every_seconds":60,"source":bundle["target"],"resource":"doc","lock":bundle["lock"],"input":bundle["config"],"context":bundle["context"],"requirements":bundle["requirements"],"bounds":bounds(1,30000,1048576),"max_age_seconds":60});
    let auto = Automation::initialize(
        t.path().join("watched"),
        secret(5),
        Policy {
            owners: BTreeSet::from([key(1)]),
            coordinator: scope,
            worker: key(2),
            checker: key(3),
            source: Some(bundle),
            max_plans: 4,
            until: 1000,
        },
        coor,
    )
    .unwrap()
    .mount_source(source)
    .unwrap();
    plan["policy"] = auto.policy().unwrap();
    plan["max_observations"] = json!(max_observations);
    let reference = admitted(&auto, &plan);
    (t, auto, plan, reference, repo)
}
#[test]
fn watched_plan_observes_revalidates_and_coalesces_the_exact_native_revision() {
    let (t, auto, plan, reference, repo) = watched();
    auto.tick(&reference, 110).unwrap();
    let first = auto.inspect(&reference).unwrap();
    assert_eq!(first["state"]["occurrences"], 1);
    assert_eq!(first["state"]["observations"], 1);
    let request = auto.advance(&reference, 110).unwrap().unwrap();
    let worker =
        coordination_host::worker::Worker::open(t.path().join("watched-worker"), secret(2), 110)
            .unwrap();
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
    let checked = auto.inspect(&reference).unwrap();
    assert_eq!(checked["state"]["observations"], 2);
    assert_eq!(checked["accounting"]["spent"]["calls"], 5);
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        5
    );
    auto.tick(&reference, 170).unwrap();
    auto.tick(&reference, 170).unwrap();
    let unchanged = auto.inspect(&reference).unwrap();
    assert_eq!(unchanged["state"]["occurrences"], 1);
    assert_eq!(unchanged["state"]["observations"], 3);
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        6
    );
    let old = git(&repo, &["rev-parse", "refs/heads/docs"], None);
    let tree = git(&repo, &["rev-parse", "refs/heads/docs^{tree}"], None);
    let next = git(
        &repo,
        &["commit-tree", &tree, "-p", &old],
        Some(b"changed source\n"),
    );
    git(&repo, &["update-ref", "refs/heads/docs", &next], None);
    auto.tick(&reference, 230).unwrap();
    auto.tick(&reference, 230).unwrap();
    let changed = auto.inspect(&reference).unwrap();
    assert_eq!(changed["state"]["occurrences"], 2);
    assert_eq!(changed["state"]["observations"], 4);
}
#[test]
fn a_changed_source_refuses_worker_intake_and_retains_the_original_occurrence_hold() {
    let (t, auto, plan, reference, repo) = watched();
    auto.tick(&reference, 110).unwrap();
    let request = auto.advance(&reference, 110).unwrap().unwrap();
    let old = git(&repo, &["rev-parse", "refs/heads/docs"], None);
    let tree = git(&repo, &["rev-parse", "refs/heads/docs^{tree}"], None);
    let next = git(
        &repo,
        &["commit-tree", &tree, "-p", &old],
        Some(b"new source revision\n"),
    );
    git(&repo, &["update-ref", "refs/heads/docs", &next], None);
    let worker =
        coordination_host::worker::Worker::open(t.path().join("stale-worker"), secret(2), 111)
            .unwrap();
    assert!(
        worker
            .run_gated(&auto.coordinator, &request, 111, &auto)
            .unwrap_err()
            .contains("stale")
    );
    let inspected = auto.inspect(&reference).unwrap();
    assert_eq!(inspected["state"]["status"], "blocked");
    assert_eq!(inspected["state"]["occurrences"], 1);
    assert_eq!(inspected["state"]["observations"], 2);
    assert_eq!(inspected["accounting"]["spent"]["calls"], 1);
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        3
    );
}
#[test]
fn unavailable_source_is_counted_without_a_revision_or_an_occurrence() {
    let (_t, auto, plan, reference, repo) = watched_with(2);
    git(&repo, &["update-ref", "-d", "refs/heads/docs"], None);
    auto.tick(&reference, 110).unwrap();
    auto.tick(&reference, 110).unwrap();
    let first = auto.inspect(&reference).unwrap();
    assert_eq!(first["state"]["observations"], 1);
    assert_eq!(first["state"]["occurrences"], 0);
    assert_eq!(first["accounting"]["spent"]["calls"], 1);
    assert!(first["skips"].as_array().unwrap().is_empty());
    let poll = first["polls"].as_object().unwrap().values().next().unwrap();
    let observation = auto.store.json(&poll["observation"]).unwrap();
    assert_eq!(observation["available"], false);
    assert!(observation["revision"].is_null());
    assert!(observation["evidence"].is_null());
    auto.tick(&reference, 170).unwrap();
    auto.tick(&reference, 230).unwrap();
    let exhausted = auto.inspect(&reference).unwrap();
    assert_eq!(exhausted["state"]["observations"], 2);
    assert_eq!(exhausted["state"]["status"], "exhausted");
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        2
    );
}
#[test]
fn pause_and_cancel_fence_source_intake_before_the_native_reader_is_charged() {
    let (_t, auto, plan, reference, _repo) = watched();
    let request = {
        let _guard = auto.store.lock().unwrap();
        let mut state = auto.state().unwrap();
        let id = plan["automation"].as_str().unwrap().to_string();
        let mut record = state.plans[&id].clone();
        let request = auto
            .prepare_poll(&mut state, &mut record, "observe", Some(0), None, 110)
            .unwrap();
        record.next_slot = 1;
        auto.transition(&mut state, &mut record, "active", vec![], 110)
            .unwrap();
        state.plans.insert(id, record);
        auto.save(&state).unwrap();
        request
    };
    let pause = control(&auto, &reference, "pause", 111);
    auto.control(&event(1, 5, pause, 111), 111).unwrap();
    assert!(
        auto.source
            .as_ref()
            .unwrap()
            .run(&auto.coordinator, &request, 111, &auto)
            .is_err()
    );
    let cancel = control(&auto, &reference, "cancel", 112);
    auto.control(&event(1, 5, cancel, 112), 112).unwrap();
    assert!(
        auto.source
            .as_ref()
            .unwrap()
            .run(&auto.coordinator, &request, 112, &auto)
            .is_err()
    );
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        0
    );
    let inspected = auto.inspect(&reference).unwrap();
    assert_eq!(inspected["state"]["status"], "cancelled");
    assert_eq!(inspected["state"]["observations"], 1);
    assert_eq!(inspected["accounting"]["spent"], json!({}));
    assert!(inspected["pending"].is_object());
}
#[test]
fn source_revalidation_uses_the_same_observation_cap_and_expiry_stops_new_reads() {
    let (t, auto, plan, reference, _repo) = watched_with(2);
    auto.tick(&reference, 110).unwrap();
    let request = auto.advance(&reference, 110).unwrap().unwrap();
    let worker =
        coordination_host::worker::Worker::open(t.path().join("capped-worker"), secret(2), 110)
            .unwrap();
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
    auto.tick(&reference, 170).unwrap();
    assert_eq!(
        auto.inspect(&reference).unwrap()["state"]["status"],
        "exhausted"
    );
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        5
    );
    let (_t, auto, plan, reference, _repo) = watched();
    auto.tick(&reference, 900).unwrap();
    assert_eq!(
        auto.inspect(&reference).unwrap()["state"]["status"],
        "expired"
    );
    assert_eq!(
        auto.inspect(&reference).unwrap()["state"]["observations"],
        0
    );
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        0
    );
}
