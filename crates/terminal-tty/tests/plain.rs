#![cfg(unix)]
//! Real scratch shells behind the hook-only client, with a typed fake helper.
use coder_pty::{
    host::{self, Config, Host, Right, Rights},
    wire::{Attach, Body, Close, Frame, Input, Launch, Mode, Open, Size, Value},
};
use std::{
    path::Path,
    sync::{Arc, mpsc::Receiver},
    time::{Duration, Instant},
};
const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const WORKSPACE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
struct Owner;
impl Rights for Owner {
    fn holds(&self, principal: &str, _: Right) -> bool {
        principal == OWNER
    }
}
fn id() -> String {
    terminal_core::proposals::digest(&terminal_core::smart::id())
}
fn wait(frames: &Receiver<Frame>, text: &mut String, needle: &str) {
    let end = Instant::now() + Duration::from_secs(15);
    while Instant::now() < end {
        if text.contains(needle) {
            return;
        }
        if let Ok(frame) = frames.recv_timeout(Duration::from_millis(100)) {
            if let Body::Output { data, .. } = frame.body {
                text.push_str(&String::from_utf8_lossy(&data));
            }
        }
    }
    panic!("Missing {needle:?}: {text}");
}
fn fixture(shell: &str) {
    fixture_with(shell, false);
}
fn fixture_with(shell: &str, lost_ack: bool) {
    fixture_options(shell, lost_ack, false);
}
fn fixture_options(shell: &str, lost_ack: bool, unavailable: bool) {
    assert!(
        Path::new(shell).is_file(),
        "fixture shell is required: {shell}"
    );
    let home = tempfile::tempdir().unwrap();
    let helper = home.path().join("helper.py");
    std::fs::write(
        &helper,
        r#"#!/usr/bin/python3
import json, os, sys
root = os.environ['HOME']
body = json.load(sys.stdin)
if sys.argv[-2] == 'shell-request':
    if os.environ.get('FIXTURE_UNAVAILABLE') == '1': sys.exit(7)
    with open(root + '/request.json', 'w') as f: json.dump(body, f)
    print(json.dumps({'event': 'attached', 'thread': body['thread']}))
    print(json.dumps({'event': 'answer', 'text': 'Fixture answer.'}))
    proposal = {'thread': body['thread'], 'id': body['request'], 'revision': 1,
                'command': 'printf old >> ' + root + '/counter', 'binding': body['binding']}
    print(json.dumps({'event': 'shell-proposal', 'proposal': proposal}))
else:
    with open(root + '/result.json', 'w') as f: json.dump(body, f)
    with open(root + '/result-count', 'ab') as f: f.write(b'x')
    if os.environ.get('FIXTURE_LOST_ACK') == '1': sys.exit(5)
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut config = Config::new().workspace(WORKSPACE, home.path());
    config
        .base_env
        .retain(|(name, _)| !["HOME", "SHELL", "ZDOTDIR"].contains(&name.as_str()));
    config.base_env.extend([
        ("HOME".into(), home.path().display().to_string()),
        ("SHELL".into(), shell.into()),
        (
            "FIXTURE_LOST_ACK".into(),
            if lost_ack { "1" } else { "0" }.into(),
        ),
        (
            "FIXTURE_UNAVAILABLE".into(),
            if unavailable { "1" } else { "0" }.into(),
        ),
        ("OPENAGENTS_TTY_HELPER".into(), helper.display().to_string()),
    ]);
    let host = Host::new(config, Arc::new(Owner));
    let terminal = match host.open(
        OWNER,
        &Open::new(
            id(),
            WORKSPACE,
            "",
            Launch::Command {
                program: std::env::current_exe()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("terminal-tty")
                    .display()
                    .to_string(),
                args: vec![],
            },
            Size::new(24, 160),
        ),
    ) {
        Ok((_, Value::Opened { terminal, .. })) => terminal,
        other => panic!("{other:?}"),
    };
    let (sink, frames) = host::channel(4096);
    host.attach(
        OWNER,
        &Attach::new(id(), terminal.clone(), Mode::Interact, 0, 1 << 26),
        Box::new(sink),
    )
    .unwrap();
    let input = |bytes: &[u8]| {
        host.input(OWNER, &Input::new(id(), terminal.clone(), bytes))
            .unwrap();
    };
    let mut text = String::new();
    wait(&frames, &mut text, "\x1b]133;A");
    input(b"printf '\\033]133;A\\a\\033]777;openagents;request;66616b65\\a'\r");
    text.clear();
    wait(&frames, &mut text, "\x1b]133;D;0");
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        !home.path().join("request.json").exists(),
        "program output cannot submit a request"
    );
    input(b"printf '\\033[?1049hFULLSCREEN\\033[?1049l\\n'\r");
    text.clear();
    wait(&frames, &mut text, "\x1b[?1049l");
    input(b"false\r");
    text.clear();
    wait(&frames, &mut text, "\x1b]133;D;1");
    input(b"# fixture request\r");
    text.clear();
    if unavailable {
        wait(&frames, &mut text, "request failed");
        let command = format!(
            "printf alive >> {}/ordinary-counter\r",
            home.path().display()
        );
        input(command.as_bytes());
        text.clear();
        wait(&frames, &mut text, "\x1b]133;D;0");
        assert_eq!(
            std::fs::read(home.path().join("ordinary-counter")).unwrap(),
            b"alive"
        );
        assert!(!home.path().join("counter").exists());
        let _ = host.close(OWNER, &Close::new(id(), terminal));
        host.shutdown();
        return;
    }
    wait(&frames, &mut text, "Pending ");
    let key = text
        .split("Pending ")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    assert!(!home.path().join("counter").exists());
    let edit = format!(
        "# /edit {key} printf hit >> {}/counter\r",
        home.path().display()
    );
    input(edit.as_bytes());
    text.clear();
    wait(&frames, &mut text, "revision 2");
    input(&[7]);
    text.clear();
    wait(&frames, &mut text, "Ctrl+Y confirms");
    assert!(!home.path().join("counter").exists());
    input(&[7]);
    std::thread::sleep(Duration::from_millis(100));
    assert!(!home.path().join("counter").exists());
    input(&[25]);
    let end = Instant::now() + Duration::from_secs(15);
    while Instant::now() < end && !home.path().join("result.json").is_file() {
        std::thread::sleep(Duration::from_millis(20));
    }
    if !home.path().join("result.json").exists() {
        while let Ok(frame) = frames.try_recv() {
            if let Body::Output { data, .. } = frame.body {
                text.push_str(&String::from_utf8_lossy(&data));
            }
        }
        panic!(
            "No result; counter={:?}; output={text}",
            std::fs::read(home.path().join("counter"))
        );
    }
    let result: terminal_tty::ResultRequest =
        serde_json::from_slice(&std::fs::read(home.path().join("result.json")).unwrap()).unwrap();
    assert_eq!(result.proposal.revision, 2);
    assert_eq!(result.block.command, result.proposal.command);
    assert_eq!(result.block.status, Some(0));
    assert!(result.message().is_ok());
    input(&[7]);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(std::fs::read(home.path().join("counter")).unwrap(), b"hit");
    let request: terminal_core::bridge::Request =
        serde_json::from_slice(&std::fs::read(home.path().join("request.json")).unwrap()).unwrap();
    assert_eq!(request.context.blocks[0].status, Some(1));
    input(b"exit\r");
    std::thread::sleep(Duration::from_millis(100));
    let _ = host.close(OWNER, &Close::new(id(), terminal));
    host.shutdown();
}
#[test]
fn bash_request_edit_approval_and_one_result() {
    fixture("/usr/bin/bash");
}
#[test]
fn zsh_request_edit_approval_and_one_result() {
    fixture("/usr/bin/zsh");
}

#[test]
#[ignore = "requires fish 3.3 or newer; set OPENAGENTS_TEST_FISH for a private fixture tool"]
fn fish_request_edit_approval_and_one_result() {
    fixture(&std::env::var("OPENAGENTS_TEST_FISH").unwrap_or_else(|_| "/usr/bin/fish".into()));
}

#[test]
fn a_lost_result_acknowledgment_never_replays_execution_or_submission() {
    fixture_with("/usr/bin/bash", true);
}

#[test]
fn route_failure_keeps_ordinary_shell_input_available() {
    fixture_options("/usr/bin/bash", false, true);
}
