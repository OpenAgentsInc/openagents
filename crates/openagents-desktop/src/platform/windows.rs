//! Windows: the host started at sign-in from the per-user `Run` key, the
//! control channel as a per-user named pipe, and the desktop's lock state,
//! clipboard, and folder chooser (the common item dialog).
//!
//! - **Keys.** The host keeps its keys as generic credentials in
//!   Credential Manager, under the service `com.openagents.desktop` and the
//!   same accounts as on a Mac. The window reads none.
//! - **Login agent.** Windows has no per-user service manager without a
//!   stored password, so the host starts at sign-in from
//!   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value
//!   [`RUN_VALUE`]. The value runs this (GUI-subsystem) executable with
//!   [`START_HOST_FLAG`], which starts `coder.exe host serve` with no
//!   console window and exits; running `coder.exe` from `Run` directly would
//!   open a console at every sign-in. The entry shows in Task Manager >
//!   Startup apps, where the person can turn it off.
//! - **Control channel.** `\\.\pipe\openagents-control-<user SID>`, whose
//!   DACL admits only that user, and whose server checks every client's
//!   token user (`coder-host`'s `control/windows.rs`).

use openagents_desktop::model::{Agent, Agents};
use std::io::{self, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The `Run` key the sign-in entry lives under, in `HKEY_CURRENT_USER`.
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// The `Run` value's name, as Task Manager shows it.
pub const RUN_VALUE: &str = "OpenAgents";
/// The argument that starts the host and exits.
pub const START_HOST_FLAG: &str = "--start-host";
/// What `coder.exe` runs with: the same as the Mac's login agent.
pub const HOST_ARGS: [&str; 5] = ["host", "serve", "--keychain", "--iroh", "--control"];
/// The host executable, beside this one.
const CODER: &str = "coder.exe";
/// The control pipe's name before the user's SID; the host's
/// `coder_host::control::windows::PIPE_STEM` after `\\.\pipe\`.
pub const PIPE_PREFIX: &str = r"\\.\pipe\openagents-control-";

// Process creation flags, from `winbase.h`.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// The `Run` value's command line for this app's executable `exe`: the
/// path in double quotes, then [`START_HOST_FLAG`]. A path that cannot be
/// quoted safely (a `"`, a control character, or not absolute) is refused.
pub fn run_command(exe: &str) -> Result<String, String> {
    let bytes = exe.as_bytes();
    let drive = bytes.len() > 3 && bytes[0].is_ascii_alphabetic() && exe[1..].starts_with(":\\");
    if !(drive || exe.starts_with(r"\\")) {
        return Err("the app's path must be absolute".into());
    }
    if exe.contains('"') || exe.chars().any(char::is_control) {
        return Err("the app's path has a character a start-up entry cannot hold".into());
    }
    Ok(format!("\"{exe}\" {START_HOST_FLAG}"))
}

/// Whether `args` (without the program name) ask this executable to start
/// the host and exit.
pub fn wants_start_host(args: &[String]) -> bool {
    args.iter().any(|arg| arg == START_HOST_FLAG)
}

/// The control pipe's name for the user `sid`.
pub fn pipe_name(sid: &str) -> String {
    format!("{PIPE_PREFIX}{sid}")
}

fn no_window(command: &mut Command) -> &mut Command {
    command.creation_flags(CREATE_NO_WINDOW)
}

/// The `coder.exe` beside this executable.
pub fn coder_path() -> Option<PathBuf> {
    let coder = std::env::current_exe().ok()?.with_file_name(CODER);
    coder.is_file().then_some(coder)
}

/// The control pipe's name for the user this process runs as.
pub fn control_path() -> Option<PathBuf> {
    sys::current_user_sid()
        .ok()
        .map(|sid| PathBuf::from(pipe_name(&sid)))
}

/// Whether a host answers this user's control pipe.
fn host_running() -> bool {
    control_path().is_some_and(|pipe| {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(pipe)
            .is_ok()
    })
}

/// Starts `coder.exe host serve` with a console that has no window, unless
/// a host already answers. The `Run` entry's command ends here.
///
/// The host gets a hidden console (`CREATE_NO_WINDOW`) rather than none
/// (`DETACHED_PROCESS`): every console program it starts, such as `git`,
/// shares that console, where with none each would open a window of its
/// own on the desktop.
pub fn start_host() -> io::Result<()> {
    if host_running() {
        return Ok(());
    }
    let coder = coder_path().ok_or_else(|| io::Error::other(
            "it is not installed on this PC. Install it in PowerShell with `irm https://openagents.com/cli/install.ps1 | iex`",
        ))?;
    Command::new(coder)
        .args(HOST_ARGS)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(drop)
}

/// Writes the sign-in entry and starts the host now. Only an installed app
/// (with `coder.exe` beside it) registers; a development build reports
/// `NotRegistered`, as on a Mac outside a bundle.
/// Windows always has Credential Manager, so `_keys` is always the
/// keychain.
pub fn register_agent(_keys: &openagents_desktop::migrate::Keys) -> Agent {
    let Ok(exe) = std::env::current_exe() else {
        return Agent::NotRegistered;
    };
    if coder_path().is_none() {
        return Agent::NotRegistered;
    }
    let Some(exe) = exe.to_str() else {
        return Agent::Failed("the app's path is not valid Unicode".into());
    };
    let command = match run_command(exe) {
        Ok(command) => command,
        Err(message) => return Agent::Failed(message),
    };
    if let Err(message) = sys::set_run_value(&command) {
        return Agent::Failed(message);
    }
    match start_host() {
        Ok(()) => Agent::Enabled,
        Err(error) => Agent::Failed(format!("Coder did not start: {error}")),
    }
}

/// Removes the sign-in entry; removing a missing one succeeds.
// For **Stop Coder** once Windows has a tray item; tested now.
#[cfg_attr(not(test), allow(dead_code))]
pub fn unregister_agent() -> Result<(), String> {
    sys::delete_run_value()
}

/// The sign-in entry's command line, if there is one.
#[cfg_attr(not(test), allow(dead_code))]
pub fn registered_agent() -> Result<Option<String>, String> {
    sys::run_value()
}

/// Windows asks no one to allow a `Run` entry; nothing to open.
pub fn open_login_items() {}

/// Whether the workstation is locked: the input desktop cannot be opened
/// while the lock screen (or another user's session) has it.
pub fn screen_locked() -> bool {
    sys::input_desktop_locked()
}

/// Whether Windows asks for less motion: "Show animations in Windows"
/// turned off (`SPI_GETCLIENTAREAANIMATION`). `false` when it cannot be read.
pub fn reduce_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    let mut animate: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the pointer.
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut animate as *mut i32).cast(),
            0,
        )
    };
    read != 0 && animate == 0
}

/// Puts `text` on the clipboard with `clip.exe`. The code is ASCII.
pub fn copy(text: &str) -> bool {
    let Ok(mut child) = no_window(&mut Command::new("clip.exe"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
    child.wait().is_ok_and(|status| status.success()) && wrote
}

fn powershell(script: &str) -> Option<String> {
    let output = no_window(&mut Command::new("powershell.exe"))
        .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Empties the clipboard if it still holds `text`.
pub fn clear_if(text: &str) {
    if powershell("Get-Clipboard -Raw").is_some_and(|held| held.trim_end() == text) {
        let _ = powershell("Set-Clipboard -Value $null");
    }
}

/// Asks the person for a folder with the system's chooser: the common
/// item dialog (`IFileOpenDialog` with `FOS_PICKFOLDERS`), which keeps any
/// path Windows can name, where a PowerShell chooser's answer passed
/// through the console's code page.
pub fn choose_folder() -> openagents_desktop::folder::Chosen {
    use openagents_desktop::folder::{Chosen, PROMPT};
    match rfd::FileDialog::new().set_title(PROMPT).pick_folder() {
        Some(path) => Chosen::Folder(path),
        None => Chosen::Cancelled,
    }
}

/// Whether Codex and Claude Code are signed in for this user, and Grok
/// Build when it is installed. On Windows Codex and Claude Code
/// keep their sign-in in a file under the profile.
pub fn signed_in(home: &Path) -> Agents {
    let claude = home.join(".claude").join(".credentials.json").exists();
    Agents {
        codex: openagents_desktop::model::codex_login(home).exists(),
        claude,
        grok: openagents_desktop::model::grok(home),
        claude_problem: openagents_desktop::claude_setup::check(home, claude),
    }
}

/// The Win32 calls, each wrapped so nothing above is `unsafe`.
mod sys {
    use super::{RUN_KEY, RUN_VALUE};
    use std::ffi::c_void;
    use std::io;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HANDLE, LocalFree,
    };
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    };
    use windows_sys::Win32::System::StationsAndDesktops::{
        CloseDesktop, DESKTOP_SWITCHDESKTOP, OpenInputDesktop,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn registry_error(code: u32) -> String {
        format!(
            "cannot change the start-up entry: {}",
            io::Error::from_raw_os_error(code as i32)
        )
    }

    pub fn set_run_value(command: &str) -> Result<(), String> {
        let data = wide(command);
        let (key, value) = (wide(RUN_KEY), wide(RUN_VALUE));
        // SAFETY: NUL-terminated wide strings; the size is in bytes and
        // includes the terminator, as REG_SZ requires.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                REG_SZ,
                data.as_ptr().cast::<c_void>(),
                (data.len() * 2) as u32,
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(registry_error(status))
        }
    }

    // For **Stop Coder** once Windows has a tray item; tested now.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn delete_run_value() -> Result<(), String> {
        let (key, value) = (wide(RUN_KEY), wide(RUN_VALUE));
        // SAFETY: NUL-terminated wide strings.
        match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) } {
            ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
            other => Err(registry_error(other)),
        }
    }

    pub fn run_value() -> Result<Option<String>, String> {
        let (key, value) = (wide(RUN_KEY), wide(RUN_VALUE));
        let mut bytes = 0u32;
        // SAFETY: a size query with no buffer.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut bytes,
            )
        };
        match status {
            ERROR_SUCCESS => {}
            ERROR_FILE_NOT_FOUND => return Ok(None),
            other => return Err(registry_error(other)),
        }
        let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
        // SAFETY: the buffer holds `bytes` bytes.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(registry_error(status));
        }
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Ok(Some(String::from_utf16_lossy(&buffer[..end])))
    }

    pub fn current_user_sid() -> io::Result<String> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: a pseudo handle for this process; `token` receives a new handle.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let result = token_sid(token);
        // SAFETY: opened above, closed once.
        unsafe { CloseHandle(token) };
        result
    }

    fn token_sid(token: HANDLE) -> io::Result<String> {
        let mut needed = 0u32;
        // SAFETY: a size query with no buffer.
        unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed) };
        if needed == 0 {
            return Err(io::Error::last_os_error());
        }
        // u64 storage keeps TOKEN_USER aligned.
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
        // SAFETY: the buffer holds at least `needed` bytes.
        if unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: TokenUser wrote a TOKEN_USER at the start of the buffer.
        let user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
        let mut text: *mut u16 = std::ptr::null_mut();
        // SAFETY: the SID points into `buffer`, alive for this call.
        if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut len = 0usize;
        // SAFETY: a NUL-terminated string from ConvertSidToStringSidW.
        while unsafe { *text.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: `len` units precede the terminator.
        let sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) });
        // SAFETY: allocated by ConvertSidToStringSidW with LocalAlloc.
        unsafe { LocalFree(text.cast()) };
        Ok(sid)
    }

    pub fn input_desktop_locked() -> bool {
        // SAFETY: opens the input desktop for a switch check, closed below.
        let desktop = unsafe { OpenInputDesktop(0, 0, DESKTOP_SWITCHDESKTOP) };
        if desktop.is_null() {
            return true;
        }
        // SAFETY: opened above, closed once.
        unsafe { CloseDesktop(desktop) };
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_run_entry_quotes_the_app_path() {
        assert_eq!(
            run_command(r"C:\Program Files\OpenAgents\OpenAgents.exe").unwrap(),
            r#""C:\Program Files\OpenAgents\OpenAgents.exe" --start-host"#
        );
        assert_eq!(
            run_command(r"C:\Users\Kai Lee\AppData\Local\Programs\OpenAgents\OpenAgents.exe")
                .unwrap(),
            r#""C:\Users\Kai Lee\AppData\Local\Programs\OpenAgents\OpenAgents.exe" --start-host"#
        );
        for bad in [
            r"OpenAgents.exe",
            r"..\OpenAgents.exe",
            r#"C:\a"b\OpenAgents.exe"#,
            "C:\\a\nb\\OpenAgents.exe",
            r"C:OpenAgents.exe",
        ] {
            assert!(run_command(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn windows_start_host_flag_is_recognised() {
        assert!(wants_start_host(&["--start-host".into()]));
        assert!(!wants_start_host(&[
            "--start-hosts".into(),
            "start-host".into()
        ]));
        assert!(!wants_start_host(&[]));
    }

    #[test]
    fn windows_pipe_name_carries_the_sid() {
        assert_eq!(
            pipe_name("S-1-5-21-1-2-3-1001"),
            r"\\.\pipe\openagents-control-S-1-5-21-1-2-3-1001"
        );
        let sid = sys::current_user_sid().unwrap();
        assert!(sid.starts_with("S-1-"));
        assert_eq!(control_path().unwrap(), PathBuf::from(pipe_name(&sid)));
    }

    /// The sign-in entry round-trips through the registry. Refuses to run
    /// when a real entry exists, so it never clobbers one.
    #[test]
    fn windows_login_agent_round_trip() {
        assert_eq!(registered_agent().unwrap(), None, "a real entry exists");
        let command = run_command(r"C:\Program Files\OpenAgents\OpenAgents.exe").unwrap();
        sys::set_run_value(&command).unwrap();
        assert_eq!(registered_agent().unwrap(), Some(command));
        unregister_agent().unwrap();
        assert_eq!(registered_agent().unwrap(), None);
        unregister_agent().unwrap();
    }
}
