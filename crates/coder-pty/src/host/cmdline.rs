//! The two strings `CreateProcessW` takes in place of an argument vector
//! and an environment: a command line quoted the way the Microsoft C
//! runtime splits it, and a sorted, NUL-separated environment block.
//!
//! Pure text, compiled on every platform so its tests run everywhere; only
//! the Windows terminal half (`windows.rs`) uses it.

#![cfg_attr(not(windows), allow(dead_code))]

use std::ffi::{OsStr, OsString};

/// Appends `arg` to `line`, quoted so the C runtime's `CommandLineToArgvW`
/// rules give it back unchanged: an argument with no space, tab, newline,
/// vertical tab, or double quote (and not empty) goes as it is; any other
/// is wrapped in double quotes, with each `"` escaped and the backslashes
/// before a `"` or the closing quote doubled.
pub(super) fn push_arg(line: &mut String, arg: &str) {
    if !line.is_empty() {
        line.push(' ');
    }
    let plain = !arg.is_empty()
        && !arg
            .chars()
            .any(|c| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '"'));
    if plain {
        line.push_str(arg);
        return;
    }
    line.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                line.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                line.push('"');
                backslashes = 0;
            }
            _ => {
                line.extend(std::iter::repeat_n('\\', backslashes));
                line.push(c);
                backslashes = 0;
            }
        }
    }
    line.extend(std::iter::repeat_n('\\', backslashes * 2));
    line.push('"');
}

/// The command line for `program` and `args`. The program is the first
/// word, quoted when it has a space; a program containing a double quote
/// cannot be named on a command line and is refused.
pub(super) fn command_line(program: &OsStr, args: &[&OsStr]) -> Result<String, String> {
    let program = program.to_string_lossy();
    if program.contains('"') || program.is_empty() {
        return Err("the program's name cannot be put on a command line".into());
    }
    let mut line = String::new();
    if program.contains([' ', '\t']) {
        line.push('"');
        line.push_str(&program);
        line.push('"');
    } else {
        line.push_str(&program);
    }
    for arg in args {
        push_arg(&mut line, &arg.to_string_lossy());
    }
    Ok(line)
}

/// The environment block for `vars`: `NAME=VALUE` entries sorted by name
/// without regard to case, as Windows expects, each ending in a NUL, and
/// one more NUL at the end. A name that is empty, or holds `=` past its
/// first character or a NUL, is refused, as is a value holding a NUL.
pub(super) fn environment_block(vars: &[(OsString, OsString)]) -> Result<Vec<u16>, String> {
    let mut entries: Vec<(String, String)> = Vec::with_capacity(vars.len());
    for (name, value) in vars {
        let name = name.to_string_lossy().into_owned();
        let value = value.to_string_lossy().into_owned();
        if name.is_empty() || name.chars().skip(1).any(|c| c == '=') || name.contains('\0') {
            return Err(format!("the variable name {name:?} cannot be set"));
        }
        if value.contains('\0') {
            return Err(format!("the value of {name} holds a NUL"));
        }
        // A later setting of the same name wins, as with a `Command`.
        entries.retain(|(held, _)| !held.eq_ignore_ascii_case(&name));
        entries.push((name, value));
    }
    entries.sort_by_key(|(name, _)| name.to_uppercase());
    let mut block = Vec::new();
    for (name, value) in &entries {
        block.extend(format!("{name}={value}").encode_utf16());
        block.push(0);
    }
    if entries.is_empty() {
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

/// `path` without the `\\?\` prefix `canonicalize` adds to a drive path,
/// since a program started in `\\?\C:\...` sees a working directory that
/// `cmd.exe` refuses. A UNC or device path keeps its form.
pub(super) fn plain_directory(path: &str) -> &str {
    match path.strip_prefix(r"\\?\") {
        Some(rest)
            if rest.len() >= 3
                && rest.as_bytes()[0].is_ascii_alphabetic()
                && &rest[1..3] == r":\" =>
        {
            rest
        }
        _ => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(args: &[&str]) -> String {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        command_line(OsStr::new(r"C:\Windows\System32\cmd.exe"), &args).unwrap()
    }

    #[test]
    fn plain_arguments_go_as_they_are() {
        assert_eq!(
            line(&["/c", "echo"]),
            r"C:\Windows\System32\cmd.exe /c echo"
        );
    }

    #[test]
    fn spaces_quotes_and_backslashes_survive_the_c_runtime() {
        assert_eq!(line(&["a b"]).split_once(' ').unwrap().1, r#""a b""#);
        assert_eq!(line(&[""]).split_once(' ').unwrap().1, r#""""#);
        assert_eq!(
            line(&[r#"say "hi""#]).split_once(' ').unwrap().1,
            r#""say \"hi\"""#
        );
        // Backslashes are literal unless a quote follows them.
        assert_eq!(
            line(&[r"C:\a b\"]).split_once(' ').unwrap().1,
            r#""C:\a b\\""#
        );
        assert_eq!(line(&[r#"a\"b"#]).split_once(' ').unwrap().1, r#""a\\\"b""#);
        assert_eq!(
            line(&[r"C:\dir\file"]).split_once(' ').unwrap().1,
            r"C:\dir\file"
        );
    }

    #[test]
    fn a_program_with_a_space_is_quoted_and_one_with_a_quote_refused() {
        let spaced = command_line(OsStr::new(r"C:\Program Files\x.exe"), &[]).unwrap();
        assert_eq!(spaced, r#""C:\Program Files\x.exe""#);
        assert!(command_line(OsStr::new(r#"C:\a"b.exe"#), &[]).is_err());
        assert!(command_line(OsStr::new(""), &[]).is_err());
    }

    fn text(block: &[u16]) -> String {
        String::from_utf16(block).unwrap()
    }

    #[test]
    fn the_environment_block_is_sorted_nul_separated_and_ends_twice() {
        let vars = [
            (OsString::from("TERM"), OsString::from("xterm-256color")),
            (OsString::from("Path"), OsString::from(r"C:\Windows")),
            (OsString::from("path"), OsString::from(r"C:\bin")),
            (OsString::from("=C:"), OsString::from(r"C:\")),
            (OsString::from("a"), OsString::from("1")),
        ];
        let block = environment_block(&vars).unwrap();
        assert_eq!(
            text(&block),
            "=C:=C:\\\0a=1\0path=C:\\bin\0TERM=xterm-256color\0\0"
        );
        assert_eq!(text(&environment_block(&[]).unwrap()), "\0\0");
    }

    #[test]
    fn a_malformed_variable_is_refused() {
        for (name, value) in [("", "x"), ("A=B", "x"), ("A\0", "x"), ("A", "x\0y")] {
            let vars = [(OsString::from(name), OsString::from(value))];
            assert!(environment_block(&vars).is_err(), "{name:?}");
        }
    }

    #[test]
    fn a_verbatim_drive_path_loses_its_prefix_and_a_unc_path_keeps_it() {
        assert_eq!(plain_directory(r"\\?\C:\work\repo"), r"C:\work\repo");
        assert_eq!(plain_directory(r"C:\work"), r"C:\work");
        assert_eq!(
            plain_directory(r"\\?\UNC\server\share"),
            r"\\?\UNC\server\share"
        );
    }
}
