//! The streaming door against a door that is there, and against three
//! that are not.
//!
//! Every test here runs the real [`coder::generate::ResponsesDoor`]
//! against a stub server on loopback, because the things being tested are
//! the door's bounds and its event mapping, and both live between the
//! socket and the answer.
//!
//! Two of the stubs are the failures openagents#9439 is about: a door that
//! accepts a request and sends nothing, and a door that starts answering
//! and stops. Before the bounds landed, either one held a turn open with
//! no limit.
//!
//! The recorded streams under `crates/coder/fixtures/gateway/` are real
//! gateway responses, one per lane. `crates/coder/fixtures/gateway/README.md`
//! says what they hold and how to record them again.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use coder::generate::{Generate, GenerateError, Lane, Message, Patience, ResponsesDoor, Role};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The `gemini` lane's recorded stream.
const GEMINI: &str = include_str!("../fixtures/gateway/google-gemini-3.8-flash.sse");

/// The `glm` lane's recorded stream.
const GLM: &str = include_str!("../fixtures/gateway/zai-glm-5.3-flash.sse");

/// Waits short enough to spend in a test, in the proportions the real ones
/// hold: the headers wait is the short one, the silence between events is
/// the long one, and the pause between attempts is shorter than both.
fn short() -> Patience {
    Patience {
        first_word: Duration::from_millis(200),
        quiet: Duration::from_millis(400),
        whole: Duration::from_secs(5),
        retry_wait: Duration::from_millis(10),
    }
}

/// What the stub server does once it has read a request.
#[derive(Clone, Copy)]
enum Stub {
    /// Answer with these bytes and close.
    Whole(&'static str),
    /// Take the request and send nothing at all, not even headers.
    Deaf,
    /// Send response headers, then nothing.
    Mute,
    /// Send response headers and the first few events, then nothing.
    Halfway,
    /// Send the first few events and close, as if the door died.
    Truncated,
    /// Send keepalive events forever, never completing.
    Endless,
}

/// A stub door on loopback. Dropping it stops the server.
struct Server {
    url: String,
    asked: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    /// A server behaving as `stub` for every request it is given.
    async fn start(stub: Stub) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
        let port = listener.local_addr().expect("an address").port();
        let asked = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&asked);
        let task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                counted.fetch_add(1, Ordering::Relaxed);
                tokio::spawn(answer(socket, stub));
            }
        });
        Server {
            url: format!("http://127.0.0.1:{port}"),
            asked,
            task,
        }
    }

    /// How many requests have reached it.
    fn asked(&self) -> usize {
        self.asked.load(Ordering::Relaxed)
    }

    /// A door onto it, with the short waits and no retries to wait out
    /// that the caller does not ask for.
    fn door(&self, model: &str) -> ResponsesDoor {
        ResponsesDoor::new(self.url.as_str(), model, "a-key").waiting(short())
    }
}

/// Reads one request and answers it as `stub` says.
async fn answer(mut socket: TcpStream, stub: Stub) {
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    // Read to the end of the request headers, then read the body the
    // door's content-length promises. A server that answers without
    // reading the request it was sent leaves bytes in the receive buffer,
    // and closing on those resets the connection instead of ending it.
    while socket.read_exact(&mut byte).await.is_ok() {
        request.push(byte[0]);
        if request.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let headers = String::from_utf8_lossy(&request).to_lowercase();
    let length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    let _ = socket.read_exact(&mut body).await;
    if matches!(stub, Stub::Deaf) {
        // Hold the connection open and say nothing. This is the failure
        // that used to have no bound at all.
        forever(socket).await;
        return;
    }
    let answer = "HTTP/1.1 200 OK\r\n\
                  content-type: text/event-stream\r\n\
                  connection: close\r\n\r\n";
    if socket.write_all(answer.as_bytes()).await.is_err() {
        return;
    }
    match stub {
        Stub::Whole(body) => {
            let _ = socket.write_all(body.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
        Stub::Halfway => {
            // Three whole events and then silence: the caller has seen
            // part of the answer.
            let cut = GEMINI
                .match_indices("event: response.output_text.delta")
                .nth(1)
                .map(|(at, _)| at)
                .expect("the recording has two text deltas");
            let _ = socket.write_all(&GEMINI.as_bytes()[..cut]).await;
            let _ = socket.flush().await;
            forever(socket).await;
        }
        Stub::Truncated => {
            let cut = GEMINI
                .match_indices("event: response.output_text.delta")
                .nth(1)
                .map(|(at, _)| at)
                .expect("the recording has two text deltas");
            let _ = socket.write_all(&GEMINI.as_bytes()[..cut]).await;
            let _ = socket.shutdown().await;
        }
        Stub::Endless => {
            let event = b"data: {\"type\":\"response.in_progress\"}\n\n";
            loop {
                if socket.write_all(event).await.is_err() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        Stub::Mute | Stub::Deaf => forever(socket).await,
    }
}

/// Holds a socket open without writing to it.
async fn forever(socket: TcpStream) {
    let _held = socket;
    std::future::pending::<()>().await;
}

/// One user turn, which is all any of these need.
fn turn() -> Vec<Message> {
    vec![Message {
        role: Role::User,
        text: "count from one to five".to_string(),
    }]
}

/// Runs a turn and answers with the text, the deltas, and the outcome.
async fn ask(door: &ResponsesDoor) -> (String, Result<(String, Option<u64>), GenerateError>) {
    let mut seen = String::new();
    let answered = door
        .generate(
            "you are terse",
            &turn(),
            &mut |delta| seen.push_str(delta),
            &mut |_| {},
        )
        .await;
    let outcome = answered.map(|(text, usage)| (text, usage.map(|usage| usage.output_tokens)));
    (seen, outcome)
}

/// Both lanes stream through one reader, and both arrive whole.
///
/// This is the claim a second lane rests on: the gateway serves one event
/// shape for every model in its catalog, so a second model is
/// configuration rather than a second client. It is checked against real
/// recordings from both, rather than reasoned about.
#[tokio::test]
async fn both_lanes_read_through_one_reader() {
    for (lane, recording, answer, output_tokens) in [
        (Lane::Gemini, GEMINI, "One\nTwo\nThree\nFour\nFive", 130),
        (Lane::Glm, GLM, "one\ntwo\nthree\nfour\nfive", 45),
    ] {
        let server = Server::start(Stub::Whole(recording)).await;
        let (seen, outcome) = ask(&server.door(lane.model())).await;
        let (text, usage) = outcome.unwrap_or_else(|error| panic!("{}: {error}", lane.name()));
        assert_eq!(text, answer, "{}", lane.name());
        assert_eq!(
            seen,
            answer,
            "{} streamed the answer it returned",
            lane.name()
        );
        assert_eq!(usage, Some(output_tokens), "{}", lane.name());
    }
}

/// A thinking lane's reasoning is not the answer.
///
/// The `glm` recording carries 34 `response.reasoning.delta` events. A
/// reader that treated an unknown delta as text would splice the model's
/// reasoning into what the person reads, and the answer above would still
/// be a substring of it.
#[tokio::test]
async fn a_lanes_reasoning_does_not_reach_the_answer() {
    let reasoning = GLM.matches("\"type\":\"response.reasoning.delta\"").count();
    assert_eq!(reasoning, 34, "the recording still carries its reasoning");

    let server = Server::start(Stub::Whole(GLM)).await;
    let (_, outcome) = ask(&server.door(Lane::Glm.model())).await;
    let (text, _) = outcome.expect("the glm lane answers");
    assert_eq!(text, "one\ntwo\nthree\nfour\nfive");
}

/// A door that accepts a request and sends nothing is asked again, and
/// then reported.
///
/// Three attempts, so three requests reach the server. Without the bound
/// this test would not finish.
#[tokio::test]
async fn a_door_that_never_answers_is_asked_again_and_then_reported() {
    let server = Server::start(Stub::Deaf).await;
    let started = Instant::now();
    let (seen, outcome) = ask(&server.door(Lane::Gemini.model())).await;
    let error = outcome.expect_err("a door that says nothing fails");

    assert_eq!(error.cause(), "door_absent");
    assert_eq!(
        error.to_string(),
        "the door did not answer: no response headers in 0 seconds, over 3 attempts"
    );
    assert_eq!(
        server.asked(),
        3,
        "a door that sent no headers is asked again"
    );
    assert!(seen.is_empty(), "nothing streamed");
    // Three 200 ms waits and two pauses, and nowhere near the 120 seconds
    // the silence bound would allow.
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

/// A door that sends its headers and then nothing fails on the silence
/// bound, says how long it waited, and is not asked again.
#[tokio::test]
async fn a_door_that_sends_headers_and_stops_is_not_asked_again() {
    let server = Server::start(Stub::Mute).await;
    let (seen, outcome) = ask(&server.door(Lane::Gemini.model())).await;
    let error = outcome.expect_err("a door that sends nothing fails");

    assert_eq!(error.cause(), "door_stalled");
    assert_eq!(
        error.to_string(),
        "the door went quiet mid-answer: 0 seconds of silence after 0 events and 0 characters"
    );
    assert_eq!(server.asked(), 1, "a door that answered is not asked again");
    assert!(seen.is_empty());
}

/// A door that streams part of an answer and stops names the wait and
/// what had arrived, and is not asked again.
///
/// "It hung" and "it sent some of the answer and stopped" are different
/// problems. The failure says which one it met, and the partial answer is
/// why a second attempt would be wrong: the caller has already seen it.
#[tokio::test]
async fn a_partial_answer_says_how_much_arrived_and_is_not_repeated() {
    let server = Server::start(Stub::Halfway).await;
    let (seen, outcome) = ask(&server.door(Lane::Gemini.model())).await;
    let error = outcome.expect_err("a stream that stops fails");

    assert_eq!(error.cause(), "door_stalled");
    assert_eq!(
        error.to_string(),
        "the door went quiet mid-answer: 0 seconds of silence after 5 events and 4 characters"
    );
    assert_eq!(seen, "One\n", "the deltas that arrived reached the caller");
    assert_eq!(
        server.asked(),
        1,
        "a partial answer is never asked for twice"
    );
}

/// A door that streams part of an answer and closes did not answer: the
/// stream ended before `response.completed`, and what arrived is in the
/// failure rather than presented as the reply.
#[tokio::test]
async fn a_stream_that_closes_before_completing_is_not_an_answer() {
    let server = Server::start(Stub::Truncated).await;
    let (seen, outcome) = ask(&server.door(Lane::Gemini.model())).await;
    let error = outcome.expect_err("a stream that ends early fails");

    assert_eq!(error.cause(), "stream");
    assert_eq!(
        error.to_string(),
        "stream: the stream ended before response.completed, after 5 events"
    );
    assert_eq!(seen, "One\n");
    assert_eq!(server.asked(), 1, "a partial answer is not asked for twice");
}

/// A door that never stops sending events is bounded by the whole-attempt
/// wait, not only by the silence between events.
#[tokio::test]
async fn a_door_that_talks_forever_is_bounded() {
    let server = Server::start(Stub::Endless).await;
    let door =
        ResponsesDoor::new(server.url.as_str(), Lane::Gemini.model(), "a-key").waiting(Patience {
            whole: Duration::from_millis(500),
            ..short()
        });
    let started = Instant::now();
    let (_, outcome) = ask(&door).await;
    let error = outcome.expect_err("an endless stream fails");

    assert_eq!(error.cause(), "door_stalled");
    assert!(
        error.to_string().contains("0 seconds without completing"),
        "{error}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(server.asked(), 1);
}

/// Model names live in one file.
///
/// `generate.rs` is where a lane's model id is written, and a caller that
/// needs one names a lane. Four places to change a model — a door, a
/// worker, a runtime, and a bench — is how a lane switch stops being
/// configuration, and how door identity comes to be two different things
/// depending on which one was edited last. This is the claim checked
/// rather than stated.
#[test]
fn a_model_id_is_written_in_one_place() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut elsewhere = Vec::new();
    let mut files = vec![source];
    while let Some(path) = files.pop() {
        if path.is_dir() {
            let listed = std::fs::read_dir(&path).expect("the source tree reads");
            files.extend(listed.map(|entry| entry.expect("an entry").path()));
            continue;
        }
        if path.extension().is_none_or(|kind| kind != "rs")
            || path.file_name().is_some_and(|name| name == "generate.rs")
        {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a source file reads");
        for lane in Lane::ALL {
            if text.contains(lane.model()) {
                elsewhere.push(format!("{} names {}", path.display(), lane.model()));
            }
        }
    }
    assert!(elsewhere.is_empty(), "{elsewhere:#?}");
}

/// The bounds are the reference implementation's, and a test that shortens
/// them does not change what the service runs.
#[test]
fn the_bounds_are_thirty_seconds_two_minutes_and_a_second() {
    let patience = Patience::default();
    assert_eq!(patience.first_word, Duration::from_secs(30));
    assert_eq!(patience.quiet, Duration::from_secs(120));
    assert_eq!(patience.whole, Duration::from_secs(600));
    assert_eq!(patience.retry_wait, Duration::from_secs(1));
    assert_eq!(coder::generate::CONNECT_TIMEOUT, Duration::from_secs(10));
}
