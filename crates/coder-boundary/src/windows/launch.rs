//! The launcher a Windows boundary wraps its command in: `coder-boundary
//! run --sid SID [--network] -- PROGRAM ARGS…`.
//!
//! On macOS and Linux the backend is a system program (`sandbox-exec`,
//! `bwrap`) that starts the command confined. Windows confines a process
//! from the moment it is created, by the security capabilities passed to
//! `CreateProcessW`, and a standard-library `Command` cannot pass them. So
//! the backend is this small program, installed beside `coder.exe`: it
//! starts the command in the AppContainer the boundary made, hands it only
//! its three standard handles, waits for it, and exits with its code. It
//! runs as the supervised child, so the job object that holds it holds the
//! command too, and a stop ends both.

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::path::Path;

use windows_sys::Win32::Foundation::{
    CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::{SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::SystemServices::SE_GROUP_ENABLED;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess,
    GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
    PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW, UpdateProcThreadAttribute,
    WaitForSingleObject,
};

use super::container::{NETWORK_CAPABILITIES, OwnedSid, sid_from_string};
use crate::cmdline::{command_line, plain_path};

/// The exit code the launcher uses for its own failure, before the
/// command started.
pub const LAUNCH_FAILED: i32 = 125;

/// What the launcher was asked to start.
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub sid: String,
    pub network: bool,
    pub program: OsString,
    pub arguments: Vec<OsString>,
}

/// Reads `run --sid SID [--network] -- PROGRAM ARGS…`.
///
/// # Errors
///
/// A usage message for anything else.
pub fn parse(arguments: &[OsString]) -> Result<Request, String> {
    let usage = || "usage: coder-boundary run --sid SID [--network] -- PROGRAM ARGS...".to_owned();
    let mut rest = arguments.iter();
    if rest.next().map(OsString::as_os_str) != Some(OsStr::new("run")) {
        return Err(usage());
    }
    let mut sid = None;
    let mut network = false;
    loop {
        match rest.next().and_then(|argument| argument.to_str()) {
            Some("--sid") => {
                sid = Some(
                    rest.next()
                        .and_then(|value| value.to_str())
                        .filter(|value| value.starts_with("S-1-15-2-"))
                        .ok_or_else(usage)?
                        .to_owned(),
                );
            }
            Some("--network") => network = true,
            Some("--") => break,
            _ => return Err(usage()),
        }
    }
    let program = rest.next().cloned().ok_or_else(usage)?;
    if !Path::new(&program).is_absolute() {
        return Err("the program must be an absolute path".into());
    }
    Ok(Request {
        sid: sid.ok_or_else(usage)?,
        network,
        program,
        arguments: rest.cloned().collect(),
    })
}

/// Runs the launcher on this process's arguments and returns its exit
/// code: the command's, or [`LAUNCH_FAILED`].
#[must_use]
pub fn main(arguments: &[OsString]) -> i32 {
    let request = match parse(arguments) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("coder-boundary: {message}");
            return LAUNCH_FAILED;
        }
    };
    match start(&request) {
        Ok(code) => code,
        Err(error) => {
            eprintln!(
                "coder-boundary: cannot start {} in the sandbox: {error}",
                Path::new(&request.program).display()
            );
            LAUNCH_FAILED
        }
    }
}

/// An attribute list for `CreateProcessW`, deleted when dropped.
struct Attributes {
    storage: Vec<u64>,
}

impl Attributes {
    fn new(count: u32) -> io::Result<Self> {
        let mut size = 0usize;
        // SAFETY: a size query with no list.
        unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), count, 0, &mut size) };
        let mut storage = vec![0u64; size.div_ceil(8).max(1)];
        // SAFETY: the storage holds `size` bytes.
        if unsafe {
            InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), count, 0, &mut size)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Attributes { storage })
    }

    fn list(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }

    /// Sets one attribute; `value` must outlive the process creation.
    fn set<T>(&mut self, attribute: u32, value: *const T, size: usize) -> io::Result<()> {
        // SAFETY: an initialized list and a value of `size` bytes the
        // caller keeps alive until the process is created.
        if unsafe {
            UpdateProcThreadAttribute(
                self.list(),
                0,
                attribute as usize,
                value.cast(),
                size,
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: an initialized list, deleted once.
        unsafe { DeleteProcThreadAttributeList(self.list()) };
    }
}

/// An inheritable duplicate of one of this process's standard handles,
/// closed when dropped.
struct Inheritable(HANDLE);

impl Drop for Inheritable {
    fn drop(&mut self) {
        // SAFETY: a handle this process duplicated, closed once.
        unsafe { CloseHandle(self.0) };
    }
}

fn inheritable(which: u32) -> Option<Inheritable> {
    // SAFETY: reads this process's standard handle.
    let handle = unsafe { GetStdHandle(which) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return None;
    }
    let mut copy: HANDLE = std::ptr::null_mut();
    // SAFETY: duplicates a live handle within this process.
    let duplicated = unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            handle,
            GetCurrentProcess(),
            &mut copy,
            0,
            1,
            DUPLICATE_SAME_ACCESS,
        )
    };
    (duplicated != 0).then_some(Inheritable(copy))
}

fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

/// Starts the command in the container and waits for it.
fn start(request: &Request) -> io::Result<i32> {
    let container = sid_from_string(&request.sid)?;
    let capability_sids: Vec<OwnedSid> = if request.network {
        NETWORK_CAPABILITIES
            .iter()
            .map(|text| sid_from_string(text))
            .collect::<io::Result<_>>()?
    } else {
        Vec::new()
    };
    let mut capabilities: Vec<SID_AND_ATTRIBUTES> = capability_sids
        .iter()
        .map(|sid| SID_AND_ATTRIBUTES {
            Sid: sid.as_ptr(),
            Attributes: SE_GROUP_ENABLED as u32,
        })
        .collect();
    let security = SECURITY_CAPABILITIES {
        AppContainerSid: container.as_ptr(),
        Capabilities: if capabilities.is_empty() {
            std::ptr::null_mut()
        } else {
            capabilities.as_mut_ptr()
        },
        CapabilityCount: u32::try_from(capabilities.len()).unwrap_or(0),
        Reserved: 0,
    };
    let standard: Vec<Option<Inheritable>> =
        [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
            .into_iter()
            .map(inheritable)
            .collect();
    let handles: Vec<HANDLE> = standard.iter().flatten().map(|handle| handle.0).collect();
    let mut attributes = Attributes::new(2)?;
    attributes.set(
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
        &security,
        std::mem::size_of::<SECURITY_CAPABILITIES>(),
    )?;
    if !handles.is_empty() {
        attributes.set(
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            handles.as_ptr(),
            handles.len() * std::mem::size_of::<HANDLE>(),
        )?;
    }
    let pick = |index: usize| {
        standard[index]
            .as_ref()
            .map_or(std::ptr::null_mut(), |handle| handle.0)
    };
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = u32::try_from(std::mem::size_of::<STARTUPINFOEXW>()).unwrap_or(0);
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = pick(0);
    startup.StartupInfo.hStdOutput = pick(1);
    startup.StartupInfo.hStdError = pick(2);
    startup.lpAttributeList = attributes.list();

    // The command starts in the verbatim-free spelling of its program and
    // working directory, which every program accepts.
    let program = plain_path(Path::new(&request.program));
    let program_wide: Vec<u16> = program.as_os_str().encode_wide().collect();
    let arguments: Vec<Vec<u16>> = request
        .arguments
        .iter()
        .map(|argument| argument.encode_wide().collect())
        .collect();
    let mut line = command_line(&program_wide, &arguments);
    line.push(0);
    let application = wide(program.as_os_str());
    let directory = std::env::current_dir()?;
    let directory = wide(plain_path(&directory).as_os_str());
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: NUL-terminated strings, a mutable command line, an
    // initialized STARTUPINFOEXW whose attribute values all outlive the
    // call, and `process` receiving two handles closed below.
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            i32::from(!handles.is_empty()),
            EXTENDED_STARTUPINFO_PRESENT,
            std::ptr::null(),
            directory.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        )
    };
    if created == 0 {
        return Err(io::Error::last_os_error());
    }
    drop(attributes);
    drop(standard);
    // SAFETY: the new process's handles, each closed once.
    unsafe {
        CloseHandle(process.hThread);
        WaitForSingleObject(process.hProcess, INFINITE);
    }
    let mut code = 0u32;
    // SAFETY: a live process handle.
    let read = unsafe { GetExitCodeProcess(process.hProcess, &mut code) };
    let error = io::Error::last_os_error();
    // SAFETY: closed once.
    unsafe { CloseHandle(process.hProcess) };
    if read == 0 {
        return Err(error);
    }
    // An exit code is a DWORD; a negative NTSTATUS comes back as its bits.
    Ok(code as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_request_names_the_container_the_network_and_the_command() {
        let request = parse(&arguments(&[
            "run",
            "--sid",
            "S-1-15-2-1-2-3",
            "--network",
            "--",
            r"C:\Program Files\Git\usr\bin\bash.exe",
            "-c",
            "true",
        ]))
        .unwrap();
        assert_eq!(request.sid, "S-1-15-2-1-2-3");
        assert!(request.network);
        assert_eq!(request.arguments, arguments(&["-c", "true"]));
        let offline = parse(&arguments(&[
            "run",
            "--sid",
            "S-1-15-2-9",
            "--",
            r"C:\Windows\System32\cmd.exe",
        ]))
        .unwrap();
        assert!(!offline.network);
        assert!(offline.arguments.is_empty());
    }

    #[test]
    fn anything_else_is_refused() {
        for words in [
            &["run", "--", r"C:\x.exe"][..],
            &["run", "--sid", "S-1-5-18", "--", r"C:\x.exe"],
            &["run", "--sid", "S-1-15-2-1", "--", "relative.exe"],
            &["run", "--sid", "S-1-15-2-1", "--open", "--", r"C:\x.exe"],
            &["exec", "--sid", "S-1-15-2-1", "--", r"C:\x.exe"],
            &["run", "--sid", "S-1-15-2-1"],
        ] {
            assert!(parse(&arguments(words)).is_err(), "{words:?}");
        }
    }
}
