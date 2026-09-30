//! The local control channel on Windows: a named pipe that only the user
//! running the host can open.
//!
//! On macOS and Linux the control channel is a Unix socket, `0600` in a
//! `0700` directory, and every accepted peer's user ID must equal the
//! host's ([`super::socket`]). Windows has neither mode bits nor
//! `SO_PEERCRED`, so the same rule is kept in two layers here:
//!
//! 1. The pipe is created with a protected DACL whose only entry grants the
//!    host's own user SID access (`D:P(A;;GA;;;<sid>)`), owned by that SID.
//!    No inherited entry, no `Administrators`, no `SYSTEM`, no `Everyone`.
//!    Remote clients are rejected (`PIPE_REJECT_REMOTE_CLIENTS`), and the
//!    first instance is created with `FILE_FLAG_FIRST_PIPE_INSTANCE`, so
//!    another process cannot squat on the name before the host binds it.
//! 2. Every accepted client is checked again: the host reads the client's
//!    process ID from the pipe, reads that process's token user, and serves
//!    it only if the SID equals its own. This is the Windows counterpart of
//!    the `SO_PEERCRED` / `getpeereid` check, and it refuses a peer the DACL
//!    somehow let through (for example, a handle duplicated into another
//!    user's process).
//!
//! The pipe's name carries the user's SID, so two users signed in to one
//! machine each get their own pipe, and a user never dials another's.
//!
//! The pure parts (names, the security descriptor, the admit rule) are
//! compiled everywhere so their tests run on every platform; the pipe
//! itself is `cfg(windows)`. The host serves it from [`super::serve`] with
//! the same protocol as the socket, and the desktop app dials it
//! (`openagents-desktop`'s `platform/windows.rs`, same name). The Windows
//! code is checked with `cargo clippy --target x86_64-pc-windows-gnu`, and
//! its tests run under Wine.

use std::fmt;

/// The prefix every local named pipe carries.
pub const PIPE_PREFIX: &str = r"\\.\pipe\";

/// The stem of the control pipe's name; the user's SID follows it.
pub const PIPE_STEM: &str = "openagents-control-";

/// Why a control peer was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The peer's process runs as a different user.
    OtherUser { peer: String },
    /// The peer's user could not be read (the process exited, or its token
    /// could not be opened). An unknown peer is never served.
    Unknown,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OtherUser { peer } => {
                write!(f, "control peer runs as {peer}, not as this host's user")
            }
            Self::Unknown => f.write_str("control peer's user could not be read"),
        }
    }
}

impl std::error::Error for Refusal {}

/// Whether `sid` is a well-formed string SID: `S-1-<authority>-<sub>...`,
/// decimal fields only, with at least one and at most fifteen
/// subauthorities. Anything else is refused before it can reach an SDDL
/// string or a pipe name.
pub fn is_valid_sid(sid: &str) -> bool {
    let Some(rest) = sid.strip_prefix("S-1-") else {
        return false;
    };
    let fields: Vec<&str> = rest.split('-').collect();
    // One identifier authority plus 1..=15 subauthorities (SID_MAX_SUB_AUTHORITIES).
    if fields.len() < 2 || fields.len() > 16 {
        return false;
    }
    fields.iter().enumerate().all(|(index, field)| {
        !field.is_empty()
            && field.len() <= 20
            && field.bytes().all(|b| b.is_ascii_digit())
            && if index == 0 {
                field.parse::<u64>().is_ok_and(|v| v < (1u64 << 48))
            } else {
                field.parse::<u32>().is_ok()
            }
    })
}

/// The control pipe's full name for the user `sid`.
pub fn pipe_name(sid: &str) -> Option<String> {
    is_valid_sid(sid).then(|| format!("{PIPE_PREFIX}{PIPE_STEM}{sid}"))
}

/// Whether `name` is a local pipe name with one component after
/// [`PIPE_PREFIX`].
pub fn is_local_pipe_name(name: &str) -> bool {
    name.strip_prefix(PIPE_PREFIX)
        .is_some_and(|rest| !rest.is_empty() && !rest.contains(['\\', '/']) && rest.len() <= 200)
}

/// The security descriptor, in SDDL, that the pipe is created with: owned by
/// `sid`, and a protected DACL (`P`: nothing inherited) with one entry that
/// allows `sid` generic-all. No other principal appears, so no other user,
/// not even an administrator or `SYSTEM`, is granted an open.
pub fn security_descriptor(sid: &str) -> Option<String> {
    is_valid_sid(sid).then(|| format!("O:{sid}D:P(A;;GA;;;{sid})"))
}

/// The admit rule applied to every accepted client, the same rule as the
/// Unix socket's peer user ID check: serve only a peer whose user SID
/// equals the host's. SIDs compare case-insensitively, as Windows does.
pub fn admit(host_sid: &str, peer_sid: Option<&str>) -> Result<(), Refusal> {
    match peer_sid {
        None => Err(Refusal::Unknown),
        Some(peer) if peer.eq_ignore_ascii_case(host_sid) && is_valid_sid(peer) => Ok(()),
        Some(peer) => Err(Refusal::OtherUser {
            peer: peer.to_string(),
        }),
    }
}

#[cfg(windows)]
pub use imp::{ControlPipe, current_user_sid};

#[cfg(windows)]
mod imp {
    use super::{Refusal, admit, is_local_pipe_name, pipe_name, security_descriptor};
    use std::ffi::c_void;
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    };
    use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    /// Closes a Win32 handle when dropped.
    struct Handle(HANDLE);

    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: the handle was returned open by Win32 and is closed once.
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    /// A `LocalAlloc`ed block, freed with `LocalFree` when dropped.
    struct Local(*mut c_void);

    impl Drop for Local {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: the pointer was allocated by Win32 with LocalAlloc.
                unsafe { LocalFree(self.0) };
            }
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The string SID of the user a token belongs to.
    fn token_user_sid(token: &Handle) -> io::Result<String> {
        let mut needed = 0u32;
        // SAFETY: a size query with a null buffer; `needed` receives the size.
        unsafe { GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut needed) };
        if needed == 0 {
            return Err(io::Error::last_os_error());
        }
        // u64 storage keeps the TOKEN_USER header suitably aligned.
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
        // SAFETY: the buffer holds at least `needed` bytes.
        let ok = unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: GetTokenInformation(TokenUser) wrote a TOKEN_USER at the start.
        let user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
        let mut text: *mut u16 = std::ptr::null_mut();
        // SAFETY: the SID points into `buffer`, alive for this call.
        if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let text = Local(text.cast());
        let chars = text.0.cast::<u16>();
        let mut len = 0usize;
        // SAFETY: ConvertSidToStringSidW returns a NUL-terminated string.
        while unsafe { *chars.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: `len` UTF-16 units precede the terminator.
        let slice = unsafe { std::slice::from_raw_parts(chars, len) };
        Ok(String::from_utf16_lossy(slice))
    }

    /// The string SID of the user this process runs as.
    pub fn current_user_sid() -> io::Result<String> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: GetCurrentProcess is a pseudo handle; `token` receives a new handle.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        token_user_sid(&Handle(token))
    }

    /// The string SID of the user the client on `pipe` runs as.
    fn client_sid(pipe: &NamedPipeServer) -> io::Result<String> {
        let mut pid = 0u32;
        // SAFETY: a connected server handle; `pid` receives the client's process ID.
        if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle() as HANDLE, &mut pid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: opening a process by ID for a limited query only.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return Err(io::Error::last_os_error());
        }
        let process = Handle(process);
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: `process` is open with query rights.
        if unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        token_user_sid(&Handle(token))
    }

    /// The host's control pipe: a listening instance, recreated after every
    /// accept so a second client can always connect.
    pub struct ControlPipe {
        name: String,
        sid: String,
        next: NamedPipeServer,
    }

    impl ControlPipe {
        /// Creates the first instance of this user's control pipe. Fails if
        /// any process already holds the name.
        pub fn bind() -> io::Result<Self> {
            let sid = current_user_sid()?;
            let name = pipe_name(&sid).ok_or_else(|| io::Error::other("malformed user SID"))?;
            Self::bind_at(&name)
        }

        /// Creates the first instance of a control pipe named `name`, with
        /// the same DACL and client check as [`ControlPipe::bind`]; a test
        /// uses its own name so it never meets a running host. The name
        /// must be a local pipe name, `\\.\pipe\` and one component.
        pub fn bind_at(name: &str) -> io::Result<Self> {
            if !is_local_pipe_name(name) {
                return Err(io::Error::other("not a local pipe name"));
            }
            let sid = current_user_sid()?;
            let next = create(name, &sid, true)?;
            Ok(Self {
                name: name.to_string(),
                sid,
                next,
            })
        }

        /// The pipe's full name, for the client to dial.
        pub fn name(&self) -> &str {
            &self.name
        }

        /// Waits for a client, checks that it runs as this user, and returns
        /// the connected pipe. A refused client is disconnected and the wait
        /// continues; the refusal is returned to the caller's log through
        /// `on_refused`.
        pub async fn accept(
            &mut self,
            mut on_refused: impl FnMut(&Refusal),
        ) -> io::Result<NamedPipeServer> {
            loop {
                self.next.connect().await?;
                let fresh = create(&self.name, &self.sid, false)?;
                let connected = std::mem::replace(&mut self.next, fresh);
                let peer = client_sid(&connected).ok();
                match admit(&self.sid, peer.as_deref()) {
                    Ok(()) => return Ok(connected),
                    Err(refusal) => {
                        on_refused(&refusal);
                        let _ = connected.disconnect();
                    }
                }
            }
        }
    }

    /// Creates one pipe instance whose DACL admits only `sid`.
    fn create(name: &str, sid: &str, first: bool) -> io::Result<NamedPipeServer> {
        let sddl =
            security_descriptor(sid).ok_or_else(|| io::Error::other("malformed user SID"))?;
        let sddl = wide(&sddl);
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: a NUL-terminated SDDL string; `descriptor` receives a LocalAlloc block.
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let descriptor = Local(descriptor);
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(first)
            .reject_remote_clients(true);
        // SAFETY: `attributes` and the descriptor it points to outlive the call.
        unsafe {
            options.create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "S-1-5-21-3623811015-3361044348-30300820-1013";
    const OTHER: &str = "S-1-5-21-3623811015-3361044348-30300820-1014";

    #[test]
    fn control_pipe_sid_shapes() {
        assert!(is_valid_sid(ME));
        assert!(is_valid_sid("S-1-5-18"));
        for bad in [
            "",
            "S-1-",
            "S-1-5",
            "S-2-5-18",
            "s-1-5-18",
            "S-1-5-18-",
            "S-1-5--18",
            "S-1-5-18)(A;;GA;;;WD",
            "S-1-5-18;WD",
            "S-1-5-4294967296",
            "S-1-281474976710656-1",
            "S-1-5-1-2-3-4-5-6-7-8-9-10-11-12-13-14-15-16",
        ] {
            assert!(!is_valid_sid(bad), "accepted {bad:?}");
        }
    }

    #[test]
    fn control_pipe_name_is_per_user() {
        assert_eq!(
            pipe_name(ME).unwrap(),
            format!(r"\\.\pipe\openagents-control-{ME}")
        );
        assert_ne!(pipe_name(ME), pipe_name(OTHER));
        assert_eq!(pipe_name(r"S-1-5-18\..\x"), None);
    }

    #[test]
    fn control_pipe_dacl_names_only_the_user() {
        let sddl = security_descriptor(ME).unwrap();
        assert_eq!(sddl, format!("O:{ME}D:P(A;;GA;;;{ME})"));
        // Protected (nothing inherited), exactly one allow entry, no deny
        // entries that could be reordered, and no well-known groups.
        assert!(sddl.contains("D:P("));
        assert_eq!(sddl.matches("(A;").count(), 1);
        assert!(!sddl.contains("(D;"));
        for group in ["WD", "BA", "SY", "AU", "IU", "AN", "BU"] {
            assert!(!sddl.contains(&format!(";;;{group})")), "grants {group}");
        }
        // An injected SID never reaches the descriptor.
        assert_eq!(security_descriptor("S-1-5-18)(A;;GA;;;WD"), None);
    }

    #[test]
    fn control_pipe_names_are_local_and_single() {
        assert!(is_local_pipe_name(&pipe_name(ME).unwrap()));
        assert!(is_local_pipe_name(r"\\.\pipe\openagents-control-test-1"));
        for bad in [
            r"\\.\pipe\",
            r"\\server\pipe\openagents-control",
            r"\\.\pipe\a\b",
            r"\\.\pipe\a/b",
            "openagents-control",
            "/tmp/control.sock",
        ] {
            assert!(!is_local_pipe_name(bad), "accepted {bad:?}");
        }
    }

    #[test]
    fn control_pipe_admits_only_the_same_user() {
        assert_eq!(admit(ME, Some(ME)), Ok(()));
        assert_eq!(
            admit(ME, Some(OTHER)),
            Err(Refusal::OtherUser { peer: OTHER.into() })
        );
        assert_eq!(
            admit(ME, Some("S-1-5-18")),
            Err(Refusal::OtherUser {
                peer: "S-1-5-18".into()
            })
        );
        assert_eq!(admit(ME, None), Err(Refusal::Unknown));
        assert!(admit(ME, Some("")).is_err());
    }

    /// On Windows: the host binds this user's pipe, a client of the same
    /// user connects and is admitted, and a second bind of the name fails
    /// (no squatting while the host holds it).
    #[cfg(windows)]
    #[tokio::test]
    async fn control_pipe_admits_this_user_on_windows() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::windows::named_pipe::ClientOptions;

        let mut pipe = ControlPipe::bind().unwrap();
        let sid = current_user_sid().unwrap();
        assert!(is_valid_sid(&sid));
        assert_eq!(pipe.name(), pipe_name(&sid).unwrap());
        // Wine does not enforce FILE_FLAG_FIRST_PIPE_INSTANCE; Windows does.
        if std::env::var_os("OPENAGENTS_TEST_UNDER_WINE").is_none() {
            assert!(
                ControlPipe::bind().is_err(),
                "a second host bound the same pipe"
            );
        }

        let name = pipe.name().to_string();
        let client = tokio::spawn(async move {
            let mut client = ClientOptions::new().open(&name).unwrap();
            client.write_all(b"ping").await.unwrap();
            let mut reply = [0u8; 4];
            client.read_exact(&mut reply).await.unwrap();
            reply
        });
        let mut refused = Vec::new();
        let mut server = pipe.accept(|r| refused.push(r.clone())).await.unwrap();
        let mut request = [0u8; 4];
        server.read_exact(&mut request).await.unwrap();
        server.write_all(b"pong").await.unwrap();
        assert_eq!(&request, b"ping");
        assert_eq!(&client.await.unwrap(), b"pong");
        assert!(refused.is_empty());
    }
}
