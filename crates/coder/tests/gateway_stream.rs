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

use coder::generate::{
    FallbackDoor, Generate, GenerateError, Lane, Message, Meta, Patience, ResponsesDoor, Role,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The `gemini` lane's recorded stream.
const GEMINI: &str = include_str!("../fixtures/gateway/google-gemini-3.8-flash.sse");

/// The `glm` lane's recorded stream.
const GLM: &str = include_str!("../fixtures/gateway/zai-glm-5.3-flash.sse");

/// The `space-bunny` lane's recorded stream, from OpenRouter.
const SPACE_BUNNY: &str = include_str!("../fixtures/gateway/stealth-space-bunny-alpha.sse");

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
    /// Send reasoning events through this wait, then Space Bunny Alpha's
    /// recorded answer: a model thinking before it answers.
    Thinking(Duration),
    /// Answer with this HTTP error status and a short JSON body, as
    /// OpenRouter does for a model it no longer serves (404) or a rate
    /// limit (429).
    Status(u16),
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
    if let Stub::Status(status) = stub {
        let body = format!("{{\"error\":{{\"code\":{status},\"message\":\"stub\"}}}}");
        let answer = format!(
            "HTTP/1.1 {status} Stub\r\ncontent-type: application/json\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(answer.as_bytes()).await;
        let _ = socket.shutdown().await;
        return;
    }
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
        Stub::Thinking(wait) => {
            // Two reasoning events half the wait apart, so the stream is
            // never quiet for the door's own silence bound.
            let event = b"data: {\"type\":\"response.reasoning_text.delta\",\"delta\":\"hm\"}\n\n";
            for _ in 0..2 {
                if socket.write_all(event).await.is_err() {
                    return;
                }
                let _ = socket.flush().await;
                tokio::time::sleep(wait / 2).await;
            }
            let _ = socket.write_all(SPACE_BUNNY.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
        Stub::Mute | Stub::Deaf => forever(socket).await,
        Stub::Status(_) => {}
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
        (
            Lane::SpaceBunny,
            SPACE_BUNNY,
            "one\ntwo\nthree\nfour\nfive",
            53,
        ),
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
        "the model endpoint did not answer: no response headers in 0 seconds, over 3 attempts"
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
        "the model endpoint stopped sending partway through its answer: 0 seconds of silence after 0 events and 0 characters"
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
        "the model endpoint stopped sending partway through its answer: 0 seconds of silence after 5 events and 4 characters"
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

/// A primary on `primary` in front of a fallback on `fallback`, with a
/// first-word wait short enough to spend in a test.
fn ordered(primary: &Server, fallback: &Server) -> FallbackDoor {
    FallbackDoor::new(
        primary.door(Lane::SpaceBunny.model()),
        fallback.door(Lane::Gemini.model()),
    )
    .first_word(Duration::from_millis(300), Duration::from_millis(900))
}

/// Runs a turn through a fallback door: the deltas, the outcome, and the
/// model the door named as the writer.
async fn ask_ordered(
    door: &FallbackDoor,
) -> (
    String,
    Result<(String, Option<u64>), GenerateError>,
    Option<String>,
) {
    let mut seen = String::new();
    let mut named = None;
    let answered = door
        .generate(
            "you are terse",
            &turn(),
            &mut |delta| seen.push_str(delta),
            &mut |meta| {
                if let Meta::Model(model) = meta {
                    named = Some(model);
                }
            },
        )
        .await;
    let outcome = answered.map(|(text, usage)| (text, usage.map(|usage| usage.output_tokens)));
    (seen, outcome, named)
}

/// The primary answers, names itself, and the fallback is never asked.
#[tokio::test]
async fn a_primary_that_answers_names_itself() {
    let primary = Server::start(Stub::Whole(SPACE_BUNNY)).await;
    let fallback = Server::start(Stub::Whole(GEMINI)).await;
    let door = ordered(&primary, &fallback);
    let (seen, outcome, named) = ask_ordered(&door).await;
    let (text, _) = outcome.expect("the primary answers");
    assert_eq!(text, "one\ntwo\nthree\nfour\nfive");
    assert_eq!(seen, text);
    assert_eq!(named.as_deref(), Some(Lane::SpaceBunny.model()));
    assert_eq!(fallback.asked(), 0);
    assert!(!door.primary_down());
    assert_eq!(door.answering().model, Lane::SpaceBunny.model());
}

/// A model OpenRouter no longer serves (404, as Space Bunny Alpha will be
/// after 2026-10-05) and a rate limit (429) both hand the same turn to the
/// fallback, which names itself; the next turn asks the primary again.
#[tokio::test]
async fn an_error_status_before_the_first_word_falls_back() {
    for status in [404, 429, 500] {
        let primary = Server::start(Stub::Status(status)).await;
        let fallback = Server::start(Stub::Whole(GEMINI)).await;
        let door = ordered(&primary, &fallback);
        let (seen, outcome, named) = ask_ordered(&door).await;
        let (text, _) = outcome.unwrap_or_else(|error| panic!("{status}: {error}"));
        assert_eq!(text, "One\nTwo\nThree\nFour\nFive", "{status}");
        assert_eq!(seen, text, "{status}: only the fallback's words were shown");
        assert_eq!(named.as_deref(), Some(Lane::Gemini.model()), "{status}");
        assert_eq!((primary.asked(), fallback.asked()), (1, 1), "{status}");
        assert!(door.primary_down(), "{status}");
        assert_eq!(door.answering().model, Lane::Gemini.model());

        // Per turn: the next one asks the primary first again, unless its
        // model is gone (404), which benches it for a while.
        let _ = ask_ordered(&door).await;
        let again = if status == 404 { 1 } else { 2 };
        assert_eq!(primary.asked(), again, "{status}");
        assert!(door.primary_down(), "{status}");
    }
}

/// A primary that takes the request and sends no answer text within the
/// first-word wait loses the turn to the fallback, whether it sent no
/// headers at all or headers and then nothing.
#[tokio::test]
async fn a_primary_silent_past_the_first_word_wait_falls_back() {
    for stub in [Stub::Deaf, Stub::Mute] {
        let primary = Server::start(stub).await;
        let fallback = Server::start(Stub::Whole(GEMINI)).await;
        let door = ordered(&primary, &fallback);
        let started = Instant::now();
        let (_, outcome, named) = ask_ordered(&door).await;
        outcome.expect("the fallback answers");
        assert_eq!(named.as_deref(), Some(Lane::Gemini.model()));
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "the fallback took over after the first-word wait, not the door's own bounds: {:?}",
            started.elapsed()
        );
        assert!(door.primary_down());
    }
}

/// A primary that fails after its first words reached the caller has
/// shown part of an answer: the failure is the turn's, and the fallback is
/// not asked to repeat it.
#[tokio::test]
async fn a_primary_that_fails_after_its_first_words_is_not_redone() {
    let primary = Server::start(Stub::Halfway).await;
    let fallback = Server::start(Stub::Whole(GEMINI)).await;
    let door = ordered(&primary, &fallback);
    let (seen, outcome, named) = ask_ordered(&door).await;
    let error = outcome.expect_err("a primary that stopped mid-answer fails the turn");
    assert!(!seen.is_empty(), "the caller saw the primary's first words");
    assert!(
        matches!(error, GenerateError::Quiet { heard: true, .. }),
        "{error}"
    );
    assert_eq!(named, None);
    assert_eq!(fallback.asked(), 0);
    assert!(!door.primary_down());
}

/// Both doors failing is the fallback's failure, as a single door's would
/// be.
#[tokio::test]
async fn both_doors_failing_is_the_fallbacks_failure() {
    let primary = Server::start(Stub::Status(404)).await;
    let fallback = Server::start(Stub::Status(503)).await;
    let (_, outcome, named) = ask_ordered(&ordered(&primary, &fallback)).await;
    let error = outcome.expect_err("nothing answered");
    assert!(matches!(error, GenerateError::Status(503, _)), "{error}");
    assert_eq!(named, None);
}

/// A primary streaming its reasoning past the first-word wait is working,
/// not absent: its answer is waited for up to the thinking bound, and the
/// fallback is not asked.
#[tokio::test]
async fn a_primary_that_is_thinking_is_waited_for() {
    let primary = Server::start(Stub::Thinking(Duration::from_millis(600))).await;
    let fallback = Server::start(Stub::Whole(GEMINI)).await;
    let door = ordered(&primary, &fallback);
    let (_, outcome, named) = ask_ordered(&door).await;
    let (text, _) = outcome.expect("the primary answers after thinking");
    assert_eq!(text, "one\ntwo\nthree\nfour\nfive");
    assert_eq!(named.as_deref(), Some(Lane::SpaceBunny.model()));
    assert_eq!(fallback.asked(), 0);
}

/// A primary that keeps working without an answer loses the turn at the
/// thinking bound, not at the door's own whole-attempt bound.
#[tokio::test]
async fn a_primary_that_thinks_past_the_bound_falls_back() {
    let primary = Server::start(Stub::Endless).await;
    let fallback = Server::start(Stub::Whole(GEMINI)).await;
    let door = ordered(&primary, &fallback);
    let started = Instant::now();
    let (_, outcome, named) = ask_ordered(&door).await;
    outcome.expect("the fallback answers");
    assert_eq!(named.as_deref(), Some(Lane::Gemini.model()));
    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(900) && took < Duration::from_secs(3),
        "{took:?}"
    );
}

/// Three doors on `servers`, in order, with the test's short first-word
/// waits, each running the model in `models`.
fn chain(servers: [&Server; 3], models: [&str; 3]) -> FallbackDoor {
    FallbackDoor::new(servers[0].door(models[0]), servers[1].door(models[1]))
        .with_backups(vec![servers[2].door(models[2])])
        .first_word(Duration::from_millis(300), Duration::from_millis(900))
}

/// 2026-10-09: the primary's account is empty (402) and the fallback's
/// model is gone (404); the third door answers, names itself, and the
/// caller sees only its words. The next turn skips both benched doors.
#[tokio::test]
async fn a_402_then_a_404_then_an_answer_is_answered() {
    let primary = Server::start(Stub::Status(402)).await;
    let fallback = Server::start(Stub::Status(404)).await;
    let backup = Server::start(Stub::Whole(GLM)).await;
    let door = chain(
        [&primary, &fallback, &backup],
        [
            Lane::Gemini.model(),
            Lane::SpaceBunny.model(),
            Lane::Glm.model(),
        ],
    );
    let (seen, outcome, named) = ask_ordered(&door).await;
    let (text, _) = outcome.expect("the third door answers");
    assert!(!text.is_empty());
    assert_eq!(seen, text, "only the answering door's words were shown");
    assert_eq!(named.as_deref(), Some(Lane::Glm.model()));
    assert_eq!(
        (primary.asked(), fallback.asked(), backup.asked()),
        (1, 1, 1)
    );
    assert!(door.primary_down());
    assert_eq!(door.answering().model, Lane::Glm.model());

    // Both refusals repeat on the next turn, so both doors sit it out.
    let (_, outcome, named) = ask_ordered(&door).await;
    outcome.expect("the backup answers again");
    assert_eq!(named.as_deref(), Some(Lane::Glm.model()));
    assert_eq!(
        (primary.asked(), fallback.asked(), backup.asked()),
        (1, 1, 2)
    );
}

/// The switches a turn through `door` names ([`Meta::Switched`]).
async fn switches(door: &FallbackDoor) -> Vec<coder::generate::Switched> {
    let mut seen = Vec::new();
    let _ = door
        .generate("you are terse", &turn(), &mut |_| {}, &mut |meta| {
            if let Meta::Switched(switched) = meta {
                seen.push(switched);
            }
        })
        .await;
    seen
}

/// A turn another door answered names the first door that missed it, why,
/// and who answered, once (#11132): a refusal, an error, and silence each
/// in their own word; a benched door skipped on a later turn is named with
/// the reason it was benched; a primary that answers names no switch.
#[tokio::test]
async fn a_turn_another_door_answered_says_which_missed_it_and_why() {
    use coder::generate::{Missed, Provider};
    for (stub, why) in [
        (Stub::Status(402), Missed::Refused),
        (Stub::Status(429), Missed::Refused),
        (Stub::Status(500), Missed::Error),
        (Stub::Deaf, Missed::Timeout),
    ] {
        let primary = Server::start(stub).await;
        let fallback = Server::start(Stub::Status(404)).await;
        let backup = Server::start(Stub::Whole(GLM)).await;
        let door = chain(
            [&primary, &fallback, &backup],
            [
                Lane::Gemini.model(),
                Lane::SpaceBunny.model(),
                Lane::Glm.model(),
            ],
        );
        let seen = switches(&door).await;
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert_eq!(seen[0].provider, Provider::Other);
        assert_eq!(seen[0].model, Lane::Gemini.model());
        assert_eq!(seen[0].why, why);
        assert_eq!(seen[0].answered.as_deref(), Some(Lane::Glm.model()));
    }

    // A primary benched for its 402 is skipped next turn and still named.
    let primary = Server::start(Stub::Status(402)).await;
    let fallback = Server::start(Stub::Whole(GEMINI)).await;
    let door = ordered(&primary, &fallback);
    let _ = switches(&door).await;
    let seen = switches(&door).await;
    assert_eq!(primary.asked(), 1, "the benched primary sat the turn out");
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].why, Missed::Refused);
    assert_eq!(seen[0].model, Lane::SpaceBunny.model());
    assert_eq!(seen[0].answered.as_deref(), Some(Lane::Gemini.model()));

    let primary = Server::start(Stub::Whole(SPACE_BUNNY)).await;
    let fallback = Server::start(Stub::Whole(GEMINI)).await;
    assert!(switches(&ordered(&primary, &fallback)).await.is_empty());
}

/// Every door failing is the last door's failure, which the worker turns
/// into its one plain failure line; nothing reached the caller.
#[tokio::test]
async fn every_door_failing_is_the_last_doors_failure() {
    let primary = Server::start(Stub::Status(402)).await;
    let fallback = Server::start(Stub::Status(404)).await;
    let backup = Server::start(Stub::Status(503)).await;
    let door = chain(
        [&primary, &fallback, &backup],
        [
            Lane::Gemini.model(),
            Lane::Glm.model(),
            Lane::Gemini.model(),
        ],
    );
    let (seen, outcome, named) = ask_ordered(&door).await;
    let error = outcome.expect_err("nothing answered");
    assert!(matches!(error, GenerateError::Status(503, _)), "{error}");
    assert!(seen.is_empty());
    assert_eq!(named, None);

    // When every door is benched, every door is asked anyway.
    let all_benched = chain(
        [&primary, &fallback, &fallback],
        [
            Lane::Gemini.model(),
            Lane::Glm.model(),
            Lane::Gemini.model(),
        ],
    );
    let _ = ask_ordered(&all_benched).await;
    let before = primary.asked();
    let _ = ask_ordered(&all_benched).await;
    assert_eq!(primary.asked(), before + 1, "benched doors are still asked");
}

/// Each door before the last has the first-word wait and no more: two
/// silent doors (no headers; headers and then nothing) cost two waits, not
/// their own thirty-second bounds, before the third answers.
#[tokio::test]
async fn each_door_has_the_first_word_wait_and_no_more() {
    let deaf = Server::start(Stub::Deaf).await;
    let mute = Server::start(Stub::Mute).await;
    let backup = Server::start(Stub::Whole(GEMINI)).await;
    let door = chain(
        [&deaf, &mute, &backup],
        [
            Lane::Glm.model(),
            Lane::SpaceBunny.model(),
            Lane::Gemini.model(),
        ],
    );
    let started = Instant::now();
    let (_, outcome, named) = ask_ordered(&door).await;
    outcome.expect("the third door answers");
    let took = started.elapsed();
    assert_eq!(named.as_deref(), Some(Lane::Gemini.model()));
    assert!(
        took >= Duration::from_millis(400) && took < Duration::from_millis(2_000),
        "{took:?}"
    );
    // A silent door is not benched: the next turn asks it again.
    let _ = ask_ordered(&door).await;
    assert_eq!(deaf.asked(), 2);
}

/// Live, the 2026-10-09 outage forced on purpose: the retired Space Bunny
/// Alpha on OpenRouter (404), then the Vercel AI Gateway (402 while its
/// account is empty, or an answer once it is funded), then GLM on
/// OpenRouter. The turn is answered with no error, and the door that
/// answered names itself. Needs `OPENROUTER_API_KEY` and
/// `AI_GATEWAY_API_KEY`; prints which door answered and when.
#[tokio::test]
#[ignore = "live: asks OpenRouter and the Vercel AI Gateway"]
async fn live_a_dead_chain_head_fails_over_to_a_live_door() {
    let key = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
    let door = FallbackDoor::new(
        ResponsesDoor::new(
            coder::generate::OPENROUTER_DOOR_URL,
            Lane::SpaceBunny.openrouter_model(),
            key("OPENROUTER_API_KEY"),
        ),
        ResponsesDoor::new(
            coder::generate::DEFAULT_DOOR_URL,
            Lane::Gemini.model(),
            key("AI_GATEWAY_API_KEY"),
        ),
    )
    .with_backups(vec![ResponsesDoor::new(
        coder::generate::OPENROUTER_DOOR_URL,
        Lane::Glm.openrouter_model(),
        key("OPENROUTER_API_KEY"),
    )]);
    let started = Instant::now();
    let mut named = None;
    let answered = door
        .generate(
            "Answer in one sentence.",
            &[Message {
                role: Role::User,
                text: "What is the capital of France?".to_string(),
            }],
            &mut |_| {},
            &mut |meta| {
                if let Meta::Model(model) = meta {
                    named = Some(model);
                }
            },
        )
        .await;
    let (text, _) = answered.unwrap_or_else(|error| panic!("no door answered: {error}"));
    println!(
        "answered by {} in {:?}: {text:?}",
        named.as_deref().unwrap_or("?"),
        started.elapsed()
    );
    assert!(text.contains("Paris"), "{text}");
    assert_ne!(named.as_deref(), Some(Lane::SpaceBunny.model()));
}

/// Live: a primary OpenRouter does not serve (as Space Bunny Alpha will
/// be after 2026-10-05) hands the turn to Gemini 3.8 Flash on the gateway,
/// which names itself. Needs `OPENROUTER_API_KEY` and `AI_GATEWAY_API_KEY`;
/// prints the timings.
#[tokio::test]
#[ignore = "live: asks OpenRouter and the Vercel AI Gateway"]
async fn live_a_retired_primary_falls_back_to_gemini() {
    let key = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
    for primary in ["stealth/no-such-model", Lane::SpaceBunny.model()] {
        let door = FallbackDoor::openrouter(
            primary,
            &key("OPENROUTER_API_KEY"),
            ResponsesDoor::new(
                coder::generate::DEFAULT_DOOR_URL,
                Lane::Gemini.model(),
                key("AI_GATEWAY_API_KEY"),
            ),
        );
        let started = Instant::now();
        let mut first = None;
        let mut named = None;
        let answered = door
            .generate(
                "Answer in one sentence.",
                &[Message {
                    role: Role::User,
                    text: "What is the capital of France?".to_string(),
                }],
                &mut |_| {
                    first.get_or_insert(started.elapsed());
                },
                &mut |meta| {
                    if let Meta::Model(model) = meta {
                        named = Some(model);
                    }
                },
            )
            .await;
        let (text, _) = answered.unwrap_or_else(|error| panic!("{primary}: {error}"));
        println!(
            "primary {primary}: answered by {} with first words at {:?}, done at {:?}: {text:?}",
            named.as_deref().unwrap_or("?"),
            first.unwrap_or_default(),
            started.elapsed()
        );
        let expected = if primary == Lane::SpaceBunny.model() {
            Lane::SpaceBunny.model()
        } else {
            Lane::Gemini.model()
        };
        assert_eq!(named.as_deref(), Some(expected));
        assert!(text.contains("Paris"), "{text}");
    }
}

/// Vertex AI's recorded `streamGenerateContent` answer from
/// `gemini-3.8-flash` (`CODER_WORKER_VERTEX`, 2026-10-10).
const VERTEX: &str = include_str!("../fixtures/gateway/vertex-gemini-3.8-flash.sse");

/// A Vertex door onto `server`, with a fixed token.
fn vertex_door(server: &Server) -> ResponsesDoor {
    let token = inference::upstream::google::TokenSource::fixed(
        inference::upstream::secret::Secret::new("a-token").expect("a token"),
    );
    let mut door = ResponsesDoor::vertex("openagentsgemini", "global", token).waiting(short());
    door.url = server.url.clone();
    door
}

/// The Vertex door reads Vertex AI's native stream: the answer text in
/// order, the usage from the last chunk, and the lane's public model id.
#[tokio::test]
async fn the_vertex_door_reads_vertex_s_own_stream() {
    let server = Server::start(Stub::Whole(VERTEX)).await;
    let door = vertex_door(&server);
    assert!(door.is_vertex());
    assert_eq!(door.model, Lane::Gemini.model());
    let mut seen = String::new();
    let (text, usage) = door
        .generate(
            "you are terse",
            &turn(),
            &mut |delta| seen.push_str(delta),
            &mut |_| {},
        )
        .await
        .expect("an answer");
    assert_eq!(text, "One\nTwo\nThree\nFour\nFive");
    assert_eq!(seen, text);
    let usage = usage.expect("usage");
    assert_eq!((usage.input_tokens, usage.output_tokens), (15, 9));
}

/// A stream that carries thought summaries shows only the answer, and one
/// cut off at its token limit is a failure, not an answer.
#[tokio::test]
async fn the_vertex_door_hides_thoughts_and_refuses_a_cut_off_answer() {
    const THINKING: &str = concat!(
        "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Planning\",\"thought\":true}]}}]}\n\n",
        "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Paris.\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":4,\"candidatesTokenCount\":2,\"thoughtsTokenCount\":7}}\n\n",
    );
    let server = Server::start(Stub::Whole(THINKING)).await;
    let (text, usage) = vertex_door(&server)
        .generate("", &turn(), &mut |_| {}, &mut |_| {})
        .await
        .expect("an answer");
    assert_eq!(text, "Paris.");
    assert_eq!(usage.expect("usage").output_tokens, 9);

    const CUT: &str = "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"Par\"}]},\"finishReason\":\"MAX_TOKENS\"}]}\n\n";
    let server = Server::start(Stub::Whole(CUT)).await;
    let failed = vertex_door(&server)
        .generate("", &turn(), &mut |_| {}, &mut |_| {})
        .await
        .expect_err("a cut-off answer");
    assert!(failed.to_string().contains("max_tokens"), "{failed}");
}

/// The Vertex door in front of the OpenRouter door: an answer from Vertex
/// names Gemini and asks nobody else; a Vertex refusal (403) hands the turn
/// to the next door, which answers, and the switch names the first model
/// provider and why, so the chat's "another provider answered" line shows.
#[tokio::test]
async fn the_vertex_door_goes_first_and_hands_a_refused_turn_on() {
    use coder::generate::{Door, Missed, Provider};
    let vertex = Server::start(Stub::Whole(VERTEX)).await;
    let openrouter = Server::start(Stub::Whole(GEMINI)).await;
    let Door::Fallback(door) =
        Door::Live(openrouter.door(Lane::Gemini.model())).behind(vertex_door(&vertex))
    else {
        panic!("a chain of two");
    };
    let door = door.first_word(Duration::from_millis(300), Duration::from_millis(900));
    let (_, outcome, named) = ask_ordered(&door).await;
    outcome.expect("Vertex answers");
    assert_eq!(named.as_deref(), Some(Lane::Gemini.model()));
    assert!(door.answering().is_vertex());
    assert_eq!(openrouter.asked(), 0);

    let refused = Server::start(Stub::Status(403)).await;
    let openrouter = Server::start(Stub::Whole(GEMINI)).await;
    let Door::Fallback(door) =
        Door::Live(openrouter.door(Lane::Gemini.model())).behind(vertex_door(&refused))
    else {
        panic!("a chain of two");
    };
    let door = door.first_word(Duration::from_millis(300), Duration::from_millis(900));
    let switched = switches(&door).await;
    assert_eq!(switched.len(), 1, "{switched:?}");
    assert_eq!(switched[0].provider, Provider::Other);
    assert_eq!(switched[0].why, Missed::Refused);
    assert_eq!(switched[0].model, Lane::Gemini.model());
    assert_eq!(switched[0].answered.as_deref(), Some(Lane::Gemini.model()));
    assert!(!door.answering().is_vertex());
    assert_eq!((refused.asked(), openrouter.asked()), (1, 1));
}

/// One real turn on Vertex AI through the door the worker builds
/// (`vertex_door_from_env`): run with `VERTEX_PROJECT` and a Google
/// credential (`GOOGLE_APPLICATION_CREDENTIALS`, or `GCE_METADATA_HOST` on
/// GCE) and `--ignored`. Prints the time to the first words and the whole.
#[tokio::test]
#[ignore = "calls Vertex AI with a real Google credential"]
async fn live_vertex_door_answers() {
    let door = coder::generate::vertex_door_from_env()
        .expect("the switch reads")
        .expect("the Vertex door is on");
    door.warm().await;
    let started = Instant::now();
    let mut first = None;
    let (text, usage) = door
        .generate(
            "You are terse.",
            &[Message {
                role: Role::User,
                text: "What is the capital of France? One word.".to_string(),
            }],
            &mut |_| {
                first.get_or_insert_with(|| started.elapsed());
            },
            &mut |_| {},
        )
        .await
        .expect("Vertex answers");
    eprintln!(
        "vertex: first words {:?}, whole {:?}, {usage:?}: {text}",
        first,
        started.elapsed()
    );
    assert!(text.contains("Paris"), "{text}");
}
