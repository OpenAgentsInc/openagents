#![cfg(target_os = "linux")]
use computer_files::{FileError, MAX_BYTES, digest};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn helper(home: &Path, args: &[&str], bytes: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_computer-files"))
        .args(args)
        .env_clear()
        .env("HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(bytes);
    }
    child.wait_with_output().unwrap()
}
fn write(home: &Path, path: &str, bytes: &[u8], overwrite: bool) -> Output {
    helper(
        home,
        &[
            "write",
            path,
            &bytes.len().to_string(),
            &digest(bytes),
            if overwrite { "true" } else { "false" },
        ],
        bytes,
    )
}
fn ok(o: Output) -> Output {
    assert!(
        o.status.success(),
        "status {:?}: {}",
        o.status.code(),
        String::from_utf8_lossy(&o.stderr)
    );
    o
}
fn refused(o: Output, error: FileError) {
    assert_eq!(
        o.status.code(),
        Some(error.exit_code()),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(o.stdout.is_empty());
}
#[test]
fn binary_nested_files_atomic_overwrite_and_identical_create_retry() {
    let t = tempfile::tempdir().unwrap();
    let data: Vec<u8> = (0..=255).cycle().take(8193).collect();
    ok(write(t.path(), "empty", b"", false));
    assert!(
        ok(helper(t.path(), &["read", "empty"], &[]))
            .stdout
            .is_empty()
    );
    ok(write(t.path(), "quote 'té.bin", b"utf8", false));
    assert_eq!(
        ok(helper(t.path(), &["read", "quote 'té.bin"], &[])).stdout,
        b"utf8"
    );
    ok(write(t.path(), "input.bin", &data, false));
    assert_eq!(
        ok(helper(t.path(), &["read", "input.bin"], &[])).stdout,
        data
    );
    ok(write(t.path(), "input.bin", &data, false));
    refused(
        write(t.path(), "input.bin", b"different", false),
        FileError::Exists,
    );
    ok(write(t.path(), "input.bin", b"replacement", true));
    assert_eq!(
        std::fs::read(t.path().join("workspace/input.bin")).unwrap(),
        b"replacement"
    );
    std::fs::create_dir(t.path().join("workspace/nested")).unwrap();
    ok(write(t.path(), "nested/one.bin", &data, false));
    assert_eq!(
        ok(helper(t.path(), &["read", "nested/one.bin"], &[])).stdout,
        data
    );
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(t.path().join("workspace/input.bin"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}
#[test]
fn partial_or_bad_checksum_never_replaces_existing_file() {
    let t = tempfile::tempdir().unwrap();
    ok(write(t.path(), "file", b"original", false));
    refused(
        helper(
            t.path(),
            &["write", "file", "100", &digest(b"partial"), "true"],
            b"partial",
        ),
        FileError::Incomplete,
    );
    refused(
        helper(
            t.path(),
            &["write", "file", "7", &digest(b"another"), "true"],
            b"partial",
        ),
        FileError::Incomplete,
    );
    assert_eq!(
        std::fs::read(t.path().join("workspace/file")).unwrap(),
        b"original"
    );
    assert_eq!(
        std::fs::read_dir(t.path().join("workspace"))
            .unwrap()
            .count(),
        1
    );
}
#[test]
fn traversal_links_devices_fifo_and_missing_paths_are_refused() {
    use std::os::unix::fs::symlink;
    let t = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    ok(write(t.path(), "safe", b"safe", false));
    std::fs::write(outside.path().join("secret"), b"outside").unwrap();
    symlink(outside.path(), t.path().join("workspace/link")).unwrap();
    symlink("/dev/zero", t.path().join("workspace/device")).unwrap();
    for path in [
        "../secret",
        "/etc/passwd",
        "nested/../safe",
        "%2e%2e/secret",
        "a\\b",
        "a//b",
        "./safe",
    ] {
        refused(helper(t.path(), &["read", path], &[]), FileError::Invalid);
    }
    for path in ["link/secret", "device"] {
        refused(helper(t.path(), &["read", path], &[]), FileError::Escape);
        refused(write(t.path(), path, b"bad", true), FileError::Escape);
    }
    let fifo = std::ffi::CString::new(t.path().join("workspace/fifo").to_str().unwrap()).unwrap();
    // SAFETY: This creates a FIFO only inside the disposable fixture.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    refused(helper(t.path(), &["read", "fifo"], &[]), FileError::Special);
    refused(write(t.path(), "fifo", b"bad", true), FileError::Special);
    refused(
        helper(t.path(), &["read", "absent"], &[]),
        FileError::Missing,
    );
    refused(
        write(t.path(), "absent/file", b"bad", true),
        FileError::Missing,
    );
    assert_eq!(
        std::fs::read(outside.path().join("secret")).unwrap(),
        b"outside"
    );
}
#[test]
fn file_and_upload_bounds_are_checked_before_publication() {
    let t = tempfile::tempdir().unwrap();
    ok(write(t.path(), "safe", b"safe", false));
    refused(
        helper(
            t.path(),
            &[
                "write",
                "safe",
                &(MAX_BYTES + 1).to_string(),
                &digest(b""),
                "true",
            ],
            &[],
        ),
        FileError::TooLarge,
    );
    let f = std::fs::File::create(t.path().join("workspace/large")).unwrap();
    f.set_len(MAX_BYTES as u64 + 1).unwrap();
    drop(f);
    refused(
        helper(t.path(), &["read", "large"], &[]),
        FileError::TooLarge,
    );
    assert_eq!(
        std::fs::read(t.path().join("workspace/safe")).unwrap(),
        b"safe"
    );
}

#[test]
fn a_file_open_for_writing_is_refused_until_the_writer_closes() {
    let t = tempfile::tempdir().unwrap();
    ok(write(t.path(), "file", b"complete", false));
    let writer = std::fs::OpenOptions::new()
        .write(true)
        .open(t.path().join("workspace/file"))
        .unwrap();
    refused(helper(t.path(), &["read", "file"], &[]), FileError::Changed);
    drop(writer);
    assert_eq!(
        ok(helper(t.path(), &["read", "file"], &[])).stdout,
        b"complete"
    );
}
