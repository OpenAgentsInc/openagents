//! Product panes: adapters describe their own kind, a kind without one
//! shows its labeled fallback, and nothing but a ready pane offers an
//! action.

use std::collections::BTreeMap;

use workbench::pane::{
    Description, PaneAdapter, PaneDescriptor, PaneKind, PaneState, Panes, Subject, View,
};
use workbench::{Host, Kind, ResourceRef, Revision};

fn host() -> Host {
    Host::Paired {
        key: "ab".repeat(32),
    }
}

fn thread(id: &str, revision: Option<u64>) -> Subject {
    let mut resource = ResourceRef::new(Kind::Thread, host(), id);
    resource.revision = revision.map(Revision::Counter);
    Subject::Resource { resource }
}

/// A fake thread store: each thread's turn count, and which are archived
/// or no longer readable.
struct Threads {
    turns: BTreeMap<&'static str, u64>,
    archived: Vec<&'static str>,
    revoked: Vec<&'static str>,
}

impl PaneAdapter for Threads {
    fn kind(&self) -> PaneKind {
        PaneKind::Thread
    }

    fn describe(&self, subject: &Subject) -> Description {
        let id = subject.id();
        if self.revoked.contains(&id) {
            // An adapter that offers actions anyway: the registry drops them.
            return Description {
                state: PaneState::Revoked,
                title: "Thread".into(),
                detail: String::new(),
                actions: vec!["reply".into()],
            };
        }
        let Some(&turns) = self.turns.get(id) else {
            return Description::only(PaneState::Missing, "No such thread");
        };
        if let Some(Revision::Counter(asked)) = subject.revision()
            && *asked != turns
        {
            return Description::only(
                PaneState::Stale {
                    current: Some(Revision::Counter(turns)),
                },
                format!("Thread {id}"),
            );
        }
        let actions = if self.archived.contains(&id) {
            Vec::new()
        } else {
            vec!["reply".into(), "archive".into()]
        };
        Description {
            state: PaneState::Ready,
            title: format!("Thread {id}"),
            detail: format!("{turns} turns"),
            actions,
        }
    }
}

/// An adapter that answers with what a pane may not show.
struct Broken;

impl PaneAdapter for Broken {
    fn kind(&self) -> PaneKind {
        PaneKind::Run
    }

    fn describe(&self, _: &Subject) -> Description {
        Description::only(PaneState::Ready, "bad\u{1b}[2Jtitle")
    }
}

fn panes() -> Panes {
    Panes::new()
        .adapter(Box::new(Threads {
            turns: BTreeMap::from([("t1", 4), ("t2", 9), ("t3", 1)]),
            archived: vec!["t2"],
            revoked: vec!["t3"],
        }))
        .adapter(Box::new(Broken))
        .fallback(
            PaneKind::Knowledge,
            View::Tty {
                command: vec![
                    "openagents".into(),
                    "kb".into(),
                    "show".into(),
                    "{id}".into(),
                ],
            },
        )
        .unwrap()
        .fallback(
            PaneKind::Receipt,
            View::Link {
                url: "https://openagents.com/receipts/{id}".into(),
            },
        )
        .unwrap()
}

fn resolve(panes: &Panes, kind: PaneKind, subject: &Subject) -> PaneDescriptor {
    let pane = panes.resolve(kind, subject).unwrap();
    pane.check().unwrap();
    assert_eq!(
        &pane.subject, subject,
        "a pane shows exactly what was asked"
    );
    let json = serde_json::to_value(&pane).unwrap();
    let back: PaneDescriptor = serde_json::from_value(json).unwrap();
    assert_eq!(back, pane);
    pane
}

#[test]
fn a_ready_thread_offers_its_owners_actions() {
    let pane = resolve(&panes(), PaneKind::Thread, &thread("t1", Some(4)));
    assert_eq!(pane.state, PaneState::Ready);
    assert_eq!(pane.title, "Thread t1");
    assert_eq!(pane.detail, "4 turns");
    assert_eq!(pane.actions, vec!["reply", "archive"]);
}

#[test]
fn read_only_revoked_missing_and_stale_panes_offer_no_action() {
    let panes = panes();
    let archived = resolve(&panes, PaneKind::Thread, &thread("t2", None));
    assert_eq!(archived.state, PaneState::Ready);
    assert!(archived.actions.is_empty());
    let revoked = resolve(&panes, PaneKind::Thread, &thread("t3", None));
    assert_eq!(revoked.state, PaneState::Revoked);
    assert!(revoked.actions.is_empty());
    let missing = resolve(&panes, PaneKind::Thread, &thread("gone", None));
    assert_eq!(missing.state, PaneState::Missing);
    // A stale reference names the current revision and creates nothing.
    let stale = resolve(&panes, PaneKind::Thread, &thread("t1", Some(2)));
    assert_eq!(
        stale.state,
        PaneState::Stale {
            current: Some(Revision::Counter(4))
        }
    );
    assert!(stale.actions.is_empty());
}

#[test]
fn a_kind_without_an_adapter_shows_its_labeled_fallback() {
    let panes = panes();
    let record = |id: &str| Subject::Record {
        host: host(),
        id: id.into(),
        revision: None,
    };
    let knowledge = resolve(&panes, PaneKind::Knowledge, &record("kb-12"));
    assert_eq!(knowledge.title, "No knowledge entry viewer here");
    assert_eq!(
        knowledge.state,
        PaneState::Fallback {
            view: View::Tty {
                command: vec![
                    "openagents".into(),
                    "kb".into(),
                    "show".into(),
                    "kb-12".into()
                ]
            }
        }
    );
    let receipt = resolve(&panes, PaneKind::Receipt, &record("r-7"));
    assert_eq!(
        receipt.state,
        PaneState::Fallback {
            view: View::Link {
                url: "https://openagents.com/receipts/r-7".into()
            }
        }
    );
    // No declared fallback: the label alone.
    let account = resolve(&panes, PaneKind::Account, &record("me"));
    assert_eq!(account.state, PaneState::Fallback { view: View::Label });
    assert!(account.actions.is_empty());
}

#[test]
fn a_malformed_answer_shows_as_unavailable() {
    let run = Subject::Resource {
        resource: ResourceRef::new(Kind::Run, host(), "run-1"),
    };
    let pane = resolve(&panes(), PaneKind::Run, &run);
    assert_eq!(pane.state, PaneState::Unavailable);
    assert!(!pane.title.contains('\u{1b}'));
}

#[test]
fn a_subject_must_fit_its_pane() {
    let panes = panes();
    // A thread pane cannot show a run, and a knowledge pane names a record.
    let run = Subject::Resource {
        resource: ResourceRef::new(Kind::Run, host(), "run-1"),
    };
    assert!(panes.resolve(PaneKind::Thread, &run).is_err());
    assert!(
        panes
            .resolve(PaneKind::Knowledge, &thread("t1", None))
            .is_err()
    );
    let record = Subject::Record {
        host: host(),
        id: "t1".into(),
        revision: None,
    };
    assert!(panes.resolve(PaneKind::Thread, &record).is_err());
    // A diff and a preview show a file and an artifact.
    assert_eq!(PaneKind::Diff.resource(), Some(Kind::File));
    assert_eq!(PaneKind::Preview.resource(), Some(Kind::Artifact));
    // A malformed fallback is refused when declared.
    assert!(
        Panes::new()
            .fallback(
                PaneKind::Account,
                View::Link {
                    url: "http://x".into()
                }
            )
            .is_err()
    );
}

#[test]
fn descriptors_refuse_unknown_fields_and_actions_off_ready() {
    let pane = resolve(&panes(), PaneKind::Thread, &thread("t1", None));
    let mut json = serde_json::to_value(&pane).unwrap();
    json["extra"] = serde_json::json!(1);
    assert!(serde_json::from_value::<PaneDescriptor>(json).is_err());
    let mut forged = pane.clone();
    forged.state = PaneState::Revoked;
    assert!(forged.check().is_err());
    let mut other = pane;
    other.v = "openagents.workbench-pane.v2".into();
    assert_eq!(
        other.check().unwrap_err().reason,
        workbench::Reason::UnsupportedVersion
    );
}

#[test]
fn exact_host_receipts_coexist_replace_deterministically_and_preserve_fallback() {
    struct Receipt(&'static str);
    impl PaneAdapter for Receipt {
        fn kind(&self) -> PaneKind {
            PaneKind::Receipt
        }
        fn describe(&self, _: &Subject) -> Description {
            Description::only(PaneState::Ready, self.0)
        }
    }
    let compute = Host::Local {
        instance: "ab".repeat(32),
    };
    let contribution = Host::Local {
        instance: "cd".repeat(32),
    };
    let wrong = Host::Local {
        instance: "ef".repeat(32),
    };
    let panes = Panes::new()
        .adapter(Box::new(Receipt("legacy")))
        .adapter_for_host(compute.clone(), Box::new(Receipt("compute")))
        .adapter_for_host(contribution.clone(), Box::new(Receipt("old contribution")))
        .adapter_for_host(contribution.clone(), Box::new(Receipt("contribution")));
    for (host, title) in [
        (compute, "compute"),
        (contribution, "contribution"),
        (wrong, "legacy"),
    ] {
        let subject = Subject::Record {
            host,
            id: "receipt".into(),
            revision: None,
        };
        assert_eq!(
            panes.resolve(PaneKind::Receipt, &subject).unwrap().title,
            title
        );
    }
    assert_eq!(panes.kinds(), vec![PaneKind::Receipt]);
}
