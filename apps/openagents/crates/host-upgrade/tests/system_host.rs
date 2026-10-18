//! The real host's process handling (#12056), with stand-in app bundles
//! whose binary is a shell script: the relauncher starts the new app only
//! after the old one exits, a process the app started (as a keeper) keeps
//! running through it, and a new app that never starts is rolled back.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use openagents_host_upgrade::Host;
use openagents_host_upgrade::layout::{BUNDLE_NAME, COMMIT_FILE, Layout};
use openagents_host_upgrade::record::{self, Outcome, Pending, Phase};
use openagents_host_upgrade::system::{self, SystemHost};

/// A bundle whose app writes `<marks>/<name>.pid`, starts a keeper that
/// writes `keeper.pid` (first app only), and sleeps.
fn make_app(bundle: &Path, name: &str, commit: &str, marks: &Path, starts: bool) {
    let macos = bundle.join("Contents").join("MacOS");
    let resources = bundle.join("Contents").join("Resources");
    std::fs::create_dir_all(&macos).unwrap();
    std::fs::create_dir_all(&resources).unwrap();
    std::fs::write(resources.join(COMMIT_FILE), commit).unwrap();
    let body = if starts {
        format!(
            "#!/bin/sh\nif [ \"$1\" = keeper ]; then echo $$ > '{marks}/keeper.pid'; exec sleep 60; fi\n\
             echo $$ > '{marks}/{name}.pid'\n\
             [ -f '{marks}/keeper.pid' ] || (\"$0\" keeper &)\n\
             exec sleep 60\n",
            marks = marks.display()
        )
    } else {
        "#!/bin/sh\nexit 3\n".to_owned()
    };
    let binary = Layout::binary(bundle);
    std::fs::write(&binary, body).unwrap();
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn wait_for(path: &Path) -> u32 {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(20) {
        if let Some(pid) = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| text.trim().parse().ok())
        {
            return pid;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("{} never appeared", path.display());
}

fn kill(pid: u32) {
    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
}

struct Fixture {
    _dir: tempfile::TempDir,
    marks: PathBuf,
    layout: Layout,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let marks = dir.path().join("marks");
    std::fs::create_dir_all(&marks).unwrap();
    let layout = Layout {
        checkout: dir.path().join("checkout"),
        target_dir: dir.path().join("target"),
        bundle: dir.path().join("macos-dev").join(BUNDLE_NAME),
        data_dir: dir.path().join("data"),
        ipc_port: 49_999,
    };
    Fixture {
        _dir: dir,
        marks,
        layout,
    }
}

fn pending(layout: &Layout, id: &str, staged: PathBuf, at_ms: u64) -> Pending {
    Pending {
        id: id.into(),
        from: Some("aaaaaaaaaa".into()),
        to: "bbbbbbbbbb".into(),
        staged,
        bundle: layout.bundle.clone(),
        previous: layout.previous(),
        requested_by: "owner".into(),
        reason: "test".into(),
        requested_at_ms: at_ms,
        phase: Phase::Restarting { at_ms, chat: None },
    }
}

#[test]
fn the_relaunch_waits_for_the_old_app_and_keepers_outlive_it() {
    let f = fixture();
    make_app(&f.layout.bundle, "old", "aaaaaaaaaa", &f.marks, true);
    let old = std::process::Command::new(Layout::binary(&f.layout.bundle))
        .spawn()
        .unwrap();
    let old_pid = wait_for(&f.marks.join("old.pid"));
    let keeper = wait_for(&f.marks.join("keeper.pid"));

    // Swap the new app in under the running one, arm the relauncher, quit.
    let staged = f.layout.stage("u1");
    make_app(&staged, "new", "bbbbbbbbbb", &f.marks, true);
    openagents_host_upgrade::swap::swap_in(&staged, &f.layout.bundle, &f.layout.previous())
        .unwrap();
    let (program, args) = system::launch_command(&f.layout, &f.layout.bundle, None, false);
    system::relaunch_after_exit(old_pid, &program, &args).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        !f.marks.join("new.pid").exists(),
        "waits for the old app to exit"
    );
    kill(old_pid);
    let mut old = old;
    let _ = old.wait();

    let new_pid = wait_for(&f.marks.join("new.pid"));
    assert_ne!(new_pid, old_pid);
    assert!(
        system::alive(keeper),
        "the keeper outlives the app's restart"
    );
    assert_eq!(
        Layout::bundle_commit(&f.layout.bundle).as_deref(),
        Some("bbbbbbbbbb")
    );
    kill(new_pid);
    kill(keeper);
}

#[test]
fn a_new_app_that_never_starts_is_rolled_back() {
    let f = fixture();
    make_app(&f.layout.bundle, "old", "aaaaaaaaaa", &f.marks, true);
    let staged = f.layout.stage("u2");
    make_app(&staged, "new", "bbbbbbbbbb", &f.marks, false);
    openagents_host_upgrade::swap::swap_in(&staged, &f.layout.bundle, &f.layout.previous())
        .unwrap();
    // The restart began four minutes ago, and no app is running.
    let started = openagents_host_upgrade::now_ms() - 240_000;
    let p = pending(&f.layout, "u2", staged, started);
    let state_dir = f.layout.state_dir();
    record::write_pending(&state_dir, &p).unwrap();

    let mut host = SystemHost::new(f.layout.clone(), Duration::from_secs(20));
    host.launch_services = false;
    host.progress = Box::new(|_| {});
    let entry = host.wait_for_outcome(&p).expect("an outcome");

    assert_eq!(entry.outcome, Outcome::RolledBack);
    assert!(entry.detail.contains("didn't start"), "{}", entry.detail);
    assert_eq!(
        Layout::bundle_commit(&f.layout.bundle).as_deref(),
        Some("aaaaaaaaaa"),
        "the old app is back"
    );
    let reopened = wait_for(&f.marks.join("old.pid"));
    assert!(system::alive(reopened), "and open again");
    assert_eq!(record::outcome_of(&state_dir, "u2"), Some(entry));
    assert!(record::read_pending(&state_dir).is_none());
    kill(reopened);
    if let Some(keeper) = std::fs::read_to_string(f.marks.join("keeper.pid"))
        .ok()
        .and_then(|t| t.trim().parse().ok())
    {
        kill(keeper);
    }
}
