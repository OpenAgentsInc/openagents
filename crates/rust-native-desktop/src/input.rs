//! Native input translated into portable desktop editing events.

/// Physical controls for an application-owned interactive viewport. Key codes
/// use the native physical key's name, independent of typed text and IME.
#[derive(Clone, Copy, Debug)]
pub enum NativeInput<'a> {
    Key {
        code: &'a str,
        pressed: bool,
        repeat: bool,
        /// Command on macOS, or Control elsewhere: either is held.
        command: bool,
        alt: bool,
        /// The Control key itself is held, on every platform.
        control: bool,
        /// The Command key (macOS) or the Super/Windows key is held.
        logo: bool,
    },
    Button {
        button: u16,
        pressed: bool,
        x: f32,
        y: f32,
    },
    Cursor {
        x: f32,
        y: f32,
    },
    Motion {
        dx: f32,
        dy: f32,
    },
    Wheel {
        lines: f32,
        x: f32,
        y: f32,
    },
    Focus(bool),
    Cancel,
}

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
    Down {
        x: f32,
        y: f32,
        shift: bool,
    },
    Move {
        x: f32,
        y: f32,
    },
    Up {
        x: f32,
        y: f32,
    },
    Wheel {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
    },
    /// Zoom about `x`, `y` by `factor` (above 1 zooms in): a pinch on a
    /// trackpad, or the wheel with Cmd or Ctrl held.
    Zoom {
        x: f32,
        y: f32,
        factor: f32,
    },
}

/// The most text a paste reads.
#[cfg(feature = "window")]
const MAX_TEXT: usize = 64 * 1024;

/// With `OPENAGENTS_CLIPBOARD_TRACE` set, says on standard error which way
/// a clipboard action went (never what it carried), for checking a desktop.
#[cfg(feature = "window")]
fn trace(action: &str, way: &str) {
    if std::env::var_os("OPENAGENTS_CLIPBOARD_TRACE").is_some() {
        eprintln!("clipboard: {action} through {way}");
    }
}

/// Read the system clipboard only in response to an explicit paste action.
/// On Linux: the window's own Wayland seat first ([`crate::wayland`]),
/// then `wl-paste` or `xclip`, then `arboard`. On Windows: the clipboard
/// API through `arboard`, as Unicode text, where a PowerShell pipe would
/// pass it through the console's code page and flash a console window.
#[cfg(feature = "window")]
pub fn paste() -> Option<String> {
    #[cfg(target_os = "linux")]
    if let Some((_, bytes)) = crate::wayland::paste(crate::transfer::TEXT) {
        trace("paste", "the window's Wayland seat");
        return (bytes.len() <= MAX_TEXT)
            .then(|| String::from_utf8(bytes).ok())
            .flatten();
    }
    if let Some(text) = clipboard_command(false).and_then(|command| read_command(command, MAX_TEXT))
    {
        trace("paste", "the clipboard command");
        return String::from_utf8(text).ok();
    }
    #[cfg(any(target_os = "linux", windows))]
    if let Some(text) = with_arboard(|clipboard| clipboard.get_text().ok()) {
        trace("paste", "arboard");
        return (text.len() <= MAX_TEXT).then_some(text);
    }
    None
}

/// Write selected text only after an explicit copy or cut action. On
/// Linux: the window's own Wayland selection first, then `wl-copy` or
/// `xclip`, then `arboard`. On Windows: the clipboard API, as [`paste`].
#[cfg(feature = "window")]
pub fn copy(text: &str) -> bool {
    #[cfg(target_os = "linux")]
    if crate::wayland::copy(text) {
        trace("copy", "the window's Wayland seat");
        return true;
    }
    if clipboard_command(true).is_some_and(|command| write_command(command, text.as_bytes())) {
        trace("copy", "the clipboard command");
        return true;
    }
    #[cfg(any(target_os = "linux", windows))]
    if with_arboard(|clipboard| clipboard.set_text(text).ok()).is_some() {
        trace("copy", "arboard");
        return true;
    }
    false
}

/// Empty the clipboard, after a copied secret (a pairing code) is used.
#[cfg(feature = "window")]
pub fn clear() -> bool {
    #[cfg(target_os = "linux")]
    {
        if crate::wayland::clear() {
            trace("clear", "the window's Wayland seat");
            return true;
        }
        let mut command = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            let mut command = std::process::Command::new("wl-copy");
            command.arg("--clear");
            command
        } else {
            let mut command = std::process::Command::new("xclip");
            command.args(["-selection", "clipboard", "-in"]);
            command
        };
        if command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return true;
        }
        with_arboard(|clipboard| clipboard.clear().ok()).is_some()
    }
    #[cfg(not(target_os = "linux"))]
    copy("")
}

/// Runs a clipboard reader and returns its output, or `None` when it could
/// not start, failed, or wrote more than `limit` bytes.
#[cfg(feature = "window")]
fn read_command(mut command: std::process::Command, limit: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let mut bytes = Vec::new();
    child
        .stdout
        .take()?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > limit {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    if !child.wait().ok()?.success() {
        return None;
    }
    Some(bytes)
}

#[cfg(feature = "window")]
fn write_command(mut command: std::process::Command, bytes: &[u8]) -> bool {
    use std::io::Write;
    let Ok(mut child) = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut input| input.write_all(bytes).is_ok());
    child.wait().is_ok_and(|status| status.success()) && wrote
}

/// One `arboard` clipboard for the process: on Linux an X11 or
/// data-control selection lasts only as long as its owner, so the owner
/// lives until the process exits. Windows keeps what is set after it.
#[cfg(all(feature = "window", any(target_os = "linux", windows)))]
fn with_arboard<T>(read: impl FnOnce(&mut arboard::Clipboard) -> Option<T>) -> Option<T> {
    static CLIPBOARD: std::sync::Mutex<Option<arboard::Clipboard>> = std::sync::Mutex::new(None);
    let mut clipboard = CLIPBOARD.lock().ok()?;
    if clipboard.is_none() {
        *clipboard = arboard::Clipboard::new().ok();
    }
    read(clipboard.as_mut()?)
}

/// The command that reads (`write` false) or writes the clipboard's text
/// on this system.
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
        let (program, args) =
            linux_clipboard(std::env::var_os("WAYLAND_DISPLAY").is_some(), write, None);
        let mut command = std::process::Command::new(program);
        command.args(args);
        Some(command)
    }
    // Windows uses the clipboard API ([`with_arboard`]), not a command.
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = write;
        None
    }
}

/// The Linux clipboard helper and its arguments: `wl-copy`/`wl-paste` on
/// Wayland, `xclip` on X11. `mime` reads that type (`"TARGETS"` lists the
/// types on X11; `wl-paste --list-types` is `Some("")` on Wayland).
#[cfg(any(target_os = "linux", test))]
pub fn linux_clipboard(
    wayland: bool,
    write: bool,
    mime: Option<&str>,
) -> (&'static str, Vec<String>) {
    let owned = |args: &[&str]| args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    match (wayland, write, mime) {
        (true, true, _) => ("wl-copy", vec![]),
        (true, false, None) => ("wl-paste", owned(&["--no-newline"])),
        (true, false, Some("")) => ("wl-paste", owned(&["--list-types"])),
        (true, false, Some(mime)) => ("wl-paste", owned(&["--no-newline", "--type", mime])),
        (false, true, _) => ("xclip", owned(&["-selection", "clipboard", "-in"])),
        (false, false, None) => ("xclip", owned(&["-selection", "clipboard", "-out"])),
        (false, false, Some("")) => (
            "xclip",
            owned(&["-selection", "clipboard", "-out", "-target", "TARGETS"]),
        ),
        (false, false, Some(mime)) => (
            "xclip",
            owned(&["-selection", "clipboard", "-out", "-target", mime]),
        ),
    }
}

/// Native image-picker output stays local until the application admits it.
#[cfg(feature = "window")]
pub fn pick_image() -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .set_title("Attach an image")
        .add_filter("Images", &["png", "jpg", "jpeg"])
        .pick_file()
}

/// Image pixels read after an explicit paste action, on the input worker.
#[cfg(feature = "window")]
pub struct ClipboardImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// An image on the clipboard, in the form the clipboard had it.
#[cfg(feature = "window")]
pub enum PastedImage {
    /// Decoded pixels (`arboard`).
    Pixels(ClipboardImage),
    /// A PNG or JPEG file's bytes, as a browser or a screenshot tool copies.
    Encoded(Vec<u8>),
    /// A file a file manager copied.
    File(std::path::PathBuf),
}

/// The clipboard's image, after an explicit paste action. On Linux: the
/// window's own Wayland seat, then `arboard`, then `wl-paste` or `xclip`
/// asked for PNG, JPEG, or a copied file. `Ok(None)` when the clipboard
/// holds no image.
#[cfg(feature = "window")]
pub fn paste_image() -> Result<Option<PastedImage>, String> {
    #[cfg(target_os = "linux")]
    if let Some((mime, bytes)) = crate::wayland::paste(crate::transfer::IMAGE)
        && let Some(image) = pasted(&mime, bytes)
    {
        trace("image paste", "the window's Wayland seat");
        return Ok(Some(image));
    }
    let from_arboard = arboard::Clipboard::new()
        .map_err(|_| String::from("Couldn't reach the image clipboard on this desktop."))
        .and_then(|mut clipboard| match clipboard.get_image() {
            Ok(image) => Ok(Some(PastedImage::Pixels(ClipboardImage {
                width: image.width,
                height: image.height,
                rgba: image.bytes.into_owned(),
            }))),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(_) => Err("Couldn't read an image from the clipboard.".into()),
        });
    #[cfg(target_os = "linux")]
    if !matches!(from_arboard, Ok(Some(_))) {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let command = |mime: Option<&str>| {
            let (program, args) = linux_clipboard(wayland, false, mime);
            let mut command = std::process::Command::new(program);
            command.args(args);
            command
        };
        if let Some(types) = read_command(command(Some("")), MAX_TEXT) {
            let types: Vec<String> = String::from_utf8_lossy(&types)
                .lines()
                .map(|line| line.trim().to_owned())
                .collect();
            if let Some(mime) = crate::transfer::pick(&types, crate::transfer::IMAGE)
                && let Some(bytes) = read_command(command(Some(mime)), crate::transfer::MAX_BYTES)
                && let Some(image) = pasted(mime, bytes)
            {
                trace("image paste", "the clipboard command");
                return Ok(Some(image));
            }
        }
    }
    from_arboard
}

/// An image from a clipboard's bytes of `mime`.
#[cfg(all(feature = "window", target_os = "linux"))]
fn pasted(mime: &str, bytes: Vec<u8>) -> Option<PastedImage> {
    if mime == "text/uri-list" {
        return crate::transfer::uri_list_paths(&bytes)
            .into_iter()
            .next()
            .map(PastedImage::File);
    }
    (!bytes.is_empty()).then_some(PastedImage::Encoded(bytes))
}

#[cfg(test)]
mod tests {
    use super::linux_clipboard;

    #[test]
    fn linux_clipboard_follows_the_display_server() {
        assert_eq!(linux_clipboard(true, true, None).0, "wl-copy");
        assert_eq!(linux_clipboard(true, false, None).1, ["--no-newline"]);
        assert_eq!(linux_clipboard(true, false, Some("")).1, ["--list-types"]);
        assert_eq!(
            linux_clipboard(true, false, Some("image/png")).1,
            ["--no-newline", "--type", "image/png"]
        );
        assert_eq!(
            linux_clipboard(false, true, None).1,
            ["-selection", "clipboard", "-in"]
        );
        assert_eq!(
            linux_clipboard(false, false, Some("")).1,
            ["-selection", "clipboard", "-out", "-target", "TARGETS"]
        );
        assert_eq!(
            linux_clipboard(false, false, Some("image/jpeg")).1,
            ["-selection", "clipboard", "-out", "-target", "image/jpeg"]
        );
    }
}
