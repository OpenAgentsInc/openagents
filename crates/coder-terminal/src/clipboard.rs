//! Copy through the terminal's system clipboard, including over SSH and tmux.
//!
//! As grok-build does it, a copy writes every leg that can reach the
//! person's clipboard: OSC 52 to the terminal (the only leg that crosses
//! SSH), tmux's paste buffer inside tmux, and on this machine the native
//! tool (`pbcopy`, `wl-copy`, `xclip` or `xsel`). Terminals that don't
//! understand OSC 52 ignore it.

use std::io::Write;
use std::process::{Command, Stdio};

use base64::Engine;

/// Encode text as OSC 52, with tmux passthrough when needed.
pub fn osc52(text: &str, tmux: bool) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let plain = format!("\x1b]52;c;{encoded}\x07");
    if tmux {
        format!("{plain}\x1bPtmux;\x1b\x1b]52;c;{encoded}\x07\x1b\\")
    } else {
        plain
    }
}

/// Whether this process runs in an SSH session, where a native copy would
/// land on the remote machine's clipboard rather than the person's.
fn remote() -> bool {
    ["SSH_CONNECTION", "SSH_TTY", "SSH_CLIENT"]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
}

/// The native clipboard tools to try on this machine, first that works
/// wins. None over SSH.
#[must_use]
pub fn native_tools(
    os: &str,
    remote: bool,
    wayland: bool,
    x11: bool,
) -> Vec<(&'static str, &'static [&'static str])> {
    if remote {
        return Vec::new();
    }
    match os {
        "macos" => vec![("pbcopy", &[])],
        "linux" | "freebsd" | "openbsd" | "netbsd" => {
            let mut tools: Vec<(&'static str, &'static [&'static str])> = Vec::new();
            if wayland {
                tools.push(("wl-copy", &[]));
            }
            if x11 {
                tools.push(("xclip", &["-selection", "clipboard"]));
                tools.push(("xsel", &["--clipboard", "--input"]));
            }
            tools
        }
        _ => Vec::new(),
    }
}

/// Hand `text` to `program` on its stdin, off the draw loop's thread so a
/// stalled helper never freezes the screen.
fn pipe(program: &'static str, args: &'static [&'static str], text: &str) -> std::io::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = child.stdin.take();
    let text = text.to_owned();
    std::thread::spawn(move || {
        if let Some(stdin) = &mut stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        drop(stdin);
        let _ = child.wait();
    });
    Ok(())
}

/// Put text on the system clipboard through every leg that can reach it.
/// Fails only when no leg could be written.
pub fn to_clipboard(text: &str) -> std::io::Result<()> {
    let tmux = std::env::var_os("TMUX").is_some();
    let mut out = std::io::stdout();
    let osc = out
        .write_all(osc52(text, tmux).as_bytes())
        .and_then(|()| out.flush());
    if tmux {
        let _ = pipe("tmux", &["load-buffer", "-"], text);
    }
    let tools = native_tools(
        std::env::consts::OS,
        remote(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
        std::env::var_os("DISPLAY").is_some(),
    );
    let native = tools
        .into_iter()
        .any(|(program, args)| pipe(program, args, text).is_ok());
    if native { Ok(()) } else { osc }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc52_carries_the_text_in_base64() {
        assert_eq!(osc52("hi ✓", false), "\x1b]52;c;aGkg4pyT\x07");
        let wrapped = osc52("hi", true);
        assert!(wrapped.starts_with("\x1b]52;c;aGk=\x07\x1bPtmux;\x1b\x1b]52;c;aGk="));
        assert!(wrapped.ends_with("\x1b\\"));
    }

    #[test]
    fn osc52_round_trips_multiline_copies() {
        let text = "fn main() {\n\tprintln!(\"日本\");\n}";
        let sequence = osc52(text, false);
        let body = sequence
            .strip_prefix("\x1b]52;c;")
            .and_then(|rest| rest.strip_suffix('\x07'))
            .unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(body)
            .unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), text);
    }

    #[test]
    fn native_tools_fit_the_machine() {
        fn names(tools: Vec<(&'static str, &'static [&'static str])>) -> Vec<&'static str> {
            tools.into_iter().map(|tool| tool.0).collect()
        }
        assert_eq!(
            names(native_tools("macos", false, false, false)),
            ["pbcopy"]
        );
        assert!(native_tools("macos", true, false, false).is_empty());
        assert_eq!(
            names(native_tools("linux", false, true, true)),
            ["wl-copy", "xclip", "xsel"]
        );
        assert_eq!(
            names(native_tools("linux", false, false, true)),
            ["xclip", "xsel"]
        );
        assert!(native_tools("linux", false, false, false).is_empty());
    }
}
