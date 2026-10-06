//! The NIP-TERM extensions: attach by snapshot, history reads, record
//! streams, paged block-journal reads, and session records
//! (`nips/openagents/NIP-TERM.md`, "Extensions").
//!
//! This module is the wire contract and its validation, for hosts and
//! clients alike. It runs no emulator: a host builds the records from its
//! own `coder-vt` state, and a client restores its own from them. A host
//! that serves none of these features keeps refusing them, because the
//! base-profile checks ([`crate::wire::Attach::check`]) admit no feature.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::client::{Applied, TerminalState};
use crate::wire::{self, Exit, Frame, Reason, Refusal, Size, TerminalRef, b64, common_id, version};

/// The snapshot feature: attach by snapshot, history reads, and record
/// streams.
pub const SNAPSHOT: &str = "openagents.terminal-snapshot.v1";
/// The block-journal feature.
pub const BLOCKS: &str = "openagents.terminal-blocks.v1";
/// The session-record feature.
pub const SESSIONS: &str = "openagents.terminal-sessions.v1";
/// The terminal-effects feature: the host's emulator answers the program's
/// queries, and the attachment receives the effects its output caused as
/// effect frames instead of acting on the output itself.
pub const EFFECTS: &str = "openagents.terminal-effects.v1";
/// The typist feature: input, resize, and signal name their attachment,
/// and take and release move the one typist role.
pub const TYPIST: &str = "openagents.terminal-typist.v1";

/// The presence capability a host advertises for [`SNAPSHOT`].
pub const CAPABILITY_SNAPSHOT: &str = "term-snapshot";
/// The presence capability a host advertises for [`BLOCKS`].
pub const CAPABILITY_BLOCKS: &str = "term-blocks";
/// The presence capability a host advertises for [`SESSIONS`].
pub const CAPABILITY_SESSIONS: &str = "term-sessions";
/// The presence capability a host advertises for [`EFFECTS`].
pub const CAPABILITY_EFFECTS: &str = "term-effects";
/// The presence capability a host advertises for [`TYPIST`].
pub const CAPABILITY_TYPIST: &str = "term-typist";

/// `v` of a records frame.
pub const RECORDS: &str = "openagents.terminal-records.v1";
/// `v` of a history read.
pub const HISTORY: &str = "openagents.terminal-history.v1";
/// `v` of a block-journal page read.
pub const BLOCK_PAGE: &str = "openagents.terminal-block-page.v1";
/// `v` of a session read.
pub const SESSION_READ: &str = "openagents.terminal-session-read.v1";
/// `v` of a session write.
pub const SESSION_WRITE: &str = "openagents.terminal-session-write.v1";
/// `v` of a session list.
pub const SESSION_LIST: &str = "openagents.terminal-session-list.v1";
/// `v` of a session removal.
pub const SESSION_REMOVE: &str = "openagents.terminal-session-remove.v1";
/// The most sessions a host keeps.
pub const SESSIONS_MAX: usize = 64;
/// `v` of a take: an attachment becomes the typist.
pub const TAKE: &str = "openagents.terminal-take.v1";
/// `v` of a release: the typist gives the role up.
pub const RELEASE: &str = "openagents.terminal-release.v1";

/// The bytes of a record header: tag, length, and checksum.
pub const RECORD_HEADER: usize = 10;
/// The longest record payload.
pub const PAYLOAD_MAX: usize = 1 << 20;
/// The longest record stream.
pub const STREAM_MAX: usize = 16 << 20;
/// The longest snapshot prefix, every record through `READY`.
pub const PREFIX_MAX: usize = 4 << 20;
/// The most stream bytes one records frame carries.
pub const PART_MAX: usize = 8 * 1024;
/// How many parts a client holds ahead of the next one it expects.
pub const PARTS_AHEAD: u32 = 64;
/// The longest `CONTINUATION` payload.
pub const CONTINUATION_MAX: usize = 4096;
/// The most rows in one `ROWS` record.
pub const ROWS_PAGE_MAX: usize = 64;
/// The most rows in one `HISTORY` record.
pub const HISTORY_PAGE_MAX: usize = 256;
/// The most history rows a snapshot stream sends, and a history read asks.
pub const HISTORY_ROWS_MAX: u64 = 2000;
/// The most `HISTORY` record bytes a snapshot stream sends.
pub const SNAPSHOT_HISTORY_BYTES: usize = 1 << 20;
/// The largest block-page or session result value, as JSON.
pub const RESULT_MAX: usize = 12 * 1024;
/// The most blocks one page asks for.
pub const BLOCK_LIMIT_MAX: u16 = 32;
/// The longest command line or directory in a block record.
pub const BLOCK_TEXT_MAX: usize = 1024;
/// The most members in a session.
pub const MEMBERS_MAX: usize = 64;
/// The most tabs in a session layout.
pub const TABS_MAX: usize = 16;
/// The deepest a tab's layout tree goes.
pub const LAYOUT_DEPTH_MAX: usize = 16;
/// The largest resource reference a session member carries, as JSON.
pub const RESOURCE_MAX: usize = 2048;

/// The extension features one side serves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Features {
    pub proposals: bool,
    pub snapshot: bool,
    pub blocks: bool,
    pub sessions: bool,
    pub effects: bool,
    pub typist: bool,
    /// Terminal shares ([`crate::share`]).
    pub shares: bool,
}

impl Features {
    /// The base profile: no feature.
    pub const NONE: Features = Features {
        proposals: false,
        snapshot: false,
        blocks: false,
        sessions: false,
        effects: false,
        typist: false,
        shares: false,
    };
    /// Every feature this module defines.
    pub const ALL: Features = Features {
        proposals: true,
        snapshot: true,
        blocks: true,
        sessions: true,
        effects: true,
        typist: true,
        shares: true,
    };

    /// The features a host's presence capabilities advertise. A client
    /// names a feature only when this says the host serves it.
    #[must_use]
    pub fn advertised<S: AsRef<str>>(capabilities: &[S]) -> Features {
        let has = |slug: &str| capabilities.iter().any(|c| c.as_ref() == slug);
        Features {
            proposals: has("term-proposals"),
            snapshot: has(CAPABILITY_SNAPSHOT),
            blocks: has(CAPABILITY_BLOCKS),
            sessions: has(CAPABILITY_SESSIONS),
            effects: has(CAPABILITY_EFFECTS),
            typist: has(CAPABILITY_TYPIST),
            shares: has(crate::share::CAPABILITY_SHARES),
        }
    }

    /// The presence capabilities that advertise these features.
    #[must_use]
    pub fn capabilities(self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.proposals {
            out.push("term-proposals");
        }
        if self.snapshot {
            out.push(CAPABILITY_SNAPSHOT);
        }
        if self.blocks {
            out.push(CAPABILITY_BLOCKS);
        }
        if self.sessions {
            out.push(CAPABILITY_SESSIONS);
        }
        if self.effects {
            out.push(CAPABILITY_EFFECTS);
        }
        if self.typist {
            out.push(CAPABILITY_TYPIST);
        }
        if self.shares {
            out.push(crate::share::CAPABILITY_SHARES);
        }
        out
    }

    fn serves(self, id: &str) -> Option<bool> {
        match id {
            crate::proposal::FEATURE => Some(self.proposals),
            SNAPSHOT => Some(self.snapshot),
            BLOCKS => Some(self.blocks),
            SESSIONS => Some(self.sessions),
            EFFECTS => Some(self.effects),
            TYPIST => Some(self.typist),
            crate::share::SHARES => Some(self.shares),
            _ => None,
        }
    }

    /// Checks a body's `requires` against what this side serves and what
    /// the operation accepts, and answers whether it names `allowed`'s
    /// first feature. An unknown or unserved feature, or one the operation
    /// does not take, refuses as `unsupported_feature`.
    pub(crate) fn admit(self, requires: &[String], allowed: &[&str]) -> Result<bool, Refusal> {
        for (index, id) in requires.iter().enumerate() {
            if requires[..index].contains(id) {
                return Err(Refusal::new(
                    Reason::Malformed,
                    "requires repeats a feature",
                ));
            }
            if self.serves(id) != Some(true) || !allowed.contains(&id.as_str()) {
                return Err(Refusal::new(
                    Reason::UnsupportedFeature,
                    format!("this host does not serve {id} here"),
                ));
            }
        }
        Ok(allowed
            .first()
            .is_some_and(|first| requires.iter().any(|id| id == first)))
    }
}

/// How an attachment begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Join {
    /// Replay the ring after `after`, as the base profile does.
    Replay,
    /// Send a snapshot stream, then live frames after its `through`.
    Snapshot,
}

pub(crate) fn ext_header(
    v: &str,
    expected: &str,
    requires: &[String],
    request: &str,
    feature: &str,
    features: Features,
) -> Result<(), Refusal> {
    version(v, expected)?;
    if !features.admit(requires, &[feature])? {
        return Err(Refusal::malformed(format!("{expected} requires {feature}")));
    }
    common_id(request, "request")
}

// ---------------------------------------------------------------------------
// Typist

/// Take or release the typist role for one of the principal's own
/// `interact` attachments: `v` is [`TAKE`] or [`RELEASE`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seat {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    pub attachment: String,
}

impl Seat {
    /// Make `attachment` the typist.
    #[must_use]
    pub fn take(
        request: impl Into<String>,
        terminal: TerminalRef,
        attachment: impl Into<String>,
    ) -> Self {
        Seat::with(TAKE, request, terminal, attachment)
    }

    /// Give the role up from `attachment`.
    #[must_use]
    pub fn release(
        request: impl Into<String>,
        terminal: TerminalRef,
        attachment: impl Into<String>,
    ) -> Self {
        Seat::with(RELEASE, request, terminal, attachment)
    }

    fn with(
        v: &str,
        request: impl Into<String>,
        terminal: TerminalRef,
        attachment: impl Into<String>,
    ) -> Self {
        Seat {
            v: v.into(),
            requires: vec![TYPIST.into()],
            request: request.into(),
            terminal,
            attachment: attachment.into(),
        }
    }

    /// Whether this is a take, rather than a release.
    #[must_use]
    pub fn takes(&self) -> bool {
        self.v == TAKE
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        let expected = if self.v == RELEASE { RELEASE } else { TAKE };
        ext_header(
            &self.v,
            expected,
            &self.requires,
            &self.request,
            TYPIST,
            features,
        )?;
        self.terminal.check()?;
        common_id(&self.attachment, "attachment")
    }
}

// ---------------------------------------------------------------------------
// Agent typists

/// `v` of a handoff: the sender's attachment hands the typist role to an
/// agent.
pub const HANDOFF: &str = "openagents.terminal-handoff.v1";
/// `v` of an agent's input under a handoff.
pub const AGENT_INPUT: &str = "openagents.terminal-agent-input.v1";
/// How many agent inputs a handoff's evidence log keeps.
pub const AGENT_LOG_MAX: usize = 256;

/// Hand the typist role to an agent, bound to the thread and run it works
/// for. The sender's own `interact` attachment must hold the role or the
/// terminal must have no typist.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    pub attachment: String,
    /// The agent's key.
    pub agent: String,
    pub thread: String,
    pub run: String,
}

impl Handoff {
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        terminal: TerminalRef,
        attachment: impl Into<String>,
        agent: impl Into<String>,
        thread: impl Into<String>,
        run: impl Into<String>,
    ) -> Self {
        Handoff {
            v: HANDOFF.into(),
            requires: vec![TYPIST.into()],
            request: request.into(),
            terminal,
            attachment: attachment.into(),
            agent: agent.into(),
            thread: thread.into(),
            run: run.into(),
        }
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            HANDOFF,
            &self.requires,
            &self.request,
            TYPIST,
            features,
        )?;
        self.terminal.check()?;
        common_id(&self.attachment, "attachment")?;
        common_id(&self.agent, "agent")?;
        common_id(&self.thread, "thread")?;
        common_id(&self.run, "run")
    }
}

/// An agent's input under the handoff `lease`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentInput {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    pub lease: String,
    #[serde(with = "b64")]
    pub data: Vec<u8>,
}

impl AgentInput {
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        terminal: TerminalRef,
        lease: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Self {
        AgentInput {
            v: AGENT_INPUT.into(),
            requires: vec![TYPIST.into()],
            request: request.into(),
            terminal,
            lease: lease.into(),
            data: data.into(),
        }
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            AGENT_INPUT,
            &self.requires,
            &self.request,
            TYPIST,
            features,
        )?;
        self.terminal.check()?;
        common_id(&self.lease, "lease")?;
        if self.data.is_empty() || self.data.len() > wire::INPUT_MAX {
            return Err(Refusal::malformed("input is 1 to 4096 bytes"));
        }
        Ok(())
    }
}

/// The agent that holds a terminal's typist role, and what it works for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTypist {
    pub agent: String,
    pub thread: String,
    pub run: String,
    /// The handoff's ID, which typist frames name as the typist.
    pub lease: String,
}

/// One input an agent sent, as the thread's private evidence records it:
/// never the bytes, only who sent how many, when, and for what.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentEvidence {
    pub request: String,
    pub agent: String,
    pub thread: String,
    pub run: String,
    pub bytes: u64,
    /// Unix milliseconds.
    pub at: u64,
}

// ---------------------------------------------------------------------------
// Effects

/// The longest clipboard write an effect frame carries, in bytes. A host
/// delivers no longer write.
pub const CLIPBOARD_MAX: usize = 8 * 1024;
/// The longest directory an effect frame carries, in bytes.
pub const DIRECTORY_MAX: usize = 4096;

/// Something a terminal's output caused that a client acts on, as the
/// host's emulator found it. With [`EFFECTS`], a client takes these from
/// effect frames and never from its own parsing, so a replayed or
/// snapshotted byte rings no bell and writes no clipboard twice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    /// The program rang the bell `count` times.
    Bell { count: u32 },
    /// The window title the program set, or empty when it cleared it.
    Title { title: String },
    /// The working directory the shell reported (OSC 7).
    Directory { dir: String },
    /// The program asked to write the clipboard. Only the attachment whose
    /// principal typed last receives it; a client may refuse it, and a
    /// program can never read the clipboard.
    Clipboard { text: String },
}

impl Effect {
    pub fn check(&self) -> Result<(), Refusal> {
        match self {
            Effect::Bell { count } if *count == 0 => {
                Err(Refusal::malformed("a bell rings at least once"))
            }
            Effect::Bell { .. } => Ok(()),
            Effect::Title { title } => clean(title, 1024, "title"),
            Effect::Directory { dir } => clean(dir, DIRECTORY_MAX, "directory"),
            Effect::Clipboard { text } => {
                if text.len() > CLIPBOARD_MAX {
                    return Err(Refusal::new(
                        Reason::LimitExceeded,
                        "a clipboard write is longer than 8192 bytes",
                    ));
                }
                if text
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
                {
                    return Err(Refusal::malformed(
                        "a clipboard write contains a control character",
                    ));
                }
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CRC-32C

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0x82F6_3B78
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

const CRC_TABLE: [u32; 256] = crc_table();

/// The CRC-32C (Castagnoli) checksum of `bytes`.
#[must_use]
pub fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc = CRC_TABLE[((crc ^ u32::from(byte)) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

// ---------------------------------------------------------------------------
// Record payloads

/// A record's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum Tag {
    Terminal = 1,
    State = 2,
    Rows = 3,
    Continuation = 4,
    Ready = 5,
    History = 6,
    Finish = 7,
}

impl Tag {
    fn from_u16(tag: u16) -> Option<Tag> {
        Some(match tag {
            1 => Tag::Terminal,
            2 => Tag::State,
            3 => Tag::Rows,
            4 => Tag::Continuation,
            5 => Tag::Ready,
            6 => Tag::History,
            7 => Tag::Finish,
            _ => return None,
        })
    }
}

/// Retained history: absolute lines `first` through `first + count - 1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistorySpan {
    pub first: u64,
    pub count: u64,
}

impl HistorySpan {
    /// The absolute line of screen row 0.
    #[must_use]
    pub fn end(self) -> u64 {
        self.first.saturating_add(self.count)
    }
}

/// `TERMINAL`: what a stream describes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalRecord {
    pub format: u32,
    pub generation: String,
    pub terminal: String,
    pub epoch: u64,
    /// The last sequenced frame the state reflects, 0 when none.
    pub through: u64,
    pub size: Size,
    pub history: HistorySpan,
    pub exit: Option<Exit>,
}

/// A color as the program asked for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Default,
    Index(u8),
    Rgb([u8; 3]),
}

/// A run's or the pen's style.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    /// Bold 1, dim 2, italic 4, underline 8, blink 16, inverse 32, hidden
    /// 64, strike 128.
    pub flags: u16,
    /// The OSC 8 target, or null.
    pub link: Option<String>,
}

impl Style {
    /// The default style.
    #[must_use]
    pub fn plain() -> Self {
        Style {
            fg: Color::Default,
            bg: Color::Default,
            flags: 0,
            link: None,
        }
    }

    fn check(&self) -> Result<(), Refusal> {
        if self.flags & !0xff != 0 {
            return Err(Refusal::malformed("a style sets an unknown flag"));
        }
        if let Some(link) = &self.link {
            clean(link, 2048, "link")?;
        }
        Ok(())
    }
}

/// A run of cells in one style.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub text: String,
    /// The columns the text occupies.
    pub cells: u16,
    pub style: Style,
}

/// One row of a screen or of history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// The row continues on the next line.
    pub wrapped: bool,
    pub runs: Vec<Run>,
}

impl Row {
    fn check(&self, cols: u16) -> Result<(), Refusal> {
        let mut total = 0u32;
        for run in &self.runs {
            if run.cells == 0 || run.text.is_empty() {
                return Err(Refusal::malformed("a run has text and at least one cell"));
            }
            clean(&run.text, usize::from(run.cells) * 16, "run text")?;
            run.style.check()?;
            total += u32::from(run.cells);
        }
        if total > u32::from(cols) {
            return Err(Refusal::malformed("a row is wider than the terminal"));
        }
        Ok(())
    }
}

/// The cursor's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Block,
    Underline,
    Bar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
    pub pending_wrap: bool,
    pub visible: bool,
    pub shape: Shape,
    pub blink: bool,
}

/// A cursor `DECSC` saved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub row: u16,
    pub col: u16,
    pub pending_wrap: bool,
    pub pen: Style,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub top: u16,
    pub bottom: u16,
}

/// The `DECSET` and `SM` modes that are set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modes {
    pub private: Vec<u16>,
    pub ansi: Vec<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Charset {
    Ascii,
    DecSpecial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Charsets {
    pub g0: Charset,
    pub g1: Charset,
    pub shift: u8,
}

/// Each screen's Kitty keyboard flag stack, bottom first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyboard {
    pub primary: Vec<u8>,
    pub alternate: Vec<u8>,
}

/// `STATE`: everything but the rows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRecord {
    pub alternate: bool,
    pub cursor: Cursor,
    pub pen: Style,
    pub saved_primary: Option<Saved>,
    pub saved_alternate: Option<Saved>,
    pub scroll: Region,
    pub tabs: Vec<u16>,
    pub modes: Modes,
    pub charsets: Charsets,
    pub keyboard: Keyboard,
    pub title: String,
}

impl StateRecord {
    fn check(&self, size: Size) -> Result<(), Refusal> {
        let within = |row: u16, col: u16| row < size.rows && col < size.cols;
        if !within(self.cursor.row, self.cursor.col) {
            return Err(Refusal::malformed("the cursor lies outside the terminal"));
        }
        self.pen.check()?;
        for saved in [&self.saved_primary, &self.saved_alternate]
            .into_iter()
            .flatten()
        {
            if !within(saved.row, saved.col) {
                return Err(Refusal::malformed(
                    "a saved cursor lies outside the terminal",
                ));
            }
            saved.pen.check()?;
        }
        // A one-row terminal's region is that row, so top equals bottom.
        let (top, bottom) = (self.scroll.top, self.scroll.bottom);
        if !(bottom < size.rows && (top < bottom || (top == bottom && size.rows == 1))) {
            return Err(Refusal::malformed("the scrolling region is out of range"));
        }
        ascending(&self.tabs, usize::from(size.cols), "tabs")?;
        if self.tabs.iter().any(|&tab| tab >= size.cols) {
            return Err(Refusal::malformed("a tab stop lies outside the terminal"));
        }
        ascending(&self.modes.private, 64, "private modes")?;
        ascending(&self.modes.ansi, 64, "ANSI modes")?;
        if self.charsets.shift > 1 {
            return Err(Refusal::malformed("the active character set is 0 or 1"));
        }
        if self.keyboard.primary.len() > 16 || self.keyboard.alternate.len() > 16 {
            return Err(Refusal::malformed("a keyboard flag stack holds at most 16"));
        }
        clean(&self.title, 1024, "title")
    }
}

/// Which screen a `ROWS` page belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenKind {
    Primary,
    Alternate,
}

/// `ROWS`: a page of one screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowsRecord {
    pub screen: ScreenKind,
    pub first: u16,
    pub rows: Vec<Row>,
}

/// `HISTORY`: a page of history rows, oldest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryRecord {
    pub first: u64,
    pub rows: Vec<Row>,
}

/// `FINISH`: what the stream sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishRecord {
    pub rows: u64,
    pub complete: bool,
}

/// One decoded record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Record {
    Terminal(TerminalRecord),
    State(StateRecord),
    Rows(RowsRecord),
    Continuation(Vec<u8>),
    Ready,
    History(HistoryRecord),
    Finish(FinishRecord),
}

impl Record {
    #[must_use]
    pub fn tag(&self) -> Tag {
        match self {
            Record::Terminal(_) => Tag::Terminal,
            Record::State(_) => Tag::State,
            Record::Rows(_) => Tag::Rows,
            Record::Continuation(_) => Tag::Continuation,
            Record::Ready => Tag::Ready,
            Record::History(_) => Tag::History,
            Record::Finish(_) => Tag::Finish,
        }
    }

    /// The record's payload bytes.
    #[must_use]
    pub fn payload(&self) -> Vec<u8> {
        fn json<T: Serialize>(value: &T) -> Vec<u8> {
            serde_json::to_vec(value).expect("a record payload serializes")
        }
        match self {
            Record::Terminal(r) => json(r),
            Record::State(r) => json(r),
            Record::Rows(r) => json(r),
            Record::History(r) => json(r),
            Record::Finish(r) => json(r),
            Record::Continuation(bytes) => bytes.clone(),
            Record::Ready => Vec::new(),
        }
    }

    /// The record with its header: tag, length, and CRC-32C.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let payload = self.payload();
        let mut out = Vec::with_capacity(RECORD_HEADER + payload.len());
        out.extend_from_slice(&(self.tag() as u16).to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&crc32c(&payload).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    fn decode(tag: Tag, payload: &[u8]) -> Result<Record, Refusal> {
        fn parse<T: serde::de::DeserializeOwned>(payload: &[u8]) -> Result<T, Refusal> {
            serde_json::from_slice(payload)
                .map_err(|error| Refusal::malformed(format!("a record payload: {error}")))
        }
        Ok(match tag {
            Tag::Terminal => Record::Terminal(parse(payload)?),
            Tag::State => Record::State(parse(payload)?),
            Tag::Rows => Record::Rows(parse(payload)?),
            Tag::History => Record::History(parse(payload)?),
            Tag::Finish => Record::Finish(parse(payload)?),
            Tag::Continuation => {
                if payload.is_empty() || payload.len() > CONTINUATION_MAX {
                    return Err(Refusal::malformed("a continuation holds 1 to 4096 bytes"));
                }
                Record::Continuation(payload.to_vec())
            }
            Tag::Ready => {
                if !payload.is_empty() {
                    return Err(Refusal::malformed("READY has no payload"));
                }
                Record::Ready
            }
        })
    }
}

/// Encodes records into one stream.
#[must_use]
pub fn encode_stream(records: &[Record]) -> Vec<u8> {
    records.iter().flat_map(Record::encode).collect()
}

/// Cuts a stream into records frames of at most `part_max` bytes each.
#[must_use]
pub fn frames(
    terminal: &TerminalRef,
    attachment: &str,
    stream: &str,
    bytes: &[u8],
    part_max: usize,
) -> Vec<RecordsFrame> {
    let size = part_max.clamp(1, PART_MAX);
    let chunks: Vec<&[u8]> = bytes.chunks(size).collect();
    let count = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(part, data)| RecordsFrame {
            v: RECORDS.into(),
            terminal: terminal.clone(),
            attachment: attachment.into(),
            stream: stream.into(),
            part: part as u32,
            last: part + 1 == count,
            data: data.to_vec(),
        })
        .collect()
}

/// One part of a record stream, delivered to one attachment. Not
/// sequenced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordsFrame {
    pub v: String,
    pub terminal: TerminalRef,
    pub attachment: String,
    pub stream: String,
    pub part: u32,
    pub last: bool,
    #[serde(with = "b64")]
    pub data: Vec<u8>,
}

impl RecordsFrame {
    pub fn check(&self) -> Result<(), Refusal> {
        version(&self.v, RECORDS)?;
        self.terminal.check()?;
        common_id(&self.attachment, "attachment")?;
        common_id(&self.stream, "stream")?;
        if self.data.is_empty() || self.data.len() > PART_MAX {
            return Err(Refusal::malformed(
                "a records frame carries 1 to 8192 bytes",
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Stream assembly

/// What a stream is expected to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamKind {
    /// The answer to an attach by snapshot.
    Snapshot,
    /// The answer to a history read of `rows` rows before `before` in
    /// `epoch`.
    History { epoch: u64, before: u64, rows: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Terminal,
    State,
    Rows(ScreenKind, u16),
    Continuation,
    Ready,
    History,
    Done,
}

/// Reassembles one record stream from its parts and checks every rule a
/// client checks: part order and duplicates, record framing and checksums,
/// record order, the stream's binding, row coverage, history contiguity,
/// and bounds. The first error ends the stream; nothing after it applies.
#[derive(Debug)]
pub struct Assembler {
    kind: StreamKind,
    terminal: TerminalRef,
    stream: Option<String>,
    next_part: u32,
    ahead: BTreeMap<u32, Vec<u8>>,
    seen: Vec<u32>,
    last: Option<u32>,
    buffer: Vec<u8>,
    total: usize,
    phase: Phase,
    binding: Option<TerminalRecord>,
    alternate: bool,
    /// The absolute line the next history page must end before.
    history_end: u64,
    history_rows: u64,
    history_bytes: usize,
    failed: bool,
}

impl Assembler {
    #[must_use]
    pub fn new(kind: StreamKind, terminal: TerminalRef) -> Self {
        Assembler {
            kind,
            terminal,
            stream: None,
            next_part: 0,
            ahead: BTreeMap::new(),
            seen: Vec::new(),
            last: None,
            buffer: Vec::new(),
            total: 0,
            phase: Phase::Terminal,
            binding: None,
            alternate: false,
            history_end: 0,
            history_rows: 0,
            history_bytes: 0,
            failed: false,
        }
    }

    /// The stream's binding, once `READY` (or, in a history stream,
    /// `TERMINAL`) arrived.
    #[must_use]
    pub fn ready(&self) -> Option<&TerminalRecord> {
        let ready = match self.kind {
            StreamKind::Snapshot => matches!(self.phase, Phase::History | Phase::Done),
            StreamKind::History { .. } => self.phase != Phase::Terminal,
        };
        if ready { self.binding.as_ref() } else { None }
    }

    /// Whether the stream ended with `FINISH` and its last part.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.phase == Phase::Done && self.last.is_some_and(|last| self.next_part > last)
    }

    /// Takes one part and returns the records it completed, in order.
    pub fn push(&mut self, frame: &RecordsFrame) -> Result<Vec<Record>, Refusal> {
        if self.failed {
            return Err(Refusal::malformed("the stream already failed"));
        }
        let result = self.push_inner(frame);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn push_inner(&mut self, frame: &RecordsFrame) -> Result<Vec<Record>, Refusal> {
        frame.check()?;
        if frame.terminal != self.terminal {
            return Err(Refusal::new(
                Reason::IdentityMismatch,
                "the records frame names another terminal",
            ));
        }
        match &self.stream {
            Some(stream) if *stream != frame.stream => {
                return Err(Refusal::malformed("a part of another stream"));
            }
            Some(_) => {}
            None => self.stream = Some(frame.stream.clone()),
        }
        if let Some(last) = self.last
            && frame.part > last
        {
            return Err(Refusal::malformed("a part after the last part"));
        }
        if frame.last {
            if self.last.is_some_and(|last| last != frame.part)
                || self.ahead.keys().any(|&part| part > frame.part)
            {
                return Err(Refusal::malformed("two different last parts"));
            }
            self.last = Some(frame.part);
        }
        let digest = crc32c(&frame.data);
        if frame.part < self.next_part {
            return if self.seen[frame.part as usize] == digest {
                Ok(Vec::new())
            } else {
                Err(Refusal::malformed("a repeated part with different bytes"))
            };
        }
        if let Some(held) = self.ahead.get(&frame.part) {
            return if *held == frame.data {
                Ok(Vec::new())
            } else {
                Err(Refusal::malformed("a repeated part with different bytes"))
            };
        }
        if frame.part - self.next_part >= PARTS_AHEAD {
            return Err(Refusal::new(
                Reason::LimitExceeded,
                "a part too far ahead of the next expected one",
            ));
        }
        self.ahead.insert(frame.part, frame.data.clone());
        let mut records = Vec::new();
        while let Some(data) = self.ahead.remove(&self.next_part) {
            self.seen.push(crc32c(&data));
            self.next_part += 1;
            self.total += data.len();
            if self.total > STREAM_MAX {
                return Err(Refusal::new(
                    Reason::LimitExceeded,
                    "the stream is too long",
                ));
            }
            self.buffer.extend_from_slice(&data);
            self.drain(&mut records)?;
        }
        if self.last.is_some_and(|last| self.next_part > last) {
            if !self.buffer.is_empty() {
                return Err(Refusal::malformed(
                    "the stream ended inside a record: a length runs past its end",
                ));
            }
            if self.phase != Phase::Done {
                return Err(Refusal::malformed("the stream ended before FINISH"));
            }
        }
        Ok(records)
    }

    fn drain(&mut self, records: &mut Vec<Record>) -> Result<(), Refusal> {
        let mut offset = 0;
        while self.buffer.len() - offset >= RECORD_HEADER {
            let header = &self.buffer[offset..offset + RECORD_HEADER];
            let tag = u16::from_le_bytes([header[0], header[1]]);
            let length = u32::from_le_bytes([header[2], header[3], header[4], header[5]]) as usize;
            let crc = u32::from_le_bytes([header[6], header[7], header[8], header[9]]);
            let Some(tag) = Tag::from_u16(tag) else {
                return Err(Refusal::malformed(format!("an unknown record tag {tag}")));
            };
            if length > PAYLOAD_MAX {
                return Err(Refusal::malformed("a record length past the bound"));
            }
            if self.buffer.len() - offset - RECORD_HEADER < length {
                break;
            }
            let start = offset + RECORD_HEADER;
            let payload = &self.buffer[start..start + length];
            if crc32c(payload) != crc {
                return Err(Refusal::malformed("a record checksum does not match"));
            }
            let record = Record::decode(tag, payload)?;
            self.accept(&record, RECORD_HEADER + length)?;
            records.push(record);
            offset = start + length;
        }
        self.buffer.drain(..offset);
        Ok(())
    }

    fn accept(&mut self, record: &Record, bytes: usize) -> Result<(), Refusal> {
        let out_of_order = || Refusal::malformed(format!("{:?} out of order", record.tag()));
        if self.kind == StreamKind::Snapshot
            && matches!(
                self.phase,
                Phase::Terminal | Phase::State | Phase::Rows(..) | Phase::Continuation
            )
            && self.total > PREFIX_MAX
        {
            return Err(Refusal::new(
                Reason::LimitExceeded,
                "the snapshot prefix is too long",
            ));
        }
        match (self.phase, record) {
            (Phase::Terminal, Record::Terminal(binding)) => {
                self.bind(binding)?;
                self.phase = match self.kind {
                    StreamKind::Snapshot => Phase::State,
                    StreamKind::History { .. } => Phase::History,
                };
            }
            (Phase::State, Record::State(state)) => {
                let size = self.size();
                state.check(size)?;
                self.alternate = state.alternate;
                self.phase = Phase::Rows(ScreenKind::Primary, 0);
            }
            (Phase::Rows(screen, next), Record::Rows(page)) => {
                let size = self.size();
                if page.screen != screen || page.first != next {
                    return Err(out_of_order());
                }
                if page.rows.is_empty() || page.rows.len() > ROWS_PAGE_MAX {
                    return Err(Refusal::malformed("a ROWS page holds 1 to 64 rows"));
                }
                let end = u32::from(next) + page.rows.len() as u32;
                if end > u32::from(size.rows) {
                    return Err(Refusal::malformed("ROWS past the bottom of the screen"));
                }
                for row in &page.rows {
                    row.check(size.cols)?;
                }
                let end = end as u16;
                self.phase = if end < size.rows {
                    Phase::Rows(screen, end)
                } else if screen == ScreenKind::Primary && self.alternate {
                    Phase::Rows(ScreenKind::Alternate, 0)
                } else {
                    Phase::Continuation
                };
            }
            (Phase::Continuation, Record::Continuation(_)) => self.phase = Phase::Ready,
            (Phase::Continuation | Phase::Ready, Record::Ready) => self.phase = Phase::History,
            (Phase::History, Record::History(page)) => self.history(page, bytes)?,
            (Phase::History, Record::Finish(finish)) => {
                let first = self.binding.as_ref().map_or(0, |b| b.history.first);
                if finish.rows != self.history_rows
                    || finish.complete != (self.history_end == first)
                {
                    return Err(Refusal::malformed("FINISH disagrees with the pages sent"));
                }
                self.phase = Phase::Done;
            }
            _ => return Err(out_of_order()),
        }
        Ok(())
    }

    fn size(&self) -> Size {
        self.binding.as_ref().map_or(Size::new(1, 1), |b| b.size)
    }

    fn bind(&mut self, binding: &TerminalRecord) -> Result<(), Refusal> {
        if binding.format != 1 {
            return Err(Refusal::new(
                Reason::UnsupportedVersion,
                "a stream format other than 1",
            ));
        }
        if binding.generation != self.terminal.generation
            || binding.terminal != self.terminal.terminal
        {
            return Err(Refusal::new(
                Reason::IdentityMismatch,
                "the stream binds another terminal or generation",
            ));
        }
        if binding.epoch == 0 {
            return Err(Refusal::malformed("line epochs start at 1"));
        }
        binding.size.check()?;
        if binding
            .history
            .first
            .checked_add(binding.history.count)
            .is_none()
        {
            return Err(Refusal::malformed("the history span overflows"));
        }
        self.history_end = binding.history.end();
        if let StreamKind::History {
            epoch,
            before,
            rows,
        } = self.kind
        {
            if binding.epoch != epoch {
                return Err(Refusal::new(
                    Reason::Stale,
                    "the stream is for another epoch",
                ));
            }
            if before > binding.history.end() || before <= binding.history.first {
                return Err(Refusal::malformed(
                    "the history stream does not cover its read",
                ));
            }
            if rows == 0 || rows > HISTORY_ROWS_MAX {
                return Err(Refusal::malformed("a history read asks 1 to 2000 rows"));
            }
            self.history_end = before;
        }
        self.binding = Some(binding.clone());
        Ok(())
    }

    fn history(&mut self, page: &HistoryRecord, bytes: usize) -> Result<(), Refusal> {
        let binding = self.binding.as_ref().expect("bound before history");
        let (cols, first) = (binding.size.cols, binding.history.first);
        if page.rows.is_empty() || page.rows.len() > HISTORY_PAGE_MAX {
            return Err(Refusal::malformed("a HISTORY page holds 1 to 256 rows"));
        }
        let count = page.rows.len() as u64;
        if page.first < first || page.first.checked_add(count) != Some(self.history_end) {
            return Err(Refusal::malformed("HISTORY pages are not contiguous"));
        }
        for row in &page.rows {
            row.check(cols)?;
        }
        self.history_end = page.first;
        self.history_rows += count;
        self.history_bytes += bytes;
        let rows_max = match self.kind {
            StreamKind::Snapshot => HISTORY_ROWS_MAX,
            StreamKind::History { rows, .. } => rows,
        };
        if self.history_rows > rows_max
            || (self.kind == StreamKind::Snapshot && self.history_bytes > SNAPSHOT_HISTORY_BYTES)
        {
            return Err(Refusal::new(Reason::LimitExceeded, "too much history"));
        }
        Ok(())
    }
}

/// A client's attach by snapshot: the stream, then live frames. It applies
/// the reconciliation rules: nothing sequenced before `READY`, then the
/// base client state from `through`.
#[derive(Debug)]
pub struct SnapshotJoin {
    assembler: Assembler,
    state: TerminalState,
    lines: usize,
    columns: usize,
}

impl SnapshotJoin {
    #[must_use]
    pub fn new(terminal: TerminalRef, lines: usize, columns: usize) -> Self {
        SnapshotJoin {
            assembler: Assembler::new(StreamKind::Snapshot, terminal.clone()),
            state: TerminalState::new(terminal, lines, columns),
            lines,
            columns,
        }
    }

    /// Applies one part of the snapshot stream and returns the records it
    /// completed. At `READY` the live state starts after `through`.
    pub fn records(&mut self, frame: &RecordsFrame) -> Result<Vec<Record>, Refusal> {
        let was_ready = self.assembler.ready().is_some();
        let records = self.assembler.push(frame)?;
        if !was_ready && let Some(binding) = self.assembler.ready() {
            self.state =
                TerminalState::new(self.state.terminal().clone(), self.lines, self.columns)
                    .starting_after(binding.through);
        }
        Ok(records)
    }

    /// Applies one base frame. A sequenced frame, or a gap, before `READY`
    /// is a protocol error: the client discards the stream and attaches
    /// again.
    pub fn frame(&mut self, frame: &Frame) -> Result<Applied, Refusal> {
        let early = frame.body.seq().is_some() || matches!(frame.body, wire::Body::Gap { .. });
        if early && self.assembler.ready().is_none() {
            return Err(Refusal::malformed("a sequenced frame arrived before READY"));
        }
        Ok(self.state.apply(frame))
    }

    #[must_use]
    pub fn assembler(&self) -> &Assembler {
        &self.assembler
    }

    #[must_use]
    pub fn state(&self) -> &TerminalState {
        &self.state
    }
}

// ---------------------------------------------------------------------------
// Requests

/// Read history rows before `before` into a record stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    pub attachment: String,
    pub epoch: u64,
    pub before: u64,
    pub rows: u64,
}

impl History {
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        terminal: TerminalRef,
        attachment: impl Into<String>,
        epoch: u64,
        before: u64,
        rows: u64,
    ) -> Self {
        History {
            v: HISTORY.into(),
            requires: vec![SNAPSHOT.into()],
            request: request.into(),
            terminal,
            attachment: attachment.into(),
            epoch,
            before,
            rows,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        self.check_with(Features::ALL)
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            HISTORY,
            &self.requires,
            &self.request,
            SNAPSHOT,
            features,
        )?;
        self.terminal.check()?;
        common_id(&self.attachment, "attachment")?;
        if self.epoch == 0 {
            return Err(Refusal::malformed("line epochs start at 1"));
        }
        if self.rows == 0 || self.rows > HISTORY_ROWS_MAX {
            return Err(Refusal::malformed("a history read asks 1 to 2000 rows"));
        }
        Ok(())
    }

    /// How a host that holds `history` in `epoch` answers the read's
    /// range: `Ok` with the rows it sends, or the refusal.
    pub fn admit(&self, epoch: u64, history: HistorySpan) -> Result<u64, Refusal> {
        if self.epoch != epoch {
            return Err(Refusal::new(Reason::Stale, "the line epoch changed"));
        }
        if self.before > history.end() {
            return Err(Refusal::malformed(
                "before lies past the newest history line",
            ));
        }
        if self.before <= history.first {
            return Err(Refusal::new(
                Reason::ContentUnavailable,
                "those rows left the host's retention",
            ));
        }
        Ok((self.before - history.first).min(self.rows))
    }
}

/// Read one page of a terminal's block journal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockPageRead {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub terminal: TerminalRef,
    /// Older blocks than this, or null for the newest.
    pub before: Option<u64>,
    pub limit: u16,
}

impl BlockPageRead {
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        terminal: TerminalRef,
        before: Option<u64>,
        limit: u16,
    ) -> Self {
        BlockPageRead {
            v: BLOCK_PAGE.into(),
            requires: vec![BLOCKS.into()],
            request: request.into(),
            terminal,
            before,
            limit,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        self.check_with(Features::ALL)
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            BLOCK_PAGE,
            &self.requires,
            &self.request,
            BLOCKS,
            features,
        )?;
        self.terminal.check()?;
        if self.limit == 0 || self.limit > BLOCK_LIMIT_MAX {
            return Err(Refusal::malformed("a block page asks 1 to 32 blocks"));
        }
        if self.before == Some(0) {
            return Err(Refusal::malformed("block numbers start at 1"));
        }
        Ok(())
    }
}

/// Who started a block's command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Typed,
    Proposal,
    Agent,
    Unattributed,
}

/// Where a block is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockState {
    Running,
    Finished,
    /// A new prompt arrived without an end mark.
    Abandoned,
}

/// A range of sequence numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeqRange {
    pub from: u64,
    pub to: u64,
}

/// A range of absolute lines in one epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lines {
    pub epoch: u64,
    pub start: u64,
    pub end: u64,
}

/// One block-journal entry: a command's record without its output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    pub block: u64,
    pub origin: Origin,
    pub command: String,
    pub command_truncated: bool,
    pub dir: String,
    pub started: Option<u64>,
    pub ended: Option<u64>,
    pub status: Option<i32>,
    pub state: BlockState,
    pub alternate: bool,
    pub output: Option<SeqRange>,
    pub retained: bool,
    pub lines: Option<Lines>,
}

impl Block {
    fn check(&self) -> Result<(), Refusal> {
        if self.block == 0 {
            return Err(Refusal::malformed("block numbers start at 1"));
        }
        clean(&self.command, BLOCK_TEXT_MAX, "command")?;
        clean(&self.dir, BLOCK_TEXT_MAX, "dir")?;
        if self.alternate && self.output.is_some() {
            return Err(Refusal::malformed(
                "an alternate-screen block has no output range",
            ));
        }
        if let Some(range) = self.output
            && (range.from == 0 || range.to < range.from)
        {
            return Err(Refusal::malformed(
                "an output range is a nonempty sequence range",
            ));
        }
        if self.output.is_none() && self.retained {
            return Err(Refusal::malformed("a block without output retains none"));
        }
        if let Some(lines) = self.lines
            && (lines.epoch == 0 || lines.end < lines.start)
        {
            return Err(Refusal::malformed(
                "a line range is ordered within an epoch",
            ));
        }
        if self.state == BlockState::Running && self.ended.is_some() {
            return Err(Refusal::malformed("a running block has not ended"));
        }
        Ok(())
    }
}

/// A page of the block journal, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockPage {
    pub newest: Option<u64>,
    pub oldest: Option<u64>,
    pub blocks: Vec<Block>,
    pub more: bool,
}

impl BlockPage {
    /// Checks the page against the read it answers.
    pub fn check(&self, read: &BlockPageRead) -> Result<(), Refusal> {
        if self.blocks.len() > usize::from(read.limit) {
            return Err(Refusal::malformed("more blocks than the read's limit"));
        }
        let mut previous = read.before;
        for block in &self.blocks {
            block.check()?;
            if previous.is_some_and(|before| block.block >= before) {
                return Err(Refusal::malformed("blocks are newest first, below before"));
            }
            if self.oldest.is_some_and(|oldest| block.block < oldest)
                || self.newest.is_some_and(|newest| block.block > newest)
            {
                return Err(Refusal::malformed("a block outside the retained range"));
            }
            previous = Some(block.block);
        }
        if self.newest.is_none() != self.oldest.is_none() {
            return Err(Refusal::malformed("an empty journal has neither end"));
        }
        if !self.fits() {
            return Err(Refusal::new(
                Reason::LimitExceeded,
                "the page is over 12288 bytes",
            ));
        }
        Ok(())
    }

    /// Whether the page's JSON fits the result bound. A host drops blocks
    /// from the end of a page until it does.
    #[must_use]
    pub fn fits(&self) -> bool {
        serde_json::to_vec(self).map_or(false, |json| json.len() <= RESULT_MAX)
    }
}

/// Read one session record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRead {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub session: String,
}

impl SessionRead {
    #[must_use]
    pub fn new(request: impl Into<String>, session: impl Into<String>) -> Self {
        SessionRead {
            v: SESSION_READ.into(),
            requires: vec![SESSIONS.into()],
            request: request.into(),
            session: session.into(),
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        self.check_with(Features::ALL)
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            SESSION_READ,
            &self.requires,
            &self.request,
            SESSIONS,
            features,
        )?;
        common_id(&self.session, "session")
    }
}

/// Create or replace one session record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionWrite {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    /// The session, or null to create one.
    pub session: Option<String>,
    /// The revision this write replaces, 0 to create.
    pub base: u64,
    pub record: SessionRecord,
}

impl SessionWrite {
    #[must_use]
    pub fn new(
        request: impl Into<String>,
        session: Option<String>,
        base: u64,
        record: SessionRecord,
    ) -> Self {
        SessionWrite {
            v: SESSION_WRITE.into(),
            requires: vec![SESSIONS.into()],
            request: request.into(),
            session,
            base,
            record,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        self.check_with(Features::ALL)
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            SESSION_WRITE,
            &self.requires,
            &self.request,
            SESSIONS,
            features,
        )?;
        match &self.session {
            None if self.base != 0 => {
                return Err(Refusal::malformed("a create replaces revision 0"));
            }
            Some(_) if self.base == 0 => {
                return Err(Refusal::malformed("a write to a session names its base"));
            }
            Some(session) => common_id(session, "session")?,
            None => {}
        }
        if self.record.session != self.session || self.record.revision != 0 {
            return Err(Refusal::malformed(
                "a written record names its session and revision 0",
            ));
        }
        if self.record.members.iter().any(|m| m.state().is_some()) {
            return Err(Refusal::malformed("a written terminal member has no state"));
        }
        self.record.check()
    }

    /// How a host whose session is at `current` answers the write's base.
    pub fn admit(&self, current: u64) -> Result<(), Refusal> {
        if self.base == current {
            Ok(())
        } else {
            Err(Refusal::new(
                Reason::Stale,
                "the session changed since that revision",
            ))
        }
    }
}

/// List the host's sessions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionList {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
}

impl SessionList {
    #[must_use]
    pub fn new(request: impl Into<String>) -> Self {
        SessionList {
            v: SESSION_LIST.into(),
            requires: vec![SESSIONS.into()],
            request: request.into(),
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        self.check_with(Features::ALL)
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            SESSION_LIST,
            &self.requires,
            &self.request,
            SESSIONS,
            features,
        )
    }
}

/// Remove one session record at revision `base`. Its terminals keep
/// running; removing a session closes nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRemove {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub session: String,
    pub base: u64,
}

impl SessionRemove {
    #[must_use]
    pub fn new(request: impl Into<String>, session: impl Into<String>, base: u64) -> Self {
        SessionRemove {
            v: SESSION_REMOVE.into(),
            requires: vec![SESSIONS.into()],
            request: request.into(),
            session: session.into(),
            base,
        }
    }

    pub fn check(&self) -> Result<(), Refusal> {
        self.check_with(Features::ALL)
    }

    pub fn check_with(&self, features: Features) -> Result<(), Refusal> {
        ext_header(
            &self.v,
            SESSION_REMOVE,
            &self.requires,
            &self.request,
            SESSIONS,
            features,
        )?;
        common_id(&self.session, "session")?;
        if self.base == 0 {
            return Err(Refusal::malformed(
                "a removal names the revision it removes",
            ));
        }
        Ok(())
    }
}

/// One session as a list shows it: no members or layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionEntry {
    pub session: String,
    pub revision: u64,
    pub name: String,
    pub members: u16,
}

/// What the host knows of a terminal member when it reads a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberState {
    Live,
    Closed,
    /// The terminal belonged to an earlier host generation.
    Lost,
}

/// One session member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Member {
    Terminal {
        member: u16,
        terminal: TerminalRef,
        state: Option<MemberState>,
    },
    /// A workbench resource reference the host stores without resolving.
    Resource {
        member: u16,
        resource: serde_json::Value,
    },
}

impl Member {
    #[must_use]
    pub fn id(&self) -> u16 {
        match self {
            Member::Terminal { member, .. } | Member::Resource { member, .. } => *member,
        }
    }

    #[must_use]
    pub fn state(&self) -> Option<MemberState> {
        match self {
            Member::Terminal { state, .. } => *state,
            Member::Resource { .. } => None,
        }
    }
}

/// How a split divides its space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Rows,
    Columns,
}

/// One node of a tab's layout tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Node {
    Pane {
        member: u16,
    },
    Split {
        axis: Axis,
        /// The first child's share, in thousandths.
        ratio: u16,
        first: Box<Node>,
        second: Box<Node>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tab {
    pub name: String,
    pub root: Node,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub tabs: Vec<Tab>,
    pub active: u16,
}

/// A session: its members and its default layout. It holds no terminal
/// output, title, directory, command line, or environment value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRecord {
    pub session: Option<String>,
    pub revision: u64,
    pub name: String,
    pub members: Vec<Member>,
    pub layout: Layout,
}

impl SessionRecord {
    /// Checks the record's shape and bounds.
    pub fn check(&self) -> Result<(), Refusal> {
        if let Some(session) = &self.session {
            common_id(session, "session")?;
        }
        if self.name.is_empty() {
            return Err(Refusal::malformed("a session has a name"));
        }
        clean(&self.name, 128, "name")?;
        if self.members.len() > MEMBERS_MAX {
            return Err(Refusal::new(Reason::LimitExceeded, "more than 64 members"));
        }
        let mut ids = Vec::new();
        for member in &self.members {
            if member.id() == 0 || ids.contains(&member.id()) {
                return Err(Refusal::malformed("member numbers are distinct, from 1"));
            }
            ids.push(member.id());
            match member {
                Member::Terminal { terminal, .. } => terminal.check()?,
                Member::Resource { resource, .. } => {
                    if !resource.is_object() {
                        return Err(Refusal::malformed("a resource reference is an object"));
                    }
                    if serde_json::to_vec(resource).map_or(true, |json| json.len() > RESOURCE_MAX) {
                        return Err(Refusal::new(
                            Reason::LimitExceeded,
                            "a resource reference is over 2048 bytes",
                        ));
                    }
                }
            }
        }
        let tabs = &self.layout.tabs;
        if tabs.is_empty() || tabs.len() > TABS_MAX {
            return Err(Refusal::malformed("a layout has 1 to 16 tabs"));
        }
        if usize::from(self.layout.active) >= tabs.len() {
            return Err(Refusal::malformed("the active tab is not in the layout"));
        }
        let mut shown = Vec::new();
        for tab in tabs {
            clean(&tab.name, 64, "tab name")?;
            node(&tab.root, &ids, &mut shown, 1)?;
        }
        if serde_json::to_vec(self).map_or(true, |json| json.len() > RESULT_MAX) {
            return Err(Refusal::new(
                Reason::LimitExceeded,
                "the record is over 12288 bytes",
            ));
        }
        Ok(())
    }
}

fn node(node: &Node, ids: &[u16], shown: &mut Vec<u16>, depth: usize) -> Result<(), Refusal> {
    if depth > LAYOUT_DEPTH_MAX {
        return Err(Refusal::malformed("a layout tree is at most 16 deep"));
    }
    match node {
        Node::Pane { member } => {
            if !ids.contains(member) {
                return Err(Refusal::malformed("a pane names no member"));
            }
            if shown.contains(member) {
                return Err(Refusal::malformed("a member appears in two panes"));
            }
            shown.push(*member);
        }
        Node::Split {
            ratio,
            first,
            second,
            ..
        } => {
            if !(1..=999).contains(ratio) {
                return Err(Refusal::malformed("a split's ratio is 1 to 999"));
            }
            self::node(first, ids, shown, depth + 1)?;
            self::node(second, ids, shown, depth + 1)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers

/// Bounded text with no control characters.
fn clean(text: &str, max: usize, what: &str) -> Result<(), Refusal> {
    if text.len() > max {
        return Err(Refusal::new(
            Reason::LimitExceeded,
            format!("{what} is longer than {max} bytes"),
        ));
    }
    if text.chars().any(char::is_control) {
        return Err(Refusal::malformed(format!(
            "{what} contains a control character"
        )));
    }
    Ok(())
}

fn ascending(values: &[u16], max: usize, what: &str) -> Result<(), Refusal> {
    if values.len() > max || values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Refusal::malformed(format!(
            "{what} are ascending and distinct, at most {max}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32c_matches_the_standard_check_value() {
        assert_eq!(crc32c(b"123456789"), 0xE306_9283);
        assert_eq!(crc32c(b""), 0);
    }

    #[test]
    fn features_follow_presence_capabilities() {
        let advertised = Features::advertised(&["task-engine", "term-snapshot"]);
        assert!(advertised.snapshot && !advertised.blocks && !advertised.sessions);
        assert!(!advertised.effects && !advertised.typist && !advertised.shares);
        assert!(!advertised.proposals);
        assert!(Features::advertised(&["term-proposals"]).proposals);
        assert_eq!(Features::ALL.capabilities().len(), 7);
    }
}
