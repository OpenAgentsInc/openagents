//! Live decisions use the SDK's final rewritten input and the owner's registry.

use super::*;
use coder::task::pending::Service;
use coder_sdk::{Hook, HookOutcome, Permission};
use route_contract::pending as wire;

pub(super) const ASK_TOOL: &str = "mcp__openagents__ask_user";

type CallKey = (u64, String, Option<String>);

pub(super) fn handler(
    gate: &Arc<Gate>,
    service: &Service,
) -> Arc<dyn claude_agent_sdk::PermissionHandler> {
    let mut registry = coder_sdk::Registry::new();
    let floor_gate = gate.clone();
    let _ = registry.register(Hook::floor(
        "worktree",
        coder_sdk::Event::PreToolUse,
        Permission::deny("Coder could not check this step. Try again."),
        move |call: &coder_sdk::Call| {
            let tool = call.tool.as_deref().unwrap_or_default();
            let request = Request::of(tool, &call.input);
            let state = floor_gate.state();
            Ok(if state.asked.is_some() || state.question.is_some() {
                HookOutcome {
                    stop: coder_sdk::Stop::End,
                    ..HookOutcome::deny(ASKED)
                }
            } else if state.denied.as_ref() == Some(&request) {
                HookOutcome::deny(DENIED)
            } else if state.once.as_ref() == Some(&request) || floor_gate.inside(tool, &call.input)
            {
                HookOutcome::allow()
            } else {
                HookOutcome::ask("May Coder run this step?")
            })
        },
    ));
    let backend = coder_sdk::claude::Claude::new(registry, coder_sdk::Budget::default())
        .expect("native Claude hooks");
    let ask_gate = gate.clone();
    let ask_service = service.clone();
    let factory: coder_sdk::claude::AskerFactory = Arc::new(move |identity| {
        let gate = ask_gate.clone();
        let service = ask_service.clone();
        Arc::new(move |applied| {
            let gate = gate.clone();
            let service = service.clone();
            let identity = identity.clone();
            Box::pin(async move {
                let tool = applied.tool.as_deref().unwrap_or_default();
                let step = step(tool, &applied.input, &gate.base);
                let opened = service.open(
                    &identity.native_call,
                    identity.child.clone(),
                    tool,
                    applied.input.clone(),
                    wire::Prompt {
                        command: step.command.clone(),
                        reason: step.reason.clone(),
                        risk: wire::Risk::High,
                    },
                    wire::Form::Approval,
                    unix_now(),
                );
                let Ok(request) = opened else {
                    return Permission::deny("This request has changed or ended.");
                };
                let answer = service.wait(&request).await;
                let active = service.active();
                let mut state = gate.state();
                match answer {
                    Ok(wire::Answer::AllowOnce) => {
                        state.native.insert(
                            (
                                service.scope().generation,
                                identity.native_call,
                                identity.child,
                            ),
                            Request::of(tool, &applied.input),
                        );
                        Permission::Allow
                    }
                    Ok(wire::Answer::Deny) => {
                        state.denied = Some(Request::of(tool, &applied.input));
                        state
                            .log
                            .push((tool.into(), brief(&applied.input), Decision::Denied));
                        Permission::deny(DENIED)
                    }
                    Ok(_) => Permission::deny("The person cancelled this request."),
                    Err(_) => {
                        // A headless client keeps the established turn-ending approval.
                        if active {
                            state.asked = Some(Pending {
                                step,
                                request: Request::of(tool, &applied.input),
                            });
                            state
                                .log
                                .push((tool.into(), brief(&applied.input), Decision::Asked));
                        }
                        Permission::deny(ASKED)
                    }
                }
            })
        })
    });
    let inner = backend.permission_handler_with_asker(factory);
    let gate = gate.clone();
    let service = service.clone();
    claude_agent_sdk::permission_handler(move |call| {
        let inner = inner.clone();
        let gate = gate.clone();
        let service = service.clone();
        async move {
            let result = inner
                .can_use_tool(
                    &call.tool_name,
                    &call.input,
                    call.suggestions,
                    call.blocked_path,
                    call.decision_reason,
                    &call.tool_use_id,
                    call.agent_id.clone(),
                )
                .await?;
            if let PermissionResult::Allow { updated_input, .. } = &result {
                let input = updated_input;
                let committed = service.with_active(|| {
                    let request = Request::of(&call.tool_name, input);
                    let mut state = gate.state();
                    let key: CallKey =
                        (service.scope().generation, call.tool_use_id, call.agent_id);
                    let decision = if state.native.remove(&key).as_ref() == Some(&request) {
                        Decision::AllowedOnce
                    } else if state.once.as_ref() == Some(&request) {
                        state.once = None;
                        Decision::AllowedOnce
                    } else if gate.inside(&call.tool_name, input) {
                        Decision::Inside
                    } else {
                        return Err(coder::task::Error::InvalidTransition);
                    };
                    state.log.push((call.tool_name, brief(input), decision));
                    Ok(())
                });
                if committed.is_err() {
                    return Ok(PermissionResult::deny("This request has changed or ended."));
                }
            }
            if matches!(result, PermissionResult::Deny { .. })
                && (gate.asked().is_some() || gate.state().question.is_some())
            {
                return Ok(PermissionResult::deny_and_interrupt(ASKED));
            }
            Ok(result)
        }
    })
}

pub(super) fn questions(
    gate: &Arc<Gate>,
    service: &Service,
    workspace: Option<coder::task::workspace::Service>,
) -> claude_agent_sdk::SdkMcpServer {
    let gate = gate.clone();
    let service = service.clone();
    let server = coder_sdk::tools::ToolServer::new("openagents", "1")
        .timeout_ms(125_000)
        .add(coder_sdk::tools::Tool::new("ask_user", "Ask the person one or more questions and wait for their answers. Use stable request and choice IDs. This does not approve tool execution.", json!({
            "type":"object", "required":["request_id","questions"], "additionalProperties":false,
            "properties": {"request_id":{"type":"string","minLength":1,"maxLength":128}, "questions":{"type":"array","minItems":1,"maxItems":8,"items":{
                "type":"object", "required":["id","text"], "additionalProperties":false,
                "properties":{"id":{"type":"string"},"text":{"type":"string"},"multiple":{"type":"boolean"},"free_text":{"type":"boolean"},
                    "choices":{"type":"array","maxItems":9,"items":{"type":"object","required":["id","label"],"properties":{"id":{"type":"string"},"label":{"type":"string"}}}}
                }
            }}}
        }), move |input| {
            let gate = gate.clone(); let service = service.clone();
            async move {
                let request_id = input["request_id"].as_str().filter(|id| !id.is_empty() && id.len() <= 128).ok_or("A request ID is required.")?;
                let form: wire::Questionnaire = serde_json::from_value(json!({"questions":input["questions"]})).map_err(|_| "These questions could not be read.")?;
                if !form.valid() { return Err("These questions need valid choices or a text answer.".into()); }
                let request = service.open(&format!("ask:{request_id}"), None, ASK_TOOL, input.clone(), wire::Prompt {
                    command: String::new(), reason: String::new(), risk: wire::Risk::Low,
                }, wire::Form::Questions { form: form.clone() }, unix_now()).map_err(|_| "This question has changed or ended.")?;
                match service.wait(&request).await {
                    Ok(answer @ wire::Answer::Questions { .. }) => {
                        let answer = serde_json::to_value(answer).map_err(|_| "The answer could not be sent.")?;
                        Ok(claude_agent_sdk::ToolResult::text(answer.to_string()).with_structured_content(answer))
                    }
                    Ok(_) => Ok(claude_agent_sdk::ToolResult::error("The person cancelled these questions.")),
                    Err(_) => {
                        if service.active() { gate.state().question = Some(form); }
                        Ok(claude_agent_sdk::ToolResult::error("The person is not available to answer now."))
                    }
                }
            }
        }));
    if let Some(workspace) = workspace.filter(|_| coder::task::workspace::Service::supported()) {
        let reference = workspace.reference().clone();
        coder_sdk::workspace::tools(server, reference, Arc::new(workspace)).in_process()
    } else {
        server.in_process()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::repository::tests::fixture_with;

    fn route(config: &mut coder::task::adapter::Configuration) {
        config.provider = "claude".into();
        config.model = "claude-opus-5-5".into();
        config.decision_endpoint = "https://decision.example.invalid".into();
        config.generation_endpoint = CLAUDE_SDK_ENDPOINT.into();
        config.access = coder::task::adapter::Access::Full;
    }

    async fn setup() -> (tempfile::TempDir, PathBuf, Host, Service, Arc<Gate>) {
        let (root, store, grant) = fixture_with("claude-opus-5-5", route);
        let host = Host::admit(&store, &grant).await.unwrap();
        let recorder = host.observations();
        let service = host.pending(recorder.scope()).unwrap();
        let gate = Arc::new(Gate::new(host.workspace(), None, None));
        (root, store, host, service, gate)
    }

    fn respond(store: &Path, request: &wire::Request, answer: wire::Answer) -> wire::Receipt {
        coder::task::pending::reply(
            store,
            "fixture",
            &wire::Reply {
                request: request.id.clone(),
                scope: request.scope.clone(),
                input_digest: request.input_digest.clone(),
                answer,
            },
        )
        .unwrap()
    }

    async fn request(store: &Path) -> wire::Request {
        for _ in 0..100 {
            if let Some(snapshot) = coder::task::pending::view(store, "fixture").unwrap()
                && let Some(entry) = snapshot.entries.iter().find(|e| e.resolution.is_none())
            {
                return entry.request.clone();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("no live request");
    }

    #[tokio::test]
    async fn exact_live_approval_is_consumed_once_and_retries_do_not_reapprove() {
        let (_root, store, _host, service, gate) = setup().await;
        let handler = handler(&gate, &service);
        let input = json!({"command":"make check"});
        let call = handler.can_use_tool(
            "Bash",
            &input,
            None,
            None,
            None,
            "tool-1",
            Some("child-1".into()),
        );
        let viewer = async {
            let request = request(&store).await;
            assert_eq!(request.input, input);
            assert_eq!(request.child.as_deref(), Some("child-1"));
            let mut stale = wire::Reply {
                request: request.id.clone(),
                scope: request.scope.clone(),
                input_digest: request.input_digest.clone(),
                answer: wire::Answer::AllowOnce,
            };
            stale.scope.generation += 1;
            assert!(coder::task::pending::reply(&store, "fixture", &stale).is_err());
            assert!(!respond(&store, &request, wire::Answer::AllowOnce).duplicate);
            assert!(respond(&store, &request, wire::Answer::AllowOnce).duplicate);
            request
        };
        let (result, request) = tokio::join!(call, viewer);
        assert!(matches!(result.unwrap(), PermissionResult::Allow { .. }));
        assert!(respond(&store, &request, wire::Answer::AllowOnce).duplicate);
        assert!(
            coder::task::pending::reply(
                &store,
                "fixture",
                &wire::Reply {
                    request: request.id.clone(),
                    scope: request.scope.clone(),
                    input_digest: request.input_digest.clone(),
                    answer: wire::Answer::Deny
                }
            )
            .is_err()
        );
        let snapshot = service.snapshot().unwrap();
        assert_eq!(
            snapshot.entries[0].resolution,
            Some(wire::Resolution::Consumed)
        );
        assert_eq!(gate.drain()[0].2, Decision::AllowedOnce);
        assert!(matches!(
            handler
                .can_use_tool(
                    "Bash",
                    &input,
                    None,
                    None,
                    None,
                    "tool-1",
                    Some("child-1".into())
                )
                .await
                .unwrap(),
            PermissionResult::Deny { .. }
        ));
    }

    #[tokio::test]
    async fn denial_cancel_close_and_changed_input_never_allow() {
        for answer in [wire::Answer::Deny, wire::Answer::Cancel] {
            let (_root, store, _host, service, gate) = setup().await;
            let handler = handler(&gate, &service);
            let input = json!({"command":"make check"});
            let (result, _) = tokio::join!(
                handler.can_use_tool("Bash", &input, None, None, None, "tool-1", None),
                async {
                    let request = request(&store).await;
                    respond(&store, &request, answer);
                }
            );
            assert!(matches!(result.unwrap(), PermissionResult::Deny { .. }));
            assert!(gate.asked().is_none());
        }
        let (_root, store, _host, service, _gate) = setup().await;
        let open = |input| {
            service.open(
                "same",
                None,
                "Bash",
                input,
                wire::Prompt {
                    command: "make check".into(),
                    reason: String::new(),
                    risk: wire::Risk::High,
                },
                wire::Form::Approval,
                unix_now(),
            )
        };
        let first = open(json!({"command":"make check"})).unwrap();
        assert_eq!(open(first.input.clone()).unwrap().id, first.id);
        assert!(open(json!({"command":"deploy"})).is_err());
        assert!(
            coder::task::pending::reply(
                &store,
                "fixture",
                &wire::Reply {
                    request: first.id.clone(),
                    scope: first.scope.clone(),
                    input_digest: first.input_digest.clone(),
                    answer: wire::Answer::AllowOnce
                }
            )
            .is_err()
        );
        service.close().unwrap();
        assert!(!service.snapshot().unwrap().active);
        assert!(open(json!({"command":"make check"})).is_err());
    }

    #[tokio::test]
    async fn lost_checkout_and_missing_viewer_deny_and_preserve_headless_question() {
        let (_root, _store, host, service, gate) = setup().await;
        let handler = handler(&gate, &service);
        let input = json!({"command":"make check"});
        let result = handler
            .can_use_tool("Bash", &input, None, None, None, "headless", None)
            .await
            .unwrap();
        assert!(matches!(result, PermissionResult::Deny { .. }));
        assert!(gate.asked().is_some());
        let moved = host.workspace().with_extension("old");
        std::fs::rename(host.workspace(), &moved).unwrap();
        std::fs::create_dir(host.workspace()).unwrap();
        assert!(!service.active());
        assert!(service.with_active(|| Ok(())).is_err());
    }

    #[tokio::test]
    async fn stop_committed_before_permission_delivery_invalidates_the_answer() {
        let (_root, store, _host, service, _gate) = setup().await;
        let request = service
            .open(
                "stop",
                None,
                "Bash",
                json!({"command":"make check"}),
                wire::Prompt {
                    command: "make check".into(),
                    reason: String::new(),
                    risk: wire::Risk::High,
                },
                wire::Form::Approval,
                unix_now(),
            )
            .unwrap();
        respond(&store, &request, wire::Answer::AllowOnce);
        {
            let mut tasks = coder::task::Store::open(&store).unwrap();
            let task = tasks.show("fixture").unwrap();
            tasks
                .apply(
                    &serde_json::to_vec(&coder::task::Command {
                        schema: coder::task::COMMAND_SCHEMA.into(),
                        command_id: "stop-pending".into(),
                        task_id: "fixture".into(),
                        expected_revision: Some(task.revision),
                        action: coder::task::Action::Cancel {
                            reason: "Stopped in fixture".into(),
                        },
                    })
                    .unwrap(),
                )
                .unwrap();
        }
        assert!(service.wait(&request).await.is_err());
        assert!(service.with_active(|| Ok(())).is_err());
        assert!(
            !coder::task::pending::view(&store, "fixture")
                .unwrap()
                .unwrap()
                .active
        );
    }

    #[tokio::test]
    async fn workspace_tools_use_the_current_owner_and_expire_with_the_attempt() {
        let (_root, store, host, service, gate) = setup().await;
        std::fs::write(host.workspace().join("shared.txt"), "workspace fixture").unwrap();
        let workspace = coder::task::workspace::Service::for_agent(&store, service.clone());
        let server = questions(&gate, &service, Some(workspace));
        let message = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"workspace_read","arguments":{"path":"shared.txt"}}});
        let reply = server.handle_message(&message).await;
        assert_eq!(
            reply["result"]["structuredContent"]["text"],
            "workspace fixture"
        );
        assert!(gate.inside(
            "mcp__openagents__workspace_read",
            &json!({"path":"shared.txt"})
        ));
        assert!(!gate.inside(
            "mcp__openagents__workspace_delete",
            &json!({"path":"shared.txt"})
        ));
        host.pending(host.observations().scope()).unwrap();
        let reply = server.handle_message(&message).await;
        assert_eq!(reply["result"]["isError"], true);
    }

    #[tokio::test]
    async fn typed_question_tool_returns_choice_ids_and_never_permission() {
        let (_root, store, _host, service, gate) = setup().await;
        let server = questions(&gate, &service, None);
        let input = json!({"request_id":"question-1","questions":[{"id":"targets","text":"Which targets?","multiple":true,"free_text":true,"choices":[{"id":"linux","label":"Linux"},{"id":"mac","label":"Mac"}]}]});
        // Exercise the actual in-process MCP dispatch.
        let message = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"ask_user","arguments":input}});
        let call = server.handle_message(&message);
        let viewer = async {
            let request = request(&store).await;
            assert!(matches!(request.form, wire::Form::Questions { .. }));
            respond(
                &store,
                &request,
                wire::Answer::Questions {
                    answers: vec![wire::QuestionAnswer {
                        question: "targets".into(),
                        choices: vec!["linux".into(), "mac".into()],
                        text: Some("Desktop builds".into()),
                    }],
                },
            );
        };
        let (result, _) = tokio::join!(call, viewer);
        let text = serde_json::to_string(&result).unwrap();
        assert!(
            text.contains("linux") && text.contains("Desktop builds"),
            "{text}"
        );
        assert!(gate.state().native.is_empty());
    }
}
