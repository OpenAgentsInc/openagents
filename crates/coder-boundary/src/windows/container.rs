//! The AppContainer a Windows boundary runs its command in, and the file
//! access the boundary grants it.
//!
//! A process in an AppContainer reaches a file only when the file's DACL
//! names the container (or all application packages, as the Windows and
//! Program Files trees do for reading). So the boundary makes one
//! container of its own, with a name never used before, and adds an
//! inheritable entry for it to exactly the paths the policy names:
//! full access to the checkout, the writable paths, and the owned scratch;
//! read and execute to the readable ones. Everything else the user owns,
//! including every protected and sealed path, names no such entry, so the
//! command can neither write it nor, since the container confines reads,
//! read it. Network access is a capability the container holds only when
//! the policy is not offline.
//!
//! The entries are removed, and the container's profile deleted, when the
//! boundary is dropped: after the child is reaped, as the Unix profile file
//! and scratch are.

use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSidToSidW, EXPLICIT_ACCESS_W, GRANT_ACCESS,
    GetEffectiveRightsFromAclW, GetNamedSecurityInfoW, NO_MULTIPLE_TRUSTEE, REVOKE_ACCESS,
    SE_FILE_OBJECT, SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN,
    TRUSTEE_W,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, FreeSid, NO_INHERITANCE, PSECURITY_DESCRIPTOR, PSID,
    SUB_CONTAINERS_AND_OBJECTS_INHERIT,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ALL_ACCESS, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ,
};

/// The SID every AppContainer belongs to: `ALL APPLICATION PACKAGES`.
pub(crate) const ALL_APPLICATION_PACKAGES: &str = "S-1-15-2-1";

/// The capabilities an online container holds: `internetClient`,
/// `internetClientServer`, and `privateNetworkClientServer`.
pub(crate) const NETWORK_CAPABILITIES: [&str; 3] = ["S-1-15-3-1", "S-1-15-3-2", "S-1-15-3-3"];

/// The name of a new container: never one used before on this computer,
/// from the process, the time, and a counter.
pub(crate) fn fresh_name() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!(
        "OpenAgents.Boundary.{}.{:x}.{}",
        std::process::id(),
        nanos,
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// A SID Windows allocated, freed as it was allocated.
struct Sid {
    sid: PSID,
    /// `FreeSid` for a container SID; `LocalFree` for a converted string.
    local: bool,
}

impl Drop for Sid {
    fn drop(&mut self) {
        if self.sid.is_null() {
            return;
        }
        // SAFETY: the SID came from the call its `local` flag names, and
        // is freed once.
        unsafe {
            if self.local {
                LocalFree(self.sid);
            } else {
                FreeSid(self.sid);
            }
        }
    }
}

/// The SID a string names.
pub(crate) fn sid_from_string(text: &str) -> io::Result<OwnedSid> {
    let mut sid: PSID = std::ptr::null_mut();
    let text = wide(text);
    // SAFETY: a NUL-terminated string; `sid` receives a LocalAlloc'ed SID.
    if unsafe { ConvertStringSidToSidW(text.as_ptr(), &mut sid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(OwnedSid(Sid { sid, local: true }))
}

/// A SID converted from its string form, freed when dropped.
pub(crate) struct OwnedSid(Sid);

impl OwnedSid {
    pub(crate) fn as_ptr(&self) -> PSID {
        self.0.sid
    }
}

/// The string form of a SID.
fn sid_string(sid: PSID) -> io::Result<String> {
    let mut text: *mut u16 = std::ptr::null_mut();
    // SAFETY: a valid SID; `text` receives a LocalAlloc'ed string.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut len = 0;
    // SAFETY: the string is NUL-terminated.
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` units precede the terminator.
    let string = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) });
    // SAFETY: allocated by ConvertSidToStringSidW, freed once.
    unsafe { LocalFree(text.cast()) };
    Ok(string)
}

/// One AppContainer profile, and the paths whose DACLs name it.
pub(crate) struct Container {
    name: String,
    sid: Sid,
    text: String,
    granted: Mutex<Vec<PathBuf>>,
}

// SAFETY: the SID is owned by this value alone and only read after
// creation; the granted list is behind a mutex.
unsafe impl Send for Container {}
// SAFETY: as above.
unsafe impl Sync for Container {}

impl std::fmt::Debug for Container {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Container")
            .field("name", &self.name)
            .field("sid", &self.text)
            .finish_non_exhaustive()
    }
}

impl Container {
    /// Makes a new AppContainer profile, under a name never used before.
    pub(crate) fn create() -> io::Result<Self> {
        let name = fresh_name();
        let wide_name = wide(&name);
        let display = wide("OpenAgents boundary");
        let mut sid: PSID = std::ptr::null_mut();
        // SAFETY: NUL-terminated strings, no capabilities, and `sid`
        // receives a SID freed with FreeSid.
        let result = unsafe {
            CreateAppContainerProfile(
                wide_name.as_ptr(),
                display.as_ptr(),
                display.as_ptr(),
                std::ptr::null(),
                0,
                &mut sid,
            )
        };
        if result < 0 {
            // The name is fresh, so an existing profile is a failure too.
            return Err(io::Error::other(format!(
                "cannot create an AppContainer (HRESULT {result:#010x})"
            )));
        }
        let sid = Sid { sid, local: false };
        let text = sid_string(sid.sid)?;
        Ok(Container {
            name,
            sid,
            text,
            granted: Mutex::new(Vec::new()),
        })
    }

    /// The container's SID in string form, for the launcher.
    pub(crate) fn sid(&self) -> &str {
        &self.text
    }

    /// Adds an inheritable entry for the container to `path`: full access
    /// when `write`, else read and execute. A read grant on a path every
    /// AppContainer can already read, as the system's own directories are,
    /// changes nothing there.
    pub(crate) fn grant(&self, path: &Path, write: bool) -> io::Result<()> {
        if !write && readable_by_every_container(path) {
            return Ok(());
        }
        let access = if write {
            FILE_ALL_ACCESS
        } else {
            FILE_GENERIC_READ | FILE_GENERIC_EXECUTE
        };
        set_entry(path, self.sid.sid, access, GRANT_ACCESS)?;
        if let Ok(mut granted) = self.granted.lock()
            && !granted.iter().any(|held| held == path)
        {
            granted.push(path.to_path_buf());
        }
        Ok(())
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        let granted = self
            .granted
            .get_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        for path in granted.iter().rev() {
            if path.exists()
                && let Err(error) = set_entry(path, self.sid.sid, 0, REVOKE_ACCESS)
            {
                eprintln!(
                    "coder-boundary: cannot remove the sandbox's access to {}: {error}",
                    path.display()
                );
            }
        }
        let name = wide(&self.name);
        // SAFETY: a NUL-terminated profile name this value created.
        unsafe { DeleteAppContainerProfile(name.as_ptr()) };
    }
}

/// A trustee naming `sid`.
fn trustee(sid: PSID) -> TRUSTEE_W {
    TRUSTEE_W {
        pMultipleTrustee: std::ptr::null_mut(),
        MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_UNKNOWN,
        ptstrName: sid.cast(),
    }
}

/// `path`'s DACL, and the descriptor that holds it (freed when dropped).
fn dacl_of(path: &Path) -> io::Result<(*mut ACL, Descriptor)> {
    let name = wide_path(path);
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: a NUL-terminated path; the DACL points into `descriptor`,
    // which is LocalAlloc'ed and freed by `Descriptor`.
    let read = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if read != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(read as i32));
    }
    Ok((dacl, Descriptor(descriptor)))
}

struct Descriptor(PSECURITY_DESCRIPTOR);

impl Drop for Descriptor {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: LocalAlloc'ed by GetNamedSecurityInfoW, freed once.
            unsafe { LocalFree(self.0) };
        }
    }
}

/// Whether `path`'s DACL already lets every AppContainer read it.
fn readable_by_every_container(path: &Path) -> bool {
    let Ok((dacl, _descriptor)) = dacl_of(path) else {
        return false;
    };
    let Ok(all) = sid_from_string(ALL_APPLICATION_PACKAGES) else {
        return false;
    };
    let trustee = trustee(all.as_ptr());
    let mut rights = 0u32;
    // SAFETY: a valid DACL and trustee for the call.
    let read = unsafe { GetEffectiveRightsFromAclW(dacl, &trustee, &mut rights) };
    let wanted = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
    read == ERROR_SUCCESS && rights & wanted == wanted
}

/// Applies one explicit entry for `sid` to `path`'s DACL, the way the
/// Windows documentation modifies an object's ACL: read it, merge the
/// entry, and set it back, which propagates an inheritable entry, or its
/// removal, to everything beneath.
fn set_entry(path: &Path, sid: PSID, access: u32, mode: i32) -> io::Result<()> {
    let (dacl, _descriptor) = dacl_of(path)?;
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: access,
        grfAccessMode: mode,
        grfInheritance: if path.is_dir() {
            SUB_CONTAINERS_AND_OBJECTS_INHERIT
        } else {
            NO_INHERITANCE
        },
        Trustee: trustee(sid),
    };
    let mut merged: *mut ACL = std::ptr::null_mut();
    // SAFETY: one entry, the object's DACL, and `merged` receiving a
    // LocalAlloc'ed ACL freed below.
    let result = unsafe { SetEntriesInAclW(1, &entry, dacl, &mut merged) };
    if result != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    let name = wide_path(path);
    // SAFETY: a NUL-terminated path and a valid ACL.
    let set = unsafe {
        SetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            merged,
            std::ptr::null(),
        )
    };
    // SAFETY: allocated by SetEntriesInAclW, freed once.
    unsafe { LocalFree(merged.cast::<c_void>()) };
    if set != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(set as i32));
    }
    Ok(())
}
