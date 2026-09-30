//! Windows command lines, and the plain spelling of a Windows path.
//!
//! A Windows program gets one command line and splits it itself. The
//! boundary's launcher builds that line for the program it starts, by the
//! convention the Microsoft C runtime and Rust's standard library read:
//! an argument is quoted when it is empty or holds whitespace or a quote,
//! backslashes are literal except before a quote, where they are doubled,
//! and a quote is escaped with a backslash. An argument holding a Cygwin
//! wildcard or quote character (`*?[]{}'`) is quoted too, so a Cygwin or
//! MSYS program (Git for Windows' `bash`) never expands one. A plain word
//! such as `/c` stays bare, which `cmd.exe` needs to read it as a switch.
//!
//! These are pure functions over UTF-16 units, so they are tested on every
//! platform.

use std::path::{Path, PathBuf};

const QUOTE: u16 = b'"' as u16;
const BACKSLASH: u16 = b'\\' as u16;
const SPACE: u16 = b' ' as u16;

/// Whether `argument` must be quoted to stay one word that nothing
/// expands.
fn needs_quotes(argument: &[u16]) -> bool {
    argument.is_empty()
        || argument.iter().any(|&unit| {
            u8::try_from(unit).is_ok_and(|byte| {
                matches!(
                    byte,
                    b' ' | b'\t'
                        | b'\n'
                        | 0x0b
                        | b'"'
                        | b'*'
                        | b'?'
                        | b'['
                        | b']'
                        | b'{'
                        | b'}'
                        | b'\''
                )
            })
        })
}

/// Appends `argument` to `line`, quoted when it needs to be.
pub fn quote_into(line: &mut Vec<u16>, argument: &[u16]) {
    if !needs_quotes(argument) {
        line.extend_from_slice(argument);
        return;
    }
    line.push(QUOTE);
    let mut backslashes = 0usize;
    for &unit in argument {
        match unit {
            BACKSLASH => backslashes += 1,
            QUOTE => {
                // Every backslash before a quote is doubled, and the quote
                // itself escaped.
                line.extend(std::iter::repeat_n(BACKSLASH, backslashes * 2 + 1));
                line.push(QUOTE);
                backslashes = 0;
            }
            _ => {
                line.extend(std::iter::repeat_n(BACKSLASH, backslashes));
                line.push(unit);
                backslashes = 0;
            }
        }
    }
    // Backslashes before the closing quote are doubled so it stays a quote.
    line.extend(std::iter::repeat_n(BACKSLASH, backslashes * 2));
    line.push(QUOTE);
}

/// The command line for `program` and `arguments`, each quoted as it
/// needs to be, joined by a space. The program is always quoted, so a
/// path with spaces stays one word.
pub fn command_line<P: AsRef<[u16]>>(program: &[u16], arguments: &[P]) -> Vec<u16> {
    let mut line = vec![QUOTE];
    line.extend_from_slice(program);
    line.push(QUOTE);
    for argument in arguments {
        line.push(SPACE);
        quote_into(&mut line, argument.as_ref());
    }
    line
}

/// The arguments the Microsoft C runtime reads from `line`: the inverse of
/// [`command_line`] for the arguments after the program. Used by the tests
/// to check the quoting round-trips.
pub fn split(line: &[u16]) -> Vec<Vec<u16>> {
    let mut arguments = Vec::new();
    let mut current = Vec::new();
    let mut in_word = false;
    let mut quoted = false;
    let mut index = 0;
    while index < line.len() {
        let unit = line[index];
        if unit == BACKSLASH {
            let start = index;
            while index < line.len() && line[index] == BACKSLASH {
                index += 1;
            }
            let count = index - start;
            if index < line.len() && line[index] == QUOTE {
                current.extend(std::iter::repeat_n(BACKSLASH, count / 2));
                if count % 2 == 1 {
                    current.push(QUOTE);
                    index += 1;
                }
            } else {
                current.extend(std::iter::repeat_n(BACKSLASH, count));
            }
            in_word = true;
            continue;
        }
        if unit == QUOTE {
            quoted = !quoted;
            in_word = true;
        } else if (unit == SPACE || unit == u16::from(b'\t')) && !quoted {
            if in_word {
                arguments.push(std::mem::take(&mut current));
                in_word = false;
            }
        } else {
            current.push(unit);
            in_word = true;
        }
        index += 1;
    }
    if in_word {
        arguments.push(current);
    }
    arguments
}

/// `path` without its `\\?\` prefix when the plain spelling names the same
/// file: a drive path under the classic length limit whose components are
/// free of characters and names Windows would reinterpret. Other paths,
/// and every path off Windows, come back unchanged.
///
/// `std::fs::canonicalize` answers in the verbatim form, which some
/// programs refuse: `cmd.exe` will not start in one, and Git records it in
/// a worktree's links.
#[must_use]
pub fn plain_path(path: &Path) -> PathBuf {
    match path.to_str().and_then(plain) {
        Some(plain) => PathBuf::from(plain),
        None => path.to_path_buf(),
    }
}

/// The plain spelling of a verbatim drive path, or `None`.
fn plain(text: &str) -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    let rest = text.strip_prefix(r"\\?\")?;
    let bytes = rest.as_bytes();
    if bytes.len() < 3
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || bytes[2] != b'\\'
        || rest.len() >= 260
    {
        return None;
    }
    for component in rest[3..].split('\\') {
        if !plain_component(component) {
            // An empty component is allowed only as a trailing separator.
            if component.is_empty() && rest.ends_with('\\') {
                continue;
            }
            return None;
        }
    }
    Some(rest.to_owned())
}

/// Whether one path component means the same thing without the verbatim
/// prefix.
fn plain_component(component: &str) -> bool {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.ends_with('.')
        || component.ends_with(' ')
        || component
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '|' | '?' | '*'))
    {
        return false;
    }
    let stem = component.split('.').next().unwrap_or_default().trim_end();
    !RESERVED.iter().any(|name| stem.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn text(units: &[u16]) -> String {
        String::from_utf16(units).unwrap()
    }

    #[test]
    fn every_argument_round_trips() {
        let arguments = [
            "",
            "plain",
            "two words",
            r#"say "hi""#,
            r"C:\dir\",
            r#"back\"slash"#,
            r"a\\b",
            "*.rs",
            "tab\there",
            r#"c=$X; unset X; eval "$c""#,
        ];
        let line = command_line(
            &wide(r"C:\Program Files\Git\usr\bin\bash.exe"),
            &arguments.map(wide),
        );
        let split = split(&line);
        assert_eq!(text(&split[0]), r"C:\Program Files\Git\usr\bin\bash.exe");
        for (index, argument) in arguments.iter().enumerate() {
            assert_eq!(text(&split[index + 1]), *argument, "{}", text(&line));
        }
    }

    #[test]
    fn quoting_spells_what_the_c_runtime_reads() {
        let mut line = Vec::new();
        quote_into(&mut line, &wide(r#"a\"b\"#));
        assert_eq!(text(&line), r#""a\\\"b\\""#);
        let mut line = Vec::new();
        quote_into(&mut line, &wide("*"));
        assert_eq!(text(&line), "\"*\"");
        let line = command_line(
            &wide(r"C:\Windows\System32\cmd.exe"),
            &[wide("/d"), wide("/c"), wide("echo x> out")],
        );
        assert_eq!(
            text(&line),
            r#""C:\Windows\System32\cmd.exe" /d /c "echo x> out""#
        );
    }

    /// A quoted argument that holds no backslash reads the same to the C
    /// runtime and to Cygwin, which escapes a quote with a backslash and
    /// keeps every other backslash: the script runner's fixed argument is
    /// one.
    #[test]
    fn a_backslash_free_argument_needs_only_escaped_quotes() {
        let mut line = Vec::new();
        quote_into(&mut line, &wide(r#"eval "$c""#));
        assert_eq!(text(&line), r#""eval \"$c\"""#);
    }

    #[cfg(windows)]
    #[test]
    fn a_verbatim_drive_path_is_spelled_plainly_when_that_is_the_same_path() {
        assert_eq!(
            plain_path(Path::new(r"\\?\C:\Users\me\repo")),
            PathBuf::from(r"C:\Users\me\repo")
        );
        for kept in [
            r"\\?\C:\Users\me\CON",
            r"\\?\C:\Users\me\trailing.",
            r"\\?\UNC\server\share\x",
            r"\\?\C:\a\nul.txt",
        ] {
            assert_eq!(plain_path(Path::new(kept)), PathBuf::from(kept), "{kept}");
        }
        assert_eq!(
            plain_path(Path::new(r"C:\plain")),
            PathBuf::from(r"C:\plain")
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_a_path_is_unchanged() {
        assert_eq!(
            plain_path(Path::new(r"\\?\C:\x")),
            PathBuf::from(r"\\?\C:\x")
        );
    }
}
