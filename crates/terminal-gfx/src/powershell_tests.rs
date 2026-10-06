//! An isolated real PowerShell/PSReadLine fixture; it does not qualify Windows hardware.
use super::pty::Program;
use super::{KeyIn, Overlay};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terminal_core::proposals::{Phase, Proposal};
use winit::keyboard::{Key as Logical, KeyCode, NamedKey};

fn wait(overlay: &mut Overlay, label: &str, done: impl Fn(&Overlay) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done(overlay) {
        assert!(
            Instant::now() < deadline,
            "{label}: {:?}",
            overlay.focused_text()
        );
        overlay.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "requires isolated PowerShell 7/PSReadLine; set OPENAGENTS_TEST_PWSH"]
fn powershell_keeps_profiles_requests_exact_proposals_and_results() {
    let shell = PathBuf::from(
        std::env::var_os("OPENAGENTS_TEST_PWSH").expect("isolated PowerShell runtime"),
    );
    let root = tempfile::tempdir().unwrap();
    let profile = "Set-StrictMode -Version Latest; $global:fixture_change_prompt=$null; function prompt { if($global:fixture_change_prompt){Set-Location -LiteralPath $global:fixture_change_prompt; $global:fixture_change_prompt=$null}; 'fixture> ' }; function fixture_greeting { 'from fixture' }\n";
    std::fs::write(root.path().join("profile.ps1"), profile).unwrap();
    let repo = root.path().join("Unicode ü repo");
    std::fs::create_dir(&repo).unwrap();
    let mut overlay = Overlay::with(root.path(), shell, Program::Shell);
    overlay.core.paper.on = false;
    overlay.open = true;
    overlay.focused = true;
    overlay.ensure_started();
    wait(&mut overlay, "initial PowerShell prompt", |o| {
        o.panes.values().any(|p| p.session.blocks.at_prompt)
    });
    let directory = format!(
        "Set-Location -LiteralPath '{}'; fixture_greeting\r",
        repo.display().to_string().replace('\'', "''")
    );
    overlay.send(directory.as_bytes());
    wait(&mut overlay, "profile and Unicode directory", |o| {
        o.panes.values().any(|p| {
            p.session
                .blocks
                .records
                .back()
                .is_some_and(|b| b.end.is_some() && b.output.contains("from fixture"))
                && p.session.blocks.at_prompt
        })
    });
    let pane = overlay.focus_id().unwrap();
    let binding = overlay.panes[&pane]
        .session
        .binding("context".into())
        .unwrap();
    assert_eq!(
        std::path::Path::new(&binding.cwd).canonicalize().unwrap(),
        repo.canonicalize().unwrap()
    );
    let before = overlay.panes[&pane].session.blocks.records.len();
    overlay.send("# explain ü without running it\r".as_bytes());
    wait(&mut overlay, "explicit Unicode request", |o| {
        o.smart.draft.is_some()
    });
    assert_eq!(
        overlay.smart.draft.as_ref().unwrap().text,
        "explain ü without running it"
    );
    assert_eq!(overlay.panes[&pane].session.blocks.records.len(), before);
    assert!(overlay.smart.workers.is_empty());
    overlay.smart.draft = None;
    wait(&mut overlay, "request returns to empty prompt", |o| {
        o.panes[&pane].session.blocks.at_prompt
    });
    let command = "Write-Output 'approved_once'".to_owned();
    let proposal = Proposal {
        thread: "fixture-thread".into(),
        id: "fixture-proposal".into(),
        revision: 1,
        command: command.clone(),
        binding: overlay.panes[&pane]
            .session
            .binding("context".into())
            .unwrap(),
    };
    overlay.panes[&pane]
        .session
        .offer_proposal(&proposal)
        .unwrap()
        .unwrap();
    let key = overlay.smart.book.offer(proposal).unwrap();
    overlay.smart.pending = Some((pane, key.clone()));
    let enter = KeyIn {
        code: KeyCode::Enter,
        logical: Logical::Named(NamedKey::Enter),
        text: None,
        plain: None,
        pressed: true,
        repeat: false,
        synthetic: false,
    };
    assert!(overlay.key(&enter));
    assert!(matches!(
        overlay.smart.book.entries[&key].phase,
        Phase::Warned { .. }
    ));
    let mut release = enter.clone();
    release.pressed = false;
    assert!(overlay.key(&release));
    assert!(overlay.key(&enter));
    let Phase::Executing { approval } = overlay.smart.book.entries[&key].phase.clone() else {
        panic!("exact approval did not execute");
    };
    // Keep the fixture offline; acknowledge the real PTY result without a chat helper.
    overlay.smart.execution = None;
    wait(&mut overlay, "one approved command result", |o| {
        o.panes[&pane]
            .session
            .blocks
            .records
            .back()
            .is_some_and(|b| b.command == command && b.end.is_some())
    });
    let block = overlay.panes[&pane]
        .session
        .blocks
        .records
        .back()
        .unwrap()
        .clone();
    assert_eq!(block.status, Some(0));
    assert!(block.output.contains("approved_once"));
    overlay
        .smart
        .book
        .complete(&key, &approval, block.clone())
        .unwrap();
    overlay.smart.book.complete(&key, &approval, block).unwrap();
    assert!(overlay.smart.book.acknowledge(&key, &approval).unwrap());
    assert!(!overlay.smart.book.acknowledge(&key, &approval).unwrap());
    assert_eq!(
        overlay.panes[&pane].session.blocks.records.len(),
        before + 1
    );
    overlay.send(b"$fn={ 'indirect-ok' }; & $fn\r");
    wait(
        &mut overlay,
        "indirect invocation preserves ordinary editor",
        |o| {
            o.panes[&pane]
                .session
                .blocks
                .records
                .back()
                .is_some_and(|b| b.end.is_some() && b.output.contains("indirect-ok"))
        },
    );
    #[cfg(unix)]
    {
        overlay.send(b"/bin/sh -c 'exit 7'\r");
        wait(&mut overlay, "native application exit code", |o| {
            o.panes[&pane]
                .session
                .blocks
                .records
                .back()
                .is_some_and(|b| b.command == "/bin/sh -c 'exit 7'" && b.status == Some(7))
        });
    }
    // A nonfilesystem provider clears eligibility rather than carrying the last filesystem prompt.
    overlay.send(b"$global:fixture_change_prompt='Env:'\r");
    wait(&mut overlay, "provider becomes unavailable", |o| {
        o.focused_text().is_some_and(|s| s.contains("fixture>"))
            && !o.panes[&pane].session.blocks.at_prompt
    });
    overlay.send(b"# provider request must not dispatch\r");
    for _ in 0..20 {
        overlay.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(overlay.smart.draft.is_none());
    assert!(overlay.smart.workers.is_empty());
    assert_eq!(
        std::fs::read_to_string(root.path().join("profile.ps1")).unwrap(),
        profile
    );
    overlay.shutdown();
}
