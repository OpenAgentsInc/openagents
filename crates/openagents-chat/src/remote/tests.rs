//! Remote placement on two scratch hosts (#10699): explicit placement with
//! a bound source, refusals for revoked, stale, and unauthorized hosts,
//! and reconciliation of a lost acknowledgment with the same task on the
//! same host, never another.

use std::collections::BTreeMap;

use route_contract::RouteFamily;
use route_contract::lifecycle::Lifecycle;
use route_contract::route::RefusalReason;
use route_contract::snapshot::{CheckScope, SourcePin, Surface, WorkspaceBinding};

use super::*;
use crate::route::{Reading, Situation, THIS_COMPUTER, admit, propose};
use crate::router::{Meta, Offer};

const STUDIO: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const LAPTOP: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";

/// A scratch host: a task inbox keyed by idempotency key, with a switch
/// that drops the acknowledgment of the next create.
struct Scratch {
    key: String,
    grant: Option<HostGrant>,
    tasks: BTreeMap<String, String>,
    creates: usize,
    drop_ack: bool,
    down: bool,
}

impl Scratch {
    fn new(key: &str) -> Self {
        Scratch {
            key: key.into(),
            grant: Some(grant(key)),
            tasks: BTreeMap::new(),
            creates: 0,
            drop_ack: false,
            down: false,
        }
    }
}

impl RemoteHost for Scratch {
    fn key(&self) -> &str {
        &self.key
    }
    fn grant(&self) -> Option<HostGrant> {
        self.grant.clone()
    }
    fn create(&mut self, key: &str, order: &Order) -> Result<String, HostError> {
        if self.down {
            return Err(HostError::Lost);
        }
        if order.workspace != "openagents" {
            return Err(HostError::Refused("forbidden".into()));
        }
        let count = self.tasks.len();
        let task = self
            .tasks
            .entry(key.to_owned())
            .or_insert_with(|| format!("task-{}-{count}", &self.key[..4]))
            .clone();
        self.creates += 1;
        if self.drop_ack {
            self.drop_ack = false;
            return Err(HostError::Lost);
        }
        Ok(task)
    }
    fn created(&mut self, key: &str) -> Result<Option<String>, HostError> {
        if self.down {
            return Err(HostError::Lost);
        }
        Ok(self.tasks.get(key).cloned())
    }
}

fn grant(host: &str) -> HostGrant {
    HostGrant {
        host: host.into(),
        generation: "gen-1".into(),
        grant: format!("grant-{}", &host[..4]),
        epoch: 2,
        revoked: false,
        operate: true,
        workspaces: Some(vec!["openagents".into()]),
    }
}

fn selected(host: &str) -> Selected {
    Selected {
        host: host.into(),
        workspace: "openagents".into(),
    }
}

fn order() -> Order {
    Order {
        title: "Fix the failing test".into(),
        prompt: "Fix the failing test".into(),
        workspace: "openagents".into(),
        engine: Some("codex".into()),
    }
}

/// A Coder route admitted through the shared policy and placed on `host`.
fn admitted(
    journal: &Journal,
    request: &str,
    host: &Scratch,
) -> Result<(RouteRecord, WorkbenchBinding), Refused> {
    let meta = Meta {
        offers: vec![Offer::RunCoder],
        ..Meta::default()
    };
    let situation = Situation {
        surface: Surface::Terminal,
        caller: "local:openagents-terminal".into(),
        request: request.into(),
        thread: Some("thread-remote".into()),
        computer: THIS_COMPUTER.into(),
        project: Some(WorkspaceBinding {
            project: "openagents".into(),
            path: Some("/work/openagents".into()),
        }),
        ready: true,
        bound: None,
        check: CheckScope::IndependentSuite,
    };
    let reading = Reading {
        meta: Some(&meta),
        computer_lane: false,
        text: "fix the failing test",
        reply: "Starting Coder.",
    };
    let result = propose(&reading, &situation, &|_| None);
    assert_eq!(result.family(), RouteFamily::Coder);
    let mut snapshot = admit(&result, &situation, Some(&meta), reading.text, None);
    snapshot.input.source = Some(SourcePin {
        revision: Some("f2429380a9".into()),
        snapshot: None,
    });
    place(&mut snapshot, &selected(&host.key), host.grant().as_ref())?;
    let binding = binding(&snapshot, &host.grant().unwrap());
    binding.check(&snapshot).unwrap();
    let mut record = RouteRecord::received(
        request,
        situation.thread.clone(),
        result,
        snapshot,
        crate::route::now_ms(),
    )
    .unwrap();
    record.step(Lifecycle::Admitted, "operator", 2).unwrap();
    journal.write(&record).unwrap();
    Ok((record, binding))
}

#[test]
fn work_runs_only_on_the_host_selected_with_its_source_bound() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::at(dir.path().join("routes"));
    let mut studio = Scratch::new(STUDIO);
    let mut laptop = Scratch::new(LAPTOP);
    let (mut record, binding) = admitted(&journal, "req-1", &studio).unwrap();
    // The snapshot names the selected host, its grant and epoch, the
    // host-scoped workspace (never a path), and the source revision.
    let placement = &record.snapshot.placement;
    assert_eq!(placement.computer.as_deref(), Some(STUDIO));
    assert_eq!(placement.grant.as_ref().unwrap().id, "grant-a1a1");
    assert_eq!(placement.grant.as_ref().unwrap().epoch, 2);
    assert_eq!(placement.workspace.as_ref().unwrap().path, None);
    assert_eq!(binding.placement.recipient, STUDIO);
    // The surface and payer are unchanged by placement.
    assert_eq!(record.snapshot.identity.surface, Surface::Terminal);
    // Another host is never handed the route.
    assert_eq!(
        dispatch(&journal, &mut record, &binding, &mut laptop, &order(), 3),
        Dispatched::Refused(Refused::Binding(Refusal::PlacementChanged))
    );
    assert_eq!(laptop.creates, 0);
    let Dispatched::Started(task) =
        dispatch(&journal, &mut record, &binding, &mut studio, &order(), 3)
    else {
        panic!("not started");
    };
    assert_eq!(studio.creates, 1);
    // The journal names the task and the send; asking again follows it.
    let kept = journal.latest("thread-remote", "req-1").unwrap();
    assert_eq!(kept.tasks(), [task.as_str()]);
    assert_eq!(kept.sent.as_ref().unwrap().recipient, STUDIO);
    let mut reopened = kept;
    assert_eq!(
        dispatch(&journal, &mut reopened, &binding, &mut studio, &order(), 4),
        Dispatched::Followed(task)
    );
    assert_eq!(studio.creates, 1);
    // A second route placed on the laptop runs there, independently.
    let (mut other, other_binding) = admitted(&journal, "req-2", &laptop).unwrap();
    assert!(matches!(
        dispatch(
            &journal,
            &mut other,
            &other_binding,
            &mut laptop,
            &order(),
            5
        ),
        Dispatched::Started(_)
    ));
    assert_eq!((studio.creates, laptop.creates), (1, 1));
}

#[test]
fn unauthorized_revoked_and_stale_hosts_refuse_before_sending() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::at(dir.path().join("routes"));
    // No grant, no operate right, a revoked grant, another workspace, or
    // an unbound source refuse at placement.
    let mut none = Scratch::new(STUDIO);
    none.grant = None;
    assert_eq!(
        admitted(&journal, "req-a", &none).unwrap_err(),
        Refused::Unauthorized
    );
    let mut observer = Scratch::new(STUDIO);
    observer.grant.as_mut().unwrap().operate = false;
    assert_eq!(
        admitted(&journal, "req-b", &observer).unwrap_err(),
        Refused::NoOperate
    );
    let mut revoked = Scratch::new(STUDIO);
    revoked.grant.as_mut().unwrap().revoked = true;
    assert_eq!(
        admitted(&journal, "req-c", &revoked).unwrap_err(),
        Refused::Revoked
    );
    let mut snapshot = admitted(&journal, "req-d", &Scratch::new(STUDIO))
        .unwrap()
        .0
        .snapshot;
    let wrong = Selected {
        host: STUDIO.into(),
        workspace: "private".into(),
    };
    assert_eq!(
        place(&mut snapshot, &wrong, Some(&grant(STUDIO))),
        Err(Refused::Workspace)
    );
    snapshot.input.source = None;
    assert_eq!(
        place(&mut snapshot, &selected(STUDIO), Some(&grant(STUDIO))),
        Err(Refused::SourceUnbound)
    );
    // Rights change between admission and dispatch: a later epoch or a
    // restart is stale, a revocation is revoked, and nothing is sent.
    for (change, want) in [
        (
            (|g: &mut HostGrant| g.epoch = 3) as fn(&mut HostGrant),
            Refusal::Stale,
        ),
        (
            |g: &mut HostGrant| g.generation = "gen-2".into(),
            Refusal::Stale,
        ),
        (|g: &mut HostGrant| g.revoked = true, Refusal::Revoked),
    ] {
        let mut host = Scratch::new(STUDIO);
        let request = format!("req-{want:?}-{}", host.tasks.len());
        let (mut record, binding) = admitted(&journal, &request, &host).unwrap();
        change(host.grant.as_mut().unwrap());
        assert_eq!(
            dispatch(&journal, &mut record, &binding, &mut host, &order(), 3),
            Dispatched::Refused(Refused::Binding(want.clone()))
        );
        assert_eq!(host.creates, 0);
        assert_eq!(record.state, Lifecycle::Failed);
        assert!(record.tasks().is_empty());
        if want == Refusal::Revoked {
            assert_eq!(record.refusal, Some(RefusalReason::MissingGrant));
        }
    }
}

#[test]
fn a_lost_acknowledgment_reconciles_the_same_task_on_the_same_host() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::at(dir.path().join("routes"));
    let mut studio = Scratch::new(STUDIO);
    let mut laptop = Scratch::new(LAPTOP);
    let (mut record, binding) = admitted(&journal, "req-lost", &studio).unwrap();
    studio.drop_ack = true;
    assert_eq!(
        dispatch(&journal, &mut record, &binding, &mut studio, &order(), 3),
        Dispatched::Unknown
    );
    // The send is on disk before the host was asked, and no task is named.
    let kept = journal.latest("thread-remote", "req-lost").unwrap();
    assert_eq!(kept.state, Lifecycle::Admitted);
    assert!(kept.tasks().is_empty());
    assert_eq!(kept.sent.as_ref().unwrap().key, "req-lost");
    // The transport is still down: still unknown, and nothing runs on the
    // laptop or anywhere else.
    studio.down = true;
    let mut reloaded = kept.clone();
    assert_eq!(
        dispatch(&journal, &mut reloaded, &binding, &mut studio, &order(), 4),
        Dispatched::Unknown
    );
    assert_eq!(
        dispatch(&journal, &mut reloaded, &binding, &mut laptop, &order(), 4),
        Dispatched::Refused(Refused::Binding(Refusal::PlacementChanged))
    );
    assert_eq!(laptop.creates, 0);
    // Reconnected: the same host says what the key created, and the record
    // follows that task. The host created exactly one.
    studio.down = false;
    let created = studio.tasks.get("req-lost").cloned().unwrap();
    assert_eq!(
        dispatch(&journal, &mut reloaded, &binding, &mut studio, &order(), 5),
        Dispatched::Followed(created.clone())
    );
    assert_eq!(studio.creates, 1);
    assert_eq!(studio.tasks.len(), 1);
    let kept = journal.latest("thread-remote", "req-lost").unwrap();
    assert_eq!(kept.tasks(), [created.as_str()]);
    // A revocation after an unknown send keeps the send for reconciliation
    // rather than ending the route as if nothing had been sent.
    let mut host = Scratch::new(STUDIO);
    let (mut record, binding) = admitted(&journal, "req-revoked", &host).unwrap();
    host.drop_ack = true;
    assert_eq!(
        dispatch(&journal, &mut record, &binding, &mut host, &order(), 3),
        Dispatched::Unknown
    );
    host.grant.as_mut().unwrap().revoked = true;
    assert_eq!(
        dispatch(&journal, &mut record, &binding, &mut host, &order(), 4),
        Dispatched::Refused(Refused::Binding(Refusal::Revoked))
    );
    assert_eq!(record.state, Lifecycle::Admitted);
    assert!(record.sent.is_some());
    assert_eq!(host.creates, 1);
}

#[test]
fn a_host_refusal_ends_the_route_without_a_task() {
    let dir = tempfile::tempdir().unwrap();
    let journal = Journal::at(dir.path().join("routes"));
    let mut studio = Scratch::new(STUDIO);
    let (mut record, binding) = admitted(&journal, "req-no", &studio).unwrap();
    let mut wrong = order();
    wrong.workspace = "elsewhere".into();
    assert_eq!(
        dispatch(&journal, &mut record, &binding, &mut studio, &wrong, 3),
        Dispatched::HostRefused("forbidden".into())
    );
    assert_eq!(record.state, Lifecycle::Failed);
    assert!(record.tasks().is_empty());
}
