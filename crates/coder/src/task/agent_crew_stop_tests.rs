//! Native lifecycle controls and original worker admissions over isolated stores.
use super::*;
use coder_host::access::crew::{Control, JobRole};
use serde_json::Value;
use std::sync::atomic::AtomicUsize;

fn clock() -> u64 {
    1_791_158_400
}
fn owner() -> Principal {
    Principal {
        device: "owner".into(),
        grant: None,
        epoch: None,
    }
}
fn host(dir: &tempfile::TempDir) -> Agents {
    let workspace = dir.path().join("work");
    std::fs::create_dir_all(workspace.join(".git")).unwrap();
    let agents = Agents::new(
        dir.path().join("host"),
        dir.path().join("tasks"),
        BTreeMap::new(),
    )
    .with_clock(clock)
    .with_coder_state(dir.path().join("coder"));
    for (name, role) in [
        ("paul", JobRole::SalesLead),
        ("researcher", JobRole::SalesResearcher),
    ] {
        agents.create_crew(name, &workspace, role, None).unwrap();
    }
    agents.create("alice", &workspace, None).unwrap();
    agents
}
fn op(
    action: ControlAction,
    cohort: &str,
    selection: Selection,
    expected: Option<String>,
) -> Operation {
    Operation::ControlCrew {
        control: Control {
            action,
            cohort: cohort.into(),
            selection,
            expected,
            reason: "Synthetic owner control.".into(),
        },
    }
}
fn control(agents: &Agents, key: &str, action: ControlAction) -> Value {
    let expected = (action == ControlAction::Resume)
        .then(|| CrewGuard::open(agents.root()).unwrap().book.digest);
    agents
        .control_crew(
            key,
            &owner(),
            &op(action, "floor", Selection::AllSales, expected),
        )
        .unwrap()
}
fn request(agents: &Agents, key: &str, text: &str) -> Result<Value, Code> {
    agents.answer(
        key,
        &owner(),
        &Operation::AskAgent {
            agent: "paul".into(),
            text: text.into(),
            workspace: None,
            context: String::new(),
            mode: Mode::Terminal,
            typist: false,
        },
    )
}
#[test]
fn native_pause_stop_resume_restart_preserve_identity_and_disable_existing_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir);
    let (store, original) = agents.store("paul").unwrap();
    let mut job = agent_jobs::template("nightly-check", "", None, None, 0, clock()).unwrap();
    // A retained native job, not permission to enable autonomous sales work.
    job.enabled = true;
    Jobs::new(store.clone()).save(&[job]).unwrap();
    let paused = control(&agents, "pause", ControlAction::Pause);
    assert_eq!(paused["state"], "complete");
    assert_eq!(agents.store("paul").unwrap().1.state, State::Paused);
    assert_eq!(agents.store("alice").unwrap().1.state, State::Active);
    assert!(Jobs::new(store.clone()).load().unwrap()[0].enabled);
    assert!(request(&agents, "blocked", "Remember a rejected note.").is_err());
    assert!(agents.pause("paul", false, "owner").is_err());
    let stopped = control(&agents, "stop", ControlAction::Stop);
    assert_eq!(
        stopped["members"]["paul"]["lifecycle"]["member_state"],
        "stopped"
    );
    assert!(!Jobs::new(store.clone()).load().unwrap()[0].enabled);
    assert_eq!(control(&agents, "stop", ControlAction::Stop), stopped);
    assert!(
        agents
            .control_crew(
                "stop",
                &owner(),
                &op(ControlAction::Pause, "floor", Selection::AllSales, None)
            )
            .is_err()
    );
    let restarted =
        Agents::new(agents.root(), dir.path().join("tasks"), BTreeMap::new()).with_clock(clock);
    assert!(request(&restarted, "restart", "Remember a rejected note.").is_err());
    assert!(
        restarted
            .control_crew(
                "stale-resume",
                &owner(),
                &op(
                    ControlAction::Resume,
                    "floor",
                    Selection::AllSales,
                    Some(format!("sha256:{}", "0".repeat(64)))
                )
            )
            .is_err()
    );
    control(&restarted, "resume", ControlAction::Resume);
    assert_eq!(restarted.store("paul").unwrap().1.pubkey, original.pubkey);
    assert_eq!(restarted.store("paul").unwrap().1.state, State::Active);
    assert!(!Jobs::new(store.clone()).load().unwrap()[0].enabled);
    let text = store
        .journal(100)
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .collect::<Vec<_>>()
        .join("\n");
    for step in ["stop 1 of 4", "stop 2 of 4", "stop 3 of 4", "stop 4 of 4"] {
        assert!(text.contains(step));
    }
}
#[test]
fn overlapping_cohort_resume_does_not_resume_another_stop_or_future_member() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir);
    control(&agents, "floor-stop", ControlAction::Stop);
    let selection = Selection::Members(vec!["paul".into()]);
    agents
        .control_crew(
            "subset-pause",
            &owner(),
            &op(ControlAction::Pause, "subset", selection.clone(), None),
        )
        .unwrap();
    let digest = CrewGuard::open(agents.root()).unwrap().book.digest;
    let result = agents
        .control_crew(
            "subset-resume",
            &owner(),
            &op(ControlAction::Resume, "subset", selection, Some(digest)),
        )
        .unwrap();
    assert_eq!(
        result["members"]["paul"]["lifecycle"]["resume"],
        "blocked_by_other_cohort"
    );
    assert!(request(&agents, "still-stopped", "Remember a rejected note.").is_err());
    agents
        .create_crew(
            "frank",
            &dir.path().join("work"),
            JobRole::SalesProspector,
            None,
        )
        .unwrap();
    assert_eq!(agents.store("frank").unwrap().1.state, State::Stopped);
    assert_eq!(agents.store("alice").unwrap().1.state, State::Active);
    control(&agents, "floor-resume", ControlAction::Resume);
    assert_eq!(agents.store("frank").unwrap().1.state, State::Active);
}
struct Partial;
impl crew_control::Revoker for Partial {
    fn revoke(&self, member: &str, _: u64) -> Result<crew_control::Revocation, String> {
        if member == "researcher" {
            return Err("The isolated queue cannot confirm cleanup.".into());
        }
        Ok(crew_control::Revocation {
            enabled: true,
            pending_revoked: 1,
            admitted: 1,
            unknown: 1,
            receipt_sha256: Some("a".repeat(64)),
        })
    }
}
#[test]
fn partial_stop_identifies_native_job_failure_and_unknown_external_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir).with_dispatch_revoker(Arc::new(Partial));
    std::fs::write(
        agents.root().join("agents/researcher/jobs.json"),
        b"invalid retained jobs",
    )
    .unwrap();
    let result = control(&agents, "partial", ControlAction::Stop);
    assert_eq!(result["state"], "partial");
    assert_eq!(result["members"]["paul"]["pending_dispatch"]["unknown"], 1);
    assert_eq!(
        result["members"]["researcher"]["pending_dispatch"]["state"],
        "unknown"
    );
    assert_eq!(
        result["members"]["researcher"]["lifecycle"]["state"],
        "partial"
    );
    for name in ["paul", "researcher"] {
        assert_eq!(agents.store(name).unwrap().1.state, State::Stopped);
    }
    assert_eq!(
        CrewGuard::open(agents.root())
            .unwrap()
            .book
            .history
            .values()
            .next()
            .unwrap()
            .state,
        "partial"
    );
}
#[test]
fn delegated_controls_refuse_before_any_native_state_is_created() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir);
    let principal = Principal {
        device: "device".into(),
        grant: Some("grant".into()),
        epoch: Some(0),
    };
    let before = CrewGuard::open(agents.root()).unwrap().book.digest;
    for op in [
        Operation::CrewStatus {},
        op(ControlAction::Stop, "floor", Selection::AllSales, None),
    ] {
        assert_eq!(
            agents.control_crew("unauthorized", &principal, &op),
            Err(Code::Forbidden)
        );
    }
    assert_eq!(CrewGuard::open(agents.root()).unwrap().book.digest, before);
}
#[test]
fn resume_preserves_old_worker_cancellation_and_refuses_unbudgeted_sales_work() {
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir);
    let engines = Arc::new(AtomicUsize::new(0));
    let count = engines.clone();
    let agents = agents.with_engine(Arc::new(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
        Ok((
            Box::new(coder_v1::Scripted::default()),
            "unadmitted fixture".into(),
        ))
    }));
    assert_eq!(
        request(&agents, "active", "Draft supplied facts."),
        Err(Code::Unavailable)
    );
    // A retained worker's cancellation must survive resumption. This fixture
    // models its blocked lifetime without executing a model or claiming spend.
    agents.with_live("paul", |live| live.busy = true);
    let old_cancel = agents.lock().live["paul"].cancel.clone();
    let cancel = old_cancel.clone();
    let copy = agents.clone();
    let (release, blocked) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        blocked.recv().unwrap();
        assert!(cancel.load(Ordering::SeqCst));
        copy.with_live("paul", |live| live.busy = false);
    });
    assert_eq!(
        request(&agents, "queued", "Draft a second request."),
        Err(Code::Unavailable)
    );
    let (answer, decision) = mpsc::channel();
    agents.with_live("paul", |live| {
        live.pending = Some((
            wire::Proposal {
                step: 9,
                command: "unadmitted fixture".into(),
                why: "Synthetic pending approval.".into(),
            },
            answer,
        ));
    });
    control(&agents, "stop", ControlAction::Stop);
    assert!(old_cancel.load(Ordering::SeqCst));
    assert_eq!(
        decision.recv_timeout(Duration::from_secs(2)).unwrap(),
        Decision::Reject
    );
    control(&agents, "resume", ControlAction::Resume);
    assert!(old_cancel.load(Ordering::SeqCst));
    assert!(!Arc::ptr_eq(
        &old_cancel,
        &agents.lock().live["paul"].cancel
    ));
    assert!(agents.decide("paul", 9, true, "owner").is_err());
    release.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(engines.load(Ordering::SeqCst), 0);
    assert!(!agents.lock().live["paul"].busy);
    assert!(agents.lock().live["paul"].queue.is_empty());
}

#[test]
fn replaced_control_custody_during_revocation_never_mutates_the_member() {
    use std::os::unix::fs::PermissionsExt;
    struct Blocking {
        entered: Sender<()>,
        release: Mutex<Receiver<()>>,
    }
    impl crew_control::Revoker for Blocking {
        fn revoke(&self, _: &str, _: u64) -> Result<crew_control::Revocation, String> {
            self.entered.send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
            Err("Synthetic unknown cleanup.".into())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let agents = host(&dir);
    let root = agents.root().to_owned();
    let (entered, ready) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let agents = agents.with_dispatch_revoker(Arc::new(Blocking {
        entered,
        release: Mutex::new(blocked),
    }));
    let copy = agents.clone();
    let running = std::thread::spawn(move || {
        copy.control_crew(
            "replace",
            &owner(),
            &op(
                ControlAction::Stop,
                "one",
                Selection::Members(vec!["paul".into()]),
                None,
            ),
        )
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    let path = root.join("crew-control/writer.lock");
    std::fs::rename(&path, path.with_extension("old")).unwrap();
    std::fs::write(&path, b"").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    release.send(()).unwrap();
    assert!(running.join().unwrap().is_err());
    let guard = CrewGuard::open(&root).unwrap();
    assert_eq!(agents.store("paul").unwrap().1.state, State::Active);
    assert!(guard.pending_stamp("paul").is_err());
    assert_eq!(
        guard.book.history.values().next().unwrap().state,
        "applying"
    );
}
