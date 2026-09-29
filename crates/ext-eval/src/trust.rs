//! Whether the operator trusts a directory target enough to run it.
//!
//! Running a suite loads the extension's components and runs them as the
//! operator. A directory target the operator hasn't trusted asks once, in
//! plain words, and defaults to no; `--trust` answers yes for scripts; and
//! without a terminal an untrusted directory refuses. A yes is remembered
//! per canonical directory in `<openagents home>/ext-eval/trusted.json`.
//! Trust covers loading and running only: every grant beyond `read` still
//! needs `--grant`. A target that is not owned by the operator, or that
//! others can write, is refused before anyone is asked.

use std::collections::BTreeSet;
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

/// The question asked about an untrusted directory.
pub const QUESTION: &str = "This runs the extension's programs and skills on this computer as you. \
It is not a security check. Trust";

/// Why a target can't run.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TrustError {
    /// The operator doesn't own the path.
    #[error("{0} is not owned by you; an extension eval runs only what you own")]
    NotOwned(String),
    /// Others can write the path.
    #[error("{0} is writable by others; an extension eval refuses it")]
    WritableByOthers(String),
    /// The operator said no, or there was no terminal to ask on.
    #[error("{0} is not trusted; answer yes when asked, or pass --trust")]
    Untrusted(String),
    /// The trust record couldn't be read or written.
    #[error("the trust record {0}: {1}")]
    Store(String, String),
}

/// Refuses a path the operator doesn't own or that others can write.
///
/// # Errors
///
/// Returns [`TrustError::NotOwned`] or [`TrustError::WritableByOthers`].
pub fn check_owner(path: &Path) -> Result<(), TrustError> {
    use std::os::unix::fs::MetadataExt;
    let shown = path.display().to_string();
    let meta = std::fs::metadata(path)
        .map_err(|error| TrustError::Store(shown.clone(), error.to_string()))?;
    // SAFETY: `geteuid` takes nothing and can't fail.
    let me = unsafe { libc::geteuid() };
    if meta.uid() != me {
        return Err(TrustError::NotOwned(shown));
    }
    if meta.mode() & 0o022 != 0 {
        return Err(TrustError::WritableByOthers(shown));
    }
    Ok(())
}

/// The remembered set of trusted directories.
#[derive(Clone, Debug)]
pub struct TrustStore {
    path: PathBuf,
}

impl TrustStore {
    /// The store under `openagents_home`.
    #[must_use]
    pub fn under(openagents_home: &Path) -> Self {
        Self {
            path: openagents_home.join("ext-eval").join("trusted.json"),
        }
    }

    fn read(&self) -> BTreeSet<String> {
        std::fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<BTreeSet<String>>(&bytes).ok())
            .unwrap_or_default()
    }

    /// Whether `dir` is trusted.
    #[must_use]
    pub fn trusts(&self, dir: &Path) -> bool {
        self.read().contains(&dir.display().to_string())
    }

    /// Remembers `dir` as trusted.
    ///
    /// # Errors
    ///
    /// Returns [`TrustError::Store`] when the record can't be written.
    pub fn remember(&self, dir: &Path) -> Result<(), TrustError> {
        let fail = |error: std::io::Error| {
            TrustError::Store(self.path.display().to_string(), error.to_string())
        };
        let mut set = self.read();
        set.insert(dir.display().to_string());
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let bytes = serde_json::to_vec_pretty(&set).unwrap_or_default();
        std::fs::write(&self.path, bytes).map_err(fail)
    }
}

/// How the operator answers the trust question.
pub enum Answer<'a> {
    /// `--trust`: yes, without asking.
    Flag,
    /// Ask on the terminal, if there is one.
    Terminal,
    /// Ask this reader, writing the question to this writer (tests).
    Scripted(&'a mut dyn BufRead, &'a mut dyn Write),
}

/// Decides whether `dir` may run: already trusted, `--trust`, or a yes on
/// the terminal. A yes is remembered.
///
/// # Errors
///
/// Returns [`TrustError::Untrusted`] for a no, an empty answer, or no
/// terminal, and [`TrustError::Store`] when a yes can't be remembered.
pub fn decide(store: &TrustStore, dir: &Path, answer: Answer<'_>) -> Result<(), TrustError> {
    if store.trusts(dir) {
        return Ok(());
    }
    let shown = dir.display().to_string();
    let yes = match answer {
        Answer::Flag => true,
        Answer::Scripted(input, output) => ask(input, output, &shown),
        Answer::Terminal => {
            if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
                return Err(TrustError::Untrusted(shown));
            }
            let stdin = std::io::stdin();
            let mut input = stdin.lock();
            let mut output = std::io::stderr();
            ask(&mut input, &mut output, &shown)
        }
    };
    if !yes {
        return Err(TrustError::Untrusted(shown));
    }
    store.remember(dir)
}

fn ask(input: &mut dyn BufRead, output: &mut dyn Write, shown: &str) -> bool {
    let _ = write!(output, "{QUESTION} {shown}? [y/N] ");
    let _ = output.flush();
    let mut line = String::new();
    if input.read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_yes_is_remembered_and_a_no_refuses() {
        let home = tempfile::tempdir().unwrap();
        let store = TrustStore::under(home.path());
        let dir = Path::new("/ext/one");
        let mut no = std::io::Cursor::new(b"\n".to_vec());
        let mut shown = Vec::new();
        assert!(matches!(
            decide(&store, dir, Answer::Scripted(&mut no, &mut shown)),
            Err(TrustError::Untrusted(_))
        ));
        assert!(String::from_utf8_lossy(&shown).contains("It is not a security check."));
        let mut yes = std::io::Cursor::new(b"y\n".to_vec());
        decide(&store, dir, Answer::Scripted(&mut yes, &mut Vec::new())).unwrap();
        assert!(store.trusts(dir));
        let mut nothing = std::io::Cursor::new(Vec::new());
        decide(&store, dir, Answer::Scripted(&mut nothing, &mut Vec::new())).unwrap();
    }

    #[test]
    fn the_flag_trusts_without_asking() {
        let home = tempfile::tempdir().unwrap();
        let store = TrustStore::under(home.path());
        decide(&store, Path::new("/ext/two"), Answer::Flag).unwrap();
        assert!(store.trusts(Path::new("/ext/two")));
    }

    #[test]
    fn a_directory_others_can_write_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        check_owner(dir.path()).unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(matches!(
            check_owner(dir.path()),
            Err(TrustError::WritableByOthers(_))
        ));
    }
}
