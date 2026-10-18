use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::*;

/// A machine made of fixtures: a temporary home, a process table, and a
/// record of the signals it would have sent. Nothing real is stopped.
struct Fixture {
    builds: Result<Vec<Proc>, String>,
    procs: Vec<ProcInfo>,
    ancestors: Vec<u32>,
    locked: Vec<PathBuf>,
    terminated: RefCell<Vec<u32>>,
}

impl Default for Fixture {
    fn default() -> Fixture {
        Fixture {
            builds: Ok(Vec::new()),
            procs: Vec::new(),
            ancestors: vec![500, 400],
            locked: Vec::new(),
            terminated: RefCell::new(Vec::new()),
        }
    }
}

impl Hands for Fixture {
    fn builds(&self) -> Result<Vec<Proc>, String> {
        self.builds.clone()
    }
    fn process(&self, pid: u32) -> Option<ProcInfo> {
        self.procs.iter().find(|proc| proc.pid == pid).cloned()
    }
    fn ancestors(&self) -> Vec<u32> {
        self.ancestors.clone()
    }
    fn terminate(&self, pid: u32) -> std::io::Result<()> {
        self.terminated.borrow_mut().push(pid);
        Ok(())
    }
    fn remove(&self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_dir_all(path)
    }
    fn cargo_locked(&self, target: &Path) -> bool {
        self.locked.iter().any(|locked| locked == target)
    }
    fn now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_000_000)
    }
    fn stamp(&self, _: SystemTime) -> String {
        "2026-10-17 9:12:00 PM CDT".to_owned()
    }
}

struct Home {
    _dir: tempfile::TempDir,
    home: PathBuf,
    root: PathBuf,
    layout: Layout,
}

fn home() -> Home {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().canonicalize().unwrap();
    let root = home.join(".openagents/leases");
    for target in [
        "work/openagents-target-1/debug/incremental/foo-1",
        "work/openagents-target-7/debug/deps",
        "work/openagents-target-app/debug/deps",
        "work/agent-targets/openagents-1234/debug/deps",
        "work/openagents/src",
        ".openagents/scratch/s1",
    ] {
        std::fs::create_dir_all(home.join(target)).unwrap();
    }
    std::fs::write(
        home.join("work/openagents-target-7/debug/deps/libfoo-1.rlib"),
        vec![7u8; 64 * 1024],
    )
    .unwrap();
    let layout = Layout::at(&home, &root);
    Home {
        _dir: dir,
        home,
        root,
        layout,
    }
}

#[test]
fn scopes_go_by_dotted_wire_names() {
    assert_eq!(
        serde_json::to_string(&Scope::ALL).unwrap(),
        r#"["build.prune","process.stop","host.upgrade"]"#
    );
    for scope in Scope::ALL {
        assert_eq!(Scope::parse(scope.word()), Some(scope));
    }
    assert_eq!(Scope::parse("prune"), Some(Scope::BuildPrune));
    assert_eq!(Scope::parse("stop"), Some(Scope::ProcessStop));
    assert_eq!(Scope::parse("deploy"), None);
}

#[test]
fn grants_go_to_one_agent_id_replace_earlier_ones_and_end() {
    let h = home();
    assert!(Grants::load(&h.root).unwrap().grants.is_empty());
    grant(&h.root, "chat-a", &[Scope::BuildPrune], None).unwrap();
    grant(
        &h.root,
        "chat-b",
        &[Scope::ProcessStop],
        Some(Duration::from_secs(60)),
    )
    .unwrap();
    let now = crate::now_ms();
    let grants = Grants::load(&h.root).unwrap();
    assert_eq!(grants.scopes_of("chat-a", now), vec![Scope::BuildPrune]);
    assert_eq!(grants.scopes_of("chat-b", now), vec![Scope::ProcessStop]);
    assert!(grants.scopes_of("Foreman", now).is_empty());
    assert!(!grants.allows("chat-b", Scope::ProcessStop, now + 61_000));
    grant(
        &h.root,
        "chat-a",
        &[Scope::ProcessStop, Scope::ProcessStop],
        None,
    )
    .unwrap();
    let grants = Grants::load(&h.root).unwrap();
    assert_eq!(grants.scopes_of("chat-a", now), vec![Scope::ProcessStop]);
    assert_eq!(grants.grants.len(), 2);
    assert!(revoke(&h.root, "chat-a").unwrap());
    assert!(!revoke(&h.root, "chat-a").unwrap());
    assert!(grant(&h.root, "two words", &[Scope::BuildPrune], None).is_err());
    assert!(grant(&h.root, "chat-c", &[], None).is_err());
}

#[test]
fn a_prune_needs_an_agent_and_a_reason_but_no_grant() {
    let h = home();
    let fixture = Fixture::default();
    let target = h.home.join("work/openagents-target-7");
    let action = Action::Prune(target.clone());
    let entry = act(&h.root, &h.layout, &fixture, " ", &action, "disk at 30 GiB").unwrap();
    assert_eq!(entry.outcome, Outcome::Refused);
    assert!(
        entry.detail.contains("needs the agent ID"),
        "{}",
        entry.detail
    );
    let entry = act(&h.root, &h.layout, &fixture, "chat-a", &action, " ").unwrap();
    assert_eq!(entry.outcome, Outcome::Refused);
    assert!(entry.detail.contains("needs a reason"));
    assert!(target.is_dir());

    // No grant on record: the prune still goes ahead (#12135).
    let entry = act(
        &h.root,
        &h.layout,
        &fixture,
        "chat-a",
        &action,
        "disk at 30 GiB",
    )
    .unwrap();
    assert_ne!(entry.outcome, Outcome::Refused, "{}", entry.detail);

    let logged = read_log(&h.root, 10).unwrap();
    assert_eq!(logged.len(), 3);
    assert_eq!(logged[1].agent, "chat-a");
    assert_eq!(logged[1].at, "2026-10-17 9:12:00 PM CDT");
    assert_eq!(logged[1].path.as_deref(), Some(target.as_path()));
}

#[test]
fn a_granted_prune_removes_the_cache_and_logs_who_what_size_and_why() {
    let h = home();
    let fixture = Fixture::default();
    grant(&h.root, "chat-a", &[Scope::BuildPrune], None).unwrap();
    let target = h.home.join("work/openagents-target-7");
    let entry = act(
        &h.root,
        &h.layout,
        &fixture,
        "chat-a",
        &Action::Prune(target.clone()),
        "disk at 30 GiB",
    )
    .unwrap();
    assert_eq!(entry.outcome, Outcome::Done, "{}", entry.detail);
    assert!(!target.exists());
    assert!(entry.bytes.unwrap() >= 64 * 1024);
    let line = entry.line();
    for part in [
        "2026-10-17 9:12:00 PM CDT",
        "chat-a",
        "build.prune",
        "done",
        "openagents-target-7",
        "reason: disk at 30 GiB",
    ] {
        assert!(line.contains(part), "{line}");
    }
    assert_eq!(read_log(&h.root, 10).unwrap(), vec![entry]);

    let inside = h.home.join("work/openagents-target-1/debug/incremental");
    let entry = act(
        &h.root,
        &h.layout,
        &fixture,
        "chat-a",
        &Action::Prune(inside.clone()),
        "stale",
    )
    .unwrap();
    assert_eq!(entry.outcome, Outcome::Done, "{}", entry.detail);
    assert!(!inside.exists());
    assert!(h.home.join("work/openagents-target-1/debug").is_dir());
}

#[test]
fn a_granted_prune_still_refuses_what_the_sweep_never_touches() {
    let h = home();
    grant(&h.root, "chat-a", &[Scope::BuildPrune], None).unwrap();
    let held = h.home.join("work/agent-targets/openagents-1234");
    let link = h.home.join("work/openagents-target-link");
    std::os::unix::fs::symlink(h.home.join("work/openagents-target-7"), &link).unwrap();
    let cases: Vec<(PathBuf, Fixture, &str)> = vec![
        (
            h.home.join("work/openagents-target-app"),
            Fixture::default(),
            "dev-app",
        ),
        (
            h.home.join("work/openagents/src"),
            Fixture::default(),
            "not a build cache",
        ),
        (
            h.home.join(".openagents/scratch/s1"),
            Fixture::default(),
            "scratch",
        ),
        (link.clone(), Fixture::default(), "not a folder"),
        (
            PathBuf::from("work/openagents-target-7"),
            Fixture::default(),
            "absolute",
        ),
        (
            h.home.join("work/openagents-target-7/../openagents"),
            Fixture::default(),
            "absolute",
        ),
        (
            h.home.join("work/openagents-target-missing"),
            Fixture::default(),
            "can't be read",
        ),
        (
            held.clone(),
            Fixture {
                builds: Ok(vec![Proc {
                    pid: 4242,
                    name: "cargo".into(),
                    target_dir: Some(held.clone()),
                    ..Proc::default()
                }]),
                ..Fixture::default()
            },
            "cargo 4242 is building",
        ),
        (
            held.clone(),
            Fixture {
                locked: vec![held.clone()],
                ..Fixture::default()
            },
            "cargo lock",
        ),
        (
            held.clone(),
            Fixture {
                builds: Err("ps failed".into()),
                ..Fixture::default()
            },
            "counts as held",
        ),
    ];
    for (path, fixture, why) in cases {
        let entry = act(
            &h.root,
            &h.layout,
            &fixture,
            "chat-a",
            &Action::Prune(path.clone()),
            "full",
        )
        .unwrap();
        assert_eq!(entry.outcome, Outcome::Refused, "{}", path.display());
        assert!(
            entry.detail.contains(why),
            "{}: {}",
            path.display(),
            entry.detail
        );
    }
    for kept in [
        "work/openagents-target-app",
        "work/openagents/src",
        ".openagents/scratch/s1",
        "work/openagents-target-7",
    ] {
        assert!(h.home.join(kept).is_dir(), "{kept}");
    }
    assert!(held.is_dir());
}

#[test]
fn a_stop_never_takes_a_deploy_init_or_the_caller() {
    let h = home();
    let proc = |pid: u32, command: &str, ours: bool| ProcInfo {
        pid,
        name: crate::command_name(command),
        args: command.split_whitespace().map(str::to_owned).collect(),
        ours,
    };
    let fixture = Fixture {
        procs: vec![
            proc(
                36662,
                "/bin/bash scripts/deploy-production.sh --token SECRET",
                true,
            ),
            proc(36663, "/usr/local/bin/wrangler deploy", true),
            proc(700, "/usr/bin/sleep 900 --token SECRET", true),
            proc(701, "/usr/sbin/sshd", false),
            proc(500, "/bin/zsh", true),
        ],
        ..Fixture::default()
    };
    // No grant is needed (#12135); the safety refusals still hold.
    for (pid, why) in [
        (36662, "deploys are the owner's"),
        (36663, "deploys are the owner's"),
        (701, "another user"),
        (500, "the caller"),
        (1, "init"),
        (99999, "no such process"),
    ] {
        let entry = act(
            &h.root,
            &h.layout,
            &fixture,
            "chat-a",
            &Action::Stop(pid),
            "hung",
        )
        .unwrap();
        assert_eq!(entry.outcome, Outcome::Refused, "{pid}");
        assert!(entry.detail.contains(why), "{pid}: {}", entry.detail);
    }
    assert!(fixture.terminated.borrow().is_empty());

    let entry = act(
        &h.root,
        &h.layout,
        &fixture,
        "chat-a",
        &Action::Stop(700),
        "hung for 40 min",
    )
    .unwrap();
    assert_eq!(entry.outcome, Outcome::Done);
    assert_eq!(*fixture.terminated.borrow(), vec![700]);
    assert_eq!(
        (entry.pid, entry.command.as_deref()),
        (Some(700), Some("sleep"))
    );
    let log = std::fs::read_to_string(h.root.join(LOG_FILE)).unwrap();
    assert!(!log.contains("SECRET"), "arguments are never logged");
    assert!(entry.line().contains("pid 700 (sleep)"));
}

#[test]
fn the_log_reads_back_its_last_entries() {
    let h = home();
    let fixture = Fixture::default();
    for n in 0..5 {
        let mut entry = Entry::new(&fixture, "chat-a", Scope::HostUpgrade, &format!("run {n}"));
        entry.outcome = Outcome::Done;
        log(&h.root, &entry).unwrap();
    }
    std::fs::OpenOptions::new()
        .append(true)
        .open(h.root.join(LOG_FILE))
        .and_then(|mut file| std::io::Write::write_all(&mut file, b"not json\n"))
        .unwrap();
    let last = read_log(&h.root, 2).unwrap();
    assert_eq!(
        last.iter()
            .map(|entry| entry.reason.as_str())
            .collect::<Vec<_>>(),
        ["run 3", "run 4"]
    );
    assert_eq!(last[0].scope, Scope::HostUpgrade);
    assert!(read_log(&h.home.join("nowhere"), 5).unwrap().is_empty());
}
