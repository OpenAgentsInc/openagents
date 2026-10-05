//! fish integration against a real fish under a temporary `HOME` (#10679).

use super::Overlay;
use super::pty::Program;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// `OPENAGENTS_TEST_FISH`, or the first installed fish.
fn fish() -> PathBuf {
    if let Some(fish) = std::env::var_os("OPENAGENTS_TEST_FISH") {
        return PathBuf::from(fish);
    }
    [
        "/opt/homebrew/bin/fish",
        "/usr/local/bin/fish",
        "/usr/bin/fish",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|fish| fish.is_file())
    .expect("set OPENAGENTS_TEST_FISH to an installed fish 3.3 or later")
}

fn wait(overlay: &mut Overlay, what: &str, done: impl Fn(&Overlay) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !done(overlay) {
        assert!(
            Instant::now() < deadline,
            "{what}: {:?}",
            overlay.focused_text()
        );
        overlay.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn last_block(overlay: &Overlay) -> Option<&terminal_core::blocks::Block> {
    overlay.panes.values().next()?.session.blocks.records.back()
}

#[test]
fn fish_hooks_keep_user_configuration_and_make_requests_pending() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join(".config/fish");
    std::fs::create_dir_all(&config).unwrap();
    let rc = concat!(
        "set -g fish_greeting ''\n",
        "function fish_prompt; printf 'fixture> '; end\n",
        "abbr -a fixture_abbr 'echo from an abbreviation'\n",
        "function fixture_function; echo 'from a function'; end\n",
        "bind \\cg 'commandline -r \"echo from a binding\"'\n",
        "if status is-login; set -g fixture_login yes; end\n",
    );
    std::fs::write(config.join("config.fish"), rc).unwrap();
    let mut overlay = Overlay::with(home.path(), fish(), Program::Shell);
    // These tests exercise the panes view.
    overlay.core.paper.on = false;
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    wait(&mut overlay, "shell prompt did not initialize", |overlay| {
        overlay
            .panes
            .values()
            .any(|pane| pane.session.blocks.at_prompt)
    });
    // A function and a failing builtin: one block with the line, its
    // output, its status, and the directory.
    overlay.send(b"fixture_function; false\r");
    wait(&mut overlay, "command did not complete", |overlay| {
        last_block(overlay).is_some_and(|block| block.status == Some(1))
    });
    let block = last_block(&overlay).unwrap();
    assert_eq!(block.command, "fixture_function; false");
    assert!(block.output.contains("from a function"), "{}", block.output);
    assert!(block.cwd.is_some(), "the directory mark arrived");
    let pane = overlay.panes.values().next().unwrap();
    let table = pane.session.blocks.table.clone().unwrap_or_default();
    assert!(table.contains("fixture_function"), "{table}");
    assert!(table.contains("fixture_abbr"), "{table}");
    // A `# ` line is a request: a draft, and no block.
    let before = pane.session.blocks.records.len();
    overlay.send(b"# why did that fail\r");
    wait(&mut overlay, "request hook did not run", |overlay| {
        overlay.smart.draft.is_some()
    });
    let draft = overlay.smart.draft.as_ref().unwrap();
    assert_eq!(draft.text, "why did that fail");
    assert_eq!(draft.context.blocks.len(), 1);
    assert!(overlay.smart.workers.is_empty());
    assert_eq!(
        overlay
            .panes
            .values()
            .next()
            .unwrap()
            .session
            .blocks
            .records
            .len(),
        before
    );
    overlay.smart.draft = None;
    // An abbreviation expands, and a command over several lines is one block
    // that sees the login configuration. fish 4 marks its own prompt, so the
    // block's command is the text fish echoed, every line of it.
    overlay.send(b"fixture_abbr\r");
    wait(&mut overlay, "abbreviation did not run", |overlay| {
        last_block(overlay).is_some_and(|block| block.output.contains("from an abbreviation"))
    });
    overlay.send(b"if true\r");
    overlay.send(b"echo continued login=$fixture_login\r");
    overlay.send(b"end\r");
    wait(
        &mut overlay,
        "continued command did not complete",
        |overlay| {
            last_block(overlay).is_some_and(|block| {
                block.command.starts_with("if true") && block.status == Some(0)
            })
        },
    );
    let block = last_block(&overlay).unwrap();
    assert!(
        block.output.contains("continued login=yes"),
        "{}",
        block.output
    );
    // The user's own binding still works.
    overlay.send(b"\x07\r");
    wait(&mut overlay, "binding did not run", |overlay| {
        last_block(overlay).is_some_and(|block| block.output.contains("from a binding"))
    });
    assert_eq!(
        overlay
            .panes
            .values()
            .next()
            .unwrap()
            .session
            .blocks
            .records
            .len(),
        before + 3
    );
    assert_eq!(
        std::fs::read_to_string(config.join("config.fish")).unwrap(),
        rc
    );
}

#[test]
fn a_users_enter_binding_is_kept() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join(".config/fish");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("config.fish"),
        "set -g fish_greeting ''\nbind \\r 'commandline -r \"echo user enter\"; commandline -f execute'\n",
    )
    .unwrap();
    let mut overlay = Overlay::with(home.path(), fish(), Program::Shell);
    overlay.core.paper.on = false;
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    wait(&mut overlay, "shell prompt did not initialize", |overlay| {
        overlay
            .panes
            .values()
            .any(|pane| pane.session.blocks.at_prompt)
    });
    overlay.send(b"# not a request\r");
    wait(&mut overlay, "the user's Enter did not run", |overlay| {
        last_block(overlay).is_some_and(|block| block.output.contains("user enter"))
    });
    assert!(overlay.smart.draft.is_none());
}
