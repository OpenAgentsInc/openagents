//! A real resident owner checks two queued principals without reopening admission.
use super::*;
use coder_host::Tasks;
use coder_host::access::protocol::{HostInvitation, Outcome};
use coder_host::access::{RelayPolicy, Right, Rights};
use coder_host::client::{Device, Link};
use std::time::Duration;

#[path = "../../../coder-control/src/tests/relay.rs"]
mod relay;

/// One host call finishes in well under a second alone, but the full
/// `cargo test -p coder --lib` run starves this two-worker runtime and the
/// host's blocking book writes. The bound only catches a hung host, so it is
/// generous; the link's own per-call deadline still applies underneath.
const CALL_BOUND: Duration = Duration::from_secs(60);

async fn enroll(
    authority: &coder_host::access::host::Host,
    relay: &str,
) -> (secp256k1::SecretKey, coder_host::access::protocol::Access) {
    let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let now = coder_host::unix_time().unwrap();
    let invitation = authority
        .invite(
            relay,
            Rights::new([Right::Observe, Right::Operate]).unwrap(),
            now,
            now + 3600,
        )
        .unwrap();
    let parsed = HostInvitation::parse(&invitation.code, now, RelayPolicy::LoopbackTest).unwrap();
    let pending = coder_host::access::client::prepare_redeem(
        &parsed,
        &secret,
        now,
        RelayPolicy::LoopbackTest,
    )
    .unwrap();
    let reply = authority
        .handle_redemption(&pending.event, || Ok(now))
        .unwrap();
    let access = coder_host::access::client::finish_redeem(
        &parsed,
        &pending,
        &reply,
        &secret,
        now,
        RelayPolicy::LoopbackTest,
    )
    .unwrap();
    (secret, access)
}

async fn connect(
    running: &coder_host::Running,
    secret: secp256k1::SecretKey,
    access: coder_host::access::protocol::Access,
) -> Link {
    let address = running.local_addr();
    let device = Arc::new(Device::new(access, secret, RelayPolicy::LoopbackTest).unwrap());
    Link::direct(
        device,
        tokio::net::TcpStream::connect(address).await.unwrap(),
        address.to_string(),
        running.generation(),
        Duration::from_secs(30),
    )
    .await
    .unwrap()
}

/// One host call, made again while the host answers `unavailable`. The
/// host's own sweep (`Tasks::tick`, run whenever the task store changes)
/// evaluates held commands under the same journal and task locks, and a
/// starved runtime can hold them past the store's lock wait; the host then
/// refuses the call as `unavailable`, the transient refusal a device
/// retries (#11565). Any other answer is the call's.
async fn call(link: &Link, op: Operation, what: &str) -> coder_host::Result<Outcome> {
    let deadline = tokio::time::Instant::now() + CALL_BOUND;
    loop {
        let answered = tokio::time::timeout_at(deadline, link.call(op.clone()))
            .await
            .unwrap_or_else(|_| panic!("{what} must finish"));
        match answered {
            Err(coder_host::Error::Access(error))
                if error.code == Code::Unavailable && tokio::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            other => return other,
        }
    }
}

async fn try_queue(
    link: &Link,
    task: &str,
    edit: QueueEdit,
    digest: Option<String>,
) -> coder_host::Result<(TaskQueue, String)> {
    let outcome = call(
        link,
        Operation::QueueTaskAtRevision {
            task: task.into(),
            revision: 1,
            edit,
            queue_digest: digest,
        },
        "queue admission",
    )
    .await?;
    let Outcome::QueueAtRevision {
        queue,
        queue_digest,
        ..
    } = outcome
    else {
        panic!("native queue required")
    };
    Ok((queue, queue_digest))
}

async fn queue(
    link: &Link,
    task: &str,
    edit: QueueEdit,
    digest: Option<String>,
) -> (TaskQueue, String) {
    try_queue(link, task, edit, digest).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_principal_queue_reorder_and_release_keep_current_admission() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path().canonicalize().unwrap();
    let checkout = root.join("checkout");
    std::fs::create_dir(&checkout).unwrap();
    let workspaces = BTreeMap::from([("checkout".into(), checkout)]);
    let inbox = Arc::new(
        Inbox::new(root.join("tasks"), workspaces.clone())
            .with_settings(root.join("unused-settings"))
            // The test is about admission, not lease expiry: a full
            // `cargo test -p coder --lib` run can starve this runtime
            // past the 60-second lease between the lease and the reorder
            // or the release, and a lapsed lease changes the queue digest,
            // which the host then refuses as stale (#11565).
            .with_queue_lease(3_600),
    );
    let task = "a".repeat(64);
    inbox
        .create(
            &task,
            "synthetic-device",
            &TaskCreate {
                title: "Synthetic mixed queue".into(),
                prompt: "An inert synthetic task.".into(),
                workspace: "checkout".into(),
                images: vec![],
                engine: None,
            },
        )
        .unwrap();
    let (relay_url, relay, _) = relay::start().await;
    let state = root.join("access");
    let authority = coder_host::access::host::Host::new(&state, RelayPolicy::LoopbackTest);
    let owner = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    authority
        .init(&coder_host::access::protocol::pubkey(&owner))
        .unwrap();
    let (first_secret, first_access) = enroll(&authority, &relay_url).await;
    let (second_secret, second_access) = enroll(&authority, &relay_url).await;
    let second_device = coder_host::access::protocol::pubkey(&second_secret);
    let mut config = coder_host::config::Config::new(state, vec![relay_url], 1);
    config.policy = RelayPolicy::LoopbackTest;
    config.workspaces = workspaces;
    config.telemetry = false;
    let running = coder_host::start(config, inbox.clone()).await.unwrap();
    let first = connect(&running, first_secret, first_access).await;
    let second = connect(&running, second_secret, second_access).await;
    let (_, digest) = queue(&first, &task, QueueEdit::List {}, None).await;
    queue(&first, &task, QueueEdit::Lease {}, Some(digest)).await;
    for (link, command, text) in [
        (&first, "b".repeat(64), "First device message."),
        (&second, "c".repeat(64), "Second device message."),
    ] {
        let command = TaskCommand {
            command,
            task: task.clone(),
            action: CommandAction::Queue,
            based_on: 1,
            text: text.into(),
            emulate: false,
            issued_at: coder_host::unix_time().unwrap(),
        };
        assert!(matches!(
            call(
                link,
                Operation::CommandTaskAtRevision {
                    command,
                    revision: 1
                },
                "mixed-device admission",
            )
            .await
            .unwrap(),
            Outcome::Dispatched { .. }
        ));
    }
    let (current, digest) = queue(&first, &task, QueueEdit::List {}, None).await;
    assert_eq!(current.items.len(), 2);
    assert!(current.items[0].text.is_some() && current.items[1].text.is_none());
    let (reordered, digest) = queue(
        &first,
        &task,
        QueueEdit::Reorder {
            commands: vec!["c".repeat(64), "b".repeat(64)],
        },
        Some(digest),
    )
    .await;
    assert_eq!(
        reordered
            .items
            .iter()
            .map(|item| item.command.clone())
            .collect::<Vec<_>>(),
        vec!["c".repeat(64), "b".repeat(64)]
    );
    let (released, _) = queue(&first, &task, QueueEdit::Release {}, Some(digest)).await;
    assert!(released.lease.is_none());
    assert_eq!(
        released.items.len(),
        2,
        "both admitted devices keep their held messages"
    );
    assert_eq!(
        inbox
            .task_read(&coder_host::access::task_read::PageQuery {
                workspace: "checkout".into(),
                task: task.clone(),
                revision: Some(1),
                cursor: None,
                limit: 1
            })
            .unwrap()
            .execution,
        "not_started"
    );
    authority
        .revoke(&second_device, coder_host::unix_time().unwrap())
        .unwrap();
    // The host's sweep rechecks standing too, and may drop the revoked
    // device's message between the read and the lease: the queue the read
    // named is then gone, the lease refuses as stale, and the device reads
    // again. Either way the lease is decided on the revocation.
    let mut current = None;
    for _ in 0..3 {
        let (_, digest) = queue(&first, &task, QueueEdit::List {}, None).await;
        match try_queue(&first, &task, QueueEdit::Lease {}, Some(digest)).await {
            Ok((leased, _)) => {
                current = Some(leased);
                break;
            }
            Err(coder_host::Error::Access(error)) if error.code == Code::Stale => {}
            Err(error) => panic!("the lease after the revocation: {error}"),
        }
    }
    let current = current.expect("the lease is granted on a current read");
    assert_eq!(
        current.items.len(),
        1,
        "revoked standing is rechecked before another effect"
    );
    assert_eq!(current.items[0].command, "b".repeat(64));
    running.shutdown().await;
    relay.abort();
}
