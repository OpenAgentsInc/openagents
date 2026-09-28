//! The control socket's contract.
//!
//! One JSON object a line in each direction, the shape the desk protocol
//! uses (`docs/desk.md`): a caller connects, writes one [`Request`], reads
//! the daemon's [`Answer`], and closes. Every request carries a
//! `generation`, the answer echoes the generation the daemon speaks, and a
//! verb or a reply a side does not know decodes as `Unknown`. The
//! `coderos-camera` command turns its words into a verb with
//! [`parse_words`] and prints a reply with [`render`].

use serde::{Deserialize, Serialize};

/// The generation of this contract a build speaks.
pub const GENERATION: u32 = 1;

/// What a caller asks the daemon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Verb {
    /// The camera, its counters, and every output.
    Status,
    /// Every output and its state.
    Outputs,
    /// Start writing the camera to a file. With no `path`, the daemon
    /// picks one under `~/Videos/`.
    RecordStart { path: Option<String> },
    /// Stop the recording and answer its receipt.
    RecordStop,
    /// Start publishing hand landmarks on the hands socket.
    HandsOn,
    /// Stop publishing landmarks. The model stays loaded.
    HandsOff,
    /// Shut the daemon down: finish the recording, close the outputs,
    /// and remove the sockets. What `SIGTERM` does, asked for.
    Stop,
    /// A verb this side does not know.
    #[serde(other)]
    Unknown,
}

/// One request on the socket.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub generation: u32,
    #[serde(flatten)]
    pub verb: Verb,
}

impl Request {
    pub fn new(verb: Verb) -> Request {
        Request {
            generation: GENERATION,
            verb,
        }
    }
}

/// One output the daemon fans frames out to, and where it stands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Output {
    /// `loopback`, `record`, or `hands`.
    pub name: String,
    /// The node, the file, or the socket the output writes, when it has one.
    pub target: Option<String>,
    /// `up`, `off`, `recording`, `on`, `starting`, or `absent: <reason>`.
    pub state: String,
    /// Frames the output took since the daemon started.
    pub frames: u64,
    /// Frames the output was too slow to take.
    pub dropped: u64,
}

/// The `status` answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusReport {
    /// The camera node the daemon reads.
    pub device: String,
    /// The four-character code the camera streams, `MJPG` or `YUYV`, or
    /// what stands in the way of reading it.
    pub format: String,
    pub width: u32,
    pub height: u32,
    /// Frames read since the daemon started.
    pub frames: u64,
    /// Frames a second over the last second.
    pub fps: f64,
    /// The tracker's status line.
    pub hands: String,
    /// The file a recording writes, while one runs.
    pub recording: Option<String>,
    pub outputs: Vec<Output>,
}

/// The receipt a recording leaves: the same facts `screen-record` writes
/// into a take's comment tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub path: String,
    pub seconds: f64,
    pub frames: u64,
    pub fps: u32,
}

impl Receipt {
    /// The comment tag the file carries.
    pub fn note(&self) -> String {
        format!(
            "coderos: camera video {:.1}s, {} frames at {} fps constant",
            self.seconds, self.frames, self.fps
        )
    }
}

/// Why the daemon refused a request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub code: String,
    pub message: String,
}

impl Refusal {
    pub fn new(code: &str, message: impl Into<String>) -> Refusal {
        Refusal {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// The refusal codes.
pub mod refusal {
    pub const UNSUPPORTED_GENERATION: &str = "unsupported_generation";
    pub const UNKNOWN_VERB: &str = "unknown_verb";
    pub const NO_RECORDING: &str = "no_recording";
    pub const ALREADY_RECORDING: &str = "already_recording";
    pub const CANNOT_RECORD: &str = "cannot_record";
}

/// What the daemon answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Reply {
    Status(StatusReport),
    Outputs {
        outputs: Vec<Output>,
    },
    Done,
    Recorded(Receipt),
    Refused(Refusal),
    #[serde(other)]
    Unknown,
}

/// The daemon's answer to one request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    pub generation: u32,
    #[serde(flatten)]
    pub reply: Reply,
}

impl Answer {
    pub fn new(reply: Reply) -> Answer {
        Answer {
            generation: GENERATION,
            reply,
        }
    }
}

/// The words the command takes.
pub const USAGE: &str = "usage: coderos-camera [--json] serve | status | outputs | record start [<path>] | record stop | hands on | hands off | stop";

/// The verb the command's words ask for.
pub fn parse_words(words: &[String]) -> Result<Verb, String> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["status"] => Ok(Verb::Status),
        ["outputs"] => Ok(Verb::Outputs),
        ["record", "start"] => Ok(Verb::RecordStart { path: None }),
        ["record", "start", path] => Ok(Verb::RecordStart {
            path: Some((*path).to_string()),
        }),
        ["record", "stop"] => Ok(Verb::RecordStop),
        ["hands", "on"] => Ok(Verb::HandsOn),
        ["hands", "off"] => Ok(Verb::HandsOff),
        ["stop"] => Ok(Verb::Stop),
        [] => Err(USAGE.to_string()),
        other => Err(format!("{}: not a verb\n{USAGE}", other.join(" "))),
    }
}

/// One output as `status` prints it.
fn output_line(output: &Output) -> String {
    let counts = format!("{} frames, {} dropped", output.frames, output.dropped);
    match &output.target {
        Some(target) => format!("{} {target}: {} ({counts})", output.name, output.state),
        None => format!("{}: {} ({counts})", output.name, output.state),
    }
}

/// A reply as the command prints it, one line a fact.
pub fn render(reply: &Reply) -> String {
    match reply {
        Reply::Status(report) => {
            let mut lines = vec![format!(
                "camera {} {} {}x{}, {} frames, {:.1} a second",
                report.device,
                report.format,
                report.width,
                report.height,
                report.frames,
                report.fps
            )];
            lines.extend(report.outputs.iter().map(output_line));
            lines.push(format!("hands: {}", report.hands));
            if let Some(path) = &report.recording {
                lines.push(format!("recording: {path}"));
            }
            lines.join("\n")
        }
        Reply::Outputs { outputs } => outputs
            .iter()
            .map(output_line)
            .collect::<Vec<_>>()
            .join("\n"),
        Reply::Done => "done".to_string(),
        Reply::Recorded(receipt) => format!(
            "Recording saved: {} ({:.1}s, {} frames at {} fps)",
            receipt.path, receipt.seconds, receipt.frames, receipt.fps
        ),
        Reply::Refused(refusal) => format!("refused ({}): {}", refusal.code, refusal.message),
        Reply::Unknown => "the daemon answered a reply this build does not know".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn every_verb_parses_from_its_words() {
        assert_eq!(parse_words(&words("status")), Ok(Verb::Status));
        assert_eq!(parse_words(&words("outputs")), Ok(Verb::Outputs));
        assert_eq!(
            parse_words(&words("record start")),
            Ok(Verb::RecordStart { path: None })
        );
        assert_eq!(
            parse_words(&words("record start /tmp/a.mp4")),
            Ok(Verb::RecordStart {
                path: Some("/tmp/a.mp4".into())
            })
        );
        assert_eq!(parse_words(&words("record stop")), Ok(Verb::RecordStop));
        assert_eq!(parse_words(&words("hands on")), Ok(Verb::HandsOn));
        assert_eq!(parse_words(&words("hands off")), Ok(Verb::HandsOff));
        assert_eq!(parse_words(&words("stop")), Ok(Verb::Stop));
    }

    #[test]
    fn a_word_that_is_not_a_verb_is_named_with_the_usage() {
        let err = parse_words(&words("hands sideways")).unwrap_err();
        assert!(err.starts_with("hands sideways: not a verb"), "{err}");
        assert!(err.contains("usage:"), "{err}");
        assert_eq!(parse_words(&[]).unwrap_err(), USAGE);
    }

    #[test]
    fn a_request_is_one_flat_object_with_its_generation() {
        let text = serde_json::to_string(&Request::new(Verb::RecordStart {
            path: Some("/tmp/a.mp4".into()),
        }))
        .expect("json");
        assert_eq!(
            text,
            "{\"generation\":1,\"type\":\"record_start\",\"path\":\"/tmp/a.mp4\"}"
        );
        let back: Request = serde_json::from_str(&text).expect("parses");
        assert_eq!(
            back.verb,
            Verb::RecordStart {
                path: Some("/tmp/a.mp4".into())
            }
        );
    }

    #[test]
    fn a_verb_this_side_does_not_know_decodes_as_unknown() {
        let back: Request =
            serde_json::from_str("{\"generation\":1,\"type\":\"stream_start\"}").expect("parses");
        assert_eq!(back.verb, Verb::Unknown);
        let answer: Answer =
            serde_json::from_str("{\"generation\":9,\"type\":\"streaming\"}").expect("parses");
        assert_eq!(answer.reply, Reply::Unknown);
        assert_eq!(answer.generation, 9);
    }

    #[test]
    fn a_receipt_notes_the_facts_the_screen_recorder_notes() {
        let receipt = Receipt {
            path: "/tmp/a.mp4".into(),
            seconds: 12.34,
            frames: 370,
            fps: 30,
        };
        assert_eq!(
            receipt.note(),
            "coderos: camera video 12.3s, 370 frames at 30 fps constant"
        );
        assert_eq!(
            render(&Reply::Recorded(receipt)),
            "Recording saved: /tmp/a.mp4 (12.3s, 370 frames at 30 fps)"
        );
    }

    #[test]
    fn status_renders_one_line_a_fact() {
        let report = StatusReport {
            device: "/dev/video0".into(),
            format: "MJPG".into(),
            width: 1280,
            height: 720,
            frames: 90,
            fps: 29.97,
            hands: "off".into(),
            recording: Some("/tmp/a.mp4".into()),
            outputs: vec![
                Output {
                    name: "loopback".into(),
                    target: Some("/dev/video10".into()),
                    state: "up".into(),
                    frames: 90,
                    dropped: 0,
                },
                Output {
                    name: "record".into(),
                    target: Some("/tmp/a.mp4".into()),
                    state: "recording".into(),
                    frames: 88,
                    dropped: 2,
                },
            ],
        };
        let text = render(&Reply::Status(report));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "camera /dev/video0 MJPG 1280x720, 90 frames, 30.0 a second"
        );
        assert_eq!(lines[1], "loopback /dev/video10: up (90 frames, 0 dropped)");
        assert_eq!(
            lines[2],
            "record /tmp/a.mp4: recording (88 frames, 2 dropped)"
        );
        assert_eq!(lines[3], "hands: off");
        assert_eq!(lines[4], "recording: /tmp/a.mp4");
        assert_eq!(
            render(&Reply::Refused(Refusal::new(
                refusal::NO_RECORDING,
                "none runs"
            ))),
            "refused (no_recording): none runs"
        );
        assert_eq!(render(&Reply::Done), "done");
    }
}
