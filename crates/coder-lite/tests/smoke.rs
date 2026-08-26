use coder_lite::runtime::{CoderRuntimeSession, Control};
use std::sync::mpsc;
use std::time::Duration;

#[tokio::test]
async fn smoke_openresponses_stream() {
    if env_unset("OPENAGENTS_API_KEY") {
        std::env::set_var("OPENAGENTS_API_KEY", "fake");
    }
    if env_unset("OPENAGENTS_BASE_URL") {
        std::env::set_var("OPENAGENTS_BASE_URL", "https://openagents.com/api/v1");
    }

    let (tx, rx) = mpsc::channel::<Control>();
    let mut session = CoderRuntimeSession::new();

    tokio::spawn(async move {
        let _ = session.execute_turn("hi", tx).await;
    });

    let mut collected = String::new();
    let mut done = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);

    while !done && tokio::time::Instant::now() < deadline {
        match rx.try_recv() {
            Ok(Control::Chunk(c)) => {
                eprint!("{}", c);
                collected.push_str(&c);
            }
            Ok(Control::Done) => done = true,
            Ok(_) => {}
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }

    eprintln!("\n---COLLECTED---");
    eprintln!("{}", collected);
    assert!(!collected.is_empty(), "no response chunks received");
    assert!(!collected.starts_with("[error:"), "received an error: {}", collected);
}

fn env_unset(name: &str) -> bool {
    std::env::var(name).is_err()
}
