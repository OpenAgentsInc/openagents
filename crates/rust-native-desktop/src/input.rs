//! Native input translated into portable desktop editing events.

/// A text key or IME callback. No keyboard callback sends a network request itself.
pub enum TextInput<'a> {
    Key {
        key: &'a str,
        text: Option<&'a str>,
        command: bool,
        alt: bool,
        shift: bool,
    },
    Preedit {
        text: &'a str,
        selection: Option<(usize, usize)>,
    },
    Commit(&'a str),
    CancelComposition,
    FocusLost,
}

#[derive(Clone, Copy, Debug)]
pub enum SurfaceInput {
    Down { x: f32, y: f32, shift: bool },
    Move { x: f32, y: f32 },
    Up { x: f32, y: f32 },
    Wheel { x: f32, y: f32, dx: f32, dy: f32 },
}

/// Read the system clipboard only in response to an explicit paste action.
#[cfg(feature = "window")]
pub fn paste() -> Option<String> {
    use std::io::Read;
    let mut command = clipboard_command(false)?;
    let mut child = command.stdout(std::process::Stdio::piped()).spawn().ok()?;
    let mut bytes = Vec::new();
    child
        .stdout
        .take()?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 64 * 1024 {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    if !child.wait().ok()?.success() {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// Write selected text only after an explicit copy or cut action.
#[cfg(feature = "window")]
pub fn copy(text: &str) -> bool {
    use std::io::Write;
    let Some(mut command) = clipboard_command(true) else {
        return false;
    };
    let Ok(mut child) = command.stdin(std::process::Stdio::piped()).spawn() else {
        return false;
    };
    if child
        .stdin
        .take()
        .is_none_or(|mut input| input.write_all(text.as_bytes()).is_err())
    {
        return false;
    }
    child.wait().is_ok_and(|status| status.success())
}

#[cfg(feature = "window")]
fn clipboard_command(write: bool) -> Option<std::process::Command> {
    #[cfg(target_os = "macos")]
    {
        Some(std::process::Command::new(if write {
            "/usr/bin/pbcopy"
        } else {
            "/usr/bin/pbpaste"
        }))
    }
    #[cfg(target_os = "linux")]
    {
        let mut command =
            std::process::Command::new(if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                if write { "wl-copy" } else { "wl-paste" }
            } else {
                "xclip"
            });
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            command.args(["-selection", "clipboard"]);
            if !write {
                command.arg("-o");
            }
        } else if !write {
            command.arg("--no-newline");
        }
        Some(command)
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = std::process::Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-Command",
            if write {
                "$input | Set-Clipboard"
            } else {
                "Get-Clipboard -Raw"
            },
        ]);
        Some(command)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = write;
        None
    }
}
