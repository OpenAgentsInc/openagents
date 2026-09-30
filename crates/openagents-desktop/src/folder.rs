//! Choosing a folder: what a chooser answers, the order Linux tries its
//! choosers in, and reading a chooser's answer.
//!
//! Linux has no one folder chooser. The window asks the desktop portal
//! (`org.freedesktop.portal.FileChooser`) first, which any desktop running
//! `xdg-desktop-portal` answers, then `zenity`, then `kdialog`. When none
//! answers, the screen says so rather than doing nothing
//! ([`Chosen::Unavailable`]).

use std::path::PathBuf;

/// What asking for a folder came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chosen {
    /// The person picked this folder.
    Folder(PathBuf),
    /// The person closed the chooser without picking.
    Cancelled,
    /// No chooser opened on this computer.
    Unavailable,
}

/// What the choosers ask with.
pub const PROMPT: &str = "Choose the folder that holds your code";

/// The choosers one computer offers. Each attempt answers `None` when that
/// chooser is not there (not installed, not running, or failed before
/// asking), so the next one is tried; tests stand in for the real ones.
pub trait Choosers {
    /// The desktop portal's chooser.
    fn portal(&mut self) -> Option<Chosen>;
    /// A chooser program, run with `args`.
    fn command(&mut self, program: &str, args: &[String]) -> Option<Chosen>;
}

/// The chooser programs tried after the portal, in order.
pub fn commands(home: &str) -> [(&'static str, Vec<String>); 2] {
    [
        (
            "zenity",
            ["--file-selection", "--directory", "--title", PROMPT]
                .map(String::from)
                .to_vec(),
        ),
        (
            "kdialog",
            ["--getexistingdirectory", home, "--title", PROMPT]
                .map(String::from)
                .to_vec(),
        ),
    ]
}

/// Asks the portal, then each chooser program, and takes the first that
/// answers.
pub fn choose(choosers: &mut dyn Choosers, home: &str) -> Chosen {
    if let Some(chosen) = choosers.portal() {
        return chosen;
    }
    for (program, args) in commands(home) {
        if let Some(chosen) = choosers.command(program, &args) {
            return chosen;
        }
    }
    Chosen::Unavailable
}

/// A chooser program's answer from its exit code and output: `zenity` and
/// `kdialog` exit 0 with the path, or 1 when the person cancels. Any other
/// exit (no display, a crash) is a chooser that did not ask.
pub fn from_command(code: Option<i32>, stdout: &[u8]) -> Option<Chosen> {
    match code {
        Some(0) => {
            let path = std::str::from_utf8(stdout).ok()?;
            let path = path.trim_end_matches(['\n', '\r']).trim_end_matches('/');
            Some(if path.is_empty() {
                Chosen::Cancelled
            } else {
                Chosen::Folder(PathBuf::from(path))
            })
        }
        Some(1) => Some(Chosen::Cancelled),
        _ => None,
    }
}

/// The local path a `file://` URI names, percent-decoded. `None` for
/// another scheme, another machine, or a malformed escape.
pub fn from_uri(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let mut bytes = Vec::with_capacity(rest.len());
    let mut input = rest.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = char::from(input.next()?).to_digit(16)?;
            let low = char::from(input.next()?).to_digit(16)?;
            bytes.push(u8::try_from(high * 16 + low).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    if bytes.contains(&0) {
        return None;
    }
    let path = path_from_bytes(bytes)?;
    let trimmed = path.to_string_lossy();
    if trimmed.len() > 1 && trimmed.ends_with('/') {
        return Some(PathBuf::from(trimmed.trim_end_matches('/')));
    }
    Some(path)
}

#[cfg(unix)]
fn path_from_bytes(bytes: Vec<u8>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(bytes).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_uri_decodes_to_its_path() {
        assert_eq!(
            from_uri("file:///home/kai/code/openagents"),
            Some(PathBuf::from("/home/kai/code/openagents"))
        );
        assert_eq!(
            from_uri("file:///home/kai/My%20Code/caf%C3%A9%2525/"),
            Some(PathBuf::from("/home/kai/My Code/café%25"))
        );
        assert_eq!(
            from_uri("file://localhost/srv/repo"),
            Some(PathBuf::from("/srv/repo"))
        );
        assert_eq!(from_uri("file:///"), Some(PathBuf::from("/")));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_uri_keeps_bytes_that_are_not_utf8() {
        use std::os::unix::ffi::OsStrExt;
        let path = from_uri("file:///home/kai/%FF").expect("a path");
        assert_eq!(path.as_os_str().as_bytes(), b"/home/kai/\xff");
    }

    #[test]
    fn other_uris_are_refused() {
        for uri in [
            "https://example.com/a",
            "file://otherhost/a",
            "file:relative",
            "/home/kai",
            "file:///home/kai/%2",
            "file:///home/kai/%zz",
            "file:///home/%00kai",
        ] {
            assert_eq!(from_uri(uri), None, "{uri}");
        }
    }

    #[test]
    fn a_chooser_programs_exit_says_what_happened() {
        assert_eq!(
            from_command(Some(0), b"/home/kai/code/\n"),
            Some(Chosen::Folder(PathBuf::from("/home/kai/code")))
        );
        assert_eq!(
            from_command(Some(0), b"/home/kai/two  spaces \n"),
            Some(Chosen::Folder(PathBuf::from("/home/kai/two  spaces ")))
        );
        assert_eq!(from_command(Some(1), b""), Some(Chosen::Cancelled));
        assert_eq!(from_command(Some(255), b""), None);
        assert_eq!(from_command(None, b""), None);
    }

    /// Stands in for a computer's choosers, recording what was tried.
    struct Computer {
        portal: Option<Chosen>,
        programs: Vec<(&'static str, Chosen)>,
        tried: Vec<String>,
    }

    impl Choosers for Computer {
        fn portal(&mut self) -> Option<Chosen> {
            self.tried.push("portal".into());
            self.portal.clone()
        }

        fn command(&mut self, program: &str, args: &[String]) -> Option<Chosen> {
            self.tried.push(program.into());
            assert!(args.iter().any(|arg| arg == PROMPT));
            self.programs
                .iter()
                .find(|(name, _)| *name == program)
                .map(|(_, chosen)| chosen.clone())
        }
    }

    fn computer(portal: Option<Chosen>, programs: Vec<(&'static str, Chosen)>) -> Computer {
        Computer {
            portal,
            programs,
            tried: Vec::new(),
        }
    }

    #[test]
    fn the_portal_is_asked_first_then_zenity_then_kdialog() {
        let picked = Chosen::Folder(PathBuf::from("/home/kai/code"));
        let mut portal = computer(
            Some(picked.clone()),
            vec![
                ("zenity", Chosen::Cancelled),
                ("kdialog", Chosen::Cancelled),
            ],
        );
        assert_eq!(choose(&mut portal, "/home/kai"), picked);
        assert_eq!(portal.tried, ["portal"]);

        let mut cancelled = computer(Some(Chosen::Cancelled), vec![]);
        assert_eq!(choose(&mut cancelled, "/home/kai"), Chosen::Cancelled);
        assert_eq!(cancelled.tried, ["portal"]);

        let mut kde = computer(None, vec![("kdialog", picked.clone())]);
        assert_eq!(choose(&mut kde, "/home/kai"), picked);
        assert_eq!(kde.tried, ["portal", "zenity", "kdialog"]);

        let mut gnome = computer(None, vec![("zenity", picked.clone())]);
        assert_eq!(choose(&mut gnome, "/home/kai"), picked);
        assert_eq!(gnome.tried, ["portal", "zenity"]);
    }

    #[test]
    fn with_no_chooser_the_answer_says_so() {
        let mut bare = computer(None, vec![]);
        assert_eq!(choose(&mut bare, "/home/kai"), Chosen::Unavailable);
        assert_eq!(bare.tried, ["portal", "zenity", "kdialog"]);
    }
}
