use coder_pty::ext::{Layout, Member, MemberState, Node, SessionRecord, Tab};
use coder_pty::wire::TerminalRef;
use std::sync::{Arc, Mutex};
use workbench::pane::{Description, PaneAdapter, PaneKind, PaneState, Panes, Subject, View};
use workbench::{Host, Kind, ResourceRef, Revision};
use workbench_session::projection::{InputProof, Resolution, project};
use workbench_session::{Consent, Current, Probe, Resolver, Saved, State};

fn id(n: char) -> String {
    n.to_string().repeat(64)
}
fn owner() -> Host {
    Host::Paired { key: id('a') }
}
fn terminal(n: char, state: Option<MemberState>) -> Member {
    Member::Terminal {
        member: (n as u16) - ('0' as u16),
        terminal: TerminalRef {
            generation: id('b'),
            terminal: id(n),
        },
        state,
    }
}
fn saved(members: Vec<Member>) -> Saved {
    Saved {
        v: workbench_session::SCHEMA.into(),
        owner: owner(),
        record: SessionRecord {
            session: Some(id('c')),
            revision: 3,
            name: "Native session".into(),
            layout: Layout {
                tabs: vec![Tab {
                    name: "Existing member".into(),
                    root: Node::Pane {
                        member: members[0].id(),
                    },
                }],
                active: 0,
            },
            members,
        },
    }
}
struct Owners {
    current: Vec<Current>,
    replacement: bool,
    reads: Mutex<Vec<ResourceRef>>,
}
impl Owners {
    fn new(saved: &Saved) -> Self {
        let mut current = Vec::<Current>::new();
        for member in saved.members().unwrap() {
            if !current.iter().any(|c| c.host == member.resource.host) {
                current.push(Current {
                    host: member.resource.host,
                    generation: member.resource.generation,
                    route: Some("direct".into()),
                    disclosure: id('d'),
                    expires_at: 100,
                    read: true,
                    input: true,
                    revoked: false,
                    capabilities: vec![Kind::Terminal, Kind::Run, Kind::Thread, Kind::Studio],
                });
            }
        }
        Self {
            current,
            replacement: false,
            reads: Mutex::new(Vec::new()),
        }
    }
}
impl Resolver for Owners {
    fn current(&self, host: &Host) -> Option<Current> {
        self.current.iter().find(|c| &c.host == host).cloned()
    }
    fn resource(&self, reference: &ResourceRef) -> Probe {
        self.reads.lock().unwrap().push(reference.clone());
        let mut resource = reference.clone();
        if self.replacement {
            resource.id = "substitution".into();
        }
        Probe {
            resource,
            state: State::Ready,
        }
    }
}
fn consents(saved: &Saved) -> Vec<Consent> {
    saved
        .members()
        .unwrap()
        .into_iter()
        .map(|m| Consent::new(m.resource, id('d')).unwrap())
        .collect()
}
fn proof(saved: &Saved) -> InputProof {
    InputProof {
        resource: saved.members().unwrap()[0].resource.clone(),
        current_snapshot: true,
        current_attachment: Some(id('e')),
        is_current_typist: true,
    }
}
fn view(
    saved: &Saved,
    owners: &Owners,
    input: &[InputProof],
) -> workbench_session::projection::Projection {
    project(saved, owners, &consents(saved), 1, &Panes::new(), input).unwrap()
}

#[test]
fn native_states_do_not_recreate_closed_lost_or_unknown_terminals() {
    let saved = saved(vec![
        terminal('1', Some(MemberState::Live)),
        terminal('2', Some(MemberState::Closed)),
        terminal('3', Some(MemberState::Lost)),
        terminal('4', None),
    ]);
    let before = serde_json::to_vec(&saved).unwrap();
    let owners = Owners::new(&saved);
    let mut input = proof(&saved);
    let mut evidence = Vec::new();
    for member in saved.members().unwrap() {
        input.resource = member.resource;
        evidence.push(input.clone());
    }
    let projected = view(&saved, &owners, &evidence);
    assert_eq!(
        projected
            .members
            .iter()
            .map(|m| m.resolution)
            .collect::<Vec<_>>(),
        vec![
            Resolution::Ready,
            Resolution::Closed,
            Resolution::Lost,
            Resolution::Unknown
        ]
    );
    assert_eq!(
        projected
            .members
            .iter()
            .map(|m| m.input)
            .collect::<Vec<_>>(),
        vec![true, false, false, false]
    );
    assert_eq!(projected.session, saved.record.session.clone().unwrap());
    assert_eq!(projected.layout, saved.record.layout);
    assert_eq!(serde_json::to_vec(&saved).unwrap(), before);
    assert_eq!(owners.reads.lock().unwrap().len(), 4);
    let mut substituted = projected;
    substituted.members[0].resource.generation = Some(id('f'));
    assert!(substituted.check(&saved).is_err());
}

#[test]
fn terminal_grant_without_current_snapshot_and_typist_evidence_never_enables_input() {
    let saved = saved(vec![terminal('1', Some(MemberState::Live))]);
    let mut owners = Owners::new(&saved);
    assert!(!view(&saved, &owners, &[]).members[0].input);
    let original = proof(&saved);
    assert!(view(&saved, &owners, &[original.clone()]).members[0].input);
    let mut incomplete = original.clone();
    incomplete.current_snapshot = false;
    assert!(!view(&saved, &owners, &[incomplete]).members[0].input);
    let mut watcher = original.clone();
    watcher.is_current_typist = false;
    assert!(!view(&saved, &owners, &[watcher]).members[0].input);
    let mut detached = original.clone();
    detached.current_attachment = None;
    assert!(!view(&saved, &owners, &[detached]).members[0].input);
    let mut other_generation = original.clone();
    other_generation.resource.generation = Some(id('f'));
    assert!(!view(&saved, &owners, &[other_generation]).members[0].input);
    owners.current[0].input = false;
    assert!(!view(&saved, &owners, &[original.clone()]).members[0].input);
    owners.current[0].input = true;
    owners.current[0].revoked = true;
    let revoked = view(&saved, &owners, &[original.clone()]);
    assert_eq!(revoked.members[0].resolution, Resolution::Revoked);
    assert!(!revoked.members[0].input);
    owners.current[0].revoked = false;
    owners.current[0].generation = Some(id('f'));
    assert_eq!(
        view(&saved, &owners, &[original.clone()]).members[0].resolution,
        Resolution::Lost
    );
    owners.current[0].generation = Some(id('b'));
    assert!(
        project(
            &saved,
            &owners,
            &consents(&saved),
            100,
            &Panes::new(),
            &[original.clone()]
        )
        .unwrap()
        .members
        .iter()
        .all(|m| !m.input)
    );
    assert!(
        project(
            &saved,
            &owners,
            &consents(&saved),
            1,
            &Panes::new(),
            &[original.clone(), original]
        )
        .is_err()
    );
}

struct Adapter {
    calls: Arc<Mutex<Vec<Subject>>>,
    state: PaneState,
    detail: String,
}
impl PaneAdapter for Adapter {
    fn kind(&self) -> PaneKind {
        PaneKind::Run
    }
    fn describe(&self, subject: &Subject) -> Description {
        self.calls.lock().unwrap().push(subject.clone());
        Description {
            state: self.state.clone(),
            title: "Retained native task".into(),
            detail: self.detail.clone(),
            actions: vec!["start".into()],
        }
    }
}
fn run(member: u16, host: Host) -> Member {
    Member::Resource {
        member,
        resource: serde_json::to_value(
            ResourceRef::new(Kind::Run, host, format!("task-{member}"))
                .with_revision(Revision::Counter(7)),
        )
        .unwrap(),
    }
}

#[test]
fn only_exact_owner_adapters_describe_original_resources_and_offer_no_effects() {
    let other = Host::Paired { key: id('f') };
    let local = Host::Local { instance: id('9') };
    let studio = ResourceRef::new(Kind::Studio, local, "decision-4")
        .studio(workbench::StudioPart::Decision)
        .with_revision(Revision::Counter(18));
    let saved = saved(vec![
        run(1, owner()),
        run(2, other),
        Member::Resource {
            member: 3,
            resource: serde_json::to_value(&studio).unwrap(),
        },
    ]);
    let mut owners = Owners::new(&saved);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let generic_calls = Arc::new(Mutex::new(Vec::new()));
    let panes = Panes::new()
        .adapter(Box::new(Adapter {
            calls: generic_calls.clone(),
            state: PaneState::Ready,
            detail: "Generic adapter must not be called".into(),
        }))
        .adapter_for_host(
            owner(),
            Box::new(Adapter {
                calls: calls.clone(),
                state: PaneState::Ready,
                detail: "An existing owner record".into(),
            }),
        );
    let projected = project(&saved, &owners, &consents(&saved), 1, &panes, &[]).unwrap();
    assert_eq!(projected.members[0].resolution, Resolution::Ready);
    assert_eq!(projected.members[1].resolution, Resolution::Unsupported);
    assert_eq!(projected.members[2].resource, studio);
    assert_eq!(projected.members[2].resolution, Resolution::Unsupported);
    assert!(
        projected
            .members
            .iter()
            .all(|m| !m.input && m.pane.as_ref().unwrap().actions.is_empty())
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert!(generic_calls.lock().unwrap().is_empty());
    owners.replacement = true;
    let replaced = project(&saved, &owners, &consents(&saved), 1, &panes, &[]).unwrap();
    assert!(
        replaced
            .members
            .iter()
            .all(|m| m.resolution == Resolution::IdentityMismatch
                && m.pane.as_ref().unwrap().state != PaneState::Ready)
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    owners.replacement = false;
    owners.current[0].disclosure = id('8');
    let changed = project(&saved, &owners, &consents(&saved), 1, &panes, &[]).unwrap();
    assert_eq!(changed.members[0].resolution, Resolution::NeedsAdmission);
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn tty_fallback_is_a_label_and_links_require_the_original_owner_adapter() {
    let saved = saved(vec![run(1, owner())]);
    let owners = Owners::new(&saved);
    let global = Panes::new()
        .fallback(
            PaneKind::Run,
            View::Link {
                url: "https://example.invalid/{id}".into(),
            },
        )
        .unwrap();
    let no_owner = project(&saved, &owners, &consents(&saved), 1, &global, &[]).unwrap();
    assert_eq!(
        no_owner.members[0].pane.as_ref().unwrap().state,
        PaneState::Fallback { view: View::Label }
    );
    let adapter = |state| {
        Box::new(Adapter {
            calls: Arc::default(),
            state,
            detail: String::new(),
        }) as Box<dyn PaneAdapter>
    };
    let tty = Panes::new().adapter_for_host(
        owner(),
        adapter(PaneState::Fallback {
            view: View::Tty {
                command: vec!["coder".into(), "task".into(), "show".into(), "{id}".into()],
            },
        }),
    );
    let tty_view = project(&saved, &owners, &consents(&saved), 1, &tty, &[]).unwrap();
    assert_eq!(
        tty_view.members[0].pane.as_ref().unwrap().state,
        PaneState::Fallback { view: View::Label }
    );
    let link = View::Link {
        url: "https://example.invalid/task-1".into(),
    };
    let linked =
        Panes::new().adapter_for_host(owner(), adapter(PaneState::Fallback { view: link.clone() }));
    let linked_view = project(&saved, &owners, &consents(&saved), 1, &linked, &[]).unwrap();
    assert_eq!(
        linked_view.members[0].pane.as_ref().unwrap().state,
        PaneState::Fallback { view: link }
    );
    assert!(
        linked_view.members[0]
            .pane
            .as_ref()
            .unwrap()
            .actions
            .is_empty()
    );
}

#[test]
fn oversized_projection_refuses_instead_of_dropping_original_members() {
    let saved = saved((1..=32).map(|n| run(n, owner())).collect());
    saved.record.check().unwrap();
    let original = serde_json::to_vec(&saved).unwrap();
    let owners = Owners::new(&saved);
    let panes = Panes::new().adapter_for_host(
        owner(),
        Box::new(Adapter {
            calls: Arc::default(),
            state: PaneState::Ready,
            detail: "x".repeat(2048),
        }),
    );
    assert_eq!(
        project(&saved, &owners, &consents(&saved), 1, &panes, &[]).unwrap_err(),
        "The session projection exceeds its display bound."
    );
    assert_eq!(serde_json::to_vec(&saved).unwrap(), original);
}

#[test]
fn malformed_native_references_refuse_before_owner_reads_without_echoing_source_fields() {
    let mut saved = saved(vec![run(1, owner())]);
    let owners = Owners::new(&saved);
    let canary = "synthetic-private-field-canary";
    if let Member::Resource { resource, .. } = &mut saved.record.members[0] {
        resource[canary] = serde_json::json!("synthetic-private-value-canary");
    }
    let error = project(&saved, &owners, &[], 1, &Panes::new(), &[]).unwrap_err();
    assert!(!error.contains(canary));
    assert!(!error.contains("synthetic-private-value-canary"));
    assert!(owners.reads.lock().unwrap().is_empty());
}
