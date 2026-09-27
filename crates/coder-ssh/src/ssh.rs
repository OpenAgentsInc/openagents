//! The system `ssh` binary, run directly with Coder's own options.

use std::ffi::OsString;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use supervise::Ending;
use supervise::blocking;

use crate::askpass::{Askpass, Prompter};
use crate::error::Error;

/// The bytes of standard output kept from one invocation.
const STDOUT_MAX: usize = 64 * 1024;

/// The bytes of standard error kept from one invocation.
const STDERR_MAX: usize = 16 * 1024;

/// The characters of standard error an error message quotes.
const DETAIL_MAX: usize = 600;

/// An SSH destination as `ssh` accepts it: a configured alias,
/// `user@host`, or an `ssh://` URI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destination(String);

impl Destination {
    /// Checks that `text` can be passed to `ssh` as a destination and never
    /// as an option.
    ///
    /// # Errors
    ///
    /// Refuses an empty destination, one longer than 255 bytes, one that
    /// starts with `-`, and one that holds whitespace or a control
    /// character.
    pub fn parse(text: &str) -> Result<Self, Error> {
        if text.is_empty() {
            return Err(Error::InvalidDestination("the destination is empty".into()));
        }
        if text.len() > 255 {
            return Err(Error::InvalidDestination(
                "the destination is longer than 255 bytes".into(),
            ));
        }
        if text.starts_with('-') {
            return Err(Error::InvalidDestination(
                "a destination cannot start with a hyphen".into(),
            ));
        }
        if text.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(Error::InvalidDestination(
                "a destination cannot hold whitespace or control characters".into(),
            ));
        }
        Ok(Destination(text.to_string()))
    }

    /// The destination as it is passed to `ssh`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where `ssh -G` says a destination leads after your SSH configuration is
/// applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The host name `ssh` connects to.
    pub hostname: String,
    /// The remote user name.
    pub user: String,
    /// The remote SSH port.
    pub port: u16,
}

/// What standard input an invocation reads.
pub(crate) enum Stdin {
    Bytes(Vec<u8>),
    File(PathBuf),
}

/// What an invocation left behind.
pub(crate) struct Output {
    pub(crate) ending: Ending,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

impl Output {
    /// The end of standard error, for an error message.
    pub(crate) fn detail(&self) -> String {
        let text = String::from_utf8_lossy(&self.stderr);
        let text = text.trim();
        let start = text
            .char_indices()
            .rev()
            .nth(DETAIL_MAX)
            .map_or(0, |(index, _)| index);
        text[start..].to_string()
    }
}

/// One destination reached through one `ssh` program.
#[derive(Clone)]
pub(crate) struct Ssh {
    pub(crate) program: PathBuf,
    pub(crate) destination: Destination,
    pub(crate) prompter: Option<Arc<dyn Prompter>>,
}

/// Whether an invocation forwards a port.
pub(crate) enum Forwarding {
    /// Clear every forwarding, including ones from the SSH configuration.
    None,
    /// Forward exactly one local port and nothing else.
    Local { local: u16, remote: u16 },
}

impl Ssh {
    /// Builds an `ssh` command for this destination, with the askpass helper
    /// that must live as long as the command runs.
    pub(crate) fn command(
        &self,
        forwarding: &Forwarding,
        remote: Option<&str>,
    ) -> Result<(Command, Option<Askpass>), Error> {
        let mut command = Command::new(&self.program);
        command.args(options(self.prompter.is_none(), forwarding));
        command.arg("--").arg(self.destination.as_str());
        if let Some(remote) = remote {
            command.arg(remote);
        }
        let askpass = match &self.prompter {
            Some(prompter) => {
                let askpass = Askpass::start(Arc::clone(prompter))?;
                command.env("SSH_ASKPASS", askpass.helper());
                command.env("SSH_ASKPASS_REQUIRE", "force");
                Some(askpass)
            }
            None => {
                command.env_remove("SSH_ASKPASS");
                command.env_remove("SSH_ASKPASS_REQUIRE");
                None
            }
        };
        Ok((command, askpass))
    }

    /// Runs one remote command with no forwarding and waits for it, at most
    /// `wall`.
    pub(crate) fn run(
        &self,
        remote: &str,
        stdin: Stdin,
        wall: Duration,
        what: &'static str,
    ) -> Result<Output, Error> {
        let (mut command, askpass) = self.command(&Forwarding::None, Some(remote))?;
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        blocking::own_group(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| Error::Spawn(format!("{}: {error}", self.program.display())))?;
        let writer = child.stdin.take().map(|mut pipe| {
            std::thread::spawn(move || {
                // A remote side that stops reading closes the pipe; the
                // script's own result says why.
                match stdin {
                    Stdin::Bytes(bytes) => {
                        let _ = pipe.write_all(&bytes);
                    }
                    Stdin::File(path) => {
                        if let Ok(mut file) = std::fs::File::open(path) {
                            let _ = std::io::copy(&mut file, &mut pipe);
                        }
                    }
                }
            })
        });
        let stdout = capture(child.stdout.take(), STDOUT_MAX);
        let stderr = capture(child.stderr.take(), STDERR_MAX);
        let ending = blocking::wait(&mut child, wall);
        drop(askpass);
        let stdout = collect(stdout);
        let stderr = collect(stderr);
        if let Some(writer) = writer
            && writer.is_finished()
        {
            let _ = writer.join();
        }
        if ending == Ending::TimedOut {
            return Err(Error::TimedOut(what));
        }
        if let Ending::Failed(why) = &ending {
            return Err(Error::Spawn(why.clone()));
        }
        Ok(Output {
            ending,
            stdout,
            stderr,
        })
    }

    /// Asks `ssh -G` where the destination leads.
    pub(crate) fn resolve(&self) -> Result<Resolved, Error> {
        let mut command = Command::new(&self.program);
        command
            .arg("-G")
            .arg("--")
            .arg(self.destination.as_str())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        blocking::own_group(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| Error::Spawn(format!("{}: {error}", self.program.display())))?;
        let stdout = capture(child.stdout.take(), STDOUT_MAX);
        let stderr = capture(child.stderr.take(), STDERR_MAX);
        let ending = blocking::wait(&mut child, Duration::from_secs(10));
        let output = Output {
            ending,
            stdout: collect(stdout),
            stderr: collect(stderr),
        };
        match output.ending {
            Ending::Exited(Some(0)) => {}
            Ending::TimedOut => return Err(Error::TimedOut("ssh -G")),
            Ending::Failed(why) => return Err(Error::Spawn(why)),
            Ending::Exited(code) => {
                return Err(Error::Ssh {
                    code,
                    detail: output.detail(),
                });
            }
        }
        parse_resolved(&String::from_utf8_lossy(&output.stdout))
    }
}

/// The options Coder sets on every connection it makes.
fn options(batch: bool, forwarding: &Forwarding) -> Vec<OsString> {
    let mut args: Vec<String> = vec!["-T".into()];
    let mut set = |option: &str| {
        args.push("-o".into());
        args.push(option.into());
    };
    // Coder's connections never share or create a multiplexed master.
    set("ControlMaster=no");
    set("ControlPath=none");
    set("ControlPersist=no");
    set("ForwardAgent=no");
    set("ForwardX11=no");
    set("PermitLocalCommand=no");
    set("RequestTTY=no");
    set("ServerAliveInterval=15");
    set("ServerAliveCountMax=3");
    set(if batch {
        "BatchMode=yes"
    } else {
        "BatchMode=no"
    });
    match forwarding {
        Forwarding::None => set("ClearAllForwardings=yes"),
        Forwarding::Local { .. } => set("ExitOnForwardFailure=yes"),
    }
    if let Forwarding::Local { local, remote } = forwarding {
        args.push("-N".into());
        args.push("-L".into());
        args.push(format!("127.0.0.1:{local}:127.0.0.1:{remote}"));
    }
    args.into_iter().map(OsString::from).collect()
}

fn parse_resolved(text: &str) -> Result<Resolved, Error> {
    let mut hostname = None;
    let mut user = None;
    let mut port = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(' ') else {
            continue;
        };
        match key {
            "hostname" if hostname.is_none() => hostname = Some(value.trim().to_string()),
            "user" if user.is_none() => user = Some(value.trim().to_string()),
            "port" if port.is_none() => {
                port = Some(value.trim().parse::<u16>().map_err(|_| {
                    Error::Protocol(format!("ssh -G reported an invalid port: {value}"))
                })?);
            }
            _ => {}
        }
    }
    match (hostname, user, port) {
        (Some(hostname), Some(user), Some(port)) if !hostname.is_empty() && !user.is_empty() => {
            Ok(Resolved {
                hostname,
                user,
                port,
            })
        }
        _ => Err(Error::Protocol(
            "ssh -G did not report a host name, user, and port".into(),
        )),
    }
}

/// Quotes one word for a POSIX shell.
pub(crate) fn quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', "'\\''"))
}

type Buffer = Arc<Mutex<Vec<u8>>>;

/// Drains a stream into a capped buffer. Bytes past the cap are read and
/// dropped, so the writer never stalls on a full pipe.
fn capture<R: std::io::Read + Send + 'static>(
    stream: Option<R>,
    max: usize,
) -> Option<(Buffer, JoinHandle<()>)> {
    let mut stream = stream?;
    let buffer: Buffer = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&buffer);
    let thread = std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(read) => {
                    let Ok(mut kept) = kept.lock() else { return };
                    let room = max.saturating_sub(kept.len());
                    kept.extend_from_slice(&chunk[..read.min(room)]);
                }
            }
        }
    });
    Some((buffer, thread))
}

/// Takes what a capture kept, waiting briefly for it to reach the end of
/// its stream.
fn collect(capture: Option<(Buffer, JoinHandle<()>)>) -> Vec<u8> {
    let Some((buffer, thread)) = capture else {
        return Vec::new();
    };
    let deadline = Instant::now() + Duration::from_secs(1);
    while !thread.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    if thread.is_finished() {
        let _ = thread.join();
    }
    buffer.lock().map(|kept| kept.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destinations_cannot_become_options() {
        assert!(Destination::parse("-oProxyCommand=x").is_err());
        assert!(Destination::parse("host name").is_err());
        assert!(Destination::parse("").is_err());
        assert!(Destination::parse("me@example.test").is_ok());
        assert!(Destination::parse("devbox").is_ok());
    }

    #[test]
    fn quoting_survives_single_quotes() {
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn connections_disable_multiplexing_and_forward_only_loopback() {
        let args: Vec<String> = options(
            true,
            &Forwarding::Local {
                local: 40000,
                remote: 47001,
            },
        )
        .into_iter()
        .map(|arg| arg.into_string().unwrap())
        .collect();
        assert!(args.contains(&"ControlMaster=no".to_string()));
        assert!(args.contains(&"ControlPath=none".to_string()));
        assert!(args.contains(&"127.0.0.1:40000:127.0.0.1:47001".to_string()));
        assert!(args.contains(&"-N".to_string()));
        let script: Vec<String> = options(false, &Forwarding::None)
            .into_iter()
            .map(|arg| arg.into_string().unwrap())
            .collect();
        assert!(script.contains(&"ClearAllForwardings=yes".to_string()));
        assert!(script.contains(&"BatchMode=no".to_string()));
    }

    #[test]
    fn resolution_reads_the_first_value_of_each_key() {
        let resolved =
            parse_resolved("user me\nhostname box.example\nport 2200\nport 22\n").unwrap();
        assert_eq!(resolved.hostname, "box.example");
        assert_eq!(resolved.user, "me");
        assert_eq!(resolved.port, 2200);
        assert!(parse_resolved("user me\n").is_err());
    }
}
