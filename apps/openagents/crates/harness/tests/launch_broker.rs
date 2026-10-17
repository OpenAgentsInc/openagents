#![cfg(unix)]

use std::cell::Cell;
use std::rc::Rc;

use openagents_harness::process::LaunchBroker;
use openagents_harness::{
    CancellationToken, ClaudeHarness, CodexHarness, Harness, OpencodeHarness, RunControls,
};
use openagents_proto::RunRequest;
use tokio::sync::{mpsc, oneshot};

#[tokio::test]
async fn every_native_driver_waits_for_the_borrowed_host_wrapper() {
    for (driver, with_mcp) in [
        ("claude", false),
        ("codex", false),
        ("opencode", false),
        ("opencode", true),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("started");
        let binary = directory.path().join(driver);
        std::fs::write(
            &binary,
            format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let (broker, mut launches) = LaunchBroker::channel();
        let harness: Box<dyn Harness> = match driver {
            "claude" => Box::new(
                ClaudeHarness::new()
                    .with_executable(&binary)
                    .with_launch_broker(broker),
            ),
            "codex" => Box::new(
                CodexHarness::new()
                    .with_executable(&binary)
                    .with_launch_broker(broker),
            ),
            _ => Box::new(
                OpencodeHarness::new()
                    .with_executable(&binary)
                    .with_launch_broker(broker),
            ),
        };
        let mut request: RunRequest = serde_json::from_value(serde_json::json!({
            "prompt": "fixture", "model": null, "reasoning": null,
            "cwd": directory.path(), "sandbox": "read-only", "resume": null,
        }))
        .unwrap();
        if with_mcp {
            request.mcp = Some(openagents_proto::McpServer {
                name: "fixture".into(),
                command: "/bin/false".into(),
                ..Default::default()
            });
        }
        let (_steer, steering) = mpsc::channel(1);
        let controls = RunControls {
            realtime: None,
            execution_lease: None,
            request_input: Box::new(|_| {
                let (_, rx) = oneshot::channel();
                rx
            }),
            steering,
            interrupt: CancellationToken::new(),
            turn: Default::default(),
        };
        // Rc<Cell<_>> deliberately cannot enter a Send + Sync harness task.
        let calls = Rc::new(Cell::new(0));
        let wrap = |command: std::process::Command| {
            assert!(!marker.exists(), "{driver} started before host admission");
            if !with_mcp {
                assert_eq!(command.get_current_dir(), Some(directory.path()));
            }
            let first = command.get_args().next().unwrap().to_string_lossy();
            assert_eq!(
                first,
                match driver {
                    "claude" => "--print",
                    "codex" => "app-server",
                    _ if with_mcp => "--version",
                    _ => "serve",
                }
            );
            calls.set(calls.get() + 1);
            Err("fixture host refusal".into())
        };
        let run = harness.run(request, controls);
        tokio::pin!(run);
        let result = loop {
            tokio::select! {
                result = &mut run => break result,
                launch = launches.recv() => launch.expect("driver requested admission").apply(&wrap),
                _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => panic!("{driver} did not request admission"),
            }
        };
        let error = match result {
            Ok(_) => panic!("{driver} ignored the host refusal"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("fixture host refusal"),
            "{driver}: {error}"
        );
        assert_eq!(calls.get(), 1, "{driver}");
        assert!(!marker.exists(), "{driver} started after refusal");
    }
}
