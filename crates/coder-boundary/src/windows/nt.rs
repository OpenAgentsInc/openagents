//! Handle-relative opens that never follow a link, on Windows.
//!
//! The Unix walk opens every child with `openat` relative to its parent's
//! descriptor under `O_NOFOLLOW`. The Windows form is `NtCreateFile` with
//! the parent's handle as `RootDirectory` and `FILE_OPEN_REPARSE_POINT`:
//! a symbolic link, a junction, or any other reparse point is opened as
//! itself, never as what it names, and a parent renamed after its handle
//! was taken cannot redirect the open. The opened handle's attributes are
//! then checked, so a reparse point where a file or directory was listed
//! is refused rather than read.

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};
use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _};
use std::path::{Component, Path, PathBuf};

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
    FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
};
use windows_sys::Win32::Foundation::{
    ERROR_NO_MORE_FILES, HANDLE, INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE, RtlNtStatusToDosError,
    UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_ID_BOTH_DIR_INFO, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_READ_DATA,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, FileIdBothDirectoryInfo,
    FileIdBothDirectoryRestartInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
    OPEN_EXISTING, SYNCHRONIZE,
};
use windows_sys::Win32::System::IO::{DeviceIoControl, IO_STATUS_BLOCK};
use windows_sys::Win32::System::Ioctl::FSCTL_GET_REPARSE_POINT;
use windows_sys::Win32::System::SystemServices::{
    IO_REPARSE_TAG_MOUNT_POINT, IO_REPARSE_TAG_SYMLINK,
};

/// What an open expects to find.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Want {
    /// A directory, opened to be listed and to open children beneath.
    Directory,
    /// A regular file, opened to be read.
    File,
    /// Whatever is there, opened as itself to read its attributes and,
    /// for a reparse point, its reparse data.
    Itself,
}

/// What one open handle is: its identity and attributes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Info {
    pub volume: u32,
    pub index: u64,
    pub attributes: u32,
    pub links: u32,
}

impl Info {
    pub(crate) fn is_reparse(&self) -> bool {
        self.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    pub(crate) fn is_dir(&self) -> bool {
        self.attributes & FILE_ATTRIBUTE_DIRECTORY != 0
    }
}

/// The identity and attributes of an open handle.
pub(crate) fn info(file: &File) -> io::Result<Info> {
    let mut data = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: an open handle and a structure of the right type.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut data) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Info {
        volume: data.dwVolumeSerialNumber,
        index: (u64::from(data.nFileIndexHigh) << 32) | u64::from(data.nFileIndexLow),
        attributes: data.dwFileAttributes,
        links: data.nNumberOfLinks,
    })
}

/// Whether `name` is one plain directory entry: not empty, not `.` or
/// `..`, and free of separators and of `:`, which would name an alternate
/// data stream.
pub(crate) fn plain_name(name: &[u16]) -> bool {
    !name.is_empty()
        && name != [u16::from(b'.')]
        && name != [u16::from(b'.'); 2]
        && !name.iter().any(|&unit| {
            unit == 0
                || unit == u16::from(b'\\')
                || unit == u16::from(b'/')
                || unit == u16::from(b':')
        })
}

/// Opens `name` relative to the open directory `dir`, as itself: a
/// reparse point is never followed. The kind is checked on the handle.
pub(crate) fn open_at(dir: &File, name: &[u16], want: Want) -> io::Result<File> {
    if !plain_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a plain directory entry",
        ));
    }
    let bytes = u16::try_from(name.len() * 2)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the name is too long"))?;
    let unicode = UNICODE_STRING {
        Length: bytes,
        MaximumLength: bytes,
        Buffer: name.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(std::mem::size_of::<OBJECT_ATTRIBUTES>()).unwrap_or(u32::MAX),
        RootDirectory: dir.as_raw_handle(),
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let (access, options) = match want {
        Want::Directory => (
            FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            FILE_DIRECTORY_FILE,
        ),
        Want::File => (
            FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            FILE_NON_DIRECTORY_FILE,
        ),
        Want::Itself => (FILE_READ_ATTRIBUTES | SYNCHRONIZE, 0),
    };
    let mut handle: HANDLE = std::ptr::null_mut();
    let mut status_block = IO_STATUS_BLOCK::default();
    // SAFETY: every pointer names a live local for the call; `handle`
    // receives a new handle owned by the `File` below.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access,
            &attributes,
            &mut status_block,
            std::ptr::null(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_OPEN,
            options | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if status < 0 {
        // SAFETY: a pure status translation.
        let code = unsafe { RtlNtStatusToDosError(status) };
        return Err(io::Error::from_raw_os_error(
            i32::try_from(code).unwrap_or(i32::MAX),
        ));
    }
    // SAFETY: a fresh handle, transferred once.
    let file = unsafe { File::from_raw_handle(handle) };
    checked(file, want)
}

/// The handle, once its attributes say it is the kind that was wanted
/// and not a reparse point.
fn checked(file: File, want: Want) -> io::Result<File> {
    if want == Want::Itself {
        return Ok(file);
    }
    let info = info(&file)?;
    if info.is_reparse() {
        return Err(io::Error::other("is a link"));
    }
    match (want, info.is_dir()) {
        (Want::Directory, false) => Err(io::Error::other("is not a directory")),
        (Want::File, true) => Err(io::Error::other("is a directory")),
        _ => Ok(file),
    }
}

/// Opens the directory `path`, resolving every component beneath its
/// drive or share root relative to its parent's handle, so no component
/// may be a link. `path` must be absolute and canonical.
pub(crate) fn open_dir(path: &Path) -> io::Result<File> {
    let mut root = PathBuf::new();
    let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir if rest.is_empty() => {
                root.push(component.as_os_str());
            }
            Component::Normal(name) => rest.push(name),
            _ => return Err(io::Error::other("the path is not canonical")),
        }
    }
    if !path.is_absolute() || root.as_os_str().is_empty() {
        return Err(io::Error::other("the path is not absolute"));
    }
    let mut dir = open_root(&root)?;
    for name in rest {
        let name: Vec<u16> = name.encode_wide().collect();
        dir = open_at(&dir, &name, Want::Directory)?;
    }
    Ok(dir)
}

/// Opens a drive or share root by its path. A root is never a link.
fn open_root(root: &Path) -> io::Result<File> {
    let wide: Vec<u16> = root
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: a NUL-terminated path; the handle is owned by the `File`.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a fresh handle, transferred once.
    checked(unsafe { File::from_raw_handle(handle) }, Want::Directory)
}

/// One entry of a directory listing, as the listing reports it.
#[derive(Clone, Debug)]
pub(crate) struct Listed {
    pub name: Vec<u16>,
    pub attributes: u32,
    pub file_id: u64,
}

impl Listed {
    pub(crate) fn name_os(&self) -> OsString {
        OsString::from_wide(&self.name)
    }
}

/// The entries of the open directory `dir`, `.` and `..` left out, and
/// whether there were more than `limit`.
pub(crate) fn listing(dir: &File, limit: usize) -> io::Result<(Vec<Listed>, bool)> {
    // u64 storage keeps each record 8-aligned, as the records are.
    let mut buffer = vec![0u64; 64 * 1024 / 8];
    let mut names = Vec::new();
    let mut class = FileIdBothDirectoryRestartInfo;
    loop {
        // SAFETY: an open directory handle and a buffer of the size named.
        let read = unsafe {
            GetFileInformationByHandleEx(
                dir.as_raw_handle(),
                class,
                buffer.as_mut_ptr().cast(),
                u32::try_from(buffer.len() * 8).unwrap_or(u32::MAX),
            )
        };
        if read == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                return Ok((names, false));
            }
            return Err(error);
        }
        class = FileIdBothDirectoryInfo;
        let base = buffer.as_ptr().cast::<u8>();
        let size = buffer.len() * 8;
        let mut offset = 0usize;
        loop {
            let header = std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
            if offset + header > size {
                return Err(io::Error::other("a malformed directory listing"));
            }
            // SAFETY: the record lies within the buffer, checked above.
            let record = unsafe {
                std::ptr::read_unaligned(base.add(offset).cast::<FILE_ID_BOTH_DIR_INFO>())
            };
            let length = record.FileNameLength as usize / 2;
            if offset + header + length * 2 > size {
                return Err(io::Error::other("a malformed directory listing"));
            }
            let mut name = vec![0u16; length];
            // SAFETY: `length` units of name follow the header, in bounds.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    base.add(offset + header),
                    name.as_mut_ptr().cast::<u8>(),
                    length * 2,
                );
            }
            let dots = name == [u16::from(b'.')] || name == [u16::from(b'.'); 2];
            if !dots {
                if names.len() == limit {
                    return Ok((names, true));
                }
                names.push(Listed {
                    name,
                    attributes: record.FileAttributes,
                    file_id: record.FileId as u64,
                });
            }
            if record.NextEntryOffset == 0 {
                break;
            }
            offset += record.NextEntryOffset as usize;
        }
    }
}

/// The longest reparse buffer a reparse point can hold.
const REPARSE_MAX: usize = 16 * 1024;

/// What the reparse point `name` in `dir` names: the substitute name of a
/// symbolic link or junction, or the tag and data of any other kind. It
/// is read from the reparse point itself, never followed.
pub(crate) fn link_target(dir: &File, name: &[u16]) -> io::Result<OsString> {
    let link = open_at(dir, name, Want::Itself)?;
    let mut buffer = vec![0u8; REPARSE_MAX];
    let mut returned = 0u32;
    // SAFETY: an open handle and an output buffer of the size named.
    if unsafe {
        DeviceIoControl(
            link.as_raw_handle(),
            FSCTL_GET_REPARSE_POINT,
            std::ptr::null(),
            0,
            buffer.as_mut_ptr().cast(),
            u32::try_from(buffer.len()).unwrap_or(u32::MAX),
            &mut returned,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    buffer.truncate(returned as usize);
    Ok(parse_reparse(&buffer))
}

/// The target a reparse buffer names. A symbolic link's or junction's
/// substitute name comes back as the path it holds; any other kind comes
/// back as its tag and the hex of its data, so a change to it is still a
/// change to the entry.
pub(crate) fn parse_reparse(buffer: &[u8]) -> OsString {
    let u16_at = |at: usize| {
        buffer
            .get(at..at + 2)
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_at = |at: usize| {
        buffer
            .get(at..at + 4)
            .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let tag = u32_at(0);
    // The header is the tag, the data length, and a reserved word; a
    // symbolic link's data then has a flags word before its path buffer.
    let paths = match tag {
        IO_REPARSE_TAG_SYMLINK => Some(20),
        IO_REPARSE_TAG_MOUNT_POINT => Some(16),
        _ => None,
    };
    if let Some(start) = paths {
        let offset = usize::from(u16_at(8));
        let length = usize::from(u16_at(10));
        if let Some(bytes) = buffer.get(start + offset..start + offset + length) {
            let mut units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            // An absolute target starts with the NT prefix `\??\`, which
            // `std::fs::read_link` spells as the verbatim `\\?\`; so does
            // this, so the two agree. A symbolic link's flags word says
            // whether it is relative.
            let relative = tag == IO_REPARSE_TAG_SYMLINK && u32_at(16) & 1 != 0;
            let nt_prefix = [
                u16::from(b'\\'),
                u16::from(b'?'),
                u16::from(b'?'),
                u16::from(b'\\'),
            ];
            if !relative && units.starts_with(&nt_prefix) {
                units[1] = u16::from(b'\\');
            }
            return OsString::from_wide(&units);
        }
    }
    let mut text = format!("reparse:{tag:08x}:");
    for byte in buffer.get(8..).unwrap_or_default() {
        text.push_str(&format!("{byte:02x}"));
    }
    OsString::from(text)
}

/// Opens the regular file `relative` beneath the directory `root`, every
/// component resolved relative to its parent's handle and none of them a
/// link: a link anywhere on the way is refused, never followed.
pub(crate) fn open_beneath(root: &Path, relative: &Path) -> io::Result<File> {
    let mut names: Vec<Vec<u16>> = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(name) => names.push(name.encode_wide().collect()),
            Component::CurDir => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "the path leaves its directory",
                ));
            }
        }
    }
    let Some((last, parents)) = names.split_last() else {
        return Err(io::Error::other("the path has no file component"));
    };
    // The root itself is trusted, as the Unix open of it by path is.
    let mut dir = open_dir(&root.canonicalize()?)?;
    for name in parents {
        dir = open_at(&dir, name, Want::Directory)?;
    }
    let file = open_at(&dir, last, Want::File)?;
    if info(&file)?.links != 1 {
        return Err(io::Error::other("the file has more than one link"));
    }
    Ok(file)
}
