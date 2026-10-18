//! Offline tests for the remote voice protocol against a fake native Codex package.
use serde_json::json;
use openagents_engine::{EngineCore, EngineProfile, HarnessRegistry};
use openagents_proto::HarnessId;
use openagents_rpc::methods;
#[path = "voice_fixture/mod.rs"]
mod voice_fixture;

#[cfg(unix)]
fn native_package(root: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../harness/tests/fixtures");
    let binary = root.join("bin/codex");
    let helper = root.join("codex-resources/voice/bin/codex-voice-host");
    std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
    std::fs::create_dir_all(helper.parent().unwrap()).unwrap();
    std::fs::write(
        root.join("codex-package.json"),
        r#"{"layoutVersion":1,"version":"0.159.0"}"#,
    )
    .unwrap();
    for (source, target) in [
        ("fake-codex-voice-native.py", &binary),
        ("fake-codex-voice-host.py", &helper),
    ] {
        std::fs::copy(fixture.join(source), target).unwrap();
        std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    binary
}
#[cfg(unix)]
async fn native_core(temp: &tempfile::TempDir) -> (EngineCore, voice_fixture::VoiceFixture) {
    let package = temp.path().join("codex-package");
    let binary = native_package(&package);
    let registry = std::sync::Arc::new(HarnessRegistry::new());
    let release = std::sync::Arc::new(tokio::sync::Semaphore::new(0));
    registry.register(std::sync::Arc::new(voice_fixture::AnswerHarness(
        release.clone(),
    )));
    registry.register(std::sync::Arc::new(
        openagents_harness::CodexHarness::new()
            .with_executable(binary)
            .with_voice_auth_home(temp.path().join("fixture-auth")),
    ));
    let core = EngineCore::assemble_with_profile(
        EngineProfile::local(&temp.path().join("data")).unwrap(),
        registry,
        HarnessId::Codex,
        None,
    )
    .unwrap();
    let config =
        serde_json::from_value(json!({"harness":"codex","sandbox":"danger-full-access"})).unwrap();
    core.workspace
        .create_chat(
            "native-voice",
            None,
            Some(&core.device_id),
            Some(config),
            Some(package.display().to_string()),
        )
        .unwrap();
    let fixture = voice_fixture::VoiceFixture::attach(&core, temp.path(), release).await;
    (core, fixture)
}

#[cfg(unix)]
#[tokio::test]
async fn remote_failed_prepare_keeps_old_transcript_but_retry_starts_fresh() {
    use openagents_proto::voice::{ORCHESTRATOR_CHAT_PREFIX, remote as wire};
    let temp = tempfile::tempdir().unwrap();
    let (core, _account) = native_core(&temp).await;
    let client = openagents_rpc::memory_client(core.rpc_service());
    let config =
        serde_json::from_value(json!({"harness":"codex","sandbox":"danger-full-access"})).unwrap();
    let mut request = wire::Prepare {
        attempt_key: wire::AttemptKey::new(),
        config,
        voice: Some("invalid-voice".into()),
    };
    let envelope = |p: serde_json::Value| json!({"targetDeviceId":core.device_id,"payload":p});

    // An earlier call has a long transcript and a saved thread.
    let previous = format!("{ORCHESTRATOR_CHAT_PREFIX}previous");
    core.workspace
        .create_chat(
            &previous,
            None,
            Some(&core.device_id),
            Some(request.config.clone()),
            None,
        )
        .unwrap();
    core.workspace
        .set_chat_harness_session(&previous, "remembered-thread", "");
    const ENTRIES: usize = 1000;
    let doc = core.doc_host.open(&previous).unwrap();
    for i in 0..ENTRIES {
        doc.write_user_message(&format!("m{i}"), "remember me", i as i64)
            .unwrap();
    }

    // An unsupported voice exercises preparation cleanup.
    let failure = client
        .call(methods::PREPARE_VOICE_V2, envelope(json!(request)))
        .await
        .unwrap_err();
    assert!(
        matches!(failure, openagents_rpc::RpcError::Failed(ref message)
            if message == "voice unavailable: Unsupported"),
        "unexpected preparation failure: {failure:?}"
    );
    let remaining: Vec<_> = core
        .workspace
        .read_chats()
        .unwrap()
        .into_iter()
        .filter(|c| openagents_proto::voice::is_orchestrator_chat(&c.id))
        .collect();
    assert_eq!(
        remaining.len(),
        1,
        "cleanup must remove the empty successor"
    );
    assert_eq!(
        remaining[0].id, previous,
        "cleanup must keep the predecessor"
    );
    assert_eq!(
        core.workspace.chat_harness_session(&previous).unwrap().0,
        "remembered-thread"
    );
    assert_eq!(doc.doc().read_entries().unwrap().len(), ENTRIES);

    // A fresh attempt never resumes the earlier call.
    request.attempt_key = wire::AttemptKey::new();
    request.voice = None;
    let prepared: wire::Prepared = client
        .call_as(methods::PREPARE_VOICE_V2, envelope(json!(request)))
        .await
        .unwrap();
    assert_ne!(prepared.chat_id, previous);
    assert_eq!(
        core.sessions
            .last_request(&prepared.chat_id)
            .unwrap()
            .resume
            .as_deref(),
        None
    );
    assert!(core.workspace.chat(&previous).unwrap().is_some());
    client
        .call(methods::STOP_VOICE_V2, envelope(json!(prepared.lease)))
        .await
        .unwrap();
    core.sessions.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn remote_voice_full_control_flow_and_idempotent_prepare_without_host_audio() {
    use openagents_proto::voice::{VoiceEvent, remote as wire};
    let temp = tempfile::tempdir().unwrap();
    let (core, _account) = native_core(&temp).await;
    std::fs::remove_file(
        temp.path()
            .join("codex-package/codex-resources/voice/bin/codex-voice-host"),
    )
    .unwrap();
    let client = openagents_rpc::memory_client(core.rpc_service());
    let request = wire::Prepare {
        attempt_key: wire::AttemptKey::new(),
        config: serde_json::from_value(json!({"harness":"codex","sandbox":"danger-full-access"}))
            .unwrap(),
        voice: None,
    };
    let envelope = |p: serde_json::Value| json!({"targetDeviceId":core.device_id,"payload":p});
    let info: openagents_proto::EngineInfo = client
        .call_as(methods::ENGINE_INFO, json!({}))
        .await
        .unwrap();
    assert!(info.supports(wire::CAPABILITY));
    let capabilities: wire::Capabilities = client
        .call_as(methods::VOICE_CAPABILITIES_V2, envelope(json!({})))
        .await
        .unwrap();
    assert_eq!(capabilities.protocol, wire::CAPABILITY);
    assert!(capabilities.client_webrtc);
    let call = envelope(serde_json::to_value(&request).unwrap());
    let prepared: wire::Prepared = client
        .call_as(methods::PREPARE_VOICE_V2, call.clone())
        .await
        .unwrap();
    let again: wire::Prepared = client
        .call_as(methods::PREPARE_VOICE_V2, call)
        .await
        .unwrap();
    assert_eq!(again.chat_id, prepared.chat_id);
    assert_eq!(
        again.lease.voice.session_id,
        prepared.lease.voice.session_id
    );
    assert!(prepared.chat_id.starts_with("voice-orchestrator-"));
    assert_eq!(
        core.workspace
            .chat(&prepared.chat_id)
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some(openagents_proto::voice::ORCHESTRATOR_CHAT_TITLE)
    );
    let mut owner = client
        .subscribe_checked(
            methods::OWN_VOICE_V2,
            envelope(serde_json::to_value(&prepared.lease).unwrap()),
        )
        .await
        .unwrap();
    assert!(
        client
            .subscribe_checked(
                methods::OWN_VOICE_V2,
                envelope(serde_json::to_value(&prepared.lease).unwrap())
            )
            .await
            .is_err()
    );
    let id = wire::AttemptKey::new();
    let negotiation = wire::Negotiate {
        lease: prepared.lease.clone(),
        negotiation_id: id.clone(),
        offer: wire::Sdp::new("fixture-offer".into()).unwrap(),
    };
    let reply: wire::Negotiated = client
        .call_as(
            methods::NEGOTIATE_VOICE_V2,
            envelope(serde_json::to_value(&negotiation).unwrap()),
        )
        .await
        .unwrap();
    assert_eq!(reply.answer.expose(), "fixture-answer");
    let repeat: wire::Negotiated = client
        .call_as(
            methods::NEGOTIATE_VOICE_V2,
            envelope(serde_json::to_value(&negotiation).unwrap()),
        )
        .await
        .unwrap();
    assert_eq!(repeat.answer, reply.answer);
    client
        .call(
            methods::CONFIRM_VOICE_MEDIA_V2,
            envelope(
                serde_json::to_value(wire::Confirm {
                    lease: prepared.lease.clone(),
                    negotiation_id: id,
                    muted: false,
                })
                .unwrap(),
            ),
        )
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if matches!(
                serde_json::from_value::<VoiceEvent>(owner.recv().await.unwrap()).unwrap(),
                VoiceEvent::Final { .. }
            ) {
                break;
            }
        }
    })
    .await
    .unwrap();
    drop(owner);
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while core.sessions.has_run(&prepared.chat_id) {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("hanging up retires the voice-only runtime");
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert!(
        client
            .call(
                methods::PREPARE_VOICE_V2,
                envelope(serde_json::to_value(request).unwrap())
            )
            .await
            .is_err()
    );
    assert!(!temp.path().join("codex-package/helper-wire.jsonl").exists());
    core.sessions.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn remote_cancel_before_prepare_and_owner_drop_preserve_other_calls() {
    use openagents_proto::voice::remote as wire;
    let temp = tempfile::tempdir().unwrap();
    let (core, _account) = native_core(&temp).await;
    let client = openagents_rpc::memory_client(core.rpc_service());
    let wrap = |p: serde_json::Value| json!({"targetDeviceId":core.device_id,"payload":p});
    let config =
        serde_json::from_value(json!({"harness":"codex","sandbox":"danger-full-access"})).unwrap();
    let mut request = wire::Prepare {
        attempt_key: wire::AttemptKey::new(),
        config,
        voice: None,
    };
    client
        .call(
            methods::CANCEL_VOICE_ATTEMPT_V2,
            wrap(json!({"attemptKey":request.attempt_key})),
        )
        .await
        .unwrap();
    assert!(
        client
            .call(methods::PREPARE_VOICE_V2, wrap(json!(request)))
            .await
            .is_err()
    );
    assert!(!temp.path().join("codex-package/voice-wire.jsonl").exists());
    request.attempt_key = wire::AttemptKey::new();
    let prepared: wire::Prepared = client
        .call_as(methods::PREPARE_VOICE_V2, wrap(json!(request)))
        .await
        .unwrap();
    let owner = client
        .subscribe_checked(methods::OWN_VOICE_V2, wrap(json!(prepared.lease)))
        .await
        .unwrap();
    drop(owner);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while core.workspace.chat(&prepared.chat_id).unwrap().is_some() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let _: openagents_proto::EngineInfo = client
        .call_as(methods::ENGINE_INFO, json!({}))
        .await
        .unwrap();
    // The dropped voice stream did not close the connection or another generation.
    request.attempt_key = wire::AttemptKey::new();
    let next: wire::Prepared = client
        .call_as(methods::PREPARE_VOICE_V2, wrap(json!(request)))
        .await
        .unwrap();
    client
        .call(methods::STOP_VOICE_V2, wrap(json!(prepared.lease)))
        .await
        .unwrap();
    let next_owner = client
        .subscribe_checked(methods::OWN_VOICE_V2, wrap(json!(next.lease)))
        .await
        .unwrap();
    drop(next_owner);
    core.sessions.shutdown().await;
}

#[cfg(unix)]
#[path = "support/voice_relay.rs"]
mod voice_relay;

#[cfg(unix)]
#[tokio::test]
async fn remote_voice_crosses_two_engines_and_owner_drop_releases_host() {
    use std::{sync::Arc, time::Duration};
    use openagents_proto::voice::{VoiceEvent, remote as wire};
    use openagents_rpc::{HostRelay, HostRelayConfig, LinkCache, LinkCacheConfig, StaticToken};
    let temp = tempfile::tempdir().unwrap();
    let (host, _account) = native_core(&temp).await;
    std::fs::remove_file(
        temp.path()
            .join("codex-package/codex-resources/voice/bin/codex-voice-host"),
    )
    .unwrap();
    let (url, room) = voice_relay::fake_device_room().await;
    let relay = HostRelay::spawn(
        HostRelayConfig::new(
            &url,
            host.device_id.clone(),
            Arc::new(StaticToken("test".into())),
        ),
        host.rpc_service(),
        Arc::new(|_| true),
    );
    let viewer_dir = tempfile::tempdir().unwrap();
    let viewer = EngineCore::assemble_with_profile(
        EngineProfile::local(viewer_dir.path()).unwrap(),
        Arc::new(HarnessRegistry::new()),
        HarnessId::Codex,
        None,
    )
    .unwrap();
    let mut links = LinkCacheConfig::new(url, Arc::new(StaticToken("test".into())));
    links.probe_timeout = Duration::from_secs(3);
    viewer.set_links(LinkCache::new(links));
    let client = openagents_rpc::memory_client(viewer.rpc_service());
    let wrap = |p: serde_json::Value| json!({"targetDeviceId":host.device_id,"payload":p});
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            if client
                .call(methods::VOICE_CAPABILITIES_V2, wrap(json!({})))
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let request = wire::Prepare {
        attempt_key: wire::AttemptKey::new(),
        config: serde_json::from_value(json!({"harness":"codex","sandbox":"danger-full-access"}))
            .unwrap(),
        voice: None,
    };
    let prepared: wire::Prepared = client
        .call_as(methods::PREPARE_VOICE_V2, wrap(json!(request)))
        .await
        .unwrap();
    assert!(viewer.workspace.chat(&prepared.chat_id).unwrap().is_none());
    let mut owner = client
        .subscribe_checked(methods::OWN_VOICE_V2, wrap(json!(prepared.lease)))
        .await
        .unwrap();
    let id = wire::AttemptKey::new();
    let _: wire::Negotiated = client
        .call_as(
            methods::NEGOTIATE_VOICE_V2,
            wrap(json!(wire::Negotiate {
                lease: prepared.lease.clone(),
                negotiation_id: id.clone(),
                offer: wire::Sdp::new("fixture-offer".into()).unwrap()
            })),
        )
        .await
        .unwrap();
    client
        .call(
            methods::CONFIRM_VOICE_MEDIA_V2,
            wrap(json!(wire::Confirm {
                lease: prepared.lease.clone(),
                negotiation_id: id,
                muted: false
            })),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !matches!(
            serde_json::from_value::<VoiceEvent>(owner.recv().await.unwrap()).unwrap(),
            VoiceEvent::Final { .. }
        ) {}
    })
    .await
    .unwrap();
    drop(owner);
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let report = wire::Report {
                lease: prepared.lease.clone(),
                sequence: 1,
                muted: true,
                state: wire::MediaState::Ready,
            };
            if client
                .call(methods::REPORT_VOICE_MEDIA_V2, wrap(json!(report)))
                .await
                .is_err()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let info: openagents_proto::EngineInfo = client
        .call_as(
            methods::ENGINE_INFO,
            json!({"targetDeviceId":host.device_id}),
        )
        .await
        .unwrap();
    assert_eq!(info.device_id, viewer.device_id); // EngineInfo is deliberately local.
    client
        .call(methods::VOICE_CAPABILITIES_V2, wrap(json!({})))
        .await
        .unwrap();
    assert!(!temp.path().join("codex-package/helper-wire.jsonl").exists());
    drop(relay);
    room.abort();
    host.sessions.shutdown().await;
    viewer.sessions.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn voice_requires_a_saved_agent_and_resolves_the_default_by_ids() {
    use openagents_proto::{foreman::ForemanSettings, voice::remote as wire};
    let temp = tempfile::tempdir().unwrap();
    let (core, account) = native_core(&temp).await;
    account.choice.lock().unwrap().target = None;
    let client = openagents_rpc::memory_client(core.rpc_service());
    let request = wire::Prepare {
        attempt_key: wire::AttemptKey::new(),
        config: serde_json::from_value(json!({"harness":"codex","sandbox":"workspace-write"}))
            .unwrap(),
        voice: None,
    };
    let error = client
        .call(
            methods::PREPARE_VOICE_V2,
            json!({"targetDeviceId":core.device_id,"payload":request}),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("VoiceAgentRequired"), "{error}");
    assert!(
        core.workspace
            .read_chats()
            .unwrap()
            .iter()
            .all(|c| !openagents_proto::voice::is_orchestrator_chat(&c.id))
    );
    core.doc_host
        .set_foreman(ForemanSettings {
            chat_id: Some(account.targets[0].chat_id.clone()),
            ..Default::default()
        })
        .unwrap();
    core.workspace
        .rename_chat(&account.targets[0].chat_id, "A renamed helper")
        .unwrap();
    let resolved = core.doc_host.voice_agent().await.unwrap();
    assert_eq!(resolved.target, Some(account.targets[0].clone()));
    assert!(
        account.choice.lock().unwrap().target.is_none(),
        "Resolving the default must not overwrite the account preference"
    );
    core.sessions.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn voice_relays_owner_messages_context_and_full_answers_through_normal_chats() {
    use std::{sync::Arc, time::Duration};
    use openagents_proto::voice::{VoiceChatContext, remote as wire};
    use openagents_doc::{MessagePart, MessageRole};
    let temp = tempfile::tempdir().unwrap();
    let (core, account) = native_core(&temp).await;
    let client = openagents_rpc::memory_client(core.rpc_service());
    let envelope = |p: serde_json::Value| json!({"targetDeviceId":core.device_id,"payload":p});
    let config =
        serde_json::from_value(json!({"harness":"codex","sandbox":"workspace-write"})).unwrap();
    let prepared: wire::Prepared = client
        .call_as(
            methods::PREPARE_VOICE_V2,
            envelope(json!(wire::Prepare {
                attempt_key: wire::AttemptKey::new(),
                config,
                voice: None
            })),
        )
        .await
        .unwrap();
    let _owner = client
        .subscribe_checked(methods::OWN_VOICE_V2, envelope(json!(prepared.lease)))
        .await
        .unwrap();
    let tools = openagents_mcp::Tools::new(Arc::new(openagents_mcp::OpenAgents::with_client(
        openagents_rpc::memory_client(core.rpc_service()),
        openagents_mcp::Origin {
            chat_id: Some(prepared.chat_id.clone()),
            device_id: Some(core.device_id.clone()),
        },
    )));
    assert_eq!(tools.list().len(), 3);
    let chats_before = core.workspace.read_chats().unwrap().len();
    for name in [
        "create_chat",
        "create_chats",
        "send_messages",
        "interrupt_chat",
        "machine_action",
        "machine_status",
        "upgrade_host",
        "notify_owner",
        "work",
    ] {
        let error = tools.call(name, json!({})).await.unwrap_err();
        assert!(error.contains("Voice can only"), "{name}: {error}");
    }
    assert_eq!(core.workspace.read_chats().unwrap().len(), chats_before);
    client
        .call(
            methods::UPDATE_VOICE_CONTEXT_V2,
            envelope(json!(wire::ContextUpdate {
                lease: prepared.lease.clone(),
                context: VoiceChatContext::new("chat-x".into(), Some("Chat X".into()))
            })),
        )
        .await
        .unwrap();
    let first = tools.call("send_message", json!({"chat":"wrong-chat","text":"First complete spoken question", "mode":"steer","wait":true})).await.unwrap();
    assert_eq!(first["chatId"], account.targets[0].chat_id);
    assert_eq!(first["sent"]["delivery"], "run");
    tokio::time::timeout(Duration::from_secs(3), async {
        while !core.sessions.turn_in_flight(&account.targets[0].chat_id) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        core.workspace
            .chat(&account.targets[0].chat_id)
            .unwrap()
            .unwrap()
            .config
            .unwrap()
            .harness,
        HarnessId::Opencode,
        "The answering fixture must not fall back to the native Codex fake"
    );
    client
        .call(
            methods::UPDATE_VOICE_CONTEXT_V2,
            envelope(json!(wire::ContextUpdate {
                lease: prepared.lease.clone(),
                context: VoiceChatContext::new("chat-y".into(), Some("Chat Y".into()))
            })),
        )
        .await
        .unwrap();
    let busy = tools
        .call(
            "send_message",
            json!({"chat":"wrong-chat","text":"Second question while busy","wait":false}),
        )
        .await
        .unwrap();
    assert_eq!(busy["sent"]["delivery"], "steer");
    account.choice.lock().unwrap().target = Some(account.targets[1].clone());
    let switched = tools.call("send_message", json!({"chat":account.targets[0].chat_id,"text":"Question for the chosen fork","wait":false})).await.unwrap();
    assert_eq!(switched["chatId"], account.targets[1].chat_id);
    assert!(
        tools
            .call(
                "wait_for_turn",
                json!({"chat":"native-voice","timeout_secs":1})
            )
            .await
            .is_err()
    );
    // Several bounded waits cover a long turn, even after the choice changes.
    for _ in 0..2 {
        let waiting = tools
            .call(
                "wait_for_turn",
                json!({"chat":account.targets[0].chat_id,"timeout_secs":1}),
            )
            .await
            .unwrap();
        assert_eq!(waiting["turn"]["outcome"], "timedOut");
    }
    account.release.add_permits(2);
    let answer = tools
        .call(
            "wait_for_turn",
            json!({"chat":account.targets[0].chat_id,"timeout_secs":5}),
        )
        .await
        .unwrap();
    assert_eq!(
        answer["turn"]["outcome"],
        "completed",
        "reply={answer}; session={:?}; in_flight={}",
        core.sessions.session_status(&account.targets[0].chat_id),
        core.sessions.turn_in_flight(&account.targets[0].chat_id),
    );
    assert!(
        answer
            .to_string()
            .contains("The full written answer stays in this chat.")
    );
    let fork = tools
        .call(
            "wait_for_turn",
            json!({"chat":account.targets[1].chat_id,"timeout_secs":5}),
        )
        .await
        .unwrap();
    assert_eq!(fork["turn"]["outcome"], "completed");
    let entries = core
        .doc_host
        .open(&account.targets[0].chat_id)
        .unwrap()
        .doc()
        .read_entries()
        .unwrap();
    let text = |e: &openagents_doc::SessionMessageEntry| {
        e.parts
            .iter()
            .filter_map(|p| match p {
                MessagePart::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    };
    let users: Vec<_> = entries
        .iter()
        .filter(|e| e.role == MessageRole::User)
        .map(&text)
        .collect();
    assert!(users.iter().any(|t| t.starts_with("Spoken:")
        && t.contains("First complete spoken question")
        && t.contains("chat-x")
        && t.contains("Chat X")));
    assert!(users.iter().any(|t| t.starts_with("Spoken:")
        && t.contains("Second question while busy")
        && t.contains("chat-y")
        && t.contains("Chat Y")));
    assert!(
        users
            .iter()
            .all(|t| !t.contains("Message from OpenAgents chat"))
    );
    let written = entries
        .iter()
        .filter(|e| e.role == MessageRole::Assistant)
        .map(&text)
        .collect::<Vec<_>>()
        .join("");
    assert!(
        written.len() > 3000 && written.contains("Second question while busy"),
        "The full answer belongs in the normal agent chat"
    );
    client
        .call(methods::STOP_VOICE_V2, envelope(json!(prepared.lease)))
        .await
        .unwrap();
    assert!(
        client
            .call(
                methods::UPDATE_VOICE_CONTEXT_V2,
                envelope(json!(wire::ContextUpdate {
                    lease: prepared.lease,
                    context: VoiceChatContext::new("late-chat".into(), None)
                }))
            )
            .await
            .is_err()
    );
    assert!(
        client
            .call(
                methods::GET_VOICE_CONTEXT,
                json!({"chatId":prepared.chat_id})
            )
            .await
            .is_err()
    );
    core.sessions.shutdown().await;
}
