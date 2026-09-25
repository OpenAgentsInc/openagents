//! An enforced filesystem write boundary around one command.
//!
//! A delegation that may not write, or that may write only into a checkout
//! of its own, is a promise the host has to keep. This module keeps it on
//! macOS by wrapping the command in `sandbox-exec` with a profile that
//! denies `file-write*` everywhere and then permits exactly the paths the
//! caller named, and on Linux by wrapping it in `bwrap` (bubblewrap) with
//! a mount namespace that binds the whole filesystem read-only and then
//! binds exactly those paths writable. On a platform with no backend,
//! [`Spec::build`] refuses — there is no path from here to an unrestricted
//! [`Command`], because a boundary that quietly stopped bounding is worse
//! than no boundary.
//!
//! The two backends enforce the same policy by different means. Seatbelt
//! evaluates rules in order, so a deny followed by an allow beneath it is
//! an exception; a mount namespace stacks binds in order, so a read-only
//! root followed by a writable bind beneath it is the same exception. Both
//! cover the whole process tree, and neither confines reads or time — see
//! the crate root. Network is open unless the caller asks for
//! [`Spec::offline`].
//!
//! # The two policies
//!
//! [`Boundary::readonly`] permits writes only beneath the paths it is
//! handed — private scratch space and the adapter state an executor is
//! trusted to keep — and requires those paths disjoint from every denied
//! path. [`Boundary::writing`] adds one isolated checkout to the writable
//! set while the main checkout, the common Git directory, and the rest of
//! the host stay denied. A delegate's worktree commonly sits *inside* the
//! checkout it branched from; the profile expresses that exception by
//! ordering every deny before every allow. The checkout is the only
//! exception: [`Spec::sealed`] paths accept none at all, which is where
//! the common Git directory belongs.
//!
//! # Ownership
//!
//! The profile lives in a file the boundary owns, and an owned scratch
//! directory is owned the same way. Both are valid only while the
//! boundary is alive, so the boundary must outlive the child *and its
//! reaping* — a child still running against a scratch that `Drop` removed
//! is the failure `supervise::Job::run_holding` exists for. The intended
//! wiring, for a caller that drives the child through `supervise`:
//!
//! ```no_run
//! # use coder_boundary::Boundary;
//! # use std::process::Command;
//! # fn example() -> Result<(), coder_boundary::Error> {
//! let boundary = Boundary::readonly()
//!     .protecting("/absolute/main-checkout")
//!     .owned_scratch_under("/absolute/private-parent")
//!     .build()?;
//! let mut command: Command = boundary.command("/usr/bin/env", ["true"])?;
//! // Spawn through the supervisor, and keep the boundary until the child
//! // is reaped: `job.run_holding(boundary.hold())`, or hold `boundary`
//! // itself in the task that awaits the supervised child.
//! let held = boundary.hold();
//! # let _ = (command, held);
//! # Ok(())
//! # }
//! ```
//!
//! A caller that speaks to `supervise::Job` rather than to a `Command`
//! builds the job over [`Boundary::backend`] and [`Boundary::arguments`],
//! and holds [`Boundary::hold`] the same way.

use std::ffi::OsStr;
use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::{NamedTempFile, TempDir};

/// The macOS backend: `sandbox-exec`, at the path Apple ships it.
pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// The Linux backend: bubblewrap, at the path distributions install it.
/// It needs unprivileged user namespaces; a host that has the binary but
/// not the namespaces is [`Error::Inoperable`], not a boundary.
pub const BUBBLEWRAP: &str = "/usr/bin/bwrap";

/// Bubblewrap on NixOS, which installs system packages under the
/// root-owned system profile instead of `/usr/bin`.
pub const BUBBLEWRAP_NIXOS: &str = "/run/current-system/sw/bin/bwrap";

/// The fixed, root-owned paths the Linux backend is taken from, in order.
/// The boundary never searches `PATH`: a writable directory on the search
/// path would let anything that can write there choose the sandbox.
pub const BUBBLEWRAP_PATHS: [&str; 2] = [BUBBLEWRAP, BUBBLEWRAP_NIXOS];

/// The backend binary this host would use: on Linux, the first of
/// [`BUBBLEWRAP_PATHS`] that exists, else [`BUBBLEWRAP`] so the refusal
/// names the conventional path.
pub fn backend_path() -> &'static str {
    if cfg!(target_os = "linux") {
        BUBBLEWRAP_PATHS
            .into_iter()
            .find(|path| Path::new(path).is_file())
            .unwrap_or(BUBBLEWRAP)
    } else {
        BACKEND.unwrap_or(SANDBOX_EXEC)
    }
}

/// The backend this platform enforces with, or `None` where there is none.
pub const BACKEND: Option<&str> = if cfg!(target_os = "macos") {
    Some(SANDBOX_EXEC)
} else if cfg!(target_os = "linux") {
    Some(BUBBLEWRAP)
} else {
    None
};

/// Why a boundary could not be built or a command could not be wrapped.
///
/// Every variant is a refusal. Nothing here degrades to an unrestricted
/// command.
#[derive(Debug)]
pub enum Error {
    /// There is no enforced backend on this platform.
    Unsupported(&'static str),
    /// The backend this platform uses is not at its path.
    Unavailable(PathBuf),
    /// The backend is present but cannot confine anything on this host —
    /// on Linux, unprivileged user namespaces are disabled.
    Inoperable { backend: PathBuf, error: String },
    /// A path that had to be absolute was not: the program, the backend,
    /// or one of the configured paths.
    Relative(PathBuf),
    /// A path that had to resolve could not — it does not exist or
    /// cannot be canonicalized.
    Resolve { path: PathBuf, error: String },
    /// A path has no safe spelling in a Seatbelt profile: it is not
    /// UTF-8, or it carries a control character.
    Unsafe(PathBuf),
    /// A writable path and a denied path overlap in a direction the
    /// profile cannot express. Writable paths stay disjoint from every
    /// protected and sealed path; the only permitted nesting is the
    /// checkout beneath a protected one, and never beneath a sealed one.
    Overlap {
        writable: PathBuf,
        protected: PathBuf,
    },
    /// The profile file or the owned scratch directory could not be made.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported(os) => {
                write!(
                    f,
                    "{os} has no supported way to limit which files a command can write"
                )
            }
            Error::Unavailable(path) => {
                write!(f, "the sandbox program {} is not installed", path.display())
            }
            Error::Inoperable { backend, error } => write!(
                f,
                "the sandbox program {} can't restrict a command on this machine: {error}",
                backend.display()
            ),
            Error::Relative(path) => {
                write!(f, "{} is not an absolute path", path.display())
            }
            Error::Resolve { path, error } => {
                write!(f, "can't find {}: {error}", path.display())
            }
            Error::Unsafe(path) => write!(
                f,
                "the path {} contains characters that a sandbox profile can't hold",
                path.display()
            ),
            Error::Overlap {
                writable,
                protected,
            } => write!(
                f,
                "writable path {} overlaps protected path {}",
                writable.display(),
                protected.display()
            ),
            Error::Io(error) => write!(f, "can't set up the filesystem write boundary: {error}"),
        }
    }
}

impl std::error::Error for Error {}

/// A policy under construction, from [`Boundary::readonly`] or
/// [`Boundary::writing`]. Every path named here is resolved before the
/// boundary exists, and [`Spec::build`] refuses what it cannot enforce.
#[derive(Debug)]
pub struct Spec {
    checkout: Option<PathBuf>,
    writable: Vec<PathBuf>,
    protected: Vec<PathBuf>,
    sealed: Vec<PathBuf>,
    scratch_under: Option<PathBuf>,
    offline: bool,
    backend: PathBuf,
}

impl Spec {
    fn new(checkout: Option<PathBuf>) -> Self {
        Spec {
            checkout,
            writable: Vec::new(),
            protected: Vec::new(),
            sealed: Vec::new(),
            scratch_under: None,
            offline: false,
            backend: PathBuf::from(backend_path()),
        }
    }

    /// A path the command may write beneath: private scratch space, or an
    /// adapter's trusted state directory. The path must exist and must
    /// canonicalize — a host that wants a state directory creates it
    /// before granting it — and a symlinked alias resolves to the real
    /// directory rather than widening the grant. Writable paths stay
    /// disjoint from every protected and sealed path in both policies.
    #[must_use]
    pub fn writable(mut self, path: impl Into<PathBuf>) -> Self {
        self.writable.push(path.into());
        self
    }

    /// A path that must stay unwritten while writes go on around it, but
    /// that may contain the one isolated checkout of a writing boundary —
    /// the main checkout, which is where a delegate's worktree lives.
    /// Protected paths must exist, and a protected path reached through a
    /// symlink is the resolved one that stays protected.
    #[must_use]
    pub fn protecting(mut self, path: impl Into<PathBuf>) -> Self {
        self.protected.push(path.into());
        self
    }

    /// A path that must stay unwritten and accepts no exception at all:
    /// the common Git directory. No writable path may sit beneath it, and
    /// a writing boundary's checkout may not either.
    #[must_use]
    pub fn sealed(mut self, path: impl Into<PathBuf>) -> Self {
        self.sealed.push(path.into());
        self
    }

    /// A scratch directory the boundary itself owns: created under
    /// `parent`, writable to the command, and removed when the boundary
    /// is dropped — which, held through the supervisor, is after the
    /// child is reaped rather than while it is still writing.
    #[must_use]
    pub fn owned_scratch_under(mut self, parent: impl Into<PathBuf>) -> Self {
        self.scratch_under = Some(parent.into());
        self
    }

    /// The command gets no network beyond the loopback interface. On
    /// Linux, `bwrap --unshare-net` gives it a network namespace of its
    /// own that holds only `lo`; on macOS, the profile denies outbound
    /// connections to any address but `localhost`. A command that tries to
    /// reach another host fails to connect.
    #[must_use]
    pub fn offline(mut self) -> Self {
        self.offline = true;
        self
    }

    /// Resolves every configured path, checks the overlaps a profile
    /// cannot express, writes the profile, and returns the boundary that
    /// owns it. On a platform with no enforced backend this is
    /// [`Error::Unsupported`], not a degraded command.
    pub fn build(self) -> Result<Boundary, Error> {
        let checkout = self.checkout.as_deref().map(existing).transpose()?;
        let mut protected = Vec::with_capacity(self.protected.len());
        for path in &self.protected {
            protected.push(existing(path)?);
        }
        let mut sealed = Vec::with_capacity(self.sealed.len());
        for path in &self.sealed {
            sealed.push(existing(path)?);
        }
        let mut writable = Vec::with_capacity(self.writable.len() + 1);
        for path in &self.writable {
            writable.push(existing(path)?);
        }
        let scratch = match &self.scratch_under {
            None => None,
            Some(parent) => {
                let parent = existing(parent)?;
                let scratch = tempfile::tempdir_in(&parent).map_err(Error::Io)?;
                writable.push(existing(scratch.path())?);
                Some(scratch)
            }
        };

        // What the profile cannot say, the spec refuses. The denies land
        // before the allows, so a writable path beneath a denied one is
        // the exception — and the only permitted exception is the
        // checkout beneath a protected path, which is where a delegate's
        // worktree lives. Every other overlap direction would either
        // silently unprotect a denied path or grant an exception nobody
        // stated, so it is refused.
        for w in &writable {
            for p in protected.iter().chain(&sealed) {
                if w.starts_with(p) || p.starts_with(w) {
                    return Err(Error::Overlap {
                        writable: w.clone(),
                        protected: p.clone(),
                    });
                }
            }
        }
        if let Some(checkout) = &checkout {
            for p in &protected {
                if p.starts_with(checkout) {
                    return Err(Error::Overlap {
                        writable: checkout.clone(),
                        protected: p.clone(),
                    });
                }
            }
            for p in &sealed {
                if checkout.starts_with(p) || p.starts_with(checkout) {
                    return Err(Error::Overlap {
                        writable: checkout.clone(),
                        protected: p.clone(),
                    });
                }
            }
        }

        let mut profile = String::from("(version 1)\n(allow default)\n(deny file-write*)\n");
        for path in protected.iter().chain(&sealed) {
            profile.push_str(&deny(path)?);
        }
        for path in checkout.iter().chain(writable.iter()) {
            profile.push_str(&allow(path)?);
        }
        profile.push_str("(allow file-write* (literal \"/dev/null\"))\n");
        if self.offline {
            profile.push_str(
                "(deny network-outbound (remote ip))\n\
                 (allow network-outbound (remote ip \"localhost:*\"))\n",
            );
        }

        // Validation first, refusal second: on a platform with no
        // backend, every answer above still describes the spec that was
        // asked for. What never happens here is the fallback — there is
        // no construction of a bare command.
        if BACKEND.is_none() {
            return Err(Error::Unsupported(std::env::consts::OS));
        }
        if !self.backend.is_absolute() {
            return Err(Error::Relative(self.backend));
        }
        if !self.backend.is_file() {
            return Err(Error::Unavailable(self.backend));
        }
        if cfg!(target_os = "linux") {
            operable(&self.backend)?;
        }

        let mut file = NamedTempFile::new().map_err(Error::Io)?;
        file.write_all(profile.as_bytes()).map_err(Error::Io)?;
        file.flush().map_err(Error::Io)?;

        Ok(Boundary {
            backend: self.backend,
            profile,
            file,
            scratch,
            checkout,
            writable,
            protected,
            sealed,
            offline: self.offline,
        })
    }
}

/// An enforceable boundary: a written profile, the resolved paths it
/// names, and whatever scratch it owns.
///
/// Dropping a `Boundary` removes the profile file and the owned scratch,
/// so it must outlive the reaping of the child it wrapped. It is `Send`
/// for exactly that reason: hand it — or [`Boundary::hold`] — to
/// `supervise::Job::run_holding`, or hold it in the task that awaits the
/// supervised child.
#[derive(Debug)]
pub struct Boundary {
    backend: PathBuf,
    profile: String,
    file: NamedTempFile,
    scratch: Option<TempDir>,
    checkout: Option<PathBuf>,
    writable: Vec<PathBuf>,
    protected: Vec<PathBuf>,
    sealed: Vec<PathBuf>,
    offline: bool,
}

impl Boundary {
    /// A policy that denies every write but the paths it is handed.
    pub fn readonly() -> Spec {
        Spec::new(None)
    }

    /// A policy that also permits one isolated checkout: the worktree the
    /// delegation owns, which commonly sits inside the protected main
    /// checkout.
    pub fn writing(checkout: impl Into<PathBuf>) -> Spec {
        Spec::new(Some(checkout.into()))
    }

    /// The command, wrapped: on macOS `sandbox-exec -f <profile> <program>
    /// <argv>`, on Linux `bwrap <binds> -- <program> <argv>`.
    ///
    /// The program must be absolute; a name resolved through the child's
    /// `PATH` is a search the boundary never approved. The result is a
    /// [`std::process::Command`], so a caller on Tokio takes it as
    /// `tokio::process::Command::from(command)` and a caller on
    /// `supervise` gives it `blocking::own_group` before spawning. Either
    /// way the boundary itself must stay held until the child is reaped.
    pub fn command<I, A>(&self, program: impl AsRef<Path>, arguments: I) -> Result<Command, Error>
    where
        I: IntoIterator<Item = A>,
        A: AsRef<OsStr>,
    {
        let program = program.as_ref();
        if !program.is_absolute() {
            return Err(Error::Relative(program.to_path_buf()));
        }
        let mut command = Command::new(&self.backend);
        command.args(self.arguments()).arg(program).args(arguments);
        Ok(command)
    }

    /// The backend the wrapped command runs under, by absolute path.
    #[must_use]
    pub fn backend(&self) -> &Path {
        &self.backend
    }

    /// The backend's arguments, which come before the program: `-f
    /// <profile>` for `sandbox-exec`; the read-only root, the device
    /// tree, the writable binds, and `--` for `bwrap`. For a caller that
    /// builds its own supervised argv over [`Boundary::backend`]. Valid
    /// only while the boundary is held.
    #[must_use]
    pub fn arguments(&self) -> Vec<std::ffi::OsString> {
        if cfg!(target_os = "linux") {
            let mut args: Vec<std::ffi::OsString> = [
                "--die-with-parent",
                "--ro-bind",
                "/",
                "/",
                "--dev",
                "/dev",
                "--proc",
                "/proc",
            ]
            .into_iter()
            .map(Into::into)
            .collect();
            // Every bind stacks over the read-only root in order, so a
            // writable path beneath a protected one is the exception and
            // the protected tree around it stays read-only.
            for path in self.checkout.iter().chain(&self.writable) {
                args.push("--bind".into());
                args.push(path.into());
                args.push(path.into());
            }
            if self.offline {
                args.push("--unshare-net".into());
            }
            args.push("--".into());
            args
        } else {
            vec!["-f".into(), self.file.path().into()]
        }
    }

    /// The profile file, for a caller that builds its own supervised
    /// argv. The path is valid only while the boundary is held.
    #[must_use]
    pub fn profile_file(&self) -> &Path {
        self.file.path()
    }

    /// The policy, spelled as a Seatbelt profile. On macOS this is the
    /// text the backend reads; on Linux it is the record of what the
    /// binds in [`Boundary::arguments`] enforce.
    #[must_use]
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// The scratch directory the boundary owns, when it owns one.
    #[must_use]
    pub fn scratch(&self) -> Option<&Path> {
        self.scratch.as_ref().map(TempDir::path)
    }

    /// The writable paths the profile permits, canonicalized. An owned
    /// scratch directory is among them; a writing boundary's checkout is
    /// reported by [`Boundary::checkout`] instead.
    #[must_use]
    pub fn writable(&self) -> &[PathBuf] {
        &self.writable
    }

    /// The protected paths the profile denies, canonicalized. These are
    /// the paths that may contain the checkout; see
    /// [`Spec::protecting`].
    #[must_use]
    pub fn protected(&self) -> &[PathBuf] {
        &self.protected
    }

    /// The sealed paths the profile denies, canonicalized — protected
    /// paths that accept no exception, not even the checkout.
    #[must_use]
    pub fn sealed(&self) -> &[PathBuf] {
        &self.sealed
    }

    /// Whether the command runs with no network beyond loopback; see
    /// [`Spec::offline`].
    #[must_use]
    pub fn offline(&self) -> bool {
        self.offline
    }

    /// The isolated checkout of a writing boundary.
    #[must_use]
    pub fn checkout(&self) -> Option<&Path> {
        self.checkout.as_deref()
    }

    /// The part of the boundary that must outlive the reaping: the
    /// profile file and the owned scratch. Build every command first,
    /// then hand this to `supervise::Job::run_holding` or hold it across
    /// the supervised wait.
    #[must_use]
    pub fn hold(self) -> Held {
        Held {
            file: self.file,
            scratch: self.scratch,
        }
    }
}

/// The held half of a [`Boundary`]: the profile file and the owned
/// scratch, and nothing else. It exists to be passed to
/// `supervise::Job::run_holding`, so both survive until the supervisor
/// has reaped the child rather than until the caller stops waiting.
#[derive(Debug)]
pub struct Held {
    file: NamedTempFile,
    scratch: Option<TempDir>,
}

impl Held {
    /// The profile path a supervised argv names with `-f`, valid while
    /// this is held.
    #[must_use]
    pub fn profile_file(&self) -> &Path {
        self.file.path()
    }

    /// The owned scratch directory's path, valid while this is held.
    #[must_use]
    pub fn scratch(&self) -> Option<&Path> {
        self.scratch.as_ref().map(TempDir::path)
    }
}

/// Whether the Linux backend can confine anything here, checked once per
/// process. A `bwrap` binary on a host with user namespaces disabled
/// fails at spawn rather than running unbounded, but a spec built over it
/// would be a promise; asking `bwrap` to run `/bin/sh -c :` inside a
/// read-only root settles the question before any boundary exists.
/// `/bin/sh` is the one command every POSIX host has; `/bin/true` is
/// absent on NixOS.
fn operable(backend: &Path) -> Result<(), Error> {
    use std::sync::OnceLock;
    static PROBE: OnceLock<Result<(), String>> = OnceLock::new();
    let probe = PROBE.get_or_init(|| {
        let output = Command::new(backend)
            .args([
                "--ro-bind",
                "/",
                "/",
                "--dev",
                "/dev",
                "--",
                "/bin/sh",
                "-c",
                ":",
            ])
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    });
    probe.clone().map_err(|error| Error::Inoperable {
        backend: backend.to_path_buf(),
        error,
    })
}

/// Resolves a path that must exist to its canonical form.
fn existing(path: &Path) -> Result<PathBuf, Error> {
    if !path.is_absolute() {
        return Err(Error::Relative(path.to_path_buf()));
    }
    path.canonicalize().map_err(|error| Error::Resolve {
        path: path.to_path_buf(),
        error: error.to_string(),
    })
}

/// A path as a Seatbelt string literal. `"` and `\` are escaped; a path
/// that is not UTF-8 or that carries a control character has no safe
/// spelling and is refused rather than quoted wrong.
fn quoted(path: &Path) -> Result<String, Error> {
    let Some(text) = path.to_str() else {
        return Err(Error::Unsafe(path.to_path_buf()));
    };
    if text.chars().any(char::is_control) {
        return Err(Error::Unsafe(path.to_path_buf()));
    }
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        if c == '"' || c == '\\' {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    Ok(quoted)
}

/// A deny rule for one protected path: the directory itself and
/// everything beneath it. Denies land before the allows, so an allowed
/// path nested under a protected one is the exception and the rest of
/// the protected tree stays denied.
fn deny(path: &Path) -> Result<String, Error> {
    let path = quoted(path)?;
    Ok(format!(
        "(deny file-write* (subpath {path}) (literal {path}))\n"
    ))
}

/// An allow rule for one writable path, after every deny.
fn allow(path: &Path) -> Result<String, Error> {
    let path = quoted(path)?;
    Ok(format!(
        "(allow file-write* (subpath {path}) (literal {path}))\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `Boundary` is `Send`, which is what `run_holding` asks of the
    /// resource it retains until the child is reaped.
    #[test]
    fn the_boundary_and_its_held_half_are_send() {
        fn send<T: Send>() {}
        send::<Boundary>();
        send::<Held>();
    }

    #[test]
    fn quoting_escapes_quotes_and_backslashes() {
        let quoted = quoted(Path::new("/tmp/evil\"dir\\name")).unwrap();
        assert_eq!(quoted, "\"/tmp/evil\\\"dir\\\\name\"");
    }

    #[cfg(unix)]
    #[test]
    fn quoting_refuses_control_characters() {
        assert!(quoted(Path::new("/tmp/evil\nline")).is_err());
    }

    /// A writable path must exist and canonicalize; a missing one is a
    /// refusal rather than a guess at its resolution.
    #[test]
    fn a_missing_writable_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("not").join("there").join("yet");
        let error = Boundary::readonly().writable(&missing).build().unwrap_err();
        assert!(matches!(error, Error::Resolve { .. }), "{error}");
    }

    #[test]
    fn a_writable_path_cannot_escape_through_dotdot() {
        let dir = tempfile::tempdir().unwrap();
        // `missing` does not exist, so the path cannot canonicalize, and
        // nothing walks it to a parent.
        let escape = dir.path().join("missing").join("..").join("other");
        let error = Boundary::readonly().writable(&escape).build().unwrap_err();
        assert!(matches!(error, Error::Resolve { .. }), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_writable_path_resolves_through_a_symlinked_alias() {
        let real = tempfile::tempdir().unwrap();
        std::fs::create_dir(real.path().join("scratch")).unwrap();
        let root = tempfile::tempdir().unwrap();
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(real.path(), &alias).unwrap();
        let resolved = existing(&alias.join("scratch")).unwrap();
        assert_eq!(
            resolved,
            real.path().canonicalize().unwrap().join("scratch")
        );
    }

    #[test]
    fn a_relative_path_is_refused() {
        assert!(matches!(
            existing(Path::new("relative/scratch")),
            Err(Error::Relative(_))
        ));
    }

    #[test]
    fn a_missing_protected_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("absent");
        assert!(matches!(existing(&missing), Err(Error::Resolve { .. })));
    }

    /// The backend path is fixed, and this module's own tests reach the
    /// field directly — there is no public setter, so no caller can
    /// point a "boundary" at a program that is not the platform's backend.
    #[test]
    fn a_missing_backend_is_a_refusal() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = Boundary::readonly().protecting(dir.path());
        spec.backend = PathBuf::from("/nonexistent/backend");
        let error = spec.build().unwrap_err();
        assert!(
            matches!(error, Error::Unavailable(_) | Error::Unsupported(_)),
            "{error}"
        );
    }
}
