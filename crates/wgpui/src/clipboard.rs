//! Clipboard utilities for copying text to the system clipboard.

use std::io::Write;
use std::process::{Command, Stdio};

/// Copy text to the system clipboard.
///
/// Uses platform-specific commands:
/// - macOS: `pbcopy`
/// - Linux: `wl-copy` (Wayland), `xclip`, or `xsel`
/// - Windows: `clip`
pub fn copy_to_clipboard(contents: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return copy_with_command("pbcopy", &[], contents);
    }

    #[cfg(target_os = "linux")]
    {
        // Try Wayland first, then X11 tools
        if try_command("wl-copy", &[], contents).is_ok() {
            return Ok(());
        }
        if try_command("xclip", &["-selection", "clipboard"], contents).is_ok() {
            return Ok(());
        }
        if try_command("xsel", &["--clipboard", "--input"], contents).is_ok() {
            return Ok(());
        }
        return Err("No clipboard tool available. Install wl-copy, xclip, or xsel.".into());
    }

    #[cfg(target_os = "windows")]
    {
        return copy_with_command("cmd", &["/C", "clip"], contents);
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        return Err("Clipboard not supported on this platform".into());
    }
}

fn copy_with_command(cmd: &str, args: &[&str], contents: &str) -> Result<(), String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start {}: {}", cmd, e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(contents.as_bytes())
            .map_err(|e| format!("Failed to write to {}: {}", cmd, e))?;
    }

    child
        .wait()
        .map_err(|e| format!("Failed to wait for {}: {}", cmd, e))?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn try_command(cmd: &str, args: &[&str], contents: &str) -> Result<(), ()> {
    copy_with_command(cmd, args, contents).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_copy_to_clipboard() {
        // Only run if pbcopy is available
        let result = copy_to_clipboard("test clipboard content");
        assert!(result.is_ok());
    }
}
