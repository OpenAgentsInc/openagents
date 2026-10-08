//! A retained native thread for isolated browser acceptance, without a provider.

use coder_access::{Code, Operation, Outcome, RelayPolicy, Right, Rights};
use coder_host::{
    Running,
    client::{Device, Link},
};
use openagents_chat::{
    basic_chats::{BasicChats, Summary},
    basic_coder::Turn,
    cache::Cache,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub const THREAD: &str = "11111111111111111111111111111111";
pub const TITLE: &str = "Synthetic retained native thread";
pub const QUESTION: &str = "Retain this original native thread for the browser reader.";
pub const ANSWER: &str = "This is retained synthetic evidence. No model or provider was called.";

pub struct Fixture {
    pub thread: &'static str,
    // Hold the socket directory until the isolated resident stops.
    _socket: tempfile::TempDir,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("fixture clock")
        .as_secs()
}

fn private_directory(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| "Synthetic thread directory is unavailable.")?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("Synthetic thread directory must be private and owned.".into());
    }
    Ok(())
}

/// Seed the existing owner's encrypted store after terminal fixture configuration.
pub fn configure(config: &mut coder_host::Config, directory: &Path) -> Result<Fixture, String> {
    if !directory.is_absolute()
        || directory.canonicalize().ok().as_deref() != Some(directory)
        || std::env::var_os("HOME")
            .map(PathBuf::from)
            .and_then(|home| home.canonicalize().ok())
            != Some(directory.join("synthetic-home"))
        || config.access != directory.join("resident-access")
        || config.policy != RelayPolicy::LoopbackTest
    {
        return Err("Synthetic thread configuration requires the isolated fixture.".into());
    }
    private_directory(directory)?;
    private_directory(&directory.join("synthetic-home"))?;
    let control = config
        .control
        .as_ref()
        .ok_or("Synthetic thread requires the native control service.")?;
    if control.root != directory.join("terminal-control")
        || control.tasks != directory.join("resident-tasks")
        || control.autostart.is_some()
        || control.uid != unsafe { libc::geteuid() }
    {
        return Err("Synthetic thread configuration differs from its native owner.".into());
    }
    let scratch = directory
        .ancestors()
        .find(|path| {
            path.file_name().is_some_and(|name| name == "scratch")
                && path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == ".openagents")
        })
        .ok_or("Synthetic thread socket requires the OpenAgents scratch directory.")?;
    if scratch.canonicalize().ok().as_deref() != Some(scratch) {
        return Err("Synthetic thread scratch directory cannot contain a symlink.".into());
    }
    private_directory(scratch)?;
    let socket = tempfile::Builder::new()
        .prefix("wb6t-")
        .tempdir_in(scratch)
        .map_err(|_| "Synthetic short socket directory failed.")?;
    std::fs::set_permissions(socket.path(), std::fs::Permissions::from_mode(0o700))
        .map_err(|_| "Synthetic short socket permissions failed.")?;
    private_directory(socket.path())?;
    let socket_path = socket.path().join("s");
    if socket_path.as_os_str().as_encoded_bytes().len() >= 104 {
        return Err("Synthetic socket path exceeds the native bound.".into());
    }
    std::fs::create_dir(&control.root)
        .map_err(|_| "Synthetic thread owner directory must be new.")?;
    std::fs::set_permissions(&control.root, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| "Synthetic thread owner permissions failed.")?;
    let authority = coder_access::host::Host::new(&config.access, config.policy);
    let mut secret = authority
        .signing_key()
        .map_err(|_| "Synthetic thread owner identity is unavailable.")?;
    let store = Cache::open(&control.root.join("basic-chats"), &secret)
        .map_err(|_| "Synthetic encrypted thread store failed.");
    secret.non_secure_erase();
    let mut chats = BasicChats::new(None, None, Some(store?));
    let timestamp = now();
    let adopted = chats
        .adopt(
            Summary {
                id: THREAD.into(),
                title: TITLE.into(),
                started: timestamp,
                updated: timestamp,
                coder: None,
                archived: false,
                pinned: false,
                named: true,
            },
            vec![Turn::user(QUESTION), Turn::assistant(ANSWER, None)],
            None,
        )
        .map_err(|_| "Synthetic original thread could not be retained.")?;
    if !adopted {
        return Err("Synthetic original thread already exists.".into());
    }
    chats.flush_pending();
    config.control.as_mut().expect("checked control").path = socket_path;
    config.chat_home = None;
    config.chat_door = Some(coder_host::config::ChatDoor {
        relay: config
            .primary()
            .map_err(|_| "Synthetic relay is unavailable.")?
            .into(),
        worker: coder_access::protocol::pubkey(&secp256k1::SecretKey::new(
            &mut secp256k1::rand::rng(),
        )),
    });
    Ok(Fixture {
        thread: THREAD,
        _socket: socket,
    })
}

/// Preserve the exact thread reference shared by native surfaces and Verse.
pub fn reference(host: &str) -> workbench::ResourceRef {
    workbench::ResourceRef::new(
        workbench::Kind::Thread,
        workbench::Host::Paired { key: host.into() },
        THREAD,
    )
}

/// Prove native observation and refusal of mutation using a separate scratch device.
pub async fn verify(
    directory: &Path,
    running: &Running,
    relay: &str,
    thread: &str,
) -> Result<(), String> {
    if thread != THREAD {
        return Err("Synthetic thread identity differs.".into());
    }
    let authority =
        coder_access::host::Host::new(directory.join("resident-access"), RelayPolicy::LoopbackTest);
    let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let timestamp = now();
    let invitation = authority
        .invite(
            relay,
            Rights::new([Right::Observe, Right::Terminal])
                .map_err(|_| "Synthetic thread rights failed.")?,
            timestamp,
            timestamp + 3600,
        )
        .map_err(|_| "Synthetic thread enrollment failed.")?;
    let invitation = coder_access::protocol::HostInvitation::parse(
        &invitation.code,
        timestamp,
        RelayPolicy::LoopbackTest,
    )
    .map_err(|_| "Synthetic thread invitation failed.")?;
    let pending = coder_access::client::prepare_redeem(
        &invitation,
        &secret,
        timestamp,
        RelayPolicy::LoopbackTest,
    )
    .map_err(|_| "Synthetic thread redemption failed.")?;
    let event = authority
        .handle_redemption(&pending.event, || Ok(now()))
        .map_err(|_| "Synthetic thread native enrollment failed.")?;
    let access = coder_access::client::finish_redeem(
        &invitation,
        &pending,
        &event,
        &secret,
        now(),
        RelayPolicy::LoopbackTest,
    )
    .map_err(|_| "Synthetic thread grant failed.")?;
    let device = Arc::new(
        Device::new(access, secret, RelayPolicy::LoopbackTest)
            .map_err(|_| "Synthetic thread device failed.")?,
    );
    let stream = tokio::net::TcpStream::connect(running.local_addr())
        .await
        .map_err(|_| "Synthetic thread route failed.")?;
    let link = Link::direct(
        device,
        stream,
        running.local_addr().to_string(),
        running.generation(),
        Duration::from_secs(10),
    )
    .await
    .map_err(|_| "Synthetic thread handshake failed.")?;
    let read = || Operation::ReadThread {
        thread: THREAD.into(),
        before: None,
    };
    let original = link
        .call(read())
        .await
        .map_err(|_| "Synthetic native thread read failed.")?;
    let Outcome::Thread { thread: page } = &original else {
        return Err("Synthetic native owner returned another resource.".into());
    };
    if page.thread != THREAD
        || page.title != TITLE
        || page.total != 2
        || page.start != 0
        || page.busy
        || !page.partial.is_empty()
        || page.failure.is_some()
        || page.turns.len() != 2
        || page.turns[0].text != QUESTION
        || page.turns[1].text != ANSWER
        || page
            .turns
            .iter()
            .any(|turn| turn.model.is_some() || turn.request.is_some())
    {
        return Err("Synthetic native thread differs from its retained source.".into());
    }
    reference(running.host_key())
        .check()
        .map_err(|_| "Synthetic canonical thread reference failed.")?;
    let refused = link
        .call(Operation::SendThread {
            thread: THREAD.into(),
            request: "33333333333333333333333333333333".into(),
            text: "This ungranted mutation must never reach a provider.".into(),
        })
        .await;
    if !matches!(refused, Err(coder_host::Error::Access(error))
        if error.code == Code::MissingRight && error.missing == Some(Right::Operate))
    {
        return Err("Synthetic native thread mutation was not refused.".into());
    }
    let current = link
        .call(read())
        .await
        .map_err(|_| "Synthetic native thread recheck failed.")?;
    if current != original {
        return Err("Synthetic thread changed after a refused mutation.".into());
    }
    Ok(())
}
