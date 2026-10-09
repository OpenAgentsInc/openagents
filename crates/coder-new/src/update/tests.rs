use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use super::*;

fn v(text: &str) -> Version {
    Version::parse(text).unwrap()
}

// ---- Fixtures --------------------------------------------------------------

/// A tar entry: name, type flag, data, link target.
struct TarEntry<'a> {
    name: &'a str,
    kind: u8,
    data: &'a [u8],
}

fn tar_gz(entries: &[TarEntry<'_>]) -> Vec<u8> {
    let mut tar = Vec::new();
    for entry in entries {
        let mut header = [0u8; 512];
        header[..entry.name.len()].copy_from_slice(entry.name.as_bytes());
        header[100..107].copy_from_slice(b"0000755");
        header[108..115].copy_from_slice(b"0000000");
        header[116..123].copy_from_slice(b"0000000");
        let size = format!("{:011o}", entry.data.len());
        header[124..135].copy_from_slice(size.as_bytes());
        header[136..147].copy_from_slice(b"00000000000");
        header[156] = entry.kind;
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        header[148..156].copy_from_slice(b"        ");
        let sum: u32 = header.iter().map(|b| u32::from(*b)).sum();
        header[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
        tar.extend_from_slice(&header);
        tar.extend_from_slice(entry.data);
        tar.resize(tar.len().div_ceil(512) * 512, 0);
    }
    tar.extend_from_slice(&[0u8; 1024]);
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&tar).unwrap();
    gz.finish().unwrap()
}

/// A zip of (name, data, deflate?, unix mode).
fn zip(entries: &[(&str, &[u8], bool, u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data, deflate, mode) in entries {
        let crc = crc32fast::hash(data);
        let packed = if *deflate {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(data).unwrap();
            encoder.finish().unwrap()
        } else {
            data.to_vec()
        };
        let method: u16 = if *deflate { 8 } else { 0 };
        let offset = out.len() as u32;
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&packed);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&0x0314u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&method.to_le_bytes());
        central.extend_from_slice(&[0; 4]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 8]);
        central.extend_from_slice(&(mode << 16).to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let at = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The three Unix commands of `version`, as shell scripts that answer
/// `--version` the way the real ones do.
fn unix_archive(version: &str) -> Vec<u8> {
    let scripts: Vec<(String, Vec<u8>)> = ["coder", "openagents", "microcoder"]
        .iter()
        .map(|name| {
            (
                (*name).to_owned(),
                format!("#!/bin/sh\necho \"{name} {version} (test)\"\n").into_bytes(),
            )
        })
        .collect();
    let entries: Vec<TarEntry<'_>> = scripts
        .iter()
        .map(|(name, data)| TarEntry {
            name,
            kind: b'0',
            data,
        })
        .collect();
    tar_gz(&entries)
}

/// A local release channel over HTTP: path -> body.
#[derive(Clone, Default)]
struct Channel(Arc<Mutex<HashMap<String, Vec<u8>>>>);

impl Channel {
    fn put(&self, path: &str, body: impl Into<Vec<u8>>) {
        self.0.lock().unwrap().insert(path.to_owned(), body.into());
    }

    /// Publishes `version` the way the release script does.
    fn publish(&self, version: &str, platform: &str, archive: &[u8]) {
        let name = archive_name(v(version), platform);
        self.put(
            &format!("/SHA256SUMS-coder-{version}"),
            format!("{}  {name}\n", sha256(archive)),
        );
        self.put(&format!("/{name}"), archive.to_vec());
    }

    fn serve(&self) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let files = self.0.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                if reader.read_line(&mut request).is_err() {
                    continue;
                }
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                }
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let body = files.lock().unwrap().get(&path).cloned();
                let _ = match body {
                    Some(body) => stream
                        .write_all(
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            )
                            .as_bytes(),
                        )
                        .and_then(|()| stream.write_all(&body)),
                    None => stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
            }
        });
        format!("http://{address}")
    }
}

fn context(dir: &Path, bin: &Path, base_url: &str, current: &str, platform: &str) -> Context {
    Context {
        dir: dir.to_owned(),
        kind: InstallKind::Standalone(bin.to_owned()),
        config: Config {
            mode: Mode::Auto,
            channel: super::Channel::Stable,
            base_url: base_url.to_owned(),
            base_url_overridden: true,
        },
        current: v(current),
        platform: platform.to_owned(),
        run_version_checks: false,
        signing_team: None,
    }
}

fn block<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn install_old(bin: &Path, names: &[&str]) {
    fs::create_dir_all(bin).unwrap();
    for name in names {
        fs::write(bin.join(name), format!("old {name}")).unwrap();
    }
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).unwrap()
}

// ---- Versions, channels, sums ---------------------------------------------

#[test]
fn versions_parse_and_order_like_the_release_script() {
    assert_eq!(v("1.0.0").to_string(), "1.0.0");
    assert_eq!(v("1.0.1-rc.2").to_string(), "1.0.1-rc.2");
    for bad in [
        "",
        "1.0",
        "1.0.0.0",
        "v1.0.0",
        "1.0.0-beta.1",
        "1.0.0-rc.",
        "1.0.0-rc.01",
        "1.0.x",
        "1..0",
    ] {
        assert_eq!(Version::parse(bad), None, "{bad}");
    }
    assert!(v("1.0.1") > v("1.0.0"));
    assert!(v("1.0.0") > v("1.0.0-rc.6"));
    assert!(v("1.0.0-rc.10") > v("1.0.0-rc.9"));
    assert!(v("1.0.1-rc.1") > v("1.0.0"));
    assert!(v("1.10.0") > v("1.9.9"));
    assert!(v("2.0.0") > v("1.99.99"));
    assert_eq!(Version::current().to_string(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn channel_pointers_name_one_version() {
    assert_eq!(parse_pointer("1.0.1\n"), Some(v("1.0.1")));
    assert_eq!(parse_pointer("  1.0.1-rc.3\r\n"), Some(v("1.0.1-rc.3")));
    assert_eq!(parse_pointer(""), None);
    assert_eq!(parse_pointer("<html>NoSuchKey</html>"), None);
    assert_eq!(parse_pointer("1.0.1\n1.0.2"), None);
    assert_eq!(parse_pointer(&"9".repeat(80)), None);
}

#[test]
fn sums_name_each_archive_once() {
    let hash = "a".repeat(64);
    let upper = "B".repeat(64);
    let sums = format!(
        "{hash}  coder-1.0.1-macos-aarch64.tar.gz\n{upper} *coder-1.0.1-windows-x86_64.zip\r\nnot a line\n{hash}  twice\n{hash}  twice\n"
    );
    assert_eq!(
        sums_entry(&sums, "coder-1.0.1-macos-aarch64.tar.gz"),
        Ok(hash.clone())
    );
    assert_eq!(
        sums_entry(&sums, "coder-1.0.1-windows-x86_64.zip"),
        Ok("b".repeat(64))
    );
    assert!(sums_entry(&sums, "coder-1.0.1-linux-x86_64.tar.gz").is_err());
    assert!(sums_entry(&sums, "twice").is_err());
    assert!(sums_entry("abc  short\n", "short").is_err());
    assert_eq!(
        archive_name(v("1.0.1"), "linux-x86_64-musl"),
        "coder-1.0.1-linux-x86_64-musl.tar.gz"
    );
    assert_eq!(
        archive_name(v("1.0.1"), "windows-x86_64"),
        "coder-1.0.1-windows-x86_64.zip"
    );
    assert_eq!(sums_name(v("1.0.1")), "SHA256SUMS-coder-1.0.1");
    assert_eq!(commands("windows-x86_64").len(), 4);
    assert_eq!(
        commands("macos-aarch64"),
        ["coder", "openagents", "microcoder"]
    );
    assert!(
        platform().starts_with("macos-")
            || platform().starts_with("linux-")
            || platform().starts_with("windows-")
    );
}

// ---- Settings, cache, install kind ------------------------------------------

#[test]
fn checks_run_at_most_once_a_day() {
    let day = CHECK_INTERVAL.as_secs();
    let at = |checked: Option<u64>| State {
        checked_at: checked,
        ..State::default()
    };
    assert!(at(None).due(10 * day));
    assert!(!at(Some(10 * day)).due(10 * day + day - 1));
    assert!(at(Some(10 * day)).due(11 * day));
    // A clock that moved back never suppresses checks for long.
    assert!(at(Some(12 * day)).due(10 * day));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/update.json");
    let state = State {
        checked_at: Some(5),
        latest: Some("1.0.1".into()),
        staged: None,
        held: Some("1.0.2".into()),
    };
    state.save(&path).unwrap();
    assert_eq!(State::load(&path), state);
    fs::write(&path, "not json").unwrap();
    assert_eq!(State::load(&path), State::default());
}

#[test]
fn environment_overrides_saved_settings() {
    let none = |_: &str| None;
    let saved = Settings {
        mode: Some(Mode::Notify),
        channel: Some(super::Channel::Rc),
    };
    let config = Config::resolve(&Settings::default(), &none, v("1.0.0"));
    assert_eq!(config.mode, Mode::Auto);
    assert_eq!(config.channel, super::Channel::Stable);
    assert_eq!(config.base_url, DEFAULT_BASE_URL);
    assert!(!config.base_url_overridden);
    // A release candidate follows rc unless told otherwise.
    assert_eq!(
        Config::resolve(&Settings::default(), &none, v("1.0.1-rc.1")).channel,
        super::Channel::Rc
    );
    let config = Config::resolve(&saved, &none, v("1.0.0"));
    assert_eq!(
        (config.mode, config.channel),
        (Mode::Notify, super::Channel::Rc)
    );
    let env = |name: &str| match name {
        "CODER_UPDATE" => Some("off".to_owned()),
        "CODER_CHANNEL" => Some("stable".to_owned()),
        "CODER_BASE_URL" => Some("http://127.0.0.1:9/coder/".to_owned()),
        _ => None,
    };
    let config = Config::resolve(&saved, &env, v("1.0.0"));
    assert_eq!(
        (config.mode, config.channel),
        (Mode::Off, super::Channel::Stable)
    );
    assert_eq!(config.base_url, "http://127.0.0.1:9/coder");
    assert!(config.base_url_overridden);
    // Unknown values fall back to the saved choice.
    let junk = |name: &str| (name == "CODER_UPDATE").then(|| "sometimes".to_owned());
    assert_eq!(
        Config::resolve(&saved, &junk, v("1.0.0")).mode,
        Mode::Notify
    );
}

#[test]
fn ci_off_and_debug_builds_never_check_on_their_own() {
    let none = |_: &str| None;
    let auto = Config::resolve(&Settings::default(), &none, v("1.0.0"));
    assert!(auto.automatic(&none, false));
    assert!(
        !auto.automatic(&none, true),
        "debug builds skip the public channel"
    );
    for value in ["true", "1", "yes"] {
        let ci = move |name: &str| (name == "CI").then(|| value.to_owned());
        assert!(!auto.automatic(&ci, false), "CI={value}");
    }
    let not_ci = |name: &str| (name == "CI").then(|| "false".to_owned());
    assert!(auto.automatic(&not_ci, false));
    let off = Config {
        mode: Mode::Off,
        ..auto.clone()
    };
    assert!(!off.automatic(&none, false));
    let test_channel = Config {
        base_url_overridden: true,
        ..auto
    };
    assert!(test_channel.automatic(&none, true));
}

#[test]
fn install_kind_comes_from_the_binary_path() {
    let home = Path::new("/Users/me/.openagents");
    let kind = |path: &str| InstallKind::detect(Path::new(path), Some(home));
    assert_eq!(
        kind("/Users/me/work/openagents/target/release/coder-new"),
        InstallKind::Source
    );
    assert_eq!(
        kind("/x/target/aarch64-apple-darwin/debug/coder-new"),
        InstallKind::Source
    );
    assert_eq!(
        kind("/Users/me/.openagents/versions/coder-openagents-abc123"),
        InstallKind::Source
    );
    assert_eq!(
        kind("/Applications/OpenAgents.app/Contents/MacOS/coder"),
        InstallKind::Desktop
    );
    assert_eq!(
        kind("/opt/homebrew/Cellar/coder/1.0.0/bin/coder"),
        InstallKind::PackageManager
    );
    assert_eq!(
        kind("/home/linuxbrew/.linuxbrew/bin/coder"),
        InstallKind::PackageManager
    );
    assert_eq!(
        kind("/usr/lib/node_modules/coder/bin/coder"),
        InstallKind::PackageManager
    );

    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("coder");
    assert_eq!(
        InstallKind::detect(&exe, Some(home)),
        InstallKind::Standalone(dir.path().to_owned())
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let locked = dir.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        let exe = locked.join("coder");
        let found = InstallKind::detect(&exe, Some(home));
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        // Root can write anywhere; everyone else gets the notice.
        if !writable(&locked) || found != InstallKind::Standalone(locked.clone()) {
            assert_eq!(found, InstallKind::Unwritable(locked));
        }
    }
}

#[test]
fn package_managed_and_desktop_installs_only_get_a_notice() {
    let latest = v("1.0.1");
    let standalone = InstallKind::Standalone(PathBuf::from("/bin"));
    assert_eq!(
        notice(&standalone, Mode::Auto, latest, true).as_deref(),
        Some("Coder 1.0.1 is ready. Restart Coder to use it.")
    );
    assert_eq!(
        notice(&standalone, Mode::Notify, latest, false).as_deref(),
        Some("Coder 1.0.1 is available. Run coder update.")
    );
    assert!(
        notice(&InstallKind::Desktop, Mode::Auto, latest, false)
            .unwrap()
            .contains("OpenAgents Desktop")
    );
    assert!(
        notice(&InstallKind::PackageManager, Mode::Auto, latest, false)
            .unwrap()
            .contains("package manager")
    );
    assert!(
        notice(
            &InstallKind::Unwritable("/opt".into()),
            Mode::Auto,
            latest,
            false
        )
        .unwrap()
        .contains("openagents.com/cli/install")
    );
    assert_eq!(
        notice(&InstallKind::Source, Mode::Auto, latest, false),
        None
    );

    // `coder update` refuses to replace what it does not own.
    let dir = tempfile::tempdir().unwrap();
    for kind in [
        InstallKind::Desktop,
        InstallKind::PackageManager,
        InstallKind::Source,
    ] {
        let mut ctx = context(
            dir.path(),
            dir.path(),
            "http://127.0.0.1:9",
            "1.0.0",
            "linux-x86_64",
        );
        ctx.kind = kind;
        let mut out = Vec::new();
        assert!(run(&ctx, None, &mut out).is_err());
        assert!(install_staged(&ctx).unwrap().is_none());
    }
}

// ---- Archives ---------------------------------------------------------------

#[test]
fn tar_archives_unpack_only_top_level_regular_files() {
    let dir = tempfile::tempdir().unwrap();
    let wanted: Vec<String> = commands("linux-x86_64");
    let write = |name: &str, bytes: Vec<u8>| {
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        path
    };
    let good = write(
        "good.tar.gz",
        tar_gz(&[
            TarEntry {
                name: "./",
                kind: b'5',
                data: b"",
            },
            TarEntry {
                name: "./coder",
                kind: b'0',
                data: b"c",
            },
            TarEntry {
                name: "openagents",
                kind: b'0',
                data: b"o",
            },
            TarEntry {
                name: "._microcoder",
                kind: b'0',
                data: b"apple double",
            },
            TarEntry {
                name: "microcoder",
                kind: b'0',
                data: b"m",
            },
        ]),
    );
    let out = dir.path().join("good");
    fs::create_dir(&out).unwrap();
    unpack(&good, &wanted, &out).unwrap();
    assert_eq!(
        (read(out.join("coder")), read(out.join("microcoder"))),
        ("c".into(), "m".into())
    );
    assert!(!out.join("._microcoder").exists());

    let refused = |name: &str, entries: &[TarEntry<'_>]| {
        let archive = write(&format!("{name}.tar.gz"), tar_gz(entries));
        let out = dir.path().join(name);
        fs::create_dir(&out).unwrap();
        unpack(&archive, &wanted, &out).unwrap_err()
    };
    let link = refused(
        "link",
        &[TarEntry {
            name: "coder",
            kind: b'2',
            data: b"",
        }],
    );
    assert!(link.contains("not a regular file"), "{link}");
    let nested = refused(
        "nested",
        &[
            TarEntry {
                name: "bin/coder",
                kind: b'0',
                data: b"c",
            },
            TarEntry {
                name: "../openagents",
                kind: b'0',
                data: b"o",
            },
        ],
    );
    assert!(nested.contains("has no coder"), "{nested}");
    assert!(!dir.path().join("openagents").exists());
    let twice = refused(
        "twice",
        &[
            TarEntry {
                name: "coder",
                kind: b'0',
                data: b"c",
            },
            TarEntry {
                name: "./coder",
                kind: b'0',
                data: b"d",
            },
        ],
    );
    assert!(twice.contains("twice"), "{twice}");
    let damaged = write("damaged.tar.gz", b"not gzip".to_vec());
    assert!(
        unpack(&damaged, &wanted, dir.path())
            .unwrap_err()
            .contains("damaged")
    );
}

#[test]
fn zip_archives_unpack_stored_and_deflated_files() {
    let dir = tempfile::tempdir().unwrap();
    let wanted = commands("windows-x86_64");
    let body = b"MZ".repeat(5000);
    let archive = dir.path().join("coder-1.0.1-windows-x86_64.zip");
    fs::write(
        &archive,
        zip(&[
            ("coder.exe", &body, true, 0o100_644),
            ("openagents.exe", b"o", false, 0o100_644),
            ("microcoder.exe", b"m", true, 0o100_644),
            ("coder-boundary.exe", b"b", false, 0),
            ("docs/readme.txt", b"r", false, 0o100_644),
        ]),
    )
    .unwrap();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    unpack(&archive, &wanted, &out).unwrap();
    assert_eq!(fs::read(out.join("coder.exe")).unwrap(), body);
    assert_eq!(read(out.join("coder-boundary.exe")), "b");

    let link = dir.path().join("link.zip");
    fs::write(&link, zip(&[("coder.exe", b"/bin/sh", false, 0o120_777)])).unwrap();
    let out = dir.path().join("link");
    fs::create_dir(&out).unwrap();
    assert!(
        unpack(&link, &wanted, &out)
            .unwrap_err()
            .contains("not a regular file")
    );

    let mut corrupt = zip(&[("coder.exe", b"hello", false, 0o100_644)]);
    corrupt[30 + "coder.exe".len()] ^= 0xff;
    let bad = dir.path().join("bad.zip");
    fs::write(&bad, corrupt).unwrap();
    let out = dir.path().join("bad");
    fs::create_dir(&out).unwrap();
    assert!(unpack(&bad, &wanted, &out).unwrap_err().contains("damaged"));
}

// ---- Install, swap, rollback ------------------------------------------------

#[test]
fn a_failed_swap_puts_every_command_back() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    install_old(&bin, &["coder", "openagents", "microcoder"]);
    let stage = dir.path().join("stage");
    fs::create_dir(&stage).unwrap();
    let names = commands("linux-x86_64");
    for name in &names {
        fs::write(stage.join(name), format!("new {name}")).unwrap();
    }
    // The third rename fails after two new commands are in place.
    let error = swap(&bin, &stage, &names, &|at| {
        if at == 2 {
            Err(io::Error::other("disk full"))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert!(error.contains("Coder is unchanged"), "{error}");
    for name in &names {
        assert_eq!(read(bin.join(name)), format!("old {name}"));
        assert_eq!(read(stage.join(name)), format!("new {name}"));
    }
    assert!(!bin.join(BACKUP_DIR).exists());
    let leftovers: Vec<_> = fs::read_dir(&bin)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(leftovers.len(), 3, "{leftovers:?}");

    // A folder where a command belongs refuses before anything moves.
    fs::remove_file(bin.join("openagents")).unwrap();
    fs::create_dir(bin.join("openagents")).unwrap();
    assert!(
        swap(&bin, &stage, &names, &|_| Ok(()))
            .unwrap_err()
            .contains("folder")
    );
    assert_eq!(read(bin.join("coder")), "old coder");

    // Success keeps the replaced set, and a missing command is added.
    fs::remove_dir(bin.join("openagents")).unwrap();
    swap(&bin, &stage, &names, &|_| Ok(())).unwrap();
    for name in &names {
        assert_eq!(read(bin.join(name)), format!("new {name}"));
    }
    assert_eq!(read(bin.join(BACKUP_DIR).join("coder")), "old coder");
    assert!(!bin.join(BACKUP_DIR).join("openagents").exists());
}

#[test]
fn a_checksum_mismatch_refuses_to_install() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    install_old(&bin, &["coder", "openagents", "microcoder"]);
    let archive = dir.path().join("coder-1.0.1-linux-x86_64.tar.gz");
    let bytes = unix_archive("1.0.1");
    fs::write(&archive, &bytes).unwrap();
    let ctx = context(
        dir.path(),
        &bin,
        "http://127.0.0.1:9",
        "1.0.0",
        "linux-x86_64",
    );
    let error = install_archive(&ctx, &bin, &archive, &"0".repeat(64), v("1.0.1"), &|_| {
        Ok(())
    })
    .unwrap_err()
    .to_string();
    assert!(
        error.starts_with("Checksum mismatch for coder-1.0.1-linux-x86_64.tar.gz"),
        "{error}"
    );
    assert_eq!(read(bin.join("coder")), "old coder");
    assert!(!bin.join(BACKUP_DIR).exists());

    // Another Coder holding the lock is skipped, not failed.
    fs::write(bin.join(LOCK_FILE), "1").unwrap();
    assert!(matches!(
        install_archive(&ctx, &bin, &archive, &sha256(&bytes), v("1.0.1"), &|_| Ok(
            ()
        )),
        Err(Refusal::Busy)
    ));
    fs::remove_file(bin.join(LOCK_FILE)).unwrap();
    install_archive(&ctx, &bin, &archive, &sha256(&bytes), v("1.0.1"), &|_| {
        Ok(())
    })
    .unwrap();
    assert!(read(bin.join("coder")).contains("coder 1.0.1"));
    assert!(!bin.join(LOCK_FILE).exists());
}

#[cfg(unix)]
#[test]
fn new_commands_must_report_the_new_version() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    install_old(&bin, &["coder", "openagents", "microcoder"]);
    let mut ctx = context(
        dir.path(),
        &bin,
        "http://127.0.0.1:9",
        "1.0.0",
        "linux-x86_64",
    );
    ctx.run_version_checks = true;
    // An archive whose commands say 1.0.0 is not 1.0.1.
    let archive = dir.path().join("coder-1.0.1-linux-x86_64.tar.gz");
    let wrong = unix_archive("1.0.0");
    fs::write(&archive, &wrong).unwrap();
    let error = install_archive(&ctx, &bin, &archive, &sha256(&wrong), v("1.0.1"), &|_| {
        Ok(())
    })
    .unwrap_err()
    .to_string();
    assert!(error.contains("does not report version 1.0.1"), "{error}");
    assert_eq!(read(bin.join("coder")), "old coder");
    let right = unix_archive("1.0.1");
    fs::write(&archive, &right).unwrap();
    install_archive(&ctx, &bin, &archive, &sha256(&right), v("1.0.1"), &|_| {
        Ok(())
    })
    .unwrap();
    let output = Command::new(bin.join("openagents"))
        .arg("--version")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "openagents 1.0.1 (test)\n"
    );
}

#[test]
fn windows_installs_replace_all_four_commands() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    install_old(
        &bin,
        &[
            "coder.exe",
            "openagents.exe",
            "microcoder.exe",
            "coder-boundary.exe",
        ],
    );
    let bytes = zip(&[
        ("coder.exe", b"new coder", true, 0),
        ("openagents.exe", b"new openagents", true, 0),
        ("microcoder.exe", b"new microcoder", true, 0),
        ("coder-boundary.exe", b"new boundary", true, 0),
    ]);
    let archive = dir.path().join("coder-1.0.1-windows-x86_64.zip");
    fs::write(&archive, &bytes).unwrap();
    let ctx = context(
        dir.path(),
        &bin,
        "http://127.0.0.1:9",
        "1.0.0",
        "windows-x86_64",
    );
    install_archive(&ctx, &bin, &archive, &sha256(&bytes), v("1.0.1"), &|_| {
        Ok(())
    })
    .unwrap();
    assert_eq!(read(bin.join("coder-boundary.exe")), "new boundary");
    assert_eq!(
        read(bin.join(BACKUP_DIR).join("coder-boundary.exe")),
        "old coder-boundary.exe"
    );
}

// ---- The channel end to end ---------------------------------------------------

#[test]
fn a_newer_version_is_found_verified_staged_and_installed() {
    let channel = Channel::default();
    let base = channel.serve();
    channel.put("/coder.stable", "1.0.1\n");
    channel.publish("1.0.1", "linux-x86_64", &unix_archive("1.0.1"));
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    install_old(&bin, &["coder", "openagents", "microcoder"]);
    let ctx = context(
        &dir.path().join("state"),
        &bin,
        &base,
        "1.0.0",
        "linux-x86_64",
    );

    // Notify-only checks record the version and stage nothing.
    let found = block(check(&ctx, false, true)).unwrap();
    assert_eq!(
        found,
        Found {
            newer: Some(v("1.0.1")),
            staged: false
        }
    );
    assert_eq!(
        ctx.cached_notice().as_deref(),
        Some("Coder 1.0.1 is available. Run coder update.")
    );
    assert!(!State::load(&ctx.state_path()).due(now()));

    let found = block(check(&ctx, true, true)).unwrap();
    assert_eq!(
        found,
        Found {
            newer: Some(v("1.0.1")),
            staged: true
        }
    );
    assert_eq!(
        ctx.cached_notice().as_deref(),
        Some("Coder 1.0.1 is ready. Restart Coder to use it.")
    );
    assert_eq!(
        read(bin.join("coder")),
        "old coder",
        "staging never touches the install"
    );
    // A second check reuses the verified download.
    assert!(block(check(&ctx, true, true)).unwrap().staged);

    assert_eq!(install_staged(&ctx).unwrap(), Some(v("1.0.1")));
    assert!(read(bin.join("coder")).contains("coder 1.0.1"));
    assert!(read(bin.join("microcoder")).contains("microcoder 1.0.1"));
    assert_eq!(read(bin.join(BACKUP_DIR).join("coder")), "old coder");
    let state = State::load(&ctx.state_path());
    assert_eq!((state.staged, state.latest), (None, None));
    assert_eq!(
        fs::read_dir(dir.path().join("state/updates"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(install_staged(&ctx).unwrap(), None);
}

#[test]
fn a_tampered_archive_is_never_staged_or_installed() {
    let channel = Channel::default();
    let base = channel.serve();
    channel.put("/coder.stable", "1.0.1");
    channel.publish("1.0.1", "linux-x86_64", &unix_archive("1.0.1"));
    // The archive is swapped after the sums were published.
    channel.put("/coder-1.0.1-linux-x86_64.tar.gz", unix_archive("6.6.6"));
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    install_old(&bin, &["coder", "openagents", "microcoder"]);
    let ctx = context(
        &dir.path().join("state"),
        &bin,
        &base,
        "1.0.0",
        "linux-x86_64",
    );
    let error = block(check(&ctx, true, true)).unwrap_err();
    assert_eq!(
        error,
        "Checksum mismatch for coder-1.0.1-linux-x86_64.tar.gz. Coder is unchanged."
    );
    assert_eq!(State::load(&ctx.state_path()).staged, None);
    assert_eq!(
        fs::read_dir(dir.path().join("state/updates"))
            .unwrap()
            .count(),
        0
    );
    let mut out = Vec::new();
    assert!(
        run(&ctx, None, &mut out)
            .unwrap_err()
            .starts_with("Checksum mismatch")
    );
    assert_eq!(read(bin.join("coder")), "old coder");

    // A staged archive altered on disk afterwards is refused at install.
    channel.publish("1.0.1", "linux-x86_64", &unix_archive("1.0.1"));
    block(check(&ctx, true, true)).unwrap();
    let staged = State::load(&ctx.state_path()).staged.unwrap();
    fs::write(&staged.archive, unix_archive("6.6.6")).unwrap();
    assert!(
        install_staged(&ctx)
            .unwrap_err()
            .starts_with("Checksum mismatch")
    );
    assert_eq!(read(bin.join("coder")), "old coder");
    assert_eq!(State::load(&ctx.state_path()).staged, None);

    // A platform the release has no build for stages nothing.
    let other = context(
        &dir.path().join("state"),
        &bin,
        &base,
        "1.0.0",
        "linux-aarch64",
    );
    assert!(
        block(check(&other, true, true))
            .unwrap_err()
            .contains("no verified")
    );
}

#[test]
fn coder_never_downgrades_and_skips_a_rolled_back_version() {
    let channel = Channel::default();
    let base = channel.serve();
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    install_old(&bin, &["coder", "openagents", "microcoder"]);
    let ctx = context(
        &dir.path().join("state"),
        &bin,
        &base,
        "1.0.1",
        "linux-x86_64",
    );
    for pointer in ["1.0.0", "1.0.1", "1.0.1-rc.9"] {
        channel.put("/coder.stable", pointer);
        assert_eq!(
            block(check(&ctx, true, true)).unwrap().newer,
            None,
            "{pointer}"
        );
        assert_eq!(ctx.cached_notice(), None);
    }
    let mut out = Vec::new();
    run(&ctx, None, &mut out).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out),
        "Checking the stable channel...\nCoder 1.0.1 is the newest on the stable channel.\n"
    );
    assert_eq!(read(bin.join("coder")), "old coder");

    // Offline: an unreadable channel is an error and changes nothing.
    channel.put("/coder.stable", "garbage");
    assert!(block(check(&ctx, true, true)).is_err());
    let offline = context(
        &dir.path().join("offline"),
        &bin,
        "http://127.0.0.1:9",
        "1.0.1",
        "linux-x86_64",
    );
    assert!(block(check(&offline, true, true)).is_err());
    assert_eq!(State::load(&offline.state_path()).checked_at, None);

    // A held version is skipped automatically but installed on request.
    channel.put("/coder.stable", "1.0.2");
    channel.publish("1.0.2", "linux-x86_64", &unix_archive("1.0.2"));
    State {
        held: Some("1.0.2".into()),
        ..State::default()
    }
    .save(&ctx.state_path())
    .unwrap();
    assert_eq!(block(check(&ctx, true, true)).unwrap().newer, None);
    let mut out = Vec::new();
    run(&ctx, None, &mut out).unwrap();
    assert!(String::from_utf8_lossy(&out).ends_with("Updated Coder to 1.0.2.\n"));
    assert_eq!(State::load(&ctx.state_path()).held, None);
}

#[cfg(unix)]
#[test]
fn rollback_restores_the_replaced_commands_and_holds_the_version() {
    let channel = Channel::default();
    let base = channel.serve();
    channel.put("/coder.stable", "1.0.1");
    channel.publish("1.0.1", "linux-x86_64", &unix_archive("1.0.1"));
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    // The 1.0.0 install, as scripts that answer --version.
    let old = dir.path().join("old.tar.gz");
    fs::write(&old, unix_archive("1.0.0")).unwrap();
    unpack(&old, &commands("linux-x86_64"), &bin).unwrap();
    for name in commands("linux-x86_64") {
        make_executable(&bin.join(name)).unwrap();
    }
    let ctx = context(
        &dir.path().join("state"),
        &bin,
        &base,
        "1.0.0",
        "linux-x86_64",
    );
    let mut out = Vec::new();
    run(&ctx, None, &mut out).unwrap();
    assert!(read(bin.join("coder")).contains("1.0.1"));

    // Now running 1.0.1, go back.
    let after = context(
        &dir.path().join("state"),
        &bin,
        &base,
        "1.0.1",
        "linux-x86_64",
    );
    let mut out = Vec::new();
    run(&after, Some("--rollback"), &mut out).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out),
        "Went back to Coder 1.0.0. Automatic updates skip 1.0.1 until you run coder update.\n"
    );
    assert!(read(bin.join("coder")).contains("1.0.0"));
    assert!(read(bin.join(BACKUP_DIR).join("coder")).contains("1.0.1"));
    assert_eq!(
        State::load(&ctx.state_path()).held.as_deref(),
        Some("1.0.1")
    );
    // The held version is not staged again automatically.
    assert_eq!(block(check(&ctx, true, true)).unwrap().newer, None);
    // A second rollback undoes the first.
    let mut out = Vec::new();
    run(&ctx, Some("--rollback"), &mut out).unwrap();
    assert!(read(bin.join("coder")).contains("1.0.1"));

    let empty = dir.path().join("empty");
    fs::create_dir(&empty).unwrap();
    let none = context(
        &dir.path().join("state"),
        &empty,
        &base,
        "1.0.0",
        "linux-x86_64",
    );
    assert_eq!(
        rollback(&none).unwrap_err(),
        "There is no earlier Coder to go back to."
    );
}

#[test]
fn settings_commands_save_the_choice() {
    let dir = tempfile::tempdir().unwrap();
    let mut out = Vec::new();
    let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    command(&args(&["--mode", "notify"]), dir.path(), &mut out).unwrap();
    command(&args(&["--channel", "rc"]), dir.path(), &mut out).unwrap();
    assert_eq!(
        load_settings(dir.path()),
        Settings {
            mode: Some(Mode::Notify),
            channel: Some(super::Channel::Rc)
        }
    );
    assert!(command(&args(&["--mode", "sometimes"]), dir.path(), &mut out).is_err());
    assert!(
        command(&args(&["--bogus"]), dir.path(), &mut out)
            .unwrap_err()
            .starts_with("Unknown option")
    );
    command(&args(&["--help"]), dir.path(), &mut out).unwrap();
    assert!(String::from_utf8_lossy(&out).contains("coder update --rollback"));
}
