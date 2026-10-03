//! `openagents terminal` and bare `openagents`, run as a process with a
//! temporary HOME and no terminal, so no screen opens and no real identity,
//! store, or host is touched.

use std::path::Path;
use std::process::{Command, Output, Stdio};

fn openagents(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env("HOME", home)
        .env("TMPDIR", home)
        .env_remove("OPENAGENTS_CHAT_HOME")
        .env_remove("OPENAGENTS_SETTINGS")
        .env_remove("XDG_RUNTIME_DIR")
        // Pipes, not a terminal.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("openagents runs")
}

#[test]
fn bare_openagents_without_a_terminal_prints_usage() {
    let home = tempfile::tempdir().unwrap();
    let output = openagents(home.path(), &[]);
    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("usage: openagents [--json] COMMAND"),
        "{stderr}"
    );
    assert!(
        stderr.contains("  terminal     OpenAgents Terminal"),
        "{stderr}"
    );
}

#[test]
fn terminal_help_names_its_options() {
    let home = tempfile::tempdir().unwrap();
    let output = openagents(home.path(), &["terminal", "--help"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("usage: openagents terminal"), "{stdout}");
    assert!(stdout.contains("--thread"), "{stdout}");
    assert!(stdout.contains("--scratch"), "{stdout}");
    assert!(stdout.contains("--resume [ID|TITLE]"), "{stdout}");
}

#[test]
fn a_bad_thread_is_a_usage_error() {
    let home = tempfile::tempdir().unwrap();
    let output = openagents(home.path(), &["terminal", "--thread", "not-a-thread"]);
    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("32 lowercase hex"), "{stderr}");
    let output = openagents(
        home.path(),
        &[
            "terminal",
            "--thread",
            "0123456789abcdef0123456789abcdef",
            "--continue",
        ],
    );
    assert_eq!(output.status.code(), Some(64));
    let output = openagents(home.path(), &["terminal", "--new", "--continue"]);
    assert_eq!(output.status.code(), Some(64));
    let output = openagents(home.path(), &["terminal", "--colour"]);
    assert_eq!(output.status.code(), Some(64));
    // `--resume` goes alone; only it takes words after it.
    for args in [
        &["terminal", "--resume", "--continue"][..],
        &[
            "terminal",
            "--resume",
            "x",
            "--thread",
            "0123456789abcdef0123456789abcdef",
        ],
        &["terminal", "--resume", "--scratch"],
        &["terminal", "stray"],
    ] {
        let output = openagents(home.path(), args);
        assert_eq!(output.status.code(), Some(64), "{args:?}");
    }
}

/// OpenAgents Terminal in a real pseudo-terminal, from a stand-in home
/// where Codex, Claude Code, Grok Build, and Devin are signed in (#10113):
/// the welcome card names every ready agent, not only the one a run would
/// start on. Agents are opt-out (#10184), so a signed-in Devin is named even
/// though the settings never mention it. The stand-in logins are fixtures,
/// and every agent the screen looks for is pinned to them or to a missing
/// path, so what this computer has installed never changes the card (#10303);
/// no real login, store, or host is touched, and the screen quits before it
/// sends anything.
#[cfg(unix)]
#[test]
fn the_welcome_card_names_every_ready_agent_here() {
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    const ROWS: u16 = 30;
    const COLS: u16 = 100;
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let bin = temp.path().join("bin");
    let repo = temp.path().join("demo");
    for dir in [
        &home.join(".codex"),
        &home.join(".grok"),
        &home.join(".local/share/devin"),
        &bin,
        &repo,
    ] {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(
        home.join(".codex/auth.json"),
        r#"{"tokens":{"access_token":"stand-in","account_id":"stand-in"}}"#,
    )
    .unwrap();
    std::fs::write(
        home.join(".claude.json"),
        r#"{"oauthAccount":{"accountUuid":"stand-in"}}"#,
    )
    .unwrap();
    std::fs::write(home.join(".grok/auth.json"), "{}").unwrap();
    std::fs::write(home.join(".local/share/devin/credentials.toml"), "x").unwrap();
    for agent in ["claude", "grok", "devin"] {
        let path = bin.join(agent);
        std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    for args in [
        &["init", "-q", "-b", "main"][..],
        &[
            "-c",
            "user.email=t@example.com",
            "-c",
            "user.name=T",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "first",
        ],
    ] {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }

    // SAFETY: plain libc calls on a descriptor this test owns; the slave's
    // name is copied before any other PTY call.
    let (master, slave) = unsafe {
        let master = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
        assert!(master >= 0, "posix_openpt");
        assert_eq!(libc::grantpt(master), 0, "grantpt");
        assert_eq!(libc::unlockpt(master), 0, "unlockpt");
        let name = libc::ptsname(master);
        assert!(!name.is_null(), "ptsname");
        let path = std::ffi::CStr::from_ptr(name)
            .to_string_lossy()
            .into_owned();
        let size = libc::winsize {
            ws_row: ROWS,
            ws_col: COLS,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        libc::ioctl(master, libc::TIOCSWINSZ as _, &raw const size);
        (std::fs::File::from_raw_fd(master), path)
    };
    let open = || {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&slave)
            .unwrap()
    };
    // The size goes on the slave too: some systems keep it per side.
    let sized = open();
    let size = libc::winsize {
        ws_row: ROWS,
        ws_col: COLS,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: TIOCSWINSZ reads the winsize passed and nothing else.
    unsafe {
        libc::ioctl(
            std::os::fd::AsRawFd::as_raw_fd(&sized),
            libc::TIOCSWINSZ as _,
            &raw const size,
        )
    };
    let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
    command
        .args(["terminal", "--scratch"])
        .current_dir(&repo)
        .env_clear()
        .env("HOME", &home)
        .env("TMPDIR", temp.path())
        .env("PATH", "/usr/bin:/bin")
        .env("TERM", "xterm-256color")
        .env("CLAUDE_BIN", bin.join("claude"))
        .env("GROK_BIN", bin.join("grok"))
        .env("DEVIN_BIN", bin.join("devin"))
        // OpenCode is not set up here; an installed one must not count.
        .env("OPENCODE_BIN", bin.join("opencode-missing"))
        .env("CODER_ONE_OPENCODE_BIN", bin.join("opencode-missing"))
        .stdin(Stdio::from(open()))
        .stdout(Stdio::from(open()))
        .stderr(Stdio::from(open()));
    // SAFETY: only async-signal-safe calls between fork and exec: a new
    // session with the pseudo-terminal as its controlling terminal.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    let screen = Arc::new(Mutex::new(coder_vt::Terminal::new(
        usize::from(ROWS),
        usize::from(COLS),
        0,
    )));
    let mut reader = master.try_clone().unwrap();
    let feed = screen.clone();
    std::thread::spawn(move || {
        let mut buffer = [0; 8192];
        while let Ok(read) = reader.read(&mut buffer) {
            if read == 0 {
                break;
            }
            feed.lock().unwrap().feed(&buffer[..read]);
        }
    });
    let text = || {
        screen
            .lock()
            .unwrap()
            .text()
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let shown = loop {
        let now = text();
        if now.contains("│ Agents") && now.contains("Grok Build") {
            break now;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("no welcome card with the agents; the screen:\n{now}");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    eprintln!("---- welcome ----\n{shown}\n");
    let agents = shown
        .lines()
        .find(|line| line.contains("│ Agents"))
        .unwrap_or_default();
    assert!(
        agents.contains("Codex · Claude Code · Grok Build · Devin"),
        "{shown}"
    );
    // Nothing else: OpenCode is not set up in the stand-in home.
    assert!(!agents.contains("OpenCode"), "{shown}");
    assert!(shown.contains("demo"), "{shown}");
    let mut master = master;
    for _ in 0..2 {
        master.write_all(b"\x03").unwrap();
        master.flush().unwrap();
        std::thread::sleep(Duration::from_millis(300));
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("the screen did not quit; it shows:\n{}", text());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
