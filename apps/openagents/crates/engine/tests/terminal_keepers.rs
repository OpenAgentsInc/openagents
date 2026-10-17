//! Terminal keepers (#11951): an open terminal outlives its engine, the next
//! engine picks it up under the same id with no gap in its output, and a kept
//! terminal doesn't hold back an engine update.
#![cfg(unix)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use openagents_engine::{EngineCore, HarnessRegistry};
use openagents_harness::keeper;
use openagents_proto::{HarnessId, TerminalEvent};

/// Runs the keeper when this test binary is started as one.
#[test]
fn keeper_entry() {
    // The keeper finds its run folder in this variable; nothing else sets it.
    if std::env::var_os("OPENAGENTS_KEEPER_DIR").is_some() {
        std::process::exit(keeper::main(std::iter::empty()));
    }
}

const WAIT: Duration = Duration::from_secs(20);

fn assemble(dir: &Path) -> EngineCore {
    openagents_engine::set_keeper_launch(
        std::env::current_exe().unwrap(),
        ["--exact", "keeper_entry", "--nocapture", "--test-threads=1"]
            .map(Into::into)
            .to_vec(),
    );
    EngineCore::assemble(dir, Arc::new(HarnessRegistry::new()), HarnessId::Mock, None).unwrap()
}

fn keepers_root(dir: &Path) -> PathBuf {
    dir.join("orgs/dev-org/dev-user/keepers")
}

/// A shell that reads nothing from the real home directory.
fn open(core: &EngineCore, dir: &Path) -> String {
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let environment = HashMap::from([
        ("HOME".to_owned(), home.display().to_string()),
        ("ENV".to_owned(), String::new()),
        ("PS1".to_owned(), "$ ".to_owned()),
    ]);
    core.terminals
        .open_with_shell_and_environment(
            &dir.display().to_string(),
            80,
            24,
            Some("/bin/sh"),
            &environment,
        )
        .unwrap()
        .id
}

fn send(core: &EngineCore, id: &str, line: &str) {
    core.terminals
        .write(id, &BASE64.encode(format!("{line}\n")))
        .unwrap();
}

/// What a viewer has seen of one terminal.
#[derive(Default)]
struct Seen {
    output: Vec<u8>,
    last_seq: u64,
}

impl Seen {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.output).into_owned()
    }

    async fn until(
        &mut self,
        events: &mut tokio::sync::mpsc::UnboundedReceiver<TerminalEvent>,
        what: &str,
        done: impl Fn(&str) -> bool,
    ) {
        let deadline = tokio::time::Instant::now() + WAIT;
        while !done(&self.text()) {
            let event = tokio::time::timeout_at(deadline, events.recv())
                .await
                .unwrap_or_else(|_| panic!("timed out waiting for {what}: {:?}", self.text()))
                .unwrap_or_else(|| panic!("terminal ended before {what}: {:?}", self.text()));
            match event {
                TerminalEvent::Data { seq, data } => {
                    assert!(seq > self.last_seq, "events arrive in order");
                    self.last_seq = seq;
                    self.output.extend(BASE64.decode(data).unwrap());
                }
                TerminalEvent::Exit { .. } => panic!("the shell exited: {:?}", self.text()),
            }
        }
    }
}

/// The numbered lines `L1`… the loop printed, in the order they arrived.
fn numbered(text: &str) -> Vec<u32> {
    text.lines()
        .filter_map(|line| line.trim_end_matches('\r').strip_prefix('L')?.parse().ok())
        .collect()
}

/// The shell's process id, from a line `PID-<pid>`.
fn shell_pid(text: &str) -> u32 {
    text.lines()
        .filter_map(|line| {
            line.trim_end_matches('\r')
                .strip_prefix("PID-")?
                .parse()
                .ok()
        })
        .last()
        .expect("the shell printed its pid")
}

fn alive(pid: u32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

fn journal_output(root: &Path) -> String {
    let mut output = Vec::new();
    for found in keeper::client::discover(root) {
        let text = std::fs::read_to_string(found.dir.join("journal.jsonl")).unwrap_or_default();
        for line in text.lines() {
            if let Ok(keeper::Frame {
                body: keeper::FrameBody::Stdout { line },
                ..
            }) = serde_json::from_str(line)
            {
                output.extend(BASE64.decode(line).unwrap());
            }
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn wait_for(what: &str, mut check: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + WAIT;
    while !check() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Engine A opens a terminal and starts a loop in it, then goes away while
/// the loop still prints. The shell keeps printing with no engine. Engine B
/// picks the terminal up: a viewer resuming where it left off sees every line
/// exactly once, input reaches the same shell, and the kept terminal never
/// holds back a handoff.
fn picks_up(graceful: bool) {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().to_path_buf();
    let root = keepers_root(&dir);

    // Engine A.
    let (id, seen, pid) = {
        let dir = dir.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let (core, id, seen, pid) = runtime.block_on(async {
                let core = assemble(&dir);
                let id = open(&core, &dir);
                let mut events = core.terminals.subscribe(&id, None).unwrap();
                let mut seen = Seen::default();
                send(&core, &id, "echo PID-$$");
                seen.until(&mut events, "the pid", |text| {
                    text.lines().any(|l| l.starts_with("PID-"))
                })
                .await;
                let pid = shell_pid(&seen.text());
                // A kept terminal doesn't count as open for an update.
                assert!(!core.terminals.any_open());
                assert!(core.terminals.any_live());
                assert!(core.sessions.handoff_ready());
                send(
                    &core,
                    &id,
                    "for i in $(seq 1 60); do echo L$i; sleep 0.05; done",
                );
                seen.until(&mut events, "the loop to start", |text| {
                    numbered(text).len() >= 5
                })
                .await;
                (core, id, seen, pid)
            });
            if graceful {
                runtime.block_on(core.shutdown());
            }
            // Tasks stop where they are, as in a killed engine.
            drop(runtime);
            drop(core);
            (id, seen, pid)
        })
        .join()
        .unwrap()
    };
    assert!(alive(pid), "the shell outlives its engine");
    assert!(
        numbered(&seen.text()).len() < 60,
        "the loop was still running"
    );

    // No engine: the shell goes on printing, and its keeper journals it.
    wait_for("the loop to finish with no engine", || {
        numbered(&journal_output(&root)).last() == Some(&60)
    });

    // Engine B.
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let core = assemble(&dir);
            let mut events = core
                .terminals
                .subscribe(&id, Some(seen.last_seq))
                .expect("the terminal keeps its id");
            let mut resumed = Seen {
                output: seen.output.clone(),
                last_seq: seen.last_seq,
            };
            resumed
                .until(&mut events, "the rest of the loop", |text| {
                    numbered(text).last() == Some(&60)
                })
                .await;
            assert_eq!(
                numbered(&resumed.text()),
                (1..=60).collect::<Vec<_>>(),
                "every line exactly once"
            );

            // Input reaches the same shell.
            send(&core, &id, "echo PID-$$; echo DONE-$((6*7))");
            resumed
                .until(&mut events, "the same shell to answer", |text| {
                    text.contains("DONE-42")
                })
                .await;
            assert_eq!(shell_pid(&resumed.text()), pid);

            // A resize goes to the keeper's PTY.
            core.terminals.resize(&id, 101, 33).unwrap();
            send(&core, &id, "stty size");
            resumed
                .until(&mut events, "the new size", |text| text.contains("33 101"))
                .await;

            assert!(!core.terminals.any_open());
            assert!(core.sessions.handoff_ready());

            // Closing it ends the shell and removes the keeper's folder.
            core.terminals.close(&id).unwrap();
            let deadline = tokio::time::Instant::now() + WAIT;
            while alive(pid) || !keeper::client::discover(&keepers_root(&dir)).is_empty() {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "closing ends the keeper"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(!core.terminals.any_live());
        });
    })
    .join()
    .unwrap();
}

#[test]
fn a_terminal_outlives_a_killed_engine() {
    picks_up(false);
}

#[test]
fn a_terminal_outlives_an_engine_that_quits() {
    picks_up(true);
}

/// A kept shell that exits with no engine is cleaned up by the next one.
#[test]
fn an_exited_shell_is_picked_up_as_ended() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().to_path_buf();
    let id = {
        let dir = dir.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let (core, id) = runtime.block_on(async {
                let core = assemble(&dir);
                let id = open(&core, &dir);
                let mut events = core.terminals.subscribe(&id, None).unwrap();
                send(&core, &id, "echo READY");
                Seen::default()
                    .until(&mut events, "the shell", |text| text.contains("READY\r\n"))
                    .await;
                send(&core, &id, "sleep 0.3; exit 7");
                (core, id)
            });
            drop(runtime);
            drop(core);
            id
        })
        .join()
        .unwrap()
    };
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            tokio::time::sleep(Duration::from_millis(800)).await;
            let core = assemble(&dir);
            let mut events = core.terminals.subscribe(&id, None).unwrap();
            let exit = tokio::time::timeout(WAIT, async {
                while let Some(event) = events.recv().await {
                    if let TerminalEvent::Exit { exit_code, .. } = event {
                        return exit_code;
                    }
                }
                panic!("the terminal ended without its exit");
            })
            .await
            .unwrap();
            assert_eq!(exit, 7);
            core.terminals.close(&id).unwrap();
            let deadline = tokio::time::Instant::now() + WAIT;
            while !keeper::client::discover(&keepers_root(&dir)).is_empty() {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "closing ends the keeper"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
    })
    .join()
    .unwrap();
}

/// `OPENAGENTS_KEEPERS=0` opens terminals directly, and they hold an update
/// back as before. Runs in its own process so the variable reaches no other test.
#[test]
fn keepers_can_be_turned_off() {
    if std::env::var_os("TERMINAL_KEEPERS_OFF").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "keepers_can_be_turned_off", "--test-threads=1"])
            .env("TERMINAL_KEEPERS_OFF", "1")
            .env(keeper::ENV, "0")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let core = assemble(dir.path());
        let id = open(&core, dir.path());
        assert!(core.terminals.any_open());
        assert!(keeper::client::discover(&keepers_root(dir.path())).is_empty());
        core.terminals.close(&id).unwrap();
    });
}
