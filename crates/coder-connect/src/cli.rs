//! Local pairing is explicit; serving discloses only those admitted roots.
use crate::pairing_ui;
use crate::{
    Error, ErrorCode, RelayPolicy, Result,
    host::{Host, ensure_parent},
    protocol,
    transport::Receiver,
    unix_time,
};
use nostr::domain::Event;
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

fn bad(message: &str) -> Error {
    Error::new(ErrorCode::Malformed, message)
}
struct Args {
    values: BTreeMap<String, String>,
    loopback: bool,
    once: bool,
    no_browser: bool,
    no_codex: bool,
    no_claude: bool,
}
impl Args {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self> {
        let mut result = Self {
            values: BTreeMap::new(),
            loopback: false,
            once: false,
            no_browser: false,
            no_codex: false,
            no_claude: false,
        };
        let mut args = args.peekable();
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--loopback-test" if !result.loopback => result.loopback = true,
                "--once" if !result.once => result.once = true,
                "--no-browser" if !result.no_browser => result.no_browser = true,
                "--no-codex" if !result.no_codex => result.no_codex = true,
                "--no-claude" if !result.no_claude => result.no_claude = true,
                _ if flag.starts_with("--") => {
                    let value = args
                        .next()
                        .filter(|v| !v.starts_with("--"))
                        .ok_or_else(|| bad("option requires a value"))?;
                    if result.values.insert(flag, value).is_some() {
                        return Err(bad("duplicate option"));
                    }
                }
                _ => return Err(bad("unexpected positional argument")),
            }
        }
        Ok(result)
    }
    fn value(&mut self, key: &str) -> Option<String> {
        self.values.remove(key)
    }
    fn required(&mut self, key: &str) -> Result<String> {
        self.value(key)
            .ok_or_else(|| bad(&format!("required option: {key}")))
    }
    fn finish(&self) -> Result<()> {
        if self.values.is_empty() {
            Ok(())
        } else {
            Err(bad("unknown option for this command"))
        }
    }
}
/// Run an observer command without creating another process or runtime.
///
/// Arguments exclude the binary name. With no arguments, shows a QR invitation.
pub async fn run(arguments: impl IntoIterator<Item = String>) -> Result<()> {
    let mut args = arguments.into_iter();
    let command = args.next().unwrap_or_else(|| "connect".into());
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        println!(
            "coder-connect connect [--relay wss://relay.openagents.com] [--codex-root PATH] [--claude-root PATH] [--coder-root PATH] [--no-codex] [--no-claude] [--expires-secs 86400] [--no-browser] [--state PATH]\n  Display a five-minute computer QR invitation, then keep serving read-only history. Defaults to existing ~/.codex and ~/.claude; --coder-root adds a Coder task directory such as ~/.openagents/tasks. With no arguments, runs connect.\ncoder-connect pair --client PUBKEY --relay wss://relay.example/ [--codex-root PATH] [--claude-root PATH] [--coder-root PATH] [--expires-secs 86400] [--state PATH]\ncoder-connect serve [--state PATH] [--relay URL] [--once]\ncoder-connect revoke --grant ID [--source ID] [--state PATH]\ncoder-connect public-key [--state PATH]\nKeys remain in the private local store. --loopback-test permits ws only for numeric loopback fixtures."
        );
        return Ok(());
    }
    let mut args = Args::parse(args)?;
    if command != "connect" && (args.no_browser || args.no_codex || args.no_claude) {
        return Err(bad("source exclusions and --no-browser belong to connect"));
    }
    let directory = match args.value("--state") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(
            std::env::var_os("HOME").ok_or_else(|| bad("HOME is unavailable; supply --state"))?,
        )
        .join(".openagents/coder-connect"),
    };
    let policy = if args.loopback {
        RelayPolicy::LoopbackTest
    } else {
        RelayPolicy::Production
    };
    let host = Host::new(&directory, policy);
    match command.as_str() {
        "connect" => {
            let relay = args
                .value("--relay")
                .unwrap_or_else(|| "wss://relay.openagents.com".into());
            policy.validate(&relay)?;
            let home = std::env::var_os("HOME").map(PathBuf::from);
            let codex = args.value("--codex-root");
            let claude = args.value("--claude-root");
            let coder = args.value("--coder-root").map(PathBuf::from);
            // A Coder root adds to the default roots rather than replacing them.
            let explicit_roots = codex.is_some() || claude.is_some();
            let select = |explicit: Option<String>,
                          disabled: bool,
                          folder: &str|
             -> Result<Option<PathBuf>> {
                if disabled && explicit.is_some() {
                    return Err(bad("a source cannot be selected and excluded together"));
                }
                if disabled {
                    return Ok(None);
                }
                if let Some(path) = explicit {
                    return Ok(Some(PathBuf::from(path)));
                }
                if explicit_roots {
                    return Ok(None);
                }
                Ok(home.as_ref().map(|h| h.join(folder)).filter(|p| p.is_dir()))
            };
            let config = coder_history::Config {
                codex: select(codex, args.no_codex, ".codex")?,
                claude: select(claude, args.no_claude, ".claude")?,
                coder,
                // No OpenCode or Devin roots: `coder host` serves only
                // Coder chats and their delegate sessions (#9920).
                opencode: None,
                devin: None,
            };
            let lifetime = args
                .value("--expires-secs")
                .map(|s| s.parse::<u64>().map_err(|_| bad("invalid grant lifetime")))
                .transpose()?
                .unwrap_or(86400);
            args.finish()?;
            if config.codex.is_none() && config.claude.is_none() && config.coder.is_none() {
                return Err(bad(
                    "no retained history roots found; select --codex-root or --claude-root",
                ));
            }
            println!("Pair your phone for read-only access to:");
            for (label, path) in [
                ("Codex", &config.codex),
                ("Claude", &config.claude),
                ("Coder", &config.coder),
            ] {
                if let Some(path) = path {
                    println!(
                        "  {label}: {}",
                        path.canonicalize()
                            .map_err(|_| bad("selected history root is unavailable"))?
                            .display()
                    );
                }
            }
            let now = unix_time()?;
            let expires = now
                .checked_add(lifetime)
                .ok_or_else(|| bad("grant lifetime overflow"))?;
            println!(
                "Relay: {relay}\nGrant expires at Unix time {expires}. This cannot run or control an agent."
            );
            ensure_parent(&directory)?;
            let code = host.invite(&relay, config, now, expires)?;
            let display =
                match pairing_ui::Display::show(&directory, &code, policy, !args.no_browser) {
                    Ok(display) => display,
                    Err(error) => {
                        if let Ok(invitation) =
                            crate::pairing::Invitation::parse(&code, now, policy)
                        {
                            let _ = host.cancel_invitation(&invitation.id);
                        }
                        return Err(error);
                    }
                };
            println!("Waiting for your phone to scan the QR code.");
            serve(host, relay, policy, args.once, Some(display)).await?;
        }
        "pair" => {
            let client = args.required("--client")?;
            let relay = args.required("--relay")?;
            let codex = args.value("--codex-root").map(PathBuf::from);
            let claude = args.value("--claude-root").map(PathBuf::from);
            let coder = args.value("--coder-root").map(PathBuf::from);
            let lifetime = args
                .value("--expires-secs")
                .map(|s| s.parse::<u64>().map_err(|_| bad("invalid grant lifetime")))
                .transpose()?
                .unwrap_or(86400);
            args.finish()?;
            if args.once {
                return Err(bad("--once belongs to serve"));
            }
            ensure_parent(&directory)?;
            let now = unix_time()?;
            let expires = now
                .checked_add(lifetime)
                .ok_or_else(|| bad("grant lifetime overflow"))?;
            let code = host.pair(
                &client,
                &relay,
                coder_history::Config {
                    codex,
                    claude,
                    coder,
                    opencode: None,
                    devin: None,
                },
                now,
                expires,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&code)
                    .map_err(|_| bad("connection serialization failed"))?
            );
        }
        "revoke" => {
            let grant = args.required("--grant")?;
            let source = args.value("--source");
            args.finish()?;
            if args.once {
                return Err(bad("--once belongs to serve"));
            }
            host.revoke(&grant, source.as_deref(), unix_time()?)?;
            println!("revoked");
        }
        "public-key" => {
            args.finish()?;
            if args.once {
                return Err(bad("--once belongs to serve"));
            }
            println!("{}", protocol::pubkey(&host.key()?));
        }
        "serve" => {
            let relays = host.relays(unix_time()?)?;
            let relay = match args.value("--relay") {
                Some(relay) if relays.contains(&relay) => relay,
                Some(_) => return Err(bad("relay has no active local grant")),
                None if relays.len() == 1 => relays[0].clone(),
                _ => return Err(bad("select one active relay with --relay")),
            };
            args.finish()?;
            serve(host, relay, policy, args.once, None).await?;
        }
        _ => return Err(bad("unknown command; use --help")),
    }
    Ok(())
}
/// Serve read-only history on `relay` until the process is interrupted,
/// without a pairing display. A resident host runs this in-process when it
/// hands out chat invitations over its tailnet (NIP-HOST tailnet admission).
pub async fn serve_observer(host: Host, relay: String, policy: RelayPolicy) -> Result<()> {
    // The store appears with the first chat invitation.
    while host.key().is_err() {
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    serve(host, relay, policy, false, None).await
}

/// Requests the relay loop answers at once.
const ANSWERING: usize = 8;
/// A relay connection that lasted this long ended with its lease, not a
/// fault: the next one opens at once.
const LEASED: Duration = Duration::from_secs(30);

async fn serve(
    host: Host,
    relay: String,
    policy: RelayPolicy,
    once: bool,
    mut display: Option<pairing_ui::Display>,
) -> Result<()> {
    let secret = host.key()?;
    let host = std::sync::Arc::new(host);
    let mut backoff = 1;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let stop = tokio::signal::ctrl_c();
    tokio::pin!(stop);
    // Requests are answered off this loop, several at once; their replies
    // come back here, to whichever connection is open, to be published.
    let (answered, mut replies) = tokio::sync::mpsc::unbounded_channel::<Result<Event>>();
    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(ANSWERING));
    // Requests this loop took, by event, so a renewed subscription's replay
    // of the last minute is not answered and published again.
    let mut taken: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    loop {
        let expiry_wait = display
            .as_ref()
            .filter(|d| d.active)
            .map(|d| {
                d.expires_at
                    .saturating_sub(unix_time().unwrap_or(d.expires_at))
            })
            .unwrap_or(300);
        let connection = tokio::select! {
            result = Receiver::connect(&relay, &secret, policy) => result,
            _ = &mut stop => {cancel_display(&host,&mut display);return Ok(());},
            _ = tokio::time::sleep(Duration::from_secs(expiry_wait)), if display.as_ref().is_some_and(|d|d.active) => {update_display(&host,&mut display)?;continue;},
        };
        let opened = std::time::Instant::now();
        match connection {
            Ok(mut receiver) => loop {
                tokio::select! {
                    reply = replies.recv() => match reply {
                        Some(Ok(reply)) => {
                            if let Err(error) = receiver.publish(&reply).await {
                                if once {
                                    return Err(error);
                                }
                                break;
                            }
                            backoff = 1;
                            update_display(&host, &mut display)?;
                            if once {
                                return Ok(());
                            }
                        }
                        Some(Err(error)) => {
                            // Record only a stable code, never source bodies, ciphertext, or paths.
                            eprintln!("observation refused: {:?}", error.code);
                            if once {
                                return Err(error);
                            }
                        }
                        None => return Err(Error::new(ErrorCode::Unavailable, "observer stopped")),
                    },
                    event = receiver.next_request() => {
                        let Ok(request) = event else { break };
                        let now = unix_time()?;
                        taken.retain(|_, until| *until > now);
                        if taken.insert(request.id.clone(), now + 120).is_some() {
                            continue;
                        }
                        let (host, relay, answered, permits) =
                            (host.clone(), relay.clone(), answered.clone(), permits.clone());
                        tokio::spawn(async move {
                            let Ok(_permit) = permits.acquire_owned().await else { return };
                            let reply = tokio::task::spawn_blocking(move || {
                                host.handle_current(&request, &relay)
                            })
                            .await
                            .unwrap_or_else(|_| {
                                Err(Error::new(ErrorCode::Unavailable, "observation failed"))
                            });
                            let _ = answered.send(reply);
                        });
                    },
                    _ = &mut stop => {cancel_display(&host,&mut display);return Ok(());},
                    _ = tick.tick(), if display.as_ref().is_some_and(|d| d.active) => {
                        update_display(&host, &mut display)?;
                    },
                }
            },
            Err(error) if once => return Err(error),
            Err(_) => eprintln!("relay unavailable; reconnecting after bounded backoff"),
        }
        update_display(&host, &mut display)?;
        if once {
            return Err(Error::new(
                ErrorCode::Transport,
                "finite observer connection ended",
            ));
        }
        if opened.elapsed() >= LEASED {
            backoff = 1;
            continue;
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(backoff.min(expiry_wait.max(1)))) => {},
            _ = &mut stop => {cancel_display(&host,&mut display);return Ok(());},
        }
        backoff = (backoff * 2).min(30);
    }
}
fn cancel_display(host: &Host, display: &mut Option<pairing_ui::Display>) {
    if let Some(display) = display {
        let _ = host.cancel_invitation(&display.id);
        display.clear();
    }
}
fn update_display(host: &Host, display: &mut Option<pairing_ui::Display>) -> Result<()> {
    let Some(display) = display.as_mut().filter(|d| d.active) else {
        return Ok(());
    };
    if let Some(grant) = host.invitation_grant(&display.id)? {
        display.clear();
        println!(
            "Phone paired. Chats are ready; keep this command running.\nGrant: {grant}\nTo revoke: coder-connect revoke --grant {grant}"
        );
    } else if unix_time()? >= display.expires_at {
        host.cancel_invitation(&display.id)?;
        display.clear();
        return Err(Error::new(
            ErrorCode::Expired,
            "pairing invitation expired; run openagents pair again",
        ));
    }
    Ok(())
}

/// Show a read-only phone invitation through the installed `openagents pair` command.
///
/// This accepts connection options only; it cannot dispatch another host command.
pub async fn pair(arguments: &[String]) -> Result<()> {
    if arguments.len() == 1 && matches!(arguments[0].as_str(), "--help" | "-h" | "help") {
        println!(
            "usage: openagents pair [--relay URL] [--codex-root PATH] [--claude-root PATH] [--coder-root PATH] [--no-codex] [--no-claude] [--expires-secs 86400] [--no-browser] [--state PATH]\nShow a QR code for read-only phone access to existing Codex and Claude chats. Keep this command running after pairing."
        );
        return Ok(());
    }
    run(std::iter::once("connect".to_owned()).chain(arguments.iter().cloned())).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn short_pair_help_does_not_open_a_host() {
        assert!(pair(&["--help".into()]).await.is_ok());
    }

    #[tokio::test]
    async fn short_pair_cannot_dispatch_other_observer_commands() {
        for command in ["serve", "revoke", "public-key", "pair"] {
            let error = pair(&[command.into()]).await.unwrap_err();
            assert_eq!(error.code, ErrorCode::Malformed);
            assert_eq!(error.message, "unexpected positional argument");
        }
    }

    #[tokio::test]
    async fn short_pair_preserves_source_exclusions_without_creating_state() {
        let temporary = tempfile::tempdir().unwrap();
        let state = temporary.path().join("host");
        let error = pair(&[
            "--state".into(),
            state.to_string_lossy().into_owned(),
            "--no-codex".into(),
            "--no-claude".into(),
        ])
        .await
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Malformed);
        assert!(error.message.contains("no retained history roots"));
        assert!(!state.exists());
    }
}
