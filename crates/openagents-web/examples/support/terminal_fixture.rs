//! Explicit scratch terminals and enrollment for browser acceptance.
//! This helper starts no engine and imports no browser device key.
use coder_host::{
    Running,
    client::{Device, Link},
    message::TermRequest,
};
use coder_pty::{
    ext::{Layout, Member, Node, SessionRecord, SessionWrite, Tab},
    wire::{Close, Launch, Open, Size, TerminalRef, Value},
};
use serde_json::json;
use std::{
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub struct Fixture {
    pub invitation_file: PathBuf,
    pub session: String,
    pub terminal: TerminalRef,
    pub route: String,
    pub capabilities: Vec<String>,
}

// These marks are original output from an explicit synthetic shell. They are
// advisory shell evidence, never proof that a command ran outside this fixture.
const SHELL: &str = r#"#!/bin/sh
printf '\033]7;file://%s\007' "$PWD"
printf '\033]133;A\007Synthetic scratch $ \033]133;B\007'
printf "printf 'Retained synthetic block\\n'\r\n"
printf '\033]777;openagents;command;7072696e7466202752657461696e65642073796e74686574696320626c6f636b5c6e27\007\033]133;C\007'
printf 'Retained synthetic block\r\n\033]133;D;0\007'
while :; do
    printf '\033]7;file://%s\007\033]133;A\007Synthetic scratch $ \033]133;B\007' "$PWD"
    IFS= read -r command || exit 0
    encoded=$(printf '%s' "$command" | /usr/bin/od -An -tx1 | /usr/bin/tr -d ' \n')
    printf '\033]777;openagents;command;%s\007\033]133;C\007' "$encoded"
    eval "$command"
    status=$?
    printf '\033]133;D;%s\007' "$status"
done
"#;
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("fixture clock")
        .as_secs()
}
fn private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|_| "Synthetic terminal fixture file could not be created.".into())
}
/// Configure only an isolated fixture process whose HOME already names scratch.
pub fn configure(config: &mut coder_host::Config, directory: &Path) -> Result<(), String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("Synthetic HOME is required.")?;
    if !home.starts_with(directory) || !directory.is_absolute() {
        return Err("Synthetic HOME must be inside this fixture.".into());
    }
    config.listen_websocket = Some(
        "127.0.0.1:0"
            .parse()
            .map_err(|_| "Synthetic listener failed.")?,
    );
    config.terminal_shell = Some(PathBuf::from("/bin/sh"));
    config.chat_home = None;
    config.control = Some(coder_host::config::Control {
        path: directory.join("terminal-control/socket"),
        root: directory.join("terminal-control"),
        autostart: None,
        tasks: directory.join("resident-tasks"),
        uid: unsafe { libc::geteuid() },
    });
    Ok(())
}
/// Open harmless scratch PTYs, retain an original session, and write a new invitation.
pub async fn seed(
    directory: &Path,
    running: &Running,
    relay: &str,
    workspace: &str,
    task: &str,
    thread: Option<&str>,
) -> Result<Fixture, String> {
    let authority = coder_access::host::Host::new(
        directory.join("resident-access"),
        coder_access::RelayPolicy::LoopbackTest,
    );
    let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let invitation = authority
        .invite(
            relay,
            coder_access::Rights::new([coder_access::Right::Terminal])
                .map_err(|_| "Synthetic terminal rights failed.")?,
            now(),
            now() + 3600,
        )
        .map_err(|_| "Synthetic local enrollment failed.")?;
    let parsed = coder_access::protocol::HostInvitation::parse(
        &invitation.code,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .map_err(|_| "Synthetic local invitation failed.")?;
    let pending = coder_access::client::prepare_redeem(
        &parsed,
        &secret,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .map_err(|_| "Synthetic local redemption failed.")?;
    let event = authority
        .handle_redemption(&pending.event, || Ok(now()))
        .map_err(|_| "Synthetic native local redemption failed.")?;
    let access = coder_access::client::finish_redeem(
        &parsed,
        &pending,
        &event,
        &secret,
        now(),
        coder_access::RelayPolicy::LoopbackTest,
    )
    .map_err(|_| "Synthetic local grant failed.")?;
    let device = Arc::new(
        Device::new(access, secret, coder_access::RelayPolicy::LoopbackTest)
            .map_err(|_| "Synthetic local device failed.")?,
    );
    let stream = tokio::net::TcpStream::connect(running.local_addr())
        .await
        .map_err(|_| "Synthetic local route failed.")?;
    let link = Link::direct(
        device,
        stream,
        running.local_addr().to_string(),
        running.generation(),
        Duration::from_secs(10),
    )
    .await
    .map_err(|_| "Synthetic local handshake failed.")?;
    let shell = directory.join("synthetic-terminal-shell.sh");
    private_file(&shell, SHELL.as_bytes())?;
    let mut open = Open::new(
        coder_access::protocol::random_id(),
        coder_host::mailbox::workspace_id(workspace),
        "",
        Launch::Command {
            program: "/bin/sh".into(),
            args: vec![shell.to_string_lossy().into_owned()],
        },
        Size::new(24, 80),
    );
    let result = link
        .terminal(TermRequest::Open(open.clone()))
        .await
        .map_err(|_| "Synthetic live terminal failed.")?;
    let Some(Value::Opened { terminal, .. }) = result.value else {
        return Err("Synthetic terminal owner refused the live PTY.".into());
    };
    seed_proposal(directory, running, &link, &terminal, thread).await?;
    open.request = coder_access::protocol::random_id();
    let result = link
        .terminal(TermRequest::Open(open))
        .await
        .map_err(|_| "Synthetic closed terminal failed.")?;
    let Some(Value::Opened {
        terminal: closed, ..
    }) = result.value
    else {
        return Err("Synthetic terminal owner refused the closed PTY.".into());
    };
    link.terminal(TermRequest::Close(Close::new(
        coder_access::protocol::random_id(),
        closed.clone(),
    )))
    .await
    .map_err(|_| "Synthetic terminal close failed.")?;
    let host = json!({"kind":"paired","key":running.host_key()});
    let reference = |kind: &str, id: &str, owner: serde_json::Value| json!({"v":"openagents.workbench-resource.v1","kind":kind,"host":owner,"id":id});
    let mut members = vec![
        Member::Terminal {
            member: 1,
            terminal: terminal.clone(),
            state: None,
        },
        Member::Terminal {
            member: 2,
            terminal: closed,
            state: None,
        },
        Member::Terminal {
            member: 3,
            terminal: TerminalRef {
                generation: "a".repeat(64),
                terminal: "b".repeat(64),
            },
            state: None,
        },
        Member::Resource {
            member: 4,
            resource: reference("run", task, host.clone()),
        },
        Member::Resource {
            member: 5,
            resource: reference(
                "thread",
                thread.unwrap_or("11111111111111111111111111111111"),
                host,
            ),
        },
        Member::Resource {
            member: 6,
            resource: reference(
                "thread",
                "22222222222222222222222222222222",
                json!({"kind":"paired","key":"e".repeat(64)}),
            ),
        },
    ];
    if thread.is_none() {
        members.push(Member::Resource {
            member: 7,
            resource: reference(
                "tool",
                "synthetic-unavailable",
                json!({"kind":"local","instance":"f".repeat(64)}),
            ),
        });
    }
    let layout = Layout {
        tabs: members
            .iter()
            .map(|m| Tab {
                name: format!("Synthetic member {}", m.id()),
                root: Node::Pane { member: m.id() },
            })
            .collect(),
        active: 0,
    };
    let record = SessionRecord {
        session: None,
        revision: 0,
        name: "Synthetic native mixed session".into(),
        members,
        layout,
    };
    let answer = link
        .terminal(TermRequest::SessionWrite(SessionWrite::new(
            coder_access::protocol::random_id(),
            None,
            0,
            record,
        )))
        .await
        .map_err(|_| "Synthetic native session write failed.")?;
    let Some(Value::Session { record }) = answer.value else {
        return Err("Synthetic owner refused the session.".into());
    };
    let session = record
        .session
        .ok_or("Synthetic owner omitted the session identity.")?;
    let invitation = authority
        .invite(
            relay,
            coder_access::Rights::new([
                coder_access::Right::Observe,
                coder_access::Right::Terminal,
            ])
            .map_err(|_| "Synthetic browser rights failed.")?,
            now(),
            now() + 3600,
        )
        .map_err(|_| "Synthetic browser invitation failed.")?;
    let invitation_file = directory.join("browser-terminal.invitation");
    private_file(&invitation_file, invitation.code.as_bytes())?;
    let route = running
        .websocket_url()
        .ok_or("Synthetic browser WebSocket route is unavailable.")?;
    let mut capabilities: Vec<String> = coder_host::CAPABILITIES
        .iter()
        .map(|s| (*s).into())
        .collect();
    for capability in running.terminal_features().capabilities() {
        if !capabilities.iter().any(|s| s == capability) {
            capabilities.push(capability.into());
        }
    }
    Ok(Fixture {
        invitation_file,
        session,
        terminal,
        route,
        capabilities,
    })
}

async fn seed_proposal(
    directory: &Path,
    running: &Running,
    link: &Link,
    terminal: &TerminalRef,
    thread: Option<&str>,
) -> Result<(), String> {
    use coder_pty::proposal::{self, Action, Proposal, Request};
    if !running.terminal_features().proposals {
        return Err("Synthetic native proposal feature is unavailable.".into());
    }
    let marker = directory.join("proposal-approved.marker");
    if marker.exists() {
        return Err("Synthetic proposal marker already exists.".into());
    }
    let command = format!("printf 'accepted\\n' > '{}'", marker.display());
    // The owner obtains the OS cwd and advisory directory. Wait only for this
    // original shell's first prompt, before any browser receives its invitation.
    for _ in 0..100 {
        let mut binding = running
            .terminal_proposal_binding(terminal)
            .map_err(|_| "Synthetic native proposal binding is unavailable.")?;
        let context = json!({
            "v":"openagents.synthetic-proposal-context.v1",
            "terminal":terminal,"cwd":binding.cwd,
            "shell_directory":binding.shell_directory,
            "command":command,"revision":1,
            "source":"synthetic-terminal-shell.sh"
        });
        binding.context_digest = proposal::digest(&context);
        let proposal = Proposal {
            thread: thread.unwrap_or("11111111111111111111111111111111").into(),
            id: "synthetic-native-proposal".into(),
            revision: 1,
            command: command.clone(),
            binding,
        };
        let answer = link
            .terminal(TermRequest::Proposal(Request::new(
                coder_access::protocol::random_id(),
                terminal.clone(),
                Action::Offer {
                    proposal: proposal.clone(),
                },
            )))
            .await
            .map_err(|_| "Synthetic native proposal offer failed.")?;
        if answer.status == coder_pty::wire::Status::Accepted {
            private_file(
                &directory.join("proposal-context.json"),
                &serde_json::to_vec(&context).map_err(|_| "Synthetic context failed.")?,
            )?;
            private_file(
                &directory.join("native-proposal.json"),
                &serde_json::to_vec(&proposal).map_err(|_| "Synthetic proposal failed.")?,
            )?;
            return Ok(());
        }
        if answer.reason != Some(coder_pty::wire::Reason::Stale) {
            return Err("Synthetic owner refused the proposal.".into());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Err("Synthetic shell did not reach its original prompt.".into())
}
