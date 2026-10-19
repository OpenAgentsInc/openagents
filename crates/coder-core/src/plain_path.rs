//! Canonical paths in the form Git and other tools accept (#11691).
//!
//! On Windows `canonicalize()` answers verbatim paths (`\\?\C:\repo`,
//! `\\?\UNC\server\share\repo`). Git refuses them — `git worktree add`
//! cannot create leading directories under `//?/C:/...` — and they compare
//! unequal to the plain paths Git itself prints. [`canonical`] resolves a
//! path like `canonicalize()` and then drops the verbatim prefix when the
//! plain form names the same file, as `dunce::simplified` does. On Unix it
//! is `canonicalize()` unchanged.

use std::io;
use std::path::{Path, PathBuf};

/// The longest path Windows accepts without the verbatim prefix.
const MAX_PLAIN: usize = 260;

/// `path`, resolved like [`Path::canonicalize`], but never verbatim when a
/// plain path names the same file. Use it wherever a canonical path is
/// compared with one Git prints or handed to Git.
///
/// # Errors
/// As [`Path::canonicalize`].
pub fn canonical(path: &Path) -> io::Result<PathBuf> {
    path.canonicalize().map(|path| simplified(&path))
}

/// `path` without the Windows verbatim prefix, when the plain path names
/// the same file; otherwise `path` unchanged. Identity off Windows.
#[must_use]
pub fn simplified(path: &Path) -> PathBuf {
    if cfg!(windows)
        && let Some(text) = path.to_str()
        && let Some(plain) = plain_windows(text)
    {
        return PathBuf::from(plain);
    }
    path.to_path_buf()
}

/// The plain form of a verbatim Windows path, or `None` when `text` is
/// not verbatim or its plain form would name something else (a reserved
/// device name, a trailing dot, a `..`, a path too long, ...).
fn plain_windows(text: &str) -> Option<String> {
    let rest = text.strip_prefix(r"\\?\")?;
    let (plain, components) = if let Some(unc) = rest
        .strip_prefix(r"UNC\")
        .or_else(|| rest.strip_prefix(r"unc\"))
    {
        let mut parts = unc.splitn(3, '\\');
        let server = parts.next().filter(|part| usable(part))?;
        let share = parts.next().filter(|part| usable(part))?;
        let tail = parts.next().unwrap_or("");
        (format!(r"\\{server}\{share}\{tail}"), tail)
    } else {
        let bytes = rest.as_bytes();
        if bytes.len() < 2 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
            return None;
        }
        let tail = &rest[2..];
        let tail = match tail.strip_prefix('\\') {
            Some(tail) => tail,
            None if tail.is_empty() => tail,
            None => return None,
        };
        (format!(r"{}:\{tail}", &rest[..1]), tail)
    };
    if plain.len() >= MAX_PLAIN {
        return None;
    }
    let tail = components.strip_suffix('\\').unwrap_or(components);
    if !tail.is_empty() && !tail.split('\\').all(usable) {
        return None;
    }
    Some(plain)
}

/// Whether one component means the same thing without the verbatim prefix.
fn usable(component: &str) -> bool {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.ends_with('.')
        || component.ends_with(' ')
        || component
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return false;
    }
    let stem = component.split('.').next().unwrap_or(component).trim_end();
    let upper = stem.to_ascii_uppercase();
    let reserved = matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ((upper.starts_with("COM") || upper.starts_with("LPT"))
        && upper.len() == 4
        && upper.as_bytes()[3].is_ascii_digit());
    !reserved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verbatim_drive_path_comes_back_plain() {
        assert_eq!(
            plain_windows(r"\\?\C:\Users\me\repo\.git").as_deref(),
            Some(r"C:\Users\me\repo\.git")
        );
        assert_eq!(plain_windows(r"\\?\d:\").as_deref(), Some(r"d:\"));
        assert_eq!(plain_windows(r"\\?\C:").as_deref(), Some(r"C:\"));
    }

    #[test]
    fn a_verbatim_unc_path_comes_back_as_a_valid_unc_path() {
        assert_eq!(
            plain_windows(r"\\?\UNC\server\share\repo").as_deref(),
            Some(r"\\server\share\repo")
        );
        assert_eq!(
            plain_windows(r"\\?\UNC\server\share").as_deref(),
            Some(r"\\server\share\")
        );
        // A plain UNC path is already valid and stays as it is.
        assert_eq!(plain_windows(r"\\server\share\repo"), None);
        assert_eq!(
            simplified(Path::new(r"\\server\share\repo")),
            PathBuf::from(r"\\server\share\repo")
        );
    }

    #[test]
    fn a_verbatim_path_whose_plain_form_means_something_else_stays_verbatim() {
        for text in [
            r"\\?\C:\repo\CON",
            r"\\?\C:\repo\nul.txt",
            r"\\?\C:\repo\com1",
            r"\\?\C:\repo\trailing.",
            r"\\?\C:\repo\trailing ",
            r"\\?\C:\repo\..\x",
            r"\\?\C:\repo\a/b",
            r"\\?\C:\repo\\x",
            r"\\?\Volume{0000}\repo",
            r"\\?\GLOBALROOT\Device",
        ] {
            assert_eq!(plain_windows(text), None, "{text}");
        }
        let long = format!(r"\\?\C:\{}", "a".repeat(MAX_PLAIN));
        assert_eq!(plain_windows(&long), None);
    }

    #[test]
    fn plain_paths_are_untouched() {
        assert_eq!(plain_windows(r"C:\repo"), None);
        assert_eq!(plain_windows("/tmp/repo"), None);
        assert_eq!(simplified(Path::new("/tmp/repo")), Path::new("/tmp/repo"));
    }

    #[cfg(windows)]
    #[test]
    fn canonical_is_never_verbatim_on_windows() {
        let directory = tempfile::tempdir().unwrap();
        let resolved = canonical(directory.path()).unwrap();
        assert!(
            !resolved.to_string_lossy().starts_with(r"\\?\"),
            "{resolved:?}"
        );
        assert!(resolved.is_absolute());
    }

    #[cfg(unix)]
    #[test]
    fn canonical_is_canonicalize_on_unix() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            canonical(directory.path()).unwrap(),
            directory.path().canonicalize().unwrap()
        );
    }
}
