//! The isolated Linux workspace file helper. Errors never contain paths or data.
use computer_files::{FileError, MAX_BYTES, digest, valid_digest, validate_path};
use std::io::{Read, Write};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
fn run() -> Result<(), FileError> {
    use std::os::fd::AsRawFd;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [operation, path, rest @ ..] = args.as_slice() else {
        return Err(FileError::Invalid);
    };
    validate_path(path)?;
    let write = operation == "write";
    if operation != "read" && !write {
        return Err(FileError::Invalid);
    }
    let home = std::env::var("HOME").map_err(|_| FileError::Unavailable)?;
    let root = linux::workspace(&home, write)?;
    linux::restrict(root.as_raw_fd())?;
    if write {
        let [size, sha256, overwrite] = rest else {
            return Err(FileError::Invalid);
        };
        let size: usize = size.parse().map_err(|_| FileError::Invalid)?;
        if size > MAX_BYTES {
            return Err(FileError::TooLarge);
        }
        if !valid_digest(sha256) || !matches!(overwrite.as_str(), "true" | "false") {
            return Err(FileError::Invalid);
        }
        let mut bytes = Vec::with_capacity(size);
        std::io::stdin()
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| FileError::Incomplete)?;
        if bytes.len() != size || digest(&bytes) != *sha256 {
            return Err(FileError::Incomplete);
        }
        linux::write(root.as_raw_fd(), path, &bytes, overwrite == "true")
    } else {
        if !rest.is_empty() {
            return Err(FileError::Invalid);
        }
        let bytes = linux::read(root.as_raw_fd(), path)?;
        std::io::stdout()
            .lock()
            .write_all(&bytes)
            .map_err(|_| FileError::Unavailable)
    }
}
#[cfg(not(target_os = "linux"))]
fn run() -> Result<(), FileError> {
    Err(FileError::Unsupported)
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e.message());
        std::process::exit(e.exit_code());
    }
}
