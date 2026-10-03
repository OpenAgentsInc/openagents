//! The places macOS guards with a privacy prompt, and the means to never
//! touch them.
//!
//! macOS asks the person before a program reads their music, photos,
//! mail, messages, documents, other apps' data, or removable volumes, or
//! sends Apple Events to another app (Transparency, Consent, and Control,
//! "TCC"). The prompt names the *responsible* program, which for every
//! process the Coder host starts is the host itself: a `du` a model ran,
//! a size the disk cleanup rule measured, or a plugin's test all show up
//! as "Coder would like to access Apple Music". Nothing OpenAgents runs
//! has a reason to be in those places, so nothing OpenAgents runs goes
//! there, by construction:
//!
//! - The host's own walks (the background rules' measuring and planning,
//!   session import) skip every path [`is_protected`] names, and the
//!   report-only walks also skip [`private_cache`] folders.
//! - Every command the host runs goes through a Seatbelt profile that
//!   denies those paths and Apple Events: a [`crate::Boundary`] carries
//!   [`rules`] in its own profile, and a full-access command or a whole
//!   coding agent with no boundary runs under [`command`] or [`argv`],
//!   whose profile is [`rules`] and nothing else. A sandbox denial happens
//!   before the privacy check and shows no prompt: the command sees an
//!   ordinary "Operation not permitted" and moves on.
//!
//! Everything here is macOS-specific. On Linux and Windows nothing is
//! protected this way, [`protected`] is empty, and [`command`] and
//! [`argv`] return the command unchanged.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::boundary::SANDBOX_EXEC;

/// The folders under the home that macOS guards with a privacy prompt,
/// relative to the home: the person's files (Desktop, Documents,
/// Downloads, Music, Movies, Pictures), what the system's own apps keep
/// (Mail, Messages, Safari, Calendar, Contacts, Reminders, Photos, Home,
/// call history), iCloud Drive and other cloud storage, and every app's
/// container (macOS asks before one app reads another's data). The
/// caches listed here are the system media and personal apps' own: the
/// Music library's (the "Apple Music" prompt) among them.
pub const PROTECTED_IN_HOME: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Music",
    "Movies",
    "Pictures",
    "Library/Mail",
    "Library/Messages",
    "Library/Safari",
    "Library/Calendars",
    "Library/Reminders",
    "Library/HomeKit",
    "Library/Photos",
    "Library/Cookies",
    "Library/Accounts",
    "Library/Suggestions",
    "Library/IdentityServices",
    "Library/PersonalizationPortrait",
    "Library/Biome",
    "Library/Sharing",
    "Library/Metadata/CoreSpotlight",
    "Library/Application Support/AddressBook",
    "Library/Application Support/CallHistoryDB",
    "Library/Application Support/CallHistoryTransactions",
    "Library/Application Support/com.apple.TCC",
    "Library/Application Support/FaceTime",
    "Library/Application Support/Knowledge",
    "Library/Application Support/MobileSync",
    "Library/Containers",
    "Library/Group Containers",
    "Library/Mobile Documents",
    "Library/CloudStorage",
    "Library/Caches/CloudKit",
    "Library/Caches/com.apple.AMPLibraryAgent",
    "Library/Caches/com.apple.AppleMediaServices",
    "Library/Caches/com.apple.HomeKit",
    "Library/Caches/com.apple.Messages",
    "Library/Caches/com.apple.Music",
    "Library/Caches/com.apple.Photos",
    "Library/Caches/com.apple.Safari",
    "Library/Caches/com.apple.TV",
    "Library/Caches/com.apple.iTunes",
    "Library/Caches/com.apple.iTunesCloud",
    "Library/Caches/com.apple.itunescloudd",
    "Library/Caches/com.apple.mail",
    "Library/Caches/com.apple.podcasts",
];

/// Absolute places macOS guards with a privacy prompt: removable and
/// network volumes. The startup disk's own entry there is a link to `/`,
/// which a profile matches by its resolved path, so it stays reachable.
pub const PROTECTED_ROOTS: &[&str] = &["/Volumes"];

/// Whether this platform has privacy-protected places at all.
pub const APPLIES: bool = cfg!(target_os = "macos");

/// The protected places for `home`, absolute. Empty off macOS.
#[must_use]
pub fn protected(home: &Path) -> Vec<PathBuf> {
    if !APPLIES {
        return Vec::new();
    }
    PROTECTED_IN_HOME
        .iter()
        .map(|relative| home.join(relative))
        .chain(PROTECTED_ROOTS.iter().map(PathBuf::from))
        .collect()
}

/// Whether `path` is a protected place or lies beneath one, for `home`.
/// A walk that meets such a path does not enter it, read it, or measure
/// it. Always false off macOS.
#[must_use]
pub fn is_protected(path: &Path, home: &Path) -> bool {
    APPLIES
        && protected(home)
            .iter()
            .any(|protected| path.starts_with(protected))
}

/// Whether a walk that only reports sizes should leave `path` out: a
/// protected place, or a folder named for one of Apple's own programs
/// (`com.apple.…`, `CloudKit`), whose caches may hold an app's private
/// data. Nothing reported is ever deleted, so a size left out costs
/// nothing but the number. Always false off macOS.
#[must_use]
pub fn private_cache(path: &Path, home: &Path) -> bool {
    if !APPLIES {
        return false;
    }
    if is_protected(path, home) {
        return true;
    }
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.starts_with("com.apple.") || name == "CloudKit")
}

/// This account's home, from the account database rather than `HOME`, so
/// a command whose `HOME` is a scratch directory still has the person's
/// real folders denied. `HOME` when the database has no answer.
#[must_use]
pub fn home() -> Option<PathBuf> {
    crate::toolchains::account_home()
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .filter(|home| home.is_absolute())
        .map(|home| home.canonicalize().unwrap_or(home))
}

/// The Seatbelt rules that keep a command out of every protected place
/// for `home` and stop it sending Apple Events or opening the camera or
/// microphone. Each protected folder stays visible as a name (its own
/// metadata is readable, so `ls ~` still works), but nothing in it can
/// be listed, read, or written. `allowed` are paths the command is
/// meant to use even though they lie in a protected place, such as a
/// checkout a person keeps in Documents; they are allowed back after the
/// denies. Paths without a safe Seatbelt spelling are left out of
/// `allowed` and named in no rule. Empty off macOS.
///
/// The rules come last in a profile, so they win over every earlier
/// allow, and their own allows win over them.
#[must_use]
pub fn rules(home: &Path, allowed: &[&Path]) -> String {
    if !APPLIES {
        return String::new();
    }
    let mut rules =
        String::from(";; macOS privacy: never prompt (crates/coder-boundary/src/privacy.rs)\n");
    for path in protected(home) {
        let Some(path) = quoted(&path) else {
            continue;
        };
        rules.push_str(&format!(
            "(deny file-read* file-write* (subpath {path}))\n\
             (allow file-read-metadata (literal {path}))\n"
        ));
    }
    for path in allowed {
        if !is_protected(path, home) {
            continue;
        }
        let Some(path) = quoted(path) else {
            continue;
        };
        rules.push_str(&format!(
            "(allow file-read* file-write* (subpath {path}))\n"
        ));
    }
    rules.push_str("(deny appleevent-send)\n(deny device-camera)\n(deny device-microphone)\n");
    rules
}

/// The system's set-user-ID programs. A sandboxed process may not start
/// one (the kernel refuses to raise a sandboxed process's privileges), so
/// under full access, where `ps`, `top`, and the rest must work as they
/// do for the owner, the privacy-only profile starts them outside itself.
/// None of them reads a protected place on its own.
pub const SETUID_PROGRAMS: &[&str] = &[
    "/bin/ps",
    "/usr/bin/top",
    "/usr/bin/at",
    "/usr/bin/atq",
    "/usr/bin/atrm",
    "/usr/bin/batch",
    "/usr/bin/crontab",
    "/usr/bin/login",
    "/usr/bin/newgrp",
    "/usr/bin/quota",
    "/usr/bin/su",
    "/usr/bin/sudo",
    "/usr/sbin/traceroute",
    "/usr/sbin/traceroute6",
    "/usr/libexec/authopen",
    "/usr/libexec/security_authtrampoline",
];

/// The whole profile of a privacy-only sandbox: everything allowed but
/// [`rules`]. A command under it may still start a [`crate::Boundary`]
/// of its own: `sandbox-exec` cannot apply a profile inside another, so
/// the system's `sandbox-exec` is started outside this one, and every
/// boundary carries [`rules`] in its own profile. The system's
/// set-user-ID programs ([`SETUID_PROGRAMS`]) start outside it too.
#[must_use]
pub fn profile(home: &Path, allowed: &[&Path]) -> String {
    profile_with(Some(home), allowed, "")
}

/// [`profile`] with `before` (such as a [`crate::source::Guard`]'s
/// rules) ahead of the privacy rules, for `home` when there is one.
pub(crate) fn profile_with(home: Option<&Path>, allowed: &[&Path], before: &str) -> String {
    let privacy = home.map(|home| rules(home, allowed)).unwrap_or_default();
    let mut profile = format!("(version 1)\n(allow default)\n{before}{privacy}");
    for program in std::iter::once(SANDBOX_EXEC).chain(SETUID_PROGRAMS.iter().copied()) {
        profile.push_str(&format!(
            "(allow process-exec (literal \"{program}\") (with no-sandbox))\n"
        ));
    }
    profile
}

/// `program` as a command that runs under [`profile`] for this account's
/// home, with `allowed` allowed back: `sandbox-exec -p <profile>
/// <program>`, to which the caller adds the arguments, environment, and
/// working directory as for `program` itself. `program` itself, off
/// macOS, when `sandbox-exec` is missing, when there is no home, or when
/// this process already runs in a sandbox (a profile can't be applied
/// inside one, and the outer one holds these rules already).
#[must_use]
pub fn command(program: impl AsRef<OsStr>, allowed: &[&Path]) -> Command {
    let program = program.as_ref();
    match wrapper(allowed) {
        Some(prefix) => {
            let mut command = Command::new(SANDBOX_EXEC);
            command.args(prefix).arg(program);
            command
        }
        None => Command::new(program),
    }
}

/// [`command`] as a program and its arguments, for a caller that takes
/// them apart, such as an agent spawned over its standard streams.
/// Unchanged when [`command`] would leave it unchanged, or when
/// `program` is already `sandbox-exec`.
#[must_use]
pub fn argv(program: PathBuf, arguments: Vec<String>, allowed: &[&Path]) -> (PathBuf, Vec<String>) {
    if program == Path::new(SANDBOX_EXEC) {
        return (program, arguments);
    }
    let Some(prefix) = wrapper(allowed) else {
        return (program, arguments);
    };
    let mut wrapped: Vec<String> = prefix
        .into_iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    wrapped.push(program.to_string_lossy().into_owned());
    wrapped.extend(arguments);
    (PathBuf::from(SANDBOX_EXEC), wrapped)
}

/// `-p <profile>` for this account, or `None` when nothing should wrap.
fn wrapper(allowed: &[&Path]) -> Option<Vec<OsString>> {
    if !APPLIES || sandboxed() || !Path::new(SANDBOX_EXEC).is_file() {
        return None;
    }
    let home = home()?;
    Some(vec!["-p".into(), profile(&home, allowed).into()])
}

/// Whether this process already runs inside a Seatbelt sandbox.
#[cfg(target_os = "macos")]
#[must_use]
pub fn sandboxed() -> bool {
    unsafe extern "C" {
        fn sandbox_check(
            pid: libc::pid_t,
            operation: *const libc::c_char,
            kind: libc::c_int,
            ...
        ) -> libc::c_int;
    }
    // SAFETY: with no operation and SANDBOX_FILTER_NONE (0), sandbox_check
    // reads no further argument and only reports whether `pid` is
    // sandboxed.
    unsafe { sandbox_check(libc::getpid(), std::ptr::null(), 0) != 0 }
}

/// Whether this process already runs inside a Seatbelt sandbox: never,
/// off macOS.
#[cfg(not(target_os = "macos"))]
#[must_use]
pub fn sandboxed() -> bool {
    false
}

/// A path as a Seatbelt string literal, or `None` when it has no safe
/// spelling (not UTF-8, or a control character).
fn quoted(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    if text.chars().any(char::is_control) {
        return None;
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
    Some(quoted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_places_are_macos_only_and_cover_what_was_prompted() {
        let home = Path::new("/Users/someone");
        if !APPLIES {
            assert!(protected(home).is_empty());
            assert!(!is_protected(&home.join("Music"), home));
            assert!(rules(home, &[]).is_empty());
            return;
        }
        for inside in [
            "Music",
            "Music/Music/Media.localized",
            "Pictures/Photos Library.photoslibrary",
            "Library/Caches/com.apple.Music/x",
            "Library/Containers/com.other.app/Data",
            "Library/Mobile Documents/com~apple~CloudDocs",
        ] {
            assert!(is_protected(&home.join(inside), home), "{inside}");
        }
        assert!(is_protected(Path::new("/Volumes/USB"), home));
        for outside in [
            "",
            "work/openagents",
            ".openagents/targets",
            ".cargo",
            "Library/Caches",
            "Library/Caches/Homebrew",
            "Library/Developer",
            "Musical",
        ] {
            assert!(!is_protected(&home.join(outside), home), "{outside}");
        }
    }

    #[test]
    fn report_walks_also_skip_apple_caches() {
        let home = Path::new("/Users/someone");
        let caches = home.join("Library/Caches");
        assert_eq!(
            private_cache(&caches.join("com.apple.Safari"), home),
            APPLIES
        );
        assert_eq!(
            private_cache(&caches.join("com.apple.anything"), home),
            APPLIES
        );
        assert_eq!(
            private_cache(Path::new("/private/var/folders/x/y/C/com.apple.mail"), home),
            APPLIES
        );
        assert!(!private_cache(&caches.join("Homebrew"), home));
        assert!(!private_cache(&caches, home));
    }

    #[test]
    fn rules_deny_then_allow_back_what_the_command_needs() {
        if !APPLIES {
            return;
        }
        let home = Path::new("/Users/someone");
        let checkout = home.join("Documents/project");
        let elsewhere = home.join("work/project");
        let rules = rules(home, &[checkout.as_path(), elsewhere.as_path()]);
        let deny = rules
            .find("(deny file-read* file-write* (subpath \"/Users/someone/Documents\"))")
            .unwrap();
        let allow = rules
            .find("(allow file-read* file-write* (subpath \"/Users/someone/Documents/project\"))")
            .unwrap();
        assert!(deny < allow, "{rules}");
        assert!(!rules.contains("work/project"), "{rules}");
        assert!(rules.contains("(allow file-read-metadata (literal \"/Users/someone/Music\"))"));
        assert!(rules.contains("(deny appleevent-send)"));
        let profile = profile(home, &[]);
        assert!(
            profile.starts_with("(version 1)\n(allow default)\n"),
            "{profile}"
        );
        assert!(
            profile.contains(&format!("(literal \"{SANDBOX_EXEC}\") (with no-sandbox)")),
            "{profile}"
        );
        assert!(
            profile.contains("(literal \"/bin/ps\") (with no-sandbox)"),
            "{profile}"
        );
    }

    #[test]
    fn argv_wraps_once() {
        let (program, arguments) = argv(
            PathBuf::from(SANDBOX_EXEC),
            vec!["-f".into(), "x".into()],
            &[],
        );
        assert_eq!(program, Path::new(SANDBOX_EXEC));
        assert_eq!(arguments, ["-f", "x"]);
        let (program, arguments) = argv(PathBuf::from("/bin/echo"), vec!["hi".into()], &[]);
        if APPLIES && !sandboxed() && Path::new(SANDBOX_EXEC).is_file() {
            assert_eq!(program, Path::new(SANDBOX_EXEC));
            assert_eq!(arguments[0], "-p");
            assert_eq!(&arguments[2..], ["/bin/echo", "hi"]);
        } else {
            assert_eq!(program, Path::new("/bin/echo"));
        }
    }
}
