#[cfg(target_os = "linux")]
use computer_files::{FileError, MAX_BYTES, digest, validate_path};
#[cfg(target_os = "linux")]
#[path = "../src/linux.rs"]
mod linux;
#[cfg(target_os = "linux")]
fn main() {
    use std::io::{BufRead, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    if let Some(home) = std::env::args().nth(1) {
        let root = linux::workspace(&home, false).unwrap();
        let parent = std::ffi::CString::new(format!("{home}/workspace/nested")).unwrap();
        // SAFETY: Open the disposable directory before it is moved by the parent.
        let raw = unsafe {
            libc::open(
                parent.as_ptr(),
                libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        assert!(raw >= 0);
        // SAFETY: The descriptor is new and owned by this process.
        let dir = unsafe { OwnedFd::from_raw_fd(raw) };
        linux::restrict(root.as_raw_fd()).unwrap();
        println!("ready");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_exact(&mut [0u8]).unwrap();
        // SAFETY: Attempt a file creation through the now-moved directory handle.
        let raw = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                c"escaped".as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                0o600,
            )
        };
        assert_eq!(raw, -1, "directory handle escaped after rename");
        assert!(matches!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EACCES | libc::EPERM)
        ));
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("workspace/nested")).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg(home.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut ready = String::new();
    stdout.read_line(&mut ready).unwrap();
    assert_eq!(ready.trim(), "ready");
    std::fs::rename(
        home.path().join("workspace/nested"),
        outside.path().join("moved"),
    )
    .unwrap();
    child.stdin.take().unwrap().write_all(b"x").unwrap();
    assert!(child.wait().unwrap().success());
    assert!(!outside.path().join("moved/escaped").exists());
    println!(
        "PASS: a directory moved outside the workspace cannot be written through its old descriptor"
    );
}
#[cfg(not(target_os = "linux"))]
fn main() {}
