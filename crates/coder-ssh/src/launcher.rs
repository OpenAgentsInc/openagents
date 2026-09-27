//! Installs, starts or adopts, enrolls, connects, and removes one host.

use std::io::Read as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest as _, Sha256};
use supervise::Ending;

use crate::REMOTE_SCRIPT;
use crate::askpass::Prompter;
use crate::error::Error;
use crate::protocol::{self, Report};
use crate::ssh::{Destination, Resolved, Ssh, Stdin, quote};
use crate::tunnel::Tunnel;

/// How long a script run may take, apart from an archive upload.
const SCRIPT_WALL: Duration = Duration::from_secs(120);

/// How long an archive upload may take.
const UPLOAD_WALL: Duration = Duration::from_secs(900);

/// How long the remote script waits for another launcher's lock.
const LOCK_WAIT_SECONDS: u32 = 30;

/// The exit code with which the script asks for the archive.
const NEED_ARCHIVE: i32 = 10;

/// The most arguments one runner command may have.
const RUNNER_ARGS_MAX: usize = 32;

/// The longest runner argument, in bytes.
const RUNNER_ARG_MAX: usize = 256;

/// The fixed command that stores an uploaded archive under a fresh name.
/// It reads the archive on standard input and interprets nothing from it.
const UPLOAD: &str = "umask 077; d=\"$HOME/.openagents/ssh-host/uploads\"; \
mkdir -p \"$d\" && chmod 700 \"$d\" && cat > \"$d/$1.part\" && mv -f \"$d/$1.part\" \"$d/$1\"";

/// A remote operating system with Coder builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    /// Linux.
    Linux,
    /// macOS.
    Macos,
}

impl Os {
    /// The name the remote script reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Os::Linux => "linux",
            Os::Macos => "macos",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [Os::Linux, Os::Macos]
            .into_iter()
            .find(|os| os.as_str() == text)
    }
}

/// A remote processor architecture with Coder builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    /// 64-bit x86.
    X86_64,
    /// 64-bit Arm.
    Aarch64,
}

impl Arch {
    /// The name the remote script reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [Arch::X86_64, Arch::Aarch64]
            .into_iter()
            .find(|arch| arch.as_str() == text)
    }
}

/// One pinned release archive: a gzip-compressed tar file whose top level
/// holds the `coder` binary for one platform.
#[derive(Clone, Debug)]
pub struct Artifact {
    /// The operating system the binary runs on.
    pub os: Os,
    /// The architecture the binary runs on.
    pub arch: Arch,
    /// The SHA-256 of the archive, as 64 lowercase hexadecimal characters.
    pub sha256: String,
    /// The archive on the local machine.
    pub archive: PathBuf,
}

/// The pinned archives one client installs, at most one per platform.
#[derive(Clone, Debug)]
pub struct Release {
    artifacts: Vec<Artifact>,
}

impl Release {
    /// Checks and pins a set of archives.
    ///
    /// # Errors
    ///
    /// Refuses an empty set, a digest that is not 64 lowercase hexadecimal
    /// characters, and two archives for one platform.
    pub fn new(artifacts: Vec<Artifact>) -> Result<Self, Error> {
        if artifacts.is_empty() {
            return Err(Error::InvalidRelease("a release needs an archive".into()));
        }
        for (index, artifact) in artifacts.iter().enumerate() {
            if !is_digest(&artifact.sha256) {
                return Err(Error::InvalidRelease(format!(
                    "the digest for {} {} is not a lowercase SHA-256",
                    artifact.os.as_str(),
                    artifact.arch.as_str()
                )));
            }
            if artifacts[..index]
                .iter()
                .any(|other| other.os == artifact.os && other.arch == artifact.arch)
            {
                return Err(Error::InvalidRelease(format!(
                    "two archives name {} {}",
                    artifact.os.as_str(),
                    artifact.arch.as_str()
                )));
            }
        }
        Ok(Release { artifacts })
    }

    fn entries(&self) -> impl Iterator<Item = String> + '_ {
        self.artifacts.iter().map(|artifact| {
            format!(
                "{}/{}/{}",
                artifact.os.as_str(),
                artifact.arch.as_str(),
                artifact.sha256
            )
        })
    }
}

fn is_digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// The arguments the installed `coder` binary runs with on the remote
/// machine.
///
/// `serve` starts a resident host. The host must bind loopback only, must
/// not fork into the background, and must write its own process identifier
/// and port to `~/.openagents/host/runtime` once it listens. `invite`
/// prints one single-use invitation line for this client and exits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runner {
    serve: Vec<String>,
    invite: Vec<String>,
}

impl Runner {
    /// Checks both argument lists.
    ///
    /// # Errors
    ///
    /// Refuses an empty list, more than 32 arguments, an argument longer
    /// than 256 bytes, and an argument that holds a control character.
    pub fn new(serve: Vec<String>, invite: Vec<String>) -> Result<Self, Error> {
        for (name, args) in [("serve", &serve), ("invite", &invite)] {
            if args.is_empty() || args.len() > RUNNER_ARGS_MAX {
                return Err(Error::InvalidRunner(format!(
                    "the {name} command needs between 1 and {RUNNER_ARGS_MAX} arguments"
                )));
            }
            if args.iter().any(|arg| {
                arg.len() > RUNNER_ARG_MAX || arg == "--" || arg.chars().any(char::is_control)
            }) {
                return Err(Error::InvalidRunner(format!(
                    "a {name} argument is too long, is `--`, or holds a control character"
                )));
            }
        }
        Ok(Runner { serve, invite })
    }

    /// The digest the remote machine records for a managed host. A change
    /// to the serve arguments relaunches a managed host.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        let mut hash = Sha256::new();
        for arg in &self.serve {
            hash.update(arg.as_bytes());
            hash.update([0]);
        }
        hex(&hash.finalize())
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// Whether the release was installed by this call or already present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Install {
    /// This call uploaded, verified, and installed the archive.
    Fresh,
    /// A verified copy was already installed.
    Reused,
}

/// How the running host came to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// This call started a new managed host.
    Started,
    /// A managed host with the same version and runner was already running.
    Reused,
    /// A managed host with a different version or runner was stopped and a
    /// new one started.
    Relaunched,
    /// A host that no launcher started was running and is now used as is.
    Adopted,
}

/// Who owns a host's lifetime, as recorded on the remote machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    /// A launcher started it. Only an explicit remove, or a launch with a
    /// changed release or runner, stops it.
    Managed,
    /// It was already running. Nothing here stops it; remove only detaches.
    External,
}

/// A host running on the remote machine. Dropping this value changes
/// nothing on the remote machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Host {
    /// The remote operating system.
    pub os: Os,
    /// The remote architecture.
    pub arch: Arch,
    /// Whether the archive was installed now or reused.
    pub install: Install,
    /// The SHA-256 of the installed archive.
    pub version: String,
    /// How the running host came to be.
    pub start: Start,
    /// Who owns the host's lifetime.
    pub ownership: Ownership,
    /// The host's process identifier on the remote machine.
    pub pid: u32,
    /// The host's loopback port on the remote machine.
    pub port: u16,
    /// Whether the run reclaimed an installation lock left by a dead owner.
    pub reclaimed_lock: bool,
}

/// What an explicit remove did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    /// A managed host was stopped.
    Stopped {
        /// The stopped host's process identifier.
        pid: u32,
    },
    /// An external host was left running; this client detached from it.
    Detached {
        /// The running host's process identifier.
        pid: u32,
    },
    /// No host was running.
    Absent,
}

/// A single-use enrollment invitation from the host. Its debug form never
/// shows its text.
#[derive(Clone, PartialEq, Eq)]
pub struct Invitation(String);

impl Invitation {
    /// The invitation text, for redemption. Do not log it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Invitation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Invitation(redacted)")
    }
}

/// Installs, starts or adopts, connects to, and removes the Coder host on
/// one SSH destination.
#[derive(Clone)]
pub struct Launcher {
    ssh: Ssh,
    release: Release,
    runner: Runner,
    lock_wait: u32,
}

impl std::fmt::Debug for Launcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Launcher")
            .field("destination", &self.ssh.destination)
            .field("program", &self.ssh.program)
            .field("prompter", &self.ssh.prompter.is_some())
            .finish_non_exhaustive()
    }
}

impl Launcher {
    /// A launcher for `destination` that runs `ssh` from `PATH`.
    ///
    /// # Errors
    ///
    /// Refuses a destination that `ssh` could read as an option.
    pub fn new(destination: &str, release: Release, runner: Runner) -> Result<Self, Error> {
        Ok(Launcher {
            ssh: Ssh {
                program: PathBuf::from("ssh"),
                destination: Destination::parse(destination)?,
                prompter: None,
            },
            release,
            runner,
            lock_wait: LOCK_WAIT_SECONDS,
        })
    }

    /// Runs this `ssh` program instead of the one on `PATH`.
    #[must_use]
    pub fn program(mut self, program: impl Into<PathBuf>) -> Self {
        self.ssh.program = program.into();
        self
    }

    /// Answers password and passphrase prompts through `prompter`. Without
    /// one, `ssh` runs in batch mode and fails rather than prompt.
    #[must_use]
    pub fn prompter(mut self, prompter: Arc<dyn Prompter>) -> Self {
        self.ssh.prompter = Some(prompter);
        self
    }

    /// Waits at most `seconds` for another launcher's remote lock.
    #[must_use]
    pub fn lock_wait(mut self, seconds: u32) -> Self {
        self.lock_wait = seconds.max(1);
        self
    }

    /// The destination this launcher reaches.
    #[must_use]
    pub fn destination(&self) -> &Destination {
        &self.ssh.destination
    }

    /// Asks `ssh -G` where the destination leads.
    ///
    /// # Errors
    ///
    /// Fails when `ssh -G` fails or reports no host name, user, and port.
    pub fn resolve(&self) -> Result<Resolved, Error> {
        self.ssh.resolve()
    }

    /// Installs the pinned archive when needed, then adopts the running host
    /// or starts one.
    ///
    /// # Errors
    ///
    /// Fails with the remote script's named error, or with [`Error::Ssh`]
    /// when `ssh` itself fails.
    pub fn up(&self) -> Result<Host, Error> {
        let (output, report) = self.script(&self.up_args("-"), SCRIPT_WALL, "the host script")?;
        if output.ending == Ending::Exited(Some(NEED_ARCHIVE)) {
            let need = report.require("need")?.to_string();
            let artifact = self
                .release
                .artifacts
                .iter()
                .find(|artifact| artifact.sha256 == need)
                .ok_or_else(|| {
                    Error::Protocol(
                        "the remote machine asked for an archive this release does not pin".into(),
                    )
                })?;
            check_local(artifact)?;
            let name = nonce()?;
            let upload = self.ssh.run(
                &format!("sh -c {} oa-ssh-upload {}", quote(UPLOAD), quote(&name)),
                Stdin::File(artifact.archive.clone()),
                UPLOAD_WALL,
                "the archive upload",
            )?;
            if upload.ending != Ending::Exited(Some(0)) {
                return Err(Error::Ssh {
                    code: upload.ending.code(),
                    detail: upload.detail(),
                });
            }
            let reclaimed = report.reclaimed;
            let (_, report) = self.script(&self.up_args(&name), SCRIPT_WALL, "the host script")?;
            return host(&report, reclaimed + report.reclaimed > 0);
        }
        host(&report, report.reclaimed > 0)
    }

    /// Forwards a reserved local loopback port to the host's port.
    ///
    /// # Errors
    ///
    /// Fails when no local port is free or `ssh` cannot start.
    pub fn connect(&self, host: &Host) -> Result<Tunnel, Error> {
        Tunnel::open(&self.ssh, host.port)
    }

    /// Runs the host's local invitation command and returns its invitation.
    /// The SSH session authorizes this one enrollment; redeem the invitation
    /// through the host's access contract.
    ///
    /// # Errors
    ///
    /// Fails when the pinned version is not installed or the invitation
    /// command fails.
    pub fn invite(&self, host: &Host) -> Result<Invitation, Error> {
        let mut args = vec![
            "invite".to_string(),
            self.lock_wait.to_string(),
            host.version.clone(),
            "--".to_string(),
        ];
        args.extend(self.runner.invite.iter().cloned());
        let (_, report) = self.script(&args, SCRIPT_WALL, "the invitation command")?;
        Ok(Invitation(report.require("invitation")?.to_string()))
    }

    /// Stops the host when the remote record says a launcher started it,
    /// and otherwise detaches from it. Apart from [`Launcher::up`] replacing
    /// a managed host whose release or runner changed, this is the only
    /// operation that stops a host.
    ///
    /// # Errors
    ///
    /// Fails with the remote script's named error, or with [`Error::Ssh`]
    /// when `ssh` itself fails.
    pub fn remove(&self) -> Result<Removal, Error> {
        let args = vec!["remove".to_string(), self.lock_wait.to_string()];
        let (_, report) = self.script(&args, SCRIPT_WALL, "the remove script")?;
        let pid = || -> Result<u32, Error> {
            report
                .require("pid")?
                .parse()
                .map_err(|_| Error::Protocol("invalid process identifier".into()))
        };
        match report.require("host")? {
            "stopped" => Ok(Removal::Stopped { pid: pid()? }),
            "detached" => Ok(Removal::Detached { pid: pid()? }),
            "absent" => Ok(Removal::Absent),
            other => Err(Error::Protocol(format!("unknown remove result {other}"))),
        }
    }

    fn up_args(&self, upload: &str) -> Vec<String> {
        let mut args = vec![
            "up".to_string(),
            self.lock_wait.to_string(),
            upload.to_string(),
            self.runner.fingerprint(),
        ];
        args.extend(self.release.entries());
        args.push("--".to_string());
        args.extend(self.runner.serve.iter().cloned());
        args
    }

    /// Sends the fixed script on standard input with `args` as its
    /// arguments, and reads its result.
    fn script(
        &self,
        args: &[String],
        wall: Duration,
        what: &'static str,
    ) -> Result<(crate::ssh::Output, Report), Error> {
        let mut remote = String::from("sh -s --");
        for arg in args {
            remote.push(' ');
            remote.push_str(&quote(arg));
        }
        let output = self.ssh.run(
            &remote,
            Stdin::Bytes(REMOTE_SCRIPT.as_bytes().to_vec()),
            wall,
            what,
        )?;
        let report = protocol::parse(&output.stdout)?;
        if let Some(error) = report.failure() {
            return Err(error);
        }
        match output.ending {
            Ending::Exited(Some(0)) => Ok((output, report)),
            Ending::Exited(Some(NEED_ARCHIVE)) if report.get("need").is_some() => {
                Ok((output, report))
            }
            Ending::Exited(code) => Err(Error::Ssh {
                code,
                detail: output.detail(),
            }),
            Ending::TimedOut => Err(Error::TimedOut(what)),
            Ending::Failed(why) => Err(Error::Spawn(why)),
        }
    }
}

fn host(report: &Report, reclaimed_lock: bool) -> Result<Host, Error> {
    let os = Os::parse(report.require("os")?)
        .ok_or_else(|| Error::Protocol("unknown operating system".into()))?;
    let arch = Arch::parse(report.require("arch")?)
        .ok_or_else(|| Error::Protocol("unknown architecture".into()))?;
    let install = match report.require("install")? {
        "fresh" => Install::Fresh,
        "reused" => Install::Reused,
        other => return Err(Error::Protocol(format!("unknown install result {other}"))),
    };
    let version = report.require("sha")?.to_string();
    if !is_digest(&version) {
        return Err(Error::Protocol("invalid installed version".into()));
    }
    let start = match report.require("host")? {
        "started" => Start::Started,
        "reused" => Start::Reused,
        "relaunched" => Start::Relaunched,
        "adopted" => Start::Adopted,
        other => return Err(Error::Protocol(format!("unknown host result {other}"))),
    };
    let ownership = match report.require("ownership")? {
        "managed" => Ownership::Managed,
        "external" => Ownership::External,
        other => return Err(Error::Protocol(format!("unknown ownership {other}"))),
    };
    if (start == Start::Adopted) != (ownership == Ownership::External) {
        return Err(Error::Protocol(
            "the host result and its ownership disagree".into(),
        ));
    }
    let pid = report
        .require("pid")?
        .parse()
        .map_err(|_| Error::Protocol("invalid process identifier".into()))?;
    let port = report
        .require("port")?
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| Error::Protocol("invalid host port".into()))?;
    Ok(Host {
        os,
        arch,
        install,
        version,
        start,
        ownership,
        pid,
        port,
        reclaimed_lock,
    })
}

/// Refuses to send a local archive that does not match its pin.
fn check_local(artifact: &Artifact) -> Result<(), Error> {
    let mut file = std::fs::File::open(&artifact.archive)?;
    let mut hash = Sha256::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        hash.update(&chunk[..read]);
    }
    if hex(&hash.finalize()) == artifact.sha256 {
        Ok(())
    } else {
        Err(Error::LocalChecksumMismatch(
            artifact.archive.display().to_string(),
        ))
    }
}

/// A fresh upload name: 16 random bytes as hexadecimal.
fn nonce() -> Result<String, Error> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(hex(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_pins_one_archive_per_platform() {
        let artifact = |sha: &str| Artifact {
            os: Os::Linux,
            arch: Arch::X86_64,
            sha256: sha.to_string(),
            archive: PathBuf::from("/dev/null"),
        };
        assert!(Release::new(vec![]).is_err());
        assert!(Release::new(vec![artifact("ABC")]).is_err());
        let digest = "a".repeat(64);
        assert!(Release::new(vec![artifact(&digest)]).is_ok());
        assert!(Release::new(vec![artifact(&digest), artifact(&digest)]).is_err());
    }

    #[test]
    fn runner_arguments_are_bounded_and_fingerprinted() {
        let serve = vec!["host".to_string(), "serve".to_string()];
        let invite = vec!["host".to_string(), "invite".to_string()];
        let one = Runner::new(serve.clone(), invite.clone()).unwrap();
        let two = Runner::new(
            vec!["host".into(), "serve".into(), "--x".into()],
            invite.clone(),
        )
        .unwrap();
        assert_ne!(one.fingerprint(), two.fingerprint());
        assert!(Runner::new(vec![], invite.clone()).is_err());
        assert!(Runner::new(vec!["a\nb".into()], invite.clone()).is_err());
        assert!(Runner::new(vec!["--".into()], invite).is_err());
    }

    #[test]
    fn invitations_do_not_print() {
        let invitation = Invitation("secret-invite".into());
        assert!(!format!("{invitation:?}").contains("secret"));
    }
}
