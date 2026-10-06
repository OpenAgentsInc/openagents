use coder_pty::ext::{Axis, Layout, Member, Node, SessionRecord, Tab};
use std::sync::Mutex;
use workbench::{Host, Kind, ResourceRef};
use workbench_session::*;
fn pin(c: char) -> String {
    c.to_string().repeat(64)
}
fn host(c: char) -> Host {
    Host::Paired { key: pin(c) }
}
fn fixture() -> Saved {
    let resources = [
        ResourceRef::terminal(host('a'), pin('1'), pin('3')),
        ResourceRef::terminal(host('b'), pin('2'), pin('4')),
        ResourceRef::new(
            Kind::Thread,
            Host::Local { instance: pin('c') },
            "thread-local",
        ),
    ];
    Saved {
        v: SCHEMA.into(),
        owner: host('f'),
        record: SessionRecord {
            session: Some(pin('e')),
            revision: 3,
            name: "mixed".into(),
            members: resources
                .into_iter()
                .enumerate()
                .map(|(i, r)| Member::Resource {
                    member: i as u16 + 1,
                    resource: serde_json::to_value(r).unwrap(),
                })
                .collect(),
            layout: Layout {
                tabs: vec![
                    Tab {
                        name: "terminals".into(),
                        root: Node::Split {
                            axis: Axis::Columns,
                            ratio: 500,
                            first: Box::new(Node::Pane { member: 1 }),
                            second: Box::new(Node::Pane { member: 2 }),
                        },
                    },
                    Tab {
                        name: "local thread".into(),
                        root: Node::Pane { member: 3 },
                    },
                ],
                active: 0,
            },
        },
    }
}
struct Owners {
    current: Mutex<Vec<Current>>,
    reads: Mutex<Vec<ResourceRef>>,
    replacement: bool,
}
impl Owners {
    fn new(saved: &Saved) -> Self {
        Self {
            current: Mutex::new(
                saved
                    .members()
                    .unwrap()
                    .iter()
                    .map(|m| Current {
                        host: m.resource.host.clone(),
                        generation: m.resource.generation.clone(),
                        route: Some(format!("route-{}", m.member)),
                        disclosure: pin('d'),
                        expires_at: 100,
                        read: true,
                        input: true,
                        revoked: false,
                        capabilities: vec![Kind::Terminal, Kind::Thread],
                    })
                    .collect(),
            ),
            reads: Mutex::new(Vec::new()),
            replacement: false,
        }
    }
}
impl Resolver for Owners {
    fn current(&self, h: &Host) -> Option<Current> {
        self.current
            .lock()
            .unwrap()
            .iter()
            .find(|c| &c.host == h)
            .cloned()
    }
    fn resource(&self, r: &ResourceRef) -> Probe {
        self.reads.lock().unwrap().push(r.clone());
        let mut resource = r.clone();
        if self.replacement {
            resource.host = host('9')
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
        .map(|m| Consent::new(m.resource, pin('d')).unwrap())
        .collect()
}
#[test]
fn two_hosts_and_local_thread_restore_exactly_on_second_client() {
    let saved = fixture();
    let bytes = serde_json::to_vec(&saved).unwrap();
    let second: Saved = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(second.members().unwrap(), saved.members().unwrap());
    let owners = Owners::new(&saved);
    let view = second.resolve(&owners, &consents(&saved), 1).unwrap();
    assert_eq!(view.len(), 3);
    assert!(view.iter().all(|p| p.state == State::Ready));
    assert!(view[0].title().contains("route-1"));
    assert!(view[1].title().contains("route-2"));
    assert_ne!(view[0].resource.host, view[1].resource.host);
    assert!(matches!(view[2].resource.host, Host::Local { .. }));
}
#[test]
fn one_outage_and_revocation_leave_other_owners_usable() {
    let saved = fixture();
    let owners = Owners::new(&saved);
    owners.current.lock().unwrap()[0].route = None;
    let panes = saved.resolve(&owners, &consents(&saved), 1).unwrap();
    assert_eq!(panes[0].state, State::Unavailable);
    assert!(!panes[0].input);
    assert!(panes[1].input);
    assert_eq!(panes[2].state, State::Ready);
    assert!(!panes[2].input);
    owners.current.lock().unwrap()[0].route = Some("repaired-direct".into());
    let restored = saved.resolve(&owners, &consents(&saved), 2).unwrap();
    assert_eq!(restored[0].resource, panes[0].resource);
    assert_eq!(restored[0].state, State::Ready);
    owners.current.lock().unwrap()[0].revoked = true;
    let revoked = saved.resolve(&owners, &consents(&saved), 3).unwrap();
    assert_eq!(revoked[0].state, State::Revoked);
    assert_eq!(revoked[1].state, State::Ready);
}
#[test]
fn changed_generation_disclosure_and_owner_do_not_rebind() {
    let saved = fixture();
    let mut owners = Owners::new(&saved);
    owners.current.lock().unwrap()[0].generation = Some(pin('9'));
    assert_eq!(
        saved.resolve(&owners, &consents(&saved), 1).unwrap()[0].state,
        State::Lost
    );
    owners.current.lock().unwrap()[0].generation = Some(pin('1'));
    owners.current.lock().unwrap()[0].disclosure = pin('8');
    assert_eq!(
        saved.resolve(&owners, &consents(&saved), 1).unwrap()[0].state,
        State::NeedsAdmission
    );
    owners.current.lock().unwrap()[0].disclosure = pin('d');
    owners.replacement = true;
    assert!(
        saved
            .resolve(&owners, &consents(&saved), 1)
            .unwrap()
            .iter()
            .all(|p| p.state == State::IdentityMismatch)
    );
}
#[test]
fn local_size_override_cannot_change_saved_layout_or_cross_devices() {
    let saved = fixture();
    let original = saved.record.layout.clone();
    let mut layout = original.clone();
    if let Node::Split { ratio, .. } = &mut layout.tabs[0].root {
        *ratio = 300;
    }
    let local = Override {
        device: pin('a'),
        session: saved.record.session.clone(),
        revision: 3,
        layout: layout.clone(),
    };
    assert_eq!(saved.layout(&pin('a'), Some(&local)).unwrap(), layout);
    assert_eq!(saved.record.layout, original);
    assert!(saved.layout(&pin('b'), Some(&local)).is_err());
    let mut stale = local;
    stale.revision = 2;
    assert!(saved.layout(&pin('a'), Some(&stale)).is_err());
}
#[test]
fn unknown_capability_expired_grant_and_missing_consent_never_read() {
    let saved = fixture();
    let owners = Owners::new(&saved);
    assert!(
        saved
            .resolve(&owners, &[], 1)
            .unwrap()
            .iter()
            .all(|p| p.state == State::NeedsAdmission)
    );
    assert!(owners.reads.lock().unwrap().is_empty());
    owners.current.lock().unwrap()[0].capabilities.clear();
    assert_eq!(
        saved.resolve(&owners, &consents(&saved), 1).unwrap()[0].state,
        State::Unsupported
    );
    assert_eq!(
        saved.resolve(&owners, &consents(&saved), 100).unwrap()[1].state,
        State::NeedsAdmission
    );
}
