use super::{
    CodexLaneCommand, CodexLaneCommandKind, CodexLaneCommandResponse, CodexLaneCommandStatus,
    CodexLaneConfig, CodexLaneLifecycle, CodexLaneNotification, CodexLaneRuntime, CodexLaneUpdate,
    CodexLaneWorker, CodexThreadTranscriptRole, extract_latest_thread_plan_artifact,
    extract_thread_transcript_messages, normalize_notification,
};

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use codex_client::{
    AppServerChannels, AppServerClient, AppsListParams, CollaborationModeListParams,
    CommandExecParams, ExperimentalFeatureListParams, FuzzyFileSearchSessionStartParams,
    FuzzyFileSearchSessionStopParams, FuzzyFileSearchSessionUpdateParams, ReviewStartParams,
    ReviewTarget, SkillsListExtraRootsForCwd, SkillsListParams, SkillsRemoteWriteParams,
    ThreadListParams, ThreadRealtimeAppendTextParams, ThreadRealtimeStartParams,
    ThreadRealtimeStopParams, TurnSteerParams, UserInput, WindowsSandboxSetupStartParams,
};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct FailingRuntime;

impl CodexLaneRuntime for FailingRuntime {
    fn connect(
        &mut self,
        _runtime: &tokio::runtime::Runtime,
        _config: &CodexLaneConfig,
    ) -> Result<(AppServerClient, AppServerChannels)> {
        Err(anyhow::anyhow!("forced startup failure"))
    }
}

struct SingleClientRuntime {
    connection: Option<(AppServerClient, AppServerChannels)>,
    _runtime_guard: Option<tokio::runtime::Runtime>,
}

impl SingleClientRuntime {
    fn new(
        connection: (AppServerClient, AppServerChannels),
        runtime_guard: tokio::runtime::Runtime,
    ) -> Self {
        Self {
            connection: Some(connection),
            _runtime_guard: Some(runtime_guard),
        }
    }
}

impl CodexLaneRuntime for SingleClientRuntime {
    fn connect(
        &mut self,
        _runtime: &tokio::runtime::Runtime,
        _config: &CodexLaneConfig,
    ) -> Result<(AppServerClient, AppServerChannels)> {
        match self.connection.take() {
            Some(connection) => Ok(connection),
            None => Err(anyhow::anyhow!("mock connection already used")),
        }
    }
}

#[test]
fn startup_failure_reports_error_snapshot() {
    let mut worker =
        CodexLaneWorker::spawn_with_runtime(CodexLaneConfig::default(), Box::new(FailingRuntime));

    let snapshot = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
        snapshot.lifecycle == CodexLaneLifecycle::Error
    });

    assert_eq!(snapshot.lifecycle, CodexLaneLifecycle::Error);
    let has_message = snapshot
        .last_error
        .as_deref()
        .is_some_and(|message| message.contains("forced startup failure"));
    assert!(has_message);

    shutdown_worker(&mut worker);
}

#[test]
fn default_config_opts_out_legacy_codex_event_stream() {
    let config = CodexLaneConfig::default();
    let methods = config.opt_out_notification_methods;
    assert!(
        methods
            .iter()
            .any(|method| method == "codex/event/agent_message_content_delta")
    );
    assert!(
        methods
            .iter()
            .any(|method| method == "codex/event/item_completed")
    );
    assert!(
        methods
            .iter()
            .any(|method| method == "codex/event/task_complete")
    );
}

#[test]
fn startup_bootstrap_transitions_to_ready_snapshot() {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);
    let runtime_guard = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("failed to build runtime"));
    let _entered = runtime_guard.enter();
    let (client, channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
    drop(_entered);

    let saw_model_list = Arc::new(AtomicBool::new(false));
    let saw_model_list_clone = Arc::clone(&saw_model_list);
    let server = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.block_on(async move {
            let mut reader = BufReader::new(server_read);
            let mut request_line = String::new();
            let mut saw_initialize = false;
            let mut saw_thread_start = false;
            let mut saw_model_list_request = false;
            loop {
                request_line.clear();
                let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                let value: Value = match serde_json::from_str(request_line.trim()) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("id").is_none() {
                    continue;
                }
                let method = value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let response = match method {
                    "initialize" => json!({
                        "id": value["id"].clone(),
                        "result": {"userAgent": "test-agent"}
                    }),
                    "thread/start" => json!({
                        "id": value["id"].clone(),
                        "result": {
                            "thread": {"id": "thread-bootstrap"},
                            "model": "gpt-5.3-codex"
                        }
                    }),
                    "model/list" => {
                        saw_model_list_clone.store(true, Ordering::SeqCst);
                        json!({
                            "id": value["id"].clone(),
                            "result": {
                                "data": [
                                    {
                                        "id": "default",
                                        "model": "o4-mini",
                                        "displayName": "o4-mini",
                                        "description": "o4-mini",
                                        "supportedReasoningEfforts": [],
                                        "defaultReasoningEffort": "medium",
                                        "isDefault": true
                                    }
                                ],
                                "nextCursor": null
                            }
                        })
                    }
                    _ => json!({
                        "id": value["id"].clone(),
                        "result": {}
                    }),
                };
                saw_initialize |= method == "initialize";
                saw_thread_start |= method == "thread/start";
                saw_model_list_request |= method == "model/list";
                if let Ok(line) = serde_json::to_string(&response) {
                    let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                    let _ = server_write.flush().await;
                }
                if saw_initialize && saw_thread_start && saw_model_list_request {
                    break;
                }
            }
            drop(server_write);
        });
    });

    let mut worker = CodexLaneWorker::spawn_with_runtime(
        CodexLaneConfig::default(),
        Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
    );

    let mut saw_ready = false;
    let mut active_thread_id = None;
    let mut models_loaded = None;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() <= deadline && (!saw_ready || models_loaded.is_none()) {
        for update in worker.drain_updates() {
            match update {
                CodexLaneUpdate::Snapshot(snapshot) => {
                    if snapshot.lifecycle == CodexLaneLifecycle::Ready {
                        saw_ready = true;
                        active_thread_id = snapshot.active_thread_id.clone();
                    }
                }
                CodexLaneUpdate::Notification(CodexLaneNotification::ModelsLoaded {
                    models,
                    default_model,
                }) => {
                    models_loaded = Some((models, default_model));
                }
                CodexLaneUpdate::Notification(_) | CodexLaneUpdate::CommandResponse(_) => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(saw_ready, "missing ready snapshot");
    assert_eq!(active_thread_id.as_deref(), Some("thread-bootstrap"));
    assert!(saw_model_list.load(Ordering::SeqCst));
    let (models, default_model) = models_loaded.expect("expected models loaded notification");
    assert_eq!(models, vec!["o4-mini".to_string()]);
    assert_eq!(default_model.as_deref(), Some("o4-mini"));

    shutdown_worker(&mut worker);
    join_server(server);
}

#[test]
fn disconnect_transitions_to_disconnected_state() {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);
    let runtime_guard = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("failed to build runtime"));
    let _entered = runtime_guard.enter();
    let (client, channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
    drop(_entered);

    let server = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.block_on(async move {
            let mut reader = BufReader::new(server_read);
            let mut request_line = String::new();
            let mut handled_requests = 0usize;
            loop {
                request_line.clear();
                let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                let value: Value = match serde_json::from_str(request_line.trim()) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("id").is_none() {
                    continue;
                }
                handled_requests = handled_requests.saturating_add(1);
                let method = value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let response = match method {
                    "initialize" => json!({
                        "id": value["id"].clone(),
                        "result": {"userAgent": "test-agent"}
                    }),
                    "thread/start" => json!({
                        "id": value["id"].clone(),
                        "result": {
                            "thread": {"id": "thread-bootstrap"},
                            "model": "gpt-5.3-codex"
                        }
                    }),
                    _ => json!({
                        "id": value["id"].clone(),
                        "result": {}
                    }),
                };
                if let Ok(line) = serde_json::to_string(&response) {
                    let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                }
                let _ = server_write.flush().await;
                if handled_requests >= 2 {
                    break;
                }
            }
            drop(server_write);
        });
    });

    let mut worker = CodexLaneWorker::spawn_with_runtime(
        CodexLaneConfig::default(),
        Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
    );

    let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
        snapshot.lifecycle == CodexLaneLifecycle::Ready
    });

    let enqueue_result =
        worker.enqueue(7, CodexLaneCommand::ThreadList(ThreadListParams::default()));
    assert!(enqueue_result.is_ok());

    let disconnected = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
        snapshot.lifecycle == CodexLaneLifecycle::Disconnected
            || snapshot.lifecycle == CodexLaneLifecycle::Error
    });
    assert!(
        matches!(
            disconnected.lifecycle,
            CodexLaneLifecycle::Disconnected | CodexLaneLifecycle::Error
        ),
        "expected disconnected/error lifecycle, got {:?}",
        disconnected.lifecycle
    );

    shutdown_worker(&mut worker);
    join_server(server);
}

#[test]
fn turn_lifecycle_notifications_are_forwarded() {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);
    let runtime_guard = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("failed to build runtime"));
    let _entered = runtime_guard.enter();
    let (client, channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
    drop(_entered);

    let server = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.block_on(async move {
            let mut reader = BufReader::new(server_read);
            let mut request_line = String::new();
            let mut bootstrapped = false;
            loop {
                request_line.clear();
                let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                let value: Value = match serde_json::from_str(request_line.trim()) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("id").is_none() {
                    continue;
                }

                let method = value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let response = match method {
                    "initialize" => json!({
                        "id": value["id"].clone(),
                        "result": {"userAgent": "test-agent"}
                    }),
                    "thread/start" => {
                        bootstrapped = true;
                        json!({
                            "id": value["id"].clone(),
                            "result": {
                                "thread": {"id": "thread-bootstrap"},
                                "model": "gpt-5.3-codex"
                            }
                        })
                    }
                    _ => json!({
                        "id": value["id"].clone(),
                        "result": {}
                    }),
                };

                if let Ok(line) = serde_json::to_string(&response) {
                    let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                    let _ = server_write.flush().await;
                }

                if bootstrapped {
                    let notifications = [
                        json!({
                            "jsonrpc": "2.0",
                            "method": "turn/started",
                            "params": {
                                "threadId": "thread-bootstrap",
                                "turn": {"id": "turn-1"}
                            }
                        }),
                        json!({
                            "jsonrpc": "2.0",
                            "method": "item/agentMessage/delta",
                            "params": {
                                "threadId": "thread-bootstrap",
                                "turnId": "turn-1",
                                "itemId": "item-1",
                                "delta": "hello world"
                            }
                        }),
                        json!({
                            "jsonrpc": "2.0",
                            "method": "turn/completed",
                            "params": {
                                "threadId": "thread-bootstrap",
                                "turn": {"id": "turn-1"}
                            }
                        }),
                        json!({
                            "jsonrpc": "2.0",
                            "method": "error",
                            "params": {
                                "threadId": "thread-bootstrap",
                                "turnId": "turn-1",
                                "willRetry": false,
                                "error": {"message": "boom"}
                            }
                        }),
                    ];
                    for notification in notifications {
                        if let Ok(line) = serde_json::to_string(&notification) {
                            let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                            let _ = server_write.flush().await;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    break;
                }
            }
            drop(server_write);
        });
    });

    let mut worker = CodexLaneWorker::spawn_with_runtime(
        CodexLaneConfig::default(),
        Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
    );

    let mut saw_ready = false;
    let mut saw_turn_started = false;
    let mut saw_delta = false;
    let mut saw_turn_completed = false;
    let mut saw_turn_error = false;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() <= deadline
        && !(saw_ready && saw_turn_started && saw_delta && saw_turn_completed && saw_turn_error)
    {
        for update in worker.drain_updates() {
            match update {
                CodexLaneUpdate::Snapshot(snapshot) => {
                    if snapshot.lifecycle == CodexLaneLifecycle::Ready {
                        saw_ready = true;
                    }
                }
                CodexLaneUpdate::Notification(notification) => match notification {
                    CodexLaneNotification::TurnStarted { thread_id, turn_id } => {
                        if thread_id == "thread-bootstrap" && turn_id == "turn-1" {
                            saw_turn_started = true;
                        }
                    }
                    CodexLaneNotification::AgentMessageDelta {
                        thread_id,
                        turn_id,
                        item_id,
                        delta,
                    } => {
                        if thread_id == "thread-bootstrap"
                            && turn_id == "turn-1"
                            && item_id == "item-1"
                            && delta == "hello world"
                        {
                            saw_delta = true;
                        }
                    }
                    CodexLaneNotification::TurnCompleted {
                        thread_id, turn_id, ..
                    } => {
                        if thread_id == "thread-bootstrap" && turn_id == "turn-1" {
                            saw_turn_completed = true;
                        }
                    }
                    CodexLaneNotification::TurnError {
                        thread_id,
                        turn_id,
                        message,
                    } => {
                        if thread_id == "thread-bootstrap"
                            && turn_id == "turn-1"
                            && message == "boom"
                        {
                            saw_turn_error = true;
                        }
                    }
                    _ => {}
                },
                CodexLaneUpdate::CommandResponse(_) => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(saw_ready, "missing ready snapshot");
    assert!(saw_turn_started, "missing turn/started notification");
    assert!(saw_delta, "missing item/agentMessage/delta notification");
    assert!(saw_turn_completed, "missing turn/completed notification");
    assert!(saw_turn_error, "missing error notification");

    shutdown_worker(&mut worker);
    join_server(server);
}

#[test]
fn delayed_notifications_forward_without_followup_command() {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);
    let runtime_guard = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("failed to build runtime"));
    let _entered = runtime_guard.enter();
    let (client, channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
    drop(_entered);

    let server = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.block_on(async move {
            let mut reader = BufReader::new(server_read);
            let mut request_line = String::new();
            let mut saw_thread_loaded_list = false;
            loop {
                request_line.clear();
                let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                let value: Value = match serde_json::from_str(request_line.trim()) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("id").is_none() {
                    continue;
                }

                let method = value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let response = match method {
                    "initialize" => json!({
                        "id": value["id"].clone(),
                        "result": {"userAgent": "test-agent"}
                    }),
                    "model/list" => json!({
                        "id": value["id"].clone(),
                        "result": {"data": [], "nextCursor": null}
                    }),
                    "thread/list" => json!({
                        "id": value["id"].clone(),
                        "result": {"data": [], "nextCursor": null}
                    }),
                    "thread/loaded/list" => {
                        saw_thread_loaded_list = true;
                        json!({
                            "id": value["id"].clone(),
                            "result": {"data": [], "nextCursor": null}
                        })
                    }
                    _ => json!({
                        "id": value["id"].clone(),
                        "result": {}
                    }),
                };

                if let Ok(line) = serde_json::to_string(&response) {
                    let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                    let _ = server_write.flush().await;
                }

                if saw_thread_loaded_list {
                    // Send a delayed notification while the lane is idle (no new commands).
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    let notification = json!({
                        "jsonrpc": "2.0",
                        "method": "item/agentMessage/delta",
                        "params": {
                            "threadId": "thread-delayed",
                            "turnId": "turn-delayed",
                            "itemId": "item-delayed",
                            "delta": "hello from delayed stream"
                        }
                    });
                    if let Ok(line) = serde_json::to_string(&notification) {
                        let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                        let _ = server_write.flush().await;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    break;
                }
            }
            drop(server_write);
        });
    });

    let config = CodexLaneConfig {
        bootstrap_thread: false,
        ..CodexLaneConfig::default()
    };
    let mut worker = CodexLaneWorker::spawn_with_runtime(
        config,
        Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
    );

    let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
        snapshot.lifecycle == CodexLaneLifecycle::Ready
    });

    let mut saw_delta = false;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() <= deadline && !saw_delta {
        for update in worker.drain_updates() {
            if let CodexLaneUpdate::Notification(CodexLaneNotification::AgentMessageDelta {
                thread_id,
                turn_id,
                item_id,
                delta,
            }) = update
                && thread_id == "thread-delayed"
                && turn_id == "turn-delayed"
                && item_id == "item-delayed"
                && delta == "hello from delayed stream"
            {
                saw_delta = true;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(saw_delta, "missing delayed notification while lane idled");

    shutdown_worker(&mut worker);
    join_server(server);
}

#[test]
fn thread_lifecycle_notifications_are_normalized() {
    let started = normalize_notification(codex_client::AppServerNotification {
        method: "thread/started".to_string(),
        params: Some(json!({
            "threadId": "thread-0"
        })),
    });
    assert_eq!(
        started,
        Some(CodexLaneNotification::ThreadStatusChanged {
            thread_id: "thread-0".to_string(),
            status: "active".to_string(),
        })
    );

    let status = normalize_notification(codex_client::AppServerNotification {
        method: "thread/status/changed".to_string(),
        params: Some(json!({
            "threadId": "thread-1",
            "status": {"type": "active", "activeFlags": ["waitingOnApproval"]}
        })),
    });
    assert_eq!(
        status,
        Some(CodexLaneNotification::ThreadStatusChanged {
            thread_id: "thread-1".to_string(),
            status: "active:waitingOnApproval".to_string(),
        })
    );

    let archived = normalize_notification(codex_client::AppServerNotification {
        method: "thread/archived".to_string(),
        params: Some(json!({
            "threadId": "thread-2"
        })),
    });
    assert_eq!(
        archived,
        Some(CodexLaneNotification::ThreadArchived {
            thread_id: "thread-2".to_string(),
        })
    );

    let renamed = normalize_notification(codex_client::AppServerNotification {
        method: "thread/name/updated".to_string(),
        params: Some(json!({
            "threadId": "thread-3",
            "threadName": "Renamed Thread"
        })),
    });
    assert_eq!(
        renamed,
        Some(CodexLaneNotification::ThreadNameUpdated {
            thread_id: "thread-3".to_string(),
            thread_name: Some("Renamed Thread".to_string()),
        })
    );

    let oauth_completed = normalize_notification(codex_client::AppServerNotification {
        method: "mcpServer/oauthLogin/completed".to_string(),
        params: Some(json!({
            "name": "github",
            "success": true,
            "error": null
        })),
    });
    assert_eq!(
        oauth_completed,
        Some(CodexLaneNotification::McpServerOauthLoginCompleted {
            server_name: "github".to_string(),
            success: true,
            error: None,
        })
    );

    let app_list_updated = normalize_notification(codex_client::AppServerNotification {
        method: "app/list/updated".to_string(),
        params: None,
    });
    assert_eq!(
        app_list_updated,
        Some(CodexLaneNotification::AppsListUpdated)
    );

    let fuzzy_updated = normalize_notification(codex_client::AppServerNotification {
        method: "fuzzyFileSearch/sessionUpdated".to_string(),
        params: Some(json!({
            "sessionId": "session-1",
            "status": "indexing"
        })),
    });
    assert_eq!(
        fuzzy_updated,
        Some(CodexLaneNotification::FuzzySessionUpdated {
            session_id: "session-1".to_string(),
            status: "indexing".to_string(),
        })
    );

    let fuzzy_completed = normalize_notification(codex_client::AppServerNotification {
        method: "fuzzyFileSearch/sessionCompleted".to_string(),
        params: Some(json!({
            "sessionId": "session-1"
        })),
    });
    assert_eq!(
        fuzzy_completed,
        Some(CodexLaneNotification::FuzzySessionCompleted {
            session_id: "session-1".to_string(),
        })
    );

    let realtime_started = normalize_notification(codex_client::AppServerNotification {
        method: "thread/realtime/started".to_string(),
        params: Some(json!({
            "threadId": "thread-rt",
            "sessionId": "rt-session"
        })),
    });
    assert_eq!(
        realtime_started,
        Some(CodexLaneNotification::RealtimeStarted {
            thread_id: "thread-rt".to_string(),
            session_id: Some("rt-session".to_string()),
        })
    );

    let windows_setup = normalize_notification(codex_client::AppServerNotification {
        method: "windowsSandbox/setupCompleted".to_string(),
        params: Some(json!({
            "mode": "enable",
            "success": true
        })),
    });
    assert_eq!(
        windows_setup,
        Some(CodexLaneNotification::WindowsSandboxSetupCompleted {
            mode: Some("enable".to_string()),
            success: Some(true),
        })
    );
}

#[test]
fn agent_message_notifications_are_normalized() {
    let delta = normalize_notification(codex_client::AppServerNotification {
        method: "codex/event/agent_message_content_delta".to_string(),
        params: Some(json!({
            "conversationId": "thread-1",
            "id": "turn-1",
            "msg": {
                "item_id": "item-1",
                "delta": "hello"
            }
        })),
    });
    assert_eq!(
        delta,
        Some(CodexLaneNotification::AgentMessageDelta {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            delta: "hello".to_string(),
        })
    );

    let delta_from_nested_ids = normalize_notification(codex_client::AppServerNotification {
        method: "codex/event/agent_message_content_delta".to_string(),
        params: Some(json!({
            "msg": {
                "conversation_id": "thread-1",
                "turn_id": "turn-1",
                "item_id": "item-1b",
                "delta": "nested"
            }
        })),
    });
    assert_eq!(
        delta_from_nested_ids,
        Some(CodexLaneNotification::AgentMessageDelta {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1b".to_string(),
            delta: "nested".to_string(),
        })
    );

    let completed = normalize_notification(codex_client::AppServerNotification {
        method: "codex/event/agent_message".to_string(),
        params: Some(json!({
            "conversationId": "thread-1",
            "id": "turn-1",
            "msg": {
                "item_id": "item-1",
                "message": "done"
            }
        })),
    });
    assert_eq!(
        completed,
        Some(CodexLaneNotification::AgentMessageCompleted {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: Some("item-1".to_string()),
            message: "done".to_string(),
        })
    );

    let item_completed = normalize_notification(codex_client::AppServerNotification {
        method: "item/completed".to_string(),
        params: Some(json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "item": {
                "id": "item-2",
                "type": "agentMessage",
                "text": "final"
            }
        })),
    });
    assert_eq!(
        item_completed,
        Some(CodexLaneNotification::ItemCompleted {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: Some("item-2".to_string()),
            item_type: Some("agentMessage".to_string()),
            message: Some("final".to_string()),
        })
    );

    let legacy_item_completed = normalize_notification(codex_client::AppServerNotification {
        method: "codex/event/item_completed".to_string(),
        params: Some(json!({
            "conversationId": "thread-1",
            "id": "turn-1",
            "msg": {
                "item": {
                    "id": "item-legacy",
                    "type": "agentMessage",
                    "text": "legacy final"
                }
            }
        })),
    });
    assert_eq!(
        legacy_item_completed,
        Some(CodexLaneNotification::ItemCompleted {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: Some("item-legacy".to_string()),
            item_type: Some("agentMessage".to_string()),
            message: Some("legacy final".to_string()),
        })
    );

    let task_complete = normalize_notification(codex_client::AppServerNotification {
        method: "codex/event/task_complete".to_string(),
        params: Some(json!({
            "conversationId": "thread-1",
            "id": "turn-1",
            "msg": {
                "last_agent_message": "task done"
            }
        })),
    });
    assert_eq!(
        task_complete,
        Some(CodexLaneNotification::TurnCompleted {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            status: Some("completed".to_string()),
            error_message: None,
            final_message: Some("task done".to_string()),
        })
    );

    let task_complete_with_nested_turn =
        normalize_notification(codex_client::AppServerNotification {
            method: "codex/event/task_complete".to_string(),
            params: Some(json!({
                "conversationId": "thread-1",
                "id": "",
                "msg": {
                    "turn_id": "turn-1",
                    "last_agent_message": "task done nested"
                }
            })),
        });
    assert_eq!(
        task_complete_with_nested_turn,
        Some(CodexLaneNotification::TurnCompleted {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            status: Some("completed".to_string()),
            error_message: None,
            final_message: Some("task done nested".to_string()),
        })
    );

    let legacy_agent_message = normalize_notification(codex_client::AppServerNotification {
        method: "agent_message".to_string(),
        params: Some(json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "message": "legacy done"
        })),
    });
    assert_eq!(
        legacy_agent_message,
        Some(CodexLaneNotification::AgentMessageCompleted {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: None,
            message: "legacy done".to_string(),
        })
    );

    let legacy_task_complete = normalize_notification(codex_client::AppServerNotification {
        method: "task_complete".to_string(),
        params: Some(json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "message": "legacy task done"
        })),
    });
    assert_eq!(
        legacy_task_complete,
        Some(CodexLaneNotification::TurnCompleted {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            status: Some("completed".to_string()),
            error_message: None,
            final_message: Some("legacy task done".to_string()),
        })
    );

    let reasoning_delta = normalize_notification(codex_client::AppServerNotification {
        method: "agent_reasoning_delta".to_string(),
        params: Some(json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "delta": "thinking"
        })),
    });
    assert_eq!(
        reasoning_delta,
        Some(CodexLaneNotification::ReasoningDelta {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: None,
            delta: "thinking".to_string(),
        })
    );

    let reasoning_content_delta = normalize_notification(codex_client::AppServerNotification {
        method: "codex/event/reasoning_content_delta".to_string(),
        params: Some(json!({
            "conversationId": "thread-1",
            "id": "turn-1",
            "msg": {
                "item_id": "reasoning-1",
                "delta": "plan"
            }
        })),
    });
    assert_eq!(
        reasoning_content_delta,
        Some(CodexLaneNotification::ReasoningDelta {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: Some("reasoning-1".to_string()),
            delta: "plan".to_string(),
        })
    );

    let reasoning_raw_content_delta = normalize_notification(codex_client::AppServerNotification {
        method: "codex/event/agent_reasoning_raw_content_delta".to_string(),
        params: Some(json!({
            "conversation_id": "thread-1",
            "turn_id": "turn-1",
            "msg": {
                "item_id": "reasoning-2",
                "delta": "raw thought"
            }
        })),
    });
    assert_eq!(
        reasoning_raw_content_delta,
        Some(CodexLaneNotification::ReasoningDelta {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: Some("reasoning-2".to_string()),
            delta: "raw thought".to_string(),
        })
    );

    let reasoning_completed = normalize_notification(codex_client::AppServerNotification {
        method: "codex/event/agent_reasoning".to_string(),
        params: Some(json!({
            "conversationId": "thread-1",
            "id": "turn-1",
            "msg": {
                "item_id": "reasoning-3",
                "text": "final rationale"
            }
        })),
    });
    assert_eq!(
        reasoning_completed,
        Some(CodexLaneNotification::ReasoningDelta {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: Some("reasoning-3".to_string()),
            delta: "final rationale".to_string(),
        })
    );
}

#[test]
fn thread_read_parser_handles_camel_case_items() {
    let thread = codex_client::ThreadSnapshot {
        id: "thread-1".to_string(),
        preview: "preview".to_string(),
        turns: vec![codex_client::ThreadTurn {
            id: "turn-1".to_string(),
            items: vec![
                json!({
                    "type": "userMessage",
                    "content": [
                        {"type": "text", "text": "hello from user"}
                    ]
                }),
                json!({
                    "type": "agentMessage",
                    "text": "hello from codex"
                }),
            ],
        }],
    };

    let messages = extract_thread_transcript_messages(&thread);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, CodexThreadTranscriptRole::User);
    assert_eq!(messages[0].content, "hello from user");
    assert_eq!(messages[1].role, CodexThreadTranscriptRole::Codex);
    assert_eq!(messages[1].content, "hello from codex");
}

#[test]
fn pre_materialization_thread_read_errors_are_benign() {
    assert!(super::is_pre_materialization_thread_read_error(
        "App-server error -32600: thread 019cd4a8-680d-7b32-970d-3bdaae6d5d12 is not materialized yet; includeTurns is unavailable before first user message"
    ));
    assert!(!super::is_pre_materialization_thread_read_error(
        "App-server error -32600: thread/read failed for another reason"
    ));
}

#[test]
fn thread_read_extracts_latest_plan_artifact() {
    let thread = codex_client::ThreadSnapshot {
        id: "thread-1".to_string(),
        preview: String::new(),
        turns: vec![
            codex_client::ThreadTurn {
                id: "turn-older".to_string(),
                items: vec![json!({
                    "type": "plan",
                    "text": "older plan"
                })],
            },
            codex_client::ThreadTurn {
                id: "turn-latest".to_string(),
                items: vec![
                    json!({
                        "type": "agentMessage",
                        "text": "done"
                    }),
                    json!({
                        "type": "plan",
                        "text": "latest plan"
                    }),
                ],
            },
        ],
    };

    let artifact = extract_latest_thread_plan_artifact(&thread).expect("plan artifact");
    assert_eq!(artifact.turn_id, "turn-latest");
    assert_eq!(artifact.text, "latest plan");
}

#[test]
fn apps_and_remote_skill_export_emit_notifications() {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);
    let runtime_guard = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("failed to build runtime"));
    let _entered = runtime_guard.enter();
    let (client, channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
    drop(_entered);

    let saw_app_list = Arc::new(AtomicBool::new(false));
    let saw_export = Arc::new(AtomicBool::new(false));
    let saw_app_list_clone = Arc::clone(&saw_app_list);
    let saw_export_clone = Arc::clone(&saw_export);
    let server = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.block_on(async move {
            let mut reader = BufReader::new(server_read);
            let mut request_line = String::new();
            let mut handled_app = false;
            let mut handled_export = false;
            loop {
                request_line.clear();
                let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                let value: Value = match serde_json::from_str(request_line.trim()) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("id").is_none() {
                    continue;
                }
                let method = value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let response = match method {
                    "initialize" => json!({
                        "id": value["id"].clone(),
                        "result": {"userAgent": "test-agent"}
                    }),
                    "thread/start" => json!({
                        "id": value["id"].clone(),
                        "result": {
                            "thread": {"id": "thread-bootstrap"},
                            "model": "gpt-5.3-codex"
                        }
                    }),
                    "app/list" => {
                        handled_app = true;
                        saw_app_list_clone.store(true, Ordering::SeqCst);
                        json!({
                            "id": value["id"].clone(),
                            "result": {
                                "data": [
                                    {
                                        "id": "github",
                                        "name": "GitHub",
                                        "description": "Code hosting",
                                        "isAccessible": true,
                                        "isEnabled": true
                                    }
                                ],
                                "nextCursor": null
                            }
                        })
                    }
                    "skills/remote/export" => {
                        handled_export = true;
                        saw_export_clone.store(true, Ordering::SeqCst);
                        json!({
                            "id": value["id"].clone(),
                            "result": {
                                "id": "skill-1",
                                "path": "/tmp/skill-1"
                            }
                        })
                    }
                    _ => json!({
                        "id": value["id"].clone(),
                        "result": {}
                    }),
                };
                if let Ok(line) = serde_json::to_string(&response) {
                    let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                    let _ = server_write.flush().await;
                }
                if handled_app && handled_export {
                    break;
                }
            }
            drop(server_write);
        });
    });

    let mut worker = CodexLaneWorker::spawn_with_runtime(
        CodexLaneConfig::default(),
        Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
    );

    let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
        snapshot.lifecycle == CodexLaneLifecycle::Ready
    });

    let app_enqueue = worker.enqueue(901, CodexLaneCommand::AppsList(AppsListParams::default()));
    assert!(app_enqueue.is_ok(), "failed to enqueue app/list");
    let app_response = wait_for_command_response(&mut worker, Duration::from_secs(2), |resp| {
        resp.command_seq == 901
    });

    let export_enqueue = worker.enqueue(
        902,
        CodexLaneCommand::SkillsRemoteExport(SkillsRemoteWriteParams {
            hazelnut_id: "skill-1".to_string(),
        }),
    );
    assert!(
        export_enqueue.is_ok(),
        "failed to enqueue skills/remote/export"
    );
    let export_response = wait_for_command_response(&mut worker, Duration::from_secs(2), |resp| {
        resp.command_seq == 902
    });
    assert_eq!(app_response.command, CodexLaneCommandKind::AppsList);
    assert_eq!(app_response.status, CodexLaneCommandStatus::Accepted);
    assert_eq!(
        export_response.command,
        CodexLaneCommandKind::SkillsRemoteExport
    );
    assert_eq!(export_response.status, CodexLaneCommandStatus::Accepted);

    assert!(saw_app_list.load(Ordering::SeqCst));
    assert!(saw_export.load(Ordering::SeqCst));

    shutdown_worker(&mut worker);
    join_server(server);
}

#[test]
fn server_request_command_approval_round_trip() {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, _server_write) = tokio::io::split(server_stream);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("failed to build runtime"));
    let _entered = runtime.enter();
    let (client, _channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
    drop(_entered);

    let mut state = super::CodexLaneState::new();
    state.client = Some(client);
    let (update_tx, update_rx) = std::sync::mpsc::channel::<CodexLaneUpdate>();

    let request = codex_client::AppServerRequest {
        id: codex_client::AppServerRequestId::String("approve-1".to_string()),
        method: "item/commandExecution/requestApproval".to_string(),
        params: Some(json!({
            "threadId": "thread-bootstrap",
            "turnId": "turn-1",
            "itemId": "item-1",
            "reason": "needs approval",
            "command": "ls"
        })),
    };
    state.handle_server_request(&runtime, request, &update_tx);

    let mut approval_request_id = None;
    let mut saw_server_request = false;
    while let Ok(update) = update_rx.try_recv() {
        if let CodexLaneUpdate::Notification(CodexLaneNotification::CommandApprovalRequested {
            request_id,
            ..
        }) = &update
        {
            approval_request_id = Some(request_id.clone());
        }
        if let CodexLaneUpdate::Notification(CodexLaneNotification::ServerRequest { method }) =
            &update
            && method == "item/commandExecution/requestApproval"
        {
            saw_server_request = true;
        }
    }
    assert!(saw_server_request, "expected server request notification");

    let Some(request_id) = approval_request_id else {
        panic!("expected command approval request");
    };
    state.handle_command(
        &runtime,
        super::SequencedCodexCommand {
            command_seq: 991,
            command: CodexLaneCommand::ServerRequestCommandApprovalRespond {
                request_id,
                response: codex_client::CommandExecutionRequestApprovalResponse {
                    decision: codex_client::ApprovalDecision::Accept,
                },
            },
        },
        &update_tx,
    );

    let mut saw_accept_response = false;
    while let Ok(update) = update_rx.try_recv() {
        if let CodexLaneUpdate::CommandResponse(response) = update
            && response.command_seq == 991
        {
            assert_eq!(
                response.command,
                CodexLaneCommandKind::ServerRequestCommandApprovalRespond
            );
            assert_eq!(response.status, CodexLaneCommandStatus::Accepted);
            saw_accept_response = true;
        }
    }
    assert!(
        saw_accept_response,
        "expected accepted command response for approval respond"
    );

    let mut server_reader = BufReader::new(server_read);
    let mut response_line = String::new();
    let read_bytes = runtime
        .block_on(async { server_reader.read_line(&mut response_line).await })
        .unwrap_or(0);
    assert!(
        read_bytes > 0,
        "expected approval response write to transport"
    );
    let response: Value = serde_json::from_str(response_line.trim())
        .unwrap_or_else(|_| panic!("expected valid approval response json"));
    assert_eq!(response.get("id"), Some(&json!("approve-1")));
    assert_eq!(response.pointer("/result/decision"), Some(&json!("accept")));
}

#[test]
fn command_routing_sends_thread_list_request() {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);
    let runtime_guard = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("failed to build runtime"));
    let _entered = runtime_guard.enter();
    let (client, channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
    drop(_entered);

    let saw_thread_list = Arc::new(AtomicBool::new(false));
    let saw_thread_list_clone = Arc::clone(&saw_thread_list);
    let server = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.block_on(async move {
            let mut reader = BufReader::new(server_read);
            let mut request_line = String::new();
            let mut thread_list_count = 0usize;
            loop {
                request_line.clear();
                let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                let value: Value = match serde_json::from_str(request_line.trim()) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("id").is_none() {
                    continue;
                }

                let method = value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let response = match method {
                    "initialize" => json!({
                        "id": value["id"].clone(),
                        "result": {"userAgent": "test-agent"}
                    }),
                    "thread/start" => json!({
                        "id": value["id"].clone(),
                        "result": {
                            "thread": {"id": "thread-bootstrap"},
                            "model": "gpt-5.3-codex"
                        }
                    }),
                    "thread/list" => {
                        thread_list_count = thread_list_count.saturating_add(1);
                        saw_thread_list_clone.store(true, Ordering::SeqCst);
                        json!({
                            "id": value["id"].clone(),
                            "result": {"data": [], "nextCursor": null}
                        })
                    }
                    _ => json!({
                        "id": value["id"].clone(),
                        "result": {}
                    }),
                };

                if let Ok(line) = serde_json::to_string(&response) {
                    let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                    let _ = server_write.flush().await;
                }
                if thread_list_count >= 2 {
                    break;
                }
            }
            drop(server_write);
        });
    });

    let mut worker = CodexLaneWorker::spawn_with_runtime(
        CodexLaneConfig::default(),
        Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
    );

    let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
        snapshot.lifecycle == CodexLaneLifecycle::Ready
    });

    let enqueue_result = worker.enqueue(
        42,
        CodexLaneCommand::ThreadList(ThreadListParams::default()),
    );
    assert!(enqueue_result.is_ok());

    let response = wait_for_command_response(&mut worker, Duration::from_secs(2), |response| {
        response.command_seq == 42
    });
    assert_eq!(response.command, CodexLaneCommandKind::ThreadList);
    assert_eq!(response.status, CodexLaneCommandStatus::Accepted);
    assert!(saw_thread_list.load(Ordering::SeqCst));

    shutdown_worker(&mut worker);
    join_server(server);
}

#[test]
fn command_routing_sends_turn_steer_request() {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);
    let runtime_guard = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("failed to build runtime"));
    let _entered = runtime_guard.enter();
    let (client, channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
    drop(_entered);

    let saw_turn_steer = Arc::new(AtomicBool::new(false));
    let saw_turn_steer_clone = Arc::clone(&saw_turn_steer);
    let server = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.block_on(async move {
            let mut reader = BufReader::new(server_read);
            let mut request_line = String::new();
            loop {
                request_line.clear();
                let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                let value: Value = match serde_json::from_str(request_line.trim()) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("id").is_none() {
                    continue;
                }
                let method = value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let response = match method {
                    "initialize" => json!({
                        "id": value["id"].clone(),
                        "result": {"userAgent": "test-agent"}
                    }),
                    "thread/start" => json!({
                        "id": value["id"].clone(),
                        "result": {
                            "thread": {"id": "thread-bootstrap"},
                            "model": "gpt-5.3-codex"
                        }
                    }),
                    "turn/steer" => {
                        saw_turn_steer_clone.store(true, Ordering::SeqCst);
                        assert_eq!(
                            value["params"],
                            json!({
                                "threadId": "thread-bootstrap",
                                "expectedTurnId": "turn-active",
                                "input": [
                                    {
                                        "type": "text",
                                        "text": "continue"
                                    }
                                ]
                            })
                        );
                        json!({
                            "id": value["id"].clone(),
                            "result": {"turnId": "turn-active"}
                        })
                    }
                    _ => json!({
                        "id": value["id"].clone(),
                        "result": {}
                    }),
                };

                if let Ok(line) = serde_json::to_string(&response) {
                    let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                    let _ = server_write.flush().await;
                }
                if saw_turn_steer_clone.load(Ordering::SeqCst) {
                    break;
                }
            }
            drop(server_write);
        });
    });

    let mut worker = CodexLaneWorker::spawn_with_runtime(
        CodexLaneConfig::default(),
        Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
    );

    let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
        snapshot.lifecycle == CodexLaneLifecycle::Ready
    });

    let enqueue_result = worker.enqueue(
        43,
        CodexLaneCommand::TurnSteer(TurnSteerParams {
            thread_id: "thread-bootstrap".to_string(),
            expected_turn_id: "turn-active".to_string(),
            input: vec![UserInput::Text {
                text: "continue".to_string(),
                text_elements: Vec::new(),
            }],
        }),
    );
    assert!(enqueue_result.is_ok());

    let response = wait_for_command_response(&mut worker, Duration::from_secs(2), |response| {
        response.command_seq == 43
    });
    assert_eq!(response.command, CodexLaneCommandKind::TurnSteer);
    assert_eq!(response.status, CodexLaneCommandStatus::Accepted);
    assert!(saw_turn_steer.load(Ordering::SeqCst));

    shutdown_worker(&mut worker);
    join_server(server);
}

#[test]
fn command_routing_sends_skills_list_request_with_extra_roots() {
    let fixture_root = unique_fixture_root("skills-list");
    let fixture_skills_root = fixture_root.join("skills");
    assert!(fs::create_dir_all(&fixture_skills_root).is_ok());

    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);
    let runtime_guard = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("failed to build runtime"));
    let _entered = runtime_guard.enter();
    let (client, channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
    drop(_entered);

    let saw_skills_list = Arc::new(AtomicBool::new(false));
    let saw_skills_list_clone = Arc::clone(&saw_skills_list);
    let expected_cwd = fixture_root.display().to_string();
    let expected_root = fixture_skills_root.display().to_string();
    let server = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.block_on(async move {
            let mut reader = BufReader::new(server_read);
            let mut request_line = String::new();
            let mut done = false;
            loop {
                request_line.clear();
                let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                let value: Value = match serde_json::from_str(request_line.trim()) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("id").is_none() {
                    continue;
                }

                let method = value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let response = match method {
                    "initialize" => json!({
                        "id": value["id"].clone(),
                        "result": {"userAgent": "test-agent"}
                    }),
                    "thread/start" => json!({
                        "id": value["id"].clone(),
                        "result": {
                            "thread": {"id": "thread-bootstrap"},
                            "model": "gpt-5.3-codex"
                        }
                    }),
                    "skills/list" => {
                        saw_skills_list_clone.store(true, Ordering::SeqCst);
                        done = true;
                        assert_eq!(
                            value["params"],
                            json!({
                                "cwds": [expected_cwd],
                                "forceReload": true,
                                "perCwdExtraUserRoots": [
                                    {
                                        "cwd": expected_cwd,
                                        "extraUserRoots": [expected_root]
                                    }
                                ]
                            })
                        );
                        json!({
                            "id": value["id"].clone(),
                            "result": {
                                "data": [
                                    {
                                        "cwd": expected_cwd,
                                        "skills": [],
                                        "errors": []
                                    }
                                ]
                            }
                        })
                    }
                    _ => json!({
                        "id": value["id"].clone(),
                        "result": {}
                    }),
                };
                if let Ok(line) = serde_json::to_string(&response) {
                    let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                    let _ = server_write.flush().await;
                }
                if done {
                    break;
                }
            }
            drop(server_write);
        });
    });

    let mut worker = CodexLaneWorker::spawn_with_runtime(
        CodexLaneConfig::default(),
        Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
    );

    let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
        snapshot.lifecycle == CodexLaneLifecycle::Ready
    });

    let enqueue_result = worker.enqueue(
        88,
        CodexLaneCommand::SkillsList(SkillsListParams {
            cwds: vec![fixture_root.clone()],
            force_reload: true,
            per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                cwd: fixture_root.clone(),
                extra_user_roots: vec![fixture_skills_root.clone()],
            }]),
        }),
    );
    assert!(enqueue_result.is_ok());

    let response = wait_for_command_response(&mut worker, Duration::from_secs(2), |response| {
        response.command_seq == 88
    });
    assert_eq!(response.command, CodexLaneCommandKind::SkillsList);
    assert_eq!(response.status, CodexLaneCommandStatus::Accepted);
    assert!(saw_skills_list.load(Ordering::SeqCst));

    shutdown_worker(&mut worker);
    join_server(server);
    let _ = fs::remove_dir_all(&fixture_root);
}

#[test]
fn labs_api_smoke_commands_emit_responses_and_notifications() {
    let (client_stream, server_stream) = tokio::io::duplex(16 * 1024);
    let (client_read, client_write) = tokio::io::split(client_stream);
    let (server_read, mut server_write) = tokio::io::split(server_stream);
    let runtime_guard = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|_| panic!("failed to build runtime"));
    let _entered = runtime_guard.enter();
    let (client, channels) =
        AppServerClient::connect_with_io(Box::new(client_write), Box::new(client_read), None);
    drop(_entered);

    let server = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.block_on(async move {
            let mut reader = BufReader::new(server_read);
            let mut request_line = String::new();
            let expected = [
                "review/start",
                "command/exec",
                "collaborationMode/list",
                "experimentalFeature/list",
                "thread/realtime/start",
                "thread/realtime/appendText",
                "thread/realtime/stop",
                "windowsSandbox/setupStart",
                "fuzzyFileSearch/sessionStart",
                "fuzzyFileSearch/sessionUpdate",
                "fuzzyFileSearch/sessionStop",
            ];
            let mut seen = HashSet::<String>::new();

            loop {
                request_line.clear();
                let bytes = reader.read_line(&mut request_line).await.unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                let value: Value = match serde_json::from_str(request_line.trim()) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("id").is_none() {
                    continue;
                }
                let method = value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if expected.contains(&method) {
                    seen.insert(method.to_string());
                }
                let response = match method {
                    "initialize" => json!({
                        "id": value["id"].clone(),
                        "result": {"userAgent": "test-agent"}
                    }),
                    "thread/start" => json!({
                        "id": value["id"].clone(),
                        "result": {
                            "thread": {"id": "thread-bootstrap"},
                            "model": "gpt-5.3-codex"
                        }
                    }),
                    "model/list" => json!({
                        "id": value["id"].clone(),
                        "result": {
                            "data": [
                                {
                                    "id": "default",
                                    "model": "gpt-5.3-codex",
                                    "displayName": "gpt-5.3-codex",
                                    "description": "default",
                                    "supportedReasoningEfforts": [],
                                    "defaultReasoningEffort": "medium",
                                    "isDefault": true
                                }
                            ],
                            "nextCursor": null
                        }
                    }),
                    "review/start" => json!({
                        "id": value["id"].clone(),
                        "result": {
                            "turn": {"id": "turn-review"},
                            "reviewThreadId": "review-thread-1"
                        }
                    }),
                    "command/exec" => json!({
                        "id": value["id"].clone(),
                        "result": {
                            "exitCode": 0,
                            "stdout": "ok",
                            "stderr": ""
                        }
                    }),
                    "collaborationMode/list" => json!({
                        "id": value["id"].clone(),
                        "result": {"data": [{"id": "pair"}]}
                    }),
                    "experimentalFeature/list" => json!({
                        "id": value["id"].clone(),
                        "result": {"data": [{"id": "feature"}], "nextCursor": null}
                    }),
                    "thread/realtime/start"
                    | "thread/realtime/appendText"
                    | "thread/realtime/stop"
                    | "fuzzyFileSearch/sessionStart"
                    | "fuzzyFileSearch/sessionUpdate"
                    | "fuzzyFileSearch/sessionStop" => json!({
                        "id": value["id"].clone(),
                        "result": {}
                    }),
                    "windowsSandbox/setupStart" => json!({
                        "id": value["id"].clone(),
                        "result": {"started": true}
                    }),
                    _ => json!({
                        "id": value["id"].clone(),
                        "result": {}
                    }),
                };

                if let Ok(line) = serde_json::to_string(&response) {
                    let _ = server_write.write_all(format!("{line}\n").as_bytes()).await;
                    let _ = server_write.flush().await;
                }
                if seen.len() == expected.len() {
                    break;
                }
            }
            drop(server_write);
        });
    });

    let mut worker = CodexLaneWorker::spawn_with_runtime(
        CodexLaneConfig::default(),
        Box::new(SingleClientRuntime::new((client, channels), runtime_guard)),
    );
    let _ = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
        snapshot.lifecycle == CodexLaneLifecycle::Ready
    });

    let commands = [
        (
            1501,
            CodexLaneCommand::ReviewStart(ReviewStartParams {
                thread_id: "thread-bootstrap".to_string(),
                target: ReviewTarget::UncommittedChanges,
                delivery: Some(codex_client::ReviewDelivery::Inline),
            }),
        ),
        (
            1502,
            CodexLaneCommand::CommandExec(CommandExecParams {
                command: vec!["pwd".to_string()],
                timeout_ms: Some(5000),
                cwd: None,
                sandbox_policy: None,
            }),
        ),
        (
            1503,
            CodexLaneCommand::CollaborationModeList(CollaborationModeListParams::default()),
        ),
        (
            1504,
            CodexLaneCommand::ExperimentalFeatureList(ExperimentalFeatureListParams {
                cursor: None,
                limit: Some(100),
            }),
        ),
        (
            1505,
            CodexLaneCommand::ThreadRealtimeStart(ThreadRealtimeStartParams {
                thread_id: "thread-bootstrap".to_string(),
                prompt: "start".to_string(),
                session_id: Some("session-a".to_string()),
            }),
        ),
        (
            1506,
            CodexLaneCommand::ThreadRealtimeAppendText(ThreadRealtimeAppendTextParams {
                thread_id: "thread-bootstrap".to_string(),
                text: "hello".to_string(),
            }),
        ),
        (
            1507,
            CodexLaneCommand::ThreadRealtimeStop(ThreadRealtimeStopParams {
                thread_id: "thread-bootstrap".to_string(),
            }),
        ),
        (
            1508,
            CodexLaneCommand::WindowsSandboxSetupStart(WindowsSandboxSetupStartParams {
                mode: "enable".to_string(),
            }),
        ),
        (
            1509,
            CodexLaneCommand::FuzzyFileSearchSessionStart(FuzzyFileSearchSessionStartParams {
                session_id: "session-a".to_string(),
                roots: vec![".".to_string()],
            }),
        ),
        (
            1510,
            CodexLaneCommand::FuzzyFileSearchSessionUpdate(FuzzyFileSearchSessionUpdateParams {
                session_id: "session-a".to_string(),
                query: "codex".to_string(),
            }),
        ),
        (
            1511,
            CodexLaneCommand::FuzzyFileSearchSessionStop(FuzzyFileSearchSessionStopParams {
                session_id: "session-a".to_string(),
            }),
        ),
    ];
    for (seq, command) in commands {
        let enqueue = worker.enqueue(seq, command);
        assert!(enqueue.is_ok(), "failed to enqueue command seq={seq}");
    }

    let mut saw_review = false;
    let mut saw_exec = false;
    let mut saw_collab = false;
    let mut saw_features = false;
    let mut saw_realtime_start = false;
    let mut saw_realtime_append = false;
    let mut saw_realtime_stop = false;
    let mut saw_windows = false;
    let mut saw_fuzzy_start = false;
    let mut saw_fuzzy_update = false;
    let mut saw_fuzzy_stop = false;
    let mut accepted_responses = HashSet::new();

    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() <= deadline {
        for update in worker.drain_updates() {
            match update {
                CodexLaneUpdate::CommandResponse(response) => {
                    if response.command_seq >= 1501 && response.command_seq <= 1511 {
                        assert_eq!(
                            response.status,
                            CodexLaneCommandStatus::Accepted,
                            "unexpected command rejection: {:?}",
                            response
                        );
                        accepted_responses.insert(response.command_seq);
                    }
                }
                CodexLaneUpdate::Notification(notification) => match notification {
                    CodexLaneNotification::ReviewStarted { .. } => saw_review = true,
                    CodexLaneNotification::CommandExecCompleted { .. } => saw_exec = true,
                    CodexLaneNotification::CollaborationModesLoaded { .. } => saw_collab = true,
                    CodexLaneNotification::ExperimentalFeaturesLoaded { .. } => saw_features = true,
                    CodexLaneNotification::RealtimeStarted { .. } => saw_realtime_start = true,
                    CodexLaneNotification::RealtimeTextAppended { .. } => {
                        saw_realtime_append = true;
                    }
                    CodexLaneNotification::RealtimeStopped { .. } => saw_realtime_stop = true,
                    CodexLaneNotification::WindowsSandboxSetupStarted { .. } => saw_windows = true,
                    CodexLaneNotification::FuzzySessionStarted { .. } => saw_fuzzy_start = true,
                    CodexLaneNotification::FuzzySessionUpdated { .. } => saw_fuzzy_update = true,
                    CodexLaneNotification::FuzzySessionStopped { .. } => saw_fuzzy_stop = true,
                    _ => {}
                },
                CodexLaneUpdate::Snapshot(_) => {}
            }
        }
        if accepted_responses.len() == 11
            && saw_review
            && saw_exec
            && saw_collab
            && saw_features
            && saw_realtime_start
            && saw_realtime_append
            && saw_realtime_stop
            && saw_windows
            && saw_fuzzy_start
            && saw_fuzzy_update
            && saw_fuzzy_stop
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(
        accepted_responses.len(),
        11,
        "missing accepted command responses"
    );
    assert!(saw_review, "missing review notification");
    assert!(saw_exec, "missing command/exec notification");
    assert!(saw_collab, "missing collaborationMode/list notification");
    assert!(
        saw_features,
        "missing experimentalFeature/list notification"
    );
    assert!(saw_realtime_start, "missing realtime start notification");
    assert!(saw_realtime_append, "missing realtime append notification");
    assert!(saw_realtime_stop, "missing realtime stop notification");
    assert!(saw_windows, "missing windows sandbox notification");
    assert!(saw_fuzzy_start, "missing fuzzy start notification");
    assert!(saw_fuzzy_update, "missing fuzzy update notification");
    assert!(saw_fuzzy_stop, "missing fuzzy stop notification");

    shutdown_worker(&mut worker);
    join_server(server);
}

#[test]
fn wire_log_path_is_forwarded_to_lane_runtime() {
    struct CaptureWireLogRuntime {
        expected: PathBuf,
        saw_expected: Arc<AtomicBool>,
    }

    impl CodexLaneRuntime for CaptureWireLogRuntime {
        fn connect(
            &mut self,
            _runtime: &tokio::runtime::Runtime,
            config: &CodexLaneConfig,
        ) -> Result<(AppServerClient, AppServerChannels)> {
            if config.wire_log_path.as_ref() == Some(&self.expected) {
                self.saw_expected.store(true, Ordering::SeqCst);
            }
            Err(anyhow::anyhow!("captured wire log config"))
        }
    }

    let expected = unique_fixture_root("wire-log").join("codex-wire.log");
    let saw_expected = Arc::new(AtomicBool::new(false));
    let config = CodexLaneConfig {
        wire_log_path: Some(expected.clone()),
        ..CodexLaneConfig::default()
    };

    let mut worker = CodexLaneWorker::spawn_with_runtime(
        config,
        Box::new(CaptureWireLogRuntime {
            expected,
            saw_expected: Arc::clone(&saw_expected),
        }),
    );

    let snapshot = wait_for_snapshot(&mut worker, Duration::from_secs(2), |snapshot| {
        snapshot.lifecycle == CodexLaneLifecycle::Error
    });
    assert!(saw_expected.load(Ordering::SeqCst));
    let has_error = snapshot
        .last_error
        .as_deref()
        .is_some_and(|message| message.contains("captured wire log config"));
    assert!(has_error, "expected captured wire log startup failure");

    shutdown_worker(&mut worker);
}

fn shutdown_worker(worker: &mut CodexLaneWorker) {
    worker.shutdown_async();
}

fn join_server(server: std::thread::JoinHandle<()>) {
    let (tx, rx) = std::sync::mpsc::sync_channel::<std::thread::Result<()>>(1);
    std::thread::spawn(move || {
        let _ = tx.send(server.join());
    });
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => {}
        Ok(Err(_)) => panic!("server thread panicked"),
        Err(_) => {
            // Keep teardown bounded in tests; avoid suite-level hangs on stalled fixtures.
        }
    }
}

fn wait_for_snapshot<F>(
    worker: &mut CodexLaneWorker,
    timeout: Duration,
    predicate: F,
) -> super::CodexLaneSnapshot
where
    F: Fn(&super::CodexLaneSnapshot) -> bool,
{
    let deadline = Instant::now() + timeout;
    let mut matched: Option<super::CodexLaneSnapshot> = None;
    while Instant::now() <= deadline {
        for update in worker.drain_updates() {
            if let CodexLaneUpdate::Snapshot(snapshot) = update
                && predicate(&snapshot)
            {
                matched = Some(*snapshot);
                break;
            }
        }
        if matched.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        matched.is_some(),
        "timed out waiting for codex lane snapshot"
    );
    matched.unwrap_or_default()
}

fn unique_fixture_root(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "openagents-codex-lane-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

fn wait_for_command_response<F>(
    worker: &mut CodexLaneWorker,
    timeout: Duration,
    predicate: F,
) -> CodexLaneCommandResponse
where
    F: Fn(&super::CodexLaneCommandResponse) -> bool,
{
    let deadline = Instant::now() + timeout;
    let mut matched: Option<super::CodexLaneCommandResponse> = None;
    while Instant::now() <= deadline {
        for update in worker.drain_updates() {
            if let CodexLaneUpdate::CommandResponse(response) = update
                && predicate(&response)
            {
                matched = Some(response);
                break;
            }
        }
        if matched.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        matched.is_some(),
        "timed out waiting for codex lane command response"
    );
    matched.unwrap_or(super::CodexLaneCommandResponse {
        command_seq: 0,
        command: super::CodexLaneCommandKind::ThreadList,
        status: super::CodexLaneCommandStatus::Retryable,
        error: Some("missing command response".to_string()),
    })
}
