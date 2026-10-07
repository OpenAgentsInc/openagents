//! Disk budgets at build admission, the reclaim hook, receipts' disk use,
//! per-session totals, and slot records.

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::*;

const GB: u64 = 1_000_000_000;

const LIMITS: Limits = Limits {
    build: 2,
    memory_gib: 96,
    disk_floor_gb: 10,
    build_disk_gb: 0,
};

fn holder(session: &str) -> Holder {
    Holder {
        session: session.to_owned(),
        agent: "none".to_owned(),
        pid: std::process::id(),
        command: "cargo".to_owned(),
    }
}

fn request(resource: Resource) -> Request {
    Request::new(resource, holder("test:1")).wait(Wait::No)
}

fn broker(dir: &tempfile::TempDir) -> Broker {
    Broker::new(dir.path().join("leases"), LIMITS).with_poll(Duration::from_millis(10))
}

#[test]
fn a_build_reclaims_before_it_refuses_for_want_of_disk() {
    let dir = tempfile::tempdir().unwrap();
    let free = Arc::new(AtomicU64::new(12 * GB));
    let calls = Arc::new(AtomicU64::new(0));
    let short = Arc::new(AtomicU64::new(0));
    // The floor is 10 GB and each build reserves 5 GB.
    let broker = Broker::new(
        dir.path().join("leases"),
        Limits {
            build_disk_gb: 5,
            ..LIMITS
        },
    )
    .with_poll(Duration::from_millis(10))
    .with_free_disk({
        let free = Arc::clone(&free);
        move |_| Ok(free.load(Ordering::SeqCst))
    })
    .with_reclaim({
        let (free, calls, short) = (Arc::clone(&free), Arc::clone(&calls), Arc::clone(&short));
        move |bytes| {
            // The reclaim frees 30 GB the first time, nothing after.
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                free.fetch_add(30 * GB, Ordering::SeqCst);
            }
            short.store(bytes, Ordering::SeqCst);
        }
    });
    // 12 GB free and 15 GB needed: the broker reclaims, then admits.
    let first = broker.acquire(request(Resource::Build)).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(short.load(Ordering::SeqCst), 3 * GB);
    // With the first build's budget held, a second needs 20 GB.
    free.store(18 * GB, Ordering::SeqCst);
    let Err(Error::DiskLow {
        free: left,
        need_gb,
        floor_gb,
    }) = broker.acquire(request(Resource::Build))
    else {
        panic!("a build short of disk was admitted");
    };
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "it reclaims before refusing"
    );
    assert_eq!(short.load(Ordering::SeqCst), 2 * GB);
    assert_eq!((left, need_gb, floor_gb), (18 * GB, 20, 10));
    // A refused request leaves the table.
    assert_eq!(broker.list().unwrap().len(), 1);
    // A disk lease counts the held build's budget too.
    let Err(Error::Busy(blocked)) = broker.acquire(request(Resource::Disk).amount(4)) else {
        panic!("a disk lease ignored the build's budget");
    };
    assert!(blocked.reason.contains("5 GB held"), "{}", blocked.reason);
    drop(first);
    assert!(broker.acquire(request(Resource::Build)).is_ok());
}

#[test]
fn receipts_carry_allocated_bytes_and_usage_sums_them_per_session() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    let slot = dir.path().join("targets/p-slot-0");
    let tree = dir.path().join("worktrees/a");
    let build = |session: &str, slot_bytes: u64, tree_bytes: Option<u64>| {
        let lease = broker
            .acquire(Request::new(Resource::Build, holder(session)).wait(Wait::No))
            .unwrap();
        std::thread::sleep(Duration::from_millis(5));
        let disk = DiskUse {
            slot: Some(slot.clone()),
            slot_bytes: Some(slot_bytes),
            worktree: tree_bytes.map(|_| tree.clone()),
            worktree_bytes: tree_bytes,
            allocated_bytes: slot_bytes + tree_bytes.unwrap_or(0),
        };
        lease.release_with(Some(0), Some(disk)).unwrap()
    };
    let receipt = build("claude-code:a", 7_000, Some(500));
    assert_eq!(receipt.disk.as_ref().unwrap().allocated_bytes, 7_500);
    let written: Receipt = serde_json::from_slice(
        &std::fs::read(
            broker
                .root()
                .join("receipts")
                .join(format!("{}.json", receipt.id)),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(written, receipt);
    build("claude-code:a", 9_000, Some(600));
    // Another session takes the slot later: the slot is now its.
    build("codex:b", 11_000, None);
    drop(broker.acquire(request(Resource::Gpu)).unwrap());

    let used = usage(broker.root()).unwrap();
    let find = |session: &str| used.iter().find(|u| u.session == session).unwrap();
    let a = find("claude-code:a");
    assert_eq!((a.leases, a.builds), (2, 2));
    assert_eq!(a.allocated_bytes, 600, "only its worktree is still its");
    let b = find("codex:b");
    assert_eq!(b.allocated_bytes, 11_000);
    assert_eq!(b.paths[0].kind, "slot");
    assert_eq!(used[0].session, "codex:b", "the largest first");
    assert_eq!(find("test:1").builds, 0);
    assert!(used.iter().all(|u| !u.live));
}

#[test]
fn a_slot_records_its_last_lease_and_a_session_ends_with_its_process() {
    let dir = tempfile::tempdir().unwrap();
    let broker = broker(&dir);
    let slot = dir.path().join("targets/p-slot-1");
    std::fs::create_dir_all(&slot).unwrap();
    let gone = {
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    };
    let record = SlotUse {
        schema: SLOT_USE_SCHEMA.to_owned(),
        slot: slot.clone(),
        lease_root: broker.root().to_owned(),
        lease: "1-2-3".to_owned(),
        session: "claude-code:x".to_owned(),
        agent_pid: Some(gone),
        released_at_ms: 5,
    };
    record.write().unwrap();
    assert_eq!(SlotUse::read(&slot), Some(record.clone()));
    assert_eq!(SlotUse::read(&dir.path().join("targets/p-slot-2")), None);
    let root = broker.root();
    assert_eq!(
        session_live(root, "claude-code:x", Some(gone)).unwrap(),
        None
    );
    assert!(
        session_live(root, "claude-code:x", Some(std::process::id()))
            .unwrap()
            .is_some()
    );
    assert!(
        session_live(root, &format!("process:{}", std::process::id()), None)
            .unwrap()
            .is_some()
    );
    let lease = broker
        .acquire(Request::new(Resource::Gpu, holder("claude-code:x")))
        .unwrap();
    assert!(
        session_live(root, "claude-code:x", None)
            .unwrap()
            .unwrap()
            .ends_with("holds a lease")
    );
    drop(lease);
    assert_eq!(session_live(root, "claude-code:x", None).unwrap(), None);
}
