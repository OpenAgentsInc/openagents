//! The native client uses the same signed relay path as a real phone.
#![cfg(unix)]
use coder_access::{Code, RelayPolicy, Rights, host::Host};
use coder_sharing::Client;
use computer_driver::{CommandCursor, CommandProgress};
use secp256k1::SecretKey;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;
const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_account_clients_share_reopen_run_once_and_revoke_without_broad_access() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    let private = temp.path().join("private");
    std::fs::create_dir(&project).unwrap();
    std::fs::create_dir(&private).unwrap();
    std::fs::write(project.join("README.md"), b"hello").unwrap();
    std::fs::write(private.join("secret"), b"private").unwrap();
    let (relay, relay_task, _) = relay::start().await;
    let access = temp.path().join("access");
    let host = Host::new(&access, POLICY);
    host.init(&coder_host::reach::pubkey(&SecretKey::new(
        &mut secp256k1::rand::rng(),
    )))
    .unwrap();
    let mut config = coder_host::config::Config::new(access, vec![relay.clone()], 1);
    config.policy = POLICY;
    config.workspaces = BTreeMap::from([
        ("project".into(), project.canonicalize().unwrap()),
        ("private".into(), private.canonicalize().unwrap()),
    ]);
    let running = coder_host::start(config, Arc::new(coder_host::NoTasks))
        .await
        .unwrap();
    let now = coder_access::unix_time().unwrap();
    let owner = Client::open(&temp.path().join("owner"), "owner", "Owner", POLICY).unwrap();
    let code = host
        .invite(&relay, Rights::pairing(), now, now + 10 * 86400)
        .unwrap()
        .code;
    let id = owner.connect(&code, false, "Laptop").await.unwrap();
    let mut projects = owner.projects(&id).await.unwrap();
    projects.sort();
    assert_eq!(projects, ["private", "project"]);
    let guest_dir = temp.path().join("guest");
    let guest = Client::open(&guest_dir, "guest", "Guest", POLICY).unwrap();
    let outsider = Client::open(
        &temp.path().join("other-device"),
        "guest",
        "Other device",
        POLICY,
    )
    .unwrap();
    let operation = coder_access::protocol::random_id();
    let (code, expiry) = owner
        .invite(
            &id,
            &operation,
            &guest.recipient(),
            vec!["project".into()],
            true,
            now + 86400,
        )
        .await
        .unwrap();
    assert_eq!(
        owner
            .invite(
                &id,
                &operation,
                &guest.recipient(),
                vec!["project".into()],
                true,
                now + 86400
            )
            .await
            .unwrap(),
        (code.clone(), expiry)
    );
    assert_eq!(
        outsider
            .connect(&code, true, "Laptop")
            .await
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    let shared = guest.connect(&code, true, "Laptop").await.unwrap();
    assert!(guest.projects(&shared).await.is_err());
    assert_eq!(guest.computers().unwrap()[0].projects, ["project"]);
    assert!(guest.workspace(&shared, "private").is_err());
    assert!(
        guest
            .read_file(&shared, "project", "../private/secret")
            .await
            .is_err()
    );
    assert_eq!(
        guest
            .read_file(&shared, "project", "README.md")
            .await
            .unwrap(),
        b"hello"
    );
    guest
        .write_file(&shared, "project", "README.md", b"edited")
        .await
        .unwrap();
    let command = coder_access::protocol::random_id();
    guest
        .start(
            &shared,
            "project",
            &command,
            "printf x >> once; printf answer",
            5,
        )
        .await
        .unwrap();
    drop(guest);
    let guest = Client::open(&guest_dir, "guest", "Guest", POLICY).unwrap();
    guest
        .start(
            &shared,
            "project",
            &command,
            "printf x >> once; printf answer",
            5,
        )
        .await
        .unwrap();
    let read = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let read = guest
                .read_job(&command, CommandCursor::default())
                .await
                .unwrap();
            if read.progress != CommandProgress::Running {
                break read;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("command did not finish");
    assert_eq!(read.stdout, b"answer");
    assert_eq!(std::fs::read(project.join("once")).unwrap(), b"x");
    assert!(
        guest
            .start(&shared, "project", &command, "touch changed", 5)
            .await
            .is_err()
    );
    let readonly = coder_access::protocol::random_id();
    let (code, _) = owner
        .invite(
            &id,
            &readonly,
            &outsider.recipient(),
            vec!["private".into()],
            false,
            now + 86400,
        )
        .await
        .unwrap();
    let other = outsider.connect(&code, true, "Read-only").await.unwrap();
    assert_eq!(
        outsider
            .read_file(&other, "private", "secret")
            .await
            .unwrap(),
        b"private"
    );
    assert!(
        outsider
            .write_file(&other, "private", "secret", b"bad")
            .await
            .is_err()
    );
    assert!(
        outsider
            .start(
                &other,
                "private",
                &coder_access::protocol::random_id(),
                "touch bad",
                5
            )
            .await
            .is_err()
    );
    let shares = owner.shares(&id).await.unwrap();
    let grant = shares
        .iter()
        .find(|share| share.scope.as_ref().unwrap().roots[0].workspace == "project")
        .unwrap()
        .grant
        .clone();
    owner
        .revoke(&id, &coder_access::protocol::random_id(), &grant)
        .await
        .unwrap();
    assert!(
        guest
            .read_file(&shared, "project", "README.md")
            .await
            .is_err()
    );
    assert_eq!(
        outsider
            .read_file(&other, "private", "secret")
            .await
            .unwrap(),
        b"private"
    );
    assert_eq!(std::fs::read(project.join("README.md")).unwrap(), b"edited");
    guest.forget(&shared).unwrap();
    assert!(guest.computers().unwrap().is_empty());
    assert!(project.exists());
    running.shutdown().await;
    relay_task.abort();
}
