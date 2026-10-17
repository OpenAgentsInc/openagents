//! Exercise the adapter through the real signed direct and relay bindings.
#![cfg(unix)]
use coder_access::enrolled::{Action, Call, Scope};
use coder_access::host::Host;
use coder_access::{Code, Operation, RelayPolicy, Rights};
use coder_host::client::{Device, Link};
use computer_driver::{
    CommandCursor, CommandProgress, CommandSpec, CreateRequest, Driver, Exec, Outcome, Profile,
    State,
};
use computer_enrolled::{Enrolled, HostTransport};
use secp256k1::SecretKey;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;
const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}
fn done<T: std::fmt::Debug>(result: Outcome<T>) -> T {
    match result {
        Outcome::Done { value } => value,
        other => panic!("{other:?}"),
    }
}
struct Fixture {
    temp: tempfile::TempDir,
    relay_task: tokio::task::JoinHandle<()>,
    host: Host,
    running: coder_host::Running,
    device: Arc<Device>,
}
impl Fixture {
    async fn new(rights: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let (relay, relay_task, _) = relay::start().await;
        let access = temp.path().join("access");
        let host = Host::new(&access, POLICY);
        host.init(&coder_host::reach::pubkey(&key())).unwrap();
        let mut config = coder_host::config::Config::new(access, vec![relay.clone()], 1);
        config.policy = POLICY;
        config.workspaces = BTreeMap::from([("project".into(), root.clone())]);
        let running = coder_host::start(config, Arc::new(coder_host::NoTasks))
            .await
            .unwrap();
        let now = coder_access::unix_time().unwrap();
        let scope = Scope::local("alice", &[("project".into(), root)]).unwrap();
        let code = host
            .invite_scoped(
                &relay,
                Rights::parse_list(rights).unwrap(),
                scope,
                now,
                now + 3600,
            )
            .unwrap()
            .code;
        let secret = key();
        let access = coder_access::client::redeem(&code, &secret, POLICY)
            .await
            .unwrap();
        let device = Arc::new(Device::new(access, secret, POLICY).unwrap());
        Self {
            temp,
            relay_task,
            host,
            running,
            device,
        }
    }
    async fn direct(&self) -> Arc<Link> {
        let address = self.running.local_addr();
        Arc::new(
            Link::direct(
                self.device.clone(),
                tokio::net::TcpStream::connect(address).await.unwrap(),
                address.to_string(),
                1,
                Duration::from_secs(3),
            )
            .await
            .unwrap(),
        )
    }
    fn driver(&self, transport: HostTransport) -> Enrolled {
        Enrolled::new(transport, "alice", self.device.host(), "project").unwrap()
    }
    async fn finish(self) {
        self.running.shutdown().await;
        self.relay_task.abort();
    }
}
fn spec(script: &str) -> CommandSpec {
    CommandSpec {
        id: coder_access::protocol::random_id(),
        command: script.into(),
        cwd: String::new(),
        credential_names: BTreeSet::new(),
        env: BTreeMap::new(),
        timeout_seconds: 5,
        digest: coder_access::enrolled::hash(script.as_bytes()),
    }
}
async fn finished(driver: &Enrolled, computer: &str, id: &str) -> computer_driver::CommandRead {
    for _ in 0..100 {
        let read = done(
            driver
                .read_command(computer, id, CommandCursor::default(), 8192)
                .await,
        );
        if read.progress != CommandProgress::Running {
            return read;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("command did not finish");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direct_loss_relay_reconnect_and_service_retry_keep_one_command_and_safe_detach() {
    let f = Fixture::new("observe,operate").await;
    let link = f.direct().await;
    let direct = f.driver(HostTransport::linked(link.clone()));
    let request = CreateRequest {
        operation: "attach-1".into(),
        ..Default::default()
    };
    let computer = done(direct.create(&request).await);
    let command = spec("printf x >> once; printf answer");
    let id = done(direct.start_command(&computer, &command).await);
    link.shutdown();
    assert!(matches!(
        direct.observe(&computer).await,
        Outcome::Unknown { .. }
    ));
    let relay = f.driver(HostTransport::relay(f.device.clone()));
    assert_eq!(done(relay.create(&request).await), computer);
    assert_eq!(done(relay.start_command(&computer, &command).await), id);
    let read = finished(&relay, &computer, &id).await;
    assert_eq!(read.progress, CommandProgress::Exited { code: 0 });
    assert_eq!(read.stdout, b"answer");
    assert_eq!(read.digest.as_deref(), Some(command.digest.as_str()));
    let mut changed = command.clone();
    changed.command = "touch duplicate".into();
    assert!(matches!(
        relay.start_command(&computer, &changed).await,
        Outcome::Failed { .. }
    ));
    assert_eq!(
        std::fs::read(f.temp.path().join("project/once")).unwrap(),
        b"x"
    );
    let output = done(
        relay
            .exec(&computer, &Exec::new("printf '%070000d' 0; printf err >&2"))
            .await,
    );
    assert_eq!(output.stdout.len(), coder_access::enrolled::MAX_OUTPUT);
    assert!(output.stderr.contains("err") && output.stderr.contains("shortened"));
    let bytes = vec![42; 70_000];
    relay
        .write_workspace_file(&computer, "file", &bytes, false)
        .await
        .unwrap();
    assert_eq!(
        relay.read_workspace_file(&computer, "file").await.unwrap(),
        bytes
    );
    assert!(
        relay
            .write_workspace_file(&computer, "../escape", b"bad", false)
            .await
            .is_err()
    );
    let outside = f.temp.path().join("private");
    std::fs::write(&outside, b"secret").unwrap();
    std::os::unix::fs::symlink(&outside, f.temp.path().join("project/link")).unwrap();
    assert!(relay.read_workspace_file(&computer, "link").await.is_err());
    let command = spec("exec sleep 30");
    let id = done(relay.start_command(&computer, &command).await);
    done(relay.stop_command(&computer, &id).await);
    assert!(matches!(
        finished(&relay, &computer, &id).await.progress,
        CommandProgress::Exited { .. }
    ));
    done(relay.delete(&computer).await);
    done(relay.delete(&computer).await);
    assert_eq!(done(relay.observe(&computer).await).state, State::Gone);
    assert!(f.temp.path().join("project/once").exists());
    assert!(f.temp.path().join("project/file").exists());
    assert_eq!(
        f.host
            .devices(coder_access::unix_time().unwrap())
            .unwrap()
            .len(),
        1
    );
    // A new logical attachment and signed host reply prove the service is still alive.
    done(
        relay
            .create(&CreateRequest {
                operation: "attach-2".into(),
                ..Default::default()
            })
            .await,
    );
    f.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn account_host_rights_revocation_and_unsupported_features_refuse_before_effects() {
    let f = Fixture::new("observe").await;
    let transport = HostTransport::linked(f.direct().await);
    assert!(Enrolled::new(transport.clone(), "mallory", f.device.host(), "project").is_err());
    assert!(Enrolled::new(transport.clone(), "alice", &"ab".repeat(32), "project").is_err());
    assert!(Enrolled::new(transport.clone(), "alice", f.device.host(), "other").is_err());
    let driver = f.driver(transport);
    assert_eq!(
        driver.capabilities().profiles,
        BTreeSet::from([Profile::Core])
    );
    assert!(driver.capabilities().workspace_attachment);
    assert!(driver.capabilities().shapes.is_empty());
    assert!(driver.capabilities().validate().is_ok());
    for request in [
        CreateRequest {
            operation: "bad-size".into(),
            size: Some("small".into()),
            ..Default::default()
        },
        CreateRequest {
            operation: "bad-image".into(),
            image: Some("cloud-image".into()),
            ..Default::default()
        },
    ] {
        assert!(matches!(
            driver.create(&request).await,
            Outcome::Failed { .. }
        ));
    }
    let computer = done(
        driver
            .create(&CreateRequest {
                operation: "readonly".into(),
                ..Default::default()
            })
            .await,
    );
    assert!(driver.suspend(&computer).await.is_unsupported());
    assert!(driver.resume(&computer).await.is_unsupported());
    assert!(driver.snapshot(&computer, "x").await.is_unsupported());
    assert!(driver.fork(&computer, "x").await.is_unsupported());
    assert!(matches!(
        driver.start_command(&computer, &spec("touch denied")).await,
        Outcome::Failed { .. }
    ));
    assert!(matches!(
        driver
            .exec(&computer, &Exec::new(":").secret("TOKEN", "test-only"))
            .await,
        Outcome::Failed { .. }
    ));
    let link = f.direct().await;
    let result = link.call(Operation::ListWorkspaces {}).await;
    assert!(
        matches!(&result,Err(coder_host::Error::Access(e)) if matches!(e.code,Code::Forbidden|Code::MissingRight)),
        "{result:?}"
    );
    let guessed = Operation::Enrolled {
        computer: Call {
            account: "alice".into(),
            action: Action::Observe {
                computer: "ab".repeat(32),
            },
        },
    };
    assert!(
        matches!(link.call(guessed).await,Err(coder_host::Error::Access(e)) if e.code==Code::Forbidden)
    );
    f.host
        .revoke(&f.device.key(), coder_access::unix_time().unwrap())
        .unwrap();
    let relay = f.driver(HostTransport::relay(f.device.clone()));
    assert!(matches!(
        relay.observe(&computer).await,
        Outcome::Failed { .. }
    ));
    assert!(!f.temp.path().join("project/denied").exists());
    f.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_owner_phone_can_share_only_configured_projects_and_revoke_one_share() {
    let f = Fixture::new("observe,operate").await;
    let now = coder_access::unix_time().unwrap();
    let admin_key = key();
    let code = f
        .host
        .invite(f.device.relay(), Rights::pairing(), now, now + 86400)
        .unwrap()
        .code;
    let access = coder_access::client::redeem(&code, &admin_key, POLICY)
        .await
        .unwrap();
    let admin = Arc::new(Device::new(access, admin_key, POLICY).unwrap());
    let address = f.running.local_addr();
    let owner = Link::direct(
        admin,
        tokio::net::TcpStream::connect(address).await.unwrap(),
        address.to_string(),
        1,
        Duration::from_secs(3),
    )
    .await
    .unwrap();
    let guest_key = key();
    let request = coder_access::sharing::Invite {
        account: "guest-account".into(),
        device: coder_host::reach::pubkey(&guest_key),
        workspaces: vec!["project".into()],
        rights: Rights::parse_list("observe,operate").unwrap(),
        expires_at: now + 86400,
    };
    let mut wrong = request.clone();
    wrong.workspaces = vec!["../private".into()];
    assert!(
        matches!(owner.call(Operation::InviteProject {share:wrong}).await,Err(coder_host::Error::Access(error)) if error.code==Code::Forbidden)
    );
    let coder_access::Outcome::Invitation { code, .. } = owner
        .call(Operation::InviteProject { share: request })
        .await
        .unwrap()
    else {
        panic!("no invitation")
    };
    assert_eq!(
        coder_access::client::redeem(&code, &key(), POLICY)
            .await
            .unwrap_err()
            .code,
        Code::Forbidden
    );
    let access = coder_access::client::redeem(&code, &guest_key, POLICY)
        .await
        .unwrap();
    assert_eq!(
        access.grant.scope.as_ref().unwrap().roots[0].path,
        f.temp
            .path()
            .join("project")
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()
    );
    let grant = access.grant.grant.clone();
    let guest = Arc::new(Device::new(access, guest_key, POLICY).unwrap());
    let driver = Enrolled::new(
        HostTransport::relay(guest.clone()),
        "guest-account",
        guest.host(),
        "project",
    )
    .unwrap();
    let computer = done(
        driver
            .create(&CreateRequest {
                operation: "guest-attach".into(),
                ..Default::default()
            })
            .await,
    );
    assert_eq!(
        done(
            driver
                .exec(&computer, &Exec::new("printf guest > shared-file"))
                .await
        )
        .code,
        0
    );
    owner
        .call(Operation::RevokeProject { grant })
        .await
        .unwrap();
    assert!(matches!(
        driver.observe(&computer).await,
        Outcome::Failed { .. }
    ));
    let original = f.driver(HostTransport::relay(f.device.clone()));
    let computer = done(
        original
            .create(&CreateRequest {
                operation: "original-still-shared".into(),
                ..Default::default()
            })
            .await,
    );
    assert_eq!(
        original
            .read_workspace_file(&computer, "shared-file")
            .await
            .unwrap(),
        b"guest"
    );
    assert!(coder_host::CAPABILITIES.contains(&coder_access::sharing::CAPABILITY));
    f.finish().await;
}
