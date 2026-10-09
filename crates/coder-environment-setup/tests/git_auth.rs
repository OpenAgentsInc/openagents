//! Ephemeral Git auth against the real `git`. A separate test binary, so
//! its child processes never share a store lease descriptor with the
//! session tests.

use coder_environment_setup::git_auth_env;

const GH_SECRET: &str = "ghp_setup_secret_value_0123456789";

/// Real Git: the per-process helper authenticates, and neither the clone's
/// `.git/config` nor any file under `.git` holds the token.
#[test]
fn ephemeral_git_auth_never_writes_the_token() {
    use std::process::{Command, Stdio};
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let git = |args: &[&str], auth: bool| {
        let mut c = Command::new("git");
        c.args(args)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", &home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if auth {
            c.envs(git_auth_env("GH_TOKEN")).env("GH_TOKEN", GH_SECRET);
        }
        c
    };
    let source = dir.path().join("source.git");
    let source = source.to_str().unwrap();
    assert!(
        git(&["init", "--bare", "-q", source], false)
            .status()
            .unwrap()
            .success()
    );

    // The helper answers from the variable at run time.
    let mut fill = git(&["credential", "fill"], true).spawn().unwrap();
    use std::io::Write;
    fill.stdin
        .take()
        .unwrap()
        .write_all(b"protocol=https\nhost=github.com\n\n")
        .unwrap();
    let out = fill.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(&format!("password={GH_SECRET}")), "{text}");
    assert!(text.contains("username=x-access-token"));

    // Any other host, or plain http, gets nothing: a setup command's Git
    // dependency or submodule elsewhere is never handed the token.
    for asked in [
        &b"protocol=https\nhost=evil.example\n\n"[..],
        b"protocol=https\nhost=github.com.evil.example\n\n",
        b"protocol=http\nhost=github.com\n\n",
    ] {
        let mut fill = git(&["credential", "fill"], true).spawn().unwrap();
        fill.stdin.take().unwrap().write_all(asked).unwrap();
        let out = fill.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(!text.contains(GH_SECRET), "{text}");
    }

    let work = dir.path().join("work");
    let work_s = work.to_str().unwrap();
    assert!(
        git(&["clone", "-q", source, work_s], true)
            .status()
            .unwrap()
            .success()
    );
    let config = std::fs::read_to_string(work.join(".git/config")).unwrap();
    assert!(!config.contains(GH_SECRET));
    assert!(!config.contains("credential"));
    let mut stack = vec![work.join(".git")];
    while let Some(p) = stack.pop() {
        for entry in std::fs::read_dir(&p).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                assert!(
                    !bytes
                        .windows(GH_SECRET.len())
                        .any(|w| w == GH_SECRET.as_bytes()),
                    "{path:?} holds the token"
                );
            }
        }
    }
    // Only the process environment carries the helper.
    let origin = git(
        &[
            "-C",
            work_s,
            "config",
            "--show-origin",
            "--get-all",
            "credential.helper",
        ],
        true,
    )
    .output()
    .unwrap();
    let origin = String::from_utf8_lossy(&origin.stdout);
    assert!(
        origin.lines().all(|l| l.starts_with("command line:")),
        "{origin}"
    );
    let none = git(
        &["-C", work_s, "config", "--get-all", "credential.helper"],
        false,
    )
    .output()
    .unwrap();
    assert!(!none.status.success());
}

/// A one-route broker on loopback: answers every request with `answer`
/// (`"<status line>|<body>"`) and keeps the bodies it was sent.
fn broker(answer: &'static str) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "http://{}/v1/github/git-credential",
        listener.local_addr().unwrap()
    );
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let kept = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let line = line.trim_end();
                if line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; length];
            reader.read_exact(&mut body).ok();
            kept.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&body).into_owned());
            let (status, text) = answer.split_once('|').unwrap();
            write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                text.len()
            )
            .ok();
        }
    });
    (url, seen)
}

/// Real Git and curl against a broker: the helper holds only the broker's
/// address and a ticket, asks for `https://github.com/<owner>/<name>`
/// only, and passes on the broker's one-repository token. Another host
/// never reaches the broker; a refusal gives Git nothing.
#[test]
fn the_broker_helper_asks_the_broker_for_github_repositories_only() {
    use coder_environment_setup::GIT_BROKER;
    use std::io::Write;
    use std::process::{Command, Stdio};
    if Command::new("git").arg("--version").output().is_err()
        || Command::new("curl").arg("--version").output().is_err()
    {
        return;
    }
    let env = git_auth_env(GIT_BROKER);
    assert!(
        env.values()
            .all(|v| !v.contains("ogb_") && !v.contains("ghs_")),
        "the helper text holds no ticket or token"
    );
    let dir = tempfile::tempdir().unwrap();
    let fill = |broker_value: &str, asked: &[u8]| {
        let mut c = Command::new("git");
        c.args(["credential", "fill"])
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", dir.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .envs(git_auth_env(GIT_BROKER))
            .env(GIT_BROKER, broker_value)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        child.stdin.take().unwrap().write_all(asked).unwrap();
        let out = child.wait_with_output().unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };

    let (url, seen) = broker(
        "200 OK|username=x-access-token\npassword=ghs_brokered_0123456789\npassword_expiry_utc=4102444800\n",
    );
    let value = format!("{url} ogb_ticket_0123");
    let text = fill(
        &value,
        b"protocol=https\nhost=github.com\npath=octo-local/hello-world.git\n\n",
    );
    assert!(text.contains("password=ghs_brokered_0123456789"), "{text}");
    assert!(text.contains("username=x-access-token"), "{text}");
    let bodies = seen.lock().unwrap().clone();
    assert_eq!(
        bodies,
        ["ticket=ogb_ticket_0123&protocol=https&host=github.com&path=octo-local/hello-world.git"]
    );

    // Another host, plain http, or a path with odd characters: the broker
    // never hears of it and Git gets nothing.
    for asked in [
        &b"protocol=https\nhost=evil.example\npath=octo-local/hello-world.git\n\n"[..],
        b"protocol=https\nhost=github.com.evil.example\npath=a/b\n\n",
        b"protocol=http\nhost=github.com\npath=a/b\n\n",
        b"protocol=https\nhost=github.com\npath=a/b&ticket=x\n\n",
    ] {
        let text = fill(&value, asked);
        assert!(!text.contains("password="), "{text}");
    }
    assert_eq!(seen.lock().unwrap().len(), 1);

    // A broker that refuses: no password, and nothing older to use.
    let (refusing, _) = broker("401 Unauthorized|error=ticket_refused\n");
    let text = fill(
        &format!("{refusing} ogb_ticket_0123"),
        b"protocol=https\nhost=github.com\npath=octo-local/hello-world.git\n\n",
    );
    assert!(!text.contains("password="), "{text}");
    // A broker address that isn't https (or loopback) is never called.
    let text = fill(
        "http://broker.example/v1/github/git-credential ogb_ticket_0123",
        b"protocol=https\nhost=github.com\npath=octo-local/hello-world.git\n\n",
    );
    assert!(!text.contains("password="), "{text}");
}
