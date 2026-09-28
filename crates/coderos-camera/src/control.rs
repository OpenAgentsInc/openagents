//! The control socket: one request a connection, answered by the daemon.
//!
//! [`serve`] binds the socket and answers each connection on a thread of
//! its own with whatever the handler says. [`ask`] is the command's half:
//! connect, write one request, read one answer.

use crate::protocol::{Answer, GENERATION, Refusal, Reply, Request, Verb, refusal};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// What answers a verb.
pub type Handler = Arc<dyn Fn(Verb) -> Reply + Send + Sync>;

/// Binds `path`, removing a socket a dead daemon left, and answers every
/// connection with `handler` until the process ends.
pub fn serve(path: &Path, handler: Handler) -> Result<JoinHandle<()>, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path).map_err(|err| format!("{}: {err}", path.display()))?;
    thread::Builder::new()
        .name("camera-control".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let handler = Arc::clone(&handler);
                let _ = thread::Builder::new()
                    .name("camera-control-call".into())
                    .spawn(move || answer_one(stream, &handler));
            }
        })
        .map_err(|err| format!("control thread: {err}"))
}

/// The answer to one line off the socket.
pub fn answer_line(line: &str, handler: &Handler) -> Answer {
    let request: Request = match serde_json::from_str(line.trim()) {
        Ok(request) => request,
        Err(err) => {
            return Answer::new(Reply::Refused(Refusal::new(
                refusal::UNKNOWN_VERB,
                format!("the request did not parse: {err}"),
            )));
        }
    };
    if request.generation != GENERATION {
        return Answer::new(Reply::Refused(Refusal::new(
            refusal::UNSUPPORTED_GENERATION,
            format!(
                "this daemon speaks generation {GENERATION}, and the request spoke {}",
                request.generation
            ),
        )));
    }
    if request.verb == Verb::Unknown {
        return Answer::new(Reply::Refused(Refusal::new(
            refusal::UNKNOWN_VERB,
            "this daemon does not know that verb",
        )));
    }
    Answer::new(handler(request.verb))
}

fn answer_one(stream: UnixStream, handler: &Handler) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        return;
    }
    let answer = answer_line(&line, handler);
    let mut text = serde_json::to_string(&answer).unwrap_or_default();
    text.push('\n');
    let mut stream = &stream;
    let _ = stream.write_all(text.as_bytes());
}

/// Asks the daemon at `path` one verb. The error names the socket when no
/// daemon answers there.
pub fn ask(path: &Path, verb: Verb) -> Result<Reply, String> {
    let mut stream = UnixStream::connect(path).map_err(|err| {
        format!(
            "no camera daemon at {}: {err}; start one with `coderos-camera serve`",
            path.display()
        )
    })?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let mut text = serde_json::to_string(&Request::new(verb)).map_err(|err| err.to_string())?;
    text.push('\n');
    stream
        .write_all(text.as_bytes())
        .map_err(|err| format!("{}: {err}", path.display()))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|err| format!("{}: {err}", path.display()))?;
    if line.trim().is_empty() {
        return Err(format!(
            "the daemon at {} closed without an answer",
            path.display()
        ));
    }
    let answer: Answer = serde_json::from_str(line.trim())
        .map_err(|err| format!("the daemon's answer did not parse: {err}"))?;
    Ok(answer.reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Output;

    fn handler() -> Handler {
        Arc::new(|verb: Verb| match verb {
            Verb::Outputs => Reply::Outputs {
                outputs: vec![Output {
                    name: "loopback".into(),
                    target: Some("/dev/video10".into()),
                    state: "up".into(),
                    frames: 1,
                    dropped: 0,
                }],
            },
            Verb::HandsOn => Reply::Done,
            _ => Reply::Refused(Refusal::new(refusal::UNKNOWN_VERB, "not in this test")),
        })
    }

    #[test]
    fn a_request_at_another_generation_is_refused() {
        let answer = answer_line("{\"generation\":2,\"type\":\"status\"}", &handler());
        match answer.reply {
            Reply::Refused(refusal) => assert_eq!(refusal.code, refusal::UNSUPPORTED_GENERATION),
            other => panic!("{other:?}"),
        }
        assert_eq!(answer.generation, GENERATION);
    }

    #[test]
    fn an_unknown_verb_and_a_broken_line_are_refused_by_code() {
        let unknown = answer_line("{\"generation\":1,\"type\":\"dance\"}", &handler());
        assert!(matches!(unknown.reply, Reply::Refused(ref r) if r.code == refusal::UNKNOWN_VERB));
        let broken = answer_line("{", &handler());
        assert!(
            matches!(broken.reply, Reply::Refused(ref r) if r.message.contains("did not parse"))
        );
    }

    #[test]
    fn the_command_asks_over_the_socket_and_reads_the_reply() {
        let dir =
            std::env::temp_dir().join(format!("coderos-camera-control-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let socket = dir.join("control.sock");
        let _server = serve(&socket, handler()).expect("bind");
        match ask(&socket, Verb::Outputs).expect("answer") {
            Reply::Outputs { outputs } => {
                assert_eq!(outputs[0].target.as_deref(), Some("/dev/video10"))
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(ask(&socket, Verb::HandsOn).expect("answer"), Reply::Done);
        let err = ask(&dir.join("none.sock"), Verb::Status).unwrap_err();
        assert!(err.starts_with("no camera daemon at "), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
