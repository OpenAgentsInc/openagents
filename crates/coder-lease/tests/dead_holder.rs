//! A holder killed with `SIGKILL` leaves no lease behind: the kernel drops
//! its holder lock, and the next reader drops its entry.
#![cfg(unix)]

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use coder_lease::{Broker, Holder, Limits, Request, Resource, State, Wait};

const ROOT: &str = "CODER_LEASE_TEST_ROOT";
const LIMITS: Limits = Limits {
    build: 2,
    memory_gib: 8,
    disk_floor_gb: 10,
};

fn broker(root: &Path) -> Broker {
    Broker::new(root.to_path_buf(), LIMITS).with_poll(Duration::from_millis(10))
}

/// The child half: run only when the parent sets the root. It takes the
/// GPU lease, waiting if it must, and then holds it until it is killed.
#[test]
fn child_holds_the_gpu() {
    let Some(root) = std::env::var_os(ROOT) else {
        return;
    };
    let holder = Holder {
        session: format!("child:{}", std::process::id()),
        agent: "none".into(),
        pid: std::process::id(),
        command: "sleep".into(),
    };
    let _lease = broker(Path::new(&root))
        .acquire(Request::new(Resource::Gpu, holder))
        .unwrap();
    std::thread::sleep(Duration::from_secs(120));
}

fn spawn_child(root: &Path) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args([
            "child_holds_the_gpu",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ROOT, root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

fn wait_for(broker: &Broker, pid: u32, state: State) {
    for _ in 0..1000 {
        if broker
            .list()
            .unwrap()
            .iter()
            .any(|entry| entry.holder.pid == pid && entry.state == state)
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("process {pid} never reached {state:?}");
}

#[test]
fn a_killed_holder_and_a_killed_waiter_free_the_lease() {
    if std::env::var_os(ROOT).is_some() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("leases");
    let broker = broker(&root);

    let mut holder = spawn_child(&root);
    wait_for(&broker, holder.id(), State::Held);
    let mut waiter = spawn_child(&root);
    wait_for(&broker, waiter.id(), State::Waiting);

    // A dead waiter leaves the queue.
    waiter.kill().unwrap();
    waiter.wait().unwrap();
    let leases = broker.list().unwrap();
    assert_eq!(leases.len(), 1, "{leases:?}");
    assert_eq!(leases[0].holder.pid, holder.id());

    // While the holder lives, the GPU is busy.
    let me = Holder {
        session: "parent".into(),
        agent: "none".into(),
        pid: std::process::id(),
        command: "test".into(),
    };
    assert!(
        broker
            .acquire(Request::new(Resource::Gpu, me.clone()).wait(Wait::No))
            .is_err()
    );

    // SIGKILL: no release runs, and the lease is free on the next request.
    holder.kill().unwrap();
    holder.wait().unwrap();
    let lease = broker
        .acquire(Request::new(Resource::Gpu, me).wait(Wait::No))
        .expect("the killed holder's lease is free");
    assert_eq!(broker.list().unwrap().len(), 1);
    drop(lease);
    assert!(broker.list().unwrap().is_empty());
    assert_eq!(std::fs::read_dir(root.join("held")).unwrap().count(), 0);
}
