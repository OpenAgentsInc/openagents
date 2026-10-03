//! Discovery previews and fetched documents through the CLI.
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    let home = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env("HOME", home.path())
        .env("OPENAGENTS_SETTINGS", home.path().join("settings.json"))
        .env("OPENAGENTS_CHAT_HOME", home.path().join("chat"))
        .env_remove("XDG_RUNTIME_DIR")
        .output()
        .unwrap()
}

#[test]
fn preview_is_not_the_origins_card() {
    let output = run(&["discover", "--origin", "https://example.com"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("preview origin  https://example.com"));
    assert!(text.contains("This checkout would serve"));
    assert!(text.contains("not fetched from the origin"));
    assert!(!text.contains("Fetched from"));
    let output = run(&["--json", "discover"]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["source"], "checkout-preview");
    assert_eq!(
        value["agent_card"]["url"],
        "https://api.typesafe.ai/v1/systemone"
    );
    assert_eq!(
        value["agent_card"]["supportedInterfaces"][1]["url"],
        "https://api.typesafe.ai/v1/classify"
    );
    assert!(value.get("remote").is_none());
}

fn server(status: &str, documents: [&str; 2]) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let status = status.to_owned();
    let documents = documents.map(str::to_owned);
    let handle = std::thread::spawn(move || {
        for (path, body) in [
            "/.well-known/agent-card.json",
            "/.well-known/agent-skills/index.json",
        ]
        .into_iter()
        .zip(documents)
        {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                assert_eq!(stream.read(&mut byte).unwrap(), 1);
                request.push(byte[0]);
            }
            assert!(
                String::from_utf8(request)
                    .unwrap()
                    .starts_with(&format!("GET {path} "))
            );
            write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    (origin, handle)
}

#[test]
fn fetch_shows_the_remote_card_and_skills_not_only_comparison() {
    let (origin, handle) = server(
        "200 OK",
        [
            r#"{"name":"Example remote agent","supportedInterfaces":[{"url":"https://example.com/actual","protocolVersion":"example.v1"}]}"#,
            r#"{"skills":[{"name":"remote-skill","url":"https://example.com/skill","digest":"sha256:remote"}]}"#,
        ],
    );
    let output = run(&["discover", "--origin", &origin, "--fetch"]);
    handle.join().unwrap();
    // A mismatch keeps the existing failure exit status.
    assert!(!output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("differs from this checkout"));
    let fetched = text.split("Fetched from").nth(1).unwrap();
    assert!(fetched.contains("Example remote agent"));
    assert!(fetched.contains("https://example.com/actual"));
    assert!(text.contains("remote-skill"));
}

#[test]
fn fetch_reports_404_without_inventing_a_remote_card() {
    let (origin, handle) = server("404 Not Found", ["{}", "{}"]);
    let output = run(&["discover", "--origin", &origin, "--fetch"]);
    handle.join().unwrap();
    assert!(!output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.matches("HTTP 404").count(), 2);
    assert!(!text.contains("Fetched from"));
    assert!(text.contains("not fetched from the origin"));
}
