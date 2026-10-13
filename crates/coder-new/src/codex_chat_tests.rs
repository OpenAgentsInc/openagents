//! The Codex chat's process (#11510).

use super::*;

/// The thread opens with Coder's identity naming Codex's model menu, the
/// worktree rule inside a session worktree, and the briefing in place of
/// Codex's own `AGENTS.md` reading.
#[test]
fn the_thread_opens_with_coders_identity_and_the_briefing() {
    let text = instructions(
        &crate::claude_tools::Tools::default(),
        "gpt-6-astra",
        None,
        std::path::Path::new("/repo/.coder/worktrees/chat-abc"),
    );
    assert!(text.starts_with("You are Coder, OpenAgents' coding agent."));
    assert!(text.contains("this chat runs GPT-6-Astra"), "{text}");
    assert!(text.contains("Codex offers GPT-6.1-Sol"), "{text}");
    assert!(text.contains(crate::checkout_guard::INSTRUCTIONS.trim()));
    let plain = instructions(
        &crate::claude_tools::Tools::default(),
        crate::codex::MODEL,
        None,
        std::path::Path::new("/repo"),
    );
    assert!(!plain.contains(crate::checkout_guard::INSTRUCTIONS.trim()));
    // Token savers (on in a test): noisy commands run through the
    // squeezer from the start (#11525).
    assert!(
        plain.contains(&format!("{} '<command>'", squeeze::CODER_SUBCOMMAND)),
        "{plain}"
    );
}

/// Waits for turn `turn` to end.
fn finish_turn(chat: &mut Chat, turn: u64) -> Result<Streamed, String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    while std::time::Instant::now() < deadline {
        for event in chat.poll() {
            match event {
                Event::TurnEnd {
                    turn: ended,
                    result,
                } if ended >= turn => return result,
                Event::Runtime {
                    event:
                        RuntimeEvent::Tool {
                            name,
                            input,
                            running: false,
                            output,
                        },
                    ..
                } => println!("  tool {name} {input} -> {output}"),
                Event::Exited => return Err("exited".into()),
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err("timed out".into())
}

/// One real chat on this computer's Codex login, in a throwaway checkout:
/// a question, an edit with its check, a follow-up on the same thread, and
/// a merge the floors hold (nobody can answer from a test, so it is held).
/// Run by hand:
/// `cargo test -p coder-new --lib a_live_codex_chat -- --ignored --nocapture`.
#[test]
#[ignore = "needs a Codex login"]
fn a_live_codex_chat() {
    crate::codex::set_test_program(crate::acp_discovery::resolve(
        std::path::Path::new("codex"),
        &|name| std::env::var_os(name),
    ));
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::process::Command::new("git")
        .args(["init", "-q"])
        .arg(repo)
        .status()
        .unwrap();
    std::fs::write(repo.join("add.sh"), "echo $(( $1 + $2 ))\n").unwrap();
    let first = "What does add.sh do? One sentence.";
    let mut chat = Chat::start(
        repo.to_owned(),
        crate::claude_tools::Tools::default(),
        None,
        first.into(),
        first.into(),
        None,
        "gpt-6.1-sol".into(),
    );
    let reply = finish_turn(&mut chat, 1).unwrap();
    println!("1: {} on {}", reply.text, reply.model);
    let thread = chat.session.clone().expect("the thread was named");

    let turn = chat.turn();
    chat.say(
        "Add a file check.sh that runs `sh add.sh 2 3` and exits 1 unless it prints 5. Run it.",
        false,
    );
    let reply = finish_turn(&mut chat, turn).unwrap();
    println!("2: {}", reply.text);
    assert!(repo.join("check.sh").is_file(), "the edit landed");
    assert_eq!(
        chat.session.as_deref(),
        Some(thread.as_str()),
        "same thread"
    );

    let turn = chat.turn();
    chat.say("What file did you just add? One word.", false);
    let reply = finish_turn(&mut chat, turn).unwrap();
    println!("3: {}", reply.text);
    assert!(reply.text.contains("check.sh"), "{}", reply.text);

    let turn = chat.turn();
    chat.say("Run exactly `gh pr merge 5 --squash` now.", false);
    let reply = finish_turn(&mut chat, turn).unwrap();
    println!("4: {}", reply.text);
    assert_eq!(
        chat.session.as_deref(),
        Some(thread.as_str()),
        "same thread"
    );
}

/// #11552: a Codex chat against a stand-in `codex app-server` that plays
/// back a real turn (codex-cli 0.162.1, recorded 2026-10-13 with the
/// reasoning summary on): the summary streams in as thinking, the turn's
/// call names its model and how long it took, and Codex's plan windows
/// are kept.
#[test]
fn a_recorded_codex_turn_streams_its_thinking_and_names_its_call() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = format!(
        "{}/fixtures/thinking/codex-app-server-turn.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    // Coder asks in order: initialize (1), initialized, thread/start (2),
    // turn/start (3); then the recorded turn plays, a beat before its
    // usage so the call takes time.
    let script = format!(
        r#"#!/bin/sh
read -r line
printf '%s\n' '{{"jsonrpc":"2.0","id":1,"result":{{"userAgent":"codex_cli_rs/0.162.1 (test)"}}}}'
read -r line
read -r line
printf '%s\n' '{{"jsonrpc":"2.0","id":2,"result":{{"thread":{{"id":"thr-1"}},"model":"gpt-6.1-sol"}}}}'
read -r line
printf '%s\n' '{{"jsonrpc":"2.0","id":3,"result":{{"turn":{{"id":"turn-1"}}}}}}'
while IFS= read -r event; do
  case "$event" in *tokenUsage*) sleep 0.1;; esac
  printf '%s\n' "$event"
done < '{fixture}'
while read -r line; do :; done
"#
    );
    let program = dir.path().join("codex");
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(
        &program,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    let mut chat = Chat::start_on(
        Some(program),
        dir.path().to_owned(),
        crate::claude_tools::Tools::default(),
        None,
        "Is 9991 prime?".into(),
        "Is 9991 prime?".into(),
        None,
        "gpt-6.1-sol".into(),
    );
    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let result = loop {
        assert!(std::time::Instant::now() < deadline, "{events:?}");
        let mut ended = None;
        for event in chat.poll() {
            match event {
                Event::TurnEnd { result, .. } => ended = Some(result),
                Event::Runtime { event, .. } => events.push(event),
                _ => {}
            }
        }
        if let Some(result) = ended {
            break result;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let reply = result.unwrap();
    assert!(reply.text.contains("9991"), "{}", reply.text);
    let (shown, pieces) = crate::claude_stream::shown_in_chat(&events);
    assert!(pieces.len() > 10, "{pieces:?}");
    let entries = crate::claude_stream::thinking_entries(&shown);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert!(entries[0].0.starts_with("**Verifying a multiplication**"));
    assert!(!entries[0].1, "closed");
    let usage = events
        .iter()
        .find_map(|event| match event {
            RuntimeEvent::Usage(usage) => Some(usage.clone()),
            _ => None,
        })
        .expect("the call's usage");
    assert_eq!(usage.model, "gpt-6.1-sol");
    assert!(usage.milliseconds >= 100, "{usage:?}");
    assert!(usage.reasoning_tokens.is_some_and(|tokens| tokens > 0));
}

/// #11552, by hand on a computer with a Codex login: one real turn shows
/// Codex's reasoning summary streaming, the call's model and time, and the
/// rail's plan windows after it.
/// `cargo test -p coder-new --lib a_live_codex_turn_thinks -- --ignored --nocapture`.
#[test]
#[ignore = "needs a Codex login"]
fn a_live_codex_turn_thinks() {
    let program = crate::acp_discovery::resolve(std::path::Path::new("codex"), &|name| {
        std::env::var_os(name)
    });
    let dir = tempfile::tempdir().unwrap();
    let ask = "Is 9991 prime? Think it through briefly, answer in one line.";
    let mut chat = Chat::start_on(
        program,
        dir.path().to_owned(),
        crate::claude_tools::Tools::default(),
        None,
        ask.into(),
        ask.into(),
        None,
        "gpt-6.1-sol".into(),
    );
    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    let result = loop {
        assert!(std::time::Instant::now() < deadline);
        let mut ended = None;
        for event in chat.poll() {
            match event {
                Event::TurnEnd { result, .. } => ended = Some(result),
                Event::Runtime { event, .. } => {
                    match &event {
                        RuntimeEvent::ThinkingText(piece) => println!("thinking+ {piece:?}"),
                        RuntimeEvent::Thinking(on) => println!("rail Thinking {on}"),
                        RuntimeEvent::Usage(usage) => println!("call {usage:?}"),
                        _ => {}
                    }
                    events.push(event);
                }
                _ => {}
            }
        }
        if let Some(result) = ended {
            break result;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let reply = result.unwrap();
    let (shown, pieces) = crate::claude_stream::shown_in_chat(&events);
    println!("reply: {}", reply.text);
    println!(
        "thinking entries: {:?}",
        crate::claude_stream::thinking_entries(&shown)
    );
    let now = coder_delegate::limit::now();
    println!(
        "rail: {} · {}",
        crate::codex::rail_label(),
        crate::codex::windows_text(now)
    );
    assert!(!pieces.is_empty(), "the summary streamed");
}
