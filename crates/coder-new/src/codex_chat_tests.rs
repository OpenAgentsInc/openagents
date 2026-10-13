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
