//! `coder link COMMAND`.

use std::collections::BTreeMap;
use std::io::{IsTerminal, Read};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use coder_access::host::Host;
use coder_access::{RelayPolicy, Rights};
use coder_host::settings::ServeSettings;
use serde_json::json;

use crate::devices::{Client, Which};
use crate::ssh::Remote;
use crate::{Error, Result, devices, directory, owner, plan, service, tailscale};

pub const EXIT_USAGE: u8 = 2;
pub const EXIT_FAILED: u8 = 1;

pub const USAGE: &str = "usage: coder link COMMAND [OPTIONS]
  setup     [--owner PUBKEY | --owner-key FILE] [--relay URL]... [--workspace LABEL=PATH]...
            [--label NAME] [--port N] [--tls auto|off] [--no-service] [--linger]
            [--no-directory] [--tailscale PATH] [--coder-service PATH]
            Make this computer a serving host: record the host, a WebSocket
            listener on its tailnet address with a tailnet hint, start the host
            service, and list the host in the owner directory.
  invite    --rights LIST [--grant-days N] [--plain]
            Mint a one-use invitation for a phone or computer: a QR code and
            the paste string, valid five minutes. LIST is explicit, such as
            observe,operate,terminal.
  join      [--label NAME]     Redeem an invitation read from standard input,
            so this computer can reach that host.
  peer      --ssh DEST [--rights LIST] [--label NAME] [--remote-label NAME]
            Enroll this computer and DEST as devices of each other's hosts.
  check     [--direct | --relay-only]   Prove every route to every joined host.
  status    Show the host, its settings, service, devices, and joined hosts.
  owner     init | show [--owner-key FILE]
  directory list | add --host KEY --label NAME [--relay URL]... | remove --host KEY
            [--owner-key FILE]
setup, invite, check, and status also take --ssh DEST [--remote-coder PATH] to
run on another machine; setup then lists that host with this computer's
owner key. The owner key defaults to $OPENAGENTS_OWNER_KEY_FILE, else
~/.openagents/coder-owner/owner.key. Nothing prints a secret key; an
invitation prints only on this terminal.";

/// The rights one linked computer gets on another: everything a person at
/// that computer could do there, short of changing access.
const PEER_RIGHTS: &str = "observe,operate,terminal,review,access_read";

/// Run `coder link ARGS`. Returns the exit code.
pub async fn run(args: &[String]) -> u8 {
    let Some((command, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    };
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        println!("{USAGE}");
        return 0;
    }
    let mut options = match Options::parse(rest) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("coder link: {message}\n\n{USAGE}");
            return EXIT_USAGE;
        }
    };
    let result = match command.as_str() {
        "setup" => setup(&mut options).await,
        "invite" => invite(&mut options),
        "join" => join(&mut options).await,
        "peer" => peer(&mut options).await,
        "check" => check(&mut options).await,
        "status" => status(&mut options).await,
        "owner" => owner_command(&mut options),
        "directory" => directory_command(&mut options).await,
        _ => Err(usage(&format!("unknown command `{command}`"))),
    };
    match result {
        Ok(code) => code,
        Err(error) if error.to_string().starts_with("usage: ") => {
            eprintln!("coder link: {}\n\n{USAGE}", &error.to_string()[7..]);
            EXIT_USAGE
        }
        Err(error) => {
            eprintln!("coder link: {error}");
            EXIT_FAILED
        }
    }
}

fn usage(message: &str) -> Error {
    Error::new(format!("usage: {message}"))
}

/// Parsed options: positional words, repeatable values, and flags.
struct Options {
    words: Vec<String>,
    values: BTreeMap<String, Vec<String>>,
    flags: Vec<String>,
}

const FLAGS: [&str; 7] = [
    "--no-service",
    "--linger",
    "--no-directory",
    "--plain",
    "--direct",
    "--relay-only",
    "--json",
];

impl Options {
    fn parse(args: &[String]) -> std::result::Result<Self, String> {
        let mut options = Self {
            words: Vec::new(),
            values: BTreeMap::new(),
            flags: Vec::new(),
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if FLAGS.contains(&arg.as_str()) {
                if options.flags.contains(arg) {
                    return Err(format!("{arg} is given twice"));
                }
                options.flags.push(arg.clone());
            } else if arg.starts_with("--") {
                let value = args
                    .next()
                    .filter(|v| !v.starts_with("--"))
                    .ok_or_else(|| format!("{arg} needs a value"))?;
                options
                    .values
                    .entry(arg.clone())
                    .or_default()
                    .push(value.clone());
            } else {
                options.words.push(arg.clone());
            }
        }
        Ok(options)
    }

    fn flag(&mut self, name: &str) -> bool {
        let position = self.flags.iter().position(|f| f == name);
        position.map(|i| self.flags.remove(i)).is_some()
    }

    fn all(&mut self, name: &str) -> Vec<String> {
        self.values.remove(name).unwrap_or_default()
    }

    fn one(&mut self, name: &str) -> Result<Option<String>> {
        let mut values = self.all(name);
        match values.len() {
            0 => Ok(None),
            1 => Ok(values.pop()),
            _ => Err(usage(&format!("{name} is given twice"))),
        }
    }

    fn word(&mut self) -> Option<String> {
        (!self.words.is_empty()).then(|| self.words.remove(0))
    }

    fn finish(&self) -> Result<()> {
        if let Some(word) = self.words.first() {
            return Err(usage(&format!("unexpected argument `{word}`")));
        }
        match (self.values.keys().next(), self.flags.first()) {
            (None, None) => Ok(()),
            (Some(name), _) | (None, Some(name)) => {
                Err(usage(&format!("{name} does not apply to this command")))
            }
        }
    }

    /// Everything left, as arguments to forward to a remote `coder link`.
    fn forward(&mut self) -> Vec<String> {
        let mut args = std::mem::take(&mut self.words);
        for (name, values) in std::mem::take(&mut self.values) {
            for value in values {
                args.push(name.clone());
                args.push(value);
            }
        }
        args.extend(std::mem::take(&mut self.flags));
        args
    }

    fn remote(&mut self) -> Result<Option<Remote>> {
        let destination = self.one("--ssh")?;
        let coder = self.one("--remote-coder")?;
        match destination {
            Some(destination) => Ok(Some(Remote::new(&destination, coder.as_deref())?)),
            None if coder.is_some() => Err(usage("--remote-coder needs --ssh")),
            None => Ok(None),
        }
    }

    fn owner_key_file(&mut self) -> Result<PathBuf> {
        match self.one("--owner-key")? {
            Some(path) => Ok(PathBuf::from(path)),
            None => owner::default_path(),
        }
    }
}

fn access_state() -> Result<PathBuf> {
    crate::home(".openagents/coder-access")
}

fn host_root() -> Result<PathBuf> {
    crate::home(".openagents/host")
}

fn now() -> u64 {
    coder_host::unix_time().unwrap_or(0)
}

/// `coder link setup`.
async fn setup(options: &mut Options) -> Result<u8> {
    if let Some(remote) = options.remote()? {
        return setup_remote(options, &remote).await;
    }
    let owner_given = options.one("--owner")?;
    let key_file = options.owner_key_file()?;
    let relays = options.all("--relay");
    let mut workspaces = BTreeMap::new();
    for entry in options.all("--workspace") {
        let (label, path) = entry
            .split_once('=')
            .ok_or_else(|| usage("--workspace takes LABEL=PATH"))?;
        let path = std::fs::canonicalize(path)
            .map_err(|_| Error::new(format!("the workspace root {path} does not exist")))?;
        workspaces.insert(label.to_owned(), path);
    }
    let label = options.one("--label")?;
    let port = match options.one("--port")? {
        Some(port) => port
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| usage("--port takes a port number"))?,
        None => plan::DEFAULT_PORT,
    };
    let tls_mode = options.one("--tls")?.unwrap_or_else(|| "auto".into());
    if !matches!(tls_mode.as_str(), "auto" | "off") {
        return Err(usage("--tls takes auto or off"));
    }
    let no_service = options.flag("--no-service");
    let linger = options.flag("--linger");
    let no_directory = options.flag("--no-directory");
    let tailscale_program =
        tailscale::program(options.one("--tailscale")?.as_deref().map(Path::new));
    let service_program =
        service::program(options.one("--coder-service")?.as_deref().map(Path::new));
    let json_out = options.flag("--json");
    options.finish()?;

    // The owner: a given public key, else the owner key file's public half.
    let owner_secret = if owner_given.is_none() || !no_directory {
        owner::load(&key_file).ok()
    } else {
        None
    };
    let owner_public = match (&owner_given, &owner_secret) {
        (Some(given), _) => owner::public_key(given)?,
        (None, Some(secret)) => coder_reach::pubkey(secret),
        (None, None) => {
            return Err(Error::new(format!(
                "no owner: pass --owner PUBKEY, or create the owner key with `coder link owner init` (looked in {})",
                key_file.display()
            )));
        }
    };
    if let (Some(given), Some(secret)) = (&owner_given, &owner_secret)
        && owner::public_key(given)? != coder_reach::pubkey(secret)
    {
        return Err(Error::new(
            "--owner differs from the owner key file's public key",
        ));
    }

    let node = tailscale::node(&tailscale_program)?;
    let label = label
        .or_else(|| node.short_name().map(str::to_owned))
        .unwrap_or_else(|| "computer".into());
    let root = host_root()?;
    let state = access_state()?;

    // TLS from `tailscale cert` when the tailnet issues certificates.
    let tls_dir = root.join("tls");
    let files = plan::TlsFiles {
        cert: tls_dir.join("chain.pem"),
        key: tls_dir.join("key.pem"),
    };
    let before = std::fs::read(&files.cert).ok();
    let tls = match (&tls_mode[..], node.certificates, node.dns_name.as_deref()) {
        ("auto", true, Some(name)) => {
            match tailscale::cert(&tailscale_program, name, &files.cert, &files.key) {
                Ok(()) => Some(files.clone()),
                Err(error) => {
                    eprintln!("coder link: {error}; serving plain ws on the tailnet instead");
                    None
                }
            }
        }
        ("auto", _, _) => {
            eprintln!(
                "coder link: this tailnet issues no certificate for this machine; serving plain ws on the tailnet"
            );
            None
        }
        _ => None,
    };
    let cert_changed = tls.is_some() && std::fs::read(&files.cert).ok() != before;

    let previous = ServeSettings::load(&root)?;
    let next = plan::settings(&previous, &relays, &workspaces, &node, port, tls.as_ref());
    let mut check = coder_host::Config::new(state.clone(), next.relays.clone(), 1);
    check.listen_websocket = next.listen_websocket;
    check.websocket_tls = next.tls();
    check.allow_nonloopback = next.allow_nonloopback;
    check.advertise = next.advertised()?;
    check.workspaces = next.workspaces.clone();
    check.validate()?;
    coder_access::host::ensure_parent(&state)?;
    let host = Host::new(&state, RelayPolicy::Production);
    host.init(&owner_public)?;
    let host_key = host.public_key()?;
    let changed = previous != next || cert_changed;
    if changed {
        next.save(&root)?;
    }
    eprintln!("coder link: host {host_key} ({label}) owned by {owner_public}");
    for advertise in &next.advertise {
        eprintln!(
            "coder link: advertises {} {} and the relays {}",
            advertise.class,
            advertise.address,
            next.relays.join(", ")
        );
    }

    let mut service_state = "skipped".to_owned();
    if !no_service {
        let bundle_root = crate::home(".openagents/host-bundle")?;
        let status = service::status(&service_program)?;
        let selected = service::selected(&bundle_root)?;
        let step = service::decide(status.as_ref(), selected.as_deref(), changed)?;
        eprintln!("coder link: host service: {step:?}");
        service::apply(&service_program, &step, &host_key, linger)?;
        let listen = next
            .listen_websocket
            .ok_or_else(|| Error::new("no WebSocket listener"))?;
        wait_listening(listen, Duration::from_secs(90))?;
        let status = service::status(&service_program)?
            .ok_or_else(|| Error::new("the host service is not installed"))?;
        service_state = format!(
            "running={} starts_at={}",
            status.running,
            status.starts_at.unwrap_or_default()
        );
        eprintln!("coder link: host service {service_state}; listening on {listen}");
    }

    let mut listed = "skipped".to_owned();
    if !no_directory {
        match &owner_secret {
            Some(secret) => {
                listed = list_host(secret, &host_key, &label, &next.relays).await?;
            }
            None => eprintln!(
                "coder link: no owner key here, so the host is not listed; run `coder link directory add` where the owner key lives"
            ),
        }
    }
    let summary = json!({
        "host": host_key,
        "label": label,
        "owner": owner_public,
        "relays": next.relays,
        "advertise": next.advertise.iter().map(|a| format!("{}={}", a.class, a.address)).collect::<Vec<_>>(),
        "workspaces": next.workspaces.keys().collect::<Vec<_>>(),
        "service": service_state,
        "directory": listed,
    });
    if json_out {
        println!("{summary}");
    } else {
        println!("linked {host_key} {label}");
    }
    Ok(0)
}

/// Set up another machine over SSH with the owner's public key, then list
/// its host with the owner key held here.
async fn setup_remote(options: &mut Options, remote: &Remote) -> Result<u8> {
    let key_file = options.owner_key_file()?;
    let no_directory = options.flag("--no-directory");
    let given = options.one("--owner")?;
    let secret = owner::load(&key_file).ok();
    let owner_public = match (&given, &secret) {
        (Some(given), _) => owner::public_key(given)?,
        (None, Some(secret)) => coder_reach::pubkey(secret),
        (None, None) => {
            return Err(Error::new(
                "no owner: pass --owner PUBKEY or create the owner key with `coder link owner init`",
            ));
        }
    };
    let _ = options.flag("--json");
    let mut args = vec![
        "setup".to_owned(),
        "--owner".into(),
        owner_public,
        "--no-directory".into(),
        "--json".into(),
    ];
    args.extend(options.forward());
    eprintln!("coder link: setting up {} over ssh", remote.destination);
    let output = remote.link(&args, None, false)?;
    if !output.success {
        return Err(Error::new(format!(
            "setup on {} failed; its errors are above",
            remote.destination
        )));
    }
    let summary: serde_json::Value = output
        .stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line).ok())
        .ok_or_else(|| Error::new("the remote setup printed no summary"))?;
    let host = summary["host"].as_str().unwrap_or_default().to_owned();
    let label = summary["label"].as_str().unwrap_or_default().to_owned();
    let relays: Vec<String> = summary["relays"]
        .as_array()
        .map(|relays| {
            relays
                .iter()
                .filter_map(|r| r.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    coder_reach::parse_pubkey(&host).map_err(|_| Error::new("the remote host key is invalid"))?;
    let listed = match (&secret, no_directory) {
        (Some(secret), false) => list_host(secret, &host, &label, &relays).await?,
        _ => "skipped".into(),
    };
    eprintln!("coder link: directory {listed}");
    println!("linked {host} {label} via {}", remote.destination);
    Ok(0)
}

async fn list_host(
    secret: &secp256k1::SecretKey,
    host: &str,
    label: &str,
    relays: &[String],
) -> Result<String> {
    let owner_hex = coder_reach::pubkey(secret);
    let host_relays: Vec<String> = relays.iter().take(1).cloned().collect();
    let current = directory::read(relays, secret, RelayPolicy::Production).await?;
    let next = directory::with_host(
        current.as_ref().map(|c| &c.directory),
        &owner_hex,
        host,
        label,
        &host_relays,
        now(),
    )?;
    Ok(match next {
        None => {
            let revision = current.map_or(0, |c| c.directory.revision);
            eprintln!(
                "coder link: the owner directory already lists {label} (revision {revision})"
            );
            format!("listed at revision {revision}")
        }
        Some(next) => {
            directory::publish(
                relays,
                secret,
                current.as_ref(),
                &next,
                RelayPolicy::Production,
            )
            .await?;
            eprintln!(
                "coder link: listed {label} in the owner directory at revision {}",
                next.revision
            );
            format!("listed at revision {}", next.revision)
        }
    })
}

/// Wait until something accepts TCP connections on `listen`.
fn wait_listening(listen: SocketAddr, limit: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < limit {
        if TcpStream::connect_timeout(&listen, Duration::from_secs(1)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(Error::new(format!(
        "the host did not start listening on {listen}; read ~/.openagents/host/logs or the user journal"
    )))
}

/// `coder link invite`.
fn invite(options: &mut Options) -> Result<u8> {
    if let Some(remote) = options.remote()? {
        let mut args = vec!["invite".to_owned()];
        args.extend(options.forward());
        let tty = std::io::stdout().is_terminal() && !args.iter().any(|a| a == "--plain");
        let output = remote.link(&args, None, tty)?;
        print!("{}", output.stdout);
        return Ok(if output.success { 0 } else { EXIT_FAILED });
    }
    let rights = options
        .one("--rights")?
        .ok_or_else(|| usage("invite needs --rights, such as observe,operate,terminal"))?;
    let rights = Rights::parse_list(&rights)?;
    let days = match options.one("--grant-days")? {
        Some(days) => days
            .parse::<u64>()
            .ok()
            .filter(|days| (1..=365).contains(days))
            .ok_or_else(|| usage("--grant-days takes 1 to 365"))?,
        None => 30,
    };
    let plain = options.flag("--plain");
    options.finish()?;
    let code = mint(&rights, days)?;
    if plain {
        println!("{code}");
        return Ok(0);
    }
    let qr = coder_connect::pairing::terminal_qr_prefixed(
        coder_access::protocol::INVITATION_PREFIX,
        &code,
    )
    .map_err(|_| Error::new("the invitation does not fit a QR code"))?;
    println!("{qr}");
    println!();
    println!("{code}");
    eprintln!(
        "One device can redeem this within five minutes, for {} with {}. Scan it or paste the \
         line in Computers > Add a computer. Show it only to that device.",
        if days == 1 {
            "1 day".into()
        } else {
            format!("{days} days")
        },
        rights.to_list()
    );
    Ok(0)
}

fn mint(rights: &Rights, days: u64) -> Result<String> {
    let root = host_root()?;
    let settings = ServeSettings::load(&root)?;
    let relay = settings
        .relays
        .first()
        .cloned()
        .ok_or_else(|| Error::new("this computer is not a host yet; run `coder link setup`"))?;
    let host = Host::new(access_state()?, RelayPolicy::Production);
    let now = now();
    let issued = host.invite(&relay, rights.clone(), now, now + days * 86_400)?;
    Ok(issued.code)
}

/// `coder link join`: the invitation comes on standard input.
async fn join(options: &mut Options) -> Result<u8> {
    let label = options.one("--label")?;
    options.finish()?;
    let mut code = String::new();
    std::io::stdin()
        .take(8 * 1024)
        .read_to_string(&mut code)
        .map_err(|_| Error::new("cannot read the invitation from standard input"))?;
    let code = code.trim();
    if code.is_empty() {
        return Err(usage("join reads the invitation from standard input"));
    }
    let mut client = Client::open(&devices::default_dir()?, RelayPolicy::Production)?;
    let label = label.unwrap_or_else(|| "computer".into());
    let host = client.join(code, &label).await?;
    eprintln!(
        "coder link: this computer ({}) joined host {host} as {label}",
        client.key()
    );
    println!("joined {host}");
    Ok(0)
}

/// `coder link peer --ssh DEST`: each computer becomes a device of the
/// other's host. Invitations travel only on the SSH channel.
async fn peer(options: &mut Options) -> Result<u8> {
    let remote = options
        .remote()?
        .ok_or_else(|| usage("peer needs --ssh DEST"))?;
    let rights = options
        .one("--rights")?
        .unwrap_or_else(|| PEER_RIGHTS.into());
    let rights = Rights::parse_list(&rights)?;
    let local_label = local_label()?;
    let remote_label = options.one("--remote-label")?;
    let label_here = options.one("--label")?.unwrap_or(local_label);
    options.finish()?;

    // This computer joins the remote host.
    let output = remote.link(
        &[
            "invite".into(),
            "--rights".into(),
            rights.to_list(),
            "--plain".into(),
        ],
        None,
        false,
    )?;
    let code = output
        .stdout
        .lines()
        .find(|line| line.starts_with(coder_access::protocol::INVITATION_PREFIX))
        .filter(|_| output.success)
        .ok_or_else(|| Error::new(format!("{} minted no invitation", remote.destination)))?
        .to_owned();
    let mut client = Client::open(&devices::default_dir()?, RelayPolicy::Production)?;
    let remote_label = remote_label.unwrap_or_else(|| remote.destination.clone());
    let remote_host = client.join(&code, &remote_label).await?;
    eprintln!("coder link: this computer joined {remote_label} ({remote_host})");

    // The remote computer joins this host.
    let code = mint(&rights, 30)?;
    let output = remote.link(
        &["join".into(), "--label".into(), label_here.clone()],
        Some(&code),
        false,
    )?;
    if !output.success {
        return Err(Error::new(format!(
            "{} could not join this host",
            remote.destination
        )));
    }
    eprintln!(
        "coder link: {} joined this host as a device labelled {label_here}",
        remote.destination
    );
    println!("peered with {}", remote.destination);
    Ok(0)
}

fn local_label() -> Result<String> {
    let node = tailscale::node(&tailscale::program(None)).ok();
    Ok(node
        .and_then(|node| node.short_name().map(str::to_owned))
        .unwrap_or_else(|| "computer".into()))
}

/// `coder link check`.
async fn check(options: &mut Options) -> Result<u8> {
    if let Some(remote) = options.remote()? {
        let mut args = vec!["check".to_owned()];
        args.extend(options.forward());
        let output = remote.link(&args, None, false)?;
        print!("{}", output.stdout);
        return Ok(if output.success { 0 } else { EXIT_FAILED });
    }
    let which = match (options.flag("--direct"), options.flag("--relay-only")) {
        (true, true) => return Err(usage("give --direct or --relay-only, not both")),
        (true, false) => Which::Direct,
        (false, true) => Which::Relay,
        (false, false) => Which::Both,
    };
    let json_out = options.flag("--json");
    options.finish()?;
    let client = Client::open(&devices::default_dir()?, RelayPolicy::Production)?;
    if client.hosts().is_empty() {
        return Err(Error::new(
            "this computer joined no host; run `coder link peer` or `coder link join`",
        ));
    }
    let results = client.check(which).await;
    let mut failed = false;
    for result in &results {
        failed |= !result.ok;
        if json_out {
            println!(
                "{}",
                json!({"label": result.label, "host": result.host, "kind": result.kind,
                    "route": result.route, "ok": result.ok, "detail": result.detail,
                    "millis": result.millis})
            );
        } else {
            println!(
                "{} {} {} {} {}ms {}",
                if result.ok { "ok  " } else { "FAIL" },
                result.label,
                result.kind,
                result.route,
                result.millis,
                result.detail
            );
        }
    }
    Ok(if failed { EXIT_FAILED } else { 0 })
}

/// `coder link status`.
async fn status(options: &mut Options) -> Result<u8> {
    if let Some(remote) = options.remote()? {
        let mut args = vec!["status".to_owned()];
        args.extend(options.forward());
        let output = remote.link(&args, None, false)?;
        print!("{}", output.stdout);
        return Ok(if output.success { 0 } else { EXIT_FAILED });
    }
    let service_program =
        service::program(options.one("--coder-service")?.as_deref().map(Path::new));
    let key_file = options.owner_key_file()?;
    options.finish()?;
    let root = host_root()?;
    let host = Host::new(access_state()?, RelayPolicy::Production);
    match host.public_key() {
        Ok(key) => {
            println!("host {key}");
            println!("owner {}", host.owner().unwrap_or_default());
            let settings = ServeSettings::load(&root)?;
            println!("relays {}", settings.relays.join(" "));
            for (label, path) in &settings.workspaces {
                println!("workspace {label}={}", path.display());
            }
            if let Some(listen) = settings.listen_websocket {
                println!(
                    "websocket {listen}{}",
                    if settings.websocket_tls.is_some() {
                        " tls"
                    } else {
                        ""
                    }
                );
            }
            for advertise in &settings.advertise {
                println!("advertise {}={}", advertise.class, advertise.address);
            }
            match service::status(&service_program) {
                Ok(Some(status)) => println!(
                    "service running={} loaded={} starts_at={} committed={}",
                    status.running,
                    status.loaded,
                    status.starts_at.unwrap_or_default(),
                    status.committed.unwrap_or_default()
                ),
                Ok(None) => println!("service not installed"),
                Err(error) => println!("service unknown: {error}"),
            }
            for device in host.devices(now())? {
                println!(
                    "device {} {:?} {} last-seen {}",
                    device.device,
                    device.state,
                    device.rights.to_list(),
                    device
                        .last_seen
                        .map_or_else(|| "never".to_owned(), |at| at.to_string())
                );
            }
        }
        Err(_) => println!("host none"),
    }
    let client = Client::open(&devices::default_dir()?, RelayPolicy::Production)?;
    println!("device-key {}", client.key());
    for saved in client.hosts() {
        println!(
            "joined {} {} rights {}{}",
            saved.access.grant.host,
            saved.label,
            saved.access.grant.rights.to_list(),
            if saved.revoked { " revoked" } else { "" }
        );
    }
    if let Ok(secret) = owner::load(&key_file) {
        let settings = ServeSettings::load(&root).unwrap_or_default();
        let relays = if settings.relays.is_empty() {
            vec![plan::DEFAULT_RELAY.to_owned()]
        } else {
            settings.relays
        };
        print_directory(&secret, &relays).await?;
    }
    Ok(0)
}

async fn print_directory(secret: &secp256k1::SecretKey, relays: &[String]) -> Result<()> {
    match directory::read(relays, secret, RelayPolicy::Production).await? {
        Some(current) => {
            println!("directory revision {}", current.directory.revision);
            for entry in &current.directory.hosts {
                println!(
                    "listed {} {} weight {} relays {}",
                    entry.host,
                    entry.label,
                    entry.weight,
                    entry.relays.join(" ")
                );
            }
        }
        None => println!("directory empty"),
    }
    Ok(())
}

/// `coder link owner init|show`.
fn owner_command(options: &mut Options) -> Result<u8> {
    let action = options
        .word()
        .ok_or_else(|| usage("owner takes init or show"))?;
    let path = options.owner_key_file()?;
    options.finish()?;
    let (public, created) = match action.as_str() {
        "init" => owner::create(&path)?,
        "show" => (coder_reach::pubkey(&owner::load(&path)?), false),
        _ => return Err(usage("owner takes init or show")),
    };
    let bytes: [u8; 32] = (0..32)
        .map(|i| u8::from_str_radix(&public[i * 2..i * 2 + 2], 16).unwrap_or(0))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or([0; 32]);
    println!("{public}");
    println!("{}", nostr::nip19::encode_npub(&bytes));
    eprintln!(
        "coder link: {} owner key {}; the secret stays in that file",
        if created { "created the" } else { "the" },
        path.display()
    );
    Ok(0)
}

/// `coder link directory list|add|remove`.
async fn directory_command(options: &mut Options) -> Result<u8> {
    let action = options
        .word()
        .ok_or_else(|| usage("directory takes list, add, or remove"))?;
    let secret = owner::load(&options.owner_key_file()?)?;
    let mut relays = options.all("--relay");
    if relays.is_empty() {
        relays.push(plan::DEFAULT_RELAY.to_owned());
    }
    match action.as_str() {
        "list" => {
            options.finish()?;
            print_directory(&secret, &relays).await?;
        }
        "add" => {
            let host = owner::public_key(
                &options
                    .one("--host")?
                    .ok_or_else(|| usage("directory add needs --host KEY"))?,
            )?;
            let label = options
                .one("--label")?
                .ok_or_else(|| usage("directory add needs --label NAME"))?;
            options.finish()?;
            list_host(&secret, &host, &label, &relays).await?;
        }
        "remove" => {
            let host = owner::public_key(
                &options
                    .one("--host")?
                    .ok_or_else(|| usage("directory remove needs --host KEY"))?,
            )?;
            options.finish()?;
            let current = directory::read(&relays, &secret, RelayPolicy::Production).await?;
            match directory::without_host(current.as_ref().map(|c| &c.directory), &host, now())? {
                Some(next) => {
                    directory::publish(
                        &relays,
                        &secret,
                        current.as_ref(),
                        &next,
                        RelayPolicy::Production,
                    )
                    .await?;
                    println!("removed at revision {}", next.revision);
                }
                None => println!("not listed"),
            }
        }
        _ => return Err(usage("directory takes list, add, or remove")),
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Options {
        Options::parse(&args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn options_forward_what_this_side_did_not_take() {
        let mut options = parse(&[
            "--ssh",
            "box",
            "--workspace",
            "oa=/w/oa",
            "--linger",
            "--label",
            "box",
        ]);
        let remote = options.remote().unwrap().unwrap();
        assert_eq!(remote.destination, "box");
        let forwarded = options.forward();
        assert_eq!(
            forwarded,
            ["--label", "box", "--workspace", "oa=/w/oa", "--linger"]
        );
        assert!(options.finish().is_ok());
    }

    #[test]
    fn unknown_and_repeated_options_refuse() {
        let mut options = parse(&["--port", "1", "--port", "2"]);
        assert!(options.one("--port").is_err());
        let options = parse(&["--nope", "x"]);
        assert!(options.finish().is_err());
        assert!(Options::parse(&["--linger".into(), "--linger".into()]).is_err());
        let mut options = parse(&["--remote-coder", "/x"]);
        assert!(options.remote().is_err());
    }

    #[tokio::test]
    async fn invite_needs_explicit_rights() {
        assert_eq!(run(&["invite".into()]).await, EXIT_USAGE);
        assert_eq!(run(&["frobnicate".into()]).await, EXIT_USAGE);
    }
}
