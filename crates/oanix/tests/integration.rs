//! Integration tests for OANIX filesystem primitives
//!
//! These tests demonstrate how the primitives compose together
//! into real-world patterns for agent execution environments.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use oanix::{
    CowFs, FileService, FuncFs, LogEvent, LogLevel, LogsFs, MapFs, MemFs, Namespace, OpenFlags,
    TaskFs, TaskMeta, TaskSpec, TaskStatus,
};

// ============================================================================
// Helper functions
// ============================================================================

fn read_file(fs: &dyn FileService, path: &str) -> String {
    let mut handle = fs.open(path, OpenFlags::read_only()).unwrap();
    let mut buf = vec![0u8; 4096];
    let n = handle.read(&mut buf).unwrap();
    String::from_utf8_lossy(&buf[..n]).to_string()
}

fn write_file(fs: &dyn FileService, path: &str, content: &str) {
    let mut handle = fs
        .open(
            path,
            OpenFlags {
                write: true,
                create: true,
                truncate: true,
                ..Default::default()
            },
        )
        .unwrap();
    handle.write(content.as_bytes()).unwrap();
    handle.flush().unwrap();
}

fn list_dir(fs: &dyn FileService, path: &str) -> Vec<String> {
    fs.readdir(path)
        .unwrap()
        .iter()
        .map(|e| e.name.clone())
        .collect()
}

// ============================================================================
// Pattern 1: TaskFs-style composition
// ============================================================================

/// Demonstrates composing MapFs + FuncFs + MemFs into a task filesystem
/// like what TaskFs would look like internally.
#[test]
fn test_taskfs_pattern() {
    // Simulate TaskFs internals:
    // - /spec.json: immutable task specification (MapFs)
    // - /status: live status computed on read (FuncFs)
    // - /result.json: writable result file (MemFs)

    let task_id = "regex-log-001";
    let spec_json = format!(
        r#"{{"id": "{}", "type": "regex", "description": "Extract dates from logs"}}"#,
        task_id
    );

    // Status tracking
    let status = Arc::new(std::sync::RwLock::new("pending".to_string()));
    let status_read = status.clone();
    let status_write = status.clone();

    // Build the composite filesystem using a namespace
    let spec_fs = MapFs::builder()
        .file("/spec.json", spec_json.as_bytes())
        .file("/meta.json", r#"{"created": "2025-12-11", "version": 1}"#)
        .build();

    let status_fs = FuncFs::builder()
        .read_write(
            "/status",
            move || {
                let s = status_read.read().unwrap();
                format!(r#"{{"status": "{}"}}"#, *s).into_bytes()
            },
            move |data| {
                if let Ok(s) = String::from_utf8(data) {
                    // Parse simple status update
                    if s.contains("running") {
                        *status_write.write().unwrap() = "running".to_string();
                    } else if s.contains("completed") {
                        *status_write.write().unwrap() = "completed".to_string();
                    }
                }
            },
        )
        .build();

    let result_fs = MemFs::new();

    // Build namespace
    let ns = Namespace::builder()
        .mount("/task/spec", spec_fs)
        .mount("/task/live", status_fs)
        .mount("/task/output", result_fs)
        .build();

    // Test 1: Read immutable spec
    let (spec_service, rel_path) = ns.resolve("/task/spec/spec.json").unwrap();
    let content = read_file(spec_service, rel_path);
    assert!(content.contains("regex-log-001"));
    assert!(content.contains("Extract dates from logs"));

    // Test 2: Read live status
    let (status_service, rel_path) = ns.resolve("/task/live/status").unwrap();
    let content = read_file(status_service, rel_path);
    assert!(content.contains(r#""status": "pending""#));

    // Test 3: Update status
    write_file(status_service, rel_path, r#"{"status": "running"}"#);

    // Test 4: Verify status changed
    let content = read_file(status_service, rel_path);
    assert!(content.contains(r#""status": "running""#));

    // Test 5: Write result
    let (result_service, rel_path) = ns.resolve("/task/output/result.json").unwrap();
    write_file(
        result_service,
        rel_path,
        r#"{"success": true, "matches": 42}"#,
    );

    // Test 6: Read result back
    let content = read_file(result_service, rel_path);
    assert!(content.contains(r#""success": true"#));
    assert!(content.contains(r#""matches": 42"#));
}

// ============================================================================
// Pattern 2: Workspace snapshots with CowFs
// ============================================================================

/// Demonstrates using CowFs for workspace snapshots where agents can modify
/// files without affecting the original.
#[test]
fn test_workspace_snapshot_pattern() {
    // Create "original" project files
    let original = MapFs::builder()
        .file("/src/main.rs", b"fn main() { println!(\"Hello\"); }")
        .file(
            "/src/lib.rs",
            b"pub fn add(a: i32, b: i32) -> i32 { a + b }",
        )
        .file(
            "/Cargo.toml",
            b"[package]\nname = \"myproject\"\nversion = \"0.1.0\"",
        )
        .file("/README.md", b"# My Project\n\nA sample project.")
        .build();

    // Wrap in CowFs for agent modifications
    let workspace = CowFs::new(original);

    // Verify original content accessible
    assert_eq!(
        read_file(&workspace, "/src/main.rs"),
        "fn main() { println!(\"Hello\"); }"
    );

    // Agent modifies main.rs
    write_file(
        &workspace,
        "/src/main.rs",
        "fn main() { println!(\"Hello, World!\"); }",
    );

    // Verify modification
    assert_eq!(
        read_file(&workspace, "/src/main.rs"),
        "fn main() { println!(\"Hello, World!\"); }"
    );

    // Other files still accessible from base
    assert!(read_file(&workspace, "/src/lib.rs").contains("pub fn add"));

    // Agent creates new file
    write_file(&workspace, "/src/utils.rs", "pub fn helper() {}");
    assert_eq!(read_file(&workspace, "/src/utils.rs"), "pub fn helper() {}");

    // Agent can delete files (tombstoned)
    workspace.remove("/README.md").unwrap();
    assert!(workspace.stat("/README.md").is_err());

    // List directory shows merged view
    let entries = list_dir(&workspace, "/src");
    assert!(entries.contains(&"main.rs".to_string()));
    assert!(entries.contains(&"lib.rs".to_string()));
    assert!(entries.contains(&"utils.rs".to_string()));
}

// ============================================================================
// Pattern 3: Control files with FuncFs
// ============================================================================

/// Demonstrates using FuncFs for control interfaces like `/cap/*/control`
#[test]
fn test_control_file_pattern() {
    // Simulate a capability service with control files
    let connection_count = Arc::new(AtomicU64::new(0));
    let last_command = Arc::new(std::sync::RwLock::new(String::new()));

    let conn_count_read = connection_count.clone();
    let conn_count_write = connection_count.clone();
    let cmd_store = last_command.clone();

    let cap_fs = FuncFs::builder()
        // Status file - read-only, shows current state
        .read_only("/status", move || {
            let count = conn_count_read.load(Ordering::SeqCst);
            format!(r#"{{"connections": {}, "active": true}}"#, count).into_bytes()
        })
        // Control file - write triggers action
        .write_only("/control", move |data| {
            if let Ok(cmd) = String::from_utf8(data) {
                let cmd = cmd.trim();
                *cmd_store.write().unwrap() = cmd.to_string();

                // Simulate command handling (check disconnect first since it contains "connect")
                if cmd.contains("disconnect") {
                    conn_count_write.fetch_sub(1, Ordering::SeqCst);
                } else if cmd.contains("connect") {
                    conn_count_write.fetch_add(1, Ordering::SeqCst);
                }
            }
        })
        .build();

    // Check initial status
    let status = read_file(&cap_fs, "/status");
    assert!(status.contains(r#""connections": 0"#));

    // Send connect command
    write_file(
        &cap_fs,
        "/control",
        r#"{"action": "connect", "url": "wss://relay.example.com"}"#,
    );

    // Check command was received
    assert!(last_command.read().unwrap().contains("connect"));

    // Check status updated
    let status = read_file(&cap_fs, "/status");
    assert!(status.contains(r#""connections": 1"#));

    // Send another connect
    write_file(&cap_fs, "/control", r#"{"action": "connect"}"#);
    let status = read_file(&cap_fs, "/status");
    assert!(status.contains(r#""connections": 2"#));

    // Disconnect
    write_file(&cap_fs, "/control", r#"{"action": "disconnect"}"#);
    let status = read_file(&cap_fs, "/status");
    assert!(status.contains(r#""connections": 1"#));
}

// ============================================================================
// Pattern 4: Full agent namespace
// ============================================================================

/// Demonstrates a complete agent namespace with all components
#[test]
fn test_full_agent_namespace() {
    // Task specification (immutable)
    let task_fs = MapFs::builder()
        .file(
            "/spec.json",
            r#"{"id": "task-001", "type": "code-review", "repo": "openagents"}"#,
        )
        .build();

    // Workspace (copy-on-write over base project)
    let project_base = MapFs::builder()
        .file("/src/main.rs", b"fn main() {}")
        .file("/Cargo.toml", b"[package]\nname = \"test\"")
        .build();
    let workspace = CowFs::new(project_base);

    // Logs (writable)
    let logs = MemFs::new();

    // Temporary storage
    let tmp = MemFs::new();

    // Build the namespace
    let ns = Namespace::builder()
        .mount("/task", task_fs)
        .mount("/workspace", workspace)
        .mount("/logs", logs)
        .mount("/tmp", tmp)
        .build();

    // Agent workflow simulation:

    // 1. Read task spec
    let (task_svc, path) = ns.resolve("/task/spec.json").unwrap();
    let spec = read_file(task_svc, path);
    assert!(spec.contains("code-review"));

    // 2. Read workspace file
    let (ws_svc, path) = ns.resolve("/workspace/src/main.rs").unwrap();
    let content = read_file(ws_svc, path);
    assert_eq!(content, "fn main() {}");

    // 3. Modify workspace
    write_file(ws_svc, path, "fn main() { todo!() }");
    assert_eq!(read_file(ws_svc, path), "fn main() { todo!() }");

    // 4. Write to logs
    let (logs_svc, _) = ns.resolve("/logs").unwrap();
    write_file(logs_svc, "/stdout.log", "Starting code review...\n");
    write_file(logs_svc, "/stderr.log", "");

    // Append to log
    {
        let mut handle = logs_svc
            .open(
                "/stdout.log",
                OpenFlags {
                    write: true,
                    append: true,
                    ..Default::default()
                },
            )
            .unwrap();
        handle.write(b"Review complete.\n").unwrap();
        handle.flush().unwrap();
    }

    // 5. Use tmp for scratch work
    let (tmp_svc, _) = ns.resolve("/tmp").unwrap();
    write_file(tmp_svc, "/scratch.txt", "intermediate results");

    // Verify logs
    let log_content = read_file(logs_svc, "/stdout.log");
    assert!(log_content.contains("Starting code review"));
    assert!(log_content.contains("Review complete"));

    // Verify namespace structure
    let mounts: Vec<&str> = ns.mounts().iter().map(|m| m.path.as_str()).collect();
    assert!(mounts.contains(&"/task"));
    assert!(mounts.contains(&"/workspace"));
    assert!(mounts.contains(&"/logs"));
    assert!(mounts.contains(&"/tmp"));
}

// ============================================================================
// Pattern 5: Layered CowFs (multiple snapshots)
// ============================================================================

/// Demonstrates layering CowFs for checkpoint/restore functionality
#[test]
fn test_layered_snapshots() {
    // Base layer: original project
    let base = MapFs::builder()
        .file("/config.json", r#"{"version": 1}"#)
        .file("/data.txt", b"original data")
        .build();

    // First snapshot: agent makes initial changes
    let snapshot1 = CowFs::new(base);
    write_file(&snapshot1, "/config.json", r#"{"version": 2}"#);
    write_file(&snapshot1, "/new_file.txt", "created in snapshot 1");

    // Verify snapshot1 state
    assert!(read_file(&snapshot1, "/config.json").contains("version\": 2"));
    assert!(read_file(&snapshot1, "/new_file.txt").contains("snapshot 1"));
    assert_eq!(read_file(&snapshot1, "/data.txt"), "original data");

    // For a second snapshot, we'd need to snapshot the CowFs state
    // This demonstrates that CowFs preserves the base while allowing modifications
}

// ============================================================================
// Pattern 6: Read-only enforcement
// ============================================================================

/// Verifies that MapFs properly enforces read-only access
#[test]
fn test_readonly_enforcement() {
    let fs = MapFs::builder()
        .file("/readonly.txt", b"cannot modify")
        .build();

    // Read works
    assert_eq!(read_file(&fs, "/readonly.txt"), "cannot modify");

    // Write should fail
    let result = fs.open(
        "/readonly.txt",
        OpenFlags {
            write: true,
            ..Default::default()
        },
    );
    assert!(result.is_err());

    // Create should fail
    let result = fs.open(
        "/new.txt",
        OpenFlags {
            write: true,
            create: true,
            ..Default::default()
        },
    );
    assert!(result.is_err());

    // mkdir should fail
    assert!(fs.mkdir("/newdir").is_err());

    // remove should fail
    assert!(fs.remove("/readonly.txt").is_err());
}

// ============================================================================
// Pattern 7: Dynamic file content
// ============================================================================

/// Verifies that FuncFs recomputes content on each read
#[test]
fn test_dynamic_recomputation() {
    let counter = Arc::new(AtomicU64::new(0));
    let counter_clone = counter.clone();

    let fs = FuncFs::builder()
        .read_only("/counter", move || {
            let val = counter_clone.fetch_add(1, Ordering::SeqCst);
            val.to_string().into_bytes()
        })
        .build();

    // Each open should get a new value
    assert_eq!(read_file(&fs, "/counter"), "0");
    assert_eq!(read_file(&fs, "/counter"), "1");
    assert_eq!(read_file(&fs, "/counter"), "2");

    // But reading from same handle should return same value
    let mut handle = fs.open("/counter", OpenFlags::read_only()).unwrap();
    let mut buf1 = [0u8; 10];
    let mut buf2 = [0u8; 10];

    handle.seek(0).unwrap();
    let n1 = handle.read(&mut buf1).unwrap();

    handle.seek(0).unwrap();
    let n2 = handle.read(&mut buf2).unwrap();

    assert_eq!(&buf1[..n1], &buf2[..n2]); // Same content from same handle
}

// ============================================================================
// Pattern 8: Namespace resolution
// ============================================================================

/// Tests that namespace correctly routes to the right filesystem
#[test]
fn test_namespace_routing() {
    let fs_a = MapFs::builder().file("/file.txt", b"from A").build();
    let fs_b = MapFs::builder().file("/file.txt", b"from B").build();
    let fs_c = MapFs::builder().file("/file.txt", b"from C").build();

    let ns = Namespace::builder()
        .mount("/a", fs_a)
        .mount("/b", fs_b)
        .mount("/nested/deep/c", fs_c)
        .build();

    // Each mount point routes to its own filesystem
    let (svc_a, path) = ns.resolve("/a/file.txt").unwrap();
    assert_eq!(read_file(svc_a, path), "from A");

    let (svc_b, path) = ns.resolve("/b/file.txt").unwrap();
    assert_eq!(read_file(svc_b, path), "from B");

    let (svc_c, path) = ns.resolve("/nested/deep/c/file.txt").unwrap();
    assert_eq!(read_file(svc_c, path), "from C");

    // Non-existent paths return None
    assert!(ns.resolve("/nonexistent/file.txt").is_none());
}

// ============================================================================
// Pattern 9: TaskFs standard service
// ============================================================================

/// Tests the TaskFs standard service for task execution environments
#[test]
fn test_taskfs_service() {
    let spec = TaskSpec {
        id: "bench-001".to_string(),
        task_type: "regex".to_string(),
        description: "Extract IP addresses from log files".to_string(),
        input: serde_json::json!({
            "log_format": "nginx",
            "target_field": "remote_addr"
        }),
    };

    let mut meta = TaskMeta::default();
    meta.tags = vec!["benchmark".to_string(), "regex".to_string()];
    meta.timeout_secs = Some(300);

    let task = TaskFs::new(spec, meta);

    // 1. Read task specification
    let spec_content = read_file(&task, "/spec.json");
    assert!(spec_content.contains("bench-001"));
    assert!(spec_content.contains("Extract IP addresses"));
    assert!(spec_content.contains("nginx"));

    // 2. Read metadata
    let meta_content = read_file(&task, "/meta.json");
    assert!(meta_content.contains("benchmark"));
    assert!(meta_content.contains("300"));

    // 3. Check initial status
    assert_eq!(task.get_status(), TaskStatus::Pending);
    let status_content = read_file(&task, "/status");
    assert!(status_content.contains("pending"));

    // 4. Simulate task lifecycle
    task.set_running();
    let status_content = read_file(&task, "/status");
    assert!(status_content.contains("running"));
    assert!(status_content.contains("started_at"));

    // 5. Write result
    write_file(
        &task,
        "/result.json",
        r#"{"matches": 1234, "accuracy": 0.99}"#,
    );

    // 6. Complete task
    task.set_completed();
    assert!(task.is_finished());

    // 7. Verify final state
    let result_content = read_file(&task, "/result.json");
    assert!(result_content.contains("1234"));

    // 8. Verify directory listing
    let entries = list_dir(&task, "/");
    assert!(entries.contains(&"spec.json".to_string()));
    assert!(entries.contains(&"meta.json".to_string()));
    assert!(entries.contains(&"status".to_string()));
    assert!(entries.contains(&"result.json".to_string()));
}

// ============================================================================
// Pattern 10: LogsFs standard service
// ============================================================================

/// Tests the LogsFs standard service for structured logging
#[test]
fn test_logsfs_service() {
    let logs = LogsFs::new();

    // 1. Programmatic stdout/stderr
    logs.write_stdout(b"[INFO] Starting task execution\n");
    logs.write_stdout(b"[INFO] Processing file 1 of 100\n");
    logs.write_stderr(b"[WARN] Deprecated API usage detected\n");

    // 2. Structured events
    logs.info("Task started");
    logs.log_event(LogEvent::with_data(
        LogLevel::Debug,
        "Configuration loaded",
        serde_json::json!({
            "timeout": 300,
            "workers": 4
        }),
    ));
    logs.warn("Rate limit approaching");
    logs.error("Connection timeout");

    // 3. Read stdout via file interface
    let stdout_content = read_file(&logs, "/stdout.log");
    assert!(stdout_content.contains("[INFO] Starting task"));
    assert!(stdout_content.contains("Processing file 1"));

    // 4. Read stderr via file interface
    let stderr_content = read_file(&logs, "/stderr.log");
    assert!(stderr_content.contains("[WARN] Deprecated"));

    // 5. Read structured events
    let events_content = read_file(&logs, "/events.jsonl");
    assert!(events_content.contains("Task started"));
    assert!(events_content.contains("Configuration loaded"));
    assert!(events_content.contains("Rate limit"));
    assert!(events_content.contains("Connection timeout"));

    // Each event should be valid JSON on its own line
    for line in events_content.trim().lines() {
        let event: LogEvent = serde_json::from_str(line).unwrap();
        assert!(!event.message.is_empty());
    }

    // 6. Verify event count
    let events = logs.events();
    assert_eq!(events.len(), 4);

    // 7. Clear and verify
    logs.clear();
    assert!(logs.stdout().is_empty());
    assert!(logs.stderr().is_empty());
    assert!(logs.events().is_empty());
}

// ============================================================================
// Pattern 11: Complete agent environment with standard services
// ============================================================================

/// Tests a complete agent execution environment with all standard services
#[test]
fn test_complete_agent_environment() {
    // Create task
    let task = TaskFs::new(
        TaskSpec {
            id: "agent-task-001".to_string(),
            task_type: "code-review".to_string(),
            description: "Review pull request #42".to_string(),
            input: serde_json::json!({"pr_number": 42}),
        },
        TaskMeta::default(),
    );

    // Create logs
    let logs = LogsFs::new();

    // Create workspace (using CowFs over MapFs as mock)
    let project_base = MapFs::builder()
        .file("/src/main.rs", b"fn main() { old_code(); }")
        .file("/src/lib.rs", b"pub fn old_code() {}")
        .file("/Cargo.toml", b"[package]\nname = \"myproject\"")
        .build();
    let workspace = CowFs::new(project_base);

    // Create tmp
    let tmp = MemFs::new();

    // Build complete namespace
    let ns = Namespace::builder()
        .mount("/task", task)
        .mount("/logs", logs)
        .mount("/workspace", workspace)
        .mount("/tmp", tmp)
        .build();

    // === Agent execution simulation ===

    // 1. Read task spec
    let (task_svc, _) = ns.resolve("/task/spec.json").unwrap();
    let spec_content = read_file(task_svc, "/spec.json");
    assert!(spec_content.contains("code-review"));
    assert!(spec_content.contains("42"));

    // 2. Log start
    let (logs_svc, _) = ns.resolve("/logs").unwrap();
    // Cast to LogsFs to use programmatic API (in real code, would write via file)
    {
        let mut handle = logs_svc
            .open(
                "/stdout.log",
                OpenFlags {
                    write: true,
                    ..Default::default()
                },
            )
            .unwrap();
        handle.write(b"Starting code review...\n").unwrap();
    }

    // 3. Read and modify workspace
    let (ws_svc, _) = ns.resolve("/workspace").unwrap();
    let main_content = read_file(ws_svc, "/src/main.rs");
    assert!(main_content.contains("old_code"));

    // Agent modifies the file
    write_file(ws_svc, "/src/main.rs", "fn main() { new_code(); }");

    // 4. Write intermediate results to tmp
    let (tmp_svc, _) = ns.resolve("/tmp").unwrap();
    write_file(
        tmp_svc,
        "/analysis.json",
        r#"{"issues": ["unused import"]}"#,
    );

    // 5. Log completion
    {
        let mut handle = logs_svc
            .open(
                "/stdout.log",
                OpenFlags {
                    write: true,
                    append: true,
                    ..Default::default()
                },
            )
            .unwrap();
        handle.write(b"Review complete.\n").unwrap();
    }

    // 6. Write final result
    write_file(
        task_svc,
        "/result.json",
        r#"{"approved": true, "comments": 3}"#,
    );

    // === Verify final state ===

    // Workspace was modified
    let modified_main = read_file(ws_svc, "/src/main.rs");
    assert!(modified_main.contains("new_code"));

    // Logs captured everything
    let stdout = read_file(logs_svc, "/stdout.log");
    assert!(stdout.contains("Starting"));
    assert!(stdout.contains("complete"));

    // Result was written
    let result = read_file(task_svc, "/result.json");
    assert!(result.contains("approved"));

    // Tmp has intermediate data
    let analysis = read_file(tmp_svc, "/analysis.json");
    assert!(analysis.contains("unused import"));
}

// ============================================================================
// Pattern 12: Task failure handling
// ============================================================================

/// Tests proper handling of task failures
#[test]
fn test_task_failure_handling() {
    let task = TaskFs::new(
        TaskSpec {
            id: "failing-task".to_string(),
            task_type: "compile".to_string(),
            description: "Compile the project".to_string(),
            input: serde_json::json!({}),
        },
        TaskMeta::default(),
    );

    // Start task
    task.set_running();
    assert!(!task.is_finished());

    // Task fails
    task.set_failed("Compilation error: undefined reference to `main`");

    // Verify failure state
    assert!(task.is_finished());

    let status = task.get_status();
    match status {
        TaskStatus::Failed { error, .. } => {
            assert!(error.contains("undefined reference"));
        }
        _ => panic!("Expected Failed status"),
    }

    // Status file reflects failure
    let status_content = read_file(&task, "/status");
    assert!(status_content.contains("failed"));
    assert!(status_content.contains("undefined reference"));
}

// ============================================================================
// Pattern 13: WsFs capability service
// ============================================================================

mod ws_tests {
    use super::*;
    use oanix::WsFs;

    /// Tests WsFs as a capability in an agent namespace
    #[test]
    fn test_ws_capability_in_namespace() {
        let ws = WsFs::new();
        let task = MapFs::builder()
            .file("/spec.json", r#"{"task": "connect to relay"}"#)
            .build();

        let ns = Namespace::builder()
            .mount("/task", task)
            .mount("/cap/ws", ws)
            .build();

        // Agent reads task
        let (task_svc, path) = ns.resolve("/task/spec.json").unwrap();
        let spec = read_file(task_svc, path);
        assert!(spec.contains("connect to relay"));

        // Agent checks WebSocket service status
        let (ws_svc, _) = ns.resolve("/cap/ws").unwrap();
        let status = read_file(ws_svc, "/status");
        assert!(status.contains("connection_count"));
        assert!(status.contains("max_connections"));
    }

    /// Tests WebSocket connection lifecycle via file interface
    #[test]
    fn test_ws_connection_lifecycle() {
        let ws = WsFs::new();

        // 1. Open a connection via control file
        let connect_json = r#"{"action": "connect", "url": "wss://relay.example.com"}"#;
        let mut handle = ws.open("/control", OpenFlags::write_only()).unwrap();
        handle.write(connect_json.as_bytes()).unwrap();
        handle.flush().unwrap();

        // 2. Verify connection exists
        let entries = ws.readdir("/conns").unwrap();
        assert_eq!(entries.len(), 1);
        let conn_id = &entries[0].name;

        // 3. Read connection status
        let conn_status = read_file(&ws, &format!("/conns/{}/status", conn_id));
        assert!(conn_status.contains("connecting") || conn_status.contains("open"));

        // 4. Read connection URL
        let conn_url = read_file(&ws, &format!("/conns/{}/url", conn_id));
        assert!(conn_url.contains("relay.example.com"));

        // 5. Simulate connection becoming open
        ws.set_connected(conn_id).unwrap();
        let conn_status = read_file(&ws, &format!("/conns/{}/status", conn_id));
        assert!(conn_status.contains("open"));

        // 6. Close connection via control file
        let close_json = format!(r#"{{"action": "close", "id": "{}"}}"#, conn_id);
        let mut handle = ws.open("/control", OpenFlags::write_only()).unwrap();
        handle.write(close_json.as_bytes()).unwrap();
        handle.flush().unwrap();

        // 7. Connection should be in closing/closed state
        let conn_status = read_file(&ws, &format!("/conns/{}/status", conn_id));
        assert!(conn_status.contains("closing") || conn_status.contains("closed"));
    }

    /// Tests sending and receiving messages via file interface
    #[test]
    fn test_ws_message_exchange() {
        let ws = WsFs::new();

        // Open connection programmatically
        let conn_id = ws.open_connection("wss://relay.example.com").unwrap();
        ws.set_connected(&conn_id).unwrap();

        // Send message via /out file
        let out_path = format!("/conns/{}/out", conn_id);
        write_file(&ws, &out_path, r#"["EVENT", {"content": "hello"}]"#);

        // Check pending outgoing messages
        let pending = ws.drain_outbox(&conn_id).unwrap();
        assert_eq!(pending.len(), 1);
        assert!(String::from_utf8_lossy(&pending[0]).contains("hello"));

        // Simulate receiving a message (external connector would do this)
        ws.receive_message(
            &conn_id,
            r#"["OK", "event-id-123", true]"#.as_bytes().to_vec(),
        )
        .unwrap();

        // Read incoming messages via /in file
        let in_path = format!("/conns/{}/in", conn_id);
        let messages = read_file(&ws, &in_path);
        assert!(messages.contains("OK"));
        assert!(messages.contains("event-id-123"));
    }

    /// Tests agent workflow with WebSocket capability
    #[test]
    fn test_ws_agent_workflow() {
        // Setup agent environment with WebSocket capability
        let task = TaskFs::new(
            TaskSpec {
                id: "ws-task-001".to_string(),
                task_type: "nostr-relay".to_string(),
                description: "Connect to relay and publish event".to_string(),
                input: serde_json::json!({"relay": "wss://relay.damus.io"}),
            },
            TaskMeta::default(),
        );
        let logs = LogsFs::new();
        let ws = WsFs::new();

        let ns = Namespace::builder()
            .mount("/task", task)
            .mount("/logs", logs)
            .mount("/cap/ws", ws)
            .build();

        // === Agent Workflow ===

        // 1. Read task
        let (task_svc, _) = ns.resolve("/task").unwrap();
        let spec = read_file(task_svc, "/spec.json");
        let task_input: serde_json::Value = serde_json::from_str(&spec).unwrap();
        let relay = task_input["input"]["relay"].as_str().unwrap();
        assert_eq!(relay, "wss://relay.damus.io");

        // 2. Open WebSocket connection
        let (ws_svc, _) = ns.resolve("/cap/ws").unwrap();
        let connect_json = format!(r#"{{"action": "connect", "url": "{}"}}"#, relay);
        let mut handle = ws_svc.open("/control", OpenFlags::write_only()).unwrap();
        handle.write(connect_json.as_bytes()).unwrap();
        handle.flush().unwrap();

        // 3. Verify connection opened
        let entries = ws_svc.readdir("/conns").unwrap();
        assert_eq!(entries.len(), 1);
        let conn_id = &entries[0].name;

        // 4. Log the action
        let (logs_svc, _) = ns.resolve("/logs").unwrap();
        let log_msg = format!("Connected to {} with id {}\n", relay, conn_id);
        let mut handle = logs_svc
            .open(
                "/stdout.log",
                OpenFlags {
                    write: true,
                    ..Default::default()
                },
            )
            .unwrap();
        handle.write(log_msg.as_bytes()).unwrap();

        // 5. Send a message (would be a Nostr event in real use)
        let out_path = format!("/conns/{}/out", conn_id);
        let mut handle = ws_svc.open(&out_path, OpenFlags::write_only()).unwrap();
        handle.write(r#"["REQ", "sub-1", {}]"#.as_bytes()).unwrap();
        handle.flush().unwrap();

        // 6. Write result
        write_file(task_svc, "/result.json", r#"{"connected": true}"#);

        // === Verify ===
        let result = read_file(task_svc, "/result.json");
        assert!(result.contains("\"connected\": true"));

        let log = read_file(logs_svc, "/stdout.log");
        assert!(log.contains("Connected to"));
    }
}

// ============================================================================
// Pattern 14: HttpFs capability service
// ============================================================================

mod http_tests {
    use super::*;
    use oanix::{HttpFs, HttpMethod, HttpRequest, HttpResponse, RequestState};
    use std::collections::HashMap;

    /// Tests HttpFs as a capability in an agent namespace
    #[test]
    fn test_http_capability_in_namespace() {
        let http = HttpFs::new();
        let task = MapFs::builder()
            .file("/spec.json", r#"{"task": "fetch API data"}"#)
            .build();

        let ns = Namespace::builder()
            .mount("/task", task)
            .mount("/cap/http", http)
            .build();

        // Agent reads task
        let (task_svc, path) = ns.resolve("/task/spec.json").unwrap();
        let spec = read_file(task_svc, path);
        assert!(spec.contains("fetch API data"));

        // Agent checks HTTP service status
        let (http_svc, _) = ns.resolve("/cap/http").unwrap();
        let status = read_file(http_svc, "/status");
        assert!(status.contains("pending_count"));
        assert!(status.contains("completed_count"));
    }

    /// Tests HTTP request/response lifecycle
    #[test]
    fn test_http_request_response_lifecycle() {
        let http = HttpFs::new();

        // 1. Submit request via file interface
        let request_json = r#"{
            "method": "GET",
            "url": "https://api.example.com/data",
            "headers": {"Authorization": "Bearer token123"}
        }"#;
        write_file(&http, "/request", request_json);

        // 2. Verify request is pending
        let entries = http.readdir("/pending").unwrap();
        assert_eq!(entries.len(), 1);
        let req_id = entries[0].name.trim_end_matches(".json");

        // 3. Read pending request details
        let pending_content = read_file(&http, &format!("/pending/{}.json", req_id));
        assert!(pending_content.contains("api.example.com"));
        assert!(pending_content.contains("Bearer token123"));

        // 4. Simulate executor completing the request
        http.complete_request(HttpResponse {
            request_id: req_id.to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: r#"{"users": [{"name": "Alice"}]}"#.to_string(),
            duration_ms: 150,
            completed_at: 1702300000,
        });

        // 5. Request no longer pending
        let entries = http.readdir("/pending").unwrap();
        assert!(entries.is_empty());

        // 6. Response available
        let response = read_file(&http, &format!("/responses/{}.json", req_id));
        assert!(response.contains("\"status\": 200"));
        assert!(response.contains("Alice"));
    }

    /// Tests HTTP error handling
    #[test]
    fn test_http_error_handling() {
        let http = HttpFs::new();

        // Submit request
        let request = HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://invalid.example.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        };
        let req_id = http.submit_request(request);

        // Simulate failure
        http.fail_request(&req_id, "DNS resolution failed");

        // Check state
        assert_eq!(http.get_state(&req_id), Some(RequestState::Failed));

        // Read failure via file interface
        let response = read_file(&http, &format!("/responses/{}.json", req_id));
        assert!(response.contains("DNS resolution failed"));
        assert!(response.contains("\"status\": \"failed\""));
    }

    /// Tests complete agent workflow with HTTP capability
    #[test]
    fn test_http_agent_workflow() {
        // Setup agent environment with HTTP capability
        let task = TaskFs::new(
            TaskSpec {
                id: "http-task-001".to_string(),
                task_type: "api-fetch".to_string(),
                description: "Fetch user data from API".to_string(),
                input: serde_json::json!({
                    "endpoint": "https://api.example.com/users",
                    "auth_token": "secret123"
                }),
            },
            TaskMeta::default(),
        );
        let logs = LogsFs::new();
        let http = HttpFs::new();

        let ns = Namespace::builder()
            .mount("/task", task)
            .mount("/logs", logs)
            .mount("/cap/http", http)
            .build();

        // === Agent Workflow ===

        // 1. Read task
        let (task_svc, _) = ns.resolve("/task").unwrap();
        let spec = read_file(task_svc, "/spec.json");
        let task_input: serde_json::Value = serde_json::from_str(&spec).unwrap();
        let endpoint = task_input["input"]["endpoint"].as_str().unwrap();
        let auth_token = task_input["input"]["auth_token"].as_str().unwrap();

        // 2. Submit HTTP request
        let (http_svc, _) = ns.resolve("/cap/http").unwrap();
        let request_json = format!(
            r#"{{"method": "GET", "url": "{}", "headers": {{"Authorization": "Bearer {}"}}}}"#,
            endpoint, auth_token
        );
        write_file(http_svc, "/request", &request_json);

        // 3. Get request ID from pending
        let entries = http_svc.readdir("/pending").unwrap();
        assert_eq!(entries.len(), 1);
        let req_id = entries[0].name.trim_end_matches(".json").to_string();

        // 4. Log the request
        let (logs_svc, _) = ns.resolve("/logs").unwrap();
        let log_msg = format!("Submitted HTTP request {} to {}\n", req_id, endpoint);
        let mut handle = logs_svc
            .open(
                "/stdout.log",
                OpenFlags {
                    write: true,
                    ..Default::default()
                },
            )
            .unwrap();
        handle.write(log_msg.as_bytes()).unwrap();

        // 5. Simulate external executor completing request
        // (In real use, this would be done by an HTTP executor task)
        {
            let (http_svc, _) = ns.resolve("/cap/http").unwrap();
            // Downcast to HttpFs to call complete_request
            // In real code, the executor would have direct access
            let http: &HttpFs = unsafe { &*(http_svc as *const dyn FileService as *const HttpFs) };
            http.complete_request(HttpResponse {
                request_id: req_id.clone(),
                status: 200,
                status_text: "OK".to_string(),
                headers: HashMap::new(),
                body: r#"{"users": [{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]}"#
                    .to_string(),
                duration_ms: 250,
                completed_at: 1702300000,
            });
        }

        // 6. Read response
        let response = read_file(http_svc, &format!("/responses/{}.json", req_id));
        assert!(response.contains("Alice"));
        assert!(response.contains("Bob"));

        // 7. Write result
        write_file(task_svc, "/result.json", r#"{"user_count": 2}"#);

        // === Verify ===
        let result = read_file(task_svc, "/result.json");
        assert!(result.contains("\"user_count\": 2"));

        let log = read_file(logs_svc, "/stdout.log");
        assert!(log.contains("Submitted HTTP request"));
    }

    /// Tests multiple concurrent requests
    #[test]
    fn test_http_multiple_requests() {
        let http = HttpFs::new();

        // Submit multiple requests
        write_file(
            &http,
            "/request",
            r#"{"method": "GET", "url": "https://api.example.com/a"}"#,
        );
        write_file(
            &http,
            "/request",
            r#"{"method": "GET", "url": "https://api.example.com/b"}"#,
        );
        write_file(
            &http,
            "/request",
            r#"{"method": "POST", "url": "https://api.example.com/c", "body": "{\"data\": 42}"}"#,
        );

        // All should be pending
        let entries = http.readdir("/pending").unwrap();
        assert_eq!(entries.len(), 3);

        // Status should reflect counts
        let status = read_file(&http, "/status");
        assert!(status.contains("\"pending_count\": 3"));

        // Complete one
        let req_id = entries[0].name.trim_end_matches(".json");
        http.complete_request(HttpResponse {
            request_id: req_id.to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: "response a".to_string(),
            duration_ms: 100,
            completed_at: 1702300000,
        });

        // Now 2 pending, 1 completed
        let status = read_file(&http, "/status");
        assert!(status.contains("\"pending_count\": 2"));
        assert!(status.contains("\"completed_count\": 1"));
    }
}

// ============================================================================
// Pattern 15: Complete namespace with all capabilities
// ============================================================================

mod combined_capabilities_tests {
    use super::*;
    use oanix::{HttpFs, WsFs};
    use std::collections::HashMap;

    /// Tests an agent environment with all capability services
    #[test]
    fn test_full_capability_namespace() {
        // Task
        let task = TaskFs::new(
            TaskSpec {
                id: "full-cap-task".to_string(),
                task_type: "integration".to_string(),
                description: "Test all capabilities".to_string(),
                input: serde_json::json!({}),
            },
            TaskMeta::default(),
        );

        // Logs
        let logs = LogsFs::new();

        // Workspace
        let workspace = MemFs::new();

        // Capabilities
        let ws = WsFs::new();
        let http = HttpFs::new();

        // Build namespace
        let ns = Namespace::builder()
            .mount("/task", task)
            .mount("/logs", logs)
            .mount("/workspace", workspace)
            .mount("/cap/ws", ws)
            .mount("/cap/http", http)
            .mount("/tmp", MemFs::new())
            .build();

        // Verify all mounts exist
        let mounts: Vec<&str> = ns.mounts().iter().map(|m| m.path.as_str()).collect();
        assert!(mounts.contains(&"/task"));
        assert!(mounts.contains(&"/logs"));
        assert!(mounts.contains(&"/workspace"));
        assert!(mounts.contains(&"/cap/ws"));
        assert!(mounts.contains(&"/cap/http"));
        assert!(mounts.contains(&"/tmp"));

        // Access each capability
        let (task_svc, _) = ns.resolve("/task/spec.json").unwrap();
        assert!(read_file(task_svc, "/spec.json").contains("integration"));

        let (ws_svc, _) = ns.resolve("/cap/ws/status").unwrap();
        assert!(read_file(ws_svc, "/status").contains("connection_count"));

        let (http_svc, _) = ns.resolve("/cap/http/status").unwrap();
        assert!(read_file(http_svc, "/status").contains("pending_count"));
    }

    /// Tests a multi-step workflow using multiple capabilities
    #[test]
    fn test_multi_capability_workflow() {
        let task = TaskFs::new(
            TaskSpec {
                id: "multi-cap-001".to_string(),
                task_type: "data-aggregation".to_string(),
                description: "Fetch data from API and stream to relay".to_string(),
                input: serde_json::json!({
                    "api_url": "https://api.example.com/data",
                    "relay_url": "wss://relay.example.com"
                }),
            },
            TaskMeta::default(),
        );
        let logs = LogsFs::new();
        let ws = WsFs::new();
        let http = HttpFs::new();
        let tmp = MemFs::new();

        let ns = Namespace::builder()
            .mount("/task", task)
            .mount("/logs", logs)
            .mount("/cap/ws", ws)
            .mount("/cap/http", http)
            .mount("/tmp", tmp)
            .build();

        // === Multi-step Workflow ===

        // 1. Read task spec
        let (task_svc, _) = ns.resolve("/task").unwrap();
        task_svc.open("/status", OpenFlags::read_only()).unwrap(); // Task starts

        // 2. Submit HTTP request
        let (http_svc, _) = ns.resolve("/cap/http").unwrap();
        write_file(
            http_svc,
            "/request",
            r#"{"method": "GET", "url": "https://api.example.com/data"}"#,
        );

        // 3. Open WebSocket connection
        let (ws_svc, _) = ns.resolve("/cap/ws").unwrap();
        let connect_json = r#"{"action": "connect", "url": "wss://relay.example.com"}"#;
        let mut handle = ws_svc.open("/control", OpenFlags::write_only()).unwrap();
        handle.write(connect_json.as_bytes()).unwrap();
        handle.flush().unwrap();

        // 4. Log progress
        let (logs_svc, _) = ns.resolve("/logs").unwrap();
        write_file(
            logs_svc,
            "/stdout.log",
            "HTTP request submitted, WS connection opened\n",
        );

        // 5. Store intermediate data in tmp
        let (tmp_svc, _) = ns.resolve("/tmp").unwrap();
        write_file(
            tmp_svc,
            "/state.json",
            r#"{"http_pending": true, "ws_connected": true}"#,
        );

        // 6. Simulate HTTP response arriving
        {
            let http: &HttpFs = unsafe { &*(http_svc as *const dyn FileService as *const HttpFs) };
            let req_id = http.list_pending()[0].clone();
            http.complete_request(oanix::HttpResponse {
                request_id: req_id,
                status: 200,
                status_text: "OK".to_string(),
                headers: HashMap::new(),
                body: r#"{"items": [1, 2, 3]}"#.to_string(),
                duration_ms: 100,
                completed_at: 1702300000,
            });
        }

        // 7. Get connection ID and send data over WebSocket
        let conn_entries = ws_svc.readdir("/conns").unwrap();
        let conn_id = &conn_entries[0].name;

        {
            let ws: &WsFs = unsafe { &*(ws_svc as *const dyn FileService as *const WsFs) };
            ws.set_connected(conn_id).unwrap();
        }

        let out_path = format!("/conns/{}/out", conn_id);
        write_file(ws_svc, &out_path, r#"["EVENT", {"items": [1, 2, 3]}]"#);

        // 8. Write final result
        write_file(
            task_svc,
            "/result.json",
            r#"{"items_sent": 3, "success": true}"#,
        );

        // === Verify ===
        let result = read_file(task_svc, "/result.json");
        assert!(result.contains("\"success\": true"));

        let status = read_file(http_svc, "/status");
        assert!(status.contains("\"completed_count\": 1"));

        let ws_status = read_file(ws_svc, "/status");
        assert!(ws_status.contains("\"connection_count\": 1"));
    }
}

// ============================================================================
// Pattern 16: NostrFs capability service (requires nostr feature)
// ============================================================================

#[cfg(feature = "nostr")]
mod nostr_tests {
    use super::*;
    use oanix::NostrFs;
    use std::collections::HashMap;

    fn test_secret_key() -> [u8; 32] {
        let hex = "d217c1ff2f8a65c3e3a1740db3b9f58b8c848bb45e26d00ed4714e4a0f4ceecf";
        let bytes = hex::decode(hex).unwrap();
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        key
    }

    /// Tests NostrFs as a capability in an agent namespace
    #[test]
    fn test_nostr_capability_in_namespace() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();
        nostr.add_relay("wss://relay.damus.io");

        let task = MapFs::builder()
            .file("/spec.json", r#"{"task": "post to nostr"}"#)
            .build();

        let ns = Namespace::builder()
            .mount("/task", task)
            .mount("/cap/nostr", nostr)
            .build();

        // Agent reads task
        let (task_svc, path) = ns.resolve("/task/spec.json").unwrap();
        let spec = read_file(task_svc, path);
        assert!(spec.contains("post to nostr"));

        // Agent reads its Nostr identity
        let (nostr_svc, _) = ns.resolve("/cap/nostr").unwrap();
        let pubkey = read_file(nostr_svc, "/identity/pubkey");
        assert_eq!(pubkey.len(), 64); // hex pubkey

        let npub = read_file(nostr_svc, "/identity/npub");
        assert!(npub.starts_with("npub1"));

        // Agent checks status
        let status = read_file(nostr_svc, "/status");
        assert!(status.contains("ready"));
        assert!(status.contains("relay.damus.io"));
    }

    /// Tests submitting events via file interface
    #[test]
    fn test_nostr_event_submission() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();

        // Submit a simple event via /submit
        let event_json = r#"{"kind": 1, "content": "Hello from OANIX agent!"}"#;
        let mut handle = nostr.open("/submit", OpenFlags::write_only()).unwrap();
        handle.write(event_json.as_bytes()).unwrap();
        handle.flush().unwrap();

        // Event should be in outbox
        let entries = nostr.readdir("/outbox").unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].name.ends_with(".json"));

        // Read the event from outbox
        let event_id = entries[0].name.trim_end_matches(".json");
        let event_content = read_file(&nostr, &format!("/outbox/{}", event_id));
        assert!(event_content.contains("Hello from OANIX agent!"));
        assert!(event_content.contains("\"kind\": 1"));
        assert!(event_content.contains("\"sig\":")); // Event is signed
    }

    /// Tests submitting NIP-90 job requests
    #[test]
    fn test_nostr_job_request_submission() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();
        nostr.add_relay("wss://relay.damus.io");

        // Submit a NIP-90 job request
        let request_json = r#"{
            "kind": 5050,
            "input": "What is the capital of France?",
            "params": {"model": "gpt-4", "temperature": "0.7"},
            "bid": 1000
        }"#;

        let mut handle = nostr.open("/request", OpenFlags::write_only()).unwrap();
        handle.write(request_json.as_bytes()).unwrap();
        handle.flush().unwrap();

        // Event should be in outbox
        let events = nostr.outbox_events();
        assert_eq!(events.len(), 1);

        let event = &events[0];
        assert_eq!(event.kind, 5050); // Text generation
        assert!(
            event
                .tags
                .iter()
                .any(|t| t[0] == "i" && t[1] == "What is the capital of France?")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|t| t[0] == "param" && t[1] == "model")
        );
        assert!(event.tags.iter().any(|t| t[0] == "bid" && t[1] == "1000"));
    }

    /// Tests complete agent workflow with NostrFs
    #[test]
    fn test_nostr_agent_workflow() {
        // Setup: Agent environment with Nostr capability
        let task = TaskFs::new(
            TaskSpec {
                id: "nostr-task-001".to_string(),
                task_type: "llm-query".to_string(),
                description: "Ask LLM a question via NIP-90".to_string(),
                input: serde_json::json!({"question": "What is 2+2?"}),
            },
            TaskMeta::default(),
        );
        let logs = LogsFs::new();
        let nostr = NostrFs::new(test_secret_key()).unwrap();
        nostr.add_relay("wss://relay.damus.io");

        let ns = Namespace::builder()
            .mount("/task", task)
            .mount("/logs", logs)
            .mount("/cap/nostr", nostr)
            .build();

        // === Agent Workflow ===

        // 1. Read task
        let (task_svc, _) = ns.resolve("/task").unwrap();
        let spec = read_file(task_svc, "/spec.json");
        let task_input: serde_json::Value = serde_json::from_str(&spec).unwrap();
        let question = task_input["input"]["question"].as_str().unwrap();
        assert_eq!(question, "What is 2+2?");

        // 2. Get Nostr identity
        let (nostr_svc, _) = ns.resolve("/cap/nostr").unwrap();
        let pubkey = read_file(nostr_svc, "/identity/pubkey");

        // 3. Submit NIP-90 job request
        let request = format!(
            r#"{{"kind": 5050, "input": "{}", "params": {{}}}}"#,
            question
        );
        let mut handle = nostr_svc.open("/request", OpenFlags::write_only()).unwrap();
        handle.write(request.as_bytes()).unwrap();
        handle.flush().unwrap();

        // 4. Verify event in outbox
        let entries = nostr_svc.readdir("/outbox").unwrap();
        assert_eq!(entries.len(), 1);

        // 5. Log the action
        let (logs_svc, _) = ns.resolve("/logs").unwrap();
        let log_msg = format!("Published NIP-90 request from {}\n", &pubkey[..16]);
        let mut handle = logs_svc
            .open(
                "/stdout.log",
                OpenFlags {
                    write: true,
                    ..Default::default()
                },
            )
            .unwrap();
        handle.write(log_msg.as_bytes()).unwrap();

        // 6. In a real system, a relay connector would:
        //    - Send the outbox event to relays
        //    - Receive response events and add them to inbox
        //    For this test, we verify the outbox flow worked

        // 7. Write final result
        write_file(
            task_svc,
            "/result.json",
            r#"{"answer": "4", "source": "nip90"}"#,
        );

        // === Verify ===
        let result = read_file(task_svc, "/result.json");
        assert!(result.contains("\"answer\": \"4\""));

        let log = read_file(logs_svc, "/stdout.log");
        assert!(log.contains("Published NIP-90 request"));
    }

    /// Tests programmatic API for creating job requests
    #[test]
    fn test_nostr_programmatic_api() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();
        nostr.add_relay("wss://relay1.com");
        nostr.add_relay("wss://relay2.com");

        let mut params = HashMap::new();
        params.insert("model".to_string(), "claude".to_string());
        params.insert("max_tokens".to_string(), "1000".to_string());

        let event = nostr
            .create_job_request(5050, "Summarize this article", params)
            .unwrap();

        // Verify event structure
        assert_eq!(event.kind, 5050);
        assert_eq!(event.pubkey, nostr.pubkey());
        assert!(!event.sig.is_empty());

        // Verify tags
        assert!(
            event
                .tags
                .iter()
                .any(|t| t[0] == "i" && t[1] == "Summarize this article")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|t| t[0] == "param" && t[1] == "model" && t[2] == "claude")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|t| t[0] == "relays" && t.contains(&"wss://relay1.com".to_string()))
        );

        // Verify in outbox
        assert_eq!(nostr.outbox_events().len(), 1);

        // Clear outbox
        nostr.clear_outbox();
        assert!(nostr.outbox_events().is_empty());
    }
}

// ============================================================================
// Pattern 17: OanixEnv - Complete environment abstraction
// ============================================================================

mod env_tests {
    use super::*;
    use oanix::{EnvBuilder, EnvStatus, OanixEnv};

    /// Tests basic OanixEnv creation and lifecycle
    #[test]
    fn test_env_creation_and_lifecycle() {
        // Create environment with standard mounts
        let env = EnvBuilder::new()
            .mount(
                "/task",
                TaskFs::new(
                    TaskSpec {
                        id: "env-test-001".into(),
                        task_type: "test".into(),
                        description: "Test task".into(),
                        input: serde_json::json!({}),
                    },
                    TaskMeta::default(),
                ),
            )
            .mount("/logs", LogsFs::new())
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();

        // Initial state
        assert_eq!(env.status(), EnvStatus::Created);
        assert!(!env.is_finished());

        // Status info
        let info = env.status_info();
        assert_eq!(info.mount_count, 3);
        assert!(info.created_at > 0);

        // Can resolve paths
        assert!(env.resolve("/task/spec.json").is_some());
        assert!(env.resolve("/logs").is_some());
        assert!(env.resolve("/tmp").is_some());

        // Set running
        env.set_running();
        match env.status() {
            EnvStatus::Running { started_at } => assert!(started_at > 0),
            _ => panic!("Expected Running status"),
        }

        // Set completed
        env.set_completed(0);
        assert!(env.is_finished());
    }

    /// Tests environment with all capability services
    #[test]
    fn test_env_with_capabilities() {
        use oanix::{HttpFs, WsFs};

        let env = EnvBuilder::new()
            .mount(
                "/task",
                TaskFs::new(
                    TaskSpec {
                        id: "cap-test".into(),
                        task_type: "full-stack".into(),
                        description: "Test with all capabilities".into(),
                        input: serde_json::json!({}),
                    },
                    TaskMeta::default(),
                ),
            )
            .mount("/logs", LogsFs::new())
            .mount("/workspace", MemFs::new())
            .mount("/cap/ws", WsFs::new())
            .mount("/cap/http", HttpFs::new())
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();

        // All mounts accessible
        assert!(env.resolve("/task").is_some());
        assert!(env.resolve("/logs").is_some());
        assert!(env.resolve("/workspace").is_some());
        assert!(env.resolve("/cap/ws").is_some());
        assert!(env.resolve("/cap/http").is_some());
        assert!(env.resolve("/tmp").is_some());

        // Can interact with capabilities
        let (ws_svc, _) = env.resolve("/cap/ws").unwrap();
        let status = read_file(ws_svc, "/status");
        assert!(status.contains("connection_count"));

        let (http_svc, _) = env.resolve("/cap/http").unwrap();
        let status = read_file(http_svc, "/status");
        assert!(status.contains("pending_count"));
    }

    /// Tests environment failure handling
    #[test]
    fn test_env_failure_handling() {
        let env = EnvBuilder::new()
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();

        env.set_running();
        env.set_failed("Task execution failed: timeout");

        match env.status() {
            EnvStatus::Failed { error, .. } => {
                assert!(error.contains("timeout"));
            }
            _ => panic!("Expected Failed status"),
        }
        assert!(env.is_finished());
    }

    /// Tests agent workflow within OanixEnv
    #[test]
    fn test_env_agent_workflow() {
        let env = EnvBuilder::new()
            .mount(
                "/task",
                TaskFs::new(
                    TaskSpec {
                        id: "workflow-001".into(),
                        task_type: "analysis".into(),
                        description: "Analyze data".into(),
                        input: serde_json::json!({"data": [1, 2, 3, 4, 5]}),
                    },
                    TaskMeta::default(),
                ),
            )
            .mount("/logs", LogsFs::new())
            .mount("/workspace", MemFs::new())
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();

        // Start
        env.set_running();

        // Read task
        let (task_svc, _) = env.resolve("/task").unwrap();
        let spec = read_file(task_svc, "/spec.json");
        assert!(spec.contains("analysis"));

        // Write to workspace
        let (ws_svc, _) = env.resolve("/workspace").unwrap();
        write_file(ws_svc, "/analysis.txt", "Sum: 15, Mean: 3.0");

        // Log progress
        let (logs_svc, _) = env.resolve("/logs").unwrap();
        let mut handle = logs_svc
            .open(
                "/stdout.log",
                OpenFlags {
                    write: true,
                    ..Default::default()
                },
            )
            .unwrap();
        handle.write(b"Analysis complete\n").unwrap();

        // Write result
        write_file(task_svc, "/result.json", r#"{"sum": 15, "mean": 3.0}"#);

        // Complete
        env.set_completed(0);
        assert!(env.is_finished());

        // Verify outputs
        let result = read_file(task_svc, "/result.json");
        assert!(result.contains("\"sum\": 15"));

        let analysis = read_file(ws_svc, "/analysis.txt");
        assert!(analysis.contains("Mean: 3.0"));
    }
}

// ============================================================================
// Pattern 18: Scheduler - Job queue and execution
// ============================================================================

mod scheduler_tests {
    use super::*;
    use oanix::{EnvBuilder, JobKind, JobSpec, JobStatus, Scheduler};

    /// Tests basic scheduler operations
    #[test]
    fn test_scheduler_basic() {
        let mut scheduler = Scheduler::new();

        // Create and register an environment
        let env = EnvBuilder::new()
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();
        let env_id = scheduler.register_env(env);

        // Submit a job
        let job = JobSpec::new(env_id, JobKind::script("echo hello"));
        let job_id = scheduler.submit(job).unwrap();

        // Verify queue state
        assert_eq!(scheduler.pending_count(), 1);
        assert_eq!(scheduler.running_count(), 0);

        // Get next job
        let running_job = scheduler.next().unwrap();
        assert_eq!(running_job.id, job_id);
        assert!(matches!(running_job.status, JobStatus::Running { .. }));

        // Verify state changed
        assert_eq!(scheduler.pending_count(), 0);
        assert_eq!(scheduler.running_count(), 1);

        // Complete the job
        scheduler.complete(&job_id, 0);
        assert_eq!(scheduler.running_count(), 0);
        assert_eq!(scheduler.completed_count(), 1);

        // Check result
        let result = scheduler.get_result(&job_id).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.error.is_none());
    }

    /// Tests priority-based job scheduling
    #[test]
    fn test_scheduler_priority() {
        let mut scheduler = Scheduler::new();

        let env = EnvBuilder::new()
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();
        let env_id = scheduler.register_env(env);

        // Submit jobs with different priorities
        let low = JobSpec::new(env_id, JobKind::script("low priority")).with_priority(-5);
        let high = JobSpec::new(env_id, JobKind::script("high priority")).with_priority(10);
        let medium = JobSpec::new(env_id, JobKind::script("medium priority")).with_priority(0);

        scheduler.submit(low).unwrap();
        scheduler.submit(high).unwrap();
        scheduler.submit(medium).unwrap();

        // Jobs should come out in priority order
        let first = scheduler.next().unwrap();
        assert_eq!(first.priority, 10);
        scheduler.complete(&first.id, 0);

        let second = scheduler.next().unwrap();
        assert_eq!(second.priority, 0);
        scheduler.complete(&second.id, 0);

        let third = scheduler.next().unwrap();
        assert_eq!(third.priority, -5);
        scheduler.complete(&third.id, 0);
    }

    /// Tests concurrency limits
    #[test]
    fn test_scheduler_concurrency() {
        let mut scheduler = Scheduler::with_max_concurrent(2);

        let env = EnvBuilder::new()
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();
        let env_id = scheduler.register_env(env);

        // Submit 5 jobs
        for i in 0..5 {
            let job = JobSpec::new(env_id, JobKind::script(format!("job {}", i)));
            scheduler.submit(job).unwrap();
        }

        // Can only run 2 at a time
        let j1 = scheduler.next().unwrap();
        let j2 = scheduler.next().unwrap();
        assert!(scheduler.next().is_none()); // Blocked

        assert_eq!(scheduler.running_count(), 2);
        assert_eq!(scheduler.pending_count(), 3);

        // Complete one, can start another
        scheduler.complete(&j1.id, 0);
        assert_eq!(scheduler.running_count(), 1);

        let j3 = scheduler.next().unwrap();
        assert_eq!(scheduler.running_count(), 2);

        // Complete remaining
        scheduler.complete(&j2.id, 0);
        scheduler.complete(&j3.id, 0);

        let j4 = scheduler.next().unwrap();
        let j5 = scheduler.next().unwrap();
        scheduler.complete(&j4.id, 0);
        scheduler.complete(&j5.id, 0);

        assert_eq!(scheduler.completed_count(), 5);
        assert_eq!(scheduler.pending_count(), 0);
    }

    /// Tests job failure handling
    #[test]
    fn test_scheduler_job_failure() {
        let mut scheduler = Scheduler::new();

        let env = EnvBuilder::new()
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();
        let env_id = scheduler.register_env(env);

        let job = JobSpec::new(env_id, JobKind::script("will fail"));
        let job_id = scheduler.submit(job).unwrap();

        let running = scheduler.next().unwrap();
        scheduler.fail(&running.id, "Script execution failed");

        let result = scheduler.get_result(&job_id).unwrap();
        assert_eq!(result.exit_code, 1);
        assert!(result.error.as_ref().unwrap().contains("failed"));
    }

    /// Tests scheduler with multiple environments
    #[test]
    fn test_scheduler_multi_env() {
        let mut scheduler = Scheduler::new();

        // Create two environments
        let env1 = EnvBuilder::new()
            .mount("/workspace", MemFs::new())
            .build()
            .unwrap();
        let env1_id = scheduler.register_env(env1);

        let env2 = EnvBuilder::new()
            .mount("/workspace", MemFs::new())
            .build()
            .unwrap();
        let env2_id = scheduler.register_env(env2);

        // Submit jobs to different environments
        let job1 = JobSpec::new(env1_id, JobKind::script("env1 job"));
        let job2 = JobSpec::new(env2_id, JobKind::script("env2 job"));

        scheduler.submit(job1).unwrap();
        scheduler.submit(job2).unwrap();

        let status = scheduler.status();
        assert_eq!(status.env_count, 2);
        assert_eq!(status.pending_count, 2);
    }

    /// Tests job with environment variables and tags
    #[test]
    fn test_scheduler_job_config() {
        let mut scheduler = Scheduler::new();

        let env = EnvBuilder::new()
            .mount("/tmp", MemFs::new())
            .build()
            .unwrap();
        let env_id = scheduler.register_env(env);

        let job = JobSpec::new(env_id, JobKind::script("configured job"))
            .with_priority(5)
            .env("API_KEY", "secret123")
            .env("DEBUG", "true")
            .with_working_dir("/workspace")
            .with_timeout(300)
            .tag("urgent")
            .tag("production");

        let job_id = scheduler.submit(job).unwrap();

        let running = scheduler.next().unwrap();
        assert_eq!(running.id, job_id);
        assert_eq!(running.priority, 5);
        assert_eq!(running.env_vars.len(), 2);
        assert_eq!(running.working_dir, Some("/workspace".to_string()));
        assert_eq!(running.timeout_secs, Some(300));
        assert_eq!(running.tags, vec!["urgent", "production"]);
    }
}
