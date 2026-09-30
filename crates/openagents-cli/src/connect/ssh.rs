//! `openagents connect --ssh DESTINATION`: pair this computer with a
//! headless computer you reach over SSH, in one command.
//!
//! 1. **Probe.** One `ssh` run of a fixed `sh` script reports the remote
//!    operating system, architecture, whether it is NixOS, and the SHA-256
//!    of the installed `openagents` binary (default `~/.local/bin/openagents`).
//! 2. **Install or update.** When the remote binary is missing or differs
//!    from the one this command would install (`--binary PATH`, or this
//!    program when the platforms match), it is streamed over `ssh`, checked
//!    against its SHA-256 on the remote side, made executable, moved into
//!    place, and run once (`openagents version`), so a binary that cannot run
//!    there (a NixOS without `nix-ld`, say) is named now and not later.
//! 3. **Start the host.** The installed binary runs `openagents connect
//!    --ssh-stdio` over `ssh`. That helper adopts a host already serving its
//!    control socket, or starts `openagents host serve --iroh
//!    --control-socket ...` detached in its own session, so the host outlives
//!    the SSH connection. It never stops a host it did not start, and it
//!    refuses to start a second host beside one that serves no control
//!    socket.
//! 4. **Enroll over the SSH channel.** The helper asks its host, over the
//!    same-user control socket (NIP-HOST's local operator), for one
//!    invitation, and sends the `openagents-connect:` code up the SSH
//!    channel's standard output. This computer signs an `enroll.redeem`
//!    request with its device key and sends it down standard input; the
//!    helper carries it to the host's `openagents/enroll/1` ALPN on loopback
//!    and carries the host-signed reply back. This computer checks the grant
//!    against the host key in the code, saves it in its computers store, and
//!    records the host's iroh address (endpoint ID, relay, direct addresses)
//!    beside it.
//!
//! The SSH login authorizes this one enrollment; afterwards the host's grant
//! governs access, not SSH. No secret key crosses the channel: the invitation
//! capability travels inside SSH, the device key stays here, and the owner is
//! passed only as a public key and only when asked (`--import-owner` or
//! `--owner`). Without one, a new host establishes its own owner key.
//!
//! Every remote step goes through [`Transport`], so the unit tests run the
//! whole command against a local shell with a scratch `HOME` and an
//! in-process host, with no `sshd`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use coder_access::RelayPolicy;
use coder_access::client::{finish_redeem, prepare_redeem};
use coder_access::protocol::{Access, HostInvitation};
use coder_computers::live::{FileStore, Saved, SavedHost, Store as _, load_or_create_key};
use openagents_connect::code::ConnectCode;
use openagents_connect::control::{self, Op, Reply, Request};
use openagents_connect::endpoint::{ConnectEndpoint, EndpointConfig};
use openagents_connect::enroll::{self, EnrollReply, EnrollRequest};
use secp256k1::SecretKey;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::{Args, Output};

pub(super) const USAGE: &str = "usage: openagents connect --ssh DESTINATION [OPTIONS]
Pair this computer with a headless computer over SSH: install or update the
openagents binary there, start its host, and enroll this computer over the
SSH channel. DESTINATION is anything ssh accepts (user@box, a Host alias).
  --binary PATH        the openagents build to install (default: this program,
                       when the remote platform matches)
  --no-install         use the binary already there
  --remote-bin PATH    where the binary goes (default ~/.local/bin/openagents)
  --remote-root DIR    the remote host's state (default ~/.openagents)
  --remote-socket PATH the remote control socket (default: the platform's, or
                       DIR/control.sock under another --remote-root)
  --import-owner       make this computer's owner (~/.openagents/coder-owner,
                       or $OPENAGENTS_OWNER_KEY_FILE) the new host's owner;
                       only the public key is sent
  --owner PUBKEY       the same with a public key (hex or npub)
  --label NAME         the computer's name (default its host name)
  --relay URL          the Nostr relay the host serves (default
                       wss://relay.openagents.com/)
  --iroh-relay URL | --no-iroh-relay
  --ssh PROGRAM        the ssh program (default ssh on PATH)
  --store DIR          this computer's computers store
  --timeout SECONDS    bound the whole command (default 300)
ssh runs in batch mode, so set up key login first.";

/// The `--ssh` options that take no value. `--terminal` is accepted and
/// ignored: every pairing now grants the full rights, a terminal included.
const SWITCHES: &[&str] = &[
    "import-owner",
    "terminal",
    "no-install",
    "no-iroh-relay",
    "loopback-test",
];

/// The version of the frames the helper sends up the SSH channel.
const FRAME: &str = "openagents.connect-ssh.v1";
/// The schema of the record of what `--ssh` set up.
const RECORD_SCHEMA: &str = "openagents.connect-ssh-hosts.v1";
const RECORD_FILE: &str = "connect-ssh.json";
/// The schema of the helper's record of the host it started.
const STARTED_SCHEMA: &str = "openagents.connect-ssh-started.v1";
const STARTED_FILE: &str = "connect-ssh-started.json";
const DEFAULT_REMOTE_BIN: &str = "~/.local/bin/openagents";
const DEFAULT_REMOTE_ROOT: &str = "~/.openagents";
/// The relay the host serves unless told otherwise; the phone app's default.
const DEFAULT_RELAY: &str = "wss://relay.openagents.com/";
/// The largest frame either side reads.
const MAX_FRAME: usize = enroll::MAX_MESSAGE_BYTES;
/// How long a started host has to answer on its control socket.
const START_WAIT: Duration = Duration::from_secs(30);
/// How long a replaced host has to exit.
const STOP_WAIT: Duration = Duration::from_secs(10);

/// Route `openagents connect --ssh ...` and the remote helper
/// `openagents connect --ssh-stdio ...`. `None` for any other command.
pub(super) fn dispatch(output: &Output, command: &str, rest: &[String]) -> Option<u8> {
    match command {
        "--ssh" => Some(run(output, rest)),
        "--ssh-stdio" => Some(stdio(rest)),
        _ => None,
    }
}

#[derive(Debug)]
enum Failure {
    Usage(String),
    Refused(String),
}

fn refused(message: impl Into<String>) -> Failure {
    Failure::Refused(message.into())
}

fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .iter()
        .any(|word| matches!(word.as_str(), "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("connect", &message, USAGE),
    };
    let result = Plan::from_args(&args).and_then(|plan| {
        let transport = Ssh {
            program: plan.ssh.clone(),
            destination: plan.destination.clone(),
        };
        connect(&plan, &transport)
    });
    match result {
        Ok(value) => {
            output.emit(&value, render);
            0
        }
        Err(Failure::Usage(message)) => output.usage("connect", &message, USAGE),
        Err(Failure::Refused(message)) => output.fail("connect", &message),
    }
}

/// Everything one `--ssh` run needs, parsed and checked.
#[derive(Clone, Debug)]
struct Plan {
    destination: String,
    binary: Option<PathBuf>,
    install: bool,
    remote_bin: String,
    remote_root: String,
    socket: Option<String>,
    owner: Option<String>,
    label: Option<String>,
    relay: String,
    iroh_relay: IrohRelay,
    ssh: PathBuf,
    store: PathBuf,
    timeout: Duration,
    policy: RelayPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum IrohRelay {
    Default,
    Url(String),
    Off,
}

impl Plan {
    fn from_args(args: &Args) -> Result<Self, Failure> {
        let destination = match args.positional() {
            [destination] => destination.clone(),
            [] => return Err(Failure::Usage("DESTINATION is required".into())),
            [_, extra, ..] => {
                return Err(Failure::Usage(format!("unexpected argument `{extra}`")));
            }
        };
        if destination.is_empty()
            || destination.starts_with('-')
            || destination
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(Failure::Usage(
                "DESTINATION is an ssh destination such as user@box".into(),
            ));
        }
        let owner = match (args.switch("import-owner"), args.option("owner")) {
            (true, Some(_)) => {
                return Err(Failure::Usage(
                    "--import-owner and --owner do not go together".into(),
                ));
            }
            (true, None) => Some(local_owner()?),
            (false, Some(text)) => Some(public_key(text)?),
            (false, None) => None,
        };
        let iroh_relay = match (args.switch("no-iroh-relay"), args.option("iroh-relay")) {
            (true, Some(_)) => {
                return Err(Failure::Usage(
                    "--iroh-relay and --no-iroh-relay do not go together".into(),
                ));
            }
            (true, None) => IrohRelay::Off,
            (false, Some(url)) => IrohRelay::Url(url.to_owned()),
            (false, None) => IrohRelay::Default,
        };
        let install = !args.switch("no-install");
        if !install && args.option("binary").is_some() {
            return Err(Failure::Usage(
                "--binary and --no-install do not go together".into(),
            ));
        }
        let seconds: u64 = args.number("timeout", 300).map_err(Failure::Usage)?;
        if seconds == 0 {
            return Err(Failure::Usage("--timeout is at least 1".into()));
        }
        let policy = if args.switch("loopback-test") {
            RelayPolicy::LoopbackTest
        } else {
            RelayPolicy::Production
        };
        let relay = args.option("relay").unwrap_or(DEFAULT_RELAY).to_owned();
        policy
            .validate(&relay)
            .map_err(|_| Failure::Usage(format!("--relay `{relay}` is not a relay this allows")))?;
        let label = args.option("label").map(str::to_owned);
        if let Some(label) = &label
            && (label.len() > openagents_connect::code::MAX_LABEL_BYTES
                || label.chars().any(char::is_control))
        {
            return Err(Failure::Usage(format!(
                "--label is at most {} bytes with no control characters",
                openagents_connect::code::MAX_LABEL_BYTES
            )));
        }
        for (name, value) in [
            ("remote-bin", args.option("remote-bin")),
            ("remote-root", args.option("remote-root")),
            ("remote-socket", args.option("remote-socket")),
        ] {
            if value.is_some_and(|value| value.is_empty() || value.contains(['\n', '\0'])) {
                return Err(Failure::Usage(format!("--{name} is a path")));
            }
        }
        Ok(Plan {
            destination,
            binary: args.option("binary").map(PathBuf::from),
            install,
            remote_bin: args
                .option("remote-bin")
                .unwrap_or(DEFAULT_REMOTE_BIN)
                .to_owned(),
            remote_root: args
                .option("remote-root")
                .unwrap_or(DEFAULT_REMOTE_ROOT)
                .to_owned(),
            socket: args.option("remote-socket").map(str::to_owned),
            owner,
            label,
            relay,
            iroh_relay,
            ssh: PathBuf::from(args.option("ssh").unwrap_or("ssh")),
            store: crate::computer::store_dir(args.option("store")),
            timeout: Duration::from_secs(seconds),
            policy,
        })
    }

    /// The helper's arguments after `connect --ssh-stdio`.
    fn helper_args(&self, restart: bool) -> Vec<String> {
        let mut args = vec![
            "--remote-root".to_owned(),
            self.remote_root.clone(),
            "--relay".to_owned(),
            self.relay.clone(),
        ];
        if let Some(socket) = &self.socket {
            args.extend(["--control-socket".to_owned(), socket.clone()]);
        }
        if let Some(owner) = &self.owner {
            args.extend(["--owner".to_owned(), owner.clone()]);
        }
        if let Some(label) = &self.label {
            args.extend(["--label".to_owned(), label.clone()]);
        }
        match &self.iroh_relay {
            IrohRelay::Default => {}
            IrohRelay::Url(url) => args.extend(["--iroh-relay".to_owned(), url.clone()]),
            IrohRelay::Off => args.push("--no-iroh-relay".to_owned()),
        }
        if restart {
            args.push("--restart".to_owned());
        }
        if self.policy == RelayPolicy::LoopbackTest {
            args.push("--loopback-test".to_owned());
        }
        args
    }
}

/// This computer's owner public key, from the owner key file. The secret is
/// read here to derive the public key and goes nowhere else.
fn local_owner() -> Result<String, Failure> {
    let path = match std::env::var_os("OPENAGENTS_OWNER_KEY_FILE").filter(|p| !p.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => home()
            .map_err(refused)?
            .join(".openagents/coder-owner/owner.key"),
    };
    let text = std::fs::read_to_string(&path).map_err(|_| {
        refused(format!(
            "--import-owner: no owner key at {}; pass --owner PUBKEY, or leave both off and the new host makes its own owner key",
            path.display()
        ))
    })?;
    let text = text.trim();
    let secret = if text.starts_with("nsec1") {
        nostr::nip19::decode_nsec(text)
            .ok()
            .and_then(|bytes| SecretKey::from_byte_array(bytes).ok())
    } else if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        text.to_ascii_lowercase().parse::<SecretKey>().ok()
    } else {
        None
    };
    let secret = secret.ok_or_else(|| refused("--import-owner: the owner key is malformed"))?;
    Ok(coder_reach::pubkey(&secret))
}

/// A public key as lowercase hex, from hex or `npub`.
fn public_key(text: &str) -> Result<String, Failure> {
    let hex = if text.starts_with("npub1") {
        nostr::nip19::decode_npub(text)
            .map_err(|_| Failure::Usage("--owner is not a valid npub".into()))?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    } else {
        text.to_ascii_lowercase()
    };
    coder_reach::parse_pubkey(&hex)
        .map_err(|_| Failure::Usage("--owner is not a public key".into()))?;
    Ok(hex)
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_owned())
}

/// Expand a leading `~/` against `HOME`.
fn expand(path: &str) -> Result<PathBuf, String> {
    match path.strip_prefix("~/") {
        Some(rest) => Ok(home()?.join(rest)),
        None if path == "~" => home(),
        None => Ok(PathBuf::from(path)),
    }
}

fn now() -> u64 {
    openagents_connect::now()
}

// ---------------------------------------------------------------------------
// Transport

/// What one remote command produced.
#[derive(Debug)]
struct Ran {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// The helper's standard input and output, and a way to wait for it.
struct Session {
    input: Box<dyn Write + Send>,
    output: Box<dyn Read + Send>,
    finish: Box<dyn FnOnce() -> String + Send>,
}

/// How the remote machine is reached. [`Ssh`] runs the system `ssh`; the
/// tests run a local shell with a scratch `HOME`.
trait Transport {
    /// Run the fixed `script` with `sh -c` and `args`, feeding `stdin`.
    fn run(
        &self,
        script: &str,
        args: &[String],
        stdin: Option<&Path>,
        wall: Duration,
    ) -> Result<Ran, String>;

    /// Start `bin connect --ssh-stdio ARGS` with piped standard streams.
    fn helper(&self, bin: &str, args: &[String], wall: Duration) -> Result<Session, String>;
}

/// Quote one word for a POSIX shell.
fn quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// A remote command line: `sh -c SCRIPT oa-connect ARGS...`.
fn sh_line(script: &str, args: &[String]) -> String {
    let mut line = format!("sh -c {} oa-connect", quote(script));
    for arg in args {
        line.push(' ');
        line.push_str(&quote(arg));
    }
    line
}

/// The system `ssh`, in batch mode, with no shared master connection.
struct Ssh {
    program: PathBuf,
    destination: String,
}

impl Ssh {
    fn command(&self, remote: &str) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args([
                "-T",
                "-o",
                "BatchMode=yes",
                "-o",
                "ControlMaster=no",
                "-o",
                "ControlPath=none",
                "-o",
                "ServerAliveInterval=15",
            ])
            .arg(&self.destination)
            .arg(remote);
        command
    }
}

/// Kill `child` if it is still running at `deadline`.
fn watchdog(pid: u32, wall: Duration) -> Arc<AtomicBool> {
    let done = Arc::new(AtomicBool::new(false));
    let seen = done.clone();
    std::thread::spawn(move || {
        let deadline = Instant::now() + wall;
        while Instant::now() < deadline {
            if seen.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if !seen.load(Ordering::SeqCst)
            && let Ok(pid) = libc::pid_t::try_from(pid)
        {
            // SAFETY: kill has no memory preconditions; the pid is our child,
            // which has not been waited for yet.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
    });
    done
}

fn run_child(mut command: Command, stdin: Option<&Path>, wall: Duration) -> Result<Ran, String> {
    command
        .stdin(match stdin {
            Some(path) => Stdio::from(
                std::fs::File::open(path)
                    .map_err(|error| format!("{}: {error}", path.display()))?,
            ),
            None => Stdio::null(),
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command
        .spawn()
        .map_err(|error| format!("cannot start ssh: {error}"))?;
    let done = watchdog(child.id(), wall);
    let output = child
        .wait_with_output()
        .map_err(|error| format!("ssh failed: {error}"))?;
    let timed_out = !done.swap(true, Ordering::SeqCst) && output.status.code().is_none();
    if timed_out {
        return Err("the remote step did not finish before --timeout".into());
    }
    Ok(Ran {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn spawn_session(mut command: Command, wall: Duration) -> Result<Session, String> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start ssh: {error}"))?;
    let done = watchdog(child.id(), wall);
    let input = child.stdin.take().ok_or("no ssh stdin")?;
    let output = child.stdout.take().ok_or("no ssh stdout")?;
    let mut stderr = child.stderr.take().ok_or("no ssh stderr")?;
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    Ok(Session {
        input: Box::new(input),
        output: Box::new(output),
        finish: Box::new(move || {
            let _ = child.wait();
            done.store(true, Ordering::SeqCst);
            errors.join().unwrap_or_default()
        }),
    })
}

impl Transport for Ssh {
    fn run(
        &self,
        script: &str,
        args: &[String],
        stdin: Option<&Path>,
        wall: Duration,
    ) -> Result<Ran, String> {
        let ran = run_child(self.command(&sh_line(script, args)), stdin, wall)?;
        // ssh exits 255 for its own failures: no route, no key login.
        if ran.code == Some(255) {
            return Err(format!(
                "ssh {} failed: {}",
                self.destination,
                last_line(&ran.stderr)
            ));
        }
        Ok(ran)
    }

    fn helper(&self, bin: &str, args: &[String], wall: Duration) -> Result<Session, String> {
        let mut line = format!("{} connect --ssh-stdio", quote(bin));
        for arg in args {
            line.push(' ');
            line.push_str(&quote(arg));
        }
        spawn_session(self.command(&line), wall)
    }
}

fn last_line(text: &str) -> String {
    text.lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no detail")
        .trim()
        .to_owned()
}

// ---------------------------------------------------------------------------
// Frames on the SSH channel: a 4-byte big-endian length, then JSON, as
// `openagents_connect::wire` frames the enroll ALPN.

fn write_frame<W: Write + ?Sized, T: Serialize>(writer: &mut W, message: &T) -> Result<(), String> {
    let body = serde_json::to_vec(message).map_err(|_| "a frame does not encode")?;
    if body.len() > MAX_FRAME {
        return Err("a frame exceeds its bound".into());
    }
    let len = u32::try_from(body.len()).map_err(|_| "a frame exceeds its bound")?;
    writer
        .write_all(&len.to_be_bytes())
        .and_then(|()| writer.write_all(&body))
        .and_then(|()| writer.flush())
        .map_err(|_| "the SSH channel closed".into())
}

/// `Ok(None)` when the peer closed before a frame.
fn read_frame<R: Read + ?Sized, T: DeserializeOwned>(reader: &mut R) -> Result<Option<T>, String> {
    let mut len = [0u8; 4];
    let mut filled = 0;
    while filled < len.len() {
        match reader.read(&mut len[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err("the SSH channel ended inside a frame".into()),
            Ok(read) => filled += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err("the SSH channel failed".into()),
        }
    }
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err("a frame exceeds its bound".into());
    }
    let mut body = vec![0u8; len];
    reader
        .read_exact(&mut body)
        .map_err(|_| "the SSH channel ended inside a frame")?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|_| "a frame is not the expected JSON".into())
}

/// What the helper sends up the SSH channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Up {
    /// The host runs and made one invitation for this enrollment.
    Ready {
        v: String,
        /// The `openagents-connect:` code. It carries the capability.
        code: String,
        /// The Nostr relay the host serves, which the grant names.
        relay: String,
        /// The remote clock.
        now: u64,
        /// `started`, `restarted`, `reused` (a host an earlier `--ssh`
        /// started), or `adopted` (any other host with a control socket).
        start: String,
        pid: Option<u32>,
    },
    /// The host's answer to the enrollment request.
    Answer { v: String, reply: EnrollReply },
    /// The helper could not go on.
    Refused { v: String, message: String },
}

fn refusal(message: impl Into<String>) -> Up {
    Up::Refused {
        v: FRAME.into(),
        message: message.into(),
    }
}

// ---------------------------------------------------------------------------
// This computer's side.

/// What the probe found on the remote machine.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Probe {
    os: String,
    arch: String,
    nixos: bool,
    /// The installed binary's SHA-256, when one is there.
    sha: Option<String>,
    /// The binary's absolute path there.
    bin: String,
}

/// Reports the platform and the installed binary. Every result line starts
/// with `oa-connect ` and holds one `key=value`.
const PROBE: &str = r#"set -u
bin=$1
case "$bin" in "~/"*) bin="$HOME/${bin#\~/}" ;; esac
case "$(uname -s 2>/dev/null)" in Linux) os=linux ;; Darwin) os=macos ;; *) os=unknown ;; esac
case "$(uname -m 2>/dev/null)" in x86_64|amd64) arch=x86_64 ;; aarch64|arm64) arch=aarch64 ;; *) arch=unknown ;; esac
nixos=no
if [ -e /etc/NIXOS ]; then nixos=yes; fi
sha=none
if [ -f "$bin" ]; then
  if command -v sha256sum >/dev/null 2>&1; then sha=$(sha256sum "$bin" | cut -d ' ' -f 1)
  elif command -v shasum >/dev/null 2>&1; then sha=$(shasum -a 256 "$bin" | cut -d ' ' -f 1)
  else sha=unknown; fi
fi
printf 'oa-connect os=%s\noa-connect arch=%s\noa-connect nixos=%s\noa-connect sha=%s\noa-connect bin=%s\n' "$os" "$arch" "$nixos" "$sha" "$bin"
"#;

/// Reads the binary on standard input, checks its SHA-256, installs it, and
/// runs it once.
const INSTALL: &str = r#"set -u
bin=$1
want=$2
case "$bin" in "~/"*) bin="$HOME/${bin#\~/}" ;; esac
dir=$(dirname "$bin")
mkdir -p "$dir" || { echo "oa-connect error=cannot create $dir"; exit 3; }
tmp="$bin.oa-upload.$$"
trap 'rm -f "$tmp"' EXIT
umask 022
cat > "$tmp" || { echo "oa-connect error=cannot write $tmp"; exit 3; }
if command -v sha256sum >/dev/null 2>&1; then got=$(sha256sum "$tmp" | cut -d ' ' -f 1)
elif command -v shasum >/dev/null 2>&1; then got=$(shasum -a 256 "$tmp" | cut -d ' ' -f 1)
else echo "oa-connect error=no sha256sum or shasum"; exit 3; fi
if [ "$got" != "$want" ]; then echo "oa-connect error=the upload does not match its SHA-256"; exit 3; fi
chmod 755 "$tmp" && mv -f "$tmp" "$bin" || { echo "oa-connect error=cannot install $bin"; exit 3; }
if ! out=$("$bin" version 2>&1); then
  echo "oa-connect error=the installed binary does not run: $(printf '%s' "$out" | tail -n 1)"
  exit 4
fi
echo "oa-connect installed=$bin"
"#;

fn fields(stdout: &str) -> Vec<(&str, &str)> {
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("oa-connect "))
        .filter_map(|field| field.split_once('='))
        .collect()
}

fn field<'a>(fields: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
}

fn parse_probe(ran: &Ran) -> Result<Probe, String> {
    let fields = fields(&ran.stdout);
    let get = |name: &str| {
        field(&fields, name).ok_or_else(|| {
            format!(
                "the remote probe did not report {name}: {}",
                last_line(&ran.stderr)
            )
        })
    };
    let sha = match get("sha")? {
        "none" => None,
        "unknown" => {
            return Err("the remote machine has neither sha256sum nor shasum".into());
        }
        sha if sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Some(sha.to_ascii_lowercase())
        }
        _ => return Err("the remote probe reported a malformed SHA-256".into()),
    };
    Ok(Probe {
        os: get("os")?.to_owned(),
        arch: get("arch")?.to_owned(),
        nixos: get("nixos")? == "yes",
        sha,
        bin: get("bin")?.to_owned(),
    })
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let failed = |error: std::io::Error| format!("{}: {error}", path.display());
    let mut file = std::fs::File::open(path).map_err(failed)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(failed)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn local_platform() -> (&'static str, &'static str) {
    (std::env::consts::OS, std::env::consts::ARCH)
}

/// The binary to install: `--binary`, or this program when it runs on the
/// remote platform.
fn binary_for(plan: &Plan, probe: &Probe) -> Result<PathBuf, Failure> {
    if let Some(binary) = &plan.binary {
        return Ok(binary.clone());
    }
    let (os, arch) = local_platform();
    if probe.os == os && probe.arch == arch {
        return std::env::current_exe()
            .map_err(|error| refused(format!("cannot find this program: {error}")));
    }
    Err(refused(format!(
        "{} is {}/{} and this computer is {os}/{arch}; pass --binary PATH with an openagents build for {}/{}",
        plan.destination, probe.os, probe.arch, probe.os, probe.arch
    )))
}

struct Deadline(Instant);

impl Deadline {
    fn left(&self) -> Result<Duration, Failure> {
        let left = self.0.saturating_duration_since(Instant::now());
        if left.is_zero() {
            Err(refused("the command did not finish before --timeout"))
        } else {
            Ok(left)
        }
    }
}

/// The whole `--ssh` run.
fn connect(plan: &Plan, transport: &dyn Transport) -> Result<Value, Failure> {
    let deadline = Deadline(Instant::now() + plan.timeout);
    let probe = transport
        .run(
            PROBE,
            std::slice::from_ref(&plan.remote_bin),
            None,
            deadline.left()?,
        )
        .map_err(refused)
        .and_then(|ran| parse_probe(&ran).map_err(refused))?;
    if probe.os == "unknown" || probe.arch == "unknown" {
        return Err(refused(format!(
            "{} runs a platform openagents does not build for",
            plan.destination
        )));
    }

    let install = if !plan.install {
        if probe.sha.is_none() {
            return Err(refused(format!(
                "--no-install, but there is no binary at {} on {}",
                probe.bin, plan.destination
            )));
        }
        "kept"
    } else {
        let binary = binary_for(plan, &probe)?;
        let sha = sha256_file(&binary).map_err(refused)?;
        if probe.sha.as_deref() == Some(sha.as_str()) {
            "current"
        } else {
            let ran = transport
                .run(
                    INSTALL,
                    &[probe.bin.clone(), sha.clone()],
                    Some(&binary),
                    deadline.left()?,
                )
                .map_err(refused)?;
            let fields = fields(&ran.stdout);
            if let Some(error) = field(&fields, "error") {
                let hint = if probe.nixos && ran.code == Some(4) {
                    "; on NixOS enable programs.nix-ld, or pass --binary with a build made on NixOS"
                } else {
                    ""
                };
                return Err(refused(format!(
                    "installing on {} failed: {error}{hint}",
                    plan.destination
                )));
            }
            if ran.code != Some(0) || field(&fields, "installed").is_none() {
                return Err(refused(format!(
                    "installing on {} failed: {}",
                    plan.destination,
                    last_line(&ran.stderr)
                )));
            }
            if probe.sha.is_some() {
                "updated"
            } else {
                "installed"
            }
        }
    };

    let device = load_or_create_key(&plan.store).map_err(refused)?;
    let session = transport
        .helper(
            &probe.bin,
            &plan.helper_args(install == "updated"),
            deadline.left()?,
        )
        .map_err(refused)?;
    let enrolled = enroll_over(session, plan, &device);
    let enrolled = enrolled?;

    let label = save_access(plan, &enrolled)?;
    let hint = json!({
        "endpoint": enrolled.code.endpoint().to_string(),
        "relay": enrolled.code.relay().map(ToString::to_string),
        "addrs": enrolled.code.addrs().iter().map(ToString::to_string).collect::<Vec<_>>(),
    });
    let record = save_record(plan, &probe, &enrolled, &hint)?;
    let grant = &enrolled.access.grant;
    Ok(json!({
        "destination": plan.destination,
        "platform": format!("{}/{}", probe.os, probe.arch),
        "nixos": probe.nixos,
        "binary": { "path": probe.bin, "install": install },
        "host": {
            "key": grant.host,
            "label": label,
            "start": enrolled.start,
            "pid": enrolled.pid,
            "clock_offset": openagents_connect::clock_warning(enrolled.host_now, enrolled.device_now),
        },
        "grant": {
            "id": grant.grant,
            "owner": grant.owner,
            "device": grant.device,
            "relay": grant.relay,
            "rights": serde_json::to_value(&grant.rights).unwrap_or(Value::Null),
            "expires_at": grant.expires_at,
        },
        "iroh": hint,
        "store": plan.store,
        "record": record,
    }))
}

/// A finished enrollment.
struct Enrolled {
    code: ConnectCode,
    access: Access,
    start: String,
    pid: Option<u32>,
    host_now: u64,
    device_now: u64,
}

fn enroll_over(mut session: Session, plan: &Plan, device: &SecretKey) -> Result<Enrolled, Failure> {
    let result = exchange(&mut session, plan, device);
    // Closing standard input tells the helper to cancel an unused
    // invitation and exit.
    let Session {
        input,
        output,
        finish,
    } = session;
    drop(input);
    drop(output);
    let stderr = finish();
    result.map_err(|message| {
        let detail = last_line(&stderr);
        if detail == "no detail" {
            refused(message)
        } else {
            refused(format!("{message} ({detail})"))
        }
    })
}

fn exchange(session: &mut Session, plan: &Plan, device: &SecretKey) -> Result<Enrolled, String> {
    let ready = read_frame::<_, Up>(&mut session.output)?
        .ok_or("the remote helper closed without an answer")?;
    let (code, relay, host_now, start, pid) = match ready {
        Up::Ready {
            v,
            code,
            relay,
            now,
            start,
            pid,
        } if v == FRAME => (code, relay, now, start, pid),
        Up::Refused { message, .. } => return Err(format!("{}: {message}", plan.destination)),
        _ => return Err("the remote helper speaks another version; update it".into()),
    };
    let device_now = now();
    let code = ConnectCode::parse(&code, device_now).map_err(|error| {
        match openagents_connect::clock_warning(host_now, device_now) {
            Some(offset) => {
                format!("the host's code does not check ({error}); the clocks differ by {offset} s")
            }
            None => format!("the host's code does not check: {error}"),
        }
    })?;
    let invitation = HostInvitation::from_parts(
        &code.host(),
        &code.invitation(),
        &code.capability(),
        &relay,
        code.issued_at(),
        code.expires_at(),
        device_now,
        plan.policy,
    )
    .map_err(|error| format!("the host's invitation does not check: {error}"))?;
    let pending = prepare_redeem(&invitation, device, device_now, plan.policy)
        .map_err(|error| format!("cannot sign the redemption: {error}"))?;
    let request = EnrollRequest::new(
        serde_json::to_string(&pending.event).map_err(|_| "the redemption does not encode")?,
    );
    write_frame(&mut session.input, &request)?;
    let answer = read_frame::<_, Up>(&mut session.output)?
        .ok_or("the remote helper closed before the host answered")?;
    let reply = match answer {
        Up::Answer { v, reply } if v == FRAME => reply,
        Up::Refused { message, .. } => return Err(format!("{}: {message}", plan.destination)),
        _ => return Err("the remote helper answered out of order".into()),
    };
    let event = reply
        .reply
        .as_deref()
        .ok_or("the host refused the invitation without a signed reply")?;
    let event = serde_json::from_str(event).map_err(|_| "the host's reply is not an event")?;
    let access = finish_redeem(&invitation, &pending, &event, device, now(), plan.policy)
        .map_err(|error| format!("the host did not grant access: {error}"))?;
    if access.grant.host != code.host() {
        return Err("the grant names another host than the code".into());
    }
    Ok(Enrolled {
        code,
        access,
        start,
        pid,
        host_now: reply.now,
        device_now,
    })
}

/// Save the grant in this computer's computers store, as a redeemed
/// invitation is saved. Returns the computer's label.
fn save_access(plan: &Plan, enrolled: &Enrolled) -> Result<String, Failure> {
    let mut store = FileStore::open(&plan.store).map_err(refused)?;
    let mut saved = store
        .load()
        .map_err(refused)?
        .unwrap_or_else(Saved::default);
    let host = enrolled.access.grant.host.clone();
    let previous = saved
        .hosts
        .iter()
        .find(|saved| saved.access.grant.host == host)
        .cloned();
    let label = previous.as_ref().map_or_else(
        || {
            plan.label
                .clone()
                .or_else(|| Some(enrolled.code.label().to_owned()).filter(|l| !l.is_empty()))
                .unwrap_or_else(|| format!("Computer {}", &host[..8]))
        },
        |previous| previous.label.clone(),
    );
    saved.hosts.retain(|saved| saved.access.grant.host != host);
    saved.hosts.push(SavedHost {
        access: enrolled.access.clone(),
        label: label.clone(),
        enabled: true,
        revoked: false,
        ssh: Some(plan.destination.clone()),
        delisted: previous.is_some_and(|previous| previous.delisted),
        // The computer's iroh route: later channels dial it first.
        iroh: Some(coder_host::client::iroh::IrohRoute::of(&enrolled.code)),
    });
    store.save(&saved).map_err(refused)?;
    Ok(label)
}

/// Record the destination, the host, and its iroh address beside the store.
/// No secret: the grant is in the store, the invitation is spent.
fn save_record(
    plan: &Plan,
    probe: &Probe,
    enrolled: &Enrolled,
    hint: &Value,
) -> Result<PathBuf, Failure> {
    let path = plan.store.join(RECORD_FILE);
    let mut records = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes)
            .ok()
            .filter(|value| value["schema"] == RECORD_SCHEMA)
            .unwrap_or_else(|| json!({ "schema": RECORD_SCHEMA, "hosts": {} })),
        Err(_) => json!({ "schema": RECORD_SCHEMA, "hosts": {} }),
    };
    records["hosts"][&plan.destination] = json!({
        "host": enrolled.access.grant.host,
        "grant": enrolled.access.grant.grant,
        "relay": enrolled.access.grant.relay,
        "iroh": hint,
        "bin": probe.bin,
        "root": plan.remote_root,
        "platform": format!("{}/{}", probe.os, probe.arch),
        "at": now(),
    });
    let bytes =
        serde_json::to_vec_pretty(&records).map_err(|_| refused("cannot encode the record"))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)
        .and_then(|()| std::fs::rename(&temporary, &path))
        .map_err(|error| refused(format!("{}: {error}", path.display())))?;
    Ok(path)
}

fn render(value: &Value) -> String {
    let text = |value: &Value| match value {
        Value::Null => "-".to_owned(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    let rights = value["grant"]["rights"]
        .as_array()
        .map(|rights| rights.iter().map(text).collect::<Vec<_>>().join(","))
        .unwrap_or_else(|| text(&value["grant"]["rights"]));
    let mut lines = vec![
        format!(
            "{} ({}): openagents {} at {}; host {} (pid {})",
            text(&value["destination"]),
            text(&value["platform"]),
            text(&value["binary"]["install"]),
            text(&value["binary"]["path"]),
            text(&value["host"]["start"]),
            text(&value["host"]["pid"]),
        ),
        format!(
            "connected to \"{}\" ({}) with {rights}; grant expires at {}",
            text(&value["host"]["label"]),
            text(&value["host"]["key"]),
            text(&value["grant"]["expires_at"]),
        ),
        format!(
            "iroh {} via {} at {}",
            text(&value["iroh"]["endpoint"]),
            text(&value["iroh"]["relay"]),
            value["iroh"]["addrs"]
                .as_array()
                .map(|addrs| addrs.iter().map(text).collect::<Vec<_>>().join(", "))
                .filter(|addrs| !addrs.is_empty())
                .unwrap_or_else(|| "no direct address".to_owned()),
        ),
    ];
    if let Some(offset) = value["host"]["clock_offset"].as_u64() {
        lines.push(format!(
            "note: this computer's clock and {}'s differ by {offset} s",
            text(&value["destination"])
        ));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// The remote helper: `openagents connect --ssh-stdio`, run over ssh with its
// standard streams as the channel. Nothing but frames goes to stdout.

/// The helper's options.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Remote {
    root: PathBuf,
    socket: PathBuf,
    relay: String,
    owner: Option<String>,
    label: Option<String>,
    iroh_relay: IrohRelay,
    restart: bool,
    loopback_test: bool,
}

impl Remote {
    fn from_args(words: &[String]) -> Result<Self, String> {
        // `--terminal` from an earlier `openagents` is accepted and ignored.
        let args = Args::parse(
            words,
            &["terminal", "restart", "loopback-test", "no-iroh-relay"],
        )?;
        if let Some(extra) = args.positional().first() {
            return Err(format!("unexpected argument `{extra}`"));
        }
        let root = expand(args.option("remote-root").unwrap_or(DEFAULT_REMOTE_ROOT))?;
        let socket = match args.option("control-socket") {
            Some(socket) => expand(socket)?,
            None => default_socket(&root)?,
        };
        let owner = args
            .option("owner")
            .map(public_key)
            .transpose()
            .map_err(|failure| match failure {
                Failure::Usage(message) | Failure::Refused(message) => message,
            })?;
        Ok(Remote {
            root,
            socket,
            relay: args.option("relay").unwrap_or(DEFAULT_RELAY).to_owned(),
            owner,
            label: args.option("label").map(str::to_owned),
            iroh_relay: match (args.switch("no-iroh-relay"), args.option("iroh-relay")) {
                (true, _) => IrohRelay::Off,
                (false, Some(url)) => IrohRelay::Url(url.to_owned()),
                (false, None) => IrohRelay::Default,
            },
            restart: args.switch("restart"),
            loopback_test: args.switch("loopback-test"),
        })
    }

    fn state(&self) -> PathBuf {
        self.root.join("coder-access")
    }

    fn policy(&self) -> RelayPolicy {
        if self.loopback_test {
            RelayPolicy::LoopbackTest
        } else {
            RelayPolicy::Production
        }
    }

    fn host_root(&self) -> PathBuf {
        self.root.join("host")
    }

    /// Where a CLI-only host keeps its iroh key, and where a host this
    /// helper started without an owner keeps its host and owner keys.
    fn keys(&self) -> PathBuf {
        self.root.join("connect")
    }

    fn started_path(&self) -> PathBuf {
        self.host_root().join(STARTED_FILE)
    }

    fn log_path(&self) -> PathBuf {
        self.host_root().join("connect-serve.log")
    }

    /// The `host serve` arguments.
    fn serve_args(&self) -> Vec<String> {
        let path = |path: PathBuf| path.display().to_string();
        let mut args = vec![
            "host".to_owned(),
            "serve".to_owned(),
            "--state".to_owned(),
            path(self.state()),
            "--root".to_owned(),
            path(self.host_root()),
            "--relay".to_owned(),
            self.relay.clone(),
            "--iroh".to_owned(),
            "--control-socket".to_owned(),
            path(self.socket.clone()),
        ];
        match &self.iroh_relay {
            IrohRelay::Default => {}
            IrohRelay::Url(url) => args.extend(["--iroh-relay".to_owned(), url.clone()]),
            IrohRelay::Off => args.push("--no-iroh-relay".to_owned()),
        }
        // A host with an owner given keeps its host key in the access store,
        // as any CLI install does. Without one it keeps its host and owner
        // keys in a file key source and establishes its own owner, as the
        // desktop app does; once there, that key source stays in use.
        let keys = openagents_connect::keys::FileKeySource::new(self.keys());
        let book = coder_host::access::host::Host::new(self.state(), self.policy()).state_path();
        let own_owner = keys.path(openagents_connect::keys::KeyName::Host).exists()
            || (self.owner.is_none() && !book.exists());
        if own_owner {
            args.extend(["--keys".to_owned(), path(self.keys())]);
        } else if let Some(owner) = &self.owner {
            args.extend(["--owner".to_owned(), owner.clone()]);
        }
        if let Some(label) = &self.label {
            args.extend(["--label".to_owned(), label.clone()]);
        }
        if self.loopback_test {
            args.push("--loopback-test".to_owned());
        }
        args
    }
}

/// The platform's control socket for the default root, as the desktop app
/// and `openagents connect` use; under any other root, `ROOT/control.sock`,
/// so a second host there never collides with the first.
fn default_socket(root: &Path) -> Result<PathBuf, String> {
    let default_root = home()?.join(".openagents");
    if root == default_root
        && let Some(socket) = control::socket_path()
    {
        return Ok(socket);
    }
    Ok(root.join("control.sock"))
}

/// The helper's recorded start: which host process it launched, from which
/// binary, with which arguments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Started {
    schema: String,
    pid: u32,
    program: String,
    args: Vec<String>,
    at: u64,
}

fn stdio(words: &[String]) -> u8 {
    let mut stdout = std::io::stdout().lock();
    let stdin = std::io::stdin().lock();
    let remote = match Remote::from_args(words) {
        Ok(remote) => remote,
        Err(message) => {
            let _ = write_frame(&mut stdout, &refusal(message));
            return 64;
        }
    };
    let program = match std::env::current_exe() {
        Ok(program) => program,
        Err(error) => {
            let _ = write_frame(&mut stdout, &refusal(format!("no program path: {error}")));
            return 1;
        }
    };
    match serve_stdio(&remote, &Launch::Program(program), stdin, &mut stdout) {
        Ok(()) => 0,
        Err(message) => {
            let _ = write_frame(&mut stdout, &refusal(message));
            1
        }
    }
}

/// How the helper starts a host when none answers.
enum Launch {
    /// Run this program's `host serve`, detached.
    Program(PathBuf),
    /// Start nothing: a test whose host is already running.
    #[cfg(test)]
    Never,
}

async fn call(socket: &Path, op: Op) -> Result<Reply, String> {
    let mut stream = tokio::net::UnixStream::connect(socket)
        .await
        .map_err(|error| format!("{}: {error}", socket.display()))?;
    match tokio::time::timeout(
        Duration::from_secs(10),
        control::call(&mut stream, &Request::new(1, op)),
    )
    .await
    {
        Ok(Ok(Reply::Refused { code, message })) => {
            Err(format!("the host refused: {code}: {message}"))
        }
        Ok(Ok(reply)) => Ok(reply),
        Ok(Err(error)) => Err(format!("the control socket failed: {error}")),
        Err(_) => Err("the control socket did not answer".into()),
    }
}

/// The helper's whole run: make sure a host serves, mint one invitation,
/// and carry one redemption.
fn serve_stdio<R: Read, W: Write>(
    remote: &Remote,
    launch: &Launch,
    mut input: R,
    output: &mut W,
) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("no runtime: {error}"))?;
    let (start, pid) = ensure_host(&runtime, remote, launch)?;
    let Reply::Invite {
        invitation, code, ..
    } = runtime.block_on(call(&remote.socket, Op::InviteCreate {}))?
    else {
        return Err("the host answered an invitation with something else".into());
    };
    let cancel = || {
        let _ = runtime.block_on(call(
            &remote.socket,
            Op::InviteCancel {
                invitation: invitation.clone(),
            },
        ));
    };
    let ready = Up::Ready {
        v: FRAME.into(),
        code: code.clone(),
        relay: remote.relay.clone(),
        now: now(),
        start,
        pid,
    };
    if let Err(error) = write_frame(output, &ready) {
        cancel();
        return Err(error);
    }
    let request: EnrollRequest = match read_frame(&mut input) {
        Ok(Some(request)) => request,
        Ok(None) => {
            cancel();
            return Ok(());
        }
        Err(error) => {
            cancel();
            return Err(error);
        }
    };
    let answer = runtime.block_on(redeem_local(&code, &request));
    match answer {
        Ok(reply) => write_frame(
            output,
            &Up::Answer {
                v: FRAME.into(),
                reply,
            },
        ),
        Err(error) => {
            cancel();
            Err(error)
        }
    }
}

/// Carry the redemption to the host's enroll ALPN on this machine.
async fn redeem_local(code: &str, request: &EnrollRequest) -> Result<EnrollReply, String> {
    let code =
        ConnectCode::parse_shape(code).map_err(|error| format!("the host's code: {error}"))?;
    let mut addr = code.endpoint_addr();
    // The host listens on every interface; reach it over loopback, which
    // needs no route and no relay.
    for socket in code.addrs() {
        let local = match socket {
            std::net::SocketAddr::V4(v4) => std::net::SocketAddr::from(([127, 0, 0, 1], v4.port())),
            std::net::SocketAddr::V6(v6) => {
                std::net::SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, v6.port()))
            }
        };
        addr = addr.with_ip_addr(local);
    }
    let endpoint = ConnectEndpoint::bind(
        openagents_connect::iroh::SecretKey::generate(),
        EndpointConfig::loopback(Vec::new()),
    )
    .await
    .map_err(|error| format!("no local iroh endpoint: {error}"))?;
    let reply = enroll::redeem(&endpoint, addr, request)
        .await
        .map_err(|error| format!("the host's enrollment did not answer: {error}"));
    endpoint.close().await;
    reply
}

fn pid_alive(pid: u32) -> bool {
    libc::pid_t::try_from(pid).is_ok_and(|pid| {
        // SAFETY: signal 0 only checks that the process exists.
        pid > 0 && unsafe { libc::kill(pid, 0) } == 0
    })
}

/// The pid in a host's runtime file (`ROOT/host/runtime`), when it runs.
fn runtime_pid(remote: &Remote) -> Option<u32> {
    let text = std::fs::read_to_string(remote.host_root().join("runtime")).ok()?;
    let pid = text
        .lines()
        .find_map(|line| line.strip_prefix("pid="))?
        .trim()
        .parse()
        .ok()?;
    pid_alive(pid).then_some(pid)
}

fn read_started(remote: &Remote) -> Option<Started> {
    let started: Started =
        serde_json::from_slice(&std::fs::read(remote.started_path()).ok()?).ok()?;
    (started.schema == STARTED_SCHEMA).then_some(started)
}

/// Adopt the host serving the control socket, or start one. Returns how the
/// host came to be and its pid.
fn ensure_host(
    runtime: &tokio::runtime::Runtime,
    remote: &Remote,
    launch: &Launch,
) -> Result<(String, Option<u32>), String> {
    // An owner asked for must be the owner the host already has, if any:
    // only a public key crosses, so this command can establish an owner but
    // never change one.
    if let Some(owner) = &remote.owner {
        let book = coder_host::access::host::Host::new(remote.state(), remote.policy());
        if book.state_path().exists() {
            // The host this helper may adopt opens the same store as it
            // serves; wait out its brief hold rather than refuse.
            let current = coder_host::authority::busy_retry(|| book.owner())
                .map_err(|error| format!("cannot read the host's owner: {error}"))?;
            if &current != owner {
                return Err(format!(
                    "the host in {} already has another owner ({current}); an owner is set only on a new host",
                    remote.root.display()
                ));
            }
        }
    }
    let answering = runtime
        .block_on(call(&remote.socket, Op::Status {}))
        .is_ok();
    let running = runtime_pid(remote);
    let ours = read_started(remote).filter(|started| Some(started.pid) == running);
    if answering {
        match (&ours, remote.restart) {
            (Some(started), true) => {
                stop(started.pid)?;
                let pid = start(runtime, remote, launch)?;
                return Ok(("restarted".into(), pid));
            }
            (Some(_), false) => return Ok(("reused".into(), running)),
            (None, _) => return Ok(("adopted".into(), running)),
        }
    }
    if let Some(pid) = running {
        return Err(format!(
            "a host already runs from {} (pid {pid}) without a control socket at {}; restart it with `--control`, or pass --remote-root DIR for a separate host",
            remote.root.display(),
            remote.socket.display()
        ));
    }
    let pid = start(runtime, remote, launch)?;
    Ok(("started".into(), pid))
}

fn stop(pid: u32) -> Result<(), String> {
    let target = libc::pid_t::try_from(pid).map_err(|_| "a bad pid")?;
    // SAFETY: kill has no memory preconditions.
    unsafe {
        libc::kill(target, libc::SIGTERM);
    }
    let until = Instant::now() + STOP_WAIT;
    while pid_alive(pid) {
        if Instant::now() >= until {
            return Err(format!("the old host (pid {pid}) did not stop"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Start `host serve` in its own session, so it outlives the SSH
/// connection, with its output in a log beside its state.
#[cfg_attr(not(test), allow(clippy::infallible_destructuring_match))]
fn start(
    runtime: &tokio::runtime::Runtime,
    remote: &Remote,
    launch: &Launch,
) -> Result<Option<u32>, String> {
    let program = match launch {
        Launch::Program(program) => program,
        #[cfg(test)]
        Launch::Never => return Err("no host answers the control socket".into()),
    };
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    use std::os::unix::process::CommandExt;
    for dir in [remote.root.clone(), remote.host_root()] {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(remote.log_path())
        .map_err(|error| format!("{}: {error}", remote.log_path().display()))?;
    let args = remote.serve_args();
    let mut command = Command::new(program);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|error| error.to_string())?)
        .stderr(log);
    // SAFETY: setsid is async-signal-safe and touches no memory; it runs in
    // the child between fork and exec.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start the host: {error}"))?;
    let pid = child.id();
    let started = Started {
        schema: STARTED_SCHEMA.into(),
        pid,
        program: program.display().to_string(),
        args,
        at: now(),
    };
    let bytes = serde_json::to_vec_pretty(&started).map_err(|error| error.to_string())?;
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(remote.started_path())
        .and_then(|mut file| file.write_all(&bytes))
        .map_err(|error| format!("{}: {error}", remote.started_path().display()))?;
    let until = Instant::now() + START_WAIT;
    loop {
        if runtime
            .block_on(call(&remote.socket, Op::Status {}))
            .is_ok()
        {
            // The host runs on in its own session; nothing here waits for it.
            drop(child);
            return Ok(Some(pid));
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "the host exited ({status}): {}",
                last_line(&std::fs::read_to_string(remote.log_path()).unwrap_or_default())
            ));
        }
        if Instant::now() >= until {
            return Err(format!(
                "the host did not open its control socket within {} s; see {}",
                START_WAIT.as_secs(),
                remote.log_path().display()
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sshd-less transport: each remote step runs in a local `sh` with a
    /// scratch `HOME`, and the helper runs in-process on its own thread with
    /// pipes for the SSH channel's standard streams.
    struct LocalShell {
        home: PathBuf,
    }

    impl Transport for LocalShell {
        fn run(
            &self,
            script: &str,
            args: &[String],
            stdin: Option<&Path>,
            wall: Duration,
        ) -> Result<Ran, String> {
            let mut command = Command::new("sh");
            command
                .arg("-c")
                .arg(script)
                .arg("oa-connect")
                .args(args)
                .env("HOME", &self.home);
            run_child(command, stdin, wall)
        }

        fn helper(&self, _bin: &str, args: &[String], _wall: Duration) -> Result<Session, String> {
            let remote = Remote::from_args(args)?;
            let (down_read, down_write) = std::io::pipe().map_err(|e| e.to_string())?;
            let (up_read, mut up_write) = std::io::pipe().map_err(|e| e.to_string())?;
            let helper = std::thread::spawn(move || {
                match serve_stdio(&remote, &Launch::Never, down_read, &mut up_write) {
                    Ok(()) => String::new(),
                    Err(message) => {
                        let _ = write_frame(&mut up_write, &refusal(message.clone()));
                        message
                    }
                }
            });
            Ok(Session {
                input: Box::new(down_write),
                output: Box::new(up_read),
                finish: Box::new(move || helper.join().unwrap_or_default()),
            })
        }
    }

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn frames_round_trip_and_refuse_what_is_over_the_bound() {
        let mut wire = Vec::new();
        let up = refusal("no");
        write_frame(&mut wire, &up).unwrap();
        let mut reader = wire.as_slice();
        assert_eq!(read_frame::<_, Up>(&mut reader).unwrap(), Some(up));
        assert_eq!(read_frame::<_, Up>(&mut reader).unwrap(), None);
        let mut long = u32::try_from(MAX_FRAME + 1).unwrap().to_be_bytes().to_vec();
        long.extend([b'{'; 8]);
        assert!(read_frame::<_, Up>(&mut long.as_slice()).is_err());
        assert!(read_frame::<_, Up>(&mut [0u8, 0].as_slice()).is_err());
        assert!(write_frame(&mut Vec::new(), &"x".repeat(MAX_FRAME)).is_err());
    }

    #[test]
    fn quoted_words_reach_the_remote_shell_unchanged() {
        for word in ["plain", "a'b", "$HOME `id` \"q\"", "~/x y"] {
            let out = Command::new("sh")
                .arg("-c")
                .arg(sh_line("printf %s \"$1\"", &[word.to_owned()]))
                .output()
                .unwrap();
            assert_eq!(String::from_utf8(out.stdout).unwrap(), word);
        }
    }

    #[test]
    fn usage_is_checked_before_anything_runs() {
        let plan = |list: &[&str]| Plan::from_args(&Args::parse(&words(list), SWITCHES).unwrap());
        assert!(matches!(plan(&[]), Err(Failure::Usage(_))));
        assert!(matches!(
            plan(&["-oProxyCommand=x"]),
            Err(Failure::Usage(_))
        ));
        assert!(matches!(plan(&["a@b", "c@d"]), Err(Failure::Usage(_))));
        assert!(matches!(
            plan(&["a@b", "--owner", "not-a-key"]),
            Err(Failure::Usage(_))
        ));
        assert!(matches!(
            plan(&["a@b", "--binary", "x", "--no-install"]),
            Err(Failure::Usage(_))
        ));
        assert!(matches!(
            plan(&["a@b", "--relay", "ws://example.com"]),
            Err(Failure::Usage(_))
        ));
        let owner = coder_reach::pubkey(&SecretKey::new(&mut secp256k1::rand::rng()));
        let ok = plan(&["a@b", "--owner", &owner, "--terminal"]).unwrap();
        assert_eq!(ok.owner.as_deref(), Some(owner.as_str()));
        let helper = ok.helper_args(true);
        // `--terminal` is accepted and changes nothing: every pairing grants
        // the full rights.
        assert!(!helper.contains(&"--terminal".to_owned()));
        assert!(helper.contains(&"--restart".to_owned()));
        assert!(helper.contains(&owner));
        // The helper reads back exactly what the plan sends.
        let remote = Remote::from_args(&helper).unwrap();
        assert_eq!(remote.owner.as_deref(), Some(owner.as_str()));
        assert!(remote.restart);
    }

    #[test]
    fn probe_and_install_run_on_a_posix_shell() {
        let temp = tempfile::tempdir().unwrap();
        let shell = LocalShell {
            home: temp.path().join("home"),
        };
        std::fs::create_dir_all(&shell.home).unwrap();
        let wall = Duration::from_secs(30);
        let probe = |shell: &LocalShell| {
            parse_probe(
                &shell
                    .run(PROBE, &[DEFAULT_REMOTE_BIN.to_owned()], None, wall)
                    .unwrap(),
            )
            .unwrap()
        };
        let first = probe(&shell);
        assert_eq!(first.sha, None);
        assert_eq!(
            first.bin,
            shell
                .home
                .join(".local/bin/openagents")
                .display()
                .to_string()
        );
        let (os, arch) = local_platform();
        assert_eq!((first.os.as_str(), first.arch.as_str()), (os, arch));

        let binary = temp.path().join("openagents");
        std::fs::write(&binary, "#!/bin/sh\necho openagents test build\n").unwrap();
        let sha = sha256_file(&binary).unwrap();
        let wrong = "0".repeat(64);
        let ran = shell
            .run(INSTALL, &[first.bin.clone(), wrong], Some(&binary), wall)
            .unwrap();
        assert!(field(&fields(&ran.stdout), "error").is_some());
        assert_eq!(probe(&shell).sha, None);
        let ran = shell
            .run(
                INSTALL,
                &[first.bin.clone(), sha.clone()],
                Some(&binary),
                wall,
            )
            .unwrap();
        assert_eq!(ran.code, Some(0), "{ran:?}");
        assert_eq!(probe(&shell).sha, Some(sha));

        // A binary that cannot run on the remote machine is named at once.
        let broken = temp.path().join("broken");
        std::fs::write(&broken, "#!/nonexistent/interpreter\n").unwrap();
        let ran = shell
            .run(
                INSTALL,
                &[first.bin, sha256_file(&broken).unwrap()],
                Some(&broken),
                wall,
            )
            .unwrap();
        assert_eq!(ran.code, Some(4), "{ran:?}");
        assert!(field(&fields(&ran.stdout), "error").is_some());
    }

    struct Fixture {
        temp: tempfile::TempDir,
        runtime: tokio::runtime::Runtime,
        running: Option<coder_host::Running>,
        socket: PathBuf,
        relay: String,
        host_key: String,
        owner: String,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Some(running) = self.running.take() {
                self.runtime.block_on(running.shutdown());
            }
        }
    }

    /// A host with an iroh endpoint on loopback and a control socket, as
    /// `host serve --iroh --control-socket` runs one.
    fn host() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let root = temp.path().join("remote");
        let access = root.join("coder-access");
        let owner_secret = SecretKey::new(&mut secp256k1::rand::rng());
        let owner = coder_reach::pubkey(&owner_secret);
        let store = coder_host::access::host::Host::new(&access, RelayPolicy::LoopbackTest);
        coder_host::access::host::ensure_parent(&access).unwrap();
        let host_key = store.init(&owner).unwrap();
        // Nothing listens here; enrollment over iroh needs no relay.
        let relay = "ws://127.0.0.1:9/".to_owned();
        let socket = root.join("control.sock");
        let mut config = coder_host::config::Config::new(access, vec![relay.clone()], 3);
        config.policy = RelayPolicy::LoopbackTest;
        config.iroh = Some(coder_host::config::Iroh::loopback());
        config.control = Some(coder_host::config::Control {
            path: socket.clone(),
            root: root.join("host"),
            autostart: None,
            tasks: root.join("tasks"),
            uid: coder_host::control::own_uid(),
        });
        config.label = "Headless Box".into();
        let running = runtime
            .block_on(coder_host::start(config, Arc::new(coder_host::NoTasks)))
            .unwrap();
        Fixture {
            temp,
            runtime,
            running: Some(running),
            socket,
            relay,
            host_key,
            owner,
        }
    }

    fn plan(fixture: &Fixture, binary: &Path, extra: &[&str]) -> Plan {
        let path = |path: &Path| path.display().to_string();
        let mut list = vec![
            "box@headless".to_owned(),
            "--binary".to_owned(),
            path(binary),
            "--remote-root".to_owned(),
            path(&fixture.temp.path().join("remote")),
            "--remote-socket".to_owned(),
            path(&fixture.socket),
            "--relay".to_owned(),
            fixture.relay.clone(),
            "--store".to_owned(),
            path(&fixture.temp.path().join("store")),
            "--loopback-test".to_owned(),
            "--timeout".to_owned(),
            "60".to_owned(),
        ];
        list.extend(words(extra));
        Plan::from_args(&Args::parse(&list, SWITCHES).unwrap()).unwrap()
    }

    #[test]
    fn one_command_yields_a_grant_and_an_iroh_hint() {
        let fixture = host();
        let shell = LocalShell {
            home: fixture.temp.path().join("home"),
        };
        std::fs::create_dir_all(&shell.home).unwrap();
        let binary = fixture.temp.path().join("openagents");
        std::fs::write(&binary, "#!/bin/sh\necho openagents test build\n").unwrap();

        let first = plan(&fixture, &binary, &[]);
        let value = connect(&first, &shell).unwrap();
        assert_eq!(value["binary"]["install"], "installed");
        assert_eq!(value["host"]["start"], "adopted");
        assert_eq!(value["host"]["key"], fixture.host_key);
        assert_eq!(value["host"]["label"], "Headless Box");
        assert_eq!(value["grant"]["owner"], fixture.owner);
        assert_eq!(value["grant"]["relay"], fixture.relay);
        assert_eq!(
            value["grant"]["rights"],
            json!([
                "observe",
                "operate",
                "terminal",
                "review",
                "access_read",
                "access_admin"
            ])
        );
        let endpoint = fixture.running.as_ref().unwrap().iroh_addr().unwrap().id;
        assert_eq!(value["iroh"]["endpoint"], endpoint.to_string());
        assert!(!value["iroh"]["addrs"].as_array().unwrap().is_empty());

        // The grant is in this computer's store, bound to its device key,
        // and the host lists the device.
        let mut store = FileStore::open(&first.store).unwrap();
        let saved = store.load().unwrap().unwrap();
        let device = load_or_create_key(&first.store).unwrap();
        let [saved_host] = saved.hosts.as_slice() else {
            panic!("one saved host");
        };
        assert_eq!(saved_host.access.grant.host, fixture.host_key);
        assert_eq!(saved_host.access.grant.device, coder_reach::pubkey(&device));
        assert_eq!(saved_host.ssh.as_deref(), Some("box@headless"));
        let route = saved_host.iroh.as_ref().expect("an iroh route");
        assert_eq!(route.id().unwrap(), endpoint);
        assert!(!route.direct.is_empty());
        let Reply::Devices { devices } = fixture
            .runtime
            .block_on(call(&fixture.socket, Op::DeviceList {}))
            .unwrap()
        else {
            panic!("devices");
        };
        assert!(
            devices
                .iter()
                .any(|entry| entry.device == coder_reach::pubkey(&device))
        );
        let record: Value =
            serde_json::from_slice(&std::fs::read(first.store.join(RECORD_FILE)).unwrap()).unwrap();
        assert_eq!(
            record["hosts"]["box@headless"]["iroh"]["endpoint"],
            endpoint.to_string()
        );

        // Again: the binary is current, and an earlier `--terminal` is
        // accepted and changes nothing.
        let again = connect(&plan(&fixture, &binary, &["--terminal"]), &shell).unwrap();
        assert_eq!(again["binary"]["install"], "current");
        assert_eq!(again["grant"]["rights"], value["grant"]["rights"]);
        // Every invitation was spent or cancelled.
        let Reply::Status(status) = fixture
            .runtime
            .block_on(call(&fixture.socket, Op::Status {}))
            .unwrap()
        else {
            panic!("status");
        };
        assert_eq!(status.outstanding_invitations, 0);
    }

    #[test]
    fn an_owner_is_set_only_on_a_new_host() {
        let fixture = host();
        let shell = LocalShell {
            home: fixture.temp.path().join("home"),
        };
        std::fs::create_dir_all(&shell.home).unwrap();
        let binary = fixture.temp.path().join("openagents");
        std::fs::write(&binary, "#!/bin/sh\necho openagents test build\n").unwrap();
        let other = coder_reach::pubkey(&SecretKey::new(&mut secp256k1::rand::rng()));
        let Err(Failure::Refused(message)) =
            connect(&plan(&fixture, &binary, &["--owner", &other]), &shell)
        else {
            panic!("another owner is refused");
        };
        assert!(message.contains("another owner"), "{message}");
        // The same owner is fine.
        let owner = fixture.owner.clone();
        connect(&plan(&fixture, &binary, &["--owner", &owner]), &shell).unwrap();
    }

    #[test]
    fn a_closed_channel_cancels_the_invitation() {
        let fixture = host();
        let root = fixture.temp.path().join("remote");
        let remote = Remote::from_args(&words(&[
            "--remote-root",
            root.to_str().unwrap(),
            "--control-socket",
            fixture.socket.to_str().unwrap(),
            "--relay",
            &fixture.relay,
            "--loopback-test",
        ]))
        .unwrap();
        let mut up = Vec::new();
        serve_stdio(&remote, &Launch::Never, std::io::empty(), &mut up).unwrap();
        let Some(Up::Ready { code, start, .. }) = read_frame(&mut up.as_slice()).unwrap() else {
            panic!("ready");
        };
        assert_eq!(start, "adopted");
        assert!(code.starts_with("openagents-connect:"));
        let Reply::Status(status) = fixture
            .runtime
            .block_on(call(&fixture.socket, Op::Status {}))
            .unwrap()
        else {
            panic!("status");
        };
        assert_eq!(status.outstanding_invitations, 0);
    }

    #[test]
    fn a_host_without_a_control_socket_is_left_alone() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("remote");
        std::fs::create_dir_all(root.join("host")).unwrap();
        std::fs::write(
            root.join("host/runtime"),
            format!(
                "schema=openagents.coder.host-runtime.v1\npid={}\nport=1\n",
                std::process::id()
            ),
        )
        .unwrap();
        let remote = Remote::from_args(&words(&[
            "--remote-root",
            root.to_str().unwrap(),
            "--loopback-test",
        ]))
        .unwrap();
        assert_eq!(remote.socket, root.join("control.sock"));
        let program = Launch::Program(PathBuf::from("/nonexistent/openagents"));
        let error = serve_stdio(&remote, &program, std::io::empty(), &mut Vec::new()).unwrap_err();
        assert!(error.contains("without a control socket"), "{error}");
    }
}
