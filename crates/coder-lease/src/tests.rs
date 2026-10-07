use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use super::*;

fn holder(session: &str) -> Holder {
    Holder {
        session: session.to_owned(),
        agent: "none".to_owned(),
        pid: std::process::id(),
        command: "cargo".to_owned(),
    }
}

const LIMITS: Limits = Limits {
    build: 2,
    memory_gib: 96,
    disk_floor_gb: 10,
};

fn broker(dir: &tempfile::TempDir) -> Broker {
    Broker::new(dir.path().join("leases"), LIMITS).with_poll(Duration::from_millis(10))
}

fn request(resource: Resource) -> Request {
    Request::new(resource, holder("test:1")).wait(Wait::No)
}

fn waiting_for(broker: &Broker, resource: &str, count: usize) {
    for _ in 0..500 {
        let waiting = broker
            .list()
            .unwrap()
            .iter()
            .filter(|entry| entry.resource == resource && entry.state == State::Waiting)
            .count();
        if waiting >= count {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("{count} {resource} requests never queued");
}

#[test]
fn an_exclusive_resource_admits_one_holder() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    let first = broker.acquire(request(Resource::Gpu)).unwrap();
    let Err(Error::Busy(blocked)) = broker.acquire(request(Resource::Gpu)) else {
        panic!("a second gpu lease was admitted");
    };
    assert_eq!(blocked.by[0].id, first.id());
    // Another exclusive resource is independent.
    let browser = broker.acquire(request(Resource::Browser)).unwrap();
    assert!(
        broker
            .acquire(request(Resource::Issue(10755)))
            .unwrap()
            .entry()
            .state
            == State::Held
    );
    drop(first);
    broker.acquire(request(Resource::Gpu)).unwrap();
    drop(browser);
    // A refused request leaves nothing behind.
    assert!(broker.list().unwrap().is_empty());
    assert_eq!(
        std::fs::read_dir(broker.root().join("held"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn counted_resources_admit_while_amounts_fit() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    let one = broker.acquire(request(Resource::Build)).unwrap();
    let _two = broker.acquire(request(Resource::Build)).unwrap();
    assert!(matches!(
        broker.acquire(request(Resource::Build)),
        Err(Error::Busy(_))
    ));
    drop(one);
    let _three = broker.acquire(request(Resource::Build)).unwrap();

    assert!(matches!(
        broker.acquire(request(Resource::Memory)),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        broker.acquire(request(Resource::Memory).amount(97)),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        broker.acquire(request(Resource::Gpu).amount(2)),
        Err(Error::Invalid(_))
    ));
    let _a = broker
        .acquire(request(Resource::Memory).amount(50))
        .unwrap();
    let _b = broker
        .acquire(request(Resource::Memory).amount(40))
        .unwrap();
    assert!(matches!(
        broker.acquire(request(Resource::Memory).amount(10)),
        Err(Error::Busy(_))
    ));
    broker.acquire(request(Resource::Memory).amount(6)).unwrap();
}

#[test]
fn disk_leases_keep_the_floor_and_count_held_budgets() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir).with_free_disk(|_| Ok(30_000_000_000));
    let held = broker.acquire(request(Resource::Disk).amount(15)).unwrap();
    let Err(Error::Busy(blocked)) = broker.acquire(request(Resource::Disk).amount(10)) else {
        panic!("a disk lease past the floor was admitted");
    };
    assert!(
        blocked.reason.contains("30 GB is free"),
        "{}",
        blocked.reason
    );
    broker.acquire(request(Resource::Disk).amount(5)).unwrap();
    drop(held);
    broker.acquire(request(Resource::Disk).amount(20)).unwrap();
}

#[test]
fn waiters_are_admitted_first_in_first_out() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    let held = broker.acquire(request(Resource::Unreal)).unwrap();
    let (tx, rx) = mpsc::channel();
    let mut threads = Vec::new();
    for (index, name) in ["first", "second", "third"].into_iter().enumerate() {
        let shared = broker.clone();
        let tx = tx.clone();
        threads.push(std::thread::spawn(move || {
            let lease = shared
                .acquire(Request::new(Resource::Unreal, holder(name)))
                .unwrap();
            tx.send(name).unwrap();
            std::thread::sleep(Duration::from_millis(30));
            lease.release(None).unwrap();
        }));
        waiting_for(&broker, "unreal", index + 1);
    }
    let queue: Vec<String> = broker
        .list()
        .unwrap()
        .into_iter()
        .map(|entry| entry.holder.session)
        .collect();
    assert_eq!(queue, ["test:1", "first", "second", "third"]);
    drop(held);
    let order: Vec<&str> = (0..3).map(|_| rx.recv().unwrap()).collect();
    assert_eq!(order, ["first", "second", "third"]);
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn a_waiter_that_times_out_leaves_the_queue() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    let _held = broker.acquire(request(Resource::Blender)).unwrap();
    let result =
        broker.acquire(request(Resource::Blender).wait(Wait::Up(Duration::from_millis(50))));
    assert!(matches!(result, Err(Error::TimedOut(_))));
    assert_eq!(broker.list().unwrap().len(), 1);
}

#[test]
fn a_dropped_holder_lock_frees_the_lease_for_the_next_reader() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    let lease = broker.acquire(request(Resource::Gpu)).unwrap();
    // A holder that vanished without releasing: its entry stays in the
    // table, but nobody holds its lock any more.
    let mut lease = std::mem::ManuallyDrop::new(lease);
    drop(lease.lock.take());
    assert_eq!(broker.list().unwrap().len(), 0);
    broker.acquire(request(Resource::Gpu)).unwrap();
}

#[test]
fn a_queued_quiet_lease_drains_builds_without_signaling_them() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    // A running build: a real process under a build lease.
    let build = broker.acquire(request(Resource::Build)).unwrap();
    let mut running = Command::new("sleep").arg("1").spawn().unwrap();

    let (tx, rx) = mpsc::channel();
    let quiet_broker = broker.clone();
    let soak = std::thread::spawn(move || {
        let quiet = quiet_broker
            .acquire(Request::new(Resource::Quiet, holder("soak")))
            .unwrap();
        tx.send(()).unwrap();
        quiet
    });
    waiting_for(&broker, "quiet", 1);

    // Queued, the quiet lease already stops new builds, though a slot is free.
    let Err(Error::Busy(blocked)) = broker.acquire(request(Resource::Build)) else {
        panic!("a build was admitted while quiet was queued");
    };
    assert!(blocked.reason.contains("quiet"), "{}", blocked.reason);
    assert!(rx.try_recv().is_err(), "quiet started while a build ran");

    // The running build is never signaled: it runs to its own end.
    assert!(running.try_wait().unwrap().is_none());
    let status = running.wait().unwrap();
    assert!(status.success(), "the build was signaled: {status:?}");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        assert_eq!(status.signal(), None);
    }
    assert!(
        rx.try_recv().is_err(),
        "quiet started before the build lease ended"
    );
    let receipt = build.release(Some(0)).unwrap();
    assert!(receipt.held_whole_run);

    rx.recv_timeout(Duration::from_secs(5))
        .expect("quiet started once the build ended");
    let quiet = soak.join().unwrap();
    assert!(matches!(
        broker.acquire(request(Resource::Build)),
        Err(Error::Busy(_))
    ));
    assert_eq!(quiet.env()[1], (LEASES_VAR.to_owned(), "quiet".to_owned()));
    drop(quiet);
    broker.acquire(request(Resource::Build)).unwrap();
}

#[test]
fn quiet_waits_behind_held_builds_but_admits_on_an_idle_machine() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    let quiet = broker.acquire(request(Resource::Quiet)).unwrap();
    drop(quiet);
    let _build = broker.acquire(request(Resource::Build)).unwrap();
    assert!(matches!(
        broker.acquire(request(Resource::Quiet)),
        Err(Error::Busy(_))
    ));
}

#[test]
fn the_screen_needs_a_grant_that_admits_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    let refused = broker.acquire(request(Resource::Screen).wait(Wait::Forever));
    assert!(matches!(refused, Err(Error::NoGrant(_))), "{refused:?}");

    broker
        .grant(&Grant::new(
            &Resource::Screen,
            Duration::from_secs(60),
            Some("seat-2".into()),
        ))
        .unwrap();
    assert!(matches!(
        broker.acquire(request(Resource::Screen)),
        Err(Error::NoGrant(_))
    ));

    broker
        .grant(&Grant::new(
            &Resource::Screen,
            Duration::from_secs(60),
            None,
        ))
        .unwrap();
    let screen = broker.acquire(request(Resource::Screen)).unwrap();
    assert_eq!(screen.entry().state, State::Held);
    drop(screen);

    assert!(broker.revoke(&Resource::Screen).unwrap());
    assert!(!broker.revoke(&Resource::Screen).unwrap());
    assert!(matches!(
        broker.acquire(request(Resource::Screen)),
        Err(Error::NoGrant(_))
    ));

    let mut expired = Grant::new(&Resource::Screen, Duration::from_secs(60), None);
    expired.expires_at_ms = now_ms() - 1;
    broker.grant(&expired).unwrap();
    assert!(matches!(
        broker.acquire(request(Resource::Screen)),
        Err(Error::NoGrant(_))
    ));
}

#[test]
fn release_writes_a_receipt_with_the_wait_and_whole_run() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    let first = broker.acquire(request(Resource::Gpu)).unwrap();
    let waiter = {
        let broker = broker.clone();
        std::thread::spawn(move || {
            broker
                .acquire(Request::new(Resource::Gpu, holder("waiter")))
                .unwrap()
        })
    };
    waiting_for(&broker, "gpu", 1);
    std::thread::sleep(Duration::from_millis(40));
    drop(first);
    let second = waiter.join().unwrap();
    let id = second.id().to_owned();
    let receipt = second.release(Some(3)).unwrap();
    assert!(receipt.wait_ms >= 40, "{receipt:?}");
    assert!(receipt.held_whole_run);
    assert_eq!(receipt.exit, Some(3));
    assert_eq!(receipt.holder.session, "waiter");
    let path = broker.root().join("receipts").join(format!("{id}.json"));
    let written: Receipt = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(written, receipt);
    assert_eq!(written.schema, RECEIPT_SCHEMA);

    // A lease whose holder lock was removed under it did not hold for the
    // whole run, and its receipt says so.
    let lease = broker.acquire(request(Resource::Gpu)).unwrap();
    std::fs::remove_file(
        broker
            .root()
            .join("held")
            .join(format!("{}.lock", lease.id())),
    )
    .unwrap();
    let receipt = lease.release(None).unwrap();
    assert!(!receipt.held_whole_run);
    let copy = dir.path().join("copy/receipt.json");
    receipt.write(&copy).unwrap();
    assert!(copy.exists());
}

#[test]
fn a_command_under_a_lease_passes_through_the_same_resource() {
    let dir = tempfile::tempdir().unwrap();
    let broker = Broker::new(dir.path().join("leases"), Limits { build: 1, ..LIMITS });
    let outer = broker.acquire(request(Resource::Build)).unwrap();
    let env = outer.env();
    let leases = env
        .iter()
        .find(|(name, _)| name == LEASES_VAR)
        .unwrap()
        .1
        .clone();
    let id = env
        .iter()
        .find(|(name, _)| name == LEASE_ID_VAR)
        .unwrap()
        .1
        .clone();
    assert_eq!(leases, "build");
    assert_eq!(id, outer.id());

    // The only slot is held, yet the nested request is admitted at once.
    let inner = broker
        .acquire(request(Resource::Build).inherit(&leases, Some(id.clone())))
        .unwrap();
    assert!(inner.nested());
    assert_eq!(inner.id(), outer.id());
    assert_eq!(broker.list().unwrap().len(), 1);
    // A different resource nests inside and lists both.
    let gpu = broker
        .acquire(request(Resource::Gpu).inherit(&leases, Some(id)))
        .unwrap();
    assert!(!gpu.nested());
    assert!(
        gpu.env()
            .contains(&(LEASES_VAR.to_owned(), "build,gpu".to_owned()))
    );
    let receipt = inner.release(None).unwrap();
    assert!(receipt.nested);
    assert!(
        !broker
            .root()
            .join("receipts")
            .join(format!("{}.json", outer.id()))
            .exists()
    );
    drop(gpu);
    drop(outer);

    // A stale variable with no live outer lease nests nothing.
    let _held = broker.acquire(request(Resource::Build)).unwrap();
    drop(_held);
    let fresh = broker
        .acquire(request(Resource::Build).inherit("build", Some("gone".into())))
        .unwrap();
    assert!(!fresh.nested());
}

#[test]
fn a_corrupt_table_is_an_error_not_a_reset() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    std::fs::create_dir_all(broker.root()).unwrap();
    std::fs::write(broker.root().join("table.json"), b"{not json").unwrap();
    assert!(matches!(broker.list(), Err(Error::Corrupt(_))));
    assert!(matches!(
        broker.acquire(request(Resource::Gpu)),
        Err(Error::Corrupt(_))
    ));
}
