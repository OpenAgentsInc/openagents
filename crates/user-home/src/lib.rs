//! The user's home folder, found the same way on every platform.
//!
//! Unix sets `HOME`; Windows normally does not and keeps the user folder in
//! `USERPROFILE`. Reading `HOME` directly therefore breaks every
//! `~/.openagents` path on Windows (Coder 1.0.0-rc.7 said "HOME is not set"
//! as soon as a message was typed). Every crate asks this one instead.
//!
//! The order is:
//!
//! 1. `HOME`, when it is set and not empty, on every platform, because tests
//!    and isolated-state runs point it at a private folder on purpose. On
//!    Windows it must also be an absolute path, so a POSIX-style value left
//!    by an MSYS shell (`/c/Users/me`) does not win over the real folder.
//! 2. [`std::env::home_dir`]: the password database on Unix, `USERPROFILE`
//!    and then the profile API on Windows.
//!
//! Only when neither answers is there no home, and [`NoHome`] says so in
//! words a person can act on.
//!
//! A guard test in this crate (`tests/no_direct_home.rs`) fails when other
//! crates read `HOME` from the environment themselves.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

/// What to tell a person when no home folder can be found.
pub const NO_HOME: &str = "Coder couldn't find your user folder. Set HOME to your user folder \
     (on Windows, USERPROFILE, for example C:\\Users\\you) and try again.";

/// The variables that name the home folder to a child process on this
/// platform: `HOME` everywhere, plus `USERPROFILE` on Windows, where most
/// programs (Node's `os.homedir()`, Rust's `home_dir`) read that instead.
/// A launch that gives a child its own home sets every one of them.
#[cfg(windows)]
pub const VARS: &[&str] = &["HOME", "USERPROFILE"];
/// The variables that name the home folder to a child process on this
/// platform: `HOME` everywhere, plus `USERPROFILE` on Windows.
#[cfg(not(windows))]
pub const VARS: &[&str] = &["HOME"];

/// No home folder could be found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoHome;

impl fmt::Display for NoHome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(NO_HOME)
    }
}

impl std::error::Error for NoHome {}

impl From<NoHome> for std::io::Error {
    fn from(error: NoHome) -> Self {
        std::io::Error::new(std::io::ErrorKind::NotFound, error.to_string())
    }
}

impl From<NoHome> for String {
    fn from(error: NoHome) -> Self {
        error.to_string()
    }
}

/// The user's home folder.
///
/// # Errors
///
/// [`NoHome`] when neither `HOME` nor the operating system names one.
pub fn home_dir() -> Result<PathBuf, NoHome> {
    home().ok_or(NoHome)
}

/// The user's home folder, or `None` when there is none.
#[must_use]
pub fn home() -> Option<PathBuf> {
    explicit(std::env::var_os("HOME"))
        .or_else(std::env::home_dir)
        .filter(|path| !path.as_os_str().is_empty())
}

/// The home folder as the raw value `std::env::var_os("HOME")` would give:
/// a drop-in for code that wants an [`OsString`].
#[must_use]
pub fn home_os() -> Option<OsString> {
    home().map(PathBuf::into_os_string)
}

/// The home folder as `std::env::var("HOME")` would give it: a drop-in for
/// code that wants a [`String`].
///
/// # Errors
///
/// [`std::env::VarError::NotPresent`] when there is no home and
/// [`std::env::VarError::NotUnicode`] when its path is not Unicode.
pub fn home_string() -> Result<String, std::env::VarError> {
    home_os()
        .ok_or(std::env::VarError::NotPresent)?
        .into_string()
        .map_err(std::env::VarError::NotUnicode)
}

/// The home folder named by an injected environment `lookup`: `HOME`, then
/// `USERPROFILE`, for code (and its tests) that reads the environment
/// through a function rather than the process.
#[must_use]
pub fn home_in<S: Into<OsString>>(lookup: impl Fn(&str) -> Option<S>) -> Option<PathBuf> {
    explicit(lookup("HOME").map(Into::into)).or_else(|| {
        lookup("USERPROFILE")
            .map(Into::into)
            .filter(|value: &OsString| !value.is_empty())
            .map(PathBuf::from)
    })
}

/// [`home_in`] as a [`String`], for lookups that speak in strings.
#[must_use]
pub fn home_in_string(lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    home_in(lookup).and_then(|path| path.into_os_string().into_string().ok())
}

/// `HOME` when it is set to something usable on this platform.
fn explicit(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| usable(value)).map(PathBuf::from)
}

fn usable(value: &OsStr) -> bool {
    !value.is_empty() && (cfg!(not(windows)) || Path::new(value).is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    #[test]
    fn home_wins_when_set() {
        let env = lookup(&[("HOME", "/h"), ("USERPROFILE", "/profile")]);
        assert_eq!(home_in(env), Some(PathBuf::from("/h")));
    }

    #[test]
    fn userprofile_answers_when_home_is_missing() {
        let env = lookup(&[("USERPROFILE", r"C:\Users\me")]);
        assert_eq!(home_in(env), Some(PathBuf::from(r"C:\Users\me")));
    }

    #[test]
    fn empty_home_falls_through_to_userprofile() {
        let env = lookup(&[("HOME", ""), ("USERPROFILE", "/profile")]);
        assert_eq!(home_in(env), Some(PathBuf::from("/profile")));
    }

    #[test]
    fn neither_is_none() {
        assert_eq!(home_in(lookup(&[])), None);
        assert_eq!(home_in(lookup(&[("HOME", ""), ("USERPROFILE", "")])), None);
    }

    #[test]
    fn string_lookups_work_too() {
        let env = |name: &str| (name == "USERPROFILE").then(|| String::from("/p"));
        assert_eq!(home_in_string(env), Some("/p".to_owned()));
    }

    #[test]
    fn no_home_speaks_plainly() {
        let text = NoHome.to_string();
        assert!(text.starts_with("Coder couldn't find your user folder."));
        assert!(!text.contains("HOME is not set"));
        let io: std::io::Error = NoHome.into();
        assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn process_home_agrees_with_home_when_set() {
        // The test process runs with HOME set on Unix CI and USERPROFILE on
        // Windows; either way the helper must find a folder.
        assert!(home().is_some());
        assert_eq!(home_os(), home().map(PathBuf::into_os_string));
        if let Some(value) = std::env::var_os("HOME").filter(|value| usable(value)) {
            assert_eq!(home(), Some(PathBuf::from(value)));
        }
    }

    #[cfg(windows)]
    #[test]
    fn msys_style_home_is_ignored_on_windows() {
        let env = lookup(&[("HOME", "/c/Users/me"), ("USERPROFILE", r"C:\Users\me")]);
        assert_eq!(home_in(env), Some(PathBuf::from(r"C:\Users\me")));
    }
}
