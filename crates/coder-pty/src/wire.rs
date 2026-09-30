//! The NIP-TERM bodies a client and a host exchange.
//!
//! Every body is a JSON object with a version string `v`, an empty
//! `requires` list, and exactly the fields below; unknown fields refuse.
//! Terminal input and output bytes travel base64-encoded. None of these
//! bodies is public: a transport carries them over an authenticated direct
//! channel or inside private `3188` artifacts, never in a public event.
//!
//! Constructing a body grants nothing. The host checks the signed caller's
//! current rights for every operation.

use serde::{Deserialize, Serialize};

/// `v` of an open request.
pub const OPEN: &str = "openagents.terminal-open.v1";
/// `v` of an attach request.
pub const ATTACH: &str = "openagents.terminal-attach.v1";
/// `v` of a detach request.
pub const DETACH: &str = "openagents.terminal-detach.v1";
/// `v` of an input request.
pub const INPUT: &str = "openagents.terminal-input.v1";
/// `v` of a resize request.
pub const RESIZE: &str = "openagents.terminal-resize.v1";
/// `v` of a signal request.
pub const SIGNAL: &str = "openagents.terminal-signal.v1";
/// `v` of a close request.
pub const CLOSE: &str = "openagents.terminal-close.v1";
/// `v` of a host's answer to any request.
pub const RESULT: &str = "openagents.terminal-result.v1";
/// `v` of a frame the host delivers to one attachment.
pub const FRAME: &str = "openagents.terminal-frame.v1";

/// The most input bytes one input request carries.
pub const INPUT_MAX: usize = 4096;
/// The most output bytes one output frame carries, before base64.
///
/// A frame at this size is about 11 KiB of JSON. That fits one NIP-REACH
/// data frame (16,384 application bytes) and the 65,535-byte NIP-44
/// plaintext limit a relay-carried `3188` artifact must fit.
pub const FRAME_MAX: usize = 8 * 1024;
/// The largest terminal dimension, in rows or columns.
pub const DIMENSION_MAX: u16 = 1024;
/// The most environment variables one open request names.
pub const ENV_MAX: usize = 64;
/// The most arguments one exact command carries.
pub const ARGS_MAX: usize = 256;
/// The longest string field, in bytes, apart from arguments and values.
pub const TEXT_MAX: usize = 1024;
/// The longest argument or environment value, in bytes.
pub const VALUE_MAX: usize = 4096;

/// A terminal size in character cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}

impl Size {
    /// A size, unchecked; each request that carries one validates it.
    #[must_use]
    pub fn new(rows: u16, cols: u16) -> Self {
        Size { rows, cols }
    }

    fn check(self) -> Result<(), Refusal> {
        let fits = |n: u16| (1..=DIMENSION_MAX).contains(&n);
        if fits(self.rows) && fits(self.cols) {
            Ok(())
        } else {
            Err(Refusal::malformed(
                "a terminal size must be 1 to 1024 rows and columns",
            ))
        }
    }
}

/// One terminal on one host generation.
///
/// A host generation changes when the host restarts. A reference to an
/// earlier generation names a terminal the host no longer has, which the
/// host reports as `lost` rather than resuming something else.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalRef {
    /// The host generation, a common ID.
    pub generation: String,
    /// The terminal, a common ID the host minted.
    pub terminal: String,
}

impl TerminalRef {
    fn check(&self) -> Result<(), Refusal> {
        common_id(&self.generation, "generation")?;
        common_id(&self.terminal, "terminal")
    }
}

/// What an open request runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Launch {
    /// The host's configured shell. The client does not choose which.
    Shell,
    /// An exact program and arguments. The program is an absolute path —
    /// `/…` on a Unix host, or a drive path such as `C:\…` on Windows;
    /// the host resolves no name through a search path.
    Command { program: String, args: Vec<String> },
}

/// One environment variable an open request asks for. The host admits only
/// names on its allowlist and refuses the request otherwise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

/// Open a terminal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Open {
    pub v: String,
    pub requires: Vec<String>,
    /// A common ID the client chose for this request.
    pub request: String,
    /// The admitted workspace, a common ID the host maps to a root.
    pub workspace: String,
    /// The working directory, relative to the workspace root. Empty is the
    /// root itself.
    pub dir: String,
    pub launch: Launch,
    pub size: Size,
    pub env: Vec<EnvVar>,
}

impl Open {
    /// An open request with no environment variables.
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        workspace: impl Into<String>,
        dir: impl Into<String>,
        launch: Launch,
        size: Size,
    ) -> Self {
        Open {
            v: OPEN.into(),
            requires: Vec::new(),
            request: request.into(),
            workspace: workspace.into(),
            dir: dir.into(),
            launch,
            size,
            env: Vec::new(),
        }
    }

    /// Validates the body's shape. The host still checks the workspace,
    /// directory, program, and environment against its own policy.
    pub fn check(&self) -> Result<(), Refusal> {
        header(&self.v, OPEN, &self.requires, &self.request)?;
        common_id(&self.workspace, "workspace")?;
        relative_dir(&self.dir)?;
        self.size.check()?;
        match &self.launch {
            Launch::Shell => {}
            Launch::Command { program, args } => {
                bounded(program, TEXT_MAX, "program")?;
                if !absolute_program(program) {
                    return Err(Refusal::malformed(
                        "a command's program must be an absolute path",
                    ));
                }
                if args.len() > ARGS_MAX {
                    return Err(Refusal::limit("a command carries at most 256 arguments"));
                }
                for arg in args {
                    bounded(arg, VALUE_MAX, "argument")?;
                }
            }
        }
        if self.env.len() > ENV_MAX {
            return Err(Refusal::limit("an open request names at most 64 variables"));
        }
        for var in &self.env {
            env_name(&var.name)?;
            bounded(&var.value, VALUE_MAX, "environment value")?;
        }
        Ok(())
    }
}

/// Whether an attachment may only read, or also type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Read output. Requires `terminal`, or `observe` when the host's
    /// policy lets observers read terminals.
    Observe,
    /// Read output as an interactive client. Requires `terminal`. Each
    /// input, resize, and signal is still checked on its own.
    Interact,
}

/// Attach to a terminal and receive its frames after `after`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attach {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    pub mode: Mode,
    /// The last sequence number the client already has; zero asks for
    /// everything the host still retains.
    pub after: u64,
    /// The most output bytes per second the client wants delivered. The
    /// host narrows it to its own ceiling; it never widens it.
    pub rate: u64,
}

impl Attach {
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        terminal: TerminalRef,
        mode: Mode,
        after: u64,
        rate: u64,
    ) -> Self {
        Attach {
            v: ATTACH.into(),
            requires: Vec::new(),
            request: request.into(),
            terminal,
            mode,
            after,
            rate,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        header(&self.v, ATTACH, &self.requires, &self.request)?;
        self.terminal.check()?;
        if self.rate == 0 {
            return Err(Refusal::malformed("an attachment's rate must be positive"));
        }
        Ok(())
    }
}

/// End one attachment. The terminal keeps running.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detach {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    /// The attachment ID the host returned from attach.
    pub attachment: String,
}

impl Detach {
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        terminal: TerminalRef,
        attachment: impl Into<String>,
    ) -> Self {
        Detach {
            v: DETACH.into(),
            requires: Vec::new(),
            request: request.into(),
            terminal,
            attachment: attachment.into(),
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        header(&self.v, DETACH, &self.requires, &self.request)?;
        self.terminal.check()?;
        common_id(&self.attachment, "attachment")
    }
}

/// Bytes typed into a terminal.
///
/// Input is live only. A client never queues it in an offline outbox, and
/// the host writes an exact retry of the same request ID once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    #[serde(with = "b64")]
    pub data: Vec<u8>,
}

impl Input {
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        terminal: TerminalRef,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        Input {
            v: INPUT.into(),
            requires: Vec::new(),
            request: request.into(),
            terminal,
            data: data.into(),
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        header(&self.v, INPUT, &self.requires, &self.request)?;
        self.terminal.check()?;
        if self.data.is_empty() {
            return Err(Refusal::malformed(
                "an input request carries at least one byte",
            ));
        }
        if self.data.len() > INPUT_MAX {
            return Err(Refusal::limit(
                "an input request carries at most 4096 bytes",
            ));
        }
        Ok(())
    }
}

/// Change a terminal's size.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resize {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    pub size: Size,
}

impl Resize {
    #[must_use]
    pub fn new(request: impl Into<String>, terminal: TerminalRef, size: Size) -> Self {
        Resize {
            v: RESIZE.into(),
            requires: Vec::new(),
            request: request.into(),
            terminal,
            size,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        header(&self.v, RESIZE, &self.requires, &self.request)?;
        self.terminal.check()?;
        self.size.check()
    }
}

/// The signals a client may send to a terminal's foreground process group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    Interrupt,
    Quit,
    Terminate,
    Hangup,
    Kill,
}

/// Send a signal to a terminal's foreground process group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signal {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    pub signal: SignalKind,
}

impl Signal {
    #[must_use]
    pub fn new(request: impl Into<String>, terminal: TerminalRef, signal: SignalKind) -> Self {
        Signal {
            v: SIGNAL.into(),
            requires: Vec::new(),
            request: request.into(),
            terminal,
            signal,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        header(&self.v, SIGNAL, &self.requires, &self.request)?;
        self.terminal.check()
    }
}

/// End a terminal and its process group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Close {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
}

impl Close {
    #[must_use]
    pub fn new(request: impl Into<String>, terminal: TerminalRef) -> Self {
        Close {
            v: CLOSE.into(),
            requires: Vec::new(),
            request: request.into(),
            terminal,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        header(&self.v, CLOSE, &self.requires, &self.request)?;
        self.terminal.check()
    }
}

/// Why the host refused a request. The first eight are the shared contract
/// codes; `lost` and `closed` are NIP-TERM's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Malformed,
    UnsupportedVersion,
    UnsupportedFeature,
    /// The caller lacks the right this operation requires.
    NotAdmitted,
    /// The host cannot do this now: no PTY on this platform, an unknown
    /// terminal, or a spawn that failed.
    Unavailable,
    LimitExceeded,
    /// The request ID was used before with different bytes.
    IdempotencyConflict,
    /// The right was revoked while the operation was in progress.
    Revoked,
    /// The terminal belonged to an earlier host generation. The host
    /// restarted and the terminal did not survive it.
    Lost,
    /// The terminal ended and the host no longer retains it.
    Closed,
}

impl Reason {
    /// The code as it appears on the wire.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Reason::Malformed => "malformed",
            Reason::UnsupportedVersion => "unsupported_version",
            Reason::UnsupportedFeature => "unsupported_feature",
            Reason::NotAdmitted => "not_admitted",
            Reason::Unavailable => "unavailable",
            Reason::LimitExceeded => "limit_exceeded",
            Reason::IdempotencyConflict => "idempotency_conflict",
            Reason::Revoked => "revoked",
            Reason::Lost => "lost",
            Reason::Closed => "closed",
        }
    }
}

/// A refused request: a typed reason and a bounded explanation.
///
/// The detail is data. A client never executes it or renders it as
/// terminal control sequences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub reason: Reason,
    pub detail: String,
}

impl Refusal {
    #[must_use]
    pub fn new(reason: Reason, detail: impl Into<String>) -> Self {
        Refusal {
            reason,
            detail: detail.into(),
        }
    }

    pub(crate) fn malformed(detail: impl Into<String>) -> Self {
        Refusal::new(Reason::Malformed, detail)
    }

    pub(crate) fn limit(detail: impl Into<String>) -> Self {
        Refusal::new(Reason::LimitExceeded, detail)
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.reason.code(), self.detail)
    }
}

impl std::error::Error for Refusal {}

/// Whether the host did what the request asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Accepted,
    /// An exact retry of a request the host already applied. Nothing was
    /// applied twice.
    Duplicate,
    Refused,
}

/// What an accepted request produced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Value {
    Opened {
        terminal: TerminalRef,
        size: Size,
    },
    Attached {
        attachment: String,
        /// The newest sequence number when the attachment began.
        head: u64,
        size: Size,
        /// Whether the terminal's process was still running.
        running: bool,
    },
    Written {
        bytes: u64,
    },
    Done,
}

/// The host's answer to one request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalResult {
    pub v: String,
    pub requires: Vec<String>,
    /// The request ID this answers.
    pub request: String,
    pub status: Status,
    pub reason: Option<Reason>,
    pub value: Option<Value>,
}

impl TerminalResult {
    /// The wire result for an operation's outcome.
    #[must_use]
    pub fn from_outcome(
        request: impl Into<String>,
        outcome: Result<(Status, Value), Refusal>,
    ) -> Self {
        let (status, reason, value) = match outcome {
            Ok((status, value)) => (status, None, Some(value)),
            Err(refusal) => (Status::Refused, Some(refusal.reason), None),
        };
        TerminalResult {
            v: RESULT.into(),
            requires: Vec::new(),
            request: request.into(),
            status,
            reason,
            value,
        }
    }
}

/// Why a terminal's process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    /// The process exited on its own.
    Exited,
    /// A client closed the terminal.
    Closed,
    /// No client attached or typed for the host's idle period.
    IdleExpired,
    /// The host shut down.
    HostShutdown,
}

/// How a terminal's process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exit {
    pub cause: Cause,
    /// The exit code, when the process exited normally.
    pub code: Option<i32>,
    /// The signal number, when a signal ended the process.
    pub signal: Option<i32>,
}

/// Why an attachment ended while its terminal kept running.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Detached {
    /// The client asked.
    Requested,
    /// The caller no longer holds the right the attachment needs.
    Revoked,
    /// The attachment's transport refused a frame and was dropped.
    Transport,
}

/// What one frame carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Body {
    /// Terminal output. Sequence numbers start at one and increase by one
    /// per sequenced frame of the terminal.
    Output {
        seq: u64,
        #[serde(with = "b64")]
        data: Vec<u8>,
    },
    /// The process ended. This is the terminal's last sequenced frame.
    Exit { seq: u64, exit: Exit },
    /// Frames `from` through `to` were discarded before this attachment
    /// read them; `bytes` output bytes are missing, or null when the host
    /// no longer knows how many. Not sequenced.
    Gap {
        from: u64,
        to: u64,
        bytes: Option<u64>,
    },
    /// The attachment ended. Not sequenced.
    Detached { reason: Detached },
}

impl Body {
    /// The body's sequence number, for sequenced frames.
    #[must_use]
    pub fn seq(&self) -> Option<u64> {
        match self {
            Body::Output { seq, .. } | Body::Exit { seq, .. } => Some(*seq),
            Body::Gap { .. } | Body::Detached { .. } => None,
        }
    }
}

/// One frame delivered to one attachment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub v: String,
    pub terminal: TerminalRef,
    pub attachment: String,
    pub body: Body,
}

impl Frame {
    #[must_use]
    pub fn new(terminal: TerminalRef, attachment: impl Into<String>, body: Body) -> Self {
        Frame {
            v: FRAME.into(),
            terminal,
            attachment: attachment.into(),
            body,
        }
    }

    /// Validates the frame's shape, which a client does before applying it.
    pub fn check(&self) -> Result<(), Refusal> {
        version(&self.v, FRAME)?;
        self.terminal.check()?;
        common_id(&self.attachment, "attachment")?;
        match &self.body {
            Body::Output { seq, data } => {
                if *seq == 0 {
                    return Err(Refusal::malformed("sequence numbers start at one"));
                }
                if data.len() > FRAME_MAX {
                    return Err(Refusal::limit("an output frame carries at most 8192 bytes"));
                }
            }
            Body::Exit { seq, .. } => {
                if *seq == 0 {
                    return Err(Refusal::malformed("sequence numbers start at one"));
                }
            }
            Body::Gap { from, to, .. } => {
                if *from == 0 || to < from {
                    return Err(Refusal::malformed(
                        "a gap names a nonempty range of sequence numbers",
                    ));
                }
            }
            Body::Detached { .. } => {}
        }
        Ok(())
    }
}

/// Whether `id` is a common ID: 64 lowercase hexadecimal characters.
#[must_use]
pub fn is_common_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn common_id(id: &str, what: &str) -> Result<(), Refusal> {
    if is_common_id(id) {
        Ok(())
    } else {
        Err(Refusal::malformed(format!(
            "{what} must be 64 lowercase hexadecimal characters"
        )))
    }
}

fn version(v: &str, expected: &str) -> Result<(), Refusal> {
    if v == expected {
        Ok(())
    } else if v.starts_with("openagents.terminal-") {
        Err(Refusal::new(
            Reason::UnsupportedVersion,
            format!("expected {expected}"),
        ))
    } else {
        Err(Refusal::malformed(format!("expected {expected}")))
    }
}

fn header(v: &str, expected: &str, requires: &[String], request: &str) -> Result<(), Refusal> {
    version(v, expected)?;
    if !requires.is_empty() {
        return Err(Refusal::new(
            Reason::UnsupportedFeature,
            "this host supports no NIP-TERM features",
        ));
    }
    common_id(request, "request")
}

fn bounded(text: &str, max: usize, what: &str) -> Result<(), Refusal> {
    if text.len() > max {
        return Err(Refusal::limit(format!("{what} is longer than {max} bytes")));
    }
    if text.contains('\0') {
        return Err(Refusal::malformed(format!("{what} contains a NUL byte")));
    }
    Ok(())
}

/// Whether `program` names a program by absolute path: `/` first, as on a
/// Unix host, or a drive letter, a colon, and a separator, as on Windows.
/// A UNC path (`\\server\share`) is not admitted: a terminal never starts
/// a program from the network.
fn absolute_program(program: &str) -> bool {
    let bytes = program.as_bytes();
    program.starts_with('/')
        || (bytes.len() > 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/'))
}

fn relative_dir(dir: &str) -> Result<(), Refusal> {
    bounded(dir, TEXT_MAX, "dir")?;
    if dir.starts_with('/') {
        return Err(Refusal::malformed("dir is relative to the workspace root"));
    }
    if dir.split('/').any(|part| part == "..") {
        return Err(Refusal::malformed("dir must not leave the workspace root"));
    }
    Ok(())
}

fn env_name(name: &str) -> Result<(), Refusal> {
    let mut bytes = name.bytes();
    let first = bytes.next();
    let valid = name.len() <= 128
        && first.is_some_and(|b| b.is_ascii_uppercase() || b == b'_')
        && bytes.all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
    if valid {
        Ok(())
    } else {
        Err(Refusal::malformed(
            "an environment name is uppercase letters, digits, and underscores",
        ))
    }
}

/// Base64 (standard alphabet, padded) for byte fields.
mod b64 {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(deserializer)?;
        STANDARD
            .decode(text.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_is_a_unix_or_a_windows_drive_path() {
        for program in ["/bin/sh", r"C:\Windows\System32\cmd.exe", "d:/tools/x.exe"] {
            assert!(absolute_program(program), "{program}");
        }
        for program in [
            "sh",
            r"cmd.exe",
            r"C:cmd.exe",
            r"\\server\share\x.exe",
            "C:",
            r"\x",
        ] {
            assert!(!absolute_program(program), "{program}");
        }
    }

    const ID: &str = "0101010101010101010101010101010101010101010101010101010101010101";

    fn terminal() -> TerminalRef {
        TerminalRef {
            generation: ID.into(),
            terminal: ID.into(),
        }
    }

    #[test]
    fn a_valid_open_request_checks() {
        let open = Open::new(ID, ID, "src", Launch::Shell, Size::new(24, 80));
        assert_eq!(open.check(), Ok(()));
    }

    #[test]
    fn a_directory_that_leaves_the_root_is_refused() {
        for dir in ["/etc", "../x", "a/../../b"] {
            let open = Open::new(ID, ID, dir, Launch::Shell, Size::new(24, 80));
            assert_eq!(open.check().unwrap_err().reason, Reason::Malformed, "{dir}");
        }
    }

    #[test]
    fn a_relative_program_is_refused() {
        let launch = Launch::Command {
            program: "sh".into(),
            args: vec![],
        };
        let open = Open::new(ID, ID, "", launch, Size::new(24, 80));
        assert_eq!(open.check().unwrap_err().reason, Reason::Malformed);
    }

    #[test]
    fn an_unknown_feature_and_version_refuse_distinctly() {
        let mut open = Open::new(ID, ID, "", Launch::Shell, Size::new(24, 80));
        open.requires.push("openagents.terminal-x".into());
        assert_eq!(open.check().unwrap_err().reason, Reason::UnsupportedFeature);
        open.requires.clear();
        open.v = "openagents.terminal-open.v2".into();
        assert_eq!(open.check().unwrap_err().reason, Reason::UnsupportedVersion);
    }

    #[test]
    fn oversized_input_is_refused() {
        let input = Input::new(ID, terminal(), vec![b'x'; INPUT_MAX + 1]);
        assert_eq!(input.check().unwrap_err().reason, Reason::LimitExceeded);
    }

    #[test]
    fn frames_round_trip_with_base64_data() {
        let frame = Frame::new(
            terminal(),
            ID,
            Body::Output {
                seq: 1,
                data: b"hi\r\n".to_vec(),
            },
        );
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("\"data\":\"aGkNCg==\""), "{json}");
        let back: Frame = serde_json::from_str(&json).unwrap();
        assert_eq!(back, frame);
    }

    #[test]
    fn the_largest_frame_fits_one_direct_channel_data_frame() {
        let frame = Frame::new(
            terminal(),
            ID,
            Body::Output {
                seq: u64::MAX >> 11,
                data: vec![0xff; FRAME_MAX],
            },
        );
        let bytes = serde_json::to_vec(&frame).unwrap();
        assert!(bytes.len() <= 16_384, "{}", bytes.len());
        let input = Input::new(ID, terminal(), vec![0xff; INPUT_MAX]);
        assert!(serde_json::to_vec(&input).unwrap().len() <= 16_384);
    }

    #[test]
    fn unknown_fields_are_refused() {
        let json = format!(
            r#"{{"v":"{CLOSE}","requires":[],"request":"{ID}","terminal":{{"generation":"{ID}","terminal":"{ID}"}},"extra":1}}"#
        );
        assert!(serde_json::from_str::<Close>(&json).is_err());
    }
}
