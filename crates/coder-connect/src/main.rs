//! Local pairing is explicit; serving discloses only those admitted roots.
use coder_connect::{
    Error, ErrorCode, RelayPolicy, Result,
    host::{Host, ensure_parent},
    protocol,
    transport::Receiver,
    unix_time,
};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
mod pairing_ui;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
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
async fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "connect".into());
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        println!(
            "coder-connect connect [--relay wss://relay.openagents.com] [--codex-root PATH] [--claude-root PATH] [--no-codex] [--no-claude] [--expires-secs 86400] [--no-browser] [--state PATH]\n  Display a five-minute computer QR invitation, then keep serving read-only history. Defaults to existing ~/.codex and ~/.claude. With no arguments, runs connect.\ncoder-connect pair --client PUBKEY --relay wss://relay.example/ [--codex-root PATH] [--claude-root PATH] [--expires-secs 86400] [--state PATH]\ncoder-connect serve [--state PATH] [--relay URL] [--once]\ncoder-connect revoke --grant ID [--source ID] [--state PATH]\ncoder-connect public-key [--state PATH]\nKeys remain in the private local store. --loopback-test permits ws only for numeric loopback fixtures."
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
            };
            let lifetime = args
                .value("--expires-secs")
                .map(|s| s.parse::<u64>().map_err(|_| bad("invalid grant lifetime")))
                .transpose()?
                .unwrap_or(86400);
            args.finish()?;
            if config.codex.is_none() && config.claude.is_none() {
                return Err(bad(
                    "no retained history roots found; select --codex-root or --claude-root",
                ));
            }
            println!(
                "Read-only phone connection. The paired phone can read these retained chat collections:"
            );
            for (label, path) in [("Codex", &config.codex), ("Claude", &config.claude)] {
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
                            coder_connect::pairing::Invitation::parse(&code, now, policy)
                        {
                            let _ = host.cancel_invitation(&invitation.id);
                        }
                        return Err(error);
                    }
                };
            serve(host, relay, policy, args.once, Some(display)).await?;
        }
        "pair" => {
            let client = args.required("--client")?;
            let relay = args.required("--relay")?;
            let codex = args.value("--codex-root").map(PathBuf::from);
            let claude = args.value("--claude-root").map(PathBuf::from);
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
                coder_history::Config { codex, claude },
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
async fn serve(
    host: Host,
    relay: String,
    policy: RelayPolicy,
    once: bool,
    mut display: Option<pairing_ui::Display>,
) -> Result<()> {
    let secret = host.key()?;
    let mut backoff = 1;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let stop = tokio::signal::ctrl_c();
    tokio::pin!(stop);
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
        match connection {
            Ok(mut receiver) => {
                loop {
                    let next = tokio::select! {
                        event = receiver.next_request() => event,
                        _ = &mut stop => {cancel_display(&host,&mut display);return Ok(());},
                        _ = tick.tick(), if display.as_ref().is_some_and(|d|d.active) => {
                            update_display(&host,&mut display)?;
                            continue;
                        },
                    };
                    let request = match next {
                        Ok(event) => event,
                        Err(_) => break,
                    };
                    match host.handle_current(&request, &relay) {
                        Ok(reply) => {
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
                        Err(error) => {
                            // Record only a stable code, never source bodies, ciphertext, or paths.
                            eprintln!("observation refused: {:?}", error.code);
                            if once {
                                return Err(error);
                            }
                        }
                    }
                }
            }
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
            "Phone paired. Read-only history is serving; keep this command running.\nGrant: {grant}\nTo revoke: coder-connect revoke --grant {grant}"
        );
    } else if unix_time()? >= display.expires_at {
        host.cancel_invitation(&display.id)?;
        display.clear();
        return Err(Error::new(
            ErrorCode::Expired,
            "pairing invitation expired; run coder-connect connect again",
        ));
    }
    Ok(())
}
