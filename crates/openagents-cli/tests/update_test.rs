//! What `oa update` refuses.
//!
//! The install path and the update path deliver the same bytes to the same
//! machine, so they answer to the same rules: a version the release naming
//! grammar does not admit is not asked for, a sums file that names no entry
//! for the artifact stops the install, a digest that disagrees stops it, and
//! nothing lands at the target path until the bytes have been proven.

use std::collections::HashMap;

use openagents_cli::update::{
    Outcome, UpdateError, Updater, artifact_name, cmp_release_versions, digest_for, hex_digest,
    platform, replace_binary, run, sums_entry_name, valid_version,
};

/// A release server that serves exactly what it is given and 404s the rest.
///
/// The refusals under test are about what arrives over the wire, so they are
/// exercised over a real socket rather than against a mocked client that could
/// only prove the mock behaves.
async fn release_server(objects: HashMap<String, Vec<u8>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());

    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _peer)) = listener.accept().await else {
                return;
            };

            let objects = objects.clone();

            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};

                let mut buffer = [0u8; 2048];
                let Ok(read) = stream.read(&mut buffer).await else {
                    return;
                };

                let request = String::from_utf8_lossy(&buffer[..read]);
                let name = request
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .trim_start_matches('/')
                    .to_string();

                let response = match objects.get(&name) {
                    Some(body) => {
                        let mut head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .into_bytes();
                        head.extend_from_slice(body);
                        head
                    }
                    None => {
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_vec()
                    }
                };

                let _ = stream.write_all(&response).await;
                let _ = stream.shutdown().await;
            });
        }
    });

    base
}

fn objects(entries: Vec<(&str, &[u8])>) -> HashMap<String, Vec<u8>> {
    entries
        .into_iter()
        .map(|(name, body)| (name.to_string(), body.to_vec()))
        .collect()
}

#[test]
fn the_version_grammar_matches_the_one_the_release_publishes() {
    // ops/release-cli.sh: ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9._]+)?$
    assert!(valid_version("0.1.0"));
    assert!(valid_version("0.1.0-rc.1"));
    assert!(valid_version("10.20.30-alpha_2"));

    assert!(!valid_version(""));
    assert!(!valid_version("0.1"));
    assert!(!valid_version("0.1.0.1"));
    assert!(!valid_version("v0.1.0"));
    assert!(!valid_version("0.1.x"));
    assert!(!valid_version("0.1.0-"));
    assert!(!valid_version("0.1.0-rc-1"));
    assert!(!valid_version("0.1.0-rc 1"));

    // A channel pointer is a URL segment away from a path, and the artifact
    // name is built out of it.
    assert!(!valid_version("../../etc/passwd"));
    assert!(!valid_version("0.1.0/../stable"));
}

#[test]
fn a_non_release_build_reports_a_development_version() {
    assert_eq!(openagents_cli::VERSION, "0.0.0-dev");
    assert_ne!(openagents_cli::VERSION, env!("CARGO_PKG_VERSION"));
    assert!(valid_version(openagents_cli::VERSION));
}

#[tokio::test]
async fn a_non_release_build_can_update_to_the_first_published_release() {
    let outcome = run(None, Some("0.0.1".to_string()), true, false, true)
        .await
        .unwrap();

    assert_eq!(
        outcome,
        Outcome::Available {
            version: "0.0.1".to_string()
        }
    );
}

#[test]
fn a_newer_installed_release_sorts_ahead_of_an_older_channel() {
    use std::cmp::Ordering;

    assert_eq!(
        cmp_release_versions("0.2.0-rc.13", "0.1.1"),
        Some(Ordering::Greater)
    );
    assert_eq!(
        cmp_release_versions("0.2.0-rc.13", "0.2.0"),
        Some(Ordering::Less)
    );
    assert_eq!(
        cmp_release_versions("0.2.0-rc.12", "0.2.0-rc.13"),
        Some(Ordering::Less)
    );
    assert_eq!(
        cmp_release_versions("0.0.0-dev", "0.0.1"),
        Some(Ordering::Less)
    );
    assert_eq!(
        cmp_release_versions("0.2.0", "0.2.0"),
        Some(Ordering::Equal)
    );
}

#[tokio::test]
async fn an_older_requested_version_is_a_downgrade_not_an_update() {
    let outcome = run(None, Some("0.0.0-alpha".to_string()), true, false, true)
        .await
        .unwrap();

    assert_eq!(
        outcome,
        Outcome::Older {
            version: "0.0.0-alpha".to_string(),
            installed: "0.0.0-dev".to_string(),
        }
    );
}

#[tokio::test]
async fn an_older_requested_version_installs_only_when_forced() {
    let outcome = run(None, Some("0.0.0-alpha".to_string()), true, true, true)
        .await
        .unwrap();

    assert_eq!(
        outcome,
        Outcome::Available {
            version: "0.0.0-alpha".to_string()
        }
    );
}

#[test]
fn the_windows_sums_entry_carries_an_extension_the_artifact_does_not() {
    // The installer downloads a URL with no extension and then looks up a name
    // with one. The published release reproduces that asymmetry, so the update
    // path has to as well or every Windows update fails on a missing entry.
    assert_eq!(
        artifact_name("0.1.0", "windows-x86_64"),
        "openagents-0.1.0-windows-x86_64"
    );
    assert_eq!(
        sums_entry_name("0.1.0", "windows-x86_64"),
        "openagents-0.1.0-windows-x86_64.exe"
    );

    assert_eq!(
        sums_entry_name("0.1.0", "linux-x86_64-musl"),
        "openagents-0.1.0-linux-x86_64-musl"
    );
}

#[test]
fn a_sums_file_is_read_the_way_the_installer_reads_one() {
    let sums = "\
aaaa  openagents-0.1.0-linux-x86_64
bbbb  openagents-0.1.0-linux-x86_64-musl
cccc *openagents-0.1.0-windows-x86_64.exe
";

    // The glibc and musl entries differ only by suffix, and a prefix match
    // would hand the musl reader the dynamically linked binary.
    assert_eq!(
        digest_for(sums, "openagents-0.1.0-linux-x86_64").as_deref(),
        Some("aaaa")
    );
    assert_eq!(
        digest_for(sums, "openagents-0.1.0-linux-x86_64-musl").as_deref(),
        Some("bbbb")
    );

    // `sha256sum` writes a `*` before the name in binary mode.
    assert_eq!(
        digest_for(sums, "openagents-0.1.0-windows-x86_64.exe").as_deref(),
        Some("cccc")
    );

    assert_eq!(digest_for(sums, "openagents-0.1.0-macos-aarch64"), None);
    assert_eq!(digest_for("", "openagents-0.1.0-linux-x86_64"), None);
}

#[test]
fn the_platform_is_the_one_this_binary_was_built_for() {
    let platform = platform().expect("this target has a published artifact");

    assert!(platform.starts_with(std::env::consts::OS));
    assert!(platform.contains(std::env::consts::ARCH));

    // The libc flavour is settled by the toolchain, not probed at runtime, so
    // it cannot disagree with the binary it names.
    if cfg!(all(target_os = "linux", target_env = "musl")) {
        assert!(platform.ends_with("-musl"));
    } else {
        assert!(!platform.ends_with("-musl"));
    }
}

#[test]
fn the_digest_is_the_one_shasum_would_print() {
    // sha256 of the empty input, which every implementation agrees on.
    assert_eq!(
        hex_digest(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[tokio::test]
async fn a_channel_resolves_to_the_version_it_names() {
    let base = release_server(objects(vec![("stable", b"0.1.0-rc.2\n")])).await;
    let updater = Updater::new(Some(base), Some("stable".to_string()));

    assert_eq!(updater.resolve_channel().await.unwrap(), "0.1.0-rc.2");
}

#[tokio::test]
async fn a_channel_that_answers_with_something_else_is_refused() {
    // A bucket that starts serving an error page, an index listing, or a
    // half-written pointer must not become a version the artifact name is
    // built out of.
    let base = release_server(objects(vec![("stable", b"<html>404</html>")])).await;
    let updater = Updater::new(Some(base), Some("stable".to_string()));

    assert!(matches!(
        updater.resolve_channel().await,
        Err(UpdateError::ChannelNotAVersion { .. })
    ));
}

#[tokio::test]
async fn a_verified_artifact_is_returned() {
    let binary = b"the published binary";
    let sums = format!("{}  openagents-0.1.0-linux-x86_64\n", hex_digest(binary));

    let base = release_server(objects(vec![
        ("openagents-0.1.0-linux-x86_64", binary),
        ("SHA256SUMS-0.1.0", sums.as_bytes()),
    ]))
    .await;

    let updater = Updater::new(Some(base), None);
    let bytes = updater
        .fetch_verified("0.1.0", "linux-x86_64")
        .await
        .unwrap();

    assert_eq!(bytes, binary);
}

#[tokio::test]
async fn a_missing_sums_file_stops_the_update() {
    // The artifact is available and would install fine. Without the sums file
    // there is no way to say it is the one that was published, and an update
    // that installs it anyway is a weaker path than the installer's.
    let base = release_server(objects(vec![(
        "openagents-0.1.0-linux-x86_64",
        b"the published binary",
    )]))
    .await;

    let updater = Updater::new(Some(base), None);

    assert!(matches!(
        updater.fetch_verified("0.1.0", "linux-x86_64").await,
        Err(UpdateError::SumsUnavailable { .. })
    ));
}

#[tokio::test]
async fn a_sums_file_naming_no_entry_stops_the_update() {
    let base = release_server(objects(vec![
        ("openagents-0.1.0-linux-x86_64", b"the published binary"),
        (
            "SHA256SUMS-0.1.0",
            b"aaaa  openagents-0.1.0-macos-aarch64\n",
        ),
    ]))
    .await;

    let updater = Updater::new(Some(base), None);

    assert!(matches!(
        updater.fetch_verified("0.1.0", "linux-x86_64").await,
        Err(UpdateError::SumsMissingEntry { .. })
    ));
}

#[tokio::test]
async fn bytes_that_do_not_match_their_digest_stop_the_update() {
    let sums = format!(
        "{}  openagents-0.1.0-linux-x86_64\n",
        hex_digest(b"expected")
    );

    let base = release_server(objects(vec![
        ("openagents-0.1.0-linux-x86_64", b"something else entirely"),
        ("SHA256SUMS-0.1.0", sums.as_bytes()),
    ]))
    .await;

    let updater = Updater::new(Some(base), None);

    assert!(matches!(
        updater.fetch_verified("0.1.0", "linux-x86_64").await,
        Err(UpdateError::DigestMismatch { .. })
    ));
}

#[test]
fn a_replaced_binary_is_never_half_written() {
    let directory = std::env::temp_dir().join(format!("oa-update-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();

    let target = directory.join("oa");
    std::fs::write(&target, b"old binary").unwrap();

    replace_binary(&target, b"new binary").unwrap();

    assert_eq!(std::fs::read(&target).unwrap(), b"new binary");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = std::fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755, "the replacement is not executable");
    }

    // The staging file is gone: a directory left holding a second copy of a
    // 40 MB binary after every update is a leak nobody looks for.
    let leftovers: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name != "oa")
        .collect();

    assert!(leftovers.is_empty(), "left behind {leftovers:?}");

    std::fs::remove_dir_all(&directory).unwrap();
}
