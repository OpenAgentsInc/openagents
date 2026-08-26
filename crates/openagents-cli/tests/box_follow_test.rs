//! The Box run follow loop, against a stub that behaves the way a box does.
//!
//! Issue #78 asks for streaming output while a run is still going. The route
//! publishes no event stream, so following is a poll over `?offset=`, and the
//! two ways that goes wrong are silent: a reader that never advances its offset
//! prints the first window forever, and a reader that stops at the first
//! terminal state drops whatever the box wrote last. Both produce output that
//! looks like a finished run.
//!
//! So the stub writes in three windows, only turns terminal after the second,
//! and writes the third *after* going terminal. A reader that gets any of that
//! wrong produces the wrong string, and this fails.

use openagents_cli::box_client::BoxClient;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Stub {
    base: String,
}

/// The windows the box "writes", in order, keyed by the offset a reader must
/// ask for to see them. Reading at an offset the stub has not reached yet
/// yields an empty window, the way a live box's bounded log does.
const WINDOWS: [&str; 3] = ["first ", "second ", "third"];

async fn start_stub() -> Stub {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}/api/v1");
    // How many output reads have happened. The run turns terminal once the
    // reader has taken the first two windows, and the third is written only
    // after that — a reader that stops at the terminal state loses it.
    let reads = Arc::new(AtomicUsize::new(0));

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let request = match read_request(&mut socket).await {
                Some(request) => request,
                None => continue,
            };
            let first_line = request.lines().next().unwrap_or("").to_string();

            let body = if first_line.contains("/output") {
                let offset = offset_of(&first_line);
                let index = WINDOWS
                    .iter()
                    .scan(0usize, |acc, window| {
                        let start = *acc;
                        *acc += window.len();
                        Some(start)
                    })
                    .position(|start| start == offset);
                let taken = reads.load(Ordering::SeqCst);
                match index {
                    // A window the box has not produced yet reads as empty and
                    // does not advance the offset.
                    Some(i) if i <= taken => {
                        reads.store(taken.max(i + 1), Ordering::SeqCst);
                        let next = offset + WINDOWS[i].len();
                        format!(
                            r#"{{"run_id":"run_1","output":{{"output":"{}","next_offset":{},"truncated":false}}}}"#,
                            WINDOWS[i], next
                        )
                    }
                    _ => format!(
                        r#"{{"run_id":"run_1","output":{{"output":"","next_offset":{},"truncated":false}}}}"#,
                        offset
                    ),
                }
            } else {
                // The run reports `running` until both of the first two windows
                // have been read, then `completed`.
                //
                // `completed` is what `OpenAgents.Box.Run` actually sets, and
                // saying so here is the point of this line. This stub used to
                // answer `succeeded`, which no deployment has ever sent, and
                // the client agreed with the stub instead of the server: it
                // treated `succeeded` as terminal and `completed` as still
                // running. Both sides were self-consistently wrong, this test
                // passed, and against production `oa box runs output --follow`
                // ran past the end of every successful run until the API
                // refused a request. A stub that invents the server's
                // vocabulary tests the stub.
                let state = if reads.load(Ordering::SeqCst) >= 2 {
                    "completed"
                } else {
                    "running"
                };
                format!(
                    r#"{{"run":{{"id":"run_1","box_id":"bx_1","command":"echo","state":"{state}","exit_status":0}}}}"#
                )
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    Stub { base }
}

fn offset_of(request_line: &str) -> usize {
    request_line
        .split_once("offset=")
        .and_then(|(_, rest)| {
            rest.split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|digits| digits.parse().ok())
        })
        .unwrap_or(0)
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<String> {
    let mut buffer = vec![0u8; 8192];
    let read = socket.read(&mut buffer).await.ok()?;
    if read == 0 {
        return None;
    }
    Some(String::from_utf8_lossy(&buffer[..read]).to_string())
}

/// Following a run reads every window in order, including the one written after
/// the run turned terminal.
#[tokio::test]
async fn following_a_run_reads_past_the_first_window_and_past_the_terminal_state() {
    let stub = start_stub().await;
    let client = BoxClient::new(&stub.base, None);

    let seen = std::cell::RefCell::new(String::new());
    let (run, next_offset) = client
        .follow_run_output(
            "conv_1",
            "bx_1",
            "run_1",
            Some(0),
            std::time::Duration::from_millis(1),
            |chunk| seen.borrow_mut().push_str(&chunk.output),
        )
        .await
        .expect("the follow failed");

    assert_eq!(
        seen.into_inner(),
        "first second third",
        "the follow must read every window, in order, including the one the box \
         wrote after the run turned terminal"
    );
    assert_eq!(run.state, "completed");
    assert!(run.finished());
    assert_eq!(next_offset, 18);
}

/// A refused read ends the follow. It does not return the bytes read so far as
/// though they were the whole run.
#[tokio::test]
async fn a_refused_read_ends_the_follow_rather_than_truncating_it() {
    let client = BoxClient::new("http://127.0.0.1:1/api/v1", None);
    let result = client
        .follow_run_output(
            "conv_1",
            "bx_1",
            "run_1",
            Some(0),
            std::time::Duration::from_millis(1),
            |_| {},
        )
        .await;
    assert!(
        result.is_err(),
        "an unreachable box must not produce a finished run"
    );
}

/// The poll interval applies to every pass, not only to the ones that found
/// nothing new.
///
/// The loop used to skip its sleep whenever the offset had advanced, so a run
/// that printed steadily was followed by an unthrottled loop issuing two
/// requests per pass — an output read and a run read — as fast as the network
/// allowed. Against production that is what a `--follow` looked like right up
/// until the edge answered 502 mid-stream.
///
/// The stub advances the offset on every read and only turns terminal on the
/// second pass, so the fixed loop sleeps exactly once — on a pass that carried
/// output, which is the pass the old code skipped. Before the fix this whole
/// follow finished in single-digit milliseconds over loopback; after it, it
/// cannot finish in less than one interval. That gap is what is asserted, and
/// it is wide enough not to turn into a timing flake on a loaded machine.
#[tokio::test]
async fn following_sleeps_between_passes_even_while_output_is_arriving() {
    let stub = start_stub().await;
    let client = BoxClient::new(&stub.base, None);
    let interval = std::time::Duration::from_millis(250);

    let started = std::time::Instant::now();
    let (run, _next_offset) = client
        .follow_run_output("conv_1", "bx_1", "run_1", Some(0), interval, |_| {})
        .await
        .expect("the follow failed");
    let elapsed = started.elapsed();

    assert_eq!(run.state, "completed");
    assert!(
        elapsed >= interval,
        "the pass that carried output must still wait out the poll interval; \
         this follow took {elapsed:?}, under the {interval:?} it asked for, \
         which means the loop is spinning on the API while output arrives"
    );
}
