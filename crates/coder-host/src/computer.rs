//! The computer itself, for a device that holds `terminal`: a screenshot of
//! its screen, the apps open on it, and files copied to and from it
//! (NIP-HOST `computer`, `coder_access::computer`).
//!
//! `coder-access` has already checked the grant and the `terminal` right
//! before anything here runs. These are the same things a terminal on the
//! computer could do, done without one: with fixed tools, bounded sizes,
//! and every file checked against its SHA-256 digest. A capture tool that
//! is missing, a screen session that is not there, and a file that does
//! not exist answer [`Answer::Unable`] with a sentence, never a guess.
//!
//! - A screenshot goes into `captures/` beside the access store, `0700`;
//!   the newest [`KEEP_CAPTURES`] are kept and older ones removed. The
//!   device reads it back with `read` like any other file.
//! - A write keeps its chunks in a hidden partial file beside the
//!   destination, named by the file's digest, each at its own place, with
//!   a record of which chunks arrived; chunks may come in any order, so a
//!   device keeps several in flight. The file moves into place only once
//!   every chunk arrived and the digest matched. An existing file is
//!   replaced only when the request says `overwrite`.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine as _;
use coder_access::Code;
use coder_access::computer::{
    Answer, App, CHUNK_BYTES, FileInfo, FilePut, MAX_APPS, MAX_FILE_BYTES, MAX_LABEL, MAX_REASON,
    MAX_SCREENSHOT_BYTES, Request, Source,
};

/// Screenshots kept in the capture folder; older ones are removed.
pub const KEEP_CAPTURES: usize = 8;

/// Answer one `computer` request. `state` is the folder beside the access
/// store, where captures go.
///
/// # Errors
/// A request outside its bounds (`malformed` / `bounds`), or a write over
/// an existing file without `overwrite` (`conflict`).
pub fn answer(state: &Path, request: &Request) -> Result<Answer, Code> {
    request.validate().map_err(|error| error.code)?;
    Ok(match request {
        Request::Screenshot { source } => screenshot(state, source),
        Request::Apps {} => apps(),
        Request::Stat { path } => stat(path),
        Request::Read {
            path,
            offset,
            length,
        } => read(path, *offset, *length),
        Request::Write { put } => return write(put),
    })
}

fn unable(reason: impl Into<String>) -> Answer {
    let reason: String = reason.into();
    Answer::Unable {
        reason: reason.chars().take(MAX_REASON).collect(),
    }
}

fn clip(text: &str) -> String {
    text.trim().chars().take(MAX_LABEL).collect()
}

/// The host user's home.
fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// The path a request names, with a leading `~` read as the home.
fn resolve(path: &str) -> Result<PathBuf, String> {
    if path == "~" {
        return home().ok_or_else(|| "this computer has no home folder set".to_owned());
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return home()
            .map(|home| home.join(rest))
            .ok_or_else(|| "this computer has no home folder set".to_owned());
    }
    Ok(PathBuf::from(path))
}

/// The size and `sha256:` digest of a file, read in full.
fn digest_file(path: &Path) -> std::io::Result<(u64, String)> {
    use sha2::Digest as _;
    let mut file = File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut size = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    let hex = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok((size, format!("sha256:{hex}")))
}

fn stat(path: &str) -> Answer {
    let resolved = match resolve(path) {
        Ok(resolved) => resolved,
        Err(reason) => return unable(reason),
    };
    describe(&resolved, None, MAX_FILE_BYTES)
}

/// A regular file's [`FileInfo`], or why there is none.
fn describe(path: &Path, media_type: Option<&str>, limit: u64) -> Answer {
    let shown = path.display();
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return unable(format!("there is no file at {shown}"));
        }
        Err(error) => return unable(format!("{shown} can't be read: {error}")),
    };
    if !metadata.is_file() {
        return unable(format!("{shown} is not a file"));
    }
    if metadata.len() > limit {
        return unable(format!(
            "{shown} is {} bytes, over the {limit} byte limit",
            metadata.len()
        ));
    }
    match digest_file(path) {
        Ok((size, digest)) => Answer::File {
            file: FileInfo {
                path: shown.to_string(),
                size,
                digest,
                media_type: media_type.map(str::to_owned),
            },
        },
        Err(error) => unable(format!("{shown} can't be read: {error}")),
    }
}

fn read(path: &str, offset: u64, length: u64) -> Answer {
    let resolved = match resolve(path) {
        Ok(resolved) => resolved,
        Err(reason) => return unable(reason),
    };
    let shown = resolved.display();
    let mut file = match File::open(&resolved) {
        Ok(file) => file,
        Err(error) => return unable(format!("{shown} can't be read: {error}")),
    };
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return unable(format!("{shown} is not a file"));
    }
    if let Err(error) = file.seek(SeekFrom::Start(offset)) {
        return unable(format!("{shown} can't be read: {error}"));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(length.min(CHUNK_BYTES)).unwrap_or(0));
    if let Err(error) = file.take(length.min(CHUNK_BYTES)).read_to_end(&mut bytes) {
        return unable(format!("{shown} can't be read: {error}"));
    }
    Answer::Chunk {
        offset,
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
    }
}

/// The hidden files a write keeps beside the destination until the digest
/// matches: the bytes so far, and which chunks of them arrived.
fn partial(target: &Path, digest: &str) -> Option<(PathBuf, PathBuf)> {
    let name = target.file_name()?.to_string_lossy();
    let hex = coder_access::computer::digest_hex(digest).ok()?;
    let short = &hex[..16];
    Some((
        target.with_file_name(format!(".{name}.{short}.oa-partial")),
        target.with_file_name(format!(".{name}.{short}.oa-chunks")),
    ))
}

/// Writes take this lock, so chunks of one file that arrive together
/// update its chunk record one at a time.
static WRITES: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Which chunks of a `size`-byte file are held: one byte per chunk, `1`
/// when held. Without a record, a partial file an earlier host left holds
/// its whole chunks from the start, as that host appended them in order.
fn held_chunks(partial: &Path, chunks_at: &Path, size: u64) -> Vec<u8> {
    let count = usize::try_from(size.div_ceil(CHUNK_BYTES).max(1)).unwrap_or(usize::MAX);
    match std::fs::read(chunks_at) {
        Ok(record) if record.len() == count => record,
        Ok(_) => {
            // A record of another shape is not this file's: start over.
            let _ = std::fs::remove_file(partial);
            let _ = std::fs::remove_file(chunks_at);
            vec![0; count]
        }
        Err(_) => {
            let appended = std::fs::metadata(partial).map_or(0, |m| m.len());
            let whole = usize::try_from(appended / CHUNK_BYTES)
                .unwrap_or(usize::MAX)
                .min(count);
            let mut record = vec![0; count];
            record[..whole].fill(1);
            record
        }
    }
}

/// The bytes held from the start of the file, in whole chunks.
fn held_from_start(record: &[u8], size: u64) -> u64 {
    let leading = record.iter().take_while(|held| **held == 1).count() as u64;
    (leading * CHUNK_BYTES).min(size)
}

fn write(put: &FilePut) -> Result<Answer, Code> {
    let bytes = put.bytes().map_err(|error| error.code)?;
    let target = match resolve(&put.path) {
        Ok(target) => target,
        Err(reason) => return Ok(unable(reason)),
    };
    let shown = target.display().to_string();
    let Some((partial, chunks_at)) = partial(&target, &put.digest) else {
        return Ok(unable(format!("{shown} does not name a file")));
    };
    let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Ok(unable(format!("{shown} does not name a file")));
    };
    if !parent.is_dir() {
        return Ok(unable(format!(
            "the folder {} does not exist on this computer",
            parent.display()
        )));
    }
    if target.is_dir() {
        return Ok(unable(format!("{shown} is a folder; name the file")));
    }
    let _writing = WRITES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let started = partial.exists() || chunks_at.exists();
    if !started {
        // A chunk again after the file was put in place (a lost reply):
        // the file there is exactly this one.
        if std::fs::metadata(&target).is_ok_and(|m| m.is_file() && m.len() == put.size)
            && digest_file(&target).is_ok_and(|(_, digest)| digest == put.digest)
        {
            return Ok(Answer::Written {
                received: put.size,
                complete: true,
            });
        }
        if target.exists() && !put.overwrite {
            return Err(Code::Conflict);
        }
    }
    let mut record = held_chunks(&partial, &chunks_at, put.size);
    let index = usize::try_from(put.offset / CHUNK_BYTES).unwrap_or(usize::MAX);
    let Some(slot) = record.get_mut(index) else {
        return Err(Code::Malformed);
    };
    // A chunk already held changes nothing when it comes again.
    if *slot == 0 {
        *slot = 1;
        // Each chunk goes to its own place, so chunks may arrive in any
        // order. The record is written after the bytes; the digest check
        // before the file is put in place catches any chunk a crash lost
        // in between.
        let placed = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&partial)
            .and_then(|mut file| {
                file.seek(SeekFrom::Start(put.offset))?;
                file.write_all(&bytes)
            })
            .and_then(|()| std::fs::write(&chunks_at, &record));
        if let Err(error) = placed {
            return Ok(unable(format!("{shown} can't be written: {error}")));
        }
    }
    if record.iter().any(|held| *held == 0) {
        return Ok(Answer::Written {
            received: held_from_start(&record, put.size),
            complete: false,
        });
    }
    let drop_partial = || {
        let _ = std::fs::remove_file(&partial);
        let _ = std::fs::remove_file(&chunks_at);
    };
    let synced = File::open(&partial).and_then(|file| file.sync_all());
    match (synced, digest_file(&partial)) {
        (Ok(()), Ok((size, digest))) if size == put.size && digest == put.digest => {}
        _ => {
            drop_partial();
            return Err(Code::Conflict);
        }
    }
    if target.exists() && !put.overwrite {
        drop_partial();
        return Err(Code::Conflict);
    }
    if let Err(error) = std::fs::rename(&partial, &target) {
        return Ok(unable(format!("{shown} can't be written: {error}")));
    }
    let _ = std::fs::remove_file(&chunks_at);
    Ok(Answer::Written {
        received: put.size,
        complete: true,
    })
}

/// The folder screenshots go in, owner-only.
fn captures(state: &Path) -> std::io::Result<PathBuf> {
    let dir = state.join("captures");
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}

/// Remove all but the newest [`KEEP_CAPTURES`] screenshots.
fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut shots: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("shot-"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    shots.sort();
    let excess = shots.len().saturating_sub(KEEP_CAPTURES);
    for (_, path) in shots.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
}

fn screenshot(state: &Path, source: &Source) -> Answer {
    let dir = match captures(state) {
        Ok(dir) => dir,
        Err(error) => return unable(format!("the capture folder can't be made: {error}")),
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let path = dir.join(format!("shot-{nanos}.png"));
    let taken = match source {
        Source::Android { serial } => android(&path, serial.as_deref()),
        Source::Screen { screen } => screen_shot(&path, screen.as_deref()),
    };
    let answer = match taken {
        Ok(()) => match std::fs::read(&path)
            .ok()
            .and_then(|bytes| bytes.starts_with(b"\x89PNG\r\n\x1a\n").then_some(()))
        {
            Some(()) => describe(&path, Some("image/png"), MAX_SCREENSHOT_BYTES),
            None => {
                let _ = std::fs::remove_file(&path);
                unable("the capture tool wrote no PNG image")
            }
        },
        Err(reason) => {
            let _ = std::fs::remove_file(&path);
            unable(reason)
        }
    };
    prune(&dir);
    answer
}

/// Run `command` and report its failure as a sentence naming `tool`.
fn ran(mut command: Command, tool: &str) -> Result<std::process::Output, String> {
    match command.output() {
        Ok(output) if output.status.success() => Ok(output),
        Ok(output) => {
            let said = String::from_utf8_lossy(&output.stderr);
            let said = said.trim();
            Err(if said.is_empty() {
                format!("{tool} failed ({})", output.status)
            } else {
                format!("{tool} failed: {}", clip(said))
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("{tool} is not installed on this computer"))
        }
        Err(error) => Err(format!("{tool} could not start: {error}")),
    }
}

fn android(path: &Path, serial: Option<&str>) -> Result<(), String> {
    let mut command = Command::new("adb");
    if let Some(serial) = serial {
        command.args(["-s", serial]);
    }
    command.args(["exec-out", "screencap", "-p"]);
    let output = ran(command, "adb")?;
    if output.stdout.is_empty() {
        return Err("adb returned no image; is an Android device attached and allowed?".into());
    }
    if output.stdout.len() as u64 > MAX_SCREENSHOT_BYTES {
        return Err("the Android screenshot is over the size limit".into());
    }
    std::fs::write(path, &output.stdout)
        .map_err(|error| format!("the image can't be saved: {error}"))
}

#[cfg(target_os = "macos")]
fn screen_shot(path: &Path, screen: Option<&str>) -> Result<(), String> {
    let mut command = Command::new("/usr/sbin/screencapture");
    command.args(["-x", "-t", "png"]);
    if let Some(screen) = screen {
        command.args(["-D", screen]);
    }
    command.arg(path);
    const PERMISSION: &str = "macOS lets a program capture the screen only with Screen \
                              Recording permission: turn it on for the OpenAgents host in \
                              System Settings > Privacy & Security > Screen Recording.";
    if let Err(reason) = ran(command, "screencapture") {
        return Err(format!("{reason}. {PERMISSION}"));
    }
    if !path.exists() {
        return Err(format!("screencapture wrote no image. {PERMISSION}"));
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn screen_shot(path: &Path, screen: Option<&str>) -> Result<(), String> {
    let session = linux::Session::find();
    let mut tried = Vec::new();
    if let Some(desk) = session.desk() {
        match desk.shot(&path.to_string_lossy(), screen) {
            Ok(()) if path.exists() => return Ok(()),
            Ok(()) => tried.push("the desktop wrote no image".to_owned()),
            Err(error) => tried.push(clip(&error.say())),
        }
    }
    if let Some(wayland) = &session.wayland {
        let mut command = Command::new("grim");
        session.environment(&mut command);
        command.env("WAYLAND_DISPLAY", wayland);
        if let Some(screen) = screen {
            command.args(["-o", screen]);
        }
        command.arg(path);
        match ran(command, "grim") {
            Ok(_) if path.exists() => return Ok(()),
            Ok(_) => tried.push("grim wrote no image".to_owned()),
            Err(reason) => tried.push(reason),
        }
    }
    if let Some(display) = screen
        .filter(|screen| screen.starts_with(':'))
        .map(str::to_owned)
        .or_else(|| session.x11.clone())
    {
        for (tool, arguments) in [
            ("maim", vec![]),
            ("scrot", vec!["-o"]),
            ("import", vec!["-window", "root"]),
        ] {
            let mut command = Command::new(tool);
            session.environment(&mut command);
            command.env("DISPLAY", &display);
            command.args(arguments).arg(path);
            match ran(command, tool) {
                Ok(_) if path.exists() => return Ok(()),
                Ok(_) => tried.push(format!("{tool} wrote no image")),
                Err(reason) => tried.push(reason),
            }
        }
    }
    Err(if tried.is_empty() {
        "no screen session is running on this computer (no Wayland or X11 display found)".to_owned()
    } else {
        format!("the screen could not be captured: {}", tried.join("; "))
    })
}

#[cfg(not(unix))]
fn screen_shot(_path: &Path, _screen: Option<&str>) -> Result<(), String> {
    Err("screenshots are not supported on this computer's system yet".into())
}

#[cfg(target_os = "macos")]
fn apps() -> Answer {
    let mut list = Command::new("/usr/bin/lsappinfo");
    list.arg("list");
    let output = match ran(list, "lsappinfo") {
        Ok(output) => output,
        Err(reason) => return unable(reason),
    };
    let mut front = Command::new("/usr/bin/lsappinfo");
    front.arg("front");
    let front = ran(front, "lsappinfo")
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_default();
    let apps = parse_lsappinfo(&String::from_utf8_lossy(&output.stdout), &front);
    Answer::Apps {
        apps,
        source: "lsappinfo".into(),
    }
}

/// The foreground apps in `lsappinfo list`, the one whose ASN is `front`
/// focused.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_lsappinfo(text: &str, front: &str) -> Vec<App> {
    let mut apps = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some((_, rest)) = trimmed
            .split_once(") \"")
            .filter(|(number, _)| number.chars().all(|c| c.is_ascii_digit()))
        {
            current = rest
                .split_once("\" ")
                .map(|(name, asn)| (name.to_owned(), asn.trim().trim_end_matches(':').to_owned()));
        } else if let Some(rest) = trimmed.strip_prefix("pid = ")
            && let Some((name, asn)) = current.take()
            && rest.contains("type=\"Foreground\"")
        {
            let pid = rest
                .split_whitespace()
                .next()
                .and_then(|pid| pid.parse().ok());
            let focused = !asn.is_empty() && front.trim_end_matches(':') == asn;
            if apps.len() < MAX_APPS {
                apps.push(App {
                    name: clip(&name),
                    title: None,
                    pid,
                    focused,
                });
            }
        }
    }
    apps
}

#[cfg(all(unix, not(target_os = "macos")))]
fn apps() -> Answer {
    let session = linux::Session::find();
    let mut tried = Vec::new();
    if let Some(desk) = session.desk() {
        let focused = desk.focused().ok().flatten().map(|window| window.handle);
        match desk.list() {
            Ok(windows) => {
                return Answer::Apps {
                    apps: windows
                        .into_iter()
                        .take(MAX_APPS)
                        .map(|window| App {
                            focused: focused.as_ref() == Some(&window.handle),
                            name: clip(&window.app_id),
                            title: Some(clip(&window.title)).filter(|title| !title.is_empty()),
                            pid: window.pid,
                        })
                        .collect(),
                    source: desk.desk().backend().into(),
                };
            }
            Err(error) => tried.push(clip(&error.say())),
        }
    }
    if let Some(display) = &session.x11 {
        let mut command = Command::new("wmctrl");
        session.environment(&mut command);
        command.env("DISPLAY", display).arg("-lp");
        match ran(command, "wmctrl") {
            Ok(output) => {
                let apps = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .take(MAX_APPS)
                    .filter_map(|line| {
                        let fields: Vec<&str> = line.splitn(5, char::is_whitespace).collect();
                        let pid = fields.get(2)?.parse().ok();
                        let title = fields.get(4).map(|title| clip(title));
                        Some(App {
                            name: title.clone().unwrap_or_default(),
                            title,
                            pid,
                            focused: false,
                        })
                    })
                    .collect();
                return Answer::Apps {
                    apps,
                    source: "wmctrl".into(),
                };
            }
            Err(reason) => tried.push(reason),
        }
    }
    unable(if tried.is_empty() {
        "no desktop session is running on this computer, so there are no windows to list".to_owned()
    } else {
        format!("the open apps could not be listed: {}", tried.join("; "))
    })
}

#[cfg(not(unix))]
fn apps() -> Answer {
    unable("listing open apps is not supported on this computer's system yet")
}

#[cfg(all(unix, not(target_os = "macos")))]
mod linux {
    //! Finding the screen session from a host that the service manager
    //! started outside it, so its environment names no display.

    use std::path::PathBuf;
    use std::process::Command;

    pub(super) struct Session {
        runtime: Option<PathBuf>,
        /// The Wayland socket's name, such as `wayland-1`.
        pub(super) wayland: Option<String>,
        /// The X11 display, such as `:0`.
        pub(super) x11: Option<String>,
        announcement: coder_desk::Announcement,
    }

    impl Session {
        pub(super) fn find() -> Self {
            let runtime = std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .filter(|path| !path.as_os_str().is_empty())
                .or_else(|| {
                    // SAFETY: getuid has no preconditions.
                    let uid = unsafe { libc::getuid() };
                    let path = PathBuf::from(format!("/run/user/{uid}"));
                    path.is_dir().then_some(path)
                });
            let wayland = std::env::var("WAYLAND_DISPLAY")
                .ok()
                .filter(|name| !name.is_empty())
                .or_else(|| {
                    let mut names: Vec<String> = std::fs::read_dir(runtime.as_ref()?)
                        .ok()?
                        .flatten()
                        .map(|entry| entry.file_name().to_string_lossy().into_owned())
                        .filter(|name| name.starts_with("wayland-") && !name.ends_with(".lock"))
                        .collect();
                    names.sort();
                    names.into_iter().next()
                });
            let x11 = std::env::var("DISPLAY")
                .ok()
                .filter(|name| !name.is_empty())
                .or_else(|| {
                    let mut names: Vec<String> = std::fs::read_dir("/tmp/.X11-unix")
                        .ok()?
                        .flatten()
                        .filter_map(|entry| {
                            let name = entry.file_name().to_string_lossy().into_owned();
                            name.strip_prefix('X').map(|n| format!(":{n}"))
                        })
                        .collect();
                    names.sort();
                    names.into_iter().next()
                });
            let mut announcement = coder_desk::Announcement::from_env();
            if announcement.runtime_dir.is_none() {
                announcement.runtime_dir.clone_from(&runtime);
            }
            if !announcement.names_a_desk()
                && let Some(runtime) = &runtime
            {
                // The Coder compositor's desk socket, newest first; a
                // session that ended leaves its file behind, and
                // `Desk::found` asks whether anything answers.
                announcement.desk_socket =
                    newest(&runtime.join("coder-desk"), |name| name.ends_with(".sock"));
                if announcement.desk_socket.is_none() {
                    announcement.signature = newest(&runtime.join("hypr"), |_| true)
                        .and_then(|path| Some(path.file_name()?.to_string_lossy().into_owned()));
                }
            }
            Self {
                runtime,
                wayland,
                x11,
                announcement,
            }
        }

        pub(super) fn desk(&self) -> Option<coder_desk::Blocking> {
            coder_desk::Blocking::found(&self.announcement).ok()
        }

        pub(super) fn environment(&self, command: &mut Command) {
            if let Some(runtime) = &self.runtime {
                command.env("XDG_RUNTIME_DIR", runtime);
            }
        }
    }

    fn newest(dir: &std::path::Path, keep: impl Fn(&str) -> bool) -> Option<PathBuf> {
        let mut entries: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .filter(|entry| keep(&entry.file_name().to_string_lossy()))
            .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
            .collect();
        entries.sort();
        entries.pop().map(|(_, path)| path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::computer::{self, digest};

    fn put(path: &Path, bytes: &[u8], offset: u64, overwrite: bool) -> FilePut {
        let start = usize::try_from(offset).unwrap();
        let end = (start + CHUNK_BYTES as usize).min(bytes.len());
        FilePut {
            path: path.display().to_string(),
            size: bytes.len() as u64,
            digest: digest(bytes),
            offset,
            data: base64::engine::general_purpose::STANDARD.encode(&bytes[start..end]),
            overwrite,
        }
    }

    #[test]
    fn a_write_lands_only_whole_and_checked_and_never_over_a_file_unasked() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("f.bin");
        let bytes: Vec<u8> = (0..(2 * CHUNK_BYTES + 5))
            .map(|i| (i % 253) as u8)
            .collect();
        let first = write(&put(&target, &bytes, 0, false)).unwrap();
        assert_eq!(
            first,
            Answer::Written {
                received: CHUNK_BYTES,
                complete: false
            }
        );
        assert!(!target.exists(), "nothing lands before the last chunk");
        // A resend of a held chunk changes nothing and says where to go on.
        assert_eq!(write(&put(&target, &bytes, 0, false)).unwrap(), first);
        // A chunk past a gap is held, and the answer still counts the
        // bytes held from the start.
        assert_eq!(
            write(&put(&target, &bytes, 2 * CHUNK_BYTES, false)).unwrap(),
            first
        );
        assert!(!target.exists(), "nothing lands with a gap");
        let last = write(&put(&target, &bytes, CHUNK_BYTES, false)).unwrap();
        assert_eq!(
            last,
            Answer::Written {
                received: bytes.len() as u64,
                complete: true
            }
        );
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        // The last chunk again (a lost reply) answers complete.
        assert_eq!(
            write(&put(&target, &bytes, 2 * CHUNK_BYTES, false)).unwrap(),
            last
        );
        // Another file over it is refused without overwrite.
        assert_eq!(
            write(&put(&target, b"new", 0, false)).unwrap_err(),
            Code::Conflict
        );
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        write(&put(&target, b"new", 0, true)).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        // No partial file is left behind.
        let left: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left.len(), 1, "{left:?}");
    }

    #[test]
    fn chunks_land_whole_in_any_order_and_together() {
        let dir = tempfile::tempdir().unwrap();
        let bytes: Vec<u8> = (0..(9 * CHUNK_BYTES + 77))
            .map(|i| (i % 233) as u8)
            .collect();
        let offsets: Vec<u64> = (0..10).map(|i| i * CHUNK_BYTES).collect();
        // In reverse, one at a time.
        let reverse = dir.path().join("reverse.bin");
        for (n, offset) in offsets.iter().rev().enumerate() {
            let answer = write(&put(&reverse, &bytes, *offset, false)).unwrap();
            let Answer::Written { received, complete } = answer else {
                panic!("{answer:?}")
            };
            // Until the first chunk comes, none is held from the start.
            assert_eq!(complete, n == offsets.len() - 1);
            assert_eq!(received, if complete { bytes.len() as u64 } else { 0 });
        }
        assert_eq!(std::fs::read(&reverse).unwrap(), bytes);
        // From several threads at once, as a device's window arrives.
        let together = dir.path().join("together.bin");
        std::thread::scope(|scope| {
            for offset in &offsets {
                let (bytes, together) = (&bytes, &together);
                scope.spawn(move || write(&put(together, bytes, *offset, false)).unwrap());
            }
        });
        assert_eq!(std::fs::read(&together).unwrap(), bytes);
        // Only the two files are left: no partial or chunk record.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn a_partial_an_earlier_host_appended_is_resumed() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("old.bin");
        let bytes: Vec<u8> = (0..(3 * CHUNK_BYTES)).map(|i| (i % 7) as u8).collect();
        let (partial, _) = partial(&target, &digest(&bytes)).unwrap();
        std::fs::write(&partial, &bytes[..CHUNK_BYTES as usize]).unwrap();
        assert_eq!(
            write(&put(&target, &bytes, 2 * CHUNK_BYTES, false)).unwrap(),
            Answer::Written {
                received: CHUNK_BYTES,
                complete: false
            }
        );
        write(&put(&target, &bytes, CHUNK_BYTES, false)).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
    }

    #[test]
    fn a_write_whose_bytes_differ_from_its_digest_is_refused_and_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("f.bin");
        let mut chunk = put(&target, b"hello", 0, false);
        chunk.digest = digest(b"other");
        assert_eq!(write(&chunk).unwrap_err(), Code::Conflict);
        assert!(!target.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn reads_and_stats_answer_bytes_digests_and_plain_reasons() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("r.txt");
        std::fs::write(&target, b"0123456789").unwrap();
        let shown = target.display().to_string();
        let Answer::File { file } = stat(&shown) else {
            panic!()
        };
        assert_eq!(file.size, 10);
        assert_eq!(file.digest, digest(b"0123456789"));
        let Answer::Chunk { offset, data } = read(&shown, 4, 100) else {
            panic!()
        };
        assert_eq!(offset, 4);
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .unwrap(),
            b"456789"
        );
        let missing = stat(&dir.path().join("nope").display().to_string());
        assert!(matches!(missing, Answer::Unable { reason } if reason.contains("no file")));
        let folder = stat(&dir.path().display().to_string());
        assert!(matches!(folder, Answer::Unable { reason } if reason.contains("not a file")));
        assert_eq!(
            answer(
                dir.path(),
                &Request::Stat {
                    path: "relative".into()
                }
            )
            .unwrap_err(),
            Code::Malformed
        );
        let _ = computer::MAX_FILE_BYTES;
    }

    #[test]
    fn captures_keep_only_the_newest() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..(KEEP_CAPTURES + 3) {
            std::fs::write(dir.path().join(format!("shot-{index:03}.png")), b"x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        std::fs::write(dir.path().join("other"), b"x").unwrap();
        prune(dir.path());
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left.len(), KEEP_CAPTURES + 1);
        assert!(!left.contains(&"shot-000.png".to_owned()));
        assert!(left.contains(&"other".to_owned()));
    }

    #[test]
    fn lsappinfo_lists_foreground_apps_and_the_front_one() {
        let text = r#" 1) "loginwindow" ASN:0x0-0x2002:
    pid = 414 type="UIElement" flavor=3
 6) "Google Chrome" ASN:0x0-0xb00b:
    pid = 684 type="Foreground" flavor=3
 7) "Finder" ASN:0x0-0x26026:
    pid = 700 type="Foreground" flavor=3
"#;
        let apps = parse_lsappinfo(text, "ASN:0x0-0x26026:");
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].name, "Google Chrome");
        assert_eq!(apps[0].pid, Some(684));
        assert!(!apps[0].focused);
        assert!(apps[1].focused);
    }
}
