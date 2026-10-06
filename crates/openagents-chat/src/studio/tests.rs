use super::*;
use route_contract::binding::HostPlacement;
use route_contract::route::AnswerSource;
use route_contract::snapshot::{CheckScope, Surface, WorkspaceBinding};
use route_contract::studio::{Intent, MergeDecision, Verdict};
use route_contract::{BINDING_SCHEMA, RouteResult};

fn admitted(intent: Intent) -> (StudioRoute, AdmissionSnapshot, WorkbenchBinding) {
    let situation = crate::route::Situation {
        surface: Surface::Terminal,
        caller: "operator".into(),
        request: "a".repeat(64),
        thread: None,
        computer: "scratch-host".into(),
        project: Some(WorkspaceBinding {
            project: "scratch".into(),
            path: None,
        }),
        ready: false,
        bound: None,
        check: CheckScope::ExecutorExit,
    };
    let mut snapshot = crate::route::admit(
        &RouteResult::Answer {
            source: AnswerSource::Model,
        },
        &situation,
        None,
        "declared Studio intent",
        None,
    );
    snapshot.route.explicit = true;
    snapshot.input.request = route_contract::digest_of(&intent);
    snapshot.placement.computer = Some("scratch-host".into());
    snapshot.placement.workspace = situation.project;
    let binding = WorkbenchBinding {
        schema: BINDING_SCHEMA.into(),
        snapshot: snapshot.digest(),
        parent: None,
        placement: HostPlacement {
            computer: "scratch-host".into(),
            generation: "generation1".into(),
            recipient: "scratch-host".into(),
        },
        run: None,
        terminal: None,
        resources: Vec::new(),
    };
    let route = StudioRoute {
        schema: route_contract::studio::SCHEMA.into(),
        request: situation.request,
        snapshot: snapshot.digest(),
        binding: binding.digest(),
        intent,
    };
    (route, snapshot, binding)
}
fn goal() -> Intent {
    Intent::Goal {
        text: "Document the greeting".into(),
        workspace: "scratch".into(),
        lead: None,
    }
}
struct Mock {
    current: Current,
    rights: Vec<Right>,
    calls: usize,
    executions: usize,
    keys: std::collections::BTreeSet<String>,
    lose: bool,
    refuse: Option<String>,
}
impl Mock {
    fn new() -> Self {
        Self {
            current: Current {
                computer: "scratch-host".into(),
                generation: "generation1".into(),
                recipient: "scratch-host".into(),
                grant: None,
                terminal_generation: None,
            },
            rights: vec![Right::Observe, Right::Operate, Right::Review],
            calls: 0,
            executions: 0,
            keys: Default::default(),
            lose: false,
            refuse: None,
        }
    }
}
impl Host for Mock {
    fn current(&mut self) -> Result<Current, String> {
        Ok(self.current.clone())
    }
    fn rights(&self) -> Vec<Right> {
        self.rights.clone()
    }
    fn send(&mut self, request: &str, op: &Operation) -> Result<Outcome, Failure> {
        self.calls += 1;
        if let Some(reason) = &self.refuse {
            return Err(Failure::Refused(reason.clone()));
        }
        if self.keys.insert(request.into()) {
            self.executions += 1;
        }
        if self.lose {
            self.lose = false;
            return Err(Failure::Unknown);
        }
        Ok(Outcome::Dispatched {
            receipt: coder_access::protocol::Receipt {
                operation: op.name().into(),
                reference: "g1".into(),
            },
        })
    }
}

#[test]
fn acknowledgment_loss_survives_reopen_and_only_explicit_exact_reconciliation_resends() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("journal");
    let (route, snapshot, binding) = admitted(goal());
    let mut host = Mock::new();
    host.lose = true;
    {
        let mut journal = FileJournal::open(&path).unwrap();
        assert!(
            FileJournal::open(&path).is_err(),
            "second writer must not send"
        );
        assert!(matches!(
            dispatch(&route, &snapshot, &binding, &mut host, &mut journal, false)
                .unwrap()
                .state,
            State::Unknown
        ));
    }
    let mut journal = FileJournal::open(&path).unwrap();
    assert!(matches!(
        dispatch(&route, &snapshot, &binding, &mut host, &mut journal, false)
            .unwrap()
            .state,
        State::Unknown
    ));
    assert_eq!(host.calls, 1);
    let result = dispatch(&route, &snapshot, &binding, &mut host, &mut journal, true).unwrap();
    assert!(matches!(result.state, State::Completed { .. }));
    assert_eq!((host.calls, host.executions), (2, 1));
    assert_eq!(
        dispatch(&route, &snapshot, &binding, &mut host, &mut journal, true).unwrap(),
        result
    );
    assert_eq!(host.calls, 2);
    let (changed, changed_snapshot, changed_binding) = admitted(Intent::Goal {
        text: "Another goal".into(),
        workspace: "scratch".into(),
        lead: None,
    });
    assert!(
        dispatch(
            &changed,
            &changed_snapshot,
            &changed_binding,
            &mut host,
            &mut journal,
            true
        )
        .is_err()
    );
    assert_eq!(host.calls, 2);
}

#[test]
fn shell_and_operate_rights_do_not_authorize_merge_and_restart_refuses_before_send() {
    let tmp = tempfile::tempdir().unwrap();
    let mut journal = FileJournal::open(tmp.path().join("journal")).unwrap();
    let (route, snapshot, binding) = admitted(Intent::Decide {
        decision: MergeDecision {
            task: "task1".into(),
            base: "b".repeat(40),
            head_commit: "c".repeat(40),
            head: "d".repeat(40),
            verdict: Verdict::Merge,
            text: String::new(),
            command: "e".repeat(64),
            issued_at: 100,
        },
    });
    let mut host = Mock::new();
    host.rights = vec![Right::Terminal, Right::Operate];
    assert!(
        dispatch(&route, &snapshot, &binding, &mut host, &mut journal, false)
            .unwrap_err()
            .contains("review")
    );
    host.rights.push(Right::Review);
    host.current.generation = "restarted".into();
    assert!(dispatch(&route, &snapshot, &binding, &mut host, &mut journal, false).is_err());
    assert_eq!(host.calls, 0);
    assert!(journal.load(&route.request).unwrap().is_none());
}

#[test]
fn missing_decision_authority_and_changed_admission_never_dispatch() {
    let tmp = tempfile::tempdir().unwrap();
    let mut journal = FileJournal::open(tmp.path().join("journal")).unwrap();
    let (mut route, snapshot, binding) = admitted(Intent::Answer {
        decision: "decision1".into(),
        based_on: 17,
        text: "Yes".into(),
        command: "b".repeat(64),
        issued_at: 100,
    });
    let mut host = Mock::new();
    host.rights = vec![Right::Observe, Right::Review, Right::Terminal];
    assert!(
        dispatch(&route, &snapshot, &binding, &mut host, &mut journal, false)
            .unwrap_err()
            .contains("operate")
    );
    host.rights.push(Right::Operate);
    if let Intent::Answer { based_on, .. } = &mut route.intent {
        *based_on += 1;
    }
    assert!(dispatch(&route, &snapshot, &binding, &mut host, &mut journal, false).is_err());
    assert_eq!(host.calls, 0);
}

#[test]
fn exact_stale_review_reaches_existing_host_refusal_without_reinterpretation() {
    let tmp = tempfile::tempdir().unwrap();
    let mut journal = FileJournal::open(tmp.path().join("journal")).unwrap();
    let (route, snapshot, binding) = admitted(Intent::Decide {
        decision: MergeDecision {
            task: "task1".into(),
            base: "b".repeat(40),
            head_commit: "c".repeat(40),
            head: "d".repeat(40),
            verdict: Verdict::Merge,
            text: String::new(),
            command: "e".repeat(64),
            issued_at: 100,
        },
    });
    let op = operation(&route).unwrap();
    let Operation::DecideMerge { decision } = op else {
        panic!()
    };
    assert_eq!(decision.head, "d".repeat(40));
    let mut host = Mock::new();
    host.refuse = Some("stale".into());
    let result = dispatch(&route, &snapshot, &binding, &mut host, &mut journal, false).unwrap();
    assert!(matches!(result.state, State::Refused { reason } if reason == "stale"));
    assert_eq!(host.executions, 0);
}
