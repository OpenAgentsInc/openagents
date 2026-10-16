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
    setup_goal(trigger, None)
}
fn setup_goal(trigger: Value, goal: Option<u64>) -> (tempfile::TempDir, Automation, Value) {
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
    let rule=goal.map(|minimum|coor.put_local_json(&json!({"v":"openagents.automation-native-completion-rule.v1","requires":[],"check":"verified-occurrence-count","minimum_successes":minimum,"acceptance":acceptance})).unwrap());
    let mut criteria = vec![acceptance.clone()];
    if let Some(rule) = &rule {
        criteria.push(rule.clone());
    }
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
    let frame=coor.put_local_json(&json!({"v":"openagents.task-frame.v1","requires":[],"task":task,"owner":key(1),"controller":key(1),"revision":0,"previous":null,"objective":text,"origin":"user","constraints":[],"acceptance":criteria,"snapshot":snapshot,"instructions":instructions,"variables":[],"attempts":[],"unresolved":[],"corrections":[]})).unwrap();
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
    let evaluator = rule.map(|rule| {
        crate::evaluator::Evaluator::initialize(
            t.path().join("evaluator3"),
            secret(3),
            crate::evaluator::Policy {
                controller: key(5),
                coordinator: key(1),
                scope: scope.clone(),
                producer: key(2),
                rule,
                until: 1000,
            },
        )
        .unwrap()
    });
    let evaluation_binding = evaluator.as_ref().map(|e| e.binding().unwrap());
    let mut auto = Automation::initialize(
        t.path().join("auto"),
        secret(5),
        Policy {
            owners: BTreeSet::from([key(1)]),
            coordinator: scope.clone(),
            worker: key(2),
            checker: key(3),
            source: None,
            evaluator: evaluation_binding.clone(),
            max_plans: 4,
            until: 1000,
        },
        coor,
    )
    .unwrap();
    if let Some(evaluator) = evaluator {
        auto = auto.mount_evaluator(evaluator).unwrap();
    }
    let mut plan = json!({"v":"openagents.automation-plan.v1","requires":[],"automation":workspace_host::artifact::token(),"run":workspace_host::artifact::token(),"owner":key(1),"controller":key(5),"coordinator":scope,"frame":frame,"policy":auto.policy().unwrap(),"reservation":claim_body["reservation"],"task":parent_task,"target":target,"lock":target_lock,"payload":payload,"context":target_context,"requirements":requirements,"acceptance":acceptance,"effects":effects(),"per_occurrence":bounds(3,100000,65536),"aggregate":bounds(24,800000,524288),"max_occurrences":8,"max_observations":16,"starts_at":100,"expires_at":900,"retain_until":2000,"trigger":trigger,"completion":null});
    if let Some(binding) = evaluation_binding {
        plan["completion"] = json!({"target":binding["target"],"lock":binding["lock"],"context":binding["context"],"requirements":binding["requirements"],"bounds":bounds(1,30000,1048576)});
        plan["per_occurrence"] = bounds(4, 100000, 1048576);
        plan["aggregate"] = bounds(24, 2400000, 25165824);
    }
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
    let outcome_ref = &inspected["outcomes"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()["reference"];
    let outcome = auto.store.json(outcome_ref).unwrap();
    let settled = auto.store.json(&outcome["settled"]).unwrap();
    assert_eq!(settled["record"]["type"], "settled");
    assert_eq!(settled["record"]["controller"], key(2));
    assert_eq!(outcome["settled"]["schema"], nostr::run::RECORD_SCHEMA);
    assert_ne!(outcome["settled"], outcome["usage"][0]);

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
    nostr::run::terminal_from_result(&result, &files, &key(4)).unwrap();
    let output =
        nostr::automation::parse(&files[result["output"]["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(output["source"], bundle["target"]);
    assert_eq!(output["available"], true);
    assert!(
        output["observed_at"]
            .as_u64()
            .is_some_and(|t| (110..140).contains(&t))
    );
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
fn occurrence_time(auto: &Automation, reference: &Value) -> u64 {
    auto.inspect(reference).unwrap()["occurrences"]
        .as_object()
        .unwrap()
        .values()
        .map(|o| o["started_at"].as_u64().unwrap())
        .max()
        .unwrap()
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
            evaluator: None,
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
    let at = occurrence_time(&auto, &reference);
    let request = auto.advance(&reference, at).unwrap().unwrap();
    let worker =
        coordination_host::worker::Worker::open(t.path().join("watched-worker"), secret(2), at)
            .unwrap();
    let (result, files) = worker
        .run_gated(&auto.coordinator, &request, at, &auto)
        .unwrap();
    auto.record_result(&reference, &result, &files, at + 1)
        .unwrap();
    auto.check(
        &reference,
        &coordination_host::checks::Checker::new(secret(3)),
        at + 2,
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
    let at = occurrence_time(&auto, &reference);
    let request = auto.advance(&reference, at).unwrap().unwrap();
    let old = git(&repo, &["rev-parse", "refs/heads/docs"], None);
    let tree = git(&repo, &["rev-parse", "refs/heads/docs^{tree}"], None);
    let next = git(
        &repo,
        &["commit-tree", &tree, "-p", &old],
        Some(b"new source revision\n"),
    );
    git(&repo, &["update-ref", "refs/heads/docs", &next], None);
    let worker =
        coordination_host::worker::Worker::open(t.path().join("stale-worker"), secret(2), at + 1)
            .unwrap();
    assert!(
        worker
            .run_gated(&auto.coordinator, &request, at + 1, &auto)
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
    let at = occurrence_time(&auto, &reference);
    let request = auto.advance(&reference, at).unwrap().unwrap();
    let worker =
        coordination_host::worker::Worker::open(t.path().join("capped-worker"), secret(2), at)
            .unwrap();
    let (result, files) = worker
        .run_gated(&auto.coordinator, &request, at, &auto)
        .unwrap();
    auto.record_result(&reference, &result, &files, at + 1)
        .unwrap();
    auto.check(
        &reference,
        &coordination_host::checks::Checker::new(secret(3)),
        at + 2,
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
fn checked_occurrence(t: &tempfile::TempDir, auto: &Automation, reference: &Value, at: u64) {
    let request = auto
        .advance(reference, at)
        .unwrap_or_else(|e| panic!("advance at {at}: {e}"))
        .unwrap();
    let worker =
        coordination_host::worker::Worker::open(t.path().join("goal-worker2"), secret(2), at)
            .unwrap();
    let (result, files) = worker
        .run_gated(&auto.coordinator, &request, at, auto)
        .unwrap();
    auto.record_result(reference, &result, &files, at + 1)
        .unwrap();
    auto.check(
        reference,
        &coordination_host::checks::Checker::new(secret(3)),
        at + 2,
    )
    .unwrap();
}
#[test]
fn actual_independent_continuation_requires_a_valid_decision_and_the_minimum_interval() {
    let (t, auto, mut plan) = setup_goal(
        json!({"kind":"continuation","minimum_interval_seconds":20}),
        Some(2),
    );
    plan["payload"] = auto
        .coordinator
        .put_local_json(&json!({"model_claim":"complete"}))
        .unwrap();
    let reference = admitted(&auto, &plan);
    auto.tick(&reference, 100).unwrap();
    checked_occurrence(&t, &auto, &reference, 100);
    auto.tick(&reference, 105).unwrap();
    assert_eq!(auto.inspect(&reference).unwrap()["state"]["occurrences"], 1);
    assert_eq!(
        auto.inspect(&reference).unwrap()["state"]["status"],
        "active"
    );
    let first_ref = auto.evaluate(&reference, 105).unwrap();
    let first = auto.store.json(&first_ref).unwrap();
    assert_eq!(first["decision"], "continue");
    let inspected = auto.inspect(&reference).unwrap();
    let first_at = inspected["evaluations"]
        .as_object()
        .unwrap()
        .values()
        .last()
        .unwrap()["at"]
        .as_u64()
        .unwrap();
    assert!(inspected["state"]["active"].is_null());
    auto.tick(&reference, first_at + 19).unwrap();
    assert_eq!(auto.inspect(&reference).unwrap()["state"]["occurrences"], 1);
    auto.tick(&reference, first_at + 20).unwrap();
    assert_eq!(auto.inspect(&reference).unwrap()["state"]["occurrences"], 2);
    checked_occurrence(&t, &auto, &reference, first_at + 20);
    let final_ref = auto.evaluate(&reference, first_at + 25).unwrap();
    let final_result = auto.store.json(&final_ref).unwrap();
    assert_eq!(final_result["decision"], "complete");
    let completed = auto.inspect(&reference).unwrap();
    assert_eq!(completed["state"]["status"], "complete");
    assert!(completed["state"]["active"].is_null());
    assert_eq!(completed["accounting"]["spent"]["calls"], 8);
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        8
    );
}
#[test]
fn continuation_refuses_a_missing_checker_changed_pins_and_unprotected_criteria() {
    let (_t, mut auto, plan) = setup_goal(
        json!({"kind":"continuation","minimum_interval_seconds":20}),
        Some(2),
    );
    let mut bad = plan.clone();
    bad["completion"]["target"] = auto.coordinator.data_operation().unwrap();
    assert!(
        auto.admit(&event(1, 5, bad, 100), &BTreeMap::new(), 100)
            .is_err()
    );
    let mut bad = plan.clone();
    bad["completion"] = Value::Null;
    assert!(
        auto.admit(&event(1, 5, bad, 100), &BTreeMap::new(), 100)
            .is_err()
    );
    let mut bad = plan.clone();
    bad["per_occurrence"] = bounds(3, 100000, 1048576);
    assert!(
        auto.admit(&event(1, 5, bad, 100), &BTreeMap::new(), 100)
            .unwrap_err()
            .contains("allowance")
    );
    let evaluator = auto.evaluator.take().unwrap();
    assert!(
        auto.admit(&event(1, 5, plan.clone(), 100), &BTreeMap::new(), 100)
            .unwrap_err()
            .contains("mounted")
    );
    auto = auto.mount_evaluator(evaluator).unwrap();
    let mut frame: Value =
        serde_json::from_slice(&auto.coordinator.get_local_bytes(&plan["frame"]).unwrap()).unwrap();
    frame["revision"] = json!(1);
    frame["previous"] = plan["frame"].clone();
    frame["acceptance"] = json!([plan["acceptance"]]);
    let newer = auto.coordinator.put_local_json(&frame).unwrap();
    auto.coordinator.frame(&event(1,1,json!({"v":"openagents.coordination-native-frame.v1","requires":[],"coordinator":plan["coordinator"],"command":workspace_host::artifact::token(),"expected_revision":auto.coordinator.scope(&plan["coordinator"]).unwrap()["revision"],"frame":newer}),100),&auto.coordinator.get_local_bytes(&newer).unwrap(),100).unwrap();
    assert!(
        auto.admit(&event(1, 5, plan, 100), &BTreeMap::new(), 100)
            .is_err()
    );
    assert!(auto.state().unwrap().plans.is_empty());
}
#[test]
fn an_unresolved_child_prevents_completion_even_when_the_success_count_is_met() {
    let (t, auto, plan) = setup_goal(
        json!({"kind":"continuation","minimum_interval_seconds":20}),
        Some(1),
    );
    let reference = admitted(&auto, &plan);
    auto.tick(&reference, 100).unwrap();
    checked_occurrence(&t, &auto, &reference, 100);
    let inspected = auto.inspect(&reference).unwrap();
    let occurrence = inspected["occurrences"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap();
    let mut proposal: Value = serde_json::from_slice(
        &auto
            .coordinator
            .get_local_bytes(&occurrence["proposal"])
            .unwrap(),
    )
    .unwrap();
    proposal["task"] = json!(workspace_host::artifact::token());
    let pr = auto
        .coordinator
        .propose(
            &event(5, 1, proposal.clone(), 104),
            &auto.input_files(&proposal["input"]).unwrap(),
            104,
        )
        .unwrap()["value"]
        .clone();
    auto.coordinator.claim(&event(5,1,json!({"v":"openagents.claim-request.v1","requires":[],"proposal":pr,"worker":key(2),"resources":[],"expected_revision":auto.coordinator.scope(&plan["coordinator"]).unwrap()["revision"],"requested_until":190,"reservation":plan["reservation"]}),104),104).unwrap();
    let evaluation = auto.evaluate(&reference, 105).unwrap();
    assert_eq!(auto.store.json(&evaluation).unwrap()["decision"], "unknown");
    auto.tick(&reference, 500).unwrap();
    let blocked = auto.inspect(&reference).unwrap();
    assert_eq!(blocked["state"]["status"], "blocked");
    assert_eq!(blocked["state"]["occurrences"], 1);
    assert!(blocked["state"]["active"].is_object());
}
#[test]
fn pause_fences_independent_checker_admission_and_exhaustion_does_not_claim_success() {
    let (t, auto, plan) = setup_goal(
        json!({"kind":"continuation","minimum_interval_seconds":20}),
        Some(2),
    );
    let reference = admitted(&auto, &plan);
    auto.tick(&reference, 100).unwrap();
    checked_occurrence(&t, &auto, &reference, 100);
    let (request, files) = {
        let _guard = auto.store.lock().unwrap();
        let mut state = auto.state().unwrap();
        let id = plan["automation"].as_str().unwrap().to_string();
        let mut record = state.plans[&id].clone();
        let event = auto
            .prepare_evaluation(&mut state, &mut record, 105)
            .unwrap();
        let files = auto.evaluation_files(&state, &record).unwrap();
        state.plans.insert(id, record);
        auto.save(&state).unwrap();
        (event, files)
    };
    let pause = control(&auto, &reference, "pause", 106);
    auto.control(&event(1, 5, pause, 106), 106).unwrap();
    assert!(
        auto.evaluator
            .as_ref()
            .unwrap()
            .run(&auto.coordinator, &request, &files, 106, &auto)
            .is_err()
    );
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        3
    );
    let (t, auto, mut plan) = setup_goal(
        json!({"kind":"continuation","minimum_interval_seconds":20}),
        Some(2),
    );
    plan["max_occurrences"] = json!(1);
    let reference = admitted(&auto, &plan);
    auto.tick(&reference, 100).unwrap();
    checked_occurrence(&t, &auto, &reference, 100);
    let evaluation = auto.evaluate(&reference, 105).unwrap();
    assert_eq!(
        auto.store.json(&evaluation).unwrap()["decision"],
        "continue"
    );
    auto.tick(&reference, 200).unwrap();
    assert_eq!(
        auto.inspect(&reference).unwrap()["state"]["status"],
        "exhausted"
    );
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        4
    );
}

#[test]
fn lost_evaluation_reply_recovers_exact_signed_result_after_restart_without_another_call() {
    let (t, auto, plan) = setup_goal(
        json!({"kind":"continuation","minimum_interval_seconds":20}),
        Some(2),
    );
    let reference = admitted(&auto, &plan);
    auto.tick(&reference, 100).unwrap();
    checked_occurrence(&t, &auto, &reference, 100);
    let (request, files) = {
        let _guard = auto.store.lock().unwrap();
        let mut state = auto.state().unwrap();
        let id = plan["automation"].as_str().unwrap().to_string();
        let mut record = state.plans[&id].clone();
        let request = auto
            .prepare_evaluation(&mut state, &mut record, 105)
            .unwrap();
        let files = auto.evaluation_files(&state, &record).unwrap();
        state.plans.insert(id, record);
        auto.save(&state).unwrap();
        (request, files)
    };
    let evaluator = auto.evaluator.as_ref().unwrap();
    let (response, returned) = evaluator
        .run(&auto.coordinator, &request, &files, 105, &auto)
        .unwrap();
    let root = auto.store.root().to_path_buf();
    let evaluator_root = t.path().join("evaluator3");
    drop(auto);
    let auto = Automation::open(
        root,
        secret(5),
        coordination_host::book::Coordinator::open(t.path().join("coor"), secret(1)).unwrap(),
    )
    .unwrap()
    .mount_evaluator(crate::evaluator::Evaluator::open(evaluator_root, secret(3)).unwrap())
    .unwrap();
    let (retry, retried) = auto
        .evaluator
        .as_ref()
        .unwrap()
        .run(&auto.coordinator, &request, &files, 106, &auto)
        .unwrap();
    assert_eq!(retry, response);
    assert_eq!(retried, returned);
    let result = auto.evaluate(&reference, 106).unwrap();
    assert_eq!(auto.evaluate(&reference, 107).unwrap(), result);
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        4
    );
    let state = auto.state().unwrap();
    let record = &state.plans[plan["automation"].as_str().unwrap()];
    let mut contradictory = auto.store.json(&result).unwrap();
    contradictory["decision"] = json!("complete");
    assert!(
        !auto
            .check_evaluation_policy(
                record,
                &contradictory,
                &auto.coordinator.scope(&plan["coordinator"]).unwrap()
            )
            .unwrap()
    );
}
#[test]
fn cleanup_fences_unstarted_identities_releases_only_known_work_and_keeps_terminal_status() {
    for prepared in [false, true] {
        let (t, auto, plan) = setup(json!({"kind":"once","at":110}));
        let reference = admitted(&auto, &plan);
        auto.tick(&reference, 110).unwrap();
        let request = if prepared {
            auto.advance(&reference, 110).unwrap()
        } else {
            None
        };
        let before = auto.inspect(&reference).unwrap();
        let occurrence = before["occurrences"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        let proposal = auto
            .occurrence_proposal(
                &auto.state().unwrap().plans[plan["automation"].as_str().unwrap()],
                occurrence,
            )
            .unwrap();
        let cancel = control(&auto, &reference, "cancel", 111);
        let cancel_event = event(1, 5, cancel, 111);
        let accepted = auto.control(&cancel_event, 111).unwrap();
        assert_eq!(auto.control(&cancel_event, 112).unwrap(), accepted);
        assert!(
            auto.coordinator
                .propose(
                    &event(5, 1, proposal, 112),
                    &auto.input_files(&occurrence["input"]).unwrap(),
                    112
                )
                .unwrap_err()
                .contains("fenced")
        );
        if let Some(request) = request {
            let worker =
                coordination_host::worker::Worker::open(t.path().join("queued"), secret(2), 112)
                    .unwrap();
            assert!(
                worker
                    .run_gated(&auto.coordinator, &request, 112, &auto)
                    .is_err()
            );
        }
        let after = auto.reconcile(&reference, None, 2100).unwrap();
        assert_eq!(after["status"], "cancelled");
        assert!(after["active"].is_null());
        let inspected = auto.inspect(&reference).unwrap();
        assert_eq!(inspected["accounting"]["spent"]["calls"], 3);
        let outcome_ref = &inspected["outcomes"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap()["reference"];
        let outcome = auto.store.json(outcome_ref).unwrap();
        assert!(outcome["result"].is_null());
        let refusal = auto.store.json(&outcome["settled"]).unwrap();
        assert_eq!(refusal["record"]["controller"], key(1));
        assert_eq!(refusal["record"]["type"], "resolved");
        assert_eq!(refusal["record"]["data"]["dispatched"], false);
        assert_eq!(
            auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
            if prepared { 1 } else { 0 }
        );
    }
}
#[test]
fn lost_worker_result_remains_held_until_original_receiver_reply_is_recovered_after_expiry() {
    let (t, auto, plan) = setup(json!({"kind":"once","at":110}));
    let reference = admitted(&auto, &plan);
    auto.tick(&reference, 110).unwrap();
    let request = auto.advance(&reference, 110).unwrap().unwrap();
    let worker =
        coordination_host::worker::Worker::open(t.path().join("lost-worker"), secret(2), 110)
            .unwrap();
    let (original, files) = worker
        .run_gated(&auto.coordinator, &request, 110, &auto)
        .unwrap();
    let first = auto.reconcile(&reference, None, 2100).unwrap();
    assert_eq!(first["status"], "expired");
    assert!(first["active"].is_object());
    assert_eq!(
        auto.inspect(&reference).unwrap()["accounting"]["spent"],
        json!({})
    );
    let empty = coordination_host::worker::Worker::open(
        t.path().join("different-empty-worker"),
        secret(2),
        2100,
    )
    .unwrap();
    assert!(auto.reconcile(&reference, Some(&empty), 2101).unwrap()["active"].is_object());
    let recovered = auto.reconcile(&reference, Some(&worker), 2102).unwrap();
    assert_eq!(recovered["status"], "expired");
    assert!(recovered["active"].is_null());
    let result_ref = auto
        .record_result(&reference, &original, &files, 2103)
        .unwrap();
    assert_eq!(
        auto.store.json(&result_ref).unwrap()["outcome"],
        "completed"
    );
    let repeat = auto.reconcile(&reference, Some(&worker), 2104).unwrap();
    assert_eq!(repeat["status"], "expired");
    assert_eq!(
        auto.inspect(&reference).unwrap()["accounting"]["spent"]["calls"],
        3
    );
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        2
    );
}
#[test]
fn blocked_source_occurrence_requires_cleanup_and_explicit_resume_without_reusing_its_slot() {
    let (t, auto, plan, reference, repo) = watched();
    auto.tick(&reference, 110).unwrap();
    let at = occurrence_time(&auto, &reference);
    let request = auto.advance(&reference, at).unwrap().unwrap();
    let old = git(&repo, &["rev-parse", "refs/heads/docs"], None);
    let tree = git(&repo, &["rev-parse", "refs/heads/docs^{tree}"], None);
    let next = git(
        &repo,
        &["commit-tree", &tree, "-p", &old],
        Some(b"changed before intake\n"),
    );
    git(&repo, &["update-ref", "refs/heads/docs", &next], None);
    let worker =
        coordination_host::worker::Worker::open(t.path().join("blocked-worker"), secret(2), at + 1)
            .unwrap();
    assert!(
        worker
            .run_gated(&auto.coordinator, &request, at + 1, &auto)
            .is_err()
    );
    assert_eq!(
        auto.inspect(&reference).unwrap()["state"]["status"],
        "blocked"
    );
    let resume = control(&auto, &reference, "resume", at + 2);
    assert!(auto.control(&event(1, 5, resume, at + 2), at + 2).is_err());
    auto.reconcile(&reference, None, at + 3).unwrap();
    let resume = control(&auto, &reference, "resume", at + 4);
    auto.control(&event(1, 5, resume, at + 4), at + 4).unwrap();
    auto.tick(&reference, 170).unwrap();
    let inspected = auto.inspect(&reference).unwrap();
    assert_eq!(inspected["state"]["occurrences"], 2);
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        4
    );
}
#[test]
fn cancelled_source_recovers_only_saved_reply_and_never_reads_to_fill_a_missing_reply() {
    for ran in [false, true] {
        let (_t, auto, plan, reference, _repo) = watched();
        let request = {
            let _guard = auto.store.lock().unwrap();
            let mut state = auto.state().unwrap();
            let id = plan["automation"].as_str().unwrap().to_string();
            let mut record = state.plans[&id].clone();
            let request = auto
                .prepare_poll(&mut state, &mut record, "observe", Some(0), None, 110)
                .unwrap();
            state.plans.insert(id, record);
            auto.save(&state).unwrap();
            request
        };
        if ran {
            auto.source
                .as_ref()
                .unwrap()
                .run(&auto.coordinator, &request, 110, &auto)
                .unwrap();
        }
        let cancel = control(&auto, &reference, "cancel", 150);
        auto.control(&event(1, 5, cancel, 150), 150).unwrap();
        if ran {
            auto.source
                .as_ref()
                .unwrap()
                .run(
                    &auto.coordinator,
                    &request,
                    1900,
                    &crate::recovery::SavedOnly,
                )
                .unwrap();
        }
        auto.reconcile(&reference, None, 1900).unwrap();
        let inspected = auto.inspect(&reference).unwrap();
        assert_eq!(inspected["state"]["status"], "cancelled");
        assert_eq!(inspected["state"]["occurrences"], 0);
        assert_eq!(inspected["pending"].is_null(), ran);
        assert_eq!(
            auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
            u64::from(ran)
        );
        if ran {
            assert_eq!(inspected["accounting"]["spent"]["calls"], 1);
        } else {
            assert_eq!(inspected["accounting"]["spent"], json!({}));
        }
    }
}
#[test]
fn cancelled_checker_recovers_original_decision_without_reopening_scheduling() {
    let (t, auto, plan) = setup_goal(
        json!({"kind":"continuation","minimum_interval_seconds":20}),
        Some(1),
    );
    let reference = admitted(&auto, &plan);
    auto.tick(&reference, 100).unwrap();
    checked_occurrence(&t, &auto, &reference, 100);
    let (request, files) = {
        let _guard = auto.store.lock().unwrap();
        let mut state = auto.state().unwrap();
        let id = plan["automation"].as_str().unwrap().to_string();
        let mut record = state.plans[&id].clone();
        let request = auto
            .prepare_evaluation(&mut state, &mut record, 105)
            .unwrap();
        let files = auto.evaluation_files(&state, &record).unwrap();
        state.plans.insert(id, record);
        auto.save(&state).unwrap();
        (request, files)
    };
    auto.evaluator
        .as_ref()
        .unwrap()
        .run(&auto.coordinator, &request, &files, 105, &auto)
        .unwrap();
    let cancel = control(&auto, &reference, "cancel", 106);
    auto.control(&event(1, 5, cancel, 106), 106).unwrap();
    auto.reconcile(&reference, None, 1900).unwrap();
    let inspected = auto.inspect(&reference).unwrap();
    assert_eq!(inspected["state"]["status"], "cancelled");
    assert!(inspected["state"]["active"].is_null());
    assert!(inspected["pending"].is_null());
    let evaluation = &inspected["evaluations"]
        .as_object()
        .unwrap()
        .values()
        .last()
        .unwrap()["body"];
    assert_eq!(evaluation["decision"], "complete");
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        4
    );
    auto.tick(&reference, 2200).unwrap();
    assert_eq!(
        auto.inspect(&reference).unwrap()["state"]["status"],
        "cancelled"
    );
}
#[test]
fn trusted_clock_expiry_fences_a_queued_continuation_before_input_disclosure() {
    let (t, auto, plan) = setup_goal(
        json!({"kind":"continuation","minimum_interval_seconds":20}),
        Some(2),
    );
    let reference = admitted(&auto, &plan);
    auto.tick(&reference, 100).unwrap();
    let request = auto.advance(&reference, 100).unwrap().unwrap();
    auto.tick(&reference, 900).unwrap();
    assert_eq!(
        auto.inspect(&reference).unwrap()["state"]["status"],
        "expired"
    );
    let claims = auto.coordinator.scope(&plan["coordinator"]).unwrap()["claims"].clone();
    assert!(
        claims
            .as_object()
            .unwrap()
            .values()
            .all(|l| l["phase"] == "released")
    );
    let worker =
        coordination_host::worker::Worker::open(t.path().join("expired-queued"), secret(2), 900)
            .unwrap();
    assert!(
        worker
            .run_gated(&auto.coordinator, &request, 900, &auto)
            .is_err()
    );
    auto.reconcile(&reference, None, 901).unwrap();
    assert!(auto.inspect(&reference).unwrap()["state"]["active"].is_null());
    assert_eq!(
        auto.coordinator.scope(&plan["coordinator"]).unwrap()["calls"],
        1
    );
}
