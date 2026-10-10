//! The Mac around the window: the login agent, the screen lock, the
//! clipboard, the folder chooser, and whether Codex, Claude Code, and Grok
//! Build are signed in. Each is a stub that reports "nothing" on other systems.
//!
//! None of this reads a secret. The sign-in check asks only whether the
//! credential exists (a file's presence, or a keychain item's attributes
//! without its data), never its contents.

use openagents_desktop::migrate::Keys;
use openagents_desktop::model::{Agent, Agents};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The login agent's property list, in `Contents/Library/LaunchAgents`.
pub const AGENT_PLIST: &str = "com.openagents.desktop.host.plist";

/// The app bundle's `Contents` folder, when this executable runs from one.
pub fn bundle_contents() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents")
        .then(|| contents.to_path_buf())
}

/// The `coder` the app runs: the bundle's, else one on `PATH`, else the
/// one an earlier setup installed.
pub fn coder_path() -> Option<PathBuf> {
    if let Some(contents) = bundle_contents() {
        let bundled = contents.join("MacOS/coder");
        if bundled.exists() {
            return Some(bundled);
        }
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("coder");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let installed = home.join(".openagents/bin/coder");
    installed.exists().then_some(installed)
}

/// Registers the login agent that runs `coder host serve`, and reports
/// whether it may run. Only an app bundle carries the agent's plist, which
/// always serves from the keychain: a Mac always has one.
#[cfg(target_os = "macos")]
pub fn register_agent(_keys: &Keys) -> Agent {
    use objc2_foundation::NSString;
    use objc2_service_management::{SMAppService, SMAppServiceStatus};
    let Some(contents) = bundle_contents() else {
        return Agent::NotRegistered;
    };
    if !contents
        .join("Library/LaunchAgents")
        .join(AGENT_PLIST)
        .exists()
    {
        return Agent::NotRegistered;
    }
    // SAFETY: plain ServiceManagement calls on a name we own; the service
    // object is retained for the calls' duration.
    unsafe {
        let service = SMAppService::agentServiceWithPlistName(&NSString::from_str(AGENT_PLIST));
        if service.status() != SMAppServiceStatus::Enabled
            && let Err(error) = service.registerAndReturnError()
        {
            if service.status() == SMAppServiceStatus::RequiresApproval {
                return Agent::NeedsApproval;
            }
            return Agent::Failed(error.localizedDescription().to_string());
        }
        match service.status() {
            SMAppServiceStatus::Enabled => Agent::Enabled,
            SMAppServiceStatus::RequiresApproval => Agent::NeedsApproval,
            other => Agent::Failed(format!("the login agent's status is {}", other.0)),
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn register_agent(_keys: &Keys) -> Agent {
    Agent::NotRegistered
}

/// Opens System Settings at Login Items, where the person allows the agent.
pub fn open_login_items() {
    #[cfg(target_os = "macos")]
    // SAFETY: a class method with no arguments.
    unsafe {
        objc2_service_management::SMAppService::openSystemSettingsLoginItems();
    }
}

/// Whether the screen is locked, or this session is not on the console
/// (another user switched in). `false` when there is no session to ask.
#[cfg(target_os = "macos")]
pub fn screen_locked() -> bool {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::CFString;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
    }
    // SAFETY: the function returns a new dictionary we own, or null.
    let raw = unsafe { CGSessionCopyCurrentDictionary() };
    if raw.is_null() {
        return false;
    }
    // SAFETY: `raw` is a non-null CFDictionary under the create rule.
    let session: CFDictionary<CFString, CFType> =
        unsafe { CFDictionary::wrap_under_create_rule(raw) };
    let flag = |name: &'static str| {
        session
            .find(CFString::from_static_string(name))
            .and_then(|value| value.downcast::<CFBoolean>())
            .map(bool::from)
    };
    flag("CGSSessionScreenIsLocked").unwrap_or(false)
        || flag("kCGSSessionOnConsoleKey") == Some(false)
}

#[cfg(not(target_os = "macos"))]
pub fn screen_locked() -> bool {
    false
}

/// Whether "Reduce motion" is on (Accessibility, Display): the backdrop
/// then shows a still frame.
#[cfg(target_os = "macos")]
pub fn reduce_motion() -> bool {
    objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

#[cfg(not(target_os = "macos"))]
pub fn reduce_motion() -> bool {
    false
}

/// Puts `text` on the clipboard.
pub fn copy(text: &str) -> bool {
    let Ok(mut child) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() else {
        return false;
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
    child.wait().is_ok_and(|status| status.success()) && wrote
}

/// Empties the clipboard if it still holds `text`.
pub fn clear_if(text: &str) {
    let holds = Command::new("pbpaste")
        .output()
        .is_ok_and(|output| output.stdout == text.as_bytes());
    if holds {
        copy("");
    }
}

/// Asks the person for a folder with the system's chooser.
pub fn choose_folder() -> openagents_desktop::folder::Chosen {
    use openagents_desktop::folder::Chosen;
    let Ok(output) = Command::new("osascript")
        .args([
            "-e",
            "POSIX path of (choose folder with prompt \"Choose the folder that holds your code\")",
        ])
        .output()
    else {
        return Chosen::Unavailable;
    };
    if !output.status.success() {
        return Chosen::Cancelled;
    }
    let path = String::from_utf8_lossy(&output.stdout);
    let path = path.trim().trim_end_matches('/');
    if path.is_empty() {
        Chosen::Cancelled
    } else {
        Chosen::Folder(PathBuf::from(path))
    }
}

/// Whether Codex and Claude Code are signed in for this user, and whether
/// Grok Build is installed and signed in.
pub fn signed_in(home: &Path) -> Agents {
    let claude_item = Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    let claude = claude_item || home.join(".claude/.credentials.json").exists();
    Agents {
        codex: openagents_desktop::model::codex_login(home).exists(),
        claude,
        grok: openagents_desktop::model::grok(home),
        claude_problem: openagents_desktop::claude_setup::check(home, claude),
    }
}
