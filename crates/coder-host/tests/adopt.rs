//! A host set up the old way moves under the desktop app's key source with
//! `coder_service::adopt`, and a phone paired before still reaches it: the
//! host key, owner, and grants are the same. The keychain names are the
//! ones adoption writes.

use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::sync::Arc;
use std::time::Duration;

use coder_host::access::client::{finish_redeem, prepare_redeem};
use coder_host::access::host::{Host, Unconnected};
use coder_host::access::protocol::HostInvitation;
use coder_host::access::{RelayPolicy, Rights};
use coder_host::client::{Device, Link};
use coder_host::config::Config;
use coder_host::reach::pubkey;
use coder_host::serve::keys::{AdoptInto, KeyName, KeySource, Keys};
use coder_service::adopt::{self, Paths};
use secp256k1::SecretKey;
use tokio::net::TcpStream;

#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;

const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;

fn key() -> SecretKey {
    SecretKey::new(&mut secp256k1::rand::rng())
}

/// No service manager: the old setup here has no agent to stop.
struct NoAgent;

impl coder_service::service::Runner for NoAgent {
    fn run(
        &mut self,
        _: &str,
        _: &[String],
    ) -> coder_service::Result<coder_service::service::Output> {
        Err(coder_service::Error::Refused(
            "no service manager in this test".into(),
        ))
    }
}

async fn adopt_and_reconnect(source: Arc<dyn KeySource>) {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::under(home.path());
    let (relay, _task, _) = relay::start().await;

    // The old way: a host key file, an owner key file, and a paired phone.
    let owner = key();
    coder_host::access::host::ensure_parent(&paths.access).unwrap();
    let store = Host::new(&paths.access, POLICY);
    let host_key = store.init(&pubkey(&owner)).unwrap();
    let owner_dir = paths.owner_key.parent().unwrap();
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(owner_dir)
        .unwrap();
    let hex: String = owner
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    std::fs::write(&paths.owner_key, format!("{hex}\n")).unwrap();
    std::fs::set_permissions(&paths.owner_key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let now = coder_host::unix_time().unwrap();
    let code = store
        .invite(&relay, Rights::standard(), now, now + 86_400)
        .unwrap()
        .code;
    let phone = key();
    let invitation = HostInvitation::parse(&code, now, POLICY).unwrap();
    let pending = prepare_redeem(&invitation, &phone, now, POLICY).unwrap();
    let reply = store
        .handle(&pending.event, &relay, now, &mut Unconnected)
        .unwrap();
    let access = finish_redeem(&invitation, &pending, &reply, &phone, now, POLICY).unwrap();

    // Adopt into the key source, under the names adoption writes.
    let adopted = adopt::adopt(
        &paths,
        now,
        &mut AdoptInto(source.as_ref()),
        &mut NoAgent,
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(adopted.kept.active_grants, 1);
    assert!(!paths.access.join("host.key").exists());
    assert!(!paths.owner_key.exists());
    let held =
        |name| SecretKey::from_byte_array(*source.load(name).unwrap().unwrap().expose()).unwrap();
    assert_eq!(pubkey(&held(KeyName::Host)), host_key);
    assert_eq!(held(KeyName::Owner), owner);

    // The desktop app's host serves from the key source: same host, same
    // owner, and the phone paired the old way opens a channel.
    let mut config = Config::new(paths.access.clone(), vec![relay], 1);
    config.policy = POLICY;
    config.keys = Some(Keys(source.clone()));
    let running = coder_host::start(config, Arc::new(coder_host::NoTasks))
        .await
        .unwrap();
    assert_eq!(running.host_key(), host_key);
    assert_eq!(running.owner(), pubkey(&owner));
    let device = Arc::new(Device::new(access, phone, POLICY).unwrap());
    let address = running.local_addr();
    let link = Link::direct(
        device,
        TcpStream::connect(address).await.unwrap(),
        address.to_string(),
        running.generation(),
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    link.ping().await.unwrap();
    running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_adopted_host_keeps_its_phones_under_a_file_key_source() {
    let keys = tempfile::tempdir().unwrap();
    adopt_and_reconnect(Arc::new(coder_host::serve::keys::FileKeySource::new(
        keys.path().join("keys"),
    )))
    .await;
}

/// The real keychain code, in a temporary keychain file this test creates
/// and unlocks, so the login keychain is never touched.
#[cfg(target_os = "macos")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_adopted_host_keeps_its_phones_under_a_temporary_keychain() {
    use security_framework::os::macos::keychain::CreateOptions;
    let dir = tempfile::tempdir().unwrap();
    let keychain = CreateOptions::new()
        .password("openagents-test")
        .create(dir.path().join("test.keychain"))
        .unwrap();
    let source = coder_host::serve::keys::Keychain::in_keychain(keychain);
    assert_eq!(source.load(KeyName::Host).unwrap(), None);
    source.delete(KeyName::HostIroh).unwrap();
    adopt_and_reconnect(Arc::new(source)).await;
}
