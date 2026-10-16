//! Managed relationships composed with separately admitted shared COORD work.
use super::*;

fn container(r: &mut Real, calls: u64) -> (Value, Value) {
    let frame: Value =
        serde_json::from_slice(&r.coordinator.get_local_bytes(&r.frame).unwrap()).unwrap();
    let scope: Value =
        serde_json::from_slice(&r.coordinator.get_local_bytes(&r.scope).unwrap()).unwrap();
    let definition: Value = serde_json::from_slice(
        &r.coordinator
            .get_local_bytes(&r.container["artifact"])
            .unwrap(),
    )
    .unwrap();
    let lock=r.coordinator.put_local_json(&json!({"v":"openagents.lock.v1","requires":[],
        "root":r.container,"entries":[{"id":r.container["id"],"definition":r.container,"dependencies":[]}]})).unwrap();
    let input = r
        .coordinator
        .put_local_json(&json!({"purpose":"two explicitly admitted session turns"}))
        .unwrap();
    let context = r
        .coordinator
        .put_local_json(&json!({"v":"openagents.context.v1","requires":[],
        "task":frame["task"],"recipient":pk(HOST),"policy":scope["policy"],"entries":[],
        "omissions":[],"coverage":"complete"}))
        .unwrap();
    let body = json!({"v":"openagents.task-proposal.v1","requires":[],"task":workspace_host::artifact::token(),
        "coordinator":r.scope,"frame":r.frame,"parent":null,"target":r.container,"lock":lock,
        "input":input,"context":context,"dependencies":[],"effects":definition["effects"],
        "bounds":[{"bound":"calls","ceiling":calls,"assurance":"host","mechanism":"native-ledger"}],
        "acceptance":frame["acceptance"][0],"reuse":"never"});
    let proposal = r
        .coordinator
        .propose(&r.signed(body.clone()), &BTreeMap::new(), r.fx.now)
        .unwrap()["value"]
        .clone();
    let claim=r.coordinator.claim(&r.signed(json!({"v":"openagents.claim-request.v1","requires":[],
        "proposal":proposal,"worker":pk(HOST),"resources":[],"expected_revision":r.revision(),
        "requested_until":r.fx.now+300,"reservation":r.coordinator.scope(&r.scope).unwrap()["allowance"]})),r.fx.now).unwrap()["value"].clone();
    let claim_body: Value =
        serde_json::from_slice(&r.coordinator.get_local_bytes(&claim).unwrap()).unwrap();
    r.parent = Some((
        body["task"].as_str().unwrap().into(),
        claim_body["reservation"].clone(),
    ));
    (proposal, claim)
}
fn link(
    r: &mut Real,
    host: &SessionHost,
    opening: &Value,
    child: &Value,
    event: &nostr::domain::Event,
) {
    let session = opening["session"].as_str().unwrap();
    let child = child["session"].as_str().unwrap();
    let status = host.status(session).unwrap();
    let grant = host
        .set_child_grants(
            vec![crate::host::ChildGrant {
                parent: session.into(),
                turn: status["active_turn"].as_str().unwrap().into(),
                child: child.into(),
                expires_at: r.fx.now + 299,
            }],
            r.fx.now,
        )
        .unwrap()
        .remove(0);
    let command = r.fx.command(
        host,
        session,
        "link_child",
        json!({"turn":status["active_turn"],
        "expected_turn_revision":status["turn_revision"],"child":child,"authorization":grant}),
    );
    let reply = host
        .continue_turn(event, &r.fx.bundle(OWNER, &command), r.fx.now)
        .unwrap();
    assert_eq!(reply["status"], "accepted", "{reply}");
}
fn child_claim(r: &Real, binding: &Binding) -> Value {
    let grant = r.allocator().store().json(&binding.grant).unwrap();
    let claim: Value =
        serde_json::from_slice(&r.coordinator.get_local_bytes(&grant["claim"]).unwrap()).unwrap();
    let reservation: Value = serde_json::from_slice(
        &r.coordinator
            .get_local_bytes(&claim["reservation"])
            .unwrap(),
    )
    .unwrap();
    reservation
}

fn separate_environment(r: &mut Real) {
    let allocator = r.allocator();
    let lease = allocator.store().json(&r.lease).unwrap();
    let old = allocator.store().json(&r.materialization).unwrap();
    let mut request = allocator.store().json(&lease["request"]).unwrap();
    request["request"] = json!(workspace_host::artifact::token());
    request["run"] = json!(workspace_host::artifact::token());
    let allocated = allocator
        .submit(&r.env_bundle(OWNER, &request), r.fx.now)
        .unwrap();
    assert_eq!(allocated["status"], "accepted", "{allocated}");
    r.lease = allocated["lease"].clone();
    r.lease_id = allocator.store().json(&r.lease).unwrap()["lease"]
        .as_str()
        .unwrap()
        .into();
    allocator.materialize(&r.lease_id,&MaterializeSpec { participants:old["participants"].as_array().unwrap().clone(),
        enforcement:json!([{"bound":"wall_ms","ceiling":600_000,"assurance":"host","mechanism":"env-host"}]),
        evidence:vec![],bootstrap:None },r.fx.now).unwrap();
    r.materialization = allocator
        .store()
        .json(&allocator.status(&r.lease_id).unwrap()["state"])
        .unwrap()["materialization"]
        .clone();
    r.copy_from(
        allocator.store(),
        vec![r.lease.clone(), r.materialization.clone()],
    );
}
fn no_remaining_child_allowance(r: &Real, binding: &Binding) {
    let grant = r.allocator().store().json(&binding.grant).unwrap();
    let claim: Value =
        serde_json::from_slice(&r.coordinator.get_local_bytes(&grant["claim"]).unwrap()).unwrap();
    let mut proposal: Value =
        serde_json::from_slice(&r.coordinator.get_local_bytes(&claim["proposal"]).unwrap())
            .unwrap();
    proposal["task"] = json!(workspace_host::artifact::token());
    let proposed = r
        .coordinator
        .propose(&r.signed(proposal), &BTreeMap::new(), r.fx.now)
        .unwrap()["value"]
        .clone();
    let result = r.coordinator.claim(
        &r.signed(json!({"v":"openagents.claim-request.v1","requires":[],
        "proposal":proposed,"worker":pk(HOST),"resources":[],"expected_revision":r.revision(),
        "requested_until":r.fx.now+300,"reservation":r.parent.as_ref().unwrap().1})),
        r.fx.now,
    );
    assert!(result.unwrap_err().contains("parent call allowance"));
}

#[test]
fn linked_native_sessions_execute_only_with_separate_shared_child_claims() {
    for restart in [false, true] {
        let mut r = Real::for_children();
        let host = r.host(LoopEngine::mid_turn());
        let child = r.opened(&host);
        let child_environment = (
            r.lease.clone(),
            r.materialization.clone(),
            r.lease_id.clone(),
        );
        separate_environment(&mut r);
        let opening = r.opened(&host);
        let (proposal, claim) = container(&mut r, 4);
        let command = r.submit_command(&host, &opening, 1);
        let (parent_event, parent_binding) = r.job(&command);
        host.serve(&parent_event, &r.fx.artifacts, &parent_binding, r.fx.now)
            .unwrap();
        link(&mut r, &host, &opening, &child, &parent_event);
        let child_command = r.submit_command(&host, &child, 2);
        // A relationship and the parent's attachment are no execution grant.
        let result = host.submit(
            &r.fx.bundle(OWNER, &child_command),
            Some(&parent_binding),
            r.fx.now,
        );
        assert!(result.is_err() || result.unwrap()["status"] != "accepted");
        assert_eq!(host.started("loop-mid-turn"), 1);
        (r.lease, r.materialization, r.lease_id) = child_environment;
        let (child_event, child_binding) = r.job(&child_command);
        no_remaining_child_allowance(&r, &child_binding);
        let parent_reservation = child_claim(&r, &parent_binding);
        let child_reservation = child_claim(&r, &child_binding);
        let container_reservation: Value = serde_json::from_slice(
            &r.coordinator
                .get_local_bytes(&r.parent.as_ref().unwrap().1)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(parent_reservation["parent"], container_reservation["id"]);
        assert_eq!(child_reservation["parent"], container_reservation["id"]);
        assert_ne!(child_reservation["id"], parent_reservation["id"]);
        host.serve(&child_event, &r.fx.artifacts, &child_binding, r.fx.now)
            .unwrap();
        assert_eq!(host.started("loop-mid-turn"), 2);
        let parent = host.poll(&parent_event, r.fx.now).unwrap();
        assert_eq!(parent.0.kind, execution::FEEDBACK_KIND);
        assert_eq!(
            host.status(opening["session"].as_str().unwrap()).unwrap()["phase"],
            "blocked"
        );
        let host = if restart {
            drop(host);
            r.host(LoopEngine::mid_turn())
        } else {
            host
        };
        let mut child_result = host.poll(&child_event, r.fx.now).unwrap();
        if child_result.0.kind != execution::RESULT_KIND {
            child_result = host.poll(&child_event, r.fx.now).unwrap();
        }
        assert_eq!(child_result.0.kind, execution::RESULT_KIND);
        let child_record = r
            .coordinator
            .record_result(&child_result.0, &child_result.1, r.fx.now);
        if restart {
            assert!(
                child_record
                    .unwrap_err()
                    .contains("unaccounted worker outcome")
            );
        } else {
            assert_eq!(child_record.unwrap()["outcome"], "completed");
        }
        let parent = host.poll(&parent_event, r.fx.now).unwrap();
        assert_eq!(parent.0.kind, execution::RESULT_KIND);
        let parent_record = r.coordinator.record_result(&parent.0, &parent.1, r.fx.now);
        if restart {
            assert!(
                parent_record
                    .unwrap_err()
                    .contains("unaccounted worker outcome")
            );
            let release = r.coordinator.control(
                &r.signed(
                    json!({"v":"openagents.coordination-control.v1","requires":[],
                "coordinator":r.scope,"proposal":proposal,"claim":claim,"action":"release",
                "expected_revision":r.revision(),"requested_until":null,"evidence":[]}),
                ),
                r.fx.now,
            );
            assert!(
                release
                    .unwrap_err()
                    .contains("descendant reservations remain held")
            );
            no_remaining_child_allowance(&r, &child_binding);
        } else {
            assert_eq!(parent_record.unwrap()["outcome"], "completed");
        }
        assert_eq!(host.started("loop-mid-turn"), if restart { 0 } else { 2 });
        assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 4);
        assert_eq!(
            parent
                .1
                .values()
                .filter_map(|b| serde_json::from_slice::<Value>(b).ok())
                .any(|v| v["v"] == "openagents.session-quiescence.v1"),
            !restart
        );
    }
}

#[test]
fn cancelling_the_shared_container_blocks_a_prepared_child_before_native_dispatch() {
    let mut r = Real::for_children();
    let host = r.host(LoopEngine::mid_turn());
    let child = r.opened(&host);
    let child_environment = (
        r.lease.clone(),
        r.materialization.clone(),
        r.lease_id.clone(),
    );
    separate_environment(&mut r);
    let opening = r.opened(&host);
    let (proposal, claim) = container(&mut r, 4);
    let command = r.submit_command(&host, &opening, 1);
    let (parent_event, parent_binding) = r.job(&command);
    host.serve(&parent_event, &r.fx.artifacts, &parent_binding, r.fx.now)
        .unwrap();
    link(&mut r, &host, &opening, &child, &parent_event);
    (r.lease, r.materialization, r.lease_id) = child_environment;
    let child_command = r.submit_command(&host, &child, 2);
    let (child_event, child_binding) = r.job(&child_command);
    no_remaining_child_allowance(&r, &child_binding);
    r.coordinator
        .control(
            &r.signed(
                json!({"v":"openagents.coordination-control.v1","requires":[],
        "coordinator":r.scope,"proposal":proposal,"claim":claim,"action":"cancel",
        "expected_revision":r.revision(),"requested_until":null,"evidence":[]}),
            ),
            r.fx.now,
        )
        .unwrap();
    let result = host.serve(&child_event, &r.fx.artifacts, &child_binding, r.fx.now);
    assert!(result.unwrap_err().contains("not_admitted"));
    assert_eq!(host.started("loop-mid-turn"), 1);
    assert_eq!(
        host.status(child["session"].as_str().unwrap()).unwrap()["active_turn"],
        Value::Null
    );
    host.advance(r.fx.now).unwrap();
    assert_eq!(host.started("loop-mid-turn"), 1);
    assert_eq!(r.coordinator.scope(&r.scope).unwrap()["calls"], 3);
}
