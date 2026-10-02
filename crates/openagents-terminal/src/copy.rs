//! Copying through the terminal: OSC 52 asks the terminal to put text on
//! the system clipboard, so a copy works over SSH and in tmux too.

use std::io::Write;

use base64::Engine;

/// The OSC 52 sequence that sets the clipboard to `text`. Inside tmux the
/// sequence is also sent wrapped for tmux to pass through to the terminal
/// around it.
pub fn osc52(text: &str, tmux: bool) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let plain = format!("\x1b]52;c;{encoded}\x07");
    if tmux {
        format!("{plain}\x1bPtmux;\x1b\x1b]52;c;{encoded}\x07\x1b\\")
    } else {
        plain
    }
}

/// Put `text` on the clipboard of the terminal this process draws on.
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
