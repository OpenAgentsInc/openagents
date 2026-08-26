//! Concurrent writers to one config directory.
//!
//! Every private file this CLI keeps — the credential store, the pending
//! device authorizations, the identity seed, `computer.json` — is written by a
//! process that has no claim on the machine. A fleet of agents runs under one
//! `$HOME` by design: the delegation engine starts children and each one
//! carries a credential. Two `oa auth login` runs at once are the same thing at
//! human speed.
//!
//! These writes used to stage through a name derived only from the target —
//! `path.with_extension("tmp")` — so every writer of a given file shared one
//! staging path. They truncated and renamed each other's half-written bytes:
//! one won, and the other's `rename` found nothing and reported a failed
//! credential write for a credential that may well have been stored. A caller
//! that cannot tell whether its own token landed has no move left.
//!
//! So each test here runs N writers against one directory and demands two
//! things a shared staging name cannot give: **every writer reports success**,
//! and **the file left behind is whole**. `computer.json` had it worse still —
//! it wrote in place, with no staging at all — so its test adds concurrent
//! readers, which is what makes the truncate window visible. Put any of these
//! writers back the way it was and the matching test fails.

use openagents_cli::auth::{CredentialStore, PendingDeviceAuthorization, PendingStore, Secret};
use openagents_cli::computer::{ComputerPaths, PolicyConfig};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::thread;

/// Enough writers to overlap on any machine that runs this, and few enough
/// that spawning them as processes stays quick.
const WRITERS: usize = 12;

// ---------------------------------------------------------------------------
// the reported failure: separate `oa` processes, one config directory
// ---------------------------------------------------------------------------

/// A server that answers every request with one canned device authorization.
///
/// `oa auth login --headless` starts an authorization, prints the code, writes
/// it to `device-authorizations.json`, and exits — no polling, so a run of it
/// is a short process whose only side effect is that write. That makes it the
/// honest way to put N real `oa` processes on one file at once.
fn stub_authorization_server() -> String {
    const BODY: &str = r#"{"device_code":"d-race","user_code":"AAAA-BBBB",
        "verification_uri":"https://example.test/device",
        "verification_uri_complete":"https://example.test/device?user_code=AAAA-BBBB",
        "expires_in":600,"interval":5,"scope":"forge:write"}"#;
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a port");
    let port = listener.local_addr().expect("read the port").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().expect("clone the stream"));
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    return;
                }
                let mut length = 0usize;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap_or(0) == 0 || header.trim().is_empty() {
                        break;
                    }
                    if let Some(value) = header.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                if length > 0 {
                    let mut discard = vec![0u8; length];
                    let _ = reader.read_exact(&mut discard);
                }
                let response = format!(
                    "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{BODY}",
                    BODY.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            });
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// N `oa` processes sharing a config directory each store their authorization,
/// and none of them is told the write failed.
///
/// This is the reported bug end to end. With the staging name derived only
/// from the target, running only the two scope tests in `tests/flags.rs` —
/// which is enough to make two `oa` runs overlap — failed 29 times in 30 with
/// `could not write .../device-authorizations.json: No such file or directory`.
/// Twelve deliberate writers make that certain rather than likely.
#[test]
fn concurrent_oa_processes_all_record_their_authorization() {
    let home = tempfile::tempdir().expect("a home of this test's own");
    let origin = stub_authorization_server();

    let runs: Vec<_> = (0..WRITERS)
        .map(|_| {
            let home = home.path().to_path_buf();
            let origin = origin.clone();
            thread::spawn(move || {
                Command::new(env!("CARGO_BIN_EXE_oa"))
                    .args(["--api-url", &origin, "auth", "login", "--headless"])
                    .env("NO_COLOR", "")
                    .env("HOME", &home)
                    .output()
                    .expect("run oa")
            })
        })
        .collect();

    for (index, run) in runs.into_iter().enumerate() {
        let output = run.join().expect("an oa process finished");
        assert_eq!(
            output.status.code(),
            Some(0),
            "writer {index} was told its authorization was not stored: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let path = home
        .path()
        .join(".config")
        .join("openagents")
        .join("device-authorizations.json");
    let stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("the file is on disk"))
            .expect("concurrent writers left whole JSON behind");
    assert_eq!(
        stored["authorizations"][&origin]["user_code"], "AAAA-BBBB",
        "the file survived but holds no authorization for the origin every writer used: {stored}"
    );
    assert_no_staging_files_left(path.parent().expect("the config directory"));
}

// ---------------------------------------------------------------------------
// the same overlap, one writer per thread, for each store in turn
// ---------------------------------------------------------------------------

/// Run `writer` on `WRITERS` threads at once and return what each one reported.
fn race<T, E>(writer: impl Fn(usize) -> Result<T, E> + Send + Sync + 'static) -> Vec<Result<T, E>>
where
    T: Send + 'static,
    E: Send + 'static,
{
    let writer = std::sync::Arc::new(writer);
    // A barrier rather than "spawn and hope": the point is that the writes
    // overlap, and a thread that starts after another has finished proves
    // nothing.
    let gate = std::sync::Arc::new(std::sync::Barrier::new(WRITERS));
    let threads: Vec<_> = (0..WRITERS)
        .map(|index| {
            let writer = writer.clone();
            let gate = gate.clone();
            thread::spawn(move || {
                gate.wait();
                writer(index)
            })
        })
        .collect();
    threads
        .into_iter()
        .map(|thread| thread.join().expect("a writer thread finished"))
        .collect()
}

/// No `.tmp` litter: a staging file left behind is a write that half happened,
/// and in this directory it is a half-written credential sitting on disk.
fn assert_no_staging_files_left(directory: &Path) {
    let left: Vec<String> = std::fs::read_dir(directory)
        .expect("read the directory")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(
        left.is_empty(),
        "staging files were left in {}: {left:?}",
        directory.display()
    );
}

#[cfg(unix)]
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .expect("stat the path")
        .permissions()
        .mode()
        & 0o777
}

/// Concurrent token writes all report where the token landed, and the store is
/// still readable afterwards.
///
/// Each writer uses its own origin, so the file is also the one place the
/// writes meet. The assertion is not that every origin survives — these are
/// read-modify-write callers and the last one legitimately wins — but that no
/// writer was told its token failed, and that the file the survivor left is a
/// credential store rather than two of them spliced together.
#[test]
fn concurrent_credential_writes_all_succeed_and_leave_a_readable_store() {
    let directory = tempfile::tempdir().expect("a config directory");
    let at = directory.path().to_path_buf();

    let results = race(move |index| {
        CredentialStore::isolated(&format!("https://writer-{index}.test"), &at)
            .store(&Secret::new(format!("token-for-{index}")))
    });
    for (index, result) in results.iter().enumerate() {
        assert!(
            result.is_ok(),
            "writer {index} was told its token was not stored: {}",
            result.as_ref().unwrap_err()
        );
    }

    let path = directory.path().join("credentials.json");
    let stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("the store is on disk"))
            .expect("concurrent writers left whole JSON behind");
    let tokens = stored["tokens"]
        .as_object()
        .expect("the store holds a token map");
    assert!(
        !tokens.is_empty(),
        "every writer reported success and the store holds nothing: {stored}"
    );
    for (origin, token) in tokens {
        let index = origin
            .trim_start_matches("https://writer-")
            .trim_end_matches(".test");
        assert_eq!(
            token,
            &serde_json::json!(format!("token-for-{index}")),
            "the store pairs {origin} with a token no writer wrote, so two writes were spliced"
        );
    }
    #[cfg(unix)]
    {
        assert_eq!(
            mode_of(&path),
            0o600,
            "the store is readable to the machine"
        );
        assert_eq!(
            mode_of(directory.path()),
            0o700,
            "the config directory is open to the machine"
        );
    }
    assert_no_staging_files_left(directory.path());
}

/// The same for the half-finished logins, which is the file the reported
/// failure actually named.
#[test]
fn concurrent_pending_authorization_writes_all_succeed() {
    let directory = tempfile::tempdir().expect("a config directory");
    let path = directory.path().join("device-authorizations.json");
    let at = path.clone();

    let results = race(move |index| {
        PendingStore::at(at.clone()).set(&PendingDeviceAuthorization {
            origin: format!("https://writer-{index}.test"),
            device_code: format!("device-{index}"),
            user_code: format!("CODE-{index:04}"),
            verification_uri: "https://example.test/device".to_string(),
            verification_uri_complete: "https://example.test/device?user_code=X".to_string(),
            expires_at_ms: 1_000_000,
            interval: 5,
            kind: None,
        })
    });
    for (index, result) in results.iter().enumerate() {
        assert!(
            result.is_ok(),
            "writer {index} was told its authorization was not stored: {}",
            result.as_ref().unwrap_err()
        );
    }

    let stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("the file is on disk"))
            .expect("concurrent writers left whole JSON behind");
    assert!(
        !stored["authorizations"]
            .as_object()
            .expect("the file holds an authorization map")
            .is_empty(),
        "every writer reported success and the file holds nothing: {stored}"
    );
    #[cfg(unix)]
    assert_eq!(mode_of(&path), 0o600, "the file is readable to the machine");
    assert_no_staging_files_left(directory.path());
}

#[test]
fn a_reader_never_sees_a_half_written_computer_policy() {
    /// Enough passes for the truncate window to be observed if it is open.
    const PASSES: usize = 40;

    let directory = tempfile::tempdir().expect("a config directory");
    let paths = ComputerPaths::in_directory(directory.path());
    // Seed the file, so a reader finding it absent means it was unlinked rather
    // than never written.
    let mut initial = PolicyConfig::closed(paths.clone());
    initial.pre_approved = vec!["writer-initial".to_string()];
    openagents_cli::computer::write_config(&initial).expect("the first write lands");

    let at = paths.clone();
    let results = race(move |index| {
        // Half write and half read, so both are going at once.
        if index.is_multiple_of(2) {
            for pass in 0..PASSES {
                let mut config = PolicyConfig::closed(at.clone());
                // Lengths differ between writers, so an interleave leaves a tail
                // rather than a file that happens to be the same size.
                config.pre_approved = (0..=index * pass % 7)
                    .map(|n| format!("writer-{index}-pass-{pass}-entry-{n}"))
                    .collect();
                openagents_cli::computer::write_config(&config)
                    .map_err(|error| format!("writer {index} pass {pass}: {error}"))?;
            }
        } else {
            for pass in 0..PASSES {
                openagents_cli::computer::load_config(&at)
                    .map(|_| ())
                    .map_err(|error| format!("reader {index} pass {pass}: {error}"))?;
            }
        }
        Ok::<(), String>(())
    });
    for result in &results {
        assert!(
            result.is_ok(),
            "a concurrent run of the policy file failed: {}",
            result.as_ref().unwrap_err()
        );
    }

    let settled = openagents_cli::computer::load_config(&paths)
        .expect("the policy the writers left behind still decodes");
    assert!(
        settled
            .pre_approved
            .iter()
            .all(|entry| entry.starts_with("writer-")),
        "the policy holds an entry no writer wrote, so two writes were spliced: {:?}",
        settled.pre_approved
    );
    #[cfg(unix)]
    assert_eq!(
        mode_of(&paths.config),
        0o600,
        "the configuration is readable to the machine"
    );
    assert_no_staging_files_left(directory.path());
}
