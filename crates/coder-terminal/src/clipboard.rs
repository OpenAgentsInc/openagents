//! Copy through the terminal's system clipboard, including over SSH and tmux.

use std::io::Write;

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

/// Ask the terminal to put text on the system clipboard.
pub fn to_clipboard(text: &str) -> std::io::Result<()> {
    let tmux = std::env::var_os("TMUX").is_some();
    let mut out = std::io::stdout();
    out.write_all(osc52(text, tmux).as_bytes())?;
    out.flush()
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
}
