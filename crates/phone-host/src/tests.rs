use super::*;
use serde_json::json;
fn key(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn grant() -> (wire::Access, SecretKey) {
    let host = key(1);
    let owner = key(2);
    let device = key(3);
    let time = now().unwrap();
    let g = coder_access::protocol::Grant {
        v: coder_access::protocol::GRANT.into(),
        requires: vec![],
        grant: "a".repeat(64),
        host: coder_reach::pubkey(&host),
        owner: coder_reach::pubkey(&owner),
        device: coder_reach::pubkey(&device),
        relay: "wss://relay.example.com/".into(),
        rights: coder_access::Rights::standard(),
        scope: None,
        epoch: 0,
        origin: coder_access::protocol::Origin {
            kind: coder_access::protocol::OriginKind::Invitation,
            id: "b".repeat(64),
            issuer: coder_reach::pubkey(&owner),
        },
        issued_at: time,
        expires_at: time + 3600,
    };
    let event = coder_connect::protocol::seal(
        &g,
        coder_access::protocol::GRANT,
        &host,
        &g.device,
        &g.grant,
        time,
        time + 3600,
    )
    .unwrap();
    let a =
        Access::from_authorization(event, &device, &g.host, time, RelayPolicy::Production).unwrap();
    (project_access(&a).unwrap(), device)
}
#[test]
fn signed_grants_survive_protobuf_and_reject_projection_changes() {
    let (a, device) = grant();
    let raw = a.authorization.as_ref().unwrap().json.clone();
    let bytes = a.encode_to_vec();
    engine_proto::wire::validate("openagents.host.v1.Access", &bytes).unwrap();
    let decoded = wire::Access::decode(&*bytes).unwrap();
    assert_eq!(decoded.authorization.as_ref().unwrap().json, raw);
    let mut q = wire::AgentRequest {
        schema: 1,
        access: Some(decoded),
        ..Default::default()
    };
    assert!(access(&q, &device).is_ok());
    assert!(access(&q, &key(4)).is_err());
    q.access
        .as_mut()
        .unwrap()
        .grant
        .as_mut()
        .unwrap()
        .rights
        .push("access_admin".into());
    assert!(access(&q, &device).is_err());
}
#[test]
fn shared_artifacts_use_the_existing_semantic_validator() {
    let native=nostr::contracts::parse_artifact(&json!({"digest":format!("sha256:{}","a".repeat(64)),"size":5,"media_type":"text/plain","sources":[{"url":"https://example.com/a.txt"}]})).unwrap();
    let projected = refs::project(&native);
    assert_eq!(refs::artifact(&projected).unwrap(), native);
    let mut invalid = projected.clone();
    invalid.sources[0].event = Some(common::EventRef {
        kind: 70000,
        ..Default::default()
    });
    assert!(refs::artifact(&invalid).is_err());
    let mut invalid = projected;
    invalid.digest = "not-a-digest".into();
    assert!(refs::artifact(&invalid).is_err());
}
#[test]
fn task_identity_and_read_only_are_bound_before_signing() {
    let mut q = wire::AgentRequest {
        schema: 1,
        operation: "start".into(),
        phone_run: "phone-run-0001".into(),
        workspace: "aethyr".into(),
        objective: "Inspect the actual source".into(),
        ..Default::default()
    };
    let host = coder_reach::pubkey(&key(1));
    let Operation::CreateTask { task } = operation(&q, &host).unwrap() else {
        panic!()
    };
    assert_eq!(task.read_only, Some(true));
    assert_eq!(task.engine, Some(nostr::cj_conversation::Engine::Codex));
    q.remote_task = "f".repeat(64);
    assert!(operation(&q, &host).is_err());
    q.remote_task = task_id(&host, &q.phone_run).unwrap();
    assert!(operation(&q, &host).is_ok());
    q.allow_writes = true;
    let Operation::CreateTask { task } = operation(&q, &host).unwrap() else {
        panic!()
    };
    assert_eq!(task.read_only, None);
}
#[test]
fn cancellation_receipt_does_not_claim_remote_completion() {
    let p = wire::PreparedCall {
        phone_run: "phone-run-0001".into(),
        remote_task: "a".repeat(64),
        operation: "cancel".into(),
        ..Default::default()
    };
    let reply = progress(
        Outcome::Dispatched {
            receipt: coder_access::protocol::Receipt {
                operation: "task.cancel".into(),
                reference: p.remote_task.clone(),
            },
        },
        &p,
    )
    .unwrap();
    assert_eq!(reply.progress.unwrap().state, "running");
    assert_eq!(reply.host_phase, "stopping");
}
#[test]
fn tool_status_and_actual_call_ids_are_preserved() {
    let page:coder_access::task_read::Page=serde_json::from_value(json!({"scope":{"workspace":"aethyr","task":"a".repeat(64),"revision":4,"attempt":1,"intent_digest":format!("sha256:{}","b".repeat(64))},"title":"Inspect","prompt":"Inspect","phase":"completed","execution":"finished","verification":"not_run","integration":"unknown","termination":"completed","delivery":"unknown","cleanup":"clear","cost_microusd":null,"cost_status":"unknown","evidence":{"state":"sealed","original":null,"total_steps":2,"steps":[{"kind":"original","index":0,"step":{"source":"agent","message":"Checking","tool_calls":[{"tool_call_id":"actual-native-call","function_name":"shell","arguments":{"command":"false"}}],"observation":{"results":[{"source_call_id":"actual-native-call","content":"actual failure","extra":{"status":"failed"}}]}}},{"kind":"original","index":1,"step":{"source":"agent","message":"The command failed as expected."}}],"faults":[],"more_faults":false},"artifacts":[],"children":[],"more_children":false,"next":null,"more_available":false})).unwrap();
    let p = wire::PreparedCall {
        phone_run: "phone-run-0001".into(),
        remote_task: "a".repeat(64),
        workspace: "aethyr".into(),
        operation: "read".into(),
        ..Default::default()
    };
    let reply = progress(
        Outcome::Task {
            task: Box::new(page),
        },
        &p,
    )
    .unwrap();
    let state = reply.progress.unwrap();
    assert_eq!(state.state, "succeeded");
    let tool = state.events.iter().find_map(|e| e.tool.as_ref()).unwrap();
    assert_eq!(tool.id, "actual-native-call");
    assert_eq!(tool.state, "failed");
    assert_eq!(tool.output, "actual failure");
    assert_eq!(state.result, "The command failed as expected.");
}
#[test]
fn malformed_native_requests_refuse_before_network() {
    let input = [8, 1, 0x98, 0x06, 1];
    let key = std::ffi::CString::new(key(9).display_secret().to_string()).unwrap();
    let mut len = 0;
    let bytes = unsafe {
        openagents_phone_host_call(input.as_ptr(), input.len(), key.as_ptr(), 0, None, &mut len)
    };
    assert!(!bytes.is_null());
    let response =
        wire::AgentReply::decode(unsafe { std::slice::from_raw_parts(bytes, len) }).unwrap();
    unsafe { openagents_phone_host_free(bytes, len) };
    assert!(
        response
            .error
            .unwrap()
            .message
            .contains("unknown semantic field")
    );
}

#[test]
fn random_phone_identity_is_the_native_request_identity() {
    let run = "d".repeat(64);
    assert_eq!(task_id(&"a".repeat(64), &run).unwrap(), run);
}
#[test]
fn shared_boundary_rejects_unknown_nested_fields_and_oversized_strings() {
    let mut a = wire::Access {
        authorization: Some(common::SignedEvent {
            json: "x".repeat(524289),
        }),
        ..Default::default()
    };
    assert!(engine_proto::wire::validate("openagents.host.v1.Access", &a.encode_to_vec()).is_err());
    a.authorization = Some(common::SignedEvent { json: "{}".into() });
    let mut nested = a.encode_to_vec();
    nested.extend_from_slice(&[0x98, 0x06, 1]);
    let mut request = vec![0x22]; // AgentRequest.access (4), length-delimited
    assert!(nested.len() < 128);
    request.push(nested.len() as u8);
    request.extend_from_slice(&nested);
    assert_eq!(
        engine_proto::wire::validate("openagents.host.v1.AgentRequest", &request),
        Err("unknown semantic field")
    );
}

#[test]
fn continuation_pins_the_native_cursor_revision() {
    let run = "a".repeat(64);
    let cursor = wire::TaskCursor {
        scope: Some(wire::TaskScope {
            workspace: "aethyr".into(),
            task: run.clone(),
            revision: 4,
            attempt: Some(1),
            intent_digest: format!("sha256:{}", "b".repeat(64)),
        }),
        source: "trace:1".into(),
        source_digest: format!("sha256:{}", "c".repeat(64)),
        source_bytes: 100,
        next_step: 2,
        prefix_digest: format!("sha256:{}", "d".repeat(64)),
    };
    let q = wire::AgentRequest {
        operation: "read".into(),
        phone_run: run.clone(),
        remote_task: run,
        workspace: "aethyr".into(),
        cursor: Some(cursor),
        ..Default::default()
    };
    let Operation::ReadTask { query } = operation(&q, &"b".repeat(64)).unwrap() else {
        panic!()
    };
    query.validate().unwrap();
    assert_eq!(query.revision, Some(4));
}
#[test]
fn native_microcoder_generated_reply_is_preserved() {
    let step = json!({"source":"system","message":"Microcoder loop observation.","extra":{"microcoder":{"event":{"event":"generated","generated":{"action":{"Ok":{"reply":"Read the real C ABI.","finished":true}}}}}}});
    assert_eq!(agent_text(&step), Some("Read the real C ABI."));
}
