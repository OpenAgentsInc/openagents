//! Local pairing is explicit; serving discloses only those admitted roots.
use coder_connect::{
    Error, ErrorCode, RelayPolicy, Result,
    host::{Host, ensure_parent},
    protocol,
    transport::Receiver,
    unix_time,
};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

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
}
impl Args {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self> {
        let mut result = Self {
            values: BTreeMap::new(),
            loopback: false,
            once: false,
        };
        let mut args = args.peekable();
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--loopback-test" if !result.loopback => result.loopback = true,
                "--once" if !result.once => result.once = true,
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
    let command = args
        .next()
        .ok_or_else(|| bad("use pair, serve, revoke, or public-key"))?;
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        println!(
            "coder-connect pair --client PUBKEY --relay wss://relay.example/ [--codex-root PATH] [--claude-root PATH] [--expires-secs 86400] [--state PATH]\ncoder-connect serve [--state PATH] [--relay URL] [--once]\ncoder-connect revoke --grant ID [--source ID] [--state PATH]\ncoder-connect public-key [--state PATH]\nPairing returns public connection JSON; keys remain in the private local store. --loopback-test permits ws only for numeric loopback fixtures."
        );
        return Ok(());
    }
    let mut args = Args::parse(args)?;
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
            serve(host, relay, policy, args.once).await?;
        }
        _ => return Err(bad("unknown command; use --help")),
    }
    Ok(())
}
async fn serve(host: Host, relay: String, policy: RelayPolicy, once: bool) -> Result<()> {
    let secret = host.key()?;
    let mut backoff = 1;
    loop {
        let connection = tokio::select! {
            result = Receiver::connect(&relay, &secret, policy) => result,
            _ = tokio::signal::ctrl_c() => return Ok(()),
        };
        match connection {
            Ok(mut receiver) => {
                loop {
                    let next = tokio::select! {
                        event = receiver.next_request() => event,
                        _ = tokio::signal::ctrl_c() => return Ok(()),
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
        if once {
            return Err(Error::new(
                ErrorCode::Transport,
                "finite observer connection ended",
            ));
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(backoff)) => {},
            _ = tokio::signal::ctrl_c() => return Ok(()),
        }
        backoff = (backoff * 2).min(30);
    }
}
