//! The Windows calls behind the crate's rule.

use std::ffi::c_void;
use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt as _;
use std::os::windows::io::AsRawHandle as _;
use std::path::Path;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, FILETIME, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_FILE_OBJECT, SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetLengthSid,
    GetSecurityDescriptorDacl, GetSecurityDescriptorOwner, GetTokenInformation, INHERIT_ONLY_ACE,
    OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_READ_ATTRIBUTES, GetFileInformationByHandle, READ_CONTROL, WRITE_DAC, WRITE_OWNER,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::SystemServices::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::owner_only_sddl;

/// A `LocalAlloc`ed block, freed when dropped.
struct Local(*mut c_void);

impl Drop for Local {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the pointer came from a Win32 call that allocates with
            // `LocalAlloc`, and is freed once.
            unsafe { LocalFree(self.0) };
        }
    }
}

/// The user this process runs as: its SID's bytes and its string form.
struct User {
    sid: Vec<u8>,
    text: String,
}

/// The user this process runs as, read once from its token.
fn user() -> io::Result<&'static User> {
    static USER: OnceLock<Result<User, i32>> = OnceLock::new();
    match USER.get_or_init(|| read_user().map_err(|error| error.raw_os_error().unwrap_or(-1))) {
        Ok(user) => Ok(user),
        Err(code) => Err(io::Error::from_raw_os_error(*code)),
    }
}

fn read_user() -> io::Result<User> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: the current-process pseudo handle; `token` receives a new
    // handle, closed below.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut needed = 0u32;
    // SAFETY: a size query with no buffer.
    unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed) };
    // u64 storage keeps the TOKEN_USER header aligned.
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8).max(1)];
    // SAFETY: the buffer holds at least `needed` bytes.
    let read = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    };
    let error = io::Error::last_os_error();
    // SAFETY: the token handle is open and closed once.
    unsafe { CloseHandle(token) };
    if read == 0 {
        return Err(error);
    }
    // SAFETY: GetTokenInformation(TokenUser) wrote a TOKEN_USER at the
    // start of the buffer, whose SID points into the buffer.
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    // SAFETY: a valid SID in the live buffer.
    let length = unsafe { GetLengthSid(sid) } as usize;
    // SAFETY: the SID is `length` bytes long.
    let bytes = unsafe { std::slice::from_raw_parts(sid.cast::<u8>(), length) }.to_vec();
    let mut text: *mut u16 = std::ptr::null_mut();
    // SAFETY: a valid SID; `text` receives a LocalAlloc'ed string.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let owned = Local(text.cast());
    let mut len = 0;
    // SAFETY: the string is NUL-terminated.
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `len` units precede the terminator.
    let text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) });
    drop(owned);
    Ok(User { sid: bytes, text })
}

/// The string SID of the user this process runs as.
///
/// # Errors
///
/// When the process token cannot be read.
pub fn user_sid() -> io::Result<String> {
    user().map(|user| user.text.clone())
}

/// Opens `options` without following a symbolic link or junction at the
/// last component, as `O_NOFOLLOW` does: a link is opened as itself, and
/// its metadata then fails an `is_file` or `is_dir` check.
pub fn nofollow(options: &mut OpenOptions) -> &mut OpenOptions {
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
}

/// Opens the directory `path` itself, not what a link there points to,
/// with the right to read its security and attributes.
///
/// # Errors
///
/// When it cannot be opened.
pub fn open_dir(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

/// Makes `path` (a file or a directory, not followed through a link) the
/// user's alone: the user owns it, and a protected DACL with one entry
/// grants the user full control, inherited by what is created inside.
///
/// # Errors
///
/// When the object cannot be opened or its security set.
pub fn restrict(path: &Path) -> io::Result<()> {
    let user = user()?;
    let object = OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC | WRITE_OWNER | FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let sddl: Vec<u16> = owner_only_sddl(&user.text)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: a NUL-terminated SDDL string; `descriptor` receives a
    // LocalAlloc'ed security descriptor, freed below.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let _descriptor = Local(descriptor);
    let (mut present, mut defaulted) = (0, 0);
    let mut dacl: *mut ACL = std::ptr::null_mut();
    // SAFETY: a valid descriptor; the DACL pointer points into it.
    if unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut owner: PSID = std::ptr::null_mut();
    // SAFETY: as above, for the owner.
    if unsafe { GetSecurityDescriptorOwner(descriptor, &mut owner, &mut defaulted) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: an open handle with WRITE_DAC and WRITE_OWNER, and an owner
    // and DACL that live in the descriptor for the call.
    let set = unsafe {
        SetSecurityInfo(
            object.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION
                | DACL_SECURITY_INFORMATION
                | PROTECTED_DACL_SECURITY_INFORMATION,
            owner,
            std::ptr::null_mut(),
            dacl,
            std::ptr::null(),
        )
    };
    if set != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(set as i32));
    }
    Ok(())
}

/// Creates the directory `path` and makes it the user's alone
/// ([`restrict`]), as `mkdir` with mode `0700` does. An existing path is
/// [`io::ErrorKind::AlreadyExists`] and is left as it is.
///
/// # Errors
///
/// When it cannot be created or restricted.
pub fn create_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir(path)?;
    restrict(path)
}

/// Creates `path` and any missing parents; each directory this creates is
/// the user's alone.
///
/// # Errors
///
/// When a directory cannot be created or restricted.
pub fn create_dir_all(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        create_dir_all(parent)?;
    }
    match create_dir(path) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => Ok(()),
        other => other,
    }
}

/// Whether the open `object` is the user's alone: the user owns it, and
/// every entry of its DACL that applies to it names the user. A missing
/// (null) DACL grants everyone and is not private. Under Wine, which keeps
/// no file DACL, only the owner is checked ([`under_wine`]). The handle needs
/// `READ_CONTROL`, which a read open and [`open_dir`] carry.
///
/// # Errors
///
/// When its security cannot be read.
pub fn is_private(object: &File) -> io::Result<bool> {
    let user = user()?;
    let mut owner: PSID = std::ptr::null_mut();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: an open handle; the owner and DACL point into `descriptor`,
    // which is LocalAlloc'ed and freed below.
    let read = unsafe {
        GetSecurityInfo(
            object.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if read != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(read as i32));
    }
    let _descriptor = Local(descriptor);
    let me: PSID = user.sid.as_ptr().cast_mut().cast();
    // SAFETY: both SIDs are valid for the call.
    if owner.is_null() || unsafe { EqualSid(owner, me) } == 0 {
        return Ok(false);
    }
    if under_wine() {
        // Wine keeps no DACL on a file: it reports one made up from the
        // Unix mode and ignores a DACL it is given, so only the owner is
        // meaningful there, and the Unix mode is what holds the file.
        return Ok(true);
    }
    if dacl.is_null() {
        return Ok(false);
    }
    // SAFETY: a valid ACL in the live descriptor.
    let count = unsafe { (*dacl).AceCount };
    for index in 0..u32::from(count) {
        let mut ace: *mut c_void = std::ptr::null_mut();
        // SAFETY: `index` is below the ACL's entry count.
        if unsafe { GetAce(dacl, index, &mut ace) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: every ACE starts with a header.
        let header = unsafe { *ace.cast::<ACE_HEADER>() };
        if u32::from(header.AceFlags) & INHERIT_ONLY_ACE != 0 {
            // Applies only to what is created inside, not to this object.
            continue;
        }
        match u32::from(header.AceType) {
            ACCESS_DENIED_ACE_TYPE => {}
            ACCESS_ALLOWED_ACE_TYPE => {
                // SAFETY: an allow entry's SID starts at `SidStart`.
                let sid = unsafe { &raw mut (*ace.cast::<ACCESS_ALLOWED_ACE>()).SidStart };
                // SAFETY: both SIDs are valid for the call.
                if unsafe { EqualSid(sid.cast(), me) } == 0 {
                    return Ok(false);
                }
            }
            // A callback, object, or other entry this rule does not read.
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// Whether this process runs under Wine, which exports `wine_get_version`
/// from its `ntdll`; Windows never does.
pub fn under_wine() -> bool {
    static WINE: OnceLock<bool> = OnceLock::new();
    *WINE.get_or_init(|| {
        let ntdll: Vec<u16> = "ntdll.dll"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: a NUL-terminated module name; ntdll is always loaded.
        let module = unsafe { GetModuleHandleW(ntdll.as_ptr()) };
        // SAFETY: a loaded module and a NUL-terminated symbol name.
        !module.is_null()
            && unsafe { GetProcAddress(module, c"wine_get_version".as_ptr().cast()) }.is_some()
    })
}

/// [`is_private`] for the file or directory `path`, not followed through a
/// link.
///
/// # Errors
///
/// When it cannot be opened or its security read.
pub fn is_private_path(path: &Path) -> io::Result<bool> {
    is_private(&open_dir(path)?)
}

/// What identifies one file and its current contents, where Unix code
/// reads `st_dev`, `st_ino`, `st_nlink`, and the times.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub volume: u32,
    pub index: u64,
    pub links: u32,
    pub size: u64,
    /// Last write and creation times, in 100-nanosecond intervals since
    /// 1601.
    pub written: u64,
    pub created: u64,
}

/// The identity of the open `file`.
///
/// # Errors
///
/// When its information cannot be read.
pub fn identity(file: &File) -> io::Result<Identity> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: an open handle and a structure of the right type.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let time = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    Ok(Identity {
        volume: info.dwVolumeSerialNumber,
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        links: info.nNumberOfLinks,
        size: (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow),
        written: time(info.ftLastWriteTime),
        created: time(info.ftCreationTime),
    })
}

/// The identity of the file or directory `path`, not followed through a
/// link, and its metadata.
///
/// # Errors
///
/// When it cannot be opened or read.
pub fn identity_of(path: &Path) -> io::Result<(Identity, Metadata)> {
    let object = open_dir(path)?;
    Ok((identity(&object)?, object.metadata()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The SDDL of `path`'s owner and DACL, for a failure message.
    fn describe(path: &Path) -> String {
        use windows_sys::Win32::Security::Authorization::ConvertSecurityDescriptorToStringSecurityDescriptorW;
        let object = open_dir(path).unwrap();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let (mut owner, mut dacl) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: as in `is_private`.
        let read = unsafe {
            GetSecurityInfo(
                object.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        if read != ERROR_SUCCESS {
            return format!("unreadable: {read}");
        }
        let _descriptor = Local(descriptor);
        let mut text: *mut u16 = std::ptr::null_mut();
        // SAFETY: a valid descriptor; `text` receives a LocalAlloc'ed string.
        unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                SDDL_REVISION_1,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut text,
                std::ptr::null_mut(),
            )
        };
        let _text = Local(text.cast());
        let mut len = 0;
        // SAFETY: NUL-terminated.
        while !text.is_null() && unsafe { *text.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: `len` units.
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) })
    }

    #[test]
    fn a_created_directory_and_its_files_are_the_users_alone() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("state");
        create_dir(&dir).unwrap();
        assert!(
            is_private_path(&dir).unwrap(),
            "{} {}",
            describe(&dir),
            user_sid().unwrap()
        );
        let path = dir.join("file");
        std::fs::write(&path, b"x").unwrap();
        let file = File::open(&path).unwrap();
        assert!(
            is_private(&file).unwrap(),
            "a file inherits the directory's rule"
        );
        let (identity, metadata) = identity_of(&path).unwrap();
        assert!(metadata.is_file());
        assert_eq!(identity.links, 1);
        assert_eq!(identity.size, 1);
        assert_eq!(identity.index, super::identity(&file).unwrap().index);
        assert!(user_sid().unwrap().starts_with("S-1-"));
        assert_eq!(
            create_dir(&dir).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
    }

    #[test]
    fn a_directory_left_with_its_inherited_entries_is_not_private() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("shared");
        std::fs::create_dir(&dir).unwrap();
        // A temporary directory inherits entries for SYSTEM and
        // Administrators, at least, so it is not the user's alone until
        // restricted.
        if !is_private_path(&dir).unwrap() {
            restrict(&dir).unwrap();
        }
        assert!(is_private_path(&dir).unwrap(), "{}", describe(&dir));
    }
}
