//! Integration tests for OANIX filesystem primitives
//!
//! These tests demonstrate how the primitives compose together
//! into real-world patterns for agent execution environments.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
        .file("/src/lib.rs", b"pub fn add(a: i32, b: i32) -> i32 { a + b }")
        .file("/Cargo.toml", b"[package]\nname = \"myproject\"\nversion = \"0.1.0\"")
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
    write_file(&cap_fs, "/control", r#"{"action": "connect", "url": "wss://relay.example.com"}"#);

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
    write_file(&task, "/result.json", r#"{"matches": 1234, "accuracy": 0.99}"#);

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
    write_file(tmp_svc, "/analysis.json", r#"{"issues": ["unused import"]}"#);

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
