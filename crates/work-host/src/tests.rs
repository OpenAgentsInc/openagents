use super::*;
use authority::Authority;
use nostr::{contracts, domain::Event};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn key(n: u8) -> String {
    nostr::domain::RelaySigner::from_secret_hex(&secret(n).display_secret().to_string())
        .unwrap()
        .pubkey()
        .into()
}
fn signed(body: Value, n: u8, now: u64) -> Event {
    workspace_host::host::request(&secret(n), &key(1), body, now).unwrap()
}
fn setup() -> (tempfile::TempDir, Authority, Value) {
    let temp = tempfile::tempdir().unwrap();
    let host = Authority::initialize(temp.path().join("host"), secret(1)).unwrap();
    let scope = host.create_scope([key(3)].into(), 100).unwrap();
    (temp, host, scope)
}
fn work(host: &Authority, scope: &Value, id: u8) -> Value {
    let state = host.state().unwrap();
    let idscope = state
        .scopes
        .iter()
        .find(|(_, s)| s.reference == *scope)
        .unwrap()
        .0;
    json!({"authority":key(1),"scope":idscope,"item":format!("{id:064x}")})
}
fn planning(host: &Authority) -> Value {
    json!({"v":"openagents.work-item.v1","requires":[],"kind":"project","title":"A tracked project","body":host.store.put(b"Objective", "text/plain",None).unwrap(),"accountable":key(1),"state":"open","frame":null,"acceptance":null,"delegation":null,"relations":[],"resources":[],"evidence":[],"disposition":null,"labels":[],"due_at":null,"timebox":null})
}
fn apply(
    host: &Authority,
    scope: &Value,
    w: &Value,
    item: Value,
    expected: Value,
    id: u8,
    n: u8,
    now: u64,
) -> Result<Value> {
    let snapshot = host.store.put_json(&item)?;
    let command = json!({"v":"openagents.work-command.v1","requires":[],"issuer":key(n),"command_id":format!("{id:064x}"),"scope":scope,"work":w,"action":if expected.is_null(){"create"}else{"replace"},"expected":expected,"snapshot":snapshot,"source":null,"reason":null,"expires_at":400});
    host.command(&signed(command, n, now), &BTreeMap::new(), now)
}
fn grant(host: &Authority, scope: &Value, n: u8, items: Vec<String>, roles: Vec<&str>, now: u64) {
    let body = json!({"v":"openagents.work-native-control.v1","requires":[],"issuer":key(1),"scope":scope,"actor":key(n),"roles":roles,"all":false,"items":items,"until":500,"revoke_scope":false});
    host.control(&signed(body, 1, now), now).unwrap();
}
fn framed(host: &Authority) -> Value {
    let mut item = planning(host);
    let task = workspace_host::artifact::token();
    let snapshot=host.store.put_json(&json!({"v":"openagents.snapshot.v1","requires":[],"scope":"work-tests","captured_at":100,"sources":[],"coverage":"complete"})).unwrap();
    let precedence = host
        .store
        .put_json(&json!({"v":"openagents.work-empty-instruction-policy.v1","requires":[]}))
        .unwrap();
    let resolver = json!({"id":format!("{}:work/empty-instructions",key(1)),"artifact":precedence});
    let instructions=host.store.put_json(&json!({"v":"openagents.instructions.v1","requires":[],"task":task,"revision":0,"resolver":resolver,"precedence":precedence,"snapshot":snapshot,"entries":[]})).unwrap();
    let acceptance=host.store.put_json(&json!({"v":"openagents.work-check-policy.v1","requires":[],"checks":["exact-output"],"independent":true})).unwrap();
    let objective = host
        .store
        .put(b"bounded objective", "text/plain", None)
        .unwrap();
    let frame = json!({"v":"openagents.task-frame.v1","requires":[],"task":task,"owner":key(1),"controller":key(1),"revision":0,"previous":null,"objective":objective,"origin":"user","constraints":[],"acceptance":[acceptance],"snapshot":snapshot,"instructions":instructions,"variables":[],"attempts":[],"unresolved":[],"corrections":[]});
    item["kind"] = json!("work");
    item["frame"] = host.store.put_json(&frame).unwrap();
    item["acceptance"] = acceptance;
    item
}
#[test]
fn conditional_history_has_exact_retries_stale_conflicts_and_current_rights() {
    let (_t, host, scope) = setup();
    let w = work(&host, &scope, 1);
    let item = planning(&host);
    let created = apply(&host, &scope, &w, item.clone(), Value::Null, 1, 1, 101).unwrap();
    assert_eq!(created["status"], "accepted");
    assert_eq!(
        apply(&host, &scope, &w, item.clone(), Value::Null, 1, 1, 102).unwrap(),
        created
    );
    let mut next = item.clone();
    next["title"] = json!("Changed");
    assert_eq!(
        apply(&host, &scope, &w, next.clone(), Value::Null, 1, 1, 102).unwrap()["status"],
        "conflict"
    );
    let replaced = apply(
        &host,
        &scope,
        &w,
        next,
        created["revision"].clone(),
        2,
        1,
        103,
    )
    .unwrap();
    assert_eq!(replaced["status"], "accepted");
    assert_eq!(
        apply(
            &host,
            &scope,
            &w,
            item.clone(),
            created["revision"].clone(),
            3,
            1,
            104
        )
        .unwrap()["status"],
        "conflict"
    );
    grant(
        &host,
        &scope,
        2,
        vec![w["item"].as_str().unwrap().into()],
        vec!["read"],
        105,
    );
    let mut changed = item;
    changed["title"] = json!("Unauthorized");
    assert!(
        apply(
            &host,
            &scope,
            &w,
            changed,
            replaced["revision"].clone(),
            4,
            2,
            106
        )
        .is_err()
    );
}
#[test]
fn a_real_delegated_cj_job_uses_frozen_inputs_and_survives_restart() {
    let (t, host, scope) = setup();
    let w = work(&host, &scope, 1);
    let mut item = framed(&host);
    let first = apply(&host, &scope, &w, item.clone(), Value::Null, 1, 1, 101).unwrap();
    let frame = host.store.json(&item["frame"]).unwrap();
    let body = json!({"v":"openagents.work-execution-grant.v1","requires":[],"issuer":key(1),"scope":scope,"work":w,"delegate":key(2),"frame":item["frame"],"parent":null,"recipients":[key(2)],"reads":[frame["objective"]["digest"]],"effects":[],"calls":2,"wall_ms":60000,"output_bytes":1048576,"until":300});
    let g = host.execution_grant(&signed(body, 1, 102), 102).unwrap();
    let d = json!({"v":"openagents.work-delegation.v1","requires":[],"issuer":key(1),"work":w,"basis":first["revision"],"delegate":key(2),"frame":item["frame"],"grant":g,"until":300,"executions":[],"order":null});
    item["delegation"] = host.submit_declaration(&signed(d, 1, 103), 103).unwrap();
    let second = apply(
        &host,
        &scope,
        &w,
        item.clone(),
        first["revision"].clone(),
        2,
        1,
        104,
    )
    .unwrap();
    assert!(
        host.state()
            .unwrap()
            .scopes
            .values()
            .all(|s| s.jobs.is_empty())
    );
    let body = json!({"v":"openagents.work-job-proposal.v1","requires":[],"issuer":key(1),"job":format!("{:064x}",4),"work":w,"basis":second["revision"],"delegation":item["delegation"]});
    let proposal = signed(body, 1, 105);
    let job = host.prepare_job(&proposal, 105).unwrap();
    let request: Event = serde_json::from_value(job["request"].clone()).unwrap();
    let worker = execution::PureWorker::open(t.path().join("worker"), secret(2), 105).unwrap();
    let (result, files) = worker.run(&host, &request, 106).unwrap();
    let payload = host.record_worker_result(&result, &files, 107).unwrap();
    assert_eq!(payload["outcome"], "completed");
    assert_eq!(payload["verification"], "not_run");
    drop(host);
    let host = Authority::open(t.path().join("host"), secret(1)).unwrap();
    assert_eq!(host.prepare_job(&proposal, 108).unwrap(), job);
    let worker = execution::PureWorker::open(t.path().join("worker"), secret(2), 108).unwrap();
    assert_eq!(worker.run(&host, &request, 109).unwrap().0, result);
    assert_eq!(
        host.record_worker_result(&result, &files, 109).unwrap(),
        payload
    );
    grant(
        &host,
        &scope,
        3,
        vec![w["item"].as_str().unwrap().into()],
        vec!["read", "evidence"],
        110,
    );
    let (report, check_files) =
        checks::verify_job(&host, &secret(3), job["job"].as_str().unwrap(), 111).unwrap();
    let report_ref = host.submit_check(&report, &check_files, 111).unwrap();
    let evidence = json!({"v":"openagents.work-evidence.v1","requires":[],"issuer":key(3),"work":w,"basis":second["revision"],"type":"verification","artifact":report_ref,"sources":[]});
    let evidence = host
        .submit_declaration(&signed(evidence, 3, 112), 112)
        .unwrap();
    item["evidence"] = json!([evidence]);
    let linked = apply(
        &host,
        &scope,
        &w,
        item.clone(),
        second["revision"].clone(),
        3,
        1,
        113,
    )
    .unwrap();
    let reason = host
        .store
        .put(
            b"The independently checked output satisfies the pinned policy.",
            "text/plain",
            None,
        )
        .unwrap();
    let disposition = json!({"v":"openagents.work-disposition.v1","requires":[],"issuer":key(1),"work":w,"basis":linked["revision"],"outcome":"accepted","evidence":[evidence],"successor":null,"reason":reason});
    item["disposition"] = host
        .submit_declaration(&signed(disposition, 1, 114), 114)
        .unwrap();
    item["state"] = json!("resolved");
    let accepted = apply(
        &host,
        &scope,
        &w,
        item.clone(),
        linked["revision"].clone(),
        4,
        1,
        115,
    )
    .unwrap();
    assert_eq!(accepted["status"], "accepted");
    let old_disposition = item["disposition"].clone();
    let disposition = json!({"v":"openagents.work-disposition.v1","requires":[],"issuer":key(1),"work":w,"basis":accepted["revision"],"outcome":"reopened","evidence":[],"successor":null,"reason":reason});
    item["disposition"] = host
        .submit_declaration(&signed(disposition, 1, 116), 116)
        .unwrap();
    item["state"] = json!("open");
    let reopened = apply(
        &host,
        &scope,
        &w,
        item.clone(),
        accepted["revision"].clone(),
        5,
        1,
        117,
    )
    .unwrap();
    assert_eq!(reopened["status"], "accepted");
    assert_eq!(
        host.store.json(&old_disposition).unwrap()["outcome"],
        "accepted"
    );
    item["disposition"] = Value::Null;
    assert!(
        apply(
            &host,
            &scope,
            &w,
            item,
            reopened["revision"].clone(),
            6,
            1,
            118
        )
        .unwrap_err()
        .contains("clearing")
    );
}

#[test]
fn admission_crash_retains_unknown_without_replaying_or_reusing_identity() {
    let (t, mut host, scope) = setup();
    let w = work(&host, &scope, 1);
    let item = planning(&host);
    host.crash_after_admission = true;
    assert!(
        apply(&host, &scope, &w, item.clone(), Value::Null, 1, 1, 101)
            .unwrap_err()
            .contains("stopped")
    );
    drop(host);
    let host = Authority::open(t.path().join("host"), secret(1)).unwrap();
    let unknown = apply(&host, &scope, &w, item.clone(), Value::Null, 1, 1, 102).unwrap();
    assert_eq!(unknown["status"], "unknown");
    assert!(unknown["revision"].is_null());
    assert_eq!(
        host.store.json(&unknown["record"]).unwrap()["record"]["data"]["dispatched"],
        true
    );
    assert_eq!(
        apply(&host, &scope, &w, item.clone(), Value::Null, 1, 1, 103).unwrap(),
        unknown
    );
    assert_eq!(
        apply(&host, &scope, &w, item, Value::Null, 2, 1, 104).unwrap()["status"],
        "conflict"
    );
}
#[test]
fn planning_graph_rejects_cycles_and_readers_retain_signed_forks() {
    let (_t, host, scope) = setup();
    let a = work(&host, &scope, 1);
    let b = work(&host, &scope, 2);
    let mut item = planning(&host);
    let ra = apply(&host, &scope, &a, item.clone(), Value::Null, 1, 1, 101).unwrap();
    item["relations"] = json!([{"type":"blocks","target":a,"evidence":null}]);
    apply(&host, &scope, &b, item, Value::Null, 2, 1, 102).unwrap();
    let mut item = planning(&host);
    item["relations"] = json!([{"type":"blocks","target":b,"evidence":null}]);
    assert!(
        apply(&host, &scope, &a, item, ra["revision"].clone(), 3, 1, 103)
            .unwrap_err()
            .contains("cycle")
    );
    let read = signed(
        json!({"v":"openagents.work-history-read.v1","requires":[],"issuer":key(1),"work":a}),
        1,
        104,
    );
    let history = host.read_history(&read, 104).unwrap();
    let event: Event = serde_json::from_value(history["records"][0]["event"].clone()).unwrap();
    let mut reader = reader::Reader::default();
    let descriptor = host.store.json(&scope).unwrap();
    reader
        .admit_scope(&signed(descriptor, 1, 104), &secret(1), &key(1))
        .unwrap();
    reader.ingest(&event, &secret(1)).unwrap();
    let mut fork = host.store.json(&ra["revision"]["artifact"]).unwrap();
    fork["recorded_at"] = json!(105);
    let event = signed(fork, 1, 105);
    assert!(
        reader
            .ingest(&event, &secret(1))
            .unwrap_err()
            .contains("forked")
    );
    host.observe_fork(&event, 105).unwrap();
    assert_eq!(
        apply(
            &host,
            &scope,
            &a,
            planning(&host),
            ra["revision"].clone(),
            4,
            1,
            106
        )
        .unwrap()["status"],
        "conflict"
    );
}
#[test]
fn two_scoped_clients_converge_and_revoked_views_stop_reading() {
    let (_t, host, scope) = setup();
    let w = work(&host, &scope, 1);
    let initial = apply(&host, &scope, &w, planning(&host), Value::Null, 1, 1, 101).unwrap();
    let hidden = work(&host, &scope, 2);
    let mut private = planning(&host);
    private["title"] = json!("Hidden item");
    apply(&host, &scope, &hidden, private, Value::Null, 2, 1, 102).unwrap();
    let mut clients = Vec::new();
    for n in [2, 3] {
        grant(
            &host,
            &scope,
            n,
            vec![w["item"].as_str().unwrap().into()],
            vec!["read"],
            103,
        );
        let event = signed(
            json!({"v":"openagents.work-native-view.v1","requires":[],"issuer":key(n),"scope":scope,"all":false,"items":[w["item"]],"retain_until":400}),
            n,
            104,
        );
        let projection = host.define_view(&event, 104).unwrap();
        let mut client = workspace_host::projection::Client::default();
        let request = json!({"v":"openagents.projection-read.v1","requires":[],"request":format!("{n:064x}"),"projection":projection,"mode":"snapshot","after":null,"max_items":256,"max_bytes":1048576});
        let result = host
            .projection_read(&signed(request.clone(), n, 105), 105)
            .unwrap();
        assert_eq!(result["status"], "completed");
        let fetch = |reference: &Value| {
            let event = signed(
                json!({"v":"openagents.work-view-fetch.v1","requires":[],"issuer":key(n),"projection":projection,"artifact":reference,"max_bytes":1048576}),
                n,
                105,
            );
            contracts::parse_strict(&host.projection_fetch(&event, 105).unwrap()).unwrap()
        };
        let page = fetch(&result["page"]);
        let cut = fetch(&page["cut"]);
        client.snapshot_page(&request, &page, &cut).unwrap();
        assert_eq!(page["rows"].as_array().unwrap().len(), 1);
        clients.push((n, projection, client, page["resume"].clone()));
    }
    let mut next = planning(&host);
    next["title"] = json!("Current title");
    let accepted = apply(
        &host,
        &scope,
        &w,
        next,
        initial["revision"].clone(),
        3,
        1,
        106,
    )
    .unwrap();
    assert_eq!(accepted["status"], "accepted");
    for (n, projection, client, after) in &mut clients {
        let request = json!({"v":"openagents.projection-read.v1","requires":[],"request":format!("{:064x}",10+*n),"projection":projection,"mode":"changes","after":after,"max_items":256,"max_bytes":1048576});
        let result = host
            .projection_read(&signed(request.clone(), *n, 107), 107)
            .unwrap();
        let fetch = |reference: &Value| {
            let event = signed(
                json!({"v":"openagents.work-view-fetch.v1","requires":[],"issuer":key(*n),"projection":projection,"artifact":reference,"max_bytes":1048576}),
                *n,
                107,
            );
            contracts::parse_strict(&host.projection_fetch(&event, 107).unwrap()).unwrap()
        };
        let page = fetch(&result["page"]);
        for r in page["deltas"].as_array().unwrap() {
            let delta = fetch(r);
            let cut = fetch(&delta["to"]);
            client.delta(&delta, &fetch(&delta["from"]), &cut).unwrap();
        }
    }
    assert_eq!(clients[0].2.rows, clients[1].2.rows);
    grant(&host, &scope, 2, Vec::new(), Vec::new(), 108);
    let event = signed(
        json!({"v":"openagents.projection-read.v1","requires":[],"request":format!("{:064x}",55),"projection":clients[0].1,"mode":"snapshot","after":null,"max_items":256,"max_bytes":1048576}),
        2,
        109,
    );
    assert_eq!(
        host.projection_read(&event, 109).unwrap()["reason"],
        "not_admitted"
    );
}

#[test]
fn github_import_preserves_node_identity_and_requires_explicit_native_resolution() {
    let (_t, host, scope) = setup();
    let capture = json!({"repo":"example/repo","node":"I_stable_node","number":7,"title":"Observed issue","updated":"2026-10-15T12:00:00Z","state":"closed","body_digest":contracts::digest_bytes(b"full body"),"excerpt":"bounded excerpt"});
    let proposal = host
        .plan_github(
            &scope,
            capture.clone(),
            planning(&host),
            None,
            None,
            99,
            101,
        )
        .unwrap();
    let event: Event = serde_json::from_value(proposal["event"].clone()).unwrap();
    let accepted = host.command(&event, &BTreeMap::new(), 102).unwrap();
    let w = proposal["work"].clone();
    let source = host.store.json(&proposal["source"]).unwrap();
    assert_eq!(
        host.store.json(&source["assurance"]).unwrap()["consistency"],
        "observational"
    );
    assert_eq!(
        host.head(
            &host.state().unwrap().scopes[w["scope"].as_str().unwrap()],
            w["item"].as_str().unwrap()
        )
        .unwrap()
        .unwrap()
        .2["state"],
        "open"
    );
    assert!(
        host.plan_github(
            &scope,
            capture.clone(),
            planning(&host),
            None,
            None,
            103,
            104
        )
        .unwrap_err()
        .contains("already exists")
    );
    let mut changed = planning(&host);
    changed["title"] = json!("Native edit");
    let updated = apply(
        &host,
        &scope,
        &w,
        changed,
        accepted["revision"].clone(),
        2,
        1,
        105,
    )
    .unwrap();
    let stale = host
        .plan_github(
            &scope,
            capture.clone(),
            planning(&host),
            Some(accepted["revision"].clone()),
            Some(b"Resolve source changes".to_vec()),
            103,
            106,
        )
        .unwrap();
    let event: Event = serde_json::from_value(stale["event"].clone()).unwrap();
    assert_eq!(
        host.command(&event, &BTreeMap::new(), 107).unwrap()["status"],
        "conflict"
    );
    let resolved = host
        .plan_github(
            &scope,
            capture,
            planning(&host),
            Some(updated["revision"].clone()),
            Some(b"Keep native state and retain this observed source".to_vec()),
            103,
            108,
        )
        .unwrap();
    assert_eq!(resolved["work"], w);
    let event: Event = serde_json::from_value(resolved["event"].clone()).unwrap();
    assert_eq!(
        host.command(&event, &BTreeMap::new(), 109).unwrap()["status"],
        "accepted"
    );
}
#[test]
fn cached_private_artifacts_cannot_be_copied_by_digest() {
    let (_t, host, scope) = setup();
    let a = work(&host, &scope, 1);
    let b = work(&host, &scope, 2);
    let item = planning(&host);
    let before = apply(&host, &scope, &a, item.clone(), Value::Null, 1, 1, 101).unwrap();
    let mut private = planning(&host);
    private["body"] = host
        .store
        .put(b"private bytes", "text/plain", None)
        .unwrap();
    apply(&host, &scope, &b, private.clone(), Value::Null, 2, 1, 102).unwrap();
    grant(
        &host,
        &scope,
        2,
        vec![a["item"].as_str().unwrap().into()],
        vec!["read", "revise"],
        103,
    );
    let mut proposed = item;
    proposed["body"] = private["body"].clone();
    let bytes = contracts::jcs(&proposed).unwrap();
    let reference = workspace_host::artifact::json_ref(&proposed).unwrap();
    let mut files = BTreeMap::from([(reference["digest"].as_str().unwrap().to_string(), bytes)]);
    let command = json!({"v":"openagents.work-command.v1","requires":[],"issuer":key(2),"command_id":format!("{:064x}",3),"scope":scope,"work":a,"action":"replace","expected":before["revision"],"snapshot":reference,"source":null,"reason":null,"expires_at":400});
    let event = signed(command, 2, 104);
    assert!(
        host.command(&event, &files, 104)
            .unwrap_err()
            .contains("not supplied or readable")
    );
    files.insert(
        private["body"]["digest"].as_str().unwrap().into(),
        b"private bytes".to_vec(),
    );
    assert_eq!(
        host.command(&event, &files, 104).unwrap()["status"],
        "accepted"
    );
    let request = signed(
        json!({"v":"openagents.work-artifact-read.v1","requires":[],"issuer":key(2),"work":b,"artifact":private["body"],"max_bytes":100}),
        2,
        105,
    );
    assert!(host.fetch_artifact(&request, 105).is_err());
}

#[test]
fn attenuated_reassignment_blocks_old_intake_and_keeps_its_call_reservation() {
    let (temp, host, scope) = setup();
    let w = work(&host, &scope, 1);
    let mut item = framed(&host);
    let created = apply(&host, &scope, &w, item.clone(), Value::Null, 1, 1, 101).unwrap();
    grant(
        &host,
        &scope,
        2,
        vec![w["item"].as_str().unwrap().into()],
        vec!["read", "assign"],
        102,
    );
    let frame = host.store.json(&item["frame"]).unwrap();
    let grant_body = json!({"v":"openagents.work-execution-grant.v1","requires":[],"issuer":key(1),"scope":scope,"work":w,"delegate":key(2),"frame":item["frame"],"parent":null,"recipients":[key(2),key(3)],"reads":[frame["objective"]["digest"]],"effects":[],"calls":2,"wall_ms":60000,"output_bytes":1048576,"until":300});
    let root = host
        .execution_grant(&signed(grant_body.clone(), 1, 103), 103)
        .unwrap();
    let delegation = json!({"v":"openagents.work-delegation.v1","requires":[],"issuer":key(1),"work":w,"basis":created["revision"],"delegate":key(2),"frame":item["frame"],"grant":root,"until":300,"executions":[],"order":null});
    item["delegation"] = host
        .submit_declaration(&signed(delegation, 1, 104), 104)
        .unwrap();
    let assigned = apply(
        &host,
        &scope,
        &w,
        item.clone(),
        created["revision"].clone(),
        2,
        1,
        105,
    )
    .unwrap();
    let proposal = json!({"v":"openagents.work-job-proposal.v1","requires":[],"issuer":key(1),"job":format!("{:064x}",40),"work":w,"basis":assigned["revision"],"delegation":item["delegation"]});
    let old = host.prepare_job(&signed(proposal, 1, 106), 106).unwrap();
    let mut child = grant_body;
    child["issuer"] = json!(key(2));
    child["delegate"] = json!(key(3));
    child["parent"] = root.clone();
    child["recipients"] = json!([key(3)]);
    child["calls"] = json!(1);
    let mut widened = child.clone();
    widened["reads"] = json!([
        frame["objective"]["digest"],
        contracts::digest_bytes(b"unadmitted")
    ]);
    assert!(host.execution_grant(&signed(widened, 2, 107), 107).is_err());
    let child = host.execution_grant(&signed(child, 2, 107), 107).unwrap();
    let delegation = json!({"v":"openagents.work-delegation.v1","requires":[],"issuer":key(2),"work":w,"basis":assigned["revision"],"delegate":key(3),"frame":item["frame"],"grant":child,"until":300,"executions":[],"order":null});
    item["delegation"] = host
        .submit_declaration(&signed(delegation, 2, 108), 108)
        .unwrap();
    let narrowed = apply(
        &host,
        &scope,
        &w,
        item.clone(),
        assigned["revision"].clone(),
        3,
        1,
        109,
    )
    .unwrap();
    let worker =
        execution::PureWorker::open(temp.path().join("old-worker"), secret(2), 110).unwrap();
    let old_request: Event = serde_json::from_value(old["request"].clone()).unwrap();
    assert!(worker.run(&host, &old_request, 110).is_err());
    let state = host.state().unwrap();
    let scope_state = &state.scopes[w["scope"].as_str().unwrap()];
    assert_eq!(
        scope_state.executions[root["digest"].as_str().unwrap()].used,
        1
    );
    assert_eq!(
        scope_state.jobs[old["job"].as_str().unwrap()].status,
        "prepared"
    );
    let proposal = json!({"v":"openagents.work-job-proposal.v1","requires":[],"issuer":key(2),"job":format!("{:064x}",41),"work":w,"basis":narrowed["revision"],"delegation":item["delegation"]});
    let job = host.prepare_job(&signed(proposal, 2, 111), 111).unwrap();
    let request: Event = serde_json::from_value(job["request"].clone()).unwrap();
    let worker =
        execution::PureWorker::open(temp.path().join("child-worker"), secret(3), 112).unwrap();
    let (event, files) = worker.run(&host, &request, 112).unwrap();
    assert_eq!(
        host.record_worker_result(&event, &files, 113).unwrap()["outcome"],
        "completed"
    );
    assert_eq!(worker.run(&host, &request, 200).unwrap().0, event);
}

#[test]
fn work_history_does_not_disclose_scope_policy_or_grant_bytes() {
    let (_temp, host, scope) = setup();
    let w = work(&host, &scope, 1);
    let state = host.state().unwrap();
    let native = &state.scopes[w["scope"].as_str().unwrap()];
    let private_grant = native.grants[&key(1)].reference.clone();
    let private_policy = native.policy_ref.clone();
    let mut item = planning(&host);
    item["meta"] = json!({"grant_reference":private_grant,"policy_reference":private_policy});
    let accepted = apply(&host, &scope, &w, item, Value::Null, 1, 1, 101).unwrap();
    grant(
        &host,
        &scope,
        2,
        vec![w["item"].as_str().unwrap().into()],
        vec!["read"],
        102,
    );
    for mut reference in [private_grant, private_policy] {
        for strip in [false, true] {
            if strip {
                reference.as_object_mut().unwrap().remove("schema");
            }
            let request = signed(
                json!({"v":"openagents.work-artifact-read.v1","requires":[],"issuer":key(2),"work":w,"artifact":reference,"max_bytes":1048576}),
                2,
                103,
            );
            assert!(host.fetch_artifact(&request, 103).is_err());
        }
    }
    let request = signed(
        json!({"v":"openagents.work-artifact-read.v1","requires":[],"issuer":key(2),"work":w,"artifact":accepted["revision"]["artifact"],"max_bytes":1048576}),
        2,
        103,
    );
    assert!(host.fetch_artifact(&request, 103).is_ok());
}
