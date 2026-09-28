use super::*;

use serde_json::json;

/// A client pointed at `port` on loopback, with no retry, so a test
/// measures one round trip rather than a policy.
fn client(port: u16) -> jev::Client {
    let config = jev::Config::new()
        .api_key("test-no-key")
        .base_url(format!("http://127.0.0.1:{port}"))
        .retry(jev::RetryPolicy {
            max_retries: 0,
            ..jev::RetryPolicy::default()
        });
    jev::Client::new(config).expect("the client builds")
}

/// A port nothing listens on: bound, read back, and closed.
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("the address").port();
    drop(listener);
    port
}

/// One ask's state, which no test here sends anywhere that reads it.
fn state() -> Value {
    json!({ "frames": [], "previous": null, "second_hand": false })
}

fn meta() -> Meta {
    Meta {
        window: 30,
        pose: "pinch".to_string(),
        margin: Some(0.04),
        rules: Vec::new(),
        at: Some((0.5, 0.5)),
    }
}

/// Answers, or what the seam has after `wait` without one.
fn drain(seam: &mut Seam, wait: Duration) -> Vec<Answer> {
    let deadline = Instant::now() + wait;
    loop {
        let answers = seam.take();
        if !answers.is_empty() || Instant::now() >= deadline {
            return answers;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_request_that_fails_comes_back_as_a_sentence_and_frees_the_seam() {
    let mut seam =
        Seam::start(client(closed_port()), "jev-latest".to_string()).expect("the worker starts");
    assert!(seam.ask(meta(), state()).is_ok());
    assert_eq!(seam.counts().asked, 1);
    let answers = drain(&mut seam, Duration::from_secs(10));
    assert_eq!(answers.len(), 1, "one answer for one ask");
    let answer = &answers[0];
    assert_eq!(answer.meta, meta(), "the window comes back with the answer");
    let failure = answer.report.as_ref().expect_err("the request failed");
    assert!(!failure.is_empty(), "the failure says something");
    assert!(!seam.busy(), "the seam takes another ask");
    assert_eq!(seam.counts().skipped(), 0);
}

#[test]
fn an_ask_while_a_request_is_in_flight_is_recorded_rather_than_dropped() {
    // A listener that accepts the connection and answers nothing, so the
    // first request stays in flight for the whole test.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = listener.local_addr().expect("the address").port();
    let mut seam = Seam::start(client(port), "jev-latest".to_string()).expect("the worker starts");
    assert!(seam.ask(meta(), state()).is_ok());
    assert_eq!(
        seam.ask(meta(), state()),
        Err(Skip::InFlight),
        "the second ask names why it sent nothing"
    );
    assert_eq!(seam.ask(meta(), state()), Err(Skip::InFlight));
    let counts = seam.counts();
    assert_eq!(counts.asked, 1);
    assert_eq!(counts.in_flight, 2);
    assert_eq!(counts.skipped(), 2);
    assert_eq!(counts.ambiguous(), 3);
    assert!(seam.busy());
    assert!(seam.take().is_empty(), "nothing answered");
    drop(listener);
}

#[test]
fn the_deadline_is_the_window_the_record_reports() {
    assert_eq!(DEADLINE, Duration::from_millis(1_000));
    assert_eq!(IN_FLIGHT, 1);
}
