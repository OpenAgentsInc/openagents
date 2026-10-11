use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};

const TICKET: &str = "ogb_0123_abcd_secret";
const TOKEN: &str = "ghs_minted_for_one_repository";

fn push(url: &str) -> Push {
    Push::new(
        "octo/hello",
        "main",
        Some("octo"),
        format!("{url}/v1/github/git-credential {TICKET}"),
    )
    .unwrap()
}

#[test]
fn access_names_one_repository_and_never_prints() {
    let push = push("https://openagents.test");
    assert_eq!(push.repository(), "octo/hello");
    assert_eq!(push.name(), "OPENAGENTS_GIT_BROKER");
    assert!(!format!("{push:?}").contains(TICKET));
    let value = |v: &str| v.to_owned();
    for (repo, base, login, credential) in [
        ("octo", "main", None, value("https://a/b t")),
        ("octo/hello/x", "main", None, value("https://a/b t")),
        ("octo/he llo", "main", None, value("https://a/b t")),
        ("octo/hello", "ma'in", None, value("https://a/b t")),
        ("octo/hello", "-main", None, value("https://a/b t")),
        ("octo/hello", "main", Some("oc to"), value("https://a/b t")),
        ("octo/hello", "main", None, value("http://evil.test/b t")),
        ("octo/hello", "main", None, value("https://a/b")),
        ("octo/hello", "main", None, value("https://a/b t u")),
    ] {
        assert!(
            Push::new(repo, base, login, credential).is_err(),
            "{repo} {base} {login:?}"
        );
    }
}

#[test]
fn the_task_says_how_to_push_or_that_it_cant() {
    let push = push("https://openagents.test");
    let note = task_note(Some(&push));
    assert!(note.contains("https://github.com/octo/hello"));
    assert!(note.contains("open-pull-request --title"));
    assert!(note.contains("targets main"));
    assert!(!note.contains(TICKET));
    assert!(task_note(None).contains("no GitHub access"));
}

/// One request a fake server took.
#[derive(Clone, Debug)]
struct Seen {
    line: String,
    head: String,
    body: String,
}

/// A fake broker and GitHub API on one port: the broker answers Git's
/// credential lines for the right ticket, and `POST .../pulls` opens a
/// pull request.
fn serve() -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut bytes = Vec::new();
            let mut buf = [0u8; 4096];
            let (head, body) = loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break (String::from_utf8_lossy(&bytes).into_owned(), String::new());
                }
                bytes.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&bytes).into_owned();
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let length = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if body.len() >= length {
                        break (head.to_owned(), body.to_owned());
                    }
                }
            };
            let line = head.lines().next().unwrap_or_default().to_owned();
            let answer = if line.starts_with("POST /v1/github/git-credential") {
                if body.contains(&format!("ticket={TICKET}"))
                    && body.contains("path=octo/hello.git")
                {
                    (200, format!("username=x-access-token\npassword={TOKEN}\n"))
                } else {
                    (403, "error=wrong_repository\n".to_owned())
                }
            } else if line.starts_with("POST /repos/octo/hello/pulls") {
                (
                    201,
                    r#"{"url":"https://api.github.com/repos/octo/hello/pulls/7","id":1,"html_url":"https://github.com/octo/hello/pull/7","user":{"html_url":"https://github.com/octo"}}"#.to_owned(),
                )
            } else {
                (404, r#"{"message":"Not Found"}"#.to_owned())
            };
            log.lock().unwrap().push(Seen { line, head, body });
            let _ = write!(
                stream,
                "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                answer.0,
                answer.1.len(),
                answer.1
            );
        }
    });
    (url, seen)
}

fn sh(script: &str, home: &std::path::Path, broker: &str) -> std::process::Output {
    Command::new("sh")
        .arg("-c")
        .arg(script)
        .env("HOME", home)
        .env("OPENAGENTS_GIT_BROKER", broker)
        .env_remove("GIT_CONFIG_GLOBAL")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap()
}

#[test]
fn a_run_pushes_through_the_broker_and_opens_a_pull_request_without_the_token_showing() {
    let (url, seen) = serve();
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&repo)
            .env("HOME", &home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap()
    };
    assert!(git(&["init", "-q"]).status.success());
    // A helper someone configured earlier never answers for this run.
    assert!(
        git(&["config", "credential.helper", "store"])
            .status
            .success()
    );

    let push = push(&url);
    let broker = push.value();
    let workdir = repo.display().to_string();
    let script = setup_script(&push, &workdir, &url);
    assert!(
        !script.contains(TICKET),
        "the ticket stays in the environment"
    );
    let out = sh(&script, &home, &broker);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let config = String::from_utf8(git(&["config", "--list", "--local"]).stdout).unwrap();
    assert!(
        !config.contains(TICKET) && !config.contains(TOKEN),
        "{config}"
    );
    assert!(!config.contains("credential.helper=store"), "{config}");
    let origin = String::from_utf8(git(&["remote", "get-url", "origin"]).stdout).unwrap();
    assert_eq!(origin.trim(), "https://github.com/octo/hello.git");
    let name = String::from_utf8(git(&["config", "user.name"]).stdout).unwrap();
    assert_eq!(name.trim(), "octo");

    // Git's push asks the broker for this repository's token.
    let fill = sh(
        &format!(
            "cd '{workdir}' && printf 'protocol=https\\nhost=github.com\\npath=octo/hello.git\\n\\n' | GIT_TERMINAL_PROMPT=0 git credential fill"
        ),
        &home,
        &broker,
    );
    let lines = String::from_utf8_lossy(&fill.stdout);
    assert!(lines.contains(&format!("password={TOKEN}")), "{lines}");
    // Another repository gets nothing.
    let other = sh(
        &format!(
            "cd '{workdir}' && printf 'protocol=https\\nhost=github.com\\npath=octo/other.git\\n\\n' | GIT_TERMINAL_PROMPT=0 git credential fill"
        ),
        &home,
        &broker,
    );
    assert!(!String::from_utf8_lossy(&other.stdout).contains(TOKEN));

    // Open a pull request with a title and body that need escaping.
    std::fs::write(
        root.path().join("body.md"),
        "Fixes #1.\n\nSays \"hi\" \\ bye.\n",
    )
    .unwrap();
    let opened = sh(
        &format!(
            "cd '{workdir}' && \"$HOME/.local/bin/open-pull-request\" --title 'Fix \"it\"' --body-file '{}' --head fix-it",
            root.path().join("body.md").display()
        ),
        &home,
        &broker,
    );
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&opened.stdout),
        String::from_utf8_lossy(&opened.stderr)
    );
    assert!(opened.status.success(), "{printed}");
    assert_eq!(printed.trim(), "https://github.com/octo/hello/pull/7");
    assert!(!printed.contains(TOKEN));
    let seen = seen.lock().unwrap().clone();
    let pull = seen
        .iter()
        .find(|s| s.line.starts_with("POST /repos/octo/hello/pulls"))
        .expect("the pull request was asked for");
    assert!(
        pull.head
            .contains(&format!("Authorization: Bearer {TOKEN}")),
        "{}",
        pull.head
    );
    let body: serde_json::Value = serde_json::from_str(&pull.body).unwrap();
    assert_eq!(body["title"], "Fix \"it\"");
    assert_eq!(body["head"], "fix-it");
    assert_eq!(body["base"], "main");
    assert_eq!(body["body"], "Fixes #1.\n\nSays \"hi\" \\ bye.");

    // Without a title it says how to use it and asks nothing.
    let usage = sh("\"$HOME/.local/bin/open-pull-request\"", &home, &broker);
    assert_eq!(usage.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("usage: open-pull-request"));
}
