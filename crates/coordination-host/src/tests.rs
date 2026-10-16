use super::*;
use book::{Coordinator, Policy};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn key(n: u8) -> String {
    nostr::domain::RelaySigner::from_secret_hex(&secret(n).display_secret().to_string())
        .unwrap()
        .pubkey()
        .into()
}
fn event(n: u8, body: Value, now: u64) -> Event {
    workspace_host::host::request(&secret(n), &key(1), body, now).unwrap()
}
fn effects() -> Value {
    json!({"reads":["artifact"],"writes":[],"network":[],"process":false,"delegates":false,"spend":false})
}
fn revision(h: &Coordinator, s: &Value) -> u64 {
    h.scope(s).unwrap()["revision"].as_u64().unwrap()
}
fn setup(calls: u64) -> (tempfile::TempDir, Coordinator, Value, Value, Value) {
    let t = tempfile::tempdir().unwrap();
    let h = Coordinator::initialize(t.path().join("coord"), secret(1)).unwrap();
    let target = h.data_operation().unwrap();
    let mut ceiling = effects();
    ceiling["delegates"] = json!(true);
    let container = h.container_operation(ceiling.clone()).unwrap();
    let scope = h
        .create_scope(
            Policy {
                recipients: BTreeSet::from([key(1), key(2), key(3)]),
                checkers: BTreeSet::from([key(3)]),
                calls,
                concurrency: 8,
                max_claim_seconds: 100,
                max_container_seconds: None,
                max_depth: 8,
                until: 5000,
                effects: ceiling,
                parent: None,
            },
            vec![],
            vec![target.clone(), container],
            100,
        )
        .unwrap();
    for n in [2, 3] {
        let control = json!({"v":"openagents.coordination-native-control.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&scope),"actor":key(n),"roles":["read","propose","claim","control","finding"],"resources":[],"targets":[],"all":true,"until":5000,"revoke":false,"scope_revoke":false});
        h.control_grants(&event(1, control, 100), 100).unwrap();
    }
    let acceptance=h.put_local_json(&json!({"v":"openagents.coordination-native-check-policy.v1","requires":[],"check":"exact-admitted-output"})).unwrap();
    let text = h.put_local_bytes(b"Objective", "text/plain", None).unwrap();
    let snapshot=h.put_local_json(&json!({"v":"openagents.snapshot.v1","requires":[],"scope":"coord-tests","captured_at":100,"sources":[],"coverage":"complete"})).unwrap();
    let task = workspace_host::artifact::token();
    let precedence = h
        .put_local_json(
            &json!({"v":"openagents.coordination-empty-instruction-policy.v1","requires":[]}),
        )
        .unwrap();
    let instructions=h.put_local_json(&json!({"v":"openagents.instructions.v1","requires":[],"task":task,"revision":0,"resolver":{"id":format!("{}:coord/empty-instructions",key(1)),"artifact":precedence},"precedence":precedence,"snapshot":snapshot,"entries":[]})).unwrap();
    let frame=h.put_local_json(&json!({"v":"openagents.task-frame.v1","requires":[],"task":task,"owner":key(1),"controller":key(1),"revision":0,"previous":null,"objective":text,"origin":"user","constraints":[],"acceptance":[acceptance],"snapshot":snapshot,"instructions":instructions,"variables":[],"attempts":[],"unresolved":[],"corrections":[]})).unwrap();
    let ctl = json!({"v":"openagents.coordination-native-frame.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&scope),"frame":frame});
    let bytes = h.store.get(&frame).unwrap();
    h.frame(&event(1, ctl, 100), &bytes, 100).unwrap();
    (t, h, scope, target, frame)
}
fn proposal(
    h: &Coordinator,
    s: &Value,
    target: &Value,
    frame: &Value,
    calls: u64,
    parent: Option<String>,
) -> Value {
    let f = h.store.json(frame).unwrap();
    let input=h.put_local_json(&json!({"v":"openagents.fixture-input.v1","requires":[],"trial":workspace_host::artifact::token(),"text":"data"})).unwrap();
    let lock=h.put_local_json(&json!({"v":"openagents.lock.v1","requires":[],"root":target,"entries":[{"id":target["id"],"definition":target,"dependencies":[]}]})).unwrap();
    let context=h.put_local_json(&json!({"v":"openagents.context.v1","requires":[],"task":f["task"],"recipient":key(2),"policy":h.store.json(s).unwrap()["policy"],"entries":[],"omissions":[],"coverage":"complete"})).unwrap();
    json!({"v":"openagents.task-proposal.v1","requires":[],"task":workspace_host::artifact::token(),"coordinator":s,"frame":frame,"parent":parent,"target":target,"lock":lock,"input":input,"context":context,"dependencies":[],"effects":effects(),"bounds":[{"bound":"calls","ceiling":calls,"assurance":"host","mechanism":"native-ledger"}],"acceptance":f["acceptance"][0],"reuse":"never"})
}
fn propose(h: &Coordinator, p: &Value) -> Value {
    h.propose(&event(1, p.clone(), 101), &BTreeMap::new(), 101)
        .unwrap()["value"]
        .clone()
}
fn claim(h: &Coordinator, s: &Value, p: &Value, parent: Value) -> Value {
    let body = json!({"v":"openagents.claim-request.v1","requires":[],"proposal":p,"worker":key(2),"resources":[],"expected_revision":revision(h,s),"requested_until":190,"reservation":parent});
    h.claim(&event(2, body, 101), 101).unwrap()["value"].clone()
}
fn control(
    h: &Coordinator,
    s: &Value,
    p: &Value,
    c: &Value,
    action: &str,
    now: u64,
) -> Result<Value> {
    h.control(&event(2,json!({"v":"openagents.coordination-control.v1","requires":[],"coordinator":s,"proposal":p,"claim":c,"action":action,"expected_revision":revision(h,s),"requested_until":null,"evidence":[]}),now),now)
}
#[test]
fn signed_cj_worker_exact_retry_restart_and_consumed_allowance() {
    let (t, h, s, target, frame) = setup(1);
    let p = proposal(&h, &s, &target, &frame, 1, None);
    let pr = propose(&h, &p);
    let c = claim(&h, &s, &pr, h.scope(&s).unwrap()["allowance"].clone());
    let dispatch = event(
        2,
        json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),
        102,
    );
    let request = h.prepare(&dispatch, 102).unwrap();
    let worker = worker::Worker::open(t.path().join("worker"), secret(2), 102).unwrap();
    let (result, files) = worker.run(&h, &request, 102).unwrap();
    h.record_result(&result, &files, 103).unwrap();
    assert_eq!(worker.run(&h, &request, 300).unwrap().0, result);
    assert_eq!(h.scope(&s).unwrap()["calls"], 1);
    control(&h, &s, &pr, &c, "release", 103).unwrap();
    drop(h);
    let h = Coordinator::open(t.path().join("coord"), secret(1)).unwrap();
    let p2 = proposal(&h, &s, &target, &frame, 1, None);
    let pr2 = propose(&h, &p2);
    let body = json!({"v":"openagents.claim-request.v1","requires":[],"proposal":pr2,"worker":key(2),"resources":[],"expected_revision":revision(&h,&s),"requested_until":190,"reservation":h.scope(&s).unwrap()["allowance"]});
    assert!(
        h.claim(&event(2, body, 104), 104)
            .unwrap_err()
            .contains("allowance")
    );
    assert_eq!(
        h.record_result(&result, &files, 104).unwrap()["outcome"],
        "completed"
    );
}
#[test]
fn siblings_share_parent_holds_and_cancellation_does_not_free_unknown() {
    let (_t, h, s, target, frame) = setup(2);
    let mut parent_effects = effects();
    parent_effects["delegates"] = json!(true);
    let container = h.container_operation(parent_effects.clone()).unwrap();
    let mut parent = proposal(&h, &s, &container, &frame, 2, None);
    parent["effects"] = parent_effects;
    let pr = propose(&h, &parent);
    let c = claim(&h, &s, &pr, h.scope(&s).unwrap()["allowance"].clone());
    let reservation = h.store.json(&c).unwrap()["reservation"].clone();
    let child = proposal(
        &h,
        &s,
        &target,
        &frame,
        2,
        Some(parent["task"].as_str().unwrap().into()),
    );
    let cr = propose(&h, &child);
    let cc = claim(&h, &s, &cr, reservation.clone());
    let sibling = proposal(
        &h,
        &s,
        &target,
        &frame,
        1,
        Some(parent["task"].as_str().unwrap().into()),
    );
    let sr = propose(&h, &sibling);
    let request = json!({"v":"openagents.claim-request.v1","requires":[],"proposal":sr,"worker":key(2),"resources":[],"expected_revision":revision(&h,&s),"requested_until":190,"reservation":reservation});
    assert!(
        h.claim(&event(2, request, 102), 102)
            .unwrap_err()
            .contains("parent call allowance")
    );
    assert!(
        control(&h, &s, &pr, &c, "release", 102)
            .unwrap_err()
            .contains("descendant")
    );
    control(&h, &s, &cr, &cc, "cancel", 103).unwrap();
    control(&h, &s, &cr, &cc, "release", 103).unwrap();
    control(&h, &s, &pr, &c, "release", 103).unwrap();
    assert_eq!(h.scope(&s).unwrap()["charged_calls"], 0);
}
#[test]
fn cancelled_prepared_request_cannot_disclose_input() {
    let (t, h, s, target, frame) = setup(2);
    let p = proposal(&h, &s, &target, &frame, 1, None);
    let pr = propose(&h, &p);
    let c = claim(&h, &s, &pr, h.scope(&s).unwrap()["allowance"].clone());
    let request=h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),102),102).unwrap();
    control(&h, &s, &pr, &c, "cancel", 103).unwrap();
    let worker = worker::Worker::open(t.path().join("worker"), secret(2), 103).unwrap();
    assert!(worker.run(&h, &request, 103).is_err());
    assert_eq!(h.scope(&s).unwrap()["calls"], 0);
}
#[test]
fn controller_cj_scope_pins_and_restart_exact_response() {
    let (t, h, s, target, frame) = setup(2);
    let p = proposal(&h, &s, &target, &frame, 1, None);
    let transport = h.scope(&s).unwrap()["transport"].clone();
    let capability =
        nostr::contracts::parse_strict(&h.store.get(&transport["target"]["artifact"]).unwrap())
            .unwrap();
    nostr::cap::parse_definition(&capability).unwrap();
    let validate = |field: &str, body: &Value| {
        let schema = &capability[field];
        let bytes = h.store.get(schema).unwrap();
        let digest = schema["digest"].as_str().unwrap();
        let closure =
            nostr::contracts::prepare_closure(&BTreeMap::from([(digest.to_string(), bytes)]))
                .unwrap();
        nostr::contracts::validate_instance(&closure, digest, body).unwrap();
    };
    validate("input", &p);
    let request = transport::invoke(
        &secret(1),
        &key(1),
        &transport,
        p,
        workspace_host::artifact::token(),
        101,
    )
    .unwrap();
    let (reply, files) = h.serve(&request, &BTreeMap::new(), 101).unwrap();
    let opened = nostr::execution::open_request(
        &request,
        &key(1),
        &secret(1),
        101,
        nostr::execution::Window::DEFAULT,
    )
    .unwrap();
    let nostr::execution::Body::Execute(execute) = opened.body else {
        panic!()
    };
    let output = nostr::execution::bind_worker_event(
        &reply,
        &nostr::execution::Pending {
            worker: &key(1),
            customer: &key(1),
            request: &execute.request,
            attempt: 1,
            execute_event: &request.id,
        },
        &secret(1),
    )
    .unwrap();
    assert_eq!(output["outcome"], "completed");
    let result: Value =
        serde_json::from_slice(&files[output["output"]["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(result["status"], "accepted");
    validate("output", &result);
    drop(h);
    let h = Coordinator::open(t.path().join("coord"), secret(1)).unwrap();
    assert_eq!(h.serve(&request, &BTreeMap::new(), 300).unwrap().0, reply);
}
#[test]
fn unknown_worker_disclosure_survives_expiry_restart_and_cancel() {
    let (t, h, s, target, frame) = setup(1);
    let p = proposal(&h, &s, &target, &frame, 1, None);
    let pr = propose(&h, &p);
    let c = claim(&h, &s, &pr, h.scope(&s).unwrap()["allowance"].clone());
    let request=h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),102),102).unwrap();
    let opened = nostr::execution::open_request(
        &request,
        &key(2),
        &secret(2),
        102,
        nostr::execution::Window::DEFAULT,
    )
    .unwrap();
    let nostr::execution::Body::Execute(execute) = opened.body else {
        panic!()
    };
    h.intake(&event(2,json!({"v":"openagents.coordination-native-intake.v1","requires":[],"request":request.id,"fingerprint":execute.fingerprint}),102),102).unwrap();
    drop(h);
    let h = Coordinator::open(t.path().join("coord"), secret(1)).unwrap();
    assert_eq!(
        control(&h, &s, &pr, &c, "cancel", 300).unwrap()["status"],
        "unknown"
    );
    assert!(
        control(&h, &s, &pr, &c, "release", 300)
            .unwrap_err()
            .contains("unknown")
    );
    assert_eq!(h.scope(&s).unwrap()["charged_calls"], 1);
}
#[test]
fn checker_is_separate_and_charged_once() {
    let (t, h, s, target, frame) = setup(2);
    let p = proposal(&h, &s, &target, &frame, 2, None);
    let pr = propose(&h, &p);
    let c = claim(&h, &s, &pr, h.scope(&s).unwrap()["allowance"].clone());
    let request=h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),102),102).unwrap();
    let worker = worker::Worker::open(t.path().join("worker"), secret(2), 102).unwrap();
    let (result, files) = worker.run(&h, &request, 102).unwrap();
    h.record_result(&result, &files, 103).unwrap();
    assert!(
        checks::Checker::new(secret(2))
            .check(&h, &s, &c, 103)
            .is_err()
    );
    let checker = checks::Checker::new(secret(3));
    let report = checker.check(&h, &s, &c, 103).unwrap();
    let receipt = h.record_check(&report, 103).unwrap();
    assert_eq!(h.store.json(&receipt).unwrap()["passed"], true);
    assert_eq!(h.scope(&s).unwrap()["calls"], 2);
    checker.check(&h, &s, &c, 104).unwrap();
    assert_eq!(h.scope(&s).unwrap()["calls"], 2);
}
fn git(root: &std::path::Path, args: &[&str], input: Option<&[u8]>) -> String {
    use std::io::Write;
    let mut child = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child.stdin.take().unwrap().write_all(input).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn source_setup() -> (
    tempfile::TempDir,
    Coordinator,
    Value,
    Value,
    Value,
    Value,
    book::Resource,
    book::Resource,
    std::path::PathBuf,
) {
    let (t, h, _s, _target, frame) = setup(8);
    let repo = t.path().join("repo.git");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "--bare"], None);
    git(&repo, &["config", "core.logAllRefUpdates", "true"], None);
    let blob = git(&repo, &["hash-object", "-w", "--stdin"], Some(b"hello\n"));
    let tree = git(
        &repo,
        &["mktree"],
        Some(format!("100644 blob {blob}\ta.txt\n100644 blob {blob}\tb.txt\n").as_bytes()),
    );
    let commit = git(&repo, &["commit-tree", &tree], Some(b"fixture\n"));
    git(&repo, &["update-ref", "refs/heads/docs", &commit], None);
    let ws = workspace_host::host::Host::create(t.path().join("source"), secret(1), 100).unwrap();
    let a = ws
        .register_git(
            workspace_host::git::GitDocument::admit(&repo, "refs/heads/docs", "a.txt").unwrap(),
            "A".into(),
            100,
        )
        .unwrap();
    let b = ws
        .register_git(
            workspace_host::git::GitDocument::admit(&repo, "refs/heads/docs", "b.txt").unwrap(),
            "B".into(),
            100,
        )
        .unwrap();
    let h = h.mount_source("docs".into(), ws).unwrap();
    let a = h
        .resource("a".into(), "docs".into(), json!(a), 100)
        .unwrap();
    let b = h
        .resource("b".into(), "docs".into(), json!(b), 100)
        .unwrap();
    assert_eq!(a.domain, b.domain);
    let target = h.git_operation("a").unwrap();
    let other = h.git_operation("b").unwrap();
    let mut effects = effects();
    effects["reads"] = json!(["artifact", "a", "b"]);
    effects["writes"] = json!(["a", "b"]);
    let s = h
        .create_scope(
            Policy {
                recipients: BTreeSet::from([key(1), key(2), key(3)]),
                checkers: BTreeSet::from([key(3)]),
                calls: 8,
                concurrency: 8,
                max_claim_seconds: 100,
                max_container_seconds: None,
                max_depth: 8,
                until: 5000,
                effects,
                parent: None,
            },
            vec![a.clone(), b.clone()],
            vec![target.clone(), other.clone()],
            100,
        )
        .unwrap();
    for n in [2, 3] {
        h.control_grants(&event(1,json!({"v":"openagents.coordination-native-control.v1","requires":[],"coordinator":s,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&s),"actor":key(n),"roles":["read","propose","claim","control","finding"],"resources":[],"targets":[],"all":true,"until":5000,"revoke":false,"scope_revoke":false}),100),100).unwrap();
    }
    h.frame(&event(1,json!({"v":"openagents.coordination-native-frame.v1","requires":[],"coordinator":s,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&s),"frame":frame}),100),&h.store.get(&frame).unwrap(),100).unwrap();
    (t, h, s, target, other, frame, a, b, repo)
}
fn patch(
    h: &Coordinator,
    s: &Value,
    target: &Value,
    frame: &Value,
    r: &book::Resource,
) -> (Value, Value) {
    let read = h.capture_resource(r, 101).unwrap();
    let content = h.put_local_bytes(b"updated\n", "text/plain", None).unwrap();
    let input=h.put_local_json(&json!({"v":"openagents.coordination-native-patch.v1","requires":[],"resource":r.id,"snapshot":read["snapshot"],"content":content})).unwrap();
    let mut p = proposal(h, s, target, frame, 3, None);
    p["input"] = input;
    p["effects"] = h.operation(target).unwrap()["effects"].clone();
    (p, read)
}
fn claim_patch(
    h: &Coordinator,
    s: &Value,
    p: &Value,
    r: &book::Resource,
    read: &Value,
) -> Result<Value> {
    h.claim(&event(2,json!({"v":"openagents.claim-request.v1","requires":[],"proposal":p,"worker":key(2),"resources":[{"id":r.id,"mode":"write","snapshot":read["snapshot"]}],"expected_revision":revision(h,s),"requested_until":190,"reservation":h.scope(s).unwrap()["allowance"]}),101),101)
}
#[test]
fn source_alias_racing_claims_and_checked_conditional_integration() {
    let (t, h, s, target, other, frame, a, b, repo) = source_setup();
    let (p, read) = patch(&h, &s, &target, &frame, &a);
    let pr = propose(&h, &p);
    let c = claim_patch(&h, &s, &pr, &a, &read).unwrap()["value"].clone();
    let (p2, read2) = patch(&h, &s, &other, &frame, &b);
    let pr2 = propose(&h, &p2);
    assert!(
        claim_patch(&h, &s, &pr2, &b, &read2)
            .unwrap_err()
            .contains("unreleased claim")
    );
    let request=h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),102),102).unwrap();
    let worker = worker::Worker::open(t.path().join("worker"), secret(2), 102).unwrap();
    let (result, files) = worker.run(&h, &request, 102).unwrap();
    h.record_result(&result, &files, 103).unwrap();
    assert_eq!(
        git(&repo, &["show", "refs/heads/docs:a.txt"], None),
        "hello"
    );
    let integration = event(
        1,
        json!({"v":"openagents.coordination-native-integration.v1","requires":[],"coordinator":s,"claim":c,"recover":false}),
        104,
    );
    assert!(
        h.integrate(&integration, 104, None)
            .unwrap_err()
            .contains("independent check")
    );
    let report = checks::Checker::new(secret(3))
        .check(&h, &s, &c, 104)
        .unwrap();
    h.record_check(&report, 104).unwrap();
    let result = h.integrate(&integration, 104, None).unwrap();
    assert_eq!(result["outcome"], "completed");
    assert_eq!(
        git(&repo, &["show", "refs/heads/docs:a.txt"], None),
        "updated"
    );
    assert_eq!(
        git(&repo, &["show", "refs/heads/docs:b.txt"], None),
        "hello"
    );
    assert_eq!(h.scope(&s).unwrap()["calls"], 3);
    assert_eq!(h.integrate(&integration, 105, None).unwrap(), result);
    control(&h, &s, &pr, &c, "release", 105).unwrap();
    assert!(
        claim_patch(&h, &s, &pr2, &b, &read2)
            .unwrap_err()
            .contains("stale")
    );
}
#[test]
fn independent_source_edit_refuses_stale_dispatch() {
    let (_t, h, s, target, _other, frame, a, _b, repo) = source_setup();
    let (p, read) = patch(&h, &s, &target, &frame, &a);
    let pr = propose(&h, &p);
    let c = claim_patch(&h, &s, &pr, &a, &read).unwrap()["value"].clone();
    let old = git(&repo, &["rev-parse", "refs/heads/docs"], None);
    let tree = git(&repo, &["rev-parse", "refs/heads/docs^{tree}"], None);
    let next = git(
        &repo,
        &["commit-tree", &tree, "-p", &old],
        Some(b"independent\n"),
    );
    git(&repo, &["update-ref", "refs/heads/docs", &next, &old], None);
    assert!(h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),102),102).unwrap_err().contains("stale"));
    assert_eq!(h.scope(&s).unwrap()["calls"], 0);
}
#[test]
fn truly_concurrent_source_claims_have_one_winner() {
    let (_t, h, s, target, other, frame, a, b, _repo) = source_setup();
    let (pa, ra) = patch(&h, &s, &target, &frame, &a);
    let pra = propose(&h, &pa);
    let (pb, rb) = patch(&h, &s, &other, &frame, &b);
    let prb = propose(&h, &pb);
    let rev = revision(&h, &s);
    let allowance = h.scope(&s).unwrap()["allowance"].clone();
    let events=[(&a,&pra,&ra),(&b,&prb,&rb)].map(|(r,p,read)|event(2,json!({"v":"openagents.claim-request.v1","requires":[],"proposal":p,"worker":key(2),"resources":[{"id":r.id,"mode":"write","snapshot":read["snapshot"]}],"expected_revision":rev,"requested_until":190,"reservation":allowance}),101));
    let barrier = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let handles = events
            .iter()
            .map(|e| {
                scope.spawn(|| {
                    barrier.wait();
                    h.claim(e, 101)
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    for (event, result) in events.iter().zip(&results) {
        if result.is_err() {
            assert!(h.claim(event, 101).unwrap_err().contains("stale"));
        }
    }
}
#[test]
fn reuse_needs_independent_check_and_trial_inputs_remain_distinct() {
    let (t, h, s, target, frame) = setup(4);
    let mut p = proposal(&h, &s, &target, &frame, 2, None);
    p["reuse"] = json!("exact");
    let pr = propose(&h, &p);
    let c = claim(&h, &s, &pr, h.scope(&s).unwrap()["allowance"].clone());
    let request=h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),102),102).unwrap();
    let worker = worker::Worker::open(t.path().join("worker"), secret(2), 102).unwrap();
    let (result, files) = worker.run(&h, &request, 102).unwrap();
    h.record_result(&result, &files, 103).unwrap();
    let mut before = p.clone();
    before["task"] = json!(workspace_host::artifact::token());
    assert_eq!(
        h.propose(&event(1, before, 103), &BTreeMap::new(), 103)
            .unwrap()["status"],
        "accepted"
    );
    let report = checks::Checker::new(secret(3))
        .check(&h, &s, &c, 104)
        .unwrap();
    h.record_check(&report, 104).unwrap();
    control(&h, &s, &pr, &c, "release", 104).unwrap();
    let mut duplicate = p.clone();
    duplicate["task"] = json!(workspace_host::artifact::token());
    let request = event(1, duplicate.clone(), 105);
    let reused = h.propose(&request, &BTreeMap::new(), 105).unwrap();
    assert_eq!(reused["status"], "duplicate");
    assert_eq!(h.propose(&request, &BTreeMap::new(), 105).unwrap(), reused);
    duplicate["input"]=h.put_local_json(&json!({"v":"openagents.fixture-input.v1","requires":[],"trial":workspace_host::artifact::token(),"text":"data"})).unwrap();
    duplicate["task"] = json!(workspace_host::artifact::token());
    assert_eq!(
        h.propose(&event(1, duplicate, 105), &BTreeMap::new(), 105)
            .unwrap()["status"],
        "accepted"
    );
}
#[test]
fn background_sources_coalesce_changes_under_one_reserved_parent() {
    let (t, h, _s, _target, _other, frame, a, b, repo) = source_setup();
    let data = h.data_operation().unwrap();
    let mut ceiling = effects();
    ceiling["reads"] = json!(["artifact", "a"]);
    ceiling["delegates"] = json!(true);
    let container = h.container_operation(ceiling.clone()).unwrap();
    let s = h
        .create_scope(
            Policy {
                recipients: BTreeSet::from([key(1), key(2), key(3)]),
                checkers: BTreeSet::from([key(3)]),
                calls: 12,
                concurrency: 8,
                max_claim_seconds: 100,
                max_container_seconds: None,
                max_depth: 8,
                until: 5000,
                effects: ceiling.clone(),
                parent: None,
            },
            vec![a, b],
            vec![data.clone(), container.clone()],
            100,
        )
        .unwrap();
    for n in [2, 3] {
        h.control_grants(&event(1,json!({"v":"openagents.coordination-native-control.v1","requires":[],"coordinator":s,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&s),"actor":key(n),"roles":["read","propose","claim","control","finding"],"resources":[],"targets":[],"all":true,"until":5000,"revoke":false,"scope_revoke":false}),100),100).unwrap();
    }
    h.frame(&event(1,json!({"v":"openagents.coordination-native-frame.v1","requires":[],"coordinator":s,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&s),"frame":frame}),100),&h.store.get(&frame).unwrap(),100).unwrap();
    let mut parent = proposal(&h, &s, &container, &frame, 10, None);
    parent["effects"] = ceiling;
    let pr = propose(&h, &parent);
    let parent_claim = claim(&h, &s, &pr, h.scope(&s).unwrap()["allowance"].clone());
    let mut template = proposal(
        &h,
        &s,
        &data,
        &frame,
        1,
        Some(parent["task"].as_str().unwrap().into()),
    );
    template["effects"]["reads"] = json!(["artifact", "a"]);
    let inputs=h.put_local_json(&json!({"v":"openagents.coordination-native-background-inputs.v1","requires":[],"template":template,"parent_claim":parent_claim,"sources":["a"],"watch":parent["task"],"max_occurrences":2})).unwrap();
    let disclosure=h.put_local_json(&json!({"v":"openagents.coordination-native-disclosure.v1","requires":[],"recipients":BTreeSet::from([key(1),key(2),key(3)])})).unwrap();
    let plan = json!({"v":"openagents.background-plan.v1","requires":[],"owner":key(1),"coordinator":s,"task_frame":frame,"operation":data,"lock":template["lock"],"trigger":"snapshot_changed","inputs":inputs,"bounds":[{"bound":"calls","ceiling":8,"assurance":"host","mechanism":"native-ledger"}],"priority":"background","lifetime":"task","expires_at":180,"disclosure":disclosure,"destination":"evidence","stale_policy":"retain_as_stale"});
    let admitted = h.activate_background(&event(1, plan, 102), 102).unwrap();
    let poll = event(
        1,
        json!({"v":"openagents.coordination-native-background-poll.v1","requires":[],"coordinator":s,"plan":admitted["reference"]}),
        103,
    );
    let first = h.poll_background(&poll, 103).unwrap();
    let c = first["value"].clone();
    assert_eq!(h.poll_background(&poll, 103).unwrap()["active"], c);
    let request=h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),103),103).unwrap();
    let worker = worker::Worker::open(t.path().join("background-worker"), secret(2), 103).unwrap();
    let (result, files) = worker.run(&h, &request, 103).unwrap();
    h.record_result(&result, &files, 104).unwrap();
    assert_eq!(h.poll_background(&poll, 104).unwrap()["coalesced"], true);
    let old = git(&repo, &["rev-parse", "refs/heads/docs"], None);
    let tree = git(&repo, &["rev-parse", "refs/heads/docs^{tree}"], None);
    let next = git(
        &repo,
        &["commit-tree", &tree, "-p", &old],
        Some(b"source changed\n"),
    );
    git(&repo, &["update-ref", "refs/heads/docs", &next, &old], None);
    let second = h.poll_background(&poll, 105).unwrap();
    assert_eq!(second["status"], "accepted");
    assert_ne!(second["value"], c);
    assert!(
        h.poll_background(&poll, 106)
            .unwrap_err()
            .contains("lifetime")
    );
    assert_eq!(h.scope(&s).unwrap()["calls"], 4);
}
#[test]
fn renewals_replace_tokens_and_revoked_workers_cannot_start() {
    let (t, h, s, target, frame) = setup(2);
    let p = proposal(&h, &s, &target, &frame, 1, None);
    let pr = propose(&h, &p);
    let c = claim(&h, &s, &pr, h.scope(&s).unwrap()["allowance"].clone());
    let renewal = event(
        2,
        json!({"v":"openagents.coordination-control.v1","requires":[],"coordinator":s,"proposal":pr,"claim":c,"action":"renew","expected_revision":revision(&h,&s),"requested_until":195,"evidence":[]}),
        102,
    );
    let renewed = h.control(&renewal, 102).unwrap();
    assert_eq!(h.control(&renewal, 102).unwrap(), renewed);
    let next = renewed["value"].clone();
    assert!(
        h.store.json(&next).unwrap()["token"].as_u64().unwrap()
            > h.store.json(&c).unwrap()["token"].as_u64().unwrap()
    );
    assert!(h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),103),103).is_err());
    let request=h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":next}),103),103).unwrap();
    h.control_grants(&event(1,json!({"v":"openagents.coordination-native-control.v1","requires":[],"coordinator":s,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&s),"actor":key(2),"roles":["read","claim"],"resources":[],"targets":[],"all":true,"until":5000,"revoke":true,"scope_revoke":false}),104),104).unwrap();
    let worker = worker::Worker::open(t.path().join("worker"), secret(2), 104).unwrap();
    assert!(worker.run(&h, &request, 104).is_err());
    assert_eq!(h.scope(&s).unwrap()["calls"], 0);
}
#[test]
fn changed_frame_keeps_original_finding_as_stale_history() {
    let (t, h, s, target, frame) = setup(2);
    let p = proposal(&h, &s, &target, &frame, 1, None);
    let pr = propose(&h, &p);
    let c = claim(&h, &s, &pr, h.scope(&s).unwrap()["allowance"].clone());
    let request=h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),102),102).unwrap();
    let worker = worker::Worker::open(t.path().join("worker"), secret(2), 102).unwrap();
    let (result, files) = worker.run(&h, &request, 102).unwrap();
    let result = h.record_result(&result, &files, 103).unwrap();
    let old = h.store.json(&frame).unwrap();
    let mut changed = old.clone();
    changed["revision"] = json!(1);
    changed["previous"] = frame.clone();
    let next = h.put_local_json(&changed).unwrap();
    h.frame(&event(1,json!({"v":"openagents.coordination-native-frame.v1","requires":[],"coordinator":s,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&s),"frame":next}),104),&h.store.get(&next).unwrap(),104).unwrap();
    let finding = json!({"v":"openagents.finding.v1","requires":[],"proposal":pr,"frame":frame,"snapshot":old["snapshot"],"context":p["context"],"producer":key(2),"kind":"fact","content":result["output"],"evidence":[],"receipts":[workspace_host::artifact::json_ref(&result).unwrap()],"valid_until":180,"verification":"not_run","integration":"not_requested"});
    let signed = event(2, finding.clone(), 105);
    let accepted = h.finding(&signed, &BTreeMap::new(), 105).unwrap();
    assert_eq!(h.store.json(&accepted["value"]).unwrap(), finding);
    assert_eq!(
        h.scope(&s).unwrap()["proposals"][p["task"].as_str().unwrap()]["findings"][0]["stale"],
        true
    );
}
#[test]
fn interrupted_controller_recovers_known_result_or_fenced_absence_without_replay() {
    for accepted in [false, true] {
        let (t, h, s, target, frame) = setup(2);
        let p = proposal(&h, &s, &target, &frame, 1, None);
        let request = transport::invoke(
            &secret(1),
            &key(1),
            &h.scope(&s).unwrap()["transport"],
            p.clone(),
            workspace_host::artifact::token(),
            101,
        )
        .unwrap();
        let opened = nostr::execution::open_request(
            &request,
            &key(1),
            &secret(1),
            101,
            nostr::execution::Window::DEFAULT,
        )
        .unwrap();
        {
            let _guard = h.store.lock().unwrap();
            let mut state = h.state().unwrap();
            let (claim, root) = match state
                .service
                .prepare(&opened, 101, &workspace_host::artifact::token())
                .unwrap()
            {
                nostr::execution::Admission::Reserved { claim, root } => (claim, root),
                _ => panic!(),
            };
            let r = h
                .store
                .put(&root, "application/json", Some(nostr::run::RECORD_SCHEMA))
                .unwrap();
            state.reply_artifacts.insert(claim.key.clone(), vec![r]);
            state.service.acknowledge(&claim.key, &root).unwrap();
            state.service.intend(&claim.key).unwrap();
            h.save(&state).unwrap();
        }
        if accepted {
            h.propose(&request, &BTreeMap::new(), 101).unwrap();
        }
        drop(h);
        let h = Coordinator::open(t.path().join("coord"), secret(1)).unwrap();
        assert!(
            h.serve(&request, &BTreeMap::new(), 102)
                .unwrap_err()
                .contains("unknown")
        );
        let (reply, files) = h.recover_controller(&request, 102).unwrap();
        assert_eq!(h.serve(&request, &BTreeMap::new(), 103).unwrap().0, reply);
        assert!(
            files
                .values()
                .filter_map(|b| serde_json::from_slice::<Value>(b).ok())
                .any(|v| v["v"] == "openagents.coordination-result.v1"
                    && v["status"] == if accepted { "accepted" } else { "refused" })
        );
        assert_eq!(
            h.scope(&s).unwrap()["proposals"].as_object().unwrap().len(),
            usize::from(accepted)
        );
        assert!(h.propose(&request, &BTreeMap::new(), 103).is_err());
        assert_eq!(h.state().unwrap().service.active, 0);
    }
}
#[test]
fn lost_integration_response_reconciles_after_expiry_without_another_write() {
    let (t, h, s, target, _other, frame, a, _b, repo) = source_setup();
    let (p, read) = patch(&h, &s, &target, &frame, &a);
    let pr = propose(&h, &p);
    let c = claim_patch(&h, &s, &pr, &a, &read).unwrap()["value"].clone();
    let request=h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),102),102).unwrap();
    let worker = worker::Worker::open(t.path().join("worker"), secret(2), 102).unwrap();
    let (result, files) = worker.run(&h, &request, 102).unwrap();
    h.record_result(&result, &files, 103).unwrap();
    let check = checks::Checker::new(secret(3))
        .check(&h, &s, &c, 104)
        .unwrap();
    h.record_check(&check, 104).unwrap();
    let result=h.integrate(&event(1,json!({"v":"openagents.coordination-native-integration.v1","requires":[],"coordinator":s,"claim":c,"recover":false}),104),104,None).unwrap();
    let head = git(&repo, &["rev-parse", "refs/heads/docs"], None);
    {
        let _guard = h.store.lock().unwrap();
        let mut state = h.state().unwrap();
        let id = h.scope_key(&state, &s).unwrap();
        let scope = state.scopes.get_mut(&id).unwrap();
        let lease = scope
            .claims
            .values_mut()
            .find(|l| l.reference == c)
            .unwrap();
        lease.integration.as_mut().unwrap()["outcome"] = Value::Null;
        lease.unknown = true;
        lease.phase = "integrating".into();
        scope.journal.pop();
        scope.revision -= 1;
        scope
            .proposals
            .get_mut(p["task"].as_str().unwrap())
            .unwrap()
            .status = "completed".into();
        h.save(&state).unwrap();
    }
    let h = Coordinator::open(t.path().join("coord"), secret(1))
        .unwrap()
        .mount_source(
            "docs".into(),
            workspace_host::host::Host::open(t.path().join("source"), secret(1)).unwrap(),
        )
        .unwrap();
    let recovered=h.integrate(&event(1,json!({"v":"openagents.coordination-native-integration.v1","requires":[],"coordinator":s,"claim":c,"recover":true}),300),300,None).unwrap();
    assert_eq!(recovered, result);
    assert_eq!(h.scope(&s).unwrap()["calls"], 3);
    assert_eq!(git(&repo, &["rev-parse", "refs/heads/docs"], None), head);
    control(&h, &s, &pr, &c, "release", 300).unwrap();
}
#[test]
fn native_cards_pass_the_shared_cap_parser_and_bind_exact_resources() {
    let (_t, h, s, target, other, _frame, _a, _b, _repo) = source_setup();
    let mut ceiling = effects();
    ceiling["delegates"] = json!(true);
    let controller = h.scope(&s).unwrap()["transport"]["target"].clone();
    for definition in [
        target.clone(),
        other,
        h.data_operation().unwrap(),
        h.container_operation(ceiling).unwrap(),
        controller,
    ] {
        let body =
            nostr::contracts::parse_strict(&h.store.get(&definition["artifact"]).unwrap()).unwrap();
        nostr::cap::parse_definition(&body).unwrap();
        assert_eq!(body["binding_contract"].as_object().unwrap().len(), 2);
    }
    let mut forged =
        nostr::contracts::parse_strict(&h.store.get(&target["artifact"]).unwrap()).unwrap();
    forged["effects"]["writes"] = json!(["b"]);
    let artifact = h
        .store
        .put(
            &nostr::contracts::jcs(&forged).unwrap(),
            "application/json",
            Some("openagents.cap.v1"),
        )
        .unwrap();
    let changed = json!({"id":target["id"],"artifact":artifact});
    assert!(h.operation(&changed).is_err());
}
#[test]
fn controller_byte_allowance_refuses_before_any_task_admission() {
    let (_t, h, s, target, frame) = setup(2);
    let proposal = proposal(&h, &s, &target, &frame, 1, None);
    let transport = h.scope(&s).unwrap()["transport"].clone();
    let request = transport::invoke(
        &secret(1),
        &key(1),
        &transport,
        proposal,
        workspace_host::artifact::token(),
        101,
    )
    .unwrap();
    let opened = nostr::execution::open_request(
        &request,
        &key(1),
        &secret(1),
        101,
        nostr::execution::Window::DEFAULT,
    )
    .unwrap();
    let nostr::execution::Body::Execute(execute) = opened.body else {
        panic!()
    };
    let mut payload = execute.payload;
    payload["bounds"]["output_bytes"] = json!(1);
    let small = h
        .seal(
            &key(1),
            nostr::execution::REQUEST_KIND,
            request.tags,
            &payload,
            101,
        )
        .unwrap();
    assert!(
        h.serve(&small, &BTreeMap::new(), 101)
            .unwrap_err()
            .contains("bounds")
    );
    assert!(
        h.scope(&s).unwrap()["proposals"]
            .as_object()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn long_container_allowance_charges_auxiliary_requests_once_and_keeps_short_child_leases() {
    let (_t, h, s, _target, frame) = setup(3);
    let old = h.state().unwrap();
    let mut policy = old.scopes[&h.scope_key(&old, &s).unwrap()].policy.clone();
    policy.max_container_seconds = Some(3600);
    let container = h.container_operation(policy.effects.clone()).unwrap();
    let data = h.data_operation().unwrap();
    let s = h
        .create_scope(
            policy.clone(),
            vec![],
            vec![container.clone(), data.clone()],
            100,
        )
        .unwrap();
    h.frame(&event(1,json!({"v":"openagents.coordination-native-frame.v1","requires":[],"coordinator":s,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&s),"frame":frame}),100),&h.store.get(&frame).unwrap(),100).unwrap();
    let mut p = proposal(&h, &s, &container, &frame, 3, None);
    p["effects"] = policy.effects;
    let pr = propose(&h, &p);
    let claim = event(
        1,
        json!({"v":"openagents.claim-request.v1","requires":[],"proposal":pr,"worker":key(1),"resources":[],"expected_revision":revision(&h,&s),"requested_until":1000,"reservation":h.scope(&s).unwrap()["allowance"]}),
        101,
    );
    let c = h.claim(&claim, 101).unwrap()["value"].clone();
    let request = workspace_host::artifact::token();
    let charge = json!({"v":"openagents.coordination-native-auxiliary.v1","requires":[],"coordinator":s,"claim":c,"command":workspace_host::artifact::token(),"purpose":"observe","request":request,"fingerprint":nostr::contracts::digest_bytes(b"exact signed source request")});
    let signed = event(1, charge.clone(), 102);
    let receipt = h.charge_auxiliary(&signed, 102).unwrap();
    assert_eq!(h.charge_auxiliary(&signed, 102).unwrap(), receipt);
    assert_eq!(h.scope(&s).unwrap()["calls"], 1);
    let mut changed = charge;
    changed["command"] = json!(workspace_host::artifact::token());
    assert!(
        h.charge_auxiliary(&event(1, changed, 102), 102)
            .unwrap_err()
            .contains("conflict")
    );
    let child = proposal(
        &h,
        &s,
        &data,
        &frame,
        1,
        Some(p["task"].as_str().unwrap().into()),
    );
    let child = propose(&h, &child);
    let request = event(
        1,
        json!({"v":"openagents.claim-request.v1","requires":[],"proposal":child,"worker":key(1),"resources":[],"expected_revision":revision(&h,&s),"requested_until":900,"reservation":h.store.json(&c).unwrap()["reservation"]}),
        102,
    );
    assert!(h.claim(&request, 102).unwrap_err().contains("lifetime"));
}
#[test]
fn delegated_control_requires_an_owned_current_ancestor_and_cannot_take_another_parent() {
    let (_t, h, s, target, frame) = setup(8);
    let mut parent_effects = effects();
    parent_effects["delegates"] = json!(true);
    let container = h.container_operation(parent_effects.clone()).unwrap();
    let mut parent = proposal(&h, &s, &container, &frame, 4, None);
    parent["effects"] = parent_effects.clone();
    let parent_ref = propose(&h, &parent);
    let owner_claim = claim(
        &h,
        &s,
        &parent_ref,
        h.scope(&s).unwrap()["allowance"].clone(),
    );
    let child = proposal(
        &h,
        &s,
        &target,
        &frame,
        2,
        Some(parent["task"].as_str().unwrap().into()),
    );
    let child_ref = propose(&h, &child);
    let body = json!({"v":"openagents.claim-request.v1","requires":[],"proposal":child_ref,"worker":key(2),"resources":[],"expected_revision":revision(&h,&s),"requested_until":190,"reservation":h.store.json(&owner_claim).unwrap()["reservation"]});
    assert!(
        h.claim(&event(3, body, 102), 102)
            .unwrap_err()
            .contains("parent worker")
    );
    let child_claim = claim(
        &h,
        &s,
        &child_ref,
        h.store.json(&owner_claim).unwrap()["reservation"].clone(),
    );
    let foreign_dispatch = event(
        3,
        json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":child_claim}),
        102,
    );
    assert!(
        h.prepare(&foreign_dispatch, 102)
            .unwrap_err()
            .contains("ancestor")
    );
    let mut parent2 = proposal(&h, &s, &container, &frame, 4, None);
    parent2["effects"] = parent_effects;
    let parent2_ref = propose(&h, &parent2);
    let claim3=h.claim(&event(3,json!({"v":"openagents.claim-request.v1","requires":[],"proposal":parent2_ref,"worker":key(3),"resources":[],"expected_revision":revision(&h,&s),"requested_until":190,"reservation":h.scope(&s).unwrap()["allowance"]}),102),102).unwrap()["value"].clone();
    let child2 = proposal(
        &h,
        &s,
        &target,
        &frame,
        2,
        Some(parent2["task"].as_str().unwrap().into()),
    );
    let child2_ref = propose(&h, &child2);
    let child2_claim=h.claim(&event(3,json!({"v":"openagents.claim-request.v1","requires":[],"proposal":child2_ref,"worker":key(2),"resources":[],"expected_revision":revision(&h,&s),"requested_until":190,"reservation":h.store.json(&claim3).unwrap()["reservation"]}),102),102).unwrap()["value"].clone();
    h.prepare(&event(3,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":child2_claim}),103),103).unwrap();
    h.control(&event(3,json!({"v":"openagents.coordination-control.v1","requires":[],"coordinator":s,"proposal":child2_ref,"claim":child2_claim,"action":"cancel","expected_revision":revision(&h,&s),"requested_until":null,"evidence":[]}),103),103).unwrap();
}
#[test]
fn automation_source_uses_real_git_versions_and_never_recaptures_an_exact_poll() {
    let (_t, h, _old, _target, _other, frame, a, _b, repo) = source_setup();
    let source = h.automation_source_operation("a").unwrap();
    let mut ceiling = effects();
    ceiling["reads"] = json!(["artifact", "a"]);
    ceiling["delegates"] = json!(true);
    let container = h.container_operation(ceiling.clone()).unwrap();
    let scope = h
        .create_scope(
            Policy {
                recipients: BTreeSet::from([key(1), key(2), key(3)]),
                checkers: BTreeSet::from([key(3)]),
                calls: 6,
                concurrency: 8,
                max_claim_seconds: 100,
                max_container_seconds: None,
                max_depth: 8,
                until: 5000,
                effects: ceiling.clone(),
                parent: None,
            },
            vec![a],
            vec![source.clone(), container.clone()],
            100,
        )
        .unwrap();
    h.control_grants(&event(1,json!({"v":"openagents.coordination-native-control.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&scope),"actor":key(2),"roles":["read","claim","control"],"resources":[],"targets":[],"all":true,"until":5000,"revoke":false,"scope_revoke":false}),100),100).unwrap();
    h.frame(&event(1,json!({"v":"openagents.coordination-native-frame.v1","requires":[],"coordinator":scope,"command":workspace_host::artifact::token(),"expected_revision":revision(&h,&scope),"frame":frame}),100),&h.store.get(&frame).unwrap(),100).unwrap();
    let mut parent = proposal(&h, &scope, &container, &frame, 6, None);
    parent["effects"] = ceiling;
    let parent_ref = propose(&h, &parent);
    let parent_claim = claim(
        &h,
        &scope,
        &parent_ref,
        h.scope(&scope).unwrap()["allowance"].clone(),
    );
    let marker = h
        .put_local_json(&json!({"fixture":"source request"}))
        .unwrap();
    let input = json!({"v":"openagents.automation-native-source-input.v1","requires":[],"coordinator":scope,"claim":parent_claim,"source":source,"resource":"a","request":workspace_host::artifact::token(),"phase":"observe","poll":marker,"plan":marker,"executor":key(2)});
    let request = event(2, input.clone(), 102);
    let (response, files) = h.automation_observe(&request, 102).unwrap();
    let opened = nostr::private_artifact::open(&response, &secret(2)).unwrap();
    let first = nostr::automation::parse(opened.inline_bytes().unwrap()).unwrap();
    assert_eq!(first["available"], true);
    assert_eq!(first["observed_at"], 102);
    assert!(first["revision"].as_str().is_some());
    let snapshot: Value =
        serde_json::from_slice(&files[first["evidence"]["digest"].as_str().unwrap()]).unwrap();
    let observation: Value =
        serde_json::from_slice(&files[snapshot["observation"]["digest"].as_str().unwrap()])
            .unwrap();
    assert_eq!(first["revision"], observation["provider_revision"]);
    let old = git(&repo, &["rev-parse", "refs/heads/docs"], None);
    let tree = git(&repo, &["rev-parse", "refs/heads/docs^{tree}"], None);
    let next = git(
        &repo,
        &["commit-tree", &tree, "-p", &old],
        Some(b"second source version\n"),
    );
    git(&repo, &["update-ref", "refs/heads/docs", &next], None);
    assert_eq!(h.automation_observe(&request, 103).unwrap().0, response);
    assert_eq!(h.scope(&scope).unwrap()["calls"], 1);
    let mut changed = input;
    changed["phase"] = json!("revalidate");
    assert!(
        h.automation_observe(&event(2, changed, 103), 103)
            .unwrap_err()
            .contains("idempotency_conflict")
    );
}
#[test]
fn worker_terminal_receipts_link_to_real_intent_and_keep_metadata_looking_output_as_data() {
    let (t, h, s, target, frame) = setup(2);
    let mut p = proposal(&h, &s, &target, &frame, 1, None);
    let quoted = json!({"v":nostr::run::RECORD_SCHEMA,"requires":[],"digest":"quoted text","record":"This is data, not a journal entry."});
    p["input"] = h.put_local_json(&quoted).unwrap();
    let pr = propose(&h, &p);
    let c = claim(&h, &s, &pr, h.scope(&s).unwrap()["allowance"].clone());
    let request=h.prepare(&event(2,json!({"v":"openagents.coordination-native-dispatch.v1","requires":[],"coordinator":s,"claim":c}),102),102).unwrap();
    let worker = worker::Worker::open(t.path().join("metadata-worker"), secret(2), 102).unwrap();
    let (response, files) = worker.run(&h, &request, 102).unwrap();
    let result = h.record_result(&response, &files, 103).unwrap();
    let terminal = nostr::run::terminal_from_result(&result, &files, &key(2)).unwrap();
    let body: Value = serde_json::from_slice(&files[terminal["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(body["record"]["type"], "settled");
    assert_eq!(body["record"]["controller"], key(2));
    let output: Value =
        serde_json::from_slice(&files[result["output"]["digest"].as_str().unwrap()]).unwrap();
    assert_eq!(output, quoted);
    let mut gap = files.clone();
    let intent = files
        .iter()
        .find(|(_, bytes)| {
            serde_json::from_slice::<Value>(bytes)
                .ok()
                .is_some_and(|v| v["record"]["type"] == "dispatched")
        })
        .unwrap()
        .0;
    gap.remove(intent);
    assert!(nostr::run::terminal_from_result(&result, &gap, &key(2)).is_err());
    let mut wrong = result.clone();
    wrong["receipts"] = json!([result["record"]]);
    assert!(nostr::run::terminal_from_result(&wrong, &files, &key(2)).is_err());
    wrong = result.clone();
    wrong["outcome"] = json!("failed");
    assert!(nostr::run::terminal_from_result(&wrong, &files, &key(2)).is_err());
}

fn external_target(h: &Coordinator, operation: &str) -> (Value, BTreeMap<String, Vec<u8>>) {
    let native = h.data_operation().unwrap();
    let mut body =
        nostr::contracts::parse_strict(&h.store.get(&native["artifact"]).unwrap()).unwrap();
    let id = format!("{}:session/external", key(2));
    body["id"] = json!(id);
    body["binding_contract"]["operation"] = json!(operation);
    body["binding_contract"]["interface"] = json!("openagents.fixture-external.v1");
    let bytes = nostr::contracts::jcs(&body).unwrap();
    let artifact =
        workspace_host::artifact::reference(&bytes, "application/json", Some("openagents.cap.v1"));
    (
        json!({"id":id,"artifact":artifact}),
        BTreeMap::from([(nostr::contracts::digest_bytes(&bytes), bytes)]),
    )
}
fn external_scope(h: &Coordinator, target: &Value, frame: &Value) -> Value {
    let policy = h
        .state()
        .unwrap()
        .scopes
        .values()
        .next()
        .unwrap()
        .policy
        .clone();
    let scope = h
        .create_scope(policy, vec![], vec![target.clone()], 100)
        .unwrap();
    for n in [2, 3] {
        let control = json!({"v":"openagents.coordination-native-control.v1","requires":[],
            "coordinator":scope,"command":workspace_host::artifact::token(),
            "expected_revision":revision(h,&scope),"actor":key(n),
            "roles":["read","propose","claim","control","finding"],"resources":[],"targets":[],
            "all":true,"until":5000,"revoke":false,"scope_revoke":false});
        h.control_grants(&event(1, control, 100), 100).unwrap();
    }
    let control = json!({"v":"openagents.coordination-native-frame.v1","requires":[],
        "coordinator":scope,"command":workspace_host::artifact::token(),
        "expected_revision":revision(h,&scope),"frame":frame});
    h.frame(&event(1, control, 100), &h.store.get(frame).unwrap(), 100)
        .unwrap();
    scope
}

#[test]
fn external_executor_pins_are_operator_owned_exact_and_revocable() {
    let (temp, h, _, _, frame) = setup(8);
    let (target, files) = external_target(&h, "session-submit");
    assert!(h.operation(&target).is_err());
    assert!(h.register_executor(&target, &key(3), &files).is_err());
    assert_eq!(
        h.register_executor(&target, &key(2), &files).unwrap(),
        target
    );
    assert_eq!(
        h.register_executor(&target, &key(2), &BTreeMap::new())
            .unwrap(),
        target
    );
    assert_eq!(
        h.operation(&target).unwrap()["binding_contract"]["operation"],
        "session-submit"
    );
    let mut changed = target.clone();
    changed["id"] = json!(format!("{}:session/changed", key(2)));
    assert!(h.register_executor(&changed, &key(2), &files).is_err());
    let scope = external_scope(&h, &target, &frame);
    let p = proposal(&h, &scope, &target, &frame, 2, None);
    let pr = propose(&h, &p);
    let bad = json!({"v":"openagents.claim-request.v1","requires":[],"proposal":pr,
        "worker":key(3),"resources":[],"expected_revision":revision(&h,&scope),
        "requested_until":190,"reservation":h.scope(&scope).unwrap()["allowance"]});
    assert!(
        h.claim(&event(3, bad, 101), 101)
            .unwrap_err()
            .contains("another executor")
    );
    let c = claim(
        &h,
        &scope,
        &pr,
        h.scope(&scope).unwrap()["allowance"].clone(),
    );
    let prepare = event(
        2,
        json!({"v":"openagents.coordination-native-dispatch.v1",
        "requires":[],"coordinator":scope,"claim":c}),
        102,
    );
    let request = h.prepare_environment(&prepare, 102).unwrap();
    assert_eq!(h.scope(&scope).unwrap()["calls"], 0);
    h.revoke_executor(&target).unwrap();
    assert!(h.operation(&target).is_err());
    assert!(h.register_executor(&target, &key(2), &files).is_err());
    let restarted = Coordinator::open(temp.path().join("coord"), secret(1)).unwrap();
    assert!(restarted.operation(&target).is_err());
    let current = restarted.state().unwrap();
    let actual = current
        .scopes
        .values()
        .find(|s| s.reference == scope)
        .unwrap();
    let lease = actual.claims.values().find(|l| l.reference == c).unwrap();
    assert!(restarted.current_lease(actual, lease, 103).is_err());
    assert_eq!(lease.execution.as_ref().unwrap(), &request);
    assert!(!lease.dispatched);
    assert_eq!(actual.calls, 0);
}

#[test]
fn a_foreign_binding_cannot_impersonate_the_fixed_worker_before_input_or_charge() {
    for operation in ["session-submit", "coord-data"] {
        let (temp, h, _, _, frame) = setup(8);
        let (target, files) = external_target(&h, operation);
        h.register_executor(&target, &key(2), &files).unwrap();
        let scope = external_scope(&h, &target, &frame);
        let p = proposal(&h, &scope, &target, &frame, 2, None);
        let pr = propose(&h, &p);
        let c = claim(
            &h,
            &scope,
            &pr,
            h.scope(&scope).unwrap()["allowance"].clone(),
        );
        let prepare = event(
            2,
            json!({"v":"openagents.coordination-native-dispatch.v1",
            "requires":[],"coordinator":scope,"claim":c}),
            102,
        );
        let request = h.prepare(&prepare, 102).unwrap();
        let worker = worker::Worker::open(temp.path().join("worker"), secret(2), 102).unwrap();
        assert!(
            worker
                .run(&h, &request, 102)
                .unwrap_err()
                .starts_with("unsupported_feature:")
        );
        assert_eq!(h.scope(&scope).unwrap()["calls"], 0);
        let state = h.state().unwrap();
        let actual = state
            .scopes
            .values()
            .find(|s| s.reference == scope)
            .unwrap();
        let lease = actual.claims.values().find(|l| l.reference == c).unwrap();
        assert!(!lease.dispatched);
        assert!(!lease.unknown);
    }
}
