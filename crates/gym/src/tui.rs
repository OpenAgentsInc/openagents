//! The Gym terminal: the Coder terminal's amber, reading the Gym's records.
//!
//! This module is a reader. It runs nothing, calls nothing, and opens no
//! socket: it draws what the chain already says. Five views answer five
//! questions — which door is ahead, which families a gate judged and how,
//! which candidates were kept, what one item actually looked like, and
//! whether the record itself still verifies.
//!
//! The design is `crates/coder-terminal`'s and is depended on rather than
//! copied: [`Intensity`], [`Ladder`], [`frame`], and [`rail`] are that
//! crate's. One amber hue over a near-black field, four steps of
//! brightness, and a hairline frame with text riding its rules.
//!
//! A table needs two things a composer does not, and both are marked where
//! they happen: a loudest step that stays legible when the terminal has no
//! color, in [`ladder`]; and a selection cursor in the frame's gutter,
//! where the view's lines are drawn.
//!
//! Two rules outrank every layout decision in here:
//!
//! - **A value nobody measured renders as [`DASH`], never as `0`.** A zero
//!   standing in for "we did not measure" is the fault this crate exists to
//!   prevent, and a rendering layer is the easiest place to reintroduce it.
//!   The [`show`] helpers are the only way a number reaches the screen.
//! - **A door's refusal and a harness failure are different columns, and
//!   they are never summed.** A door whose guardrails fire on the hard
//!   questions would otherwise score better for refusing to answer them.
//!
//! The third rule is nearly as easy to lose: `unverifiable` is a verdict of
//! its own, meaning "we could not tell". It is not a failure and does not
//! render like one. A gate that draws its unverifiable families as failures
//! soon looks like a gate that refuses everything.
//!
//! The types under [`records`] are the terminal's own fixtures. They carry
//! the field names of `openagents.gym.eval_row.v2`, so wiring the real store
//! is a change of type and not a change of column.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

pub use coder_terminal::{Colorless, Colors, Intensity, Ladder, NEAR_BLACK, NEAR_BLACK_TINT, rgb};
use coder_terminal::{frame, rail};

pub use records::{
    Candidate, ChainStatus, DoorIdentity, DoorScore, FamilyVerdict, Outcome, Records, RowView,
    Verdict,
};

/// What a value nobody measured looks like.
pub const DASH: &str = "—";

/// The Gym's ladder over `colors`.
///
/// It is the Coder terminal's ladder with one option taken differently.
/// Under `NO_COLOR` the composer dims the faint half and leaves the rest
/// plain, which is right for prose; a table of verdicts needs its loudest
/// step to separate too, or a failed gate and an unverifiable one draw
/// identically on a colorless terminal and the distinction the third
/// verdict exists for is gone.
pub fn ladder(colors: Colors) -> Ladder {
    Ladder::new(colors).when_colorless(Colorless::DimAndBold)
}

/// The Gym's ladder, at whatever color depth the environment reports.
pub fn ladder_from_environment() -> Ladder {
    Ladder::from_environment().when_colorless(Colorless::DimAndBold)
}

// ---------------------------------------------------------------------------
// Printing a measurement, or printing that there is none
// ---------------------------------------------------------------------------

/// Renderers that print [`DASH`] for anything nobody measured.
///
/// Every value on the screen goes through one of these. A `None` is a
/// measurement that was not taken, and it prints as a dash — not as a zero,
/// a blank, or a plausible default. A measured zero prints as `0`, and the
/// two are different things.
pub mod show {
    use super::DASH;

    /// A measurement, to `decimals` places, or [`DASH`].
    pub fn number(value: Option<f64>, decimals: usize) -> String {
        match value {
            Some(value) => format!("{value:.decimals$}"),
            None => DASH.to_owned(),
        }
    }

    /// A count, or [`DASH`]. A counted zero is still a zero.
    pub fn whole(value: Option<u64>) -> String {
        match value {
            Some(value) => value.to_string(),
            None => DASH.to_owned(),
        }
    }

    /// A duration in milliseconds, or [`DASH`] with no unit attached.
    pub fn millis(value: Option<u64>) -> String {
        match value {
            Some(value) => format!("{value} ms"),
            None => DASH.to_owned(),
        }
    }

    /// Text, or [`DASH`]. Blank text is missing too: an identity field left
    /// empty was never filled in, and empty is not evidence.
    pub fn words(value: Option<&str>) -> String {
        match value {
            Some(value) if !value.trim().is_empty() => value.to_owned(),
            _ => DASH.to_owned(),
        }
    }
}

// ---------------------------------------------------------------------------
// The records the terminal reads
// ---------------------------------------------------------------------------

/// The records the terminal draws.
///
/// These are the terminal's own fixtures, written so the views can be built
/// and tested before the store lands. They carry the field names of
/// `openagents.gym.eval_row.v2`, so the real store replaces a type without
/// moving a column.
pub mod records {
    /// A door's verifiable identity.
    ///
    /// A hosted closed model cannot prove what answered, so `verified` is
    /// false and the rest stays empty rather than invented. That is the
    /// unknown-is-not-zero rule applied to identity.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct DoorIdentity {
        /// The model id the door reports.
        pub model: Option<String>,
        /// A signature over the base weights, when they can be hashed.
        pub base_signature: Option<String>,
        /// The adapter package on top of the base, when there is one.
        pub adapter: Option<String>,
        /// Whether the fields above were checked rather than reported.
        pub verified: bool,
    }

    /// One door's line on the scoreboard.
    ///
    /// `refused` and `harness_failures` are two columns on purpose. A
    /// refusal is a property of the door and belongs in the record; a
    /// harness failure means the record was never made. Adding them would
    /// hide a door that refuses the hard questions behind a broken runner.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct DoorScore {
        /// The door's name, as the run recorded it.
        pub door: String,
        /// The estimator the door answered through.
        pub estimator: String,
        /// Who answered, and whether that can be checked.
        pub identity: DoorIdentity,
        /// Share of scored items answered correctly.
        pub accuracy: Option<f64>,
        /// Expected calibration error over the scored items.
        pub ece: Option<f64>,
        /// Brier score over the scored items.
        pub brier: Option<f64>,
        /// Log loss over the scored items.
        pub log_loss: Option<f64>,
        /// Wrong answers given above the confidence floor.
        pub confident_errors: Option<u64>,
        /// Share of items whose answer changed under a permutation.
        pub flip_rate: Option<f64>,
        /// Items this door scored.
        pub scored: u64,
        /// Items this door refused. Never added to `harness_failures`.
        pub refused: u64,
        /// Items the harness never got an answer for. Never added to
        /// `refused`.
        pub harness_failures: u64,
    }

    /// What a gate concluded about a family.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Verdict {
        /// The family met the rule.
        Passed,
        /// The family missed the rule.
        Failed,
        /// The rule could not be applied. Not a failure.
        #[default]
        Unverifiable,
    }

    impl Verdict {
        /// The verdict's own word.
        pub const fn word(self) -> &'static str {
            match self {
                Verdict::Passed => "passed",
                Verdict::Failed => "failed",
                Verdict::Unverifiable => "unverifiable",
            }
        }

        /// The glyph that leads the word, so the three separate without
        /// color.
        pub const fn mark(self) -> char {
            match self {
                Verdict::Passed => '✓',
                Verdict::Failed => '✗',
                Verdict::Unverifiable => '?',
            }
        }
    }

    /// One family, and the gate that judged it.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct FamilyVerdict {
        /// The family the gate read.
        pub family: String,
        /// What the gate concluded.
        pub verdict: Verdict,
        /// The gate's name.
        pub gate_id: String,
        /// The gate's digest, so changing the rule makes a new rule.
        pub gate_digest: String,
        /// The gate's own sentence about this family.
        pub reason: String,
        /// The metric the gate read, when there was one to read.
        pub observed: Option<f64>,
        /// The threshold the rule set.
        pub threshold: Option<f64>,
        /// Items scored in this family.
        pub scored: u64,
        /// Items the door refused in this family.
        pub refused: u64,
    }

    /// What became of a candidate.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Outcome {
        /// The change was kept.
        Kept,
        /// The change was reverted.
        Reverted,
        /// The comparison did not decide.
        #[default]
        Unverifiable,
    }

    impl Outcome {
        /// The outcome's own word.
        pub const fn word(self) -> &'static str {
            match self {
                Outcome::Kept => "kept",
                Outcome::Reverted => "reverted",
                Outcome::Unverifiable => "unverifiable",
            }
        }

        /// The glyph that leads the word.
        pub const fn mark(self) -> char {
            match self {
                Outcome::Kept => '+',
                Outcome::Reverted => '-',
                Outcome::Unverifiable => '?',
            }
        }
    }

    /// One step of the optimization history.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Candidate {
        /// The candidate's id.
        pub id: String,
        /// What the candidate changed, in one phrase.
        pub changed: String,
        /// The metric the decision turned on.
        pub metric: String,
        /// The metric before the change.
        pub before: Option<f64>,
        /// The metric after the change.
        pub after: Option<f64>,
        /// Whether it was kept, reverted, or left undecided.
        pub outcome: Outcome,
        /// When the decision was recorded.
        pub recorded_at: String,
    }

    /// One item as one door answered it.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct RowView {
        /// The item's id within the suite.
        pub item_id: String,
        /// The family the item belongs to.
        pub family: String,
        /// Which partition the item came from.
        pub split: String,
        /// The state the question was asked about.
        pub state: String,
        /// The question put to the door.
        pub question: String,
        /// The options the door was allowed to choose from.
        pub options: Vec<String>,
        /// The probability the door gave each option, when it gave any.
        pub distribution: Vec<(String, Option<f64>)>,
        /// The label, when the item carries one.
        pub label: Option<String>,
        /// Who wrote the label.
        pub label_source: Option<String>,
        /// The door that answered.
        pub door: String,
        /// The estimator the door answered through.
        pub estimator: String,
        /// Who answered, and whether that can be checked.
        pub identity: DoorIdentity,
        /// Whether an answer came back at all.
        pub answered: bool,
        /// The refusal code, when the door refused.
        pub refusal: Option<String>,
        /// How long the call took.
        pub latency_ms: Option<u64>,
        /// The seed base this row was drawn under.
        pub seed_base: Option<u64>,
        /// The option permutation this row was asked under.
        pub permutation: Option<u32>,
        /// This row's receipt.
        pub receipt: String,
    }

    /// Whether the receipt chain still verifies, and where it does not.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum ChainStatus {
        /// Every row extends the one before it.
        Verified {
            /// How many rows were checked.
            rows: usize,
            /// The receipt at the head.
            head: String,
        },
        /// A row's contents no longer produce the receipt it carries.
        Edited {
            /// The row, counted from one.
            row: usize,
            /// The receipt the row carries.
            carries: String,
            /// The receipt its contents produce.
            digests_to: String,
        },
        /// A row does not follow the row before it: something was inserted,
        /// removed, or reordered.
        Broken {
            /// The row, counted from one.
            row: usize,
            /// The receipt the row says it follows.
            follows: String,
            /// The receipt the row before it carries.
            previous: String,
        },
        /// The chain was not checked. Not the same as a chain that failed to
        /// verify.
        Unchecked {
            /// Why the check did not run.
            why: String,
        },
    }

    impl Default for ChainStatus {
        fn default() -> Self {
            ChainStatus::Unchecked {
                why: "no chain was read".to_owned(),
            }
        }
    }

    impl ChainStatus {
        /// Whether the chain verified.
        pub const fn verified(&self) -> bool {
            matches!(self, ChainStatus::Verified { .. })
        }

        /// Whether the chain failed to verify, which is louder than not
        /// having been checked.
        pub const fn failed(&self) -> bool {
            matches!(
                self,
                ChainStatus::Edited { .. } | ChainStatus::Broken { .. }
            )
        }

        /// The phrase that rides the top rail of every view.
        pub fn phrase(&self) -> String {
            match self {
                ChainStatus::Verified { rows, .. } => format!("chain verified, {rows} rows"),
                ChainStatus::Edited { row, .. } => format!("CHAIN EDITED AT ROW {row}"),
                ChainStatus::Broken { row, .. } => format!("CHAIN BROKEN AT ROW {row}"),
                ChainStatus::Unchecked { .. } => "chain unchecked".to_owned(),
            }
        }
    }

    /// Everything the terminal draws, and where it came from.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Records {
        /// Where these records were read from, for the header line.
        pub source: String,
        /// The suite the run scored.
        pub suite: String,
        /// The suite's content digest.
        pub suite_digest: String,
        /// When the records were produced.
        pub recorded_at: String,
        /// One line per door.
        pub scores: Vec<DoorScore>,
        /// One line per family.
        pub families: Vec<FamilyVerdict>,
        /// The optimization history, newest first.
        pub ladder: Vec<Candidate>,
        /// The rows behind every number above.
        pub rows: Vec<RowView>,
        /// Whether the record still verifies.
        pub chain: ChainStatus,
    }

    impl Records {
        /// The built-in fixture: a small chain, no doors, no network.
        ///
        /// The terminal opens on this so the views can be read and tested
        /// before the store lands. The header says `fixture`, because a
        /// fixture that reads as a measurement is the same lie as a zero
        /// that reads as an observation.
        pub fn fixture() -> Self {
            Self {
                source: "fixture".to_owned(),
                suite: "families-196".to_owned(),
                suite_digest: "suite:9f3c14a2".to_owned(),
                recorded_at: "2026-09-18T11:20:04Z".to_owned(),
                scores: vec![
                    DoorScore {
                        door: "lev-adapter".to_owned(),
                        estimator: "band".to_owned(),
                        identity: DoorIdentity {
                            model: Some("lev-adapter".to_owned()),
                            base_signature: Some("sha256:41b0d8".to_owned()),
                            adapter: Some("families-4e".to_owned()),
                            verified: true,
                        },
                        accuracy: Some(0.862),
                        ece: Some(0.041),
                        brier: Some(0.118),
                        log_loss: Some(0.181),
                        confident_errors: Some(3),
                        flip_rate: Some(0.021),
                        scored: 196,
                        refused: 4,
                        harness_failures: 3,
                    },
                    DoorScore {
                        door: "lev-base".to_owned(),
                        estimator: "band".to_owned(),
                        identity: DoorIdentity {
                            model: Some("lev-base".to_owned()),
                            base_signature: Some("sha256:41b0d8".to_owned()),
                            adapter: None,
                            verified: true,
                        },
                        accuracy: Some(0.784),
                        ece: Some(0.126),
                        brier: Some(0.203),
                        log_loss: Some(1.207),
                        confident_errors: Some(11),
                        flip_rate: Some(0.094),
                        scored: 191,
                        refused: 9,
                        harness_failures: 0,
                    },
                    DoorScore {
                        door: "jev-hosted".to_owned(),
                        estimator: "choice".to_owned(),
                        identity: DoorIdentity {
                            model: Some("systemone-hosted".to_owned()),
                            base_signature: None,
                            adapter: None,
                            verified: false,
                        },
                        accuracy: Some(0.871),
                        ece: None,
                        brier: None,
                        log_loss: None,
                        confident_errors: None,
                        flip_rate: Some(0.016),
                        scored: 200,
                        refused: 0,
                        harness_failures: 0,
                    },
                    DoorScore {
                        door: "kev-0.3".to_owned(),
                        estimator: "pointer".to_owned(),
                        identity: DoorIdentity {
                            model: Some("kev-0.3".to_owned()),
                            base_signature: Some("sha256:7ad901".to_owned()),
                            adapter: None,
                            verified: true,
                        },
                        accuracy: None,
                        ece: None,
                        brier: None,
                        log_loss: None,
                        confident_errors: None,
                        flip_rate: None,
                        scored: 0,
                        refused: 0,
                        harness_failures: 200,
                    },
                ],
                families: vec![
                    FamilyVerdict {
                        family: "weather".to_owned(),
                        verdict: Verdict::Passed,
                        gate_id: "floor".to_owned(),
                        gate_digest: "gate:7c22ef".to_owned(),
                        reason: "accuracy is above the floor on 64 scored items".to_owned(),
                        observed: Some(0.914),
                        threshold: Some(0.800),
                        scored: 64,
                        refused: 2,
                    },
                    FamilyVerdict {
                        family: "scheduling".to_owned(),
                        verdict: Verdict::Failed,
                        gate_id: "floor".to_owned(),
                        gate_digest: "gate:7c22ef".to_owned(),
                        reason: "accuracy is below the floor on 50 scored items".to_owned(),
                        observed: Some(0.611),
                        threshold: Some(0.800),
                        scored: 50,
                        refused: 0,
                    },
                    FamilyVerdict {
                        family: "long-branch".to_owned(),
                        verdict: Verdict::Unverifiable,
                        gate_id: "floor".to_owned(),
                        gate_digest: "gate:7c22ef".to_owned(),
                        reason: "19 of 21 items were refused, so the rule has nothing to read"
                            .to_owned(),
                        observed: None,
                        threshold: Some(0.800),
                        scored: 2,
                        refused: 19,
                    },
                ],
                ladder: vec![
                    Candidate {
                        id: "c-004".to_owned(),
                        changed: "condition the calibration map on the band".to_owned(),
                        metric: "log loss".to_owned(),
                        before: Some(1.207),
                        after: Some(0.181),
                        outcome: Outcome::Kept,
                        recorded_at: "2026-09-18T10:02:11Z".to_owned(),
                    },
                    Candidate {
                        id: "c-003".to_owned(),
                        changed: "shuffle option order in the training data".to_owned(),
                        metric: "flip rate".to_owned(),
                        before: Some(0.094),
                        after: Some(0.021),
                        outcome: Outcome::Kept,
                        recorded_at: "2026-09-17T16:44:52Z".to_owned(),
                    },
                    Candidate {
                        id: "c-002".to_owned(),
                        changed: "widen the admitted option set".to_owned(),
                        metric: "accuracy".to_owned(),
                        before: Some(0.802),
                        after: Some(0.784),
                        outcome: Outcome::Reverted,
                        recorded_at: "2026-09-17T09:31:07Z".to_owned(),
                    },
                    Candidate {
                        id: "c-001".to_owned(),
                        changed: "raise the confidence floor to 0.45".to_owned(),
                        metric: "confident errors".to_owned(),
                        before: None,
                        after: None,
                        outcome: Outcome::Unverifiable,
                        recorded_at: "2026-09-16T14:05:39Z".to_owned(),
                    },
                ],
                rows: vec![
                    RowView {
                        item_id: "weather-0012".to_owned(),
                        family: "weather".to_owned(),
                        split: "development".to_owned(),
                        state: "The caller asked twice for tomorrow's forecast and the agent \
                                answered for today both times."
                            .to_owned(),
                        question: "Should the agent ask the caller to confirm the date?".to_owned(),
                        options: vec!["confirm".to_owned(), "answer".to_owned()],
                        distribution: vec![
                            ("confirm".to_owned(), Some(0.61)),
                            ("answer".to_owned(), Some(0.39)),
                        ],
                        label: Some("confirm".to_owned()),
                        label_source: Some("author".to_owned()),
                        door: "lev-adapter".to_owned(),
                        estimator: "band".to_owned(),
                        identity: DoorIdentity {
                            model: Some("lev-adapter".to_owned()),
                            base_signature: Some("sha256:41b0d8".to_owned()),
                            adapter: Some("families-4e".to_owned()),
                            verified: true,
                        },
                        answered: true,
                        refusal: None,
                        latency_ms: Some(214),
                        seed_base: Some(7),
                        permutation: Some(0),
                        receipt: "receipt:0f3c8a41".to_owned(),
                    },
                    RowView {
                        item_id: "long-branch-0003".to_owned(),
                        family: "long-branch".to_owned(),
                        split: "development".to_owned(),
                        state: "The transcript runs to forty turns and the last eight \
                                contradict the first."
                            .to_owned(),
                        question: "Should the agent escalate to a person?".to_owned(),
                        options: vec!["escalate".to_owned(), "continue".to_owned()],
                        distribution: vec![
                            ("escalate".to_owned(), None),
                            ("continue".to_owned(), None),
                        ],
                        label: Some("escalate".to_owned()),
                        label_source: Some("author".to_owned()),
                        door: "lev-adapter".to_owned(),
                        estimator: "band".to_owned(),
                        identity: DoorIdentity {
                            model: Some("lev-adapter".to_owned()),
                            base_signature: Some("sha256:41b0d8".to_owned()),
                            adapter: Some("families-4e".to_owned()),
                            verified: true,
                        },
                        answered: false,
                        refusal: Some("branch_too_long".to_owned()),
                        latency_ms: None,
                        seed_base: Some(7),
                        permutation: Some(0),
                        receipt: "receipt:5b1d7702".to_owned(),
                    },
                    RowView {
                        item_id: "scheduling-0044".to_owned(),
                        family: "scheduling".to_owned(),
                        split: "calibration".to_owned(),
                        state: "Two meetings overlap and only one has a room booked.".to_owned(),
                        question: "Should the agent move the unbooked meeting?".to_owned(),
                        options: vec!["move".to_owned(), "keep".to_owned(), "ask".to_owned()],
                        distribution: vec![
                            ("move".to_owned(), Some(0.44)),
                            ("keep".to_owned(), Some(0.33)),
                            ("ask".to_owned(), Some(0.23)),
                        ],
                        label: Some("ask".to_owned()),
                        label_source: Some("author".to_owned()),
                        door: "jev-hosted".to_owned(),
                        estimator: "choice".to_owned(),
                        identity: DoorIdentity {
                            model: Some("systemone-hosted".to_owned()),
                            base_signature: None,
                            adapter: None,
                            verified: false,
                        },
                        answered: true,
                        refusal: None,
                        latency_ms: Some(602),
                        seed_base: None,
                        permutation: Some(1),
                        receipt: "receipt:9c40be15".to_owned(),
                    },
                ],
                chain: ChainStatus::Verified {
                    rows: 3,
                    head: "receipt:9c40be15".to_owned(),
                },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The five views
// ---------------------------------------------------------------------------

/// One of the terminal's five views.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum View {
    /// One line per door, sorted, with the current best marked.
    #[default]
    Scoreboard,
    /// Per-family verdicts, and the gate that reached them.
    Families,
    /// The optimization history, kept and reverted alike.
    Ladder,
    /// One item, in full.
    Row,
    /// Whether the record still verifies.
    Chain,
}

impl View {
    /// The views, in the order the number keys select them.
    pub const ALL: [View; 5] = [
        View::Scoreboard,
        View::Families,
        View::Ladder,
        View::Row,
        View::Chain,
    ];

    /// The view's name, as the top rail prints it.
    pub const fn title(self) -> &'static str {
        match self {
            View::Scoreboard => "scoreboard",
            View::Families => "families",
            View::Ladder => "ladder",
            View::Row => "row",
            View::Chain => "chain",
        }
    }

    /// The digit that selects this view.
    pub const fn digit(self) -> char {
        match self {
            View::Scoreboard => '1',
            View::Families => '2',
            View::Ladder => '3',
            View::Row => '4',
            View::Chain => '5',
        }
    }

    /// The view a digit selects, if any.
    pub fn from_digit(digit: char) -> Option<View> {
        View::ALL.into_iter().find(|view| view.digit() == digit)
    }

    /// Where this view sits in [`View::ALL`].
    const fn index(self) -> usize {
        match self {
            View::Scoreboard => 0,
            View::Families => 1,
            View::Ladder => 2,
            View::Row => 3,
            View::Chain => 4,
        }
    }
}

/// A run of text at one intensity.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Span {
    text: String,
    intensity: Intensity,
}

impl Span {
    fn new(text: impl Into<String>, intensity: Intensity) -> Self {
        Self {
            text: text.into(),
            intensity,
        }
    }
}

/// One drawn line.
type Line = Vec<Span>;

/// What a key asked the terminal to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Keep reading.
    Continue,
    /// Leave, restoring the terminal.
    Quit,
}

/// The keys the terminal answers, named by what they do.
///
/// The binary maps crossterm events onto these, so the model can be driven
/// in a test with no terminal attached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyLike {
    /// Leave.
    Quit,
    /// Move the cursor down.
    Down,
    /// Move the cursor up.
    Up,
    /// Open the next view.
    NextView,
    /// Open the previous view.
    PreviousView,
    /// Move the cursor to the first item.
    Home,
    /// Move the cursor to the last item.
    End,
    /// Open the inspector on the selection.
    Inspect,
    /// Open a named view.
    Open(View),
}

/// The terminal's state: the records, which view is open, and where the
/// cursor sits in each.
#[derive(Clone, Debug)]
pub struct App {
    records: Records,
    ladder: Ladder,
    view: View,
    cursors: [usize; 5],
    order: Vec<usize>,
}

impl App {
    /// A terminal over `records`, drawn on `ladder`.
    pub fn new(records: Records, ladder: Ladder) -> Self {
        let order = scoreboard_order(&records);
        Self {
            records,
            ladder,
            view: View::Scoreboard,
            cursors: [0; 5],
            order,
        }
    }

    /// The records being read.
    pub fn records(&self) -> &Records {
        &self.records
    }

    /// The open view.
    pub fn view(&self) -> View {
        self.view
    }

    /// Opens `view`.
    pub fn open(&mut self, view: View) {
        self.view = view;
    }

    /// The cursor in the open view.
    pub fn cursor(&self) -> usize {
        self.cursors[self.view.index()].min(self.length().saturating_sub(1))
    }

    /// How many items the open view can select.
    pub fn length(&self) -> usize {
        match self.view {
            View::Scoreboard => self.records.scores.len(),
            View::Families => self.records.families.len(),
            View::Ladder => self.records.ladder.len(),
            View::Row => self.records.rows.len(),
            View::Chain => 0,
        }
    }

    /// Moves the cursor down one, stopping at the end.
    pub fn down(&mut self) {
        let last = self.length().saturating_sub(1);
        let cursor = self.cursor();
        self.cursors[self.view.index()] = cursor.saturating_add(1).min(last);
    }

    /// Moves the cursor up one, stopping at the start.
    pub fn up(&mut self) {
        let cursor = self.cursor();
        self.cursors[self.view.index()] = cursor.saturating_sub(1);
    }

    /// Opens the next view, wrapping.
    pub fn next_view(&mut self) {
        let next = (self.view.index() + 1) % View::ALL.len();
        self.view = View::ALL[next];
    }

    /// Opens the previous view, wrapping.
    pub fn previous_view(&mut self) {
        let last = View::ALL.len() - 1;
        let previous = (self.view.index() + last) % View::ALL.len();
        self.view = View::ALL[previous];
    }

    /// Opens the inspector on the first row behind the current selection.
    ///
    /// Every number on the screen traces to a row, and this is how you get
    /// there: a door on the scoreboard, or a family under a gate, opens the
    /// first row it was computed from. A selection with no rows behind it
    /// leaves the inspector where it was.
    pub fn inspect(&mut self) {
        let found = match self.view {
            View::Scoreboard => self
                .records
                .scores
                .get(self.selected_score())
                .and_then(|score| {
                    self.records
                        .rows
                        .iter()
                        .position(|row| row.door == score.door)
                }),
            View::Families => self.records.families.get(self.cursor()).and_then(|family| {
                self.records
                    .rows
                    .iter()
                    .position(|row| row.family == family.family)
            }),
            View::Row => match self.records.rows.is_empty() {
                true => None,
                false => Some(self.cursor()),
            },
            View::Ladder | View::Chain => None,
        };
        if let Some(index) = found {
            self.cursors[View::Row.index()] = index;
            self.view = View::Row;
        }
    }

    /// The index into `records.scores` the scoreboard cursor sits on, after
    /// sorting.
    fn selected_score(&self) -> usize {
        self.order.get(self.cursor()).copied().unwrap_or(0)
    }

    /// Maps a key onto the terminal.
    pub fn handle_key(&mut self, key: KeyLike) -> Action {
        match key {
            KeyLike::Quit => return Action::Quit,
            KeyLike::Down => self.down(),
            KeyLike::Up => self.up(),
            KeyLike::NextView => self.next_view(),
            KeyLike::PreviousView => self.previous_view(),
            KeyLike::Home => self.cursors[self.view.index()] = 0,
            KeyLike::End => self.cursors[self.view.index()] = self.length().saturating_sub(1),
            KeyLike::Inspect => self.inspect(),
            KeyLike::Open(view) => self.open(view),
        }
        Action::Continue
    }
}

/// The order the scoreboard draws its doors in.
///
/// Doors sort by accuracy, best first. A door with no accuracy sorts last:
/// an unmeasured door is not a door that scored zero, and it never takes a
/// rank from a door that answered.
fn scoreboard_order(records: &Records) -> Vec<usize> {
    let mut order: Vec<usize> = (0..records.scores.len()).collect();
    order.sort_by(|&left, &right| {
        match (
            records.scores[left].accuracy,
            records.scores[right].accuracy,
        ) {
            (Some(left), Some(right)) => right.total_cmp(&left),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
    order
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

/// The smallest width worth a frame. Below this the terminal says the window
/// is too small rather than drawing half a table.
const WIDTH_MIN: u16 = 24;
/// The smallest height worth a frame.
const HEIGHT_MIN: u16 = 6;

impl App {
    /// Draws the open view into `buf` at `area`.
    ///
    /// Any area is safe to pass. An area too small for the frame gets one
    /// line saying so, and an empty area draws nothing.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let base = Style::new().bg(self.ladder.background());
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_style(base);
            }
        }
        if area.width < WIDTH_MIN || area.height < HEIGHT_MIN {
            self.put(
                buf,
                area.left(),
                area.top(),
                area.width,
                &[Span::new("window too small", Intensity::Half)],
            );
            return;
        }

        // The header names where the records came from, so a fixture never
        // reads as a measurement.
        let header = vec![
            Span::new("gym ", Intensity::Full),
            Span::new(
                format!(
                    "{}  {}  {}  {}",
                    show::words(Some(&self.records.source)),
                    show::words(Some(&self.records.suite)),
                    show::words(Some(&self.records.suite_digest)),
                    show::words(Some(&self.records.recorded_at)),
                ),
                Intensity::Half,
            ),
        ];
        self.put(buf, area.left() + 1, area.top(), area.width - 1, &header);

        let box_area = Rect::new(area.left(), area.top() + 1, area.width, area.height - 1);
        frame(box_area, buf, self.ladder.style(Intensity::ThreeQuarters));

        // A chain that failed to verify rides the top rail of every view, in
        // the loudest tone the terminal has. A chain nobody checked stays
        // quiet: not checked is not the same as not verifying.
        let chain = self.records.chain.phrase();
        let chain_tone = match self.records.chain.failed() {
            true => Intensity::Full,
            false => Intensity::Half,
        };
        rail(
            box_area,
            buf,
            0,
            Some((self.view.title(), self.ladder.style(Intensity::Full))),
            Some((chain.as_str(), self.ladder.style(chain_tone))),
        );

        let keys = "1-5 view   j/k move   enter inspect   q quit";
        let position = match self.length() {
            0 => None,
            length => Some(format!("{}/{}", self.cursor() + 1, length)),
        };
        rail(
            box_area,
            buf,
            box_area.height - 1,
            Some((keys, self.ladder.style(Intensity::Quarter))),
            position
                .as_deref()
                .map(|text| (text, self.ladder.style(Intensity::Half))),
        );

        let inner = Rect::new(
            box_area.left() + 2,
            box_area.top() + 1,
            box_area.width - 4,
            box_area.height - 2,
        );
        self.render_view(inner, buf);
    }

    /// Draws the open view's lines inside the frame.
    fn render_view(&self, inner: Rect, buf: &mut Buffer) {
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let (head, body) = match self.view {
            View::Scoreboard => self.scoreboard(),
            View::Families => self.families(),
            View::Ladder => self.ladder_view(),
            View::Row => self.row(),
            View::Chain => self.chain(),
        };

        let mut top = inner.top();
        if let Some(head) = head {
            self.put(buf, inner.left(), top, inner.width, &head);
            top += 1;
        }

        // The window follows the cursor: it scrolls only when the selection
        // would otherwise fall outside the frame.
        let room = usize::from(inner.bottom().saturating_sub(top));
        let selected = self.selected_line();
        let scroll = match selected {
            Some(line) if line >= room => line + 1 - room,
            _ => 0,
        };
        for (offset, line) in body.iter().skip(scroll).take(room).enumerate() {
            let y = top + offset as u16;
            if Some(scroll + offset) == selected {
                // The cursor the composer has no use for. A composer has
                // one draft and the caret says where you are in it; a table
                // has forty rows and needs to say which one the keys and
                // the inspector will act on. It sits in the gutter between
                // the wall and the text, which is the terminal's own cell
                // and not the shared frame's: `frame` draws its rules and
                // stops there.
                let tint = Style::new().bg(self.ladder.selection());
                for x in inner.left() - 1..inner.right() {
                    buf[(x, y)].set_style(tint);
                }
                let marker = self
                    .ladder
                    .style(Intensity::Full)
                    .bg(self.ladder.selection());
                buf[(inner.left() - 1, y)].set_char('▸').set_style(marker);
            }
            self.put(buf, inner.left(), y, inner.width, line);
        }
    }

    /// Which body line the cursor sits on, when the view has a cursor.
    fn selected_line(&self) -> Option<usize> {
        match self.view {
            View::Scoreboard | View::Families | View::Ladder => Some(self.cursor()),
            View::Row | View::Chain => None,
        }
    }

    /// Writes spans left to right, clipped at `width`, keeping whatever
    /// background the cells already carry.
    fn put(&self, buf: &mut Buffer, x: u16, y: u16, width: u16, spans: &[Span]) {
        if width == 0 || y >= buf.area.bottom() || x >= buf.area.right() {
            return;
        }
        let limit = x.saturating_add(width).min(buf.area.right());
        let mut at = x;
        for span in spans {
            if at >= limit {
                break;
            }
            let text = clip(&span.text, usize::from(limit - at));
            let background = buf[(at, y)].bg;
            let style = self.ladder.style(span.intensity).bg(background);
            buf.set_string(at, y, &text, style);
            at = at.saturating_add(count(&text) as u16);
        }
    }

    /// Draws into a fresh buffer and returns the text, one line per row with
    /// trailing spaces removed.
    ///
    /// This is how the views are tested, and how `gym-terminal --print`
    /// writes them to a pipe with no terminal attached.
    pub fn to_text(&self, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        self.render(area, &mut buf);
        let mut out = String::new();
        for y in area.top()..area.bottom() {
            let mut line = String::new();
            for x in area.left()..area.right() {
                line.push_str(buf[(x, y)].symbol());
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }
}

/// How many cells a string takes. Everything the terminal draws is one cell
/// wide, including the em dash and the box rules.
fn count(text: &str) -> usize {
    text.chars().count()
}

/// Truncates to `width` cells, marking the cut with `…`.
fn clip(text: &str, width: usize) -> String {
    if count(text) <= width {
        return text.to_owned();
    }
    match width {
        0 => String::new(),
        1 => "…".to_owned(),
        _ => {
            let mut out: String = text.chars().take(width - 1).collect();
            out.push('…');
            out
        }
    }
}

/// Pads to `width` cells on the right, so a column of words lines up.
fn left(text: &str, width: usize) -> String {
    let taken = count(text);
    match taken >= width {
        true => clip(text, width),
        false => format!("{text}{}", " ".repeat(width - taken)),
    }
}

/// Pads to `width` cells on the left, so a column of numbers lines up.
fn right(text: &str, width: usize) -> String {
    let taken = count(text);
    match taken >= width {
        true => clip(text, width),
        false => format!("{}{text}", " ".repeat(width - taken)),
    }
}

/// A `name  value` line.
fn field(name: &str, value: &str, tone: Intensity) -> Line {
    vec![
        Span::new(left(name, 14), Intensity::Half),
        Span::new(value.to_owned(), tone),
    ]
}

/// The identity line: what answered, and whether that was checked.
fn identity_line(identity: &DoorIdentity) -> String {
    format!(
        "model {}   base {}   adapter {}   {}",
        show::words(identity.model.as_deref()),
        show::words(identity.base_signature.as_deref()),
        show::words(identity.adapter.as_deref()),
        match identity.verified {
            true => "verified",
            false => "unverified",
        }
    )
}

/// Breaks prose at the last space before `width`.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = match current.is_empty() {
            true => word.to_owned(),
            false => format!("{current} {word}"),
        };
        if count(&candidate) > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current = word.to_owned();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(DASH.to_owned());
    }
    lines
}

// ---------------------------------------------------------------------------
// The view bodies
// ---------------------------------------------------------------------------

impl App {
    /// One line per door: the metrics, the items scored, and the two kinds
    /// of missing answer kept apart.
    fn scoreboard(&self) -> (Option<Line>, Vec<Line>) {
        let head = vec![Span::new(
            format!(
                "{}{}{}{}{}{}{}{}{}{}",
                left("door", 16),
                right("acc", 8),
                right("ece", 8),
                right("brier", 8),
                right("logloss", 9),
                right("conf.err", 9),
                right("flip", 8),
                right("scored", 8),
                right("refused", 9),
                right("harness", 9),
            ),
            Intensity::Half,
        )];

        // The best door is the first one that has an accuracy at all. A door
        // with nothing measured is not in the running.
        let best = self
            .order
            .iter()
            .copied()
            .find(|&index| self.records.scores[index].accuracy.is_some());

        let body = self
            .order
            .iter()
            .map(|&index| {
                let score = &self.records.scores[index];
                let is_best = Some(index) == best;
                let name = match is_best {
                    true => format!("{} best", score.door),
                    false => score.door.clone(),
                };
                let tone = match is_best {
                    true => Intensity::Full,
                    false => Intensity::ThreeQuarters,
                };
                vec![
                    Span::new(left(&name, 16), tone),
                    Span::new(
                        format!(
                            "{}{}{}{}{}{}{}",
                            right(&show::number(score.accuracy, 3), 8),
                            right(&show::number(score.ece, 3), 8),
                            right(&show::number(score.brier, 3), 8),
                            right(&show::number(score.log_loss, 3), 9),
                            right(&show::whole(score.confident_errors), 9),
                            right(&show::number(score.flip_rate, 3), 8),
                            right(&score.scored.to_string(), 8),
                        ),
                        Intensity::ThreeQuarters,
                    ),
                    // Refusals belong to the door; harness failures do not.
                    // Two columns, never one sum.
                    Span::new(right(&score.refused.to_string(), 9), Intensity::Half),
                    Span::new(
                        right(&score.harness_failures.to_string(), 9),
                        Intensity::Quarter,
                    ),
                ]
            })
            .collect();
        (Some(head), body)
    }

    /// One line per family, with the gate that judged it and the verdict's
    /// own word.
    fn families(&self) -> (Option<Line>, Vec<Line>) {
        let head = vec![Span::new(
            format!(
                "{}{}{}{}{}{}{}",
                left("family", 16),
                left("verdict", 16),
                left("gate", 20),
                right("observed", 10),
                right("threshold", 11),
                right("scored", 8),
                right("refused", 9),
            ),
            Intensity::Half,
        )];

        let mut body: Vec<Line> = self
            .records
            .families
            .iter()
            .map(|family| {
                // A failed gate is loud, a passed gate is present, and an
                // unverifiable gate is quiet. Three verdicts, three tones,
                // three words, three glyphs: nothing about `unverifiable`
                // reads as a failure.
                let tone = match family.verdict {
                    Verdict::Passed => Intensity::ThreeQuarters,
                    Verdict::Failed => Intensity::Full,
                    Verdict::Unverifiable => Intensity::Half,
                };
                let verdict = format!("{} {}", family.verdict.mark(), family.verdict.word());
                vec![
                    Span::new(left(&family.family, 16), Intensity::ThreeQuarters),
                    Span::new(left(&verdict, 16), tone),
                    Span::new(
                        left(&format!("{} {}", family.gate_id, family.gate_digest), 20),
                        Intensity::Quarter,
                    ),
                    Span::new(
                        format!(
                            "{}{}{}{}",
                            right(&show::number(family.observed, 3), 10),
                            right(&show::number(family.threshold, 3), 11),
                            right(&family.scored.to_string(), 8),
                            right(&family.refused.to_string(), 9),
                        ),
                        Intensity::ThreeQuarters,
                    ),
                ]
            })
            .collect();

        // The gate's own sentence about the selected family, under the
        // table, because a verdict without its reason is a number again.
        if let Some(family) = self.records.families.get(self.cursor()) {
            body.push(Vec::new());
            body.push(vec![Span::new(
                format!("{}: {}", family.family, family.reason),
                Intensity::Half,
            )]);
        }
        (Some(head), body)
    }

    /// The optimization history. A reverted candidate draws as brightly as a
    /// kept one, because a change that did not survive is evidence too.
    fn ladder_view(&self) -> (Option<Line>, Vec<Line>) {
        let head = vec![Span::new(
            format!(
                "{}{}{}{}{}{}",
                left("candidate", 11),
                left("changed", 44),
                left("metric", 18),
                right("before", 9),
                right("after", 9),
                left("  outcome", 16),
            ),
            Intensity::Half,
        )];

        let mut body: Vec<Line> = self
            .records
            .ladder
            .iter()
            .map(|candidate| {
                let tone = match candidate.outcome {
                    Outcome::Kept | Outcome::Reverted => Intensity::ThreeQuarters,
                    Outcome::Unverifiable => Intensity::Half,
                };
                let outcome = format!(
                    "  {} {}",
                    candidate.outcome.mark(),
                    candidate.outcome.word()
                );
                vec![
                    Span::new(left(&candidate.id, 11), Intensity::ThreeQuarters),
                    Span::new(left(&candidate.changed, 44), Intensity::ThreeQuarters),
                    Span::new(left(&candidate.metric, 18), Intensity::Half),
                    Span::new(
                        format!(
                            "{}{}",
                            right(&show::number(candidate.before, 3), 9),
                            right(&show::number(candidate.after, 3), 9),
                        ),
                        Intensity::ThreeQuarters,
                    ),
                    Span::new(left(&outcome, 16), tone),
                ]
            })
            .collect();

        if let Some(candidate) = self.records.ladder.get(self.cursor()) {
            body.push(Vec::new());
            body.push(vec![Span::new(
                format!(
                    "{} recorded at {}",
                    candidate.id,
                    show::words(Some(&candidate.recorded_at))
                ),
                Intensity::Quarter,
            )]);
        }
        (Some(head), body)
    }

    /// One item in full: the state, the question, the options, the
    /// distribution, the label, and who answered.
    fn row(&self) -> (Option<Line>, Vec<Line>) {
        let Some(row) = self.records.rows.get(self.cursor()) else {
            return (
                None,
                vec![vec![Span::new("no rows in this chain", Intensity::Half)]],
            );
        };

        let mut body: Vec<Line> = vec![
            vec![
                Span::new(left("item", 13), Intensity::Half),
                Span::new(
                    format!(
                        "{}   family {}   split {}",
                        row.item_id, row.family, row.split
                    ),
                    Intensity::Full,
                ),
            ],
            vec![
                Span::new(left("door", 13), Intensity::Half),
                Span::new(
                    format!(
                        "{}   estimator {}   seed {}   permutation {}",
                        row.door,
                        row.estimator,
                        show::whole(row.seed_base),
                        show::whole(row.permutation.map(u64::from)),
                    ),
                    Intensity::ThreeQuarters,
                ),
            ],
            vec![
                Span::new(left("identity", 13), Intensity::Half),
                Span::new(identity_line(&row.identity), Intensity::ThreeQuarters),
            ],
            Vec::new(),
            vec![Span::new("state", Intensity::Half)],
        ];
        for line in wrap(&row.state, 76) {
            body.push(vec![Span::new(
                format!("  {line}"),
                Intensity::ThreeQuarters,
            )]);
        }
        body.push(vec![Span::new("question", Intensity::Half)]);
        for line in wrap(&row.question, 76) {
            body.push(vec![Span::new(
                format!("  {line}"),
                Intensity::ThreeQuarters,
            )]);
        }

        body.push(vec![Span::new("options", Intensity::Half)]);
        body.push(vec![Span::new(
            match row.options.is_empty() {
                true => format!("  {DASH}"),
                false => format!("  {}", row.options.join("   ")),
            },
            Intensity::ThreeQuarters,
        )]);

        body.push(vec![Span::new("distribution", Intensity::Half)]);
        if row.distribution.is_empty() {
            body.push(vec![Span::new(
                format!("  {DASH}"),
                Intensity::ThreeQuarters,
            )]);
        }
        for (option, probability) in &row.distribution {
            body.push(vec![
                Span::new(format!("  {}", left(option, 14)), Intensity::ThreeQuarters),
                Span::new(right(&show::number(*probability, 3), 7), Intensity::Full),
            ]);
        }
        body.push(Vec::new());

        // A refusal is the door's answer, not a missing row. It says so in
        // its own line, in its own word.
        if let Some(refusal) = &row.refusal {
            body.push(vec![
                Span::new(left("refused", 13), Intensity::Half),
                Span::new(refusal.clone(), Intensity::Full),
            ]);
        }
        body.push(vec![
            Span::new(left("label", 13), Intensity::Half),
            Span::new(
                format!(
                    "{}   source {}",
                    show::words(row.label.as_deref()),
                    show::words(row.label_source.as_deref())
                ),
                Intensity::ThreeQuarters,
            ),
        ]);
        body.push(vec![
            Span::new(left("answered", 13), Intensity::Half),
            Span::new(
                format!(
                    "{}   latency {}",
                    match row.answered {
                        true => "yes",
                        false => "no",
                    },
                    show::millis(row.latency_ms)
                ),
                Intensity::ThreeQuarters,
            ),
        ]);
        body.push(vec![
            Span::new(left("receipt", 13), Intensity::Half),
            Span::new(show::words(Some(&row.receipt)), Intensity::Quarter),
        ]);
        (None, body)
    }

    /// Whether the record still verifies, and where it does not.
    fn chain(&self) -> (Option<Line>, Vec<Line>) {
        let mut body: Vec<Line> = Vec::new();
        match &self.records.chain {
            ChainStatus::Verified { rows, head } => {
                body.push(field("chain", "verified", Intensity::ThreeQuarters));
                body.push(field("rows", &rows.to_string(), Intensity::ThreeQuarters));
                body.push(field("head", head, Intensity::Quarter));
                body.push(Vec::new());
                body.push(vec![Span::new(
                    "Every row digests to the receipt it carries, and every row follows \
                     the one before it.",
                    Intensity::Half,
                )]);
            }
            ChainStatus::Edited {
                row,
                carries,
                digests_to,
            } => {
                body.push(field("chain", "EDITED", Intensity::Full));
                body.push(field("row", &row.to_string(), Intensity::Full));
                body.push(field("carries", carries, Intensity::ThreeQuarters));
                body.push(field("digests to", digests_to, Intensity::ThreeQuarters));
                body.push(Vec::new());
                body.push(vec![Span::new(
                    format!(
                        "Row {row} carries a receipt its own contents do not produce. The \
                         row was edited after it was written."
                    ),
                    Intensity::Full,
                )]);
            }
            ChainStatus::Broken {
                row,
                follows,
                previous,
            } => {
                body.push(field("chain", "BROKEN", Intensity::Full));
                body.push(field("row", &row.to_string(), Intensity::Full));
                body.push(field("follows", follows, Intensity::ThreeQuarters));
                body.push(field("previous", previous, Intensity::ThreeQuarters));
                body.push(Vec::new());
                body.push(vec![Span::new(
                    format!(
                        "Row {row} says it follows a receipt the row before it does not \
                         carry. A row was inserted, removed, or reordered."
                    ),
                    Intensity::Full,
                )]);
            }
            ChainStatus::Unchecked { why } => {
                // Unchecked is the chain's own third verdict, and it reads
                // like the gate's: not a pass, and not a failure.
                body.push(field("chain", "unchecked", Intensity::Half));
                body.push(field("why", why, Intensity::Half));
                body.push(Vec::new());
                body.push(vec![Span::new(
                    "The chain was not checked. That is not the same as a chain that \
                     failed to verify.",
                    Intensity::Half,
                )]);
            }
        }
        (None, body)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    /// The terminal as a reader with `NO_COLOR` set sees it.
    fn app(records: Records) -> App {
        App::new(records, ladder(Colors::None))
    }

    fn colored(records: Records) -> App {
        App::new(records, ladder(Colors::True))
    }

    fn view_text(records: &Records, view: View) -> String {
        let mut app = app(records.clone());
        app.open(view);
        app.to_text(120, 40)
    }

    /// The line holding `needle`, with the frame and the cursor marker taken
    /// out so the columns can be read by splitting on whitespace.
    fn line_containing(text: &str, needle: &str) -> String {
        text.lines()
            .find(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no line contains {needle}:\n{text}"))
            .replace(['▸', '│'], " ")
    }

    fn columns(line: &str) -> Vec<&str> {
        line.split_whitespace().collect()
    }

    /// The color and the modifiers of the cell the first character of
    /// `needle` lands in. On a colorless terminal the color says nothing
    /// and the modifiers carry the whole distinction, so both are read.
    fn drawn(app: &App, needle: &str) -> (Color, Modifier) {
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);
        app.render(area, &mut buf);
        for y in area.top()..area.bottom() {
            let mut line = String::new();
            for x in area.left()..area.right() {
                line.push_str(buf[(x, y)].symbol());
            }
            if let Some(index) = line.find(needle) {
                let column = line[..index].chars().count() as u16;
                let cell = &buf[(area.left() + column, y)];
                return (cell.fg, cell.modifier);
            }
        }
        panic!("{needle} was not drawn");
    }

    /// The color of the cell the first character of `needle` lands in.
    fn color_of(app: &App, needle: &str) -> Color {
        drawn(app, needle).0
    }

    /// A door with nothing measured, so every metric column is unknown.
    fn unmeasured_door() -> DoorScore {
        DoorScore {
            door: "kev-0.3".to_owned(),
            estimator: "pointer".to_owned(),
            identity: DoorIdentity::default(),
            accuracy: None,
            ece: None,
            brier: None,
            log_loss: None,
            confident_errors: None,
            flip_rate: None,
            scored: 196,
            refused: 4,
            harness_failures: 3,
        }
    }

    /// Records in which nothing optional was measured.
    fn nothing_measured() -> Records {
        Records {
            scores: vec![unmeasured_door()],
            families: vec![FamilyVerdict {
                family: "long-branch".to_owned(),
                verdict: Verdict::Unverifiable,
                gate_id: "floor".to_owned(),
                gate_digest: "gate:7c22ef".to_owned(),
                reason: "every item was refused, so the rule has nothing to read".to_owned(),
                observed: None,
                threshold: None,
                scored: 196,
                refused: 4,
            }],
            ladder: vec![Candidate {
                id: "c-001".to_owned(),
                changed: "raise the confidence floor".to_owned(),
                metric: "confident errors".to_owned(),
                before: None,
                after: None,
                outcome: Outcome::Unverifiable,
                recorded_at: "2026-09-16T14:05:39Z".to_owned(),
            }],
            rows: vec![RowView {
                item_id: "long-branch-0003".to_owned(),
                family: "long-branch".to_owned(),
                split: "development".to_owned(),
                state: "Forty turns, and the last eight contradict the first.".to_owned(),
                question: "Should the agent escalate?".to_owned(),
                options: vec!["escalate".to_owned(), "continue".to_owned()],
                distribution: vec![("escalate".to_owned(), None), ("continue".to_owned(), None)],
                label: None,
                label_source: None,
                door: "kev-0.3".to_owned(),
                estimator: "pointer".to_owned(),
                identity: DoorIdentity::default(),
                answered: false,
                refusal: Some("branch_too_long".to_owned()),
                latency_ms: None,
                seed_base: None,
                permutation: None,
                receipt: "receipt:5b1d7702".to_owned(),
            }],
            chain: ChainStatus::Unchecked {
                why: "the store has not landed yet".to_owned(),
            },
            ..Records::fixture()
        }
    }

    #[test]
    fn the_ladder_orders_by_brightness() {
        assert!(Intensity::Quarter < Intensity::Half);
        assert!(Intensity::Half < Intensity::ThreeQuarters);
        assert!(Intensity::ThreeQuarters < Intensity::Full);
        assert_eq!(Intensity::ALL.len(), 4);
        assert_eq!(Intensity::Full.color(), 0xffb000);
    }

    #[test]
    fn no_color_wins_over_colorterm() {
        let both = |name: &str| match name {
            "NO_COLOR" => Some("1".to_owned()),
            "COLORTERM" => Some("truecolor".to_owned()),
            _ => None,
        };
        assert_eq!(Ladder::detected(both).colors(), Colors::None);
        let truecolor = |name: &str| match name {
            "COLORTERM" => Some("24bit".to_owned()),
            _ => None,
        };
        assert_eq!(Ladder::detected(truecolor).colors(), Colors::True);
        assert_eq!(Ladder::detected(|_| None).colors(), Colors::Indexed);
    }

    #[test]
    fn unknown_renders_as_a_dash() {
        assert_eq!(show::number(None, 3), DASH);
        assert_eq!(show::whole(None), DASH);
        assert_eq!(show::millis(None), DASH);
        assert_eq!(show::words(None), DASH);
        assert_eq!(show::words(Some("  ")), DASH);
        // A measured zero is still a zero. Only the absent one is a dash.
        assert_eq!(show::number(Some(0.0), 3), "0.000");
        assert_eq!(show::whole(Some(0)), "0");
    }

    #[test]
    fn no_view_stands_a_zero_in_for_an_unknown() {
        let records = nothing_measured();
        for view in View::ALL {
            let text = view_text(&records, view);
            assert!(
                view == View::Chain || text.contains(DASH),
                "{} should print a dash for what it does not know:\n{text}",
                view.title()
            );
            assert!(
                !text.contains("0.0"),
                "an unmeasured value became a zero in {}:\n{text}",
                view.title()
            );
            assert!(
                !text.contains("0 ms"),
                "an unmeasured latency became a zero in {}:\n{text}",
                view.title()
            );
        }
    }

    #[test]
    fn an_unmeasured_latency_is_a_dash_and_never_a_zero() {
        let text = view_text(&nothing_measured(), View::Row);
        let line = line_containing(&text, "latency");
        assert!(line.contains(&format!("latency {DASH}")), "{line}");
        assert!(!line.contains("latency 0"), "{line}");
    }

    #[test]
    fn an_unmeasured_metric_is_a_dash_on_the_scoreboard() {
        let text = view_text(&nothing_measured(), View::Scoreboard);
        let line = line_containing(&text, "kev-0.3");
        // The door, then six unmeasured metric columns, then the counts.
        assert_eq!(&columns(&line)[1..7], &[DASH; 6], "{line}");
    }

    #[test]
    fn a_refusal_and_a_harness_failure_are_different_columns() {
        let text = view_text(&nothing_measured(), View::Scoreboard);
        let line = line_containing(&text, "kev-0.3");
        let columns = columns(&line);
        assert_eq!(&columns[columns.len() - 3..], &["196", "4", "3"], "{line}");
        assert!(
            !line.contains(" 7 "),
            "a refusal was summed with a harness failure: {line}"
        );
    }

    #[test]
    fn an_unmeasured_door_never_takes_the_best_mark() {
        let mut records = Records::fixture();
        records.scores.retain(|score| score.door != "jev-hosted");
        let text = view_text(&records, View::Scoreboard);
        assert!(line_containing(&text, "best").contains("lev-adapter"));
        assert!(!line_containing(&text, "kev-0.3").contains("best"));
        // The door with no accuracy sorts last rather than sorting as a zero.
        let lines: Vec<&str> = text.lines().collect();
        let best_at = lines.iter().position(|line| line.contains("best"));
        let unmeasured_at = lines.iter().position(|line| line.contains("kev-0.3"));
        assert!(best_at < unmeasured_at, "{text}");
    }

    #[test]
    fn a_failed_verdict_and_an_unverifiable_verdict_look_different() {
        let records = Records::fixture();
        let text = view_text(&records, View::Families);
        let failed = line_containing(&text, "scheduling");
        let unverifiable = line_containing(&text, "long-branch");
        assert!(failed.contains("✗ failed"), "{failed}");
        assert!(unverifiable.contains("? unverifiable"), "{unverifiable}");
        assert!(
            !unverifiable.contains("✗"),
            "an unverifiable family took the failure mark: {unverifiable}"
        );

        // With no color the glyphs separate the two, and so does the
        // weight: `Colorless::DimAndBold` is what keeps the loudest step
        // from flattening into the plain one here.
        let mut colorless = app(records.clone());
        colorless.open(View::Families);
        let (_, failed_weight) = drawn(&colorless, "✗ failed");
        let (_, unverifiable_weight) = drawn(&colorless, "? unverifiable");
        assert_ne!(
            failed_weight, unverifiable_weight,
            "a colorless terminal drew the two verdicts alike"
        );
        assert!(failed_weight.contains(Modifier::BOLD), "{failed_weight:?}");
        assert!(
            unverifiable_weight.contains(Modifier::DIM),
            "{unverifiable_weight:?}"
        );

        let mut app = colored(records);
        app.open(View::Families);
        let failed = color_of(&app, "✗ failed");
        let unverifiable = color_of(&app, "? unverifiable");
        let passed = color_of(&app, "✓ passed");
        assert_ne!(failed, unverifiable, "unverifiable drew as a failure");
        assert_ne!(passed, unverifiable);
        assert_eq!(failed, rgb(Intensity::Full.color()));
        assert_eq!(unverifiable, rgb(Intensity::Half.color()));
    }

    #[test]
    fn an_unverifiable_family_keeps_its_gate_and_reads_its_metric_as_unknown() {
        let mut app = app(Records::fixture());
        app.open(View::Families);
        app.handle_key(KeyLike::End);
        let text = app.to_text(120, 40);
        assert!(text.contains("gate:7c22ef"), "{text}");
        let unverifiable = line_containing(&text, "? unverifiable");
        assert!(columns(&unverifiable).contains(&DASH), "{unverifiable}");
        // The gate's own sentence about the selection, so a verdict never
        // stands on the screen without its reason.
        assert!(text.contains("nothing to read"), "{text}");
    }

    #[test]
    fn a_reverted_candidate_draws_as_brightly_as_a_kept_one() {
        let mut app = colored(Records::fixture());
        app.open(View::Ladder);
        assert_eq!(color_of(&app, "+ kept"), color_of(&app, "- reverted"));
        let text = view_text(&Records::fixture(), View::Ladder);
        assert!(text.contains("- reverted"), "{text}");
        assert!(text.contains("? unverifiable"), "{text}");
        assert!(text.contains("widen the admitted option set"), "{text}");
    }

    #[test]
    fn the_inspector_shows_the_distribution_the_label_and_the_identity() {
        let text = view_text(&Records::fixture(), View::Row);
        assert!(text.contains("weather-0012"), "{text}");
        assert!(text.contains("confirm"), "{text}");
        assert!(text.contains("0.610"), "{text}");
        assert!(text.contains("source author"), "{text}");
        assert!(text.contains("latency 214 ms"), "{text}");
    }

    #[test]
    fn a_hosted_door_reads_as_unverified_with_no_signature_invented() {
        let mut records = Records::fixture();
        records.rows.retain(|row| row.door == "jev-hosted");
        let text = view_text(&records, View::Row);
        let line = line_containing(&text, "identity");
        assert!(line.contains("unverified"), "{line}");
        assert!(line.contains(&format!("base {DASH}")), "{line}");
        assert!(line.contains(&format!("adapter {DASH}")), "{line}");
    }

    #[test]
    fn a_refused_row_says_so_and_is_not_a_missing_row() {
        let mut records = Records::fixture();
        records.rows.retain(|row| row.refusal.is_some());
        let text = view_text(&records, View::Row);
        assert!(text.contains("branch_too_long"), "{text}");
        assert!(line_containing(&text, "answered").contains("no"), "{text}");
        assert!(text.contains("long-branch-0003"), "{text}");
    }

    #[test]
    fn a_broken_chain_is_the_loudest_thing_on_the_screen() {
        let mut records = Records::fixture();
        records.chain = ChainStatus::Broken {
            row: 3,
            follows: "receipt:aa11".to_owned(),
            previous: "receipt:bb22".to_owned(),
        };
        // The break rides the rail of every view, not only the chain view.
        for view in View::ALL {
            let text = view_text(&records, view);
            assert!(
                text.contains("CHAIN BROKEN AT ROW 3"),
                "{} hid a broken chain:\n{text}",
                view.title()
            );
        }
        let app = colored(records.clone());
        assert_eq!(
            color_of(&app, "CHAIN BROKEN AT ROW 3"),
            rgb(Intensity::Full.color())
        );
        let text = view_text(&records, View::Chain);
        assert!(text.contains("inserted, removed, or reordered"), "{text}");
    }

    #[test]
    fn an_edited_row_reads_differently_from_a_reordered_one() {
        let mut records = Records::fixture();
        records.chain = ChainStatus::Edited {
            row: 2,
            carries: "receipt:aa11".to_owned(),
            digests_to: "receipt:bb22".to_owned(),
        };
        let edited = view_text(&records, View::Chain);
        assert!(
            edited.contains("was edited after it was written"),
            "{edited}"
        );
        assert!(!edited.contains("reordered"), "{edited}");

        records.chain = ChainStatus::Broken {
            row: 2,
            follows: "receipt:aa11".to_owned(),
            previous: "receipt:bb22".to_owned(),
        };
        let broken = view_text(&records, View::Chain);
        assert!(broken.contains("reordered"), "{broken}");
        assert!(!broken.contains("was edited"), "{broken}");
    }

    #[test]
    fn an_unchecked_chain_does_not_read_as_a_failed_one() {
        let records = nothing_measured();
        let text = view_text(&records, View::Chain);
        assert!(text.contains("chain unchecked"), "{text}");
        assert!(!text.contains("BROKEN"), "{text}");
        assert!(text.contains("the store has not landed yet"), "{text}");
        let mut app = colored(records);
        app.open(View::Chain);
        assert_eq!(
            color_of(&app, "chain unchecked"),
            rgb(Intensity::Half.color())
        );
    }

    #[test]
    fn the_verified_chain_names_its_head() {
        let text = view_text(&Records::fixture(), View::Chain);
        assert!(text.contains("chain verified, 3 rows"), "{text}");
        assert!(text.contains("receipt:9c40be15"), "{text}");
    }

    #[test]
    fn the_header_says_where_the_records_came_from() {
        let text = view_text(&Records::fixture(), View::Scoreboard);
        let header = text.lines().next().unwrap_or_default();
        assert!(header.contains("gym"), "{header}");
        assert!(header.contains("fixture"), "{header}");
        assert!(header.contains("suite:9f3c14a2"), "{header}");
    }

    #[test]
    fn keys_walk_the_views_and_the_cursor() {
        let mut app = app(Records::fixture());
        assert_eq!(app.view(), View::Scoreboard);
        assert_eq!(app.handle_key(KeyLike::NextView), Action::Continue);
        assert_eq!(app.view(), View::Families);
        app.handle_key(KeyLike::PreviousView);
        assert_eq!(app.view(), View::Scoreboard);
        app.handle_key(KeyLike::PreviousView);
        assert_eq!(app.view(), View::Chain, "the views wrap");
        app.handle_key(KeyLike::Open(View::Ladder));
        assert_eq!(app.view(), View::Ladder);
        app.handle_key(KeyLike::Down);
        assert_eq!(app.cursor(), 1);
        app.handle_key(KeyLike::End);
        assert_eq!(app.cursor(), app.length() - 1);
        app.handle_key(KeyLike::Down);
        assert_eq!(
            app.cursor(),
            app.length() - 1,
            "the cursor stops at the end"
        );
        app.handle_key(KeyLike::Home);
        assert_eq!(app.cursor(), 0);
        app.handle_key(KeyLike::Up);
        assert_eq!(app.cursor(), 0, "the cursor stops at the start");
        assert_eq!(app.handle_key(KeyLike::Quit), Action::Quit);
    }

    #[test]
    fn a_digit_opens_its_view() {
        for view in View::ALL {
            assert_eq!(View::from_digit(view.digit()), Some(view));
        }
        assert_eq!(View::from_digit('9'), None);
    }

    #[test]
    fn the_cursor_is_remembered_per_view() {
        let mut app = app(Records::fixture());
        app.handle_key(KeyLike::Down);
        assert_eq!(app.cursor(), 1);
        app.handle_key(KeyLike::Open(View::Families));
        assert_eq!(app.cursor(), 0);
        app.handle_key(KeyLike::Open(View::Scoreboard));
        assert_eq!(app.cursor(), 1);
    }

    #[test]
    fn a_number_traces_to_a_row_through_the_inspector() {
        let mut app = app(Records::fixture());
        // The best door on the scoreboard opens the first row it answered.
        app.handle_key(KeyLike::Inspect);
        assert_eq!(app.view(), View::Row);
        assert_eq!(app.records().rows[app.cursor()].door, "jev-hosted");

        app.open(View::Families);
        app.handle_key(KeyLike::End);
        app.handle_key(KeyLike::Inspect);
        assert_eq!(app.view(), View::Row);
        assert_eq!(app.records().rows[app.cursor()].family, "long-branch");
    }

    #[test]
    fn a_selection_with_no_rows_behind_it_leaves_the_inspector_alone() {
        let mut records = Records::fixture();
        records.rows.clear();
        let mut app = app(records);
        app.handle_key(KeyLike::Inspect);
        assert_eq!(app.view(), View::Scoreboard);
    }

    #[test]
    fn an_empty_chain_draws_without_panicking() {
        let app = app(Records::default());
        let text = app.to_text(80, 24);
        assert!(text.contains("gym"), "{text}");
        assert!(text.contains("chain unchecked"), "{text}");
        assert!(text.contains(DASH), "an empty header still says unknown");
    }

    #[test]
    fn every_size_draws_without_panicking() {
        let records = Records::fixture();
        for view in View::ALL {
            let mut app = app(records.clone());
            app.open(view);
            for width in [0u16, 1, 2, 7, 23, 24, 25, 60, 120, 240] {
                for height in [0u16, 1, 2, 5, 6, 7, 12, 40] {
                    let _ = app.to_text(width, height);
                }
            }
        }
    }

    #[test]
    fn a_window_too_small_says_so_rather_than_drawing_half_a_table() {
        let text = app(Records::fixture()).to_text(20, 4);
        assert!(text.contains("window too small"), "{text}");
        assert!(!text.contains('┌'), "{text}");
    }

    #[test]
    fn the_cursor_scrolls_the_window_rather_than_leaving_the_frame() {
        let mut records = Records::fixture();
        records.scores = (0..40)
            .map(|index| DoorScore {
                door: format!("door-{index:02}"),
                accuracy: Some(0.5 + f64::from(index) / 1000.0),
                scored: 10,
                ..DoorScore::default()
            })
            .collect();
        let mut app = App::new(records, ladder(Colors::None));
        app.handle_key(KeyLike::End);
        let text = app.to_text(120, 12);
        assert!(
            text.contains("door-00"),
            "the last door is on screen:\n{text}"
        );
        assert!(
            !text.contains("door-39"),
            "the window scrolled past the first door:\n{text}"
        );
    }

    #[test]
    fn a_long_cell_is_cut_with_an_ellipsis_and_never_overruns() {
        assert_eq!(clip("abcdef", 4), "abc…");
        assert_eq!(clip("abc", 4), "abc");
        assert_eq!(clip("abc", 1), "…");
        assert_eq!(clip("abc", 0), "");
        assert_eq!(left("ab", 4), "ab  ");
        assert_eq!(right("ab", 4), "  ab");
        assert_eq!(count(DASH), 1, "the em dash is one cell wide");
    }

    #[test]
    fn prose_wraps_at_the_last_space() {
        assert_eq!(wrap("the quick brown fox", 9), ["the quick", "brown fox"]);
        assert_eq!(wrap("", 9), [DASH]);
    }

    #[test]
    fn the_frame_draws_its_corners_under_the_header() {
        let text = app(Records::fixture()).to_text(40, 10);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[1].starts_with('┌'), "{text}");
        assert!(lines[1].ends_with('┐'), "{text}");
        assert!(lines[9].starts_with('└'), "{text}");
        assert!(lines[9].ends_with('┘'), "{text}");
    }

    #[test]
    fn the_rails_name_the_view_and_the_keys() {
        let text = app(Records::fixture()).to_text(120, 20);
        assert!(text.contains(" scoreboard "), "{text}");
        assert!(text.contains("q quit"), "{text}");
        assert!(text.contains(" 1/4 "), "the position rides the bottom rail");
    }
}
