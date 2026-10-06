//! bash integration against a real bash under a temporary `HOME` (#10678).

use super::Overlay;
use super::pty::Program;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// bash's version as `major * 100 + minor`.
fn version(bash: &Path) -> Option<u32> {
    let output = std::process::Command::new(bash)
        .args([
            "--norc",
            "--noprofile",
            "-c",
            "echo $((BASH_VERSINFO[0] * 100 + BASH_VERSINFO[1]))",
        ])
        .env_clear()
        .output()
        .ok()?;
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

/// A bash with the hooks' features: `OPENAGENTS_TEST_BASH`, or the first
/// installed bash 4.4 or later.
fn modern_bash() -> PathBuf {
    if let Some(bash) = std::env::var_os("OPENAGENTS_TEST_BASH") {
        return PathBuf::from(bash);
    }
    [
        "/opt/homebrew/bin/bash",
        "/usr/local/bin/bash",
        "/usr/bin/bash",
        "/bin/bash",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|bash| bash.is_file() && version(bash).is_some_and(|v| v >= 404))
    .expect("set OPENAGENTS_TEST_BASH to an installed bash 4.4 or later")
}

fn wait(overlay: &mut Overlay, what: &str, done: impl Fn(&Overlay) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
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

fn start(home: &Path, bash: PathBuf) -> Overlay {
    let mut overlay = Overlay::with(home, bash, Program::Shell);
    // These tests exercise the panes view.
    overlay.core.paper.on = false;
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    overlay
}

fn last_status(overlay: &Overlay) -> Option<i32> {
    overlay
        .panes
        .values()
        .next()?
        .session
        .blocks
        .records
        .back()?
        .status
}

#[test]
fn bash_hooks_keep_user_configuration_and_make_requests_pending() {
    let home = tempfile::tempdir().unwrap();
    let profile = "fixture_login=yes\n. \"$HOME/.bashrc\"\n";
    let rc = concat!(
        "PS1='fixture> '\n",
        "alias fixture_greeting='echo hello'\n",
        "fixture_function() { echo \"from a function\"; }\n",
        "fixture_prompts=0\n",
        "PROMPT_COMMAND='fixture_prompts=$((fixture_prompts + 1))'\n",
        "set -o vi\n",
    );
    std::fs::write(home.path().join(".bash_profile"), profile).unwrap();
    std::fs::write(home.path().join(".bashrc"), rc).unwrap();
    let mut overlay = start(home.path(), modern_bash());
    wait(&mut overlay, "shell prompt did not initialize", |overlay| {
        overlay
            .panes
            .values()
            .any(|pane| pane.session.blocks.at_prompt)
    });
    // An alias and a failing builtin: one block with the accepted line, its
    // output, and its status.
    overlay.send(b"fixture_greeting; false\r");
    wait(&mut overlay, "command did not complete", |overlay| {
        last_status(overlay) == Some(1)
    });
    let pane = overlay.panes.values().next().unwrap();
    let block = pane.session.blocks.records.back().unwrap();
    assert_eq!(block.command, "fixture_greeting; false");
    assert!(block.output.contains("hello"), "{}", block.output);
    assert!(block.cwd.is_some(), "the directory mark arrived");
    let table = pane.session.blocks.table.clone().unwrap_or_default();
    assert!(table.contains("fixture_greeting"), "{table}");
    assert!(table.contains("fixture_function"), "{table}");
    // A `# ` line is a request: a draft, and no block or command.
    let before = pane.session.blocks.records.len();
    overlay.send(b"# why did that fail\r");
    wait(&mut overlay, "request hook did not run", |overlay| {
        overlay.smart.draft.is_some()
    });
    let draft = overlay.smart.draft.as_ref().unwrap();
    assert_eq!(draft.text, "why did that fail");
    assert_eq!(draft.context.blocks.len(), 1);
    assert!(overlay.smart.workers.is_empty());
    let pane = overlay.panes.values().next().unwrap();
    assert_eq!(pane.session.blocks.records.len(), before);
    overlay.smart.draft = None;
    // A command continued over lines is one block, and the function, the
    // login profile, and the user's prompt hook all still work.
    overlay.send(b"if true; then\r");
    overlay.send(b"fixture_function; echo \"login=$fixture_login prompts=$fixture_prompts\"\r");
    overlay.send(b"fi\r");
    wait(
        &mut overlay,
        "continued command did not complete",
        |overlay| {
            overlay
                .panes
                .values()
                .next()
                .unwrap()
                .session
                .blocks
                .records
                .back()
                .is_some_and(|block| block.command == "if true; then" && block.status == Some(0))
        },
    );
    let pane = overlay.panes.values().next().unwrap();
    let block = pane.session.blocks.records.back().unwrap();
    assert!(block.output.contains("from a function"), "{}", block.output);
    assert!(block.output.contains("login=yes"), "{}", block.output);
    assert!(!block.output.contains("prompts=0"), "{}", block.output);
    assert_eq!(pane.session.blocks.records.len(), before + 1);
    // The user's files are untouched.
    assert_eq!(
        std::fs::read_to_string(home.path().join(".bashrc")).unwrap(),
        rc
    );
    assert_eq!(
        std::fs::read_to_string(home.path().join(".bash_profile")).unwrap(),
        profile
    );
}

#[test]
fn an_older_bash_runs_the_users_files_without_hooks() {
    let bash = PathBuf::from("/bin/bash");
    if !bash.is_file() || version(&bash).is_none_or(|v| v >= 404) {
        // No bash older than 4.4 here to check.
        return;
    }
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join(".bash_profile"),
        "PS1='old> '\nalias fixture_greeting='echo hello from old bash'\n",
    )
    .unwrap();
    let mut overlay = start(home.path(), bash);
    overlay.send(b"fixture_greeting\r");
    wait(&mut overlay, "the alias did not run", |overlay| {
        overlay
            .focused_text()
            .is_some_and(|text| text.contains("hello from old bash"))
    });
    let pane = overlay.panes.values().next().unwrap();
    assert!(!pane.session.blocks.at_prompt);
    assert!(pane.session.blocks.records.is_empty());
}

#[test]
fn the_startup_file_reads_the_login_profile_or_the_rc_file() {
    let bash = modern_bash();
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join(".bash_profile"), "fixture_from=profile\n").unwrap();
    std::fs::write(home.path().join(".bashrc"), "fixture_from=rc\n").unwrap();
    let hooks = super::integration::Integration::bash().unwrap();
    for (login, expected) in [(true, "profile"), (false, "rc")] {
        let start = hooks.bash_start(login);
        let (args, env) = (start.args, start.env);
        let output = std::process::Command::new(&bash)
            .args(&args)
            .args([
                "-i",
                "-c",
                "echo \"from=$fixture_from hooks=$_openagents_integrated\"",
            ])
            .env_clear()
            .env("HOME", home.path())
            .env("PATH", "/usr/bin:/bin")
            .envs(env)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(&format!("from={expected} hooks=1")),
            "login={login}: {stdout} {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
