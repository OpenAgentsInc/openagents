//! Local operator pairing and the foreground Gym host.
use gym_bridge::{
    Error, ErrorCode, RelayPolicy,
    host::{Config, Host},
    unix_time,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};
fn failure(message: &str) -> Error {
    Error::new(ErrorCode::Malformed, message)
}
fn options(args: &[String]) -> Result<BTreeMap<String, String>, Error> {
    if !args.len().is_multiple_of(2) {
        return Err(failure("each Gym option needs one value"));
    }
    let mut found = BTreeMap::new();
    for pair in args.chunks_exact(2) {
        if ![
            "--state",
            "--config",
            "--client",
            "--relay",
            "--expires-in",
            "--grant",
        ]
        .contains(&pair[0].as_str())
            || found.insert(pair[0].clone(), pair[1].clone()).is_some()
        {
            return Err(failure("unknown or duplicate Gym option"));
        }
    }
    Ok(found)
}
fn required<'a>(options: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, Error> {
    options
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| failure("required Gym option missing; see --help"))
}
fn private_file(path: &Path) -> Result<std::fs::File, Error> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| failure("Gym host lock unavailable"))?;
    let m = file
        .metadata()
        .map_err(|_| failure("Gym host lock metadata unavailable"))?;
    // SAFETY: geteuid takes no parameters.
    if !m.is_file()
        || m.nlink() != 1
        || m.uid() != unsafe { libc::geteuid() }
        || m.permissions().mode() & 0o077 != 0
    {
        return Err(failure("Gym host lock is not private"));
    }
    file.try_lock()
        .map_err(|_| failure("another Gym host owns this state directory"))?;
    Ok(file)
}
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
async fn run() -> Result<(), Error> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args[0] == "--help" {
        println!(
            "gym-bridge pair --state DIR --config FILE --client PUBKEY --relay wss://RELAY [--expires-in SECONDS]\ngym-bridge serve --state DIR --relay wss://RELAY\ngym-bridge revoke --state DIR --grant GRANT_ID\n\nPair admits only the exact source roots and executable recipes in FILE. It prints a public gym-connect: code for that client key. Launches can spend provider credits; bounds do not enforce dollars. Serve runs in the foreground and never launches work until an authorized client asks."
        );
        return Ok(());
    }
    let options = options(&args[1..])?;
    let state = PathBuf::from(required(&options, "--state")?);
    let host = Host::new(&state, RelayPolicy::Production);
    match args[0].as_str() {
        "pair" => {
            use std::io::Read;
            let path = PathBuf::from(required(&options, "--config")?);
            let file =
                std::fs::File::open(&path).map_err(|_| failure("Gym configuration unavailable"))?;
            let mut bytes = Vec::new();
            file.take(128 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| failure("Gym configuration unreadable"))?;
            if bytes.len() > 128 * 1024 {
                return Err(failure("Gym configuration exceeds its bound"));
            }
            let value = nostr::contracts::parse_strict_bounded(&bytes, 128 * 1024)
                .map_err(|_| failure("invalid Gym configuration JSON"))?;
            let config: Config = serde_json::from_value(value)
                .map_err(|_| failure("unsupported Gym configuration fields"))?;
            let seconds = options
                .get("--expires-in")
                .map_or(Ok(24 * 60 * 60), |v| v.parse::<u64>())
                .map_err(|_| failure("invalid Gym grant duration"))?;
            let now = unix_time()?;
            eprintln!(
                "Admitting {} Gym source collections and {} executable recipes for the selected client.",
                config.sources.len(),
                config.recipes.len()
            );
            for source in &config.sources {
                eprintln!("Source: {} ({})", source.label, source.root.display());
            }
            for recipe in &config.recipes {
                eprintln!(
                    "Recipe: {} — {} ms per run; {} starts; no enforced dollar cap",
                    recipe.id, recipe.wall_ms, recipe.max_starts
                );
            }
            let code = host.pair(
                required(&options, "--client")?,
                required(&options, "--relay")?,
                config,
                now,
                now.checked_add(seconds)
                    .ok_or_else(|| failure("invalid Gym grant duration"))?,
            )?;
            eprintln!(
                "Gym grant {} expires at {}. Paste the connection code into Verse Gym settings.",
                code.grant, code.expires_at
            );
            println!("{}", code.encode()?);
        }
        "revoke" => {
            host.revoke(required(&options, "--grant")?)?;
            println!("Gym grant revoked. Previously admitted processes are not cancelled.");
        }
        "serve" => {
            let _owner = private_file(&state.join("serve.lock"))?;
            host.recover()?;
            let secret = host.key()?;
            let relay = required(&options, "--relay")?;
            RelayPolicy::Production.validate(relay)?;
            eprintln!(
                "Gym host listening for explicitly granted requests; no run starts automatically."
            );
            loop {
                let connected = tokio::select! {r=gym_bridge::transport::Receiver::connect(relay,&secret,RelayPolicy::Production)=>r,_=tokio::signal::ctrl_c()=>break};
                match connected {
                    Ok(mut receiver) => loop {
                        let request = tokio::select! {r=receiver.next_request()=>r,_=tokio::signal::ctrl_c()=>return Ok(())};
                        let Ok(request) = request else {
                            break;
                        };
                        if let Ok(reply) = host.handle_current(&request, relay)
                            && receiver.publish(&reply).await.is_err()
                        {
                            break;
                        }
                    },
                    Err(_) => eprintln!("Gym relay unavailable; retrying with bounded backoff."),
                }
                tokio::select! {_=tokio::time::sleep(Duration::from_secs(2))=>{},_=tokio::signal::ctrl_c()=>break}
            }
        }
        _ => return Err(failure("unknown Gym command; see --help")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn second_service_cannot_recover_an_active_hosts_store() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("serve.lock");
        let first = private_file(&path).unwrap();
        assert!(private_file(&path).is_err());
        drop(first);
        assert!(private_file(&path).is_ok());
    }
    #[test]
    fn repeated_or_unknown_cli_options_refuse() {
        assert!(
            options(&[
                "--state".into(),
                "one".into(),
                "--state".into(),
                "two".into()
            ])
            .is_err()
        );
        assert!(options(&["--command".into(), "anything".into()]).is_err());
    }
}
