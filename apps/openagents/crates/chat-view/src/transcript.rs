//! The conversation view: virtualized transcript with block-granularity rows,
//! stick-to-bottom, tool-group folding, and streaming markdown.
//!
//! Row model (docs/research/mugen-pretext.md §3):
//! - one row per BLOCK: user message = one bubble row; assistant messages split
//!   into one row per markdown top-level block, plus consecutive-tool groups
//!   (agent/spawn chips split out so they never collapse) and input/error chips;
//! - stable row ids `{msgId}#{partId}.{blockIx}` / `{msgId}#g{groupIx}` — LIVE
//!   (streaming) entries split per block exactly like completed ones (the list
//!   virtualizes them, so a fading live reply re-renders only its visible tail
//!   each frame — flat cost in the reply length); on completion each block row
//!   keeps its id, so row identity is continuous and nothing flickers;
//! - rows are cached per entry keyed by a content fingerprint — only changed
//!   messages rebuild (the anti-"streaming stutter" trick);
//! - row-set changes diff by (id, version) into one minimal `splice`.
//!
//! Stick-to-bottom is a velocity spring (mugen §1e, the same shape as
//! stackblitz's use-stick-to-bottom): while pinned, a per-frame stepper glides
//! the viewport toward the list end with a feed-forward term tracking the
//! smoothed target growth, so 120ms doc commits read as a continuous glide
//! instead of per-commit snaps. The pin breaks only on user input (the list's
//! scroll handler fires exclusively from its wheel/touch path) and re-engages
//! inside the 70px band; the first send in an empty chat anchors the prompt at
//! the viewport top and hands off to the same glide when the reply overflows.
//! Wheel/touch releases that anchor immediately, including when background
//! streaming has advanced beyond the last measured frame.

pub mod host;
pub mod media;

pub use self::media::TranscriptMedia;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, Weak};
use std::time::Duration;

use web_time::Instant;

use gpui::{
    AnyElement, Bounds, ClipboardItem, ContentMask, Context, ListAlignment, ListOffset,
    ListScrollEvent, ListState, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    SharedString, StyledText, Subscription, Task, TextAlign, TextRun, Window, canvas, div, list,
    point, prelude::*, px, size,
};

#[cfg(not(target_family = "wasm"))]
use gpui::PathBuilder;

use openagents_doc::{
    MessagePart, MessageRole, MessageStatus, SessionMessageEntry, SubagentStatus,
};
use openagents_proto::ToolCall;

use self::host::TranscriptHost;
use crate::markdown::parser::{
    Block, BlockTree, IncrementalParser, InlineRun, InlineStyle, parse_full,
};
use crate::markdown::render::{self, RenderCache, RenderOptions};
use crate::markdown::veil::RowVeil;
use crate::motion::{self, AnimationExt as _};
use crate::notice::{NoticeChipIcon::Tile, notice_chip};
use crate::syntax_cache::{DocumentHighlightKey, SyntaxHighlightCache};
use crate::theme::Theme;
use openagents_syntax::LanguageId as Lang;

/// How a host lays a transcript out when it has none of the desktop's
/// window chrome (a web page's chat).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostLayout {
    /// The first row's gap.
    pub top: f32,
    /// The gap under the last row.
    pub bottom: f32,
    /// Each side's gutter around the column.
    pub side: f32,
    /// Size to the rows, up to the room the host gives, instead of filling it.
    pub fit_content: bool,
    /// Set each reply on a plate of its own, for a host that draws the
    /// transcript straight over a busy scene with no card behind it (the
    /// Grid homepage, #12459). A reply's rows share one plate.
    pub plates: bool,
}

/// A reply's plate when [`HostLayout::plates`] is set: dark enough to read
/// over the Grid's lines, light enough to show them through.
pub fn reply_plate_bg() -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 0.02, 0.62)
}

/// The person's own message on a host with plates: a little lighter than a
/// reply's, so the two still read apart.
pub fn user_plate_bg() -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 0.16, 0.8)
}

/// A reply plate's inset around its text.
const PLATE_PAD_X: f32 = 14.0;
const PLATE_PAD_Y: f32 = 10.0;

/// The engine method that returns a tool's full output or diff.
pub const FETCH_TOOL_BLOB: &str = "FetchToolBlob";

/// Open the chat a fork was made from: the "Forked from" link in a fork's
/// transcript. The app's shell handles it.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = shell, no_json)]
pub struct OpenForkSource(pub String);

// ---------------------------------------------------------------------------
// Constants (mugen ports)
// ---------------------------------------------------------------------------

/// Re-engage the bottom pin when the user returns within this many px of the end.
pub const STICK_THRESHOLD_PX: f32 = 70.0;
/// List overdraw beyond the viewport.
pub const OVERDRAW_PX: f32 = 320.0;
/// Show the scroll-to-bottom button beyond this distance from the end.
pub const SCROLL_BUTTON_THRESHOLD_PX: f32 = 320.0;

pub fn jump_visibility(was_shown: bool, distance: f32) -> bool {
    // Once offered, keep the control until close to the end. A single 320px
    // threshold made it disappear halfway through a downward scroll gesture.
    distance
        > if was_shown {
            AT_BOTTOM_PX
        } else {
            SCROLL_BUTTON_THRESHOLD_PX
        }
}
/// Bound session-local viewport memory independently of total chat history.
pub const MAX_SAVED_VIEWPORTS: usize = 256;
/// Bound locally-authored queue ids waiting to become transcript prompts.
pub const MAX_PENDING_QUEUED_TURNS: usize = 256;
/// Text-selection edge scrolling runs only during a drag. A 24 ms cadence is
/// smooth enough to track text while avoiding a permanent animation-frame loop
/// on low-end devices.
pub const SELECTION_SCROLL_TICK_MS: u64 = 24;
pub const SELECTION_SCROLL_EDGE_PX: f32 = 36.0;
pub const SELECTION_SCROLL_MAX_STEP_PX: f32 = 24.0;
/// Activity row height / gap — analytic, so fold heights need no measurement.
/// Ordinary tools place their icon on the rail; subagents retain a 30px card.
/// Rows stack without a gap so the rail continues alongside expanded output.
pub const CHIP_HEIGHT: f32 = 38.0;
pub const CHIP_GAP: f32 = 0.0;
pub const CHIP_CARD_HEIGHT: f32 = 30.0;
/// Inner height of the chip header: [`CHIP_CARD_HEIGHT`] is the card's
/// border-box (explicit `h` in gpui includes the 1px border), so a 30px
/// header inside a 30px bordered card clips 2px off the bottom and every
/// glyph/icon reads high (user report).
pub const CHIP_HEADER_HEIGHT: f32 = CHIP_CARD_HEIGHT - 2.0;
/// Child labels and expanded text share one edge beneath the compact summary.
/// The child gutter reserves room for
/// a longer elbow, a 4px break before the icon, and an 8px icon-to-text gap.
pub const ACTIVITY_GUTTER_WIDTH: f32 = 48.0;
pub const ACTIVITY_TEXT_GAP: f32 = 8.0;
pub const ACTIVITY_TRUNK_X: f32 = 12.5;
pub const ACTIVITY_BEND_RADIUS: f32 = 6.0;
pub const ACTIVITY_BRANCH_END_X: f32 = 28.0;
pub const ACTIVITY_ICON_LEFT: f32 = 32.0;
pub const ACTIVITY_ICON_SIZE: f32 = 16.0;
pub const TOOL_TEXT_SIZE: f32 = 12.0;
pub const TOOL_LABEL_SIZE: f32 = TOOL_TEXT_SIZE;
pub const TOOL_LABEL_LINE_HEIGHT: f32 = 18.0;
pub const TOOL_GROUP_HEADER_HEIGHT: f32 = 26.0;
/// Compact rows retain the analytic heights used by row and group folds.
pub const TOOL_TREE_ROW_HEIGHT: f32 = 32.0;
pub const TOOL_FOLD: motion::MotionSpec = motion::MotionSpec::new(140, motion::EASE_OUT);
/// BoardUI task-list cadence: a slow light sweep keeps the active summary
/// legible, while each newly appended row reveals quickly enough to read as a
/// continuous log rather than a stack of discrete pop-ins.
pub const TOOL_GROUP_SHIMMER_DURATION: Duration = Duration::from_millis(3_400);
pub const TOOL_GROUP_SHIMMER_HALF_WIDTH: f32 = 0.36;
pub const TOOL_GROUP_SHIMMER_STRIP_WIDTH: f32 = 2.0;
pub const TOOL_ROW_REVEAL: motion::MotionSpec = motion::MotionSpec::new(360, motion::EASE_OUT_EXPO);
/// The connector draws briskly, then eases into the branch tip so its arrival
/// remains visible without feeling mechanically linear.
pub const TOOL_CONNECTOR_REVEAL: motion::MotionSpec =
    motion::MotionSpec::new(480, motion::EASE_OUT_QUINT);
pub const TOOL_FIRST_ROW_DELAY_MS: u64 = 90;
pub const TOOL_ROW_STAGGER_MS: u64 = 65;

/// Signed list scroll step for a pointer near a viewport edge.
///
/// GPUI list offsets increase toward the document bottom. The quadratic ramp
/// keeps entry into the edge zone gentle and reaches full speed at the edge.
pub fn selection_scroll_step(bounds: Bounds<Pixels>, position: Point<Pixels>) -> f32 {
    let height = f32::from(bounds.size.height);
    if height <= 0.0 {
        return 0.0;
    }
    let edge = SELECTION_SCROLL_EDGE_PX.min(height / 3.0);
    if edge <= 0.0 {
        return 0.0;
    }
    let y = f32::from(position.y);
    let top = f32::from(bounds.top());
    let bottom = f32::from(bounds.bottom());
    let scaled = |penetration: f32| {
        let t = (penetration / edge).clamp(0.0, 1.0);
        SELECTION_SCROLL_MAX_STEP_PX * t * t
    };
    if y < top + edge {
        -scaled(top + edge - y)
    } else if y > bottom - edge {
        scaled(y - (bottom - edge))
    } else {
        0.0
    }
}
pub const CHIPS_TOP_PAD: f32 = 2.0;
/// How long a user fold toggle keeps its height tween armed: the RESIZE
/// spec's 200ms plus margin. Past this the fold renders statically — an armed
/// tween replays on remount, i.e. on every scroll-back-into-view.
pub const FOLD_TWEEN_WINDOW: std::time::Duration = std::time::Duration::from_millis(400);
/// A user prompt renders at most this many wrapped lines until expanded. A
/// pasted log or file drops into the transcript as one endless slab otherwise
/// (user report) — past the cap the bubble clips and grows a chevron.
pub const USER_COLLAPSED_LINES: usize = 5;
/// The user bubble's line box.
pub const USER_LINE_HEIGHT: f32 = 22.0;
/// Conservative first-frame soft-wrap proxy for the fixed-width long-prompt
/// bubble. The final decision uses the wrapped `StyledText` layout, but this
/// fallback lets clearly long prompts render their affordance immediately
/// before that first layout has completed.
pub const USER_COLLAPSE_CHARS: usize = 400;
/// Vertical separation before the plain expand/collapse link.
pub const USER_TOGGLE_GAP: f32 = 8.0;
/// User-bubble attachment thumbnails (user-attachments.tsx): 112×80 thumbs in
/// a wrapping strip. Fixed thumbnail sizes keep load-state flips from
/// shifting the virtualizer.
pub const ATT_THUMB_W: f32 = 112.0;
pub const ATT_THUMB_H: f32 = 80.0;

// ---------------------------------------------------------------------------
// Stick-to-bottom spring (mugen §1e — same constants as its DEFAULT_SPRING,
// which follows the shape of stackblitz/use-stick-to-bottom)
// ---------------------------------------------------------------------------

/// Retains velocity frame-to-frame (higher = more glide).
pub const SPRING_DAMPING: f32 = 0.7;
/// Pull toward the target (higher = snappier).
pub const SPRING_STIFFNESS: f32 = 0.05;
/// Inertia (higher = slower to start/stop).
pub const SPRING_MASS: f32 = 1.25;
/// Reference frame for the fixed-timestep integration (60fps).
pub const SPRING_FRAME_MS: f32 = 1000.0 / 60.0;
/// Cap on simulated frames per tick — a hitch catches up instead of teleporting.
pub const SPRING_MAX_CATCHUP_FRAMES: f32 = 8.0;
/// EMA rate for the feed-forward target-growth estimate.
pub const SPRING_GROWTH_EMA: f32 = 0.12;
/// While streaming, chase up to this many px above the true bottom (keeps the
/// growing tail visible instead of hugging a moving edge).
pub const SPRING_CHASE_MAX_LEAD: f32 = 32.0;
/// Treat as exactly pinned within this distance of the bottom.
pub const AT_BOTTOM_PX: f32 = 2.0;

/// A live stream already resting at the end should keep that end anchored as
/// its measured height grows. This is deliberately narrower than `pinned`:
/// users gliding back toward the bottom keep the normal spring behavior.
pub fn should_anchor_live_stream(pinned: bool, distance_from_bottom: f32, streaming: bool) -> bool {
    pinned && streaming && distance_from_bottom <= AT_BOTTOM_PX
}

/// Retain the spring's state this long after landing, so a streaming pause
/// resumes at cruise. Retaining state does not require drawing idle frames.
pub const SPRING_SETTLE_GRACE_MS: u64 = 500;
/// Teleport when farther than this many viewports from the end; glide the rest.
pub const GLIDE_MAX_VIEWPORTS: f32 = 2.5;
/// A freshly-sent prompt rests this far below the transcript viewport's top.
/// The titlebar overlays the full-height list, so its height is part of the
/// inset; the extra 10px matches the first row's breathing room.
pub const OWN_SEND_TOP_INSET_PX: f32 = Theme::TITLEBAR_HEIGHT + 10.0;
/// Epsilon of extra height under the reservation. The runway ends AT the
/// app's bottom — this is not scroll room (24px of it read as a janky
/// overshoot-and-fight zone, user report) — it exists only to keep the held
/// layout out of gpui's shorter-than-viewport regime, where a bottom-aligned
/// list reports no item bounds (sizing goes blind) and position becomes a
/// function of content height instead of the hold. Two pixels of travel is
/// below perception.
pub const OWN_SEND_SCROLL_SLACK_PX: f32 = 2.0;
/// Per-60fps-frame fraction of the remaining entry glide retained (~90%
/// covered in ~230ms, ease-out).
pub const OWN_SEND_GLIDE_RETAIN: f32 = 0.85;
/// The entry glide snaps to the absolute hold within this error.
pub const OWN_SEND_GLIDE_SNAP_PX: f32 = 1.0;

/// A bounds-free guard for gliding through rows whose heights are still
/// being measured. The provisional reservation is never a scroll target.
pub fn own_turn_glide_crossed(offset: ListOffset, anchor_ix: usize, inset: f32) -> bool {
    offset.item_ix > anchor_ix
        || (offset.item_ix == anchor_ix && f32::from(offset.offset_in_item) > -inset)
}

/// Pure stick-to-bottom spring stepper — the mugen `tick()` integration:
/// velocity relaxes toward `(damping·v + stiffness·diff)/mass` per 60fps
/// sub-frame, position advances by `v + target_vel` where `target_vel` is a
/// feed-forward EMA of target growth px/frame, and the chase point sits up to
/// [`SPRING_CHASE_MAX_LEAD`] px above the true bottom proportional to growth.
#[derive(Debug, Clone, Copy)]
pub struct StickSpring {
    /// Spring velocity, px per 60fps frame.
    pub velocity: f32,
    /// Feed-forward: smoothed target growth, px per 60fps frame.
    pub target_vel: f32,
    /// Target observed at the previous tick (`None` = fresh/parked).
    pub last_target: Option<f32>,
}

impl Default for StickSpring {
    fn default() -> Self {
        Self::new()
    }
}

impl StickSpring {
    pub fn new() -> Self {
        Self {
            velocity: 0.0,
            target_vel: 0.0,
            last_target: None,
        }
    }

    /// Park the spring (drops all state; the next tick starts cold).
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Residual motion below mugen's settle thresholds (`v < .05 && targetVel
    /// < .05`)?
    pub fn is_idle(&self) -> bool {
        self.velocity < 0.05 && self.target_vel < 0.05
    }

    pub fn needs_frame(distance: f32) -> bool {
        // The spring is clamped to the target. Residual velocity cannot move
        // a viewport already there; virtual-list height estimates can keep
        // that velocity nonzero indefinitely even after a turn completes.
        distance > 0.5
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn target_vel(&self) -> f32 {
        self.target_vel
    }

    /// Advance one tick. `pos`/`target` are scroll offsets in px (larger =
    /// closer to the bottom); `frames` is elapsed time in 60fps frames
    /// (clamped by the caller to [`SPRING_MAX_CATCHUP_FRAMES`]). Returns the
    /// new position: never overshoots `target`, monotone while approaching,
    /// and snaps exactly once within 0.5px.
    pub fn step(&mut self, mut pos: f32, target: f32, mut frames: f32) -> f32 {
        let grew = self.last_target.map_or(0.0, |last| target - last);
        self.last_target = Some(target);
        if grew < -1.0 {
            // Target shrank (row collapse/removal) — growth estimate is stale.
            self.target_vel = 0.0;
        } else {
            let observed = grew.max(0.0) / frames.max(0.25);
            self.target_vel += SPRING_GROWTH_EMA * (observed - self.target_vel);
        }
        let chase = target - (self.target_vel * 9.0).min(SPRING_CHASE_MAX_LEAD);
        let mut v = self.velocity;
        while frames > 0.0 {
            let h = frames.min(1.0);
            frames -= h;
            let diff = (chase - pos).max(0.0);
            v += h * ((SPRING_DAMPING * v + SPRING_STIFFNESS * diff) / SPRING_MASS - v);
            pos = (pos + (v + self.target_vel) * h).min(target);
        }
        self.velocity = v;
        if target - pos <= 0.5 { target } else { pos }
    }
}

// ---------------------------------------------------------------------------
// Row model (pure)
// ---------------------------------------------------------------------------

/// What a chip inside a [`RowKind::ToolGroup`] represents. `Call` items come
/// from doc tool parts; `Thought` (reasoning) and `Note` (a text part folded
/// by compact mode) are UI-synthesized in [`rows_for_entry`] — the doc never
/// emits either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolItemKind {
    Call,
    Thought,
    Note,
}

/// One tool invocation inside a group row.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolItem {
    /// Stable document part identity, independent of arrival order.
    pub part_id: String,
    pub call: ToolCall,
    pub is_error: bool,
    pub resolved: bool,
    /// Expandable detail: a code-block of output lines, or a real diff
    /// section rendered by the changes pane's component (ACP harnesses).
    /// Precomputed here because rows are cached by fingerprint — diffing and
    /// tokenizing per paint would run on every scroll frame.
    pub detail: Option<Arc<ToolDetail>>,
    /// Expandable full-invocation block: the complete tool call (whole
    /// command / pattern / URL / input JSON) that the chip header collapses
    /// to one truncated line. Rendered above `detail` in the open card.
    /// Precomputed for the same reason as `detail`.
    pub invocation: Option<Arc<ToolDetail>>,
    /// Sidecar key of the full output (chat2-sync A3) — the doc carries only
    /// a one-line summary; expanding offers a lazy "Show full output" fetch.
    pub output_ref: Option<SharedString>,
    /// Full-output size, for the affordance label ("Show full output (12 KB)").
    pub output_bytes: Option<u64>,
    /// Sidecar key of the full diff (doc carries only per-file stats).
    pub diff_ref: Option<SharedString>,
    /// The spawned SUBAGENT's doc id — the chip IS the index (there is no
    /// listing endpoint); with it the chip offers "Open subagent".
    pub subagent_ref: Option<SharedString>,
    /// Subagent lifecycle, distinct from `resolved` (eager-done: the spawn
    /// tool's own result lands while the subagent still runs).
    pub subagent_status: Option<SubagentStatus>,
    /// One-line live tail — LEGACY docs only (new runs stopped folding it;
    /// per-delta header rewrites read as noise). Never rendered; still
    /// fingerprinted so an old doc's chips re-splice correctly.
    pub subagent_tail: Option<SharedString>,
    /// `Call` is a real doc tool invocation; `Thought` (a reasoning part
    /// riding the tool group — the thought process belongs inside the
    /// combined "Ran N commands" accordion, opening/closing with the same
    /// tween) and `Note` (compact mode's folded narration) are synthesized
    /// in [`rows_for_entry`], never from a doc tool part. A `Thought`'s text
    /// is the `detail`; `resolved == false` means it is still streaming (the
    /// chip then defaults open).
    pub kind: ToolItemKind,
    /// Image files the call read, wrote or printed (as reported; relative
    /// paths resolve against the chat cwd). Expanding the chip previews them.
    pub images: Arc<[SharedString]>,
}

/// Subagent spawn chips — [`ToolCall::is_subagent_spawn`], the shared genus
/// every driver decodes its spawn tool into. These stay out of the
/// collapsible "Called N tools" wrap so a running subagent is visible
/// without opening the fold.
pub fn is_agent_call(call: &ToolCall) -> bool {
    call.is_subagent_spawn()
}

/// The chip's GENUS is the call itself, never the ref: docs written before
/// the claude-driver fix carry stray `subagent_ref`s on ordinary Run chips
/// (a background shell's `task_notification` was mis-tagged as subagent
/// traffic), and honoring the ref alone turned those Runs into spawn chips
/// that opened empty, never-created subagent docs.
pub fn is_agent_tool(item: &ToolItem) -> bool {
    is_agent_call(&item.call)
}

/// A chip renders as the spawn LINK (whole-card click → subagent tab) only
/// when an agent call has actually been bound to its doc.
pub fn is_spawn_link(item: &ToolItem) -> bool {
    is_agent_call(&item.call) && item.subagent_ref.is_some()
}

/// Ordinary tool groups fold behind a summary header; agent/spawn chips
/// render as their own always-open row.
pub fn tool_group_collapses(tools: &[ToolItem]) -> bool {
    tools.iter().any(|t| !is_agent_tool(t))
}

/// Column budget for soft-wrapping thought text into detail lines. The
/// detail body is preformatted (no element wrapping), so the wrap happens
/// here — conservative enough to fit the card at typical transcript widths.
pub const THOUGHT_WRAP_COLS: usize = 96;

/// Flatten a thought's parsed markdown into wrapped, STYLED detail lines —
/// inline markers render as real styling (bold/italic/code/links) instead of
/// literal `**` glyphs; blocks flatten structurally (headings bold, list
/// bullets, quote bars, verbatim code lines). Every line is one fixed-height
/// row, so the detail height stays analytic (lines × [`OUTPUT_LINE_HEIGHT`])
/// and the group's fold tween keeps working without measurement.
pub fn thought_lines(tree: &BlockTree) -> Vec<Vec<InlineRun>> {
    let mut out: Vec<Vec<InlineRun>> = Vec::new();
    for top in &tree.blocks {
        if !out.is_empty() {
            // One blank separator row between top-level blocks (the old
            // plain-text wrap kept paragraph gaps the same way).
            out.push(Vec::new());
        }
        thought_block_lines(&top.block, 0, &mut out);
    }
    while out
        .last()
        .is_some_and(|l| l.iter().all(|r| r.text.trim().is_empty()))
    {
        out.pop();
    }
    out
}

/// The slot-0 indent run every emitted thought line opens with (possibly
/// empty). List/quote handlers rewrite it in place to plant markers/bars, so
/// it must exist even at zero indent.
pub fn indent_run(indent: usize) -> Vec<InlineRun> {
    vec![InlineRun {
        text: " ".repeat(indent),
        style: InlineStyle::default(),
    }]
}

/// Append text to a line's run list, merging into the tail run when styles
/// match (keeps run counts small for the shaper).
pub fn push_styled(line: &mut Vec<InlineRun>, text: &str, style: &InlineStyle) {
    if text.is_empty() {
        return;
    }
    match line.last_mut() {
        Some(last) if last.style == *style => last.text.push_str(text),
        _ => line.push(InlineRun {
            text: text.to_owned(),
            style: style.clone(),
        }),
    }
}

/// Close a wrapped line: the slot-0 indent run in front (see [`indent_run`]).
pub fn finish_line(indent: usize, mut line: Vec<InlineRun>) -> Vec<InlineRun> {
    let mut full = indent_run(indent);
    full.append(&mut line);
    full
}

/// Word-wrap styled runs at the thought column budget. Char-counted like
/// every detail wrap — block heights must stay analytic — with words glued
/// across style boundaries (`**bold**tail` wraps as one unit), separator
/// spaces riding the preceding run, and pathological overlong tokens
/// hard-split at the budget. Hard breaks (`\n` runs) split into
/// separately-wrapped segments.
pub fn wrap_styled_runs(runs: &[InlineRun], indent: usize, out: &mut Vec<Vec<InlineRun>>) {
    let budget = THOUGHT_WRAP_COLS.saturating_sub(indent).max(16);
    let mut segments: Vec<Vec<InlineRun>> = vec![Vec::new()];
    for run in runs {
        for (ix, piece) in run.text.split('\n').enumerate() {
            if ix > 0 {
                segments.push(Vec::new());
            }
            if !piece.is_empty() {
                segments.last_mut().unwrap().push(InlineRun {
                    text: piece.to_owned(),
                    style: run.style.clone(),
                });
            }
        }
    }
    for segment in segments {
        // Tokens: maximal non-whitespace piece lists, glued across run
        // boundaries so a word split by styling never wraps mid-word.
        let mut tokens: Vec<Vec<InlineRun>> = Vec::new();
        let mut in_token = false;
        for run in &segment {
            let text = run.text.as_str();
            let mut pos = 0;
            while pos < text.len() {
                let rest = &text[pos..];
                let ws = rest.chars().next().is_some_and(char::is_whitespace);
                let end = rest
                    .char_indices()
                    .find(|(_, c)| c.is_whitespace() != ws)
                    .map_or(text.len(), |(i, _)| pos + i);
                if ws {
                    in_token = false;
                } else {
                    if !in_token {
                        tokens.push(Vec::new());
                        in_token = true;
                    }
                    push_styled(tokens.last_mut().unwrap(), &text[pos..end], &run.style);
                }
                pos = end;
            }
        }
        let mut line: Vec<InlineRun> = Vec::new();
        let mut len = 0usize;
        for token in tokens {
            let tok_len: usize = token.iter().map(|r| r.text.chars().count()).sum();
            if tok_len > budget {
                // Hard-split a pathological token at the budget.
                if len > 0 {
                    out.push(finish_line(indent, std::mem::take(&mut line)));
                    len = 0;
                }
                for piece in token {
                    let mut chars = piece.text.chars();
                    loop {
                        let chunk: String = chars.by_ref().take(budget - len).collect();
                        if chunk.is_empty() {
                            break;
                        }
                        len += chunk.chars().count();
                        push_styled(&mut line, &chunk, &piece.style);
                        if len == budget {
                            out.push(finish_line(indent, std::mem::take(&mut line)));
                            len = 0;
                        }
                    }
                }
                continue;
            }
            if len > 0 && len + 1 + tok_len > budget {
                out.push(finish_line(indent, std::mem::take(&mut line)));
                len = 0;
            }
            if len > 0 {
                if let Some(last) = line.last_mut() {
                    last.text.push(' ');
                }
                len += 1;
            }
            for piece in token {
                push_styled(&mut line, &piece.text, &piece.style);
            }
            len += tok_len;
        }
        if len > 0 {
            out.push(finish_line(indent, line));
        }
    }
}

/// One markdown block into thought detail lines, `indent` spaces deep.
pub fn thought_block_lines(block: &Block, indent: usize, out: &mut Vec<Vec<InlineRun>>) {
    match block {
        Block::Paragraph { runs } => wrap_styled_runs(runs, indent, out),
        Block::Heading { runs, .. } => {
            // Headings keep the detail's single type size — bold is the cue
            // (an 18px line box can't host display sizes).
            let bold: Vec<InlineRun> = runs
                .iter()
                .map(|r| {
                    let mut r = r.clone();
                    r.style.bold = true;
                    r
                })
                .collect();
            wrap_styled_runs(&bold, indent, out);
        }
        Block::CodeBlock { code, .. } => {
            let style = InlineStyle {
                code: true,
                ..InlineStyle::default()
            };
            for line in code.lines() {
                for chunk in wrap_cols(line, THOUGHT_WRAP_COLS.saturating_sub(indent).max(16)) {
                    let mut row = indent_run(indent);
                    if !chunk.is_empty() {
                        row.push(InlineRun {
                            text: chunk.to_string(),
                            style: style.clone(),
                        });
                    }
                    out.push(row);
                }
            }
        }
        Block::List {
            ordered_start,
            items,
        } => {
            // Tight rendering: no blank rows inside a list.
            for (ix, item) in items.iter().enumerate() {
                let marker = match ordered_start {
                    Some(start) => format!("{}. ", start + ix as u64),
                    None => "• ".to_string(),
                };
                let inner = indent + marker.chars().count();
                let mark = out.len();
                for child in item {
                    thought_block_lines(child, inner, out);
                }
                if out.len() == mark {
                    // An empty item still shows its marker.
                    out.push(indent_run(inner));
                }
                // The item's first line trades its indent spaces for the
                // marker (the slot-0 run is always the indent).
                if let Some(first) = out[mark].first_mut() {
                    first.text = format!("{}{marker}", " ".repeat(indent));
                }
            }
        }
        Block::BlockQuote { children } => {
            let mark = out.len();
            for (ix, child) in children.iter().enumerate() {
                if ix > 0 {
                    out.push(Vec::new());
                }
                thought_block_lines(child, indent + 2, out);
            }
            // Trade the two quote-indent spaces for the bar on every quoted
            // line — replace, not overwrite: nested list handlers already
            // planted markers after their own deeper indent.
            for line in &mut out[mark..] {
                if let Some(first) = line.first_mut()
                    && first.text.len() >= indent + 2
                {
                    first.text.replace_range(indent..indent + 2, "│ ");
                }
            }
        }
        Block::Table { header, rows, .. } => {
            // A thought is a record, not a layout surface: cells joined with
            // a dot separator, header bold — no column machinery.
            let join = |cells: &[Vec<InlineRun>], bold: bool| -> Vec<InlineRun> {
                let mut line: Vec<InlineRun> = Vec::new();
                for (ix, cell) in cells.iter().enumerate() {
                    if ix > 0 {
                        push_styled(&mut line, " · ", &InlineStyle::default());
                    }
                    for r in cell {
                        let mut r = r.clone();
                        r.style.bold |= bold;
                        line.push(r);
                    }
                }
                line
            };
            wrap_styled_runs(&join(header, true), indent, out);
            for row in rows {
                wrap_styled_runs(&join(row, false), indent, out);
            }
        }
        Block::Rule => {
            let mut row = indent_run(indent);
            row.push(InlineRun {
                text: "———".into(),
                style: InlineStyle::default(),
            });
            out.push(row);
        }
    }
}

/// A reasoning part as a tool-group chip: "Thought process" header over the
/// thought's markdown flattened into styled detail lines (analytic height —
/// the group's fold tween needs it; see [`thought_lines`]). Capped like tool
/// outputs, with the counted tail. `live` = the part is still streaming
/// (chip defaults open).
pub fn thought_item(part_id: &str, tree: &BlockTree, live: bool) -> ToolItem {
    let mut lines = thought_lines(tree);
    let truncated_by = lines.len().saturating_sub(OUTPUT_DETAIL_MAX_LINES);
    if truncated_by > 0 {
        // Keep the TAIL while streaming (the fresh thinking is the signal);
        // settled thoughts keep the head like tool outputs do.
        if live {
            lines.drain(..truncated_by);
            // The cut can land on a block separator — drop the orphan blank.
            while lines
                .first()
                .is_some_and(|l| l.iter().all(|r| r.text.trim().is_empty()))
            {
                lines.remove(0);
            }
        } else {
            lines.truncate(OUTPUT_DETAIL_MAX_LINES);
        }
    }
    ToolItem {
        part_id: part_id.into(),
        call: ToolCall::Unknown {
            name: "Thought process".into(),
            input: None,
        },
        is_error: false,
        resolved: !live,
        detail: (!lines.is_empty()).then(|| {
            Arc::new(ToolDetail::Thought {
                lines,
                truncated_by,
            })
        }),
        invocation: None,
        output_ref: None,
        output_bytes: None,
        diff_ref: None,
        subagent_ref: None,
        subagent_status: None,
        subagent_tail: None,
        kind: ToolItemKind::Thought,
        images: Arc::new([]),
    }
}

/// A folded assistant TEXT part as a tool-group chip (compact mode): the
/// narration between work steps collapses into the turn's single accordion
/// as a "Wrote" chip whose detail is the text's markdown flattened exactly
/// like a thought's. `live` = the part is the streaming tail — unresolved,
/// so the collapsed header keeps its working shimmer until the reply lands.
pub fn note_item(part_id: &str, tree: &BlockTree, live: bool) -> ToolItem {
    let mut item = thought_item(part_id, tree, live);
    item.call = ToolCall::Unknown {
        name: "Note".into(),
        input: None,
    };
    item.kind = ToolItemKind::Note;
    item
}

/// A chip's expandable detail payload.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolDetail {
    /// Command/tool output as a code block: verbatim lines (indentation
    /// intact), capped at [`OUTPUT_DETAIL_MAX_LINES`] with a counted tail.
    Output {
        lines: Vec<SharedString>,
        truncated_by: usize,
    },
    /// A thought's markdown, pre-flattened into wrapped STYLED lines — one
    /// fixed-height row each, so the height stays analytic like `Output`
    /// while inline markers render as real styling ([`thought_lines`]).
    Thought {
        lines: Vec<Vec<InlineRun>>,
        truncated_by: usize,
    },
    /// A file diff, in the changes pane's model: hunks with 3 lines of
    /// context, dual line numbers, and (for recognized languages) syntax
    /// tokens — rendered by `changes::render_file_body`.
    Diff {
        file: Arc<crate::diff::FileDiff>,
        old_text: Option<Arc<str>>,
        new_text: Option<Arc<str>>,
    },
    /// Per-file `+N −N` stat rows — what the thin doc keeps of an edit
    /// (chat2-sync A1). The full diff upgrades this to [`ToolDetail::Diff`]
    /// via the sidecar fetch.
    Stats {
        stats: Arc<Vec<openagents_doc::ToolDiffStat>>,
    },
}

/// Max verbatim output lines per chip before the counted tail row.
pub const OUTPUT_DETAIL_MAX_LINES: usize = 24;

/// Max diff lines an inline tool-diff detail renders — the detail is one
/// stacked element inside its transcript row, so it must stay bounded
/// (~600 lines ≈ 12.6k px, several screens of context before the cut).
pub const DIFF_DETAIL_MAX_LINES: usize = 600;

/// Per-line height of an output detail block (diff blocks use the changes
/// pane's own [`crate::diff::DIFF_LINE_HEIGHT`]).
pub const OUTPUT_LINE_HEIGHT: f32 = 18.0;

/// Vertical padding of an output detail body (py(6) × 2).
pub const OUTPUT_BODY_PAD: f32 = 12.0;

/// The hairline between an expanded chip's header row and its detail body.
pub const DETAIL_SEPARATOR: f32 = 1.0;

/// Build a tool part's expandable detail. A diff wins over raw output (it is
/// the more structured record of the same action); post-strip docs carry diff
/// STATS instead of inline diff text, which win the same way.
pub fn tool_detail(
    output: Option<&str>,
    diff: Option<&openagents_proto::ToolDiff>,
    diff_stats: Option<&[openagents_doc::ToolDiffStat]>,
) -> Option<ToolDetail> {
    if let Some(diff) = diff {
        let mut file = diff_to_file(diff);
        if file.hunks.is_empty() {
            return None;
        }
        // A transcript diff renders as one stacked element inside its row —
        // cap it so a whole-file rewrite (or fetched full-diff blob) can't
        // build tens of thousands of elements per frame. The changes pane
        // has no such cap; it virtualizes per line.
        crate::diff::truncate_file_lines(&mut file, DIFF_DETAIL_MAX_LINES);
        return Some(ToolDetail::Diff {
            file: Arc::new(file),
            old_text: diff.old_text.as_deref().map(Arc::from),
            new_text: Some(Arc::from(diff.new_text.as_str())),
        });
    }
    if let Some(stats) = diff_stats.filter(|s| !s.is_empty()) {
        return Some(ToolDetail::Stats {
            stats: Arc::new(stats.to_vec()),
        });
    }
    let output = output?;
    let mut lines: Vec<SharedString> = output.lines().map(output_line).collect();
    // Trim trailing blank output lines so the block hugs its content.
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return None;
    }
    let truncated_by = lines.len().saturating_sub(OUTPUT_DETAIL_MAX_LINES);
    lines.truncate(OUTPUT_DETAIL_MAX_LINES);
    Some(ToolDetail::Output {
        lines,
        truncated_by,
    })
}

/// Chars kept per output line. Lines clip rather than wrap, but each one is
/// still shaped whole for selection — a minified megabyte must not be.
pub const OUTPUT_LINE_MAX_CHARS: usize = 8 * 1024;

/// One verbatim output line, cut to [`OUTPUT_LINE_MAX_CHARS`] with an ellipsis.
pub fn output_line(line: &str) -> SharedString {
    match line.char_indices().nth(OUTPUT_LINE_MAX_CHARS) {
        Some((cut, _)) => SharedString::from(format!("{}…", &line[..cut])),
        None => SharedString::from(line.to_owned()),
    }
}

/// Columns at which an invocation line soft-wraps into continuation lines.
/// The wrap is char-counted, not measured — block heights must be analytic —
/// so the budget is sized to fit the narrowest useful transcript pane.
pub const CALL_WRAP_COLS: usize = 80;

/// Soft-wrap one raw line into [`CALL_WRAP_COLS`]-char chunks so a long
/// single-line command stays fully readable instead of ellipsizing.
pub fn wrap_cols(line: &str, cols: usize) -> Vec<SharedString> {
    if line.chars().count() <= cols {
        return vec![SharedString::from(line.to_owned())];
    }
    line.chars()
        .collect::<Vec<_>>()
        .chunks(cols)
        .map(|chunk| SharedString::from(chunk.iter().collect::<String>()))
        .collect()
}

/// Build a chip's full-invocation block — the complete tool call the header
/// truncates to one line: the whole command, pattern, or URL, todo items one
/// per line, MCP/unknown input as pretty-printed JSON. Reuses the output
/// code-block payload so rendering and height stay one implementation.
pub fn call_block(call: &ToolCall) -> Option<ToolDetail> {
    let text: String = match call {
        ToolCall::Exec { command } => command.clone(),
        ToolCall::ReadFile { path } => path.clone(),
        ToolCall::WriteFile { path, content } => match content {
            Some(content) => format!("{path}\n{content}"),
            None => path.clone(),
        },
        ToolCall::EditFile { path, .. } => path.clone(),
        ToolCall::ApplyPatch { path } => path.clone().unwrap_or_else(|| "workspace".into()),
        ToolCall::Search { pattern, path } => match path {
            Some(path) => format!("{pattern} in {path}"),
            None => pattern.clone(),
        },
        ToolCall::Glob { pattern } => pattern.clone(),
        ToolCall::WebFetch { url, prompt } => match prompt {
            Some(prompt) => format!("{url}\n{prompt}"),
            None => url.clone(),
        },
        ToolCall::WebSearch { query } => query.clone(),
        ToolCall::Todo { items } => items
            .iter()
            .map(|i| {
                let mark = match i.status() {
                    openagents_proto::TodoStatus::Completed => "[x]",
                    openagents_proto::TodoStatus::InProgress => "[~]",
                    openagents_proto::TodoStatus::Pending => "[ ]",
                };
                format!("{mark} {}", i.text)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        ToolCall::Mcp {
            server,
            tool,
            input,
        } => match input {
            Some(input) => format!(
                "{server} · {tool}\n{}",
                serde_json::to_string_pretty(input).unwrap_or_default()
            ),
            None => format!("{server} · {tool}"),
        },
        ToolCall::Unknown { name, input } => match input {
            Some(input) => format!(
                "{name}\n{}",
                serde_json::to_string_pretty(input).unwrap_or_default()
            ),
            None => name.clone(),
        },
    };
    let mut lines: Vec<SharedString> = text
        .lines()
        .flat_map(|l| wrap_cols(l, CALL_WRAP_COLS))
        .collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return None;
    }
    let truncated_by = lines.len().saturating_sub(OUTPUT_DETAIL_MAX_LINES);
    lines.truncate(OUTPUT_DETAIL_MAX_LINES);
    Some(ToolDetail::Output {
        lines,
        truncated_by,
    })
}

/// Reduce an inline [`openagents_proto::ToolDiff`] to the changes pane's
/// [`crate::diff::FileDiff`]: hunks grouped with 3 context lines, dual
/// 1-based line numbers, unified-diff hunk headers, and add/del counts.
#[cfg(not(target_family = "wasm"))]
pub fn diff_to_file(diff: &openagents_proto::ToolDiff) -> crate::diff::FileDiff {
    use crate::diff::{DiffLine, FileDiff, FileStatus, Hunk, LineKind};
    let old = diff.old_text.as_deref().unwrap_or("");
    let text_diff = similar::TextDiff::from_lines(old, &diff.new_text);
    let mut hunks = Vec::new();
    let (mut additions, mut deletions) = (0u32, 0u32);
    let mut max_line = 0u32;
    for group in text_diff.grouped_ops(3) {
        let (Some(first), Some(last)) = (group.first(), group.last()) else {
            continue;
        };
        let old_range = first.old_range().start..last.old_range().end;
        let new_range = first.new_range().start..last.new_range().end;
        let header = format!(
            "@@ -{},{} +{},{} @@",
            old_range.start + 1,
            old_range.len(),
            new_range.start + 1,
            new_range.len(),
        );
        let mut lines = Vec::new();
        for op in &group {
            for change in text_diff.iter_changes(op) {
                let kind = match change.tag() {
                    similar::ChangeTag::Delete => {
                        deletions += 1;
                        LineKind::Del
                    }
                    similar::ChangeTag::Insert => {
                        additions += 1;
                        LineKind::Add
                    }
                    similar::ChangeTag::Equal => LineKind::Context,
                };
                let old_no = change.old_index().map(|n| n as u32 + 1);
                let new_no = change.new_index().map(|n| n as u32 + 1);
                max_line = max_line.max(old_no.unwrap_or(0)).max(new_no.unwrap_or(0));
                lines.push(DiffLine {
                    kind,
                    old_no,
                    new_no,
                    text: change.value().trim_end_matches('\n').to_owned(),
                });
            }
        }
        hunks.push(Hunk { header, lines });
    }
    FileDiff {
        path: diff.path.clone(),
        old_path: None,
        status: if diff.old_text.is_none() {
            FileStatus::Added
        } else {
            FileStatus::Modified
        },
        binary: false,
        notices: Vec::new(),
        hunks,
        additions,
        deletions,
        max_line,
    }
}

/// The web's [`diff_to_file`], without a diff library: the lines both sides
/// share at the start and end are context (three of each kept), and the
/// rest reads as removed, then added. One hunk.
#[cfg(target_family = "wasm")]
pub fn diff_to_file(diff: &openagents_proto::ToolDiff) -> crate::diff::FileDiff {
    use crate::diff::{DiffLine, FileDiff, FileStatus, Hunk, LineKind};
    let old: Vec<&str> = diff.old_text.as_deref().unwrap_or("").lines().collect();
    let new: Vec<&str> = diff.new_text.lines().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let (old_end, new_end) = (old.len() - suffix, new.len() - suffix);
    let mut hunks = Vec::new();
    let (mut additions, mut deletions) = (0u32, 0u32);
    if prefix < old_end || prefix < new_end {
        let lead = prefix.saturating_sub(3);
        let tail = (old_end + 3).min(old.len());
        let mut lines = Vec::new();
        let context = |ix: usize, offset: usize| DiffLine {
            kind: LineKind::Context,
            old_no: Some(ix as u32 + 1),
            new_no: Some((ix + offset) as u32 + 1),
            text: old[ix].to_owned(),
        };
        lines.extend((lead..prefix).map(|ix| context(ix, 0)));
        for ix in prefix..old_end {
            deletions += 1;
            lines.push(DiffLine {
                kind: LineKind::Del,
                old_no: Some(ix as u32 + 1),
                new_no: None,
                text: old[ix].to_owned(),
            });
        }
        for ix in prefix..new_end {
            additions += 1;
            lines.push(DiffLine {
                kind: LineKind::Add,
                old_no: None,
                new_no: Some(ix as u32 + 1),
                text: new[ix].to_owned(),
            });
        }
        let shift = new_end as isize - old_end as isize;
        lines.extend((old_end..tail).map(|ix| context(ix, 0)).map(|mut line| {
            line.new_no = line.old_no.map(|n| (n as isize + shift) as u32);
            line
        }));
        let old_len = tail - lead;
        let new_len = (old_len as isize + shift) as usize;
        hunks.push(Hunk {
            header: format!("@@ -{},{} +{},{} @@", lead + 1, old_len, lead + 1, new_len),
            lines,
        });
    }
    FileDiff {
        path: diff.path.clone(),
        old_path: None,
        status: if diff.old_text.is_none() {
            FileStatus::Added
        } else {
            FileStatus::Modified
        },
        binary: false,
        notices: Vec::new(),
        hunks,
        additions,
        deletions,
        max_line: old.len().max(new.len()) as u32,
    }
}

#[derive(Clone)]
pub enum RowKind {
    GeneratedImage {
        owner: String,
        path: String,
        name: String,
        mime_type: String,
    },
    /// An agent's `show_visual` script, rendered inline by [`crate::visuals`].
    Visual {
        owner: String,
        path: String,
        title: String,
    },
    User {
        /// Visible prompt (attachment-ref trailer already stripped). When the
        /// prompt carries file mentions this is the *projected* display text —
        /// chip labels in place of the raw Markdown links.
        text: SharedString,
        /// File-mention chips over `text`, in display-byte terms. Computed
        /// once per entry change in [`rows_for_entry`] (rows are cached by
        /// fingerprint), never per frame. Empty for ordinary prompts.
        mentions: Arc<Vec<crate::composer_mentions::SentMentionSpan>>,
        /// Image refs parsed out of the message text (message-attachments.ts):
        /// thumbnails load from the owning device via ReadAttachmentChunk.
        attachments: Arc<Vec<crate::attachments::UserImageAttachment>>,
        /// Context the prompt folded in as text, lifted back out by `badges`.
        badges: Arc<Vec<crate::badges::MessageBadge>>,
        /// Optimistic echo not yet confirmed by a doc frame.
        pending: bool,
    },
    /// One top-level markdown block of a completed message.
    Markdown {
        tree: Arc<BlockTree>,
        block_ix: usize,
    },
    /// One top-level block of a STREAMING message. Split per block like
    /// completed rows (only the tail blocks' versions change per commit, so
    /// the settled prefix is never respliced or re-rendered); rendered with
    /// the fade veil.
    LiveMarkdown {
        tree: Arc<BlockTree>,
        block_ix: usize,
    },
    ToolGroup {
        summary: SharedString,
        tools: Arc<Vec<ToolItem>>,
        auto_open: bool,
        /// Compact-mode settled duration for this turn, in seconds.
        worked_secs: Option<i64>,
        /// Compact-mode work header. Expanded content is sibling rows tagged
        /// [`Row::compact_fold`], not chips.
        compact_shell: bool,
    },
    InputChip {
        /// First question's header (chat-view.tsx `InputChip`: the resolved
        /// chip shows it; unresolved shows "Awaiting your answer…" — which
        /// stays TRUE even across a run death: the composer keeps the panel
        /// up until the user answers, and the engine delivers a dead run's
        /// answer as a resumed turn).
        header: SharedString,
        resolved: bool,
    },
    ErrorChip {
        message: SharedString,
    },
    /// The fork seam: a labeled divider between copied history and the
    /// chat's own turns.
    ForkMarker {
        source_chat_id: SharedString,
        source_title: SharedString,
    },
}

pub fn generated_image_devices(owner: &str, fallback: &[String]) -> Vec<String> {
    let mut devices = Vec::new();
    for device in std::iter::once(owner).chain(fallback.iter().map(String::as_str)) {
        if !device.is_empty() && !devices.iter().any(|id| id == device) {
            devices.push(device.to_owned());
        }
    }
    devices
}

/// A transcript row: stable id + content version (diff key) + block payload.
#[derive(Clone)]
pub struct Row {
    pub id: SharedString,
    pub version: u64,
    /// First row of its message entry (gets the turn gap).
    pub turn_start: bool,
    pub kind: RowKind,
    /// The owning message entry — hover anywhere on the entry's rows reveals
    /// its timestamp strip (openagents chat-view.tsx `group`/`group-hover`).
    pub entry_id: SharedString,
    /// Epoch-ms for the 16px hover-timestamp strip UNDER this row: set on the
    /// LAST row of a completed entry (user rows always; assistant rows only
    /// once streaming ends — "the turn isn't at a time yet", chat-view.tsx).
    pub timestamp: Option<i64>,
    /// Text copied by the entry-level hover action. Present only on the last
    /// settled row, beside the timestamp; tools and transport-only metadata
    /// are deliberately excluded.
    pub copy_text: Option<SharedString>,
    /// Hidden while the named compact-work fold is closed.
    pub compact_fold: Option<SharedString>,
}

/// Absolute hover-timestamp label, e.g. "Jul 1, 3:45 PM" — the exact
/// `formatTimestamp` shape (utils.ts: short month, numeric day, hour,
/// 2-digit minutes, no leading zero on the hour). Pure over an explicit
/// timezone so tests don't depend on the host's local time.
pub fn format_timestamp<Tz: chrono::TimeZone>(ms: i64, tz: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(utc) => utc
            .with_timezone(tz)
            .format("%b %-d, %-I:%M %p")
            .to_string(),
        None => String::new(),
    }
}

pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x1_0000_01b3);
    }
    hash
}

pub fn tool_fingerprint(tools: &[ToolItem], auto_open: bool) -> u64 {
    let mut acc = Vec::with_capacity(tools.len() * 8 + 1);
    for t in tools {
        acc.extend_from_slice(t.part_id.as_bytes());
        acc.push(0);
        let (label, detail) = tool_chip_content(&t.call);
        acc.extend_from_slice(label.as_bytes());
        acc.extend_from_slice(&(detail.len() as u32).to_le_bytes());
        acc.push(t.is_error as u8 | (t.resolved as u8) << 1);
        // Thought/note chips share `ToolCall::Unknown` — the kind is the
        // label/icon discriminator, so a flip must re-splice.
        acc.push(t.kind as u8);
        acc.push(t.images.len() as u8);
        // Detail payload arriving (or growing) must re-splice the row even
        // when the resolved bit didn't change.
        match t.detail.as_deref() {
            None => acc.push(0),
            Some(ToolDetail::Output {
                lines,
                truncated_by,
            }) => {
                acc.push(1);
                acc.extend_from_slice(&(lines.len() as u32).to_le_bytes());
                acc.extend_from_slice(&(*truncated_by as u32).to_le_bytes());
                let bytes: usize = lines.iter().map(|l| l.len()).sum();
                acc.extend_from_slice(&(bytes as u32).to_le_bytes());
            }
            Some(ToolDetail::Thought {
                lines,
                truncated_by,
            }) => {
                // Byte-exact plus style bits: a live mend can restyle runs
                // without changing the flattened length, and the row must
                // still re-splice.
                acc.push(4);
                acc.extend_from_slice(&(lines.len() as u32).to_le_bytes());
                acc.extend_from_slice(&(*truncated_by as u32).to_le_bytes());
                for line in lines {
                    for run in line {
                        acc.extend_from_slice(run.text.as_bytes());
                        acc.push(
                            run.style.bold as u8
                                | (run.style.italic as u8) << 1
                                | (run.style.code as u8) << 2
                                | (run.style.strikethrough as u8) << 3
                                | (run.style.link.is_some() as u8) << 4,
                        );
                    }
                    acc.push(b'\n');
                }
            }
            Some(ToolDetail::Diff { file, .. }) => {
                acc.push(2);
                acc.extend_from_slice(file.path.as_bytes());
                acc.extend_from_slice(&file.additions.to_le_bytes());
                acc.extend_from_slice(&file.deletions.to_le_bytes());
                acc.extend_from_slice(&(file.hunks.len() as u32).to_le_bytes());
            }
            Some(ToolDetail::Stats { stats }) => {
                acc.push(3);
                for stat in stats.iter() {
                    acc.extend_from_slice(stat.path.as_bytes());
                    acc.extend_from_slice(&stat.additions.to_le_bytes());
                    acc.extend_from_slice(&stat.deletions.to_le_bytes());
                }
            }
        }
        // The invocation block is pure over `call`, which the one-line hash
        // above only covers by length — hash its bytes so an in-place call
        // update (a streaming MCP input, a growing todo list) re-splices.
        if let Some(ToolDetail::Output {
            lines,
            truncated_by,
        }) = t.invocation.as_deref()
        {
            for line in lines {
                acc.extend_from_slice(line.as_bytes());
            }
            acc.extend_from_slice(&(*truncated_by as u32).to_le_bytes());
        }
        // Sidecar refs arriving after the resolve tick must re-splice too —
        // they add the fetch affordance without changing the detail payload.
        acc.push(t.output_ref.is_some() as u8 | (t.diff_ref.is_some() as u8) << 1);
        // Subagent lifecycle mutates the chip in place (status flips, the
        // live tail grows) — hash it so the row re-splices on every change.
        acc.push(
            t.subagent_ref.is_some() as u8
                | match t.subagent_status {
                    None => 0,
                    Some(SubagentStatus::Running) => 1 << 1,
                    Some(SubagentStatus::Done) => 2 << 1,
                    Some(SubagentStatus::Failed) => 3 << 1,
                },
        );
        if let Some(tail) = &t.subagent_tail {
            acc.extend_from_slice(tail.as_bytes());
        }
    }
    acc.push(auto_open as u8);
    fnv1a(&acc)
}

/// Clipboard payload for an assistant/system entry: authored text parts in
/// document order, preserving Markdown while excluding tool traces and other
/// structured parts.
pub fn assistant_copy_text(entry: &SessionMessageEntry) -> Option<SharedString> {
    let text = entry
        .parts
        .iter()
        .filter_map(|part| match part {
            // Inspect the trimmed view only to reject empty parts. Copy the
            // original bytes so indentation-based code blocks and Markdown
            // hard-break whitespace survive the clipboard round trip.
            MessagePart::Text { text, .. } if !text.trim().is_empty() => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    (!text.is_empty()).then(|| text.into())
}

/// Conservative first-frame fallback for whether a prompt may need a fold
/// affordance. Once the text element has measured, `render_user_body` replaces
/// this proxy with the exact wrapped-line count, so glyph width and script no
/// longer affect eligibility heuristically.
pub fn user_message_needs_collapse(text: &str) -> bool {
    text.lines().count() > USER_COLLAPSED_LINES || text.chars().count() > USER_COLLAPSE_CHARS
}

/// Layout transitions need more time when they travel farther, otherwise a
/// 1,000px pasted log crosses the screen in the same 200ms as a six-line note
/// and reads as a snap. Keep ordinary messages close to the catalog RESIZE
/// timing, then scale to an 850ms ceiling for genuinely large pasted content.
pub fn user_resize_duration_ms(height_delta: f32) -> u64 {
    (220.0 + height_delta.max(0.0) * 0.32).min(850.0).round() as u64
}

/// Short folds keep the app's familiar decisive ease-out. Large folds use
/// ease-in-out so thousands of pixels do not disappear in the first few frames
/// of an aggressively front-loaded curve.
pub fn user_resize_spec(height_delta: f32) -> motion::MotionSpec {
    let curve = if height_delta > 500.0 {
        motion::EASE_IN_OUT
    } else {
        motion::EASE_OUT
    };
    motion::MotionSpec::new(user_resize_duration_ms(height_delta), curve)
}

#[cfg(any(test, feature = "test-support"))]
thread_local! { pub static FORBID_ROW_PREPARATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

/// Build the block rows of one (already continuation-joined) entry.
///
/// `parse` maps `(part_key, text)` to a block tree — the entity supplies
/// incremental parsers for live parts and a cache for complete ones; tests pass
/// a plain `parse_full`.
/// `compact` is the transcript's compact mode: every working step of the
/// turn — tool calls, thoughts, and the narration text between them — folds
/// into ONE collapsed [`RowKind::ToolGroup`], so only the reply text (the
/// trailing run of text parts) stays a visible row.
pub fn rows_for_entry(
    entry: &SessionMessageEntry,
    pending: bool,
    compact: bool,
    parse: &mut dyn FnMut(&str, &str) -> Arc<BlockTree>,
) -> Vec<Row> {
    #[cfg(any(test, feature = "test-support"))]
    FORBID_ROW_PREPARATION
        .with(|forbidden| assert!(!forbidden.get(), "row preparation ran on the UI thread"));
    let mut rows: Vec<Row> = Vec::new();
    let streaming = entry.status == Some(MessageStatus::Streaming);
    let entry_id: SharedString = entry.id.clone().into();

    if entry.role == MessageRole::User {
        let raw: String = entry
            .parts
            .iter()
            .filter_map(|p| match p {
                MessagePart::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        // Attachment refs ride the plain text (the `withAttachments`
        // transport); split them back out for the thumbnail strip.
        let parsed = crate::attachments::parse_user_message_images(&raw);
        // File mentions render as chips here too, not just in the composer.
        // The projection is pure over the text, so the raw-length row version
        // below stays a valid cache/diff key.
        // Lifted before the mention projection, so a comment body's own
        // Markdown never lands in the bubble.
        let (body, badges) = crate::badges::split(&parsed.text);
        let body = agent_message_display(&body);
        // Chips stand in for their attachments: those leave the strip, and a
        // chip carries its upload's path for its progress and, for an image,
        // a click opening it.
        let chips = openagents_proto::attachment_mentions::attachment_mentions(&body);
        let mut attachments = parsed.attachments;
        let (text, mut mentions) = match crate::composer_mentions::sent_mention_display(&body) {
            Some((display, spans)) => (display, spans),
            None => (body, Vec::new()),
        };
        for span in &mut mentions {
            let Some(chip) = chips
                .iter()
                .find(|chip| Some(chip.index) == span.attachment)
            else {
                continue;
            };
            span.upload = attachments
                .iter()
                .find(|att| crate::attachments::chip_names_attachment(chip, &att.path))
                .map(|att| att.path.clone().into());
        }
        attachments.retain(|att| {
            att.appshot.is_some()
                || !chips
                    .iter()
                    .any(|chip| crate::attachments::chip_names_attachment(chip, &att.path))
        });
        let copy_text = (!text.trim().is_empty()).then(|| SharedString::from(text.clone()));
        return vec![Row {
            id: entry.id.clone().into(),
            version: (raw.len() as u64) << 1 | pending as u64,
            turn_start: true,
            kind: RowKind::User {
                text: text.into(),
                mentions: Arc::new(mentions),
                attachments: Arc::new(attachments),
                badges: Arc::new(badges),
                pending,
            },
            entry_id,
            // User rows always carry the strip (chat-view.tsx: whenever
            // `createdAt` exists — the optimistic echo included).
            timestamp: Some(entry.created_at),
            copy_text,
            compact_fold: None,
        }];
    }

    // Assistant/system: split parts into block rows, folding consecutive
    // ordinary tools. Agent/spawn chips flush into their own group so they
    // never share a collapse with Reads/Runs.
    let last_part_ix = entry.parts.len().saturating_sub(1);
    // Compact mode's reply boundary: the index where the turn's TRAILING run
    // of non-empty text parts begins. Every text part before it is narration
    // that folds into the single work group; the run itself stays visible
    // rows. While the turn still STREAMS there is no reply yet — a "tail"
    // text would only fold back in once work resumes, so it rides the group
    // too and nothing visible expands by default. `None` means the turn ends
    // on work (or is mid-flight) — only the accordion renders.
    let reply_start = (compact && !streaming)
        .then(|| {
            let mut start = entry.parts.iter().rposition(
                |part| matches!(part, MessagePart::Text { text, .. } if !text.trim().is_empty()),
            )?;
            while start > 0
                && matches!(
                    &entry.parts[start - 1],
                    MessagePart::Text { text, .. } if !text.trim().is_empty()
                )
            {
                start -= 1;
            }
            Some(start)
        })
        .flatten();
    let mut group_ix = 0usize;
    let mut pending_group: Vec<ToolItem> = Vec::new();
    let mut group_last_part_ix = 0usize;
    // Compact mode: the row index the single work group lands at — the
    // position of the FIRST foldable part, so Input/Error chips ahead of it
    // keep their doc order.
    let mut compact_group_pos: Option<usize> = None;

    let flush_group =
        |rows: &mut Vec<Row>, group: &mut Vec<ToolItem>, group_ix: &mut usize, last_ix: usize| {
            if group.is_empty() {
                return;
            }
            let tools = std::mem::take(group);
            let auto_open = streaming && last_ix == last_part_ix;
            rows.push(Row {
                id: format!("{}#g{}", entry.id, group_ix).into(),
                version: tool_fingerprint(&tools, auto_open),
                turn_start: false,
                kind: RowKind::ToolGroup {
                    summary: tool_group_summary(&tools).into(),
                    tools: Arc::new(tools),
                    auto_open,
                    worked_secs: None,
                    compact_shell: false,
                },
                entry_id: entry.id.clone().into(),
                timestamp: None,
                copy_text: None,
                compact_fold: None,
            });
            *group_ix += 1;
        };

    for (part_ix, part) in entry.parts.iter().enumerate() {
        match part {
            MessagePart::Tool {
                call,
                is_error,
                resolved,
                output,
                diff,
                output_ref,
                output_bytes,
                diff_ref,
                diff_stats,
                subagent_ref,
                subagent_status,
                subagent_tail,
                ..
            } => {
                let item = ToolItem {
                    part_id: part.id().to_owned(),
                    call: call.clone(),
                    is_error: *is_error,
                    resolved: *resolved,
                    detail: tool_detail(output.as_deref(), diff.as_ref(), diff_stats.as_deref())
                        .map(Arc::new),
                    invocation: call_block(call).map(Arc::new),
                    output_ref: output_ref.clone().map(SharedString::from),
                    output_bytes: *output_bytes,
                    diff_ref: diff_ref.clone().map(SharedString::from),
                    subagent_ref: subagent_ref.clone().map(SharedString::from),
                    subagent_status: *subagent_status,
                    subagent_tail: subagent_tail.clone().map(SharedString::from),
                    kind: ToolItemKind::Call,
                    images: crate::tool_image_paths::image_paths(call, output.as_deref()),
                };
                if compact {
                    // Compact mode keeps ONE group for the whole turn —
                    // agent chips fold in with everything else, so the genus
                    // split below never runs.
                    compact_group_pos.get_or_insert(rows.len());
                } else if pending_group
                    .first()
                    .is_some_and(|head| is_agent_tool(head) != is_agent_tool(&item))
                {
                    // Agent chips don't share a fold with ordinary tools:
                    // flush whenever the genus flips so each group is uniform.
                    flush_group(
                        &mut rows,
                        &mut pending_group,
                        &mut group_ix,
                        group_last_part_ix,
                    );
                }
                pending_group.push(item);
                group_last_part_ix = part_ix;
            }
            // Thinking rides the SAME accordion as the tools around it
            // (user request) — a thought chip in the group, not its own row.
            MessagePart::Reasoning { id: part_id, text } => {
                if text.trim().is_empty() {
                    continue;
                }
                // Live only while it is the tail of a streaming reply — once
                // text or a tool follows, the thought is finished even though
                // the entry still streams.
                let live = streaming && part_ix == last_part_ix;
                // The same parse wiring as text parts: incremental while
                // streaming, hanging inline markers mended for display, the
                // settled cache once complete.
                let tree = parse(&format!("{}#{}", entry.id, part_id), text);
                let item = thought_item(part_id, &tree, live);
                if compact {
                    compact_group_pos.get_or_insert(rows.len());
                } else if pending_group.first().is_some_and(is_agent_tool) {
                    // Thoughts join ordinary tool groups; agent (spawn-link)
                    // groups stay pure, exactly like the tool genus rule.
                    flush_group(
                        &mut rows,
                        &mut pending_group,
                        &mut group_ix,
                        group_last_part_ix,
                    );
                }
                pending_group.push(item);
                group_last_part_ix = part_ix;
            }
            other => {
                // Compact mode: narration text folds into the single group as
                // a "Wrote" chip — only the reply (the trailing text run)
                // stays a row.
                if compact
                    && let MessagePart::Text { id: part_id, text } = other
                    && !text.trim().is_empty()
                    && reply_start.is_none_or(|start| part_ix < start)
                {
                    compact_group_pos.get_or_insert(rows.len());
                    let tree = parse(&format!("{}#{}", entry.id, part_id), text);
                    let live = streaming && part_ix == last_part_ix;
                    pending_group.push(note_item(part_id, &tree, live));
                    group_last_part_ix = part_ix;
                    continue;
                }
                if !compact {
                    flush_group(
                        &mut rows,
                        &mut pending_group,
                        &mut group_ix,
                        group_last_part_ix,
                    );
                }
                match other {
                    MessagePart::Text { id: part_id, text } => {
                        if text.trim().is_empty() {
                            continue;
                        }
                        let key = format!("{}#{}", entry.id, part_id);
                        let tree = parse(&key, text);
                        // Live and completed parts split identically — one row
                        // per top-level block, same ids, so the live→complete
                        // handoff never changes row identity. The version is a
                        // content hash of the block's bytes (LSB = streaming),
                        // so a commit only splices rows whose bytes actually
                        // changed — the settled prefix of a live reply is
                        // untouched (and its render caches stay valid).
                        for block_ix in 0..tree.blocks.len() {
                            let range = &tree.blocks[block_ix].range;
                            let end = range.end.min(text.len());
                            let bytes = text
                                .as_bytes()
                                .get(range.start.min(end)..end)
                                .unwrap_or_default();
                            let version = (fnv1a(bytes) << 1) | streaming as u64;
                            rows.push(Row {
                                id: format!("{key}.{block_ix}").into(),
                                version,
                                turn_start: false,
                                entry_id: entry_id.clone(),
                                timestamp: None,
                                copy_text: None,
                                compact_fold: None,
                                kind: if streaming {
                                    RowKind::LiveMarkdown {
                                        tree: tree.clone(),
                                        block_ix,
                                    }
                                } else {
                                    RowKind::Markdown {
                                        tree: tree.clone(),
                                        block_ix,
                                    }
                                },
                            });
                        }
                    }
                    MessagePart::Image {
                        id: part_id,
                        path,
                        name,
                        mime_type,
                    } => {
                        rows.push(Row {
                            id: format!("{}#{}", entry.id, part_id).into(),
                            version: fnv1a(
                                format!("{}\0{path}\0{name}\0{mime_type}", entry.device_id)
                                    .as_bytes(),
                            ),
                            turn_start: false,
                            entry_id: entry_id.clone(),
                            timestamp: None,
                            copy_text: None,
                            compact_fold: None,
                            kind: RowKind::GeneratedImage {
                                owner: entry.device_id.clone(),
                                path: path.clone(),
                                name: name.clone(),
                                mime_type: mime_type.clone(),
                            },
                        });
                    }
                    MessagePart::Visual {
                        id: part_id,
                        path,
                        title,
                    } => {
                        rows.push(Row {
                            id: format!("{}#{}", entry.id, part_id).into(),
                            version: fnv1a(
                                format!("{}\0{path}\0{title}", entry.device_id).as_bytes(),
                            ),
                            turn_start: false,
                            entry_id: entry_id.clone(),
                            timestamp: None,
                            copy_text: None,
                            compact_fold: None,
                            kind: RowKind::Visual {
                                owner: entry.device_id.clone(),
                                path: path.clone(),
                                title: title.clone(),
                            },
                        });
                    }
                    MessagePart::Input {
                        id: part_id,
                        questions,
                        resolved,
                        ..
                    } => {
                        // Model-generated header onto the one-line chip.
                        let header: SharedString = single_line(
                            &questions
                                .first()
                                .map(|q| q.header.clone())
                                .unwrap_or_else(|| "Question".to_string()),
                        )
                        .into();
                        rows.push(Row {
                            id: format!("{}#{}", entry.id, part_id).into(),
                            version: fnv1a(header.as_bytes()) << 1 | *resolved as u64,
                            turn_start: false,
                            kind: RowKind::InputChip {
                                header,
                                resolved: *resolved,
                            },
                            entry_id: entry_id.clone(),
                            timestamp: None,
                            copy_text: None,
                            compact_fold: None,
                        });
                    }
                    MessagePart::Error {
                        id: part_id,
                        message,
                    } => {
                        rows.push(Row {
                            id: format!("{}#{}", entry.id, part_id).into(),
                            version: message.len() as u64,
                            turn_start: false,
                            kind: RowKind::ErrorChip {
                                // Harness-generated; the chip is one line.
                                message: single_line(message).into(),
                            },
                            entry_id: entry_id.clone(),
                            timestamp: None,
                            copy_text: None,
                            compact_fold: None,
                        });
                    }
                    MessagePart::Fork {
                        id: part_id,
                        source_chat_id,
                        source_title,
                    } => {
                        rows.push(Row {
                            id: format!("{}#{}", entry.id, part_id).into(),
                            version: fnv1a(source_title.as_bytes()),
                            turn_start: false,
                            kind: RowKind::ForkMarker {
                                source_chat_id: source_chat_id.clone().into(),
                                source_title: single_line(source_title).into(),
                            },
                            entry_id: entry_id.clone(),
                            timestamp: None,
                            copy_text: None,
                            compact_fold: None,
                        });
                    }
                    // Tools and thoughts are grouped by the outer arms;
                    // nothing reaches here.
                    MessagePart::Tool { .. } | MessagePart::Reasoning { .. } => {}
                }
            }
        }
    }
    if compact {
        // Collapsed: one work header. Expanded: the same rows compact-off
        // would emit for the work parts, tagged so the fold can hide them.
        if !pending_group.is_empty() {
            let tools = std::mem::take(&mut pending_group);
            let work_id: SharedString = format!("{}#work", entry.id).into();
            let work_parts: Vec<MessagePart> = entry
                .parts
                .iter()
                .enumerate()
                .filter(|(ix, part)| is_compact_work_part(*ix, part, reply_start))
                .map(|(_, part)| part.clone())
                .collect();
            let mut inner = if work_parts.is_empty() {
                Vec::new()
            } else {
                let work_entry = SessionMessageEntry {
                    parts: work_parts,
                    duration_ms: None,
                    continuation_of: None,
                    ..entry.clone()
                };
                rows_for_entry(&work_entry, pending, false, parse)
            };
            for row in &mut inner {
                row.turn_start = false;
                row.timestamp = None;
                row.copy_text = None;
                row.compact_fold = Some(work_id.clone());
            }
            let header = Row {
                id: work_id,
                version: tool_fingerprint(&tools, false),
                turn_start: false,
                kind: RowKind::ToolGroup {
                    summary: tool_group_summary(&tools).into(),
                    tools: Arc::new(tools),
                    auto_open: false,
                    worked_secs: (!streaming)
                        .then(|| {
                            entry
                                .duration_ms
                                .filter(|&ms| ms > 0)
                                .map(|ms| (ms / 1000).max(1))
                        })
                        .flatten(),
                    compact_shell: true,
                },
                entry_id: entry.id.clone().into(),
                timestamp: None,
                copy_text: None,
                compact_fold: None,
            };
            let pos = compact_group_pos.unwrap_or(rows.len());
            for (i, row) in inner.into_iter().enumerate() {
                rows.insert(pos + i, row);
            }
            rows.insert(pos, header);
        }
    } else {
        flush_group(
            &mut rows,
            &mut pending_group,
            &mut group_ix,
            group_last_part_ix,
        );
    }

    if let Some(first) = rows.first_mut() {
        first.turn_start = true;
    }
    // Timestamp strip under the entry's LAST row once the turn has settled
    // (chat-view.tsx: "No timestamp hover mid-stream"). The version bit keeps
    // the diff key honest for last-row kinds whose own version wouldn't
    // change when streaming flips off (chips).
    if !streaming
        && let Some(last) = rows.last_mut()
        && !matches!(last.kind, RowKind::ForkMarker { .. })
    {
        last.timestamp = Some(entry.created_at);
        last.copy_text = assistant_copy_text(entry);
        last.version ^= 1 << 62;
    }
    rows
}

pub fn is_compact_work_part(ix: usize, part: &MessagePart, reply_start: Option<usize>) -> bool {
    match part {
        MessagePart::Tool { .. } => true,
        MessagePart::Reasoning { text, .. } => !text.trim().is_empty(),
        MessagePart::Text { text, .. } => {
            !text.trim().is_empty() && reply_start.is_none_or(|start| ix < start)
        }
        _ => false,
    }
}

/// `OPENAGENTS_FRAME_STATS=1` logs live-row render-cost percentiles (p50/p95 µs
/// over rolling windows of [`FRAME_STATS_WINDOW`] samples) at `warn` level —
/// the smoothness measurement knob. Off by default; zero cost when off.
pub fn frame_stats_enabled() -> bool {
    pub static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        openagents_brand::env_var("OPENAGENTS_FRAME_STATS").is_ok_and(|v| !v.is_empty() && v != "0")
    })
}

pub const FRAME_STATS_WINDOW: usize = 240;

/// Opt-in cadence counters complement per-row timings: inexpensive rows can
/// still exhaust a battery when an unrelated animation rebuilds them at 120Hz.
pub fn record_view_frame(view: &'static str) -> bool {
    if !frame_stats_enabled() {
        return false;
    }
    thread_local! {
        static COUNTERS: RefCell<HashMap<&'static str, (Instant, u64)>> = RefCell::default();
    }
    COUNTERS.with(|counters| {
        let mut counters = counters.borrow_mut();
        let (start, frames) = counters.entry(view).or_insert_with(|| (Instant::now(), 0));
        *frames += 1;
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            tracing::warn!(
                view,
                frames_per_second = *frames as f64 / elapsed,
                "view render cadence"
            );
            *start = Instant::now();
            *frames = 0;
            return true;
        }
        false
    })
}

/// `OPENAGENTS_NO_RENDER_CACHE=1` bypasses the cross-frame flatten cache — the
/// A/B knob for the frame-cost measurement above.
pub fn render_cache_disabled() -> bool {
    pub static DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DISABLED.get_or_init(|| {
        openagents_brand::env_var("OPENAGENTS_NO_RENDER_CACHE")
            .is_ok_and(|v| !v.is_empty() && v != "0")
    })
}

pub fn record_live_frame_us(us: u64) {
    thread_local! {
        static SAMPLES: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
    }
    SAMPLES.with(|s| {
        let mut s = s.borrow_mut();
        s.push(us);
        if s.len() >= FRAME_STATS_WINDOW {
            s.sort_unstable();
            let p50 = s[s.len() / 2];
            let p95 = s[s.len() * 95 / 100];
            let max = *s.last().unwrap();
            tracing::warn!(
                n = s.len(),
                p50_us = p50,
                p95_us = p95,
                max_us = max,
                "live-row render cost"
            );
            s.clear();
        }
    });
}

/// How [`parse_for_row`] produced its tree — carries the incremental parser's
/// work counters so callers (and tests) can see that per-append parse work is
/// bounded by the reparsed tail, never the whole accumulated reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseOutcome {
    /// Streaming row: the live [`IncrementalParser`] advanced by one commit.
    Incremental {
        /// Bytes fed through `parse_full` for this commit (the reparse tail).
        parsed_bytes: usize,
        /// Leading top-level blocks left untouched (render caches stay valid).
        stable_prefix_blocks: usize,
    },
    /// Completed row served from the settled tree cache (no parse at all).
    Cached,
    /// Live→complete handoff: the live parser's exact tree was adopted.
    Handoff,
    /// Completed row parsed from scratch.
    Full,
}

/// The transcript's markdown parse wiring, extracted for testability: one call
/// per text part per sync. Streaming parts keep one [`IncrementalParser`] per
/// row key and advance it with the full accumulated text (`set_text` takes the
/// O(tail) append path for the prefix-extensions the doc watch delivers);
/// completed parts hit the settled cache, adopt the live parser's tree on the
/// live→complete flip (flicker-free handoff), or do one full parse.
pub fn parse_for_row(
    streaming: bool,
    key: &str,
    text: &str,
    live_parsers: &mut HashMap<String, IncrementalParser>,
    tree_cache: &mut HashMap<String, (usize, Arc<BlockTree>)>,
) -> (Arc<BlockTree>, ParseOutcome) {
    if streaming {
        let parser = live_parsers.entry(key.to_string()).or_default();
        parser.set_text(text);
        (
            // Display tree: hanging inline markers mended so closers arriving
            // later never reflow painted text (markdown/mend.rs). Completed
            // rows below use the canonical tree — the honest settle.
            Arc::new(parser.display_tree()),
            ParseOutcome::Incremental {
                parsed_bytes: parser.last_parse_bytes(),
                stable_prefix_blocks: parser.stable_prefix_blocks(),
            },
        )
    } else {
        if let Some((len, tree)) = tree_cache.get(key)
            && *len == text.len()
        {
            return (tree.clone(), ParseOutcome::Cached);
        }
        // On the live→complete flip reuse the live parser's tree when
        // the sources match — the split rows then share the exact tree
        // the unsplit row painted, guaranteeing a flicker-free handoff.
        let (tree, outcome) = match live_parsers.remove(key) {
            Some(parser) if parser.source() == text => {
                (Arc::new(parser.tree().clone()), ParseOutcome::Handoff)
            }
            _ => (Arc::new(parse_full(text)), ParseOutcome::Full),
        };
        tree_cache.insert(key.to_string(), (text.len(), tree.clone()));
        (tree, outcome)
    }
}

/// Markdown row ids are `{entry}#{part}.{blockIx}` — the part prefix is
/// everything before the block index.
pub fn part_prefix(id: &str) -> &str {
    id.rsplit_once('.').map(|(p, _)| p).unwrap_or(id)
}

/// Vertical gap opening `row` given its predecessor: turn gap at turn starts;
/// the markdown block gap between sibling block rows split from the same text
/// part — matching the live row's internal spacing exactly, so the
/// live→split handoff cannot shift a pixel. Tool groups get one larger global
/// step on either boundary so their dense chip stack has room to breathe.
pub fn top_gap_for(prev: Option<&Row>, row: &Row) -> f32 {
    if row.turn_start {
        return Theme::SPACE_LG;
    }
    let is_md = |k: &RowKind| matches!(k, RowKind::Markdown { .. } | RowKind::LiveMarkdown { .. });
    let same_part_markdown = prev.is_some_and(|p| {
        is_md(&p.kind) && is_md(&row.kind) && part_prefix(&p.id) == part_prefix(&row.id)
    });
    if same_part_markdown {
        render::MD_BLOCK_GAP
    } else if matches!(row.kind, RowKind::ToolGroup { .. })
        || prev.is_some_and(|row| matches!(row.kind, RowKind::ToolGroup { .. }))
    {
        Theme::SPACE_MD
    } else {
        Theme::SPACE_SM
    }
}

/// Minimal splice for a row-set change: `Some((old_range, new_count))`, or
/// `None` when the sets are identical by (id, version).
pub fn diff_rows(old: &[Row], new: &[Row]) -> Option<(Range<usize>, usize)> {
    let eq = |a: &Row, b: &Row| a.id == b.id && a.version == b.version;
    let mut prefix = 0usize;
    let max_prefix = old.len().min(new.len());
    while prefix < max_prefix && eq(&old[prefix], &new[prefix]) {
        prefix += 1;
    }
    if prefix == old.len() && prefix == new.len() {
        return None;
    }
    let mut suffix = 0usize;
    let max_suffix = (old.len() - prefix).min(new.len() - prefix);
    while suffix < max_suffix && eq(&old[old.len() - 1 - suffix], &new[new.len() - 1 - suffix]) {
        suffix += 1;
    }
    Some((prefix..old.len() - suffix, new.len() - suffix - prefix))
}

// ---------------------------------------------------------------------------
// Tool summaries / chips (pure)
// ---------------------------------------------------------------------------

/// The ToolGroup summary line — "Ran 3 commands · edited 2 files".
///
/// The rule lives in `openagents_proto::view` so the terminal viewport reports the
/// same summary; this only adapts the row model's [`ToolItem`] to it.
pub fn tool_group_summary(tools: &[ToolItem]) -> String {
    #[cfg(any(test, feature = "test-support"))]
    FORBID_ROW_PREPARATION
        .with(|forbidden| assert!(!forbidden.get(), "tool summary formatting ran on UI thread"));
    let pairs: Vec<(ToolCall, bool)> = tools
        .iter()
        .filter(|t| t.kind == ToolItemKind::Call)
        .map(|t| (t.call.clone(), t.is_error))
        .collect();
    let thoughts = tools
        .iter()
        .filter(|t| t.kind == ToolItemKind::Thought)
        .count();
    let notes = tools
        .iter()
        .filter(|t| t.kind == ToolItemKind::Note)
        .count();
    // The shared summary answers "used 0 tools" for an empty set — a
    // thought-only group must not inherit that.
    let base = if pairs.is_empty() {
        String::new()
    } else {
        openagents_proto::view::tool_group_summary(&pairs)
    };
    // Thought and note chips ride the group (they are UI-synthesized, so the
    // shared view summary never sees them): name them on the collapsed line.
    let mut segments: Vec<String> = Vec::new();
    match thoughts {
        0 => {}
        1 => segments.push("thought process".into()),
        n => segments.push(format!("thought {n} times")),
    }
    match notes {
        0 => {}
        1 => segments.push("wrote a note".into()),
        n => segments.push(format!("wrote {n} notes")),
    }
    if !base.is_empty() {
        segments.push(base);
    }
    let mut summary = segments.join(" · ");
    // Same capitalization rule as the shared summary — only the leading
    // letter lifts.
    if let Some(first) = summary.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    summary
}

pub fn tool_group_title(
    text: SharedString,
    shimmer_phase: Option<f32>,
    theme: &Theme,
) -> AnyElement {
    let Some(shimmer_phase) = shimmer_phase else {
        // Keep the ordinary inherited hover color when the group is settled
        // (and when reduced motion turns the active shimmer off).
        return text.into_any_element();
    };
    // Keep the title as ONE normal text run. Splitting it per character copies
    // the gradient stops, but also splits shaping/kerning and makes the sweep
    // visibly hop one glyph at a time. The overlay repaints the intact shaped
    // line through narrow moving clips, which is the native equivalent of
    // CSS `background-clip: text` without duplicating accessible text.
    let overlay_text = text.clone();
    let overlay_font = gpui::font(theme.font_sans_fixed.clone());
    let base = theme.text_muted;
    let peak = theme.text;
    let overlay = canvas(
        move |bounds, window, _| {
            let probe = window.text_system().shape_line(
                overlay_text.clone(),
                px(TOOL_LABEL_SIZE),
                &[TextRun {
                    len: overlay_text.len(),
                    font: overlay_font.clone(),
                    color: peak,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            let text_width = f32::from(probe.width()).min(f32::from(bounds.size.width));
            let strip_count = (text_width / TOOL_GROUP_SHIMMER_STRIP_WIDTH).ceil() as usize;
            let mut strips = Vec::with_capacity(strip_count);
            for ix in 0..strip_count {
                let left = ix as f32 * TOOL_GROUP_SHIMMER_STRIP_WIDTH;
                let right = ((ix + 1) as f32 * TOOL_GROUP_SHIMMER_STRIP_WIDTH).min(text_width);
                let x = (left + right) * 0.5 / text_width.max(1.0);
                let amount = tool_title_shimmer_amount(x, shimmer_phase);
                if amount <= 0.001 {
                    continue;
                }
                let line = window.text_system().shape_line(
                    overlay_text.clone(),
                    px(TOOL_LABEL_SIZE),
                    &[TextRun {
                        len: overlay_text.len(),
                        font: overlay_font.clone(),
                        color: motion::mix(base, peak, amount),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                );
                strips.push((left, right, line));
            }
            strips
        },
        move |bounds, strips, window, cx| {
            for (left, right, line) in strips {
                let mask = ContentMask {
                    bounds: Bounds {
                        origin: point(bounds.origin.x + px(left), bounds.origin.y),
                        size: size(px(right - left), bounds.size.height),
                    },
                };
                window.with_content_mask(Some(mask), |window| {
                    let line_height = px(TOOL_LABEL_LINE_HEIGHT);
                    let _ = line.paint(
                        bounds.origin,
                        line_height,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                });
            }
        },
    )
    .absolute()
    .inset_0();
    div()
        .relative()
        .h_full()
        .min_w_0()
        .flex_1()
        .overflow_hidden()
        .child(text)
        .child(overlay)
        .into_any_element()
}

// `single_line` and the per-kind chip label/detail are shared with the terminal
// viewport (`openagents_proto::view`): a tool must be named identically on every
// surface, and the one-line collapse is needed for the same reason in both (a
// literal newline breaks gpui's ellipsis logic and would be a cursor move in a
// cell grid).
pub use openagents_proto::view::{single_line, tool_chip_content};

/// Analytic expanded-chips height — no measurement needed for the fold tween.
pub fn chips_height(count: usize) -> f32 {
    if count == 0 {
        return 0.0;
    }
    CHIPS_TOP_PAD + count as f32 * CHIP_HEIGHT + (count as f32 - 1.0) * CHIP_GAP
}

/// Analytic height an open detail adds to its chip's card (separator + body)
/// — output blocks by line count, diff blocks via the changes pane's own
/// [`crate::diff::body_height`]. The chip's own [`CHIP_HEIGHT`] is already
/// counted by [`chips_height`].
pub fn detail_height(detail: &ToolDetail) -> f32 {
    let body = match detail {
        ToolDetail::Output {
            lines,
            truncated_by,
        } => {
            let rows = lines.len() + usize::from(*truncated_by > 0);
            rows as f32 * OUTPUT_LINE_HEIGHT + OUTPUT_BODY_PAD
        }
        ToolDetail::Thought {
            lines,
            truncated_by,
        } => {
            let rows = lines.len() + usize::from(*truncated_by > 0);
            rows as f32 * OUTPUT_LINE_HEIGHT + OUTPUT_BODY_PAD
        }
        ToolDetail::Diff { file, .. } => crate::diff::body_height(file),
        ToolDetail::Stats { stats } => stats.len() as f32 * OUTPUT_LINE_HEIGHT + OUTPUT_BODY_PAD,
    };
    DETAIL_SEPARATOR + body
}

/// Tool image preview frame height; widths follow each image's aspect.
pub const TOOL_IMAGE_HEIGHT: f32 = 220.0;
pub const TOOL_IMAGE_MAX_WIDTH: f32 = 560.0;
pub const TOOL_IMAGE_PAD: f32 = 8.0;
/// Analytic height an open chip's image strip adds (separator + frames).
pub const TOOL_IMAGE_STRIP_HEIGHT: f32 =
    DETAIL_SEPARATOR + TOOL_IMAGE_HEIGHT + 2.0 * TOOL_IMAGE_PAD;

/// The open image context menu: what to copy and where it lives.
pub struct ImageMenu {
    pub path: String,
    pub devices: Vec<String>,
    pub position: Point<Pixels>,
    pub active: Option<usize>,
}

/// Spawn chips never expand, so they never preview images either.
pub fn chip_has_images(tool: &ToolItem) -> bool {
    !tool.images.is_empty() && !is_spawn_link(tool)
}

/// Height of the "Show full output/diff" affordance row appended below an
/// open detail whose full payload lives in the sidecar (chat2-sync A3).
pub const BLOB_AFFORDANCE_HEIGHT: f32 = 24.0;

/// What an open chip's [`BLOB_AFFORDANCE_HEIGHT`] row offers: a lazy sidecar
/// fetch ("Show full output/diff"). One slot, so the analytic height sums
/// stay a single `is_some` check.
#[derive(Clone)]
pub struct ChipAffordance {
    pub blob_ref: SharedString,
    pub label: SharedString,
}

/// Line cap for a FETCHED full output (a defensive ceiling, not a doc cap —
/// the harness bounds outputs at 4KiB, so this is rarely reached).
pub const FULL_OUTPUT_MAX_LINES: usize = 400;

/// Build the upgraded detail from a fetched sidecar blob. Diff blobs parse
/// the `ToolDiff` JSON through the same pipeline as inline diffs; output
/// blobs render (near-)uncapped — fetching past the summary was the point.
pub fn blob_detail(text: &str, is_diff: bool) -> Option<ToolDetail> {
    if is_diff {
        let diff: openagents_proto::ToolDiff = serde_json::from_str(text).ok()?;
        return tool_detail(None, Some(&diff), None);
    }
    let mut lines: Vec<SharedString> = text.lines().map(output_line).collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return None;
    }
    let truncated_by = lines.len().saturating_sub(FULL_OUTPUT_MAX_LINES);
    lines.truncate(FULL_OUTPUT_MAX_LINES);
    Some(ToolDetail::Output {
        lines,
        truncated_by,
    })
}

/// Compact byte size for the fetch affordance label ("812 B", "12 KB").
pub fn format_kb(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

// ---------------------------------------------------------------------------
// Working indicator helpers
// ---------------------------------------------------------------------------

/// The working trailer's "Sending…" bridge: true while an in-flight send is
/// fresher than the session row's turn start — the row still carries the
/// PREVIOUS turn (or none), so a timer would count the send round-trip and
/// restart when the turn actually begins.
pub fn sending_bridge(
    send_started: Option<chrono::DateTime<chrono::Utc>>,
    turn_started: Option<chrono::DateTime<chrono::Utc>>,
) -> bool {
    match (send_started, turn_started) {
        (Some(send), Some(turn)) => turn <= send,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// Seconds the current turn has run, or `None` when its start isn't known
/// (#12204). A start at or before the Unix epoch is a missing timestamp read
/// as zero (an account chat's saved messages carry no time), and a start
/// before the chat existed can't be this chat's turn, so neither gets a
/// timer.
pub fn turn_elapsed_secs(
    started_ms: Option<i64>,
    chat_created_ms: Option<i64>,
    now_ms: i64,
) -> Option<i64> {
    let started = started_ms.filter(|ms| *ms > 0)?;
    if chat_created_ms.is_some_and(|created| created > 0 && started < created) {
        return None;
    }
    Some((now_ms - started).max(0) / 1000)
}

/// Compact elapsed formatting, using at most two units up to days.
pub fn format_elapsed(secs: i64) -> String {
    let secs = secs.max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3_600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86_400 {
        format!("{}h {}m", secs / 3_600, (secs % 3_600) / 60)
    } else {
        format!("{}d {}h", secs / 86_400, (secs % 86_400) / 3_600)
    }
}

pub fn worked_for_label(secs: i64) -> String {
    format!("Worked for {}", format_elapsed(secs))
}

/// Compact-mode header: the live tool summary crossfades into "Worked for".
/// `t` is 0 = summary, 1 = duration.
pub fn compact_work_title(
    summary: SharedString,
    worked: SharedString,
    t: f32,
    shimmer_phase: Option<f32>,
    theme: &Theme,
) -> AnyElement {
    if t >= 1.0 {
        return tool_group_title(worked, None, theme);
    }
    if t <= 0.0 {
        return tool_group_title(summary, shimmer_phase, theme);
    }
    div()
        .relative()
        .min_w_0()
        .w_full()
        .h(px(TOOL_LABEL_LINE_HEIGHT))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .h(px(TOOL_LABEL_LINE_HEIGHT))
                .flex()
                .items_center()
                .overflow_hidden()
                .opacity(1.0 - t)
                .child(tool_group_title(summary, None, theme)),
        )
        .child(
            div()
                .relative()
                .top(px(4.0 * (1.0 - t)))
                .h(px(TOOL_LABEL_LINE_HEIGHT))
                .flex()
                .items_center()
                .overflow_hidden()
                .opacity(t)
                .child(tool_group_title(worked, None, theme)),
        )
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Highlight store (background, time-sliced, paint-only)
// ---------------------------------------------------------------------------

pub struct HighlightEntry {
    pub key: DocumentHighlightKey,
    pub document: Option<Weak<openagents_syntax::HighlightedDocument>>,
    pub _task: Option<Task<()>>,
}

/// Cache of tokenized code blocks keyed by `(row id, block ix)`. Tokenization
/// runs on the background executor, time-sliced; results apply as paint-only
/// run colors when they land.
#[derive(Default)]
pub struct HighlightStore {
    pub entries: HashMap<(SharedString, usize), HighlightEntry>,
    pub cache: SyntaxHighlightCache,
}

impl HighlightStore {
    /// Current tokens if ready; kicks a background tokenize when stale/missing.
    fn request<M: TranscriptMedia>(
        &mut self,
        row_id: SharedString,
        block_ix: usize,
        lang: Lang,
        code: &str,
        cx: &mut Context<TranscriptView<M>>,
    ) -> Option<Arc<openagents_syntax::HighlightedDocument>> {
        let slot_key = (row_id.clone(), block_ix);
        let document_key = DocumentHighlightKey::new(lang, code);
        if let Some(entry) = self.entries.get(&slot_key)
            && entry.key == document_key
        {
            let document = entry.document.as_ref()?;
            if let Some(document) = document.upgrade() {
                return Some(document);
            }
        }
        if let Some(document) = self.cache.get(&document_key) {
            self.entries.insert(
                slot_key,
                HighlightEntry {
                    key: document_key,
                    document: Some(Arc::downgrade(&document)),
                    _task: None,
                },
            );
            return Some(document);
        }
        let code = code.to_string();
        let source_bytes = code.len();
        let task = cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let document = cx
                .background_executor()
                .spawn(async move {
                    openagents_syntax::highlight(openagents_syntax::HighlightRequest {
                        source: &code,
                        path: None,
                        fence_tag: Some(match lang {
                            Lang::Rust => "rust",
                            Lang::JavaScript => "javascript",
                            Lang::Jsx => "jsx",
                            Lang::TypeScript => "typescript",
                            Lang::Tsx => "tsx",
                            Lang::Python => "python",
                            Lang::Go => "go",
                            Lang::Json => "json",
                            Lang::Jsonc => "jsonc",
                            Lang::Bash => "bash",
                            Lang::Toml => "toml",
                            Lang::Markdown => "markdown",
                            Lang::Html => "html",
                            Lang::Css => "css",
                            Lang::Yaml => "yaml",
                            Lang::C => "c",
                            Lang::Cpp => "cpp",
                            Lang::CSharp => "csharp",
                            Lang::Java => "java",
                            Lang::Kotlin => "kotlin",
                            Lang::Swift => "swift",
                            Lang::Ruby => "ruby",
                            Lang::Php => "php",
                            Lang::Sql => "sql",
                            Lang::Lua => "lua",
                            Lang::Dockerfile => "dockerfile",
                            Lang::Nix => "nix",
                            Lang::Make => "make",
                        }),
                    })
                    .ok()
                })
                .await;
            this.update(cx, |transcript, cx| {
                if let Some(document) = document {
                    let document = Arc::new(document);
                    let retained = transcript
                        .highlights
                        .cache
                        .insert(document_key, document.clone());
                    if let Some(entry) = transcript.highlights.entries.get_mut(&slot_key)
                        && entry.key == document_key
                    {
                        tracing::debug!(
                            language = ?lang,
                            source_bytes,
                            spans = document.lines.iter().map(Vec::len).sum::<usize>(),
                            elapsed_us = started.elapsed().as_micros() as u64,
                            "syntax highlight ready"
                        );
                        entry.document = retained.then(|| Arc::downgrade(&document));
                        cx.notify();
                    }
                }
            })
            .ok();
        });
        self.entries.insert(
            (row_id, block_ix),
            HighlightEntry {
                key: document_key,
                document: None,
                _task: Some(task),
            },
        );
        None
    }
}

// ---------------------------------------------------------------------------
// Transcript entity
// ---------------------------------------------------------------------------

/// Expensive presentation work belongs to the subscription's background job,
/// never to a GPUI observer/render callback. Shared rows survive navigation.
#[derive(Default)]
pub struct TranscriptPreparation {
    pub entries: Vec<SessionMessageEntry>,
    pub cache: HashMap<String, Arc<Vec<Row>>>,
    pub live_parsers: HashMap<String, IncrementalParser>,
    pub tree_cache: HashMap<String, (usize, Arc<BlockTree>)>,
    pub baseline: Option<openagents_doc::TranscriptBaseline>,
}

pub struct PreparedTranscript {
    pub rows: HashMap<String, Arc<Vec<Row>>>,
    pub historical: HashMap<String, Vec<Row>>,
    pub fully_historical: HashSet<String>,
    pub navigation_baseline: Arc<openagents_doc::TranscriptBaseline>,
    pub bytes: usize,
}

impl TranscriptPreparation {
    pub fn prepare(
        &mut self,
        update: &openagents_doc::TranscriptUpdate,
    ) -> Result<Arc<PreparedTranscript>, openagents_doc::TranscriptDesync> {
        match &update.frame {
            openagents_doc::TranscriptFrame::Reset { .. } => {
                self.cache.clear();
                self.tree_cache.clear();
                self.live_parsers.clear();
            }
            openagents_doc::TranscriptFrame::Delta {
                upsert,
                append,
                remove,
                ..
            } => {
                if !upsert.is_empty() {
                    self.tree_cache.clear();
                }
                for id in upsert
                    .iter()
                    .map(|u| &u.entry.id)
                    .chain(append.iter().map(|a| &a.entry))
                    .chain(remove.iter())
                {
                    self.cache.remove(id);
                }
            }
        }
        openagents_doc::apply_transcript_frame(&mut self.entries, update.frame.clone())?;
        if let Some(baseline) = &update.replay_baseline {
            self.baseline = Some(baseline.clone());
        }
        let mut rows = HashMap::new();
        let mut historical = HashMap::new();
        let mut fully_historical = HashSet::new();
        let mut bytes = 0;
        for entry in &self.entries {
            let built = if let Some(rows) = self.cache.get(&entry.id) {
                rows.clone()
            } else {
                let streaming = entry.status == Some(MessageStatus::Streaming);
                let built = Arc::new(rows_for_entry(entry, false, false, &mut |key, text| {
                    parse_for_row(
                        streaming,
                        key,
                        text,
                        &mut self.live_parsers,
                        &mut self.tree_cache,
                    )
                    .0
                }));
                self.cache.insert(entry.id.clone(), built.clone());
                built
            };
            rows.insert(entry.id.clone(), built);
            if self.baseline.as_ref().is_some_and(|b| b.covers(entry)) {
                fully_historical.insert(entry.id.clone());
            }
            if let Some(baseline) = &self.baseline
                && !fully_historical.contains(&entry.id)
                && let Some(prefix) = baseline.historical_entry(entry)
            {
                historical.insert(
                    entry.id.clone(),
                    rows_for_entry(&prefix, false, false, &mut |_, text| {
                        Arc::new(parse_full(text))
                    }),
                );
            }
            bytes += std::mem::size_of::<SessionMessageEntry>()
                + entry.id.len()
                + entry
                    .parts
                    .iter()
                    .map(|part| std::mem::size_of::<MessagePart>() + part.byte_len())
                    .sum::<usize>();
        }
        self.cache.retain(|id, _| rows.contains_key(id));
        Ok(Arc::new(PreparedTranscript {
            rows,
            historical,
            fully_historical,
            bytes,
            navigation_baseline: Arc::new(openagents_doc::TranscriptBaseline::capture(
                &self.entries,
            )),
        }))
    }
}

pub struct CachedRows {
    pub fingerprint: u64,
    pub rows: Vec<Row>,
}

#[derive(Default, Clone, Copy)]
pub struct FoldState {
    /// User pin (click); `None` follows the auto-open rule.
    pub open: Option<bool>,
    /// Bumped per toggle — keys the 200ms height tween.
    pub epoch: usize,
    /// Height at the moment of the toggle (the tween's start). The destination
    /// is always the *current* target height, so content growth after a toggle
    /// snaps instead of replaying a stale tween.
    pub from: f32,
    /// When the toggle happened. The tween is armed only for a short window
    /// after the click: gpui replays an element's animation on REMOUNT, and a
    /// virtualized row scrolling back into view is a remount — an armed-forever
    /// tween made every once-collapsed group flash open→closed on each
    /// reappearance (user report).
    pub toggled_at: Option<Instant>,
    pub disclosure_at: Option<Instant>,
    /// Per-toggle duration. User bubbles scale this with travel distance;
    /// existing tool folds leave it at zero and keep their catalog constants.
    pub duration_ms: u64,
    /// Extra user-body height revealed by Show more. It is not reply growth
    /// and must not permanently consume the sent turn's reservation.
    pub user_expansion_height: f32,
}

/// Reveal epochs for one live ordinary tool group. `None` means the row was
/// already present when this transcript attached (or has finished revealing),
/// so replaying history and scrolling a virtualized row back into view stay
/// completely still.
#[derive(Default)]
pub struct ToolGroupReveal {
    /// A newly streamed task header participates in the same height/fade/lift
    /// reveal as its steps. Replayed headers leave this unset.
    pub header_started_at: Option<Instant>,
    pub starts: Vec<Option<Instant>>,
    /// A title sweep begins with this group instead of inheriting the shared
    /// loader clock at an arbitrary point midway across the label.
    pub shimmer_started_at: Option<Instant>,
    pub rendered_open: Option<bool>,
    pub rendered_height: f32,
}

pub fn tool_row_reveal_progress(start: Option<Instant>, now: Instant, reduce_motion: bool) -> f32 {
    let Some(start) = start.filter(|_| !reduce_motion) else {
        return 1.0;
    };
    let elapsed = now.checked_duration_since(start).unwrap_or_default();
    let raw = elapsed.as_secs_f32() / TOOL_ROW_REVEAL.total().as_secs_f32();
    TOOL_ROW_REVEAL.curve.eval(raw)
}

pub fn tool_connector_reveal_progress(
    start: Option<Instant>,
    now: Instant,
    reduce_motion: bool,
) -> f32 {
    let Some(start) = start.filter(|_| !reduce_motion) else {
        return 1.0;
    };
    let elapsed = now.checked_duration_since(start).unwrap_or_default();
    let raw = elapsed.as_secs_f32() / TOOL_CONNECTOR_REVEAL.total().as_secs_f32();
    TOOL_CONNECTOR_REVEAL.curve.eval(raw)
}

/// Split one arrival into a continuous tree draw. For child rows, the previous
/// row grows its continuation to the boundary first, then this row draws its
/// incoming trunk and elbow. The branch slightly overlaps the end of the
/// incoming phase so there is no dead frame at the bend.
pub fn tool_connector_parts(progress: f32, has_predecessor: bool) -> (f32, f32) {
    let progress = progress.clamp(0.0, 1.0);
    let (incoming_start, incoming_end, branch_start) = if has_predecessor {
        (0.45, 0.72, 0.68)
    } else {
        (0.0, 0.62, 0.58)
    };
    let incoming = ((progress - incoming_start) / (incoming_end - incoming_start)).clamp(0.0, 1.0);
    let branch = ((progress - branch_start) / (1.0 - branch_start)).clamp(0.0, 1.0);
    (incoming, branch)
}

/// The outgoing trunk belongs visually to the row that is already present,
/// but its timing belongs to the next row's arrival.
pub fn tool_connector_continuation(next_progress: Option<f32>) -> f32 {
    next_progress
        .map(|progress| (progress / 0.45).clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

/// Reveal by distance along the elbow and straight leg, so changing the branch
/// length does not introduce a speed jump at their junction.
pub fn activity_branch_points(progress: f32) -> Vec<Point<f32>> {
    let mut path: Vec<_> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.0;
            point(
                ACTIVITY_BEND_RADIUS * t * t,
                ACTIVITY_BEND_RADIUS * (2.0 * t - t * t),
            )
        })
        .collect();
    path.push(point(
        ACTIVITY_BRANCH_END_X - ACTIVITY_TRUNK_X,
        ACTIVITY_BEND_RADIUS,
    ));
    if progress >= 1.0 {
        return path;
    }
    let lengths: Vec<_> = path
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .collect();
    let mut remaining = lengths.iter().sum::<f32>() * progress.clamp(0.0, 1.0);
    let mut visible = vec![path[0]];
    for (pair, length) in path.windows(2).zip(lengths) {
        if remaining <= 0.0 {
            break;
        }
        let t = (remaining / length).min(1.0);
        visible.push(point(
            motion::lerp(pair[0].x, pair[1].x, t),
            motion::lerp(pair[0].y, pair[1].y, t),
        ));
        remaining -= length;
    }
    visible
}

pub fn tool_disclosure_progress(open: bool, fold: FoldState, now: Instant) -> f32 {
    let Some(start) = fold.disclosure_at else {
        return if open { 1.0 } else { 0.0 };
    };
    let raw = now
        .checked_duration_since(start)
        .unwrap_or_default()
        .as_secs_f32()
        / TOOL_FOLD.total().as_secs_f32();
    let progress = TOOL_FOLD.curve.eval(raw);
    if open { progress } else { 1.0 - progress }
}

/// The compact work fold's animated body budget: `fold.from` lerps toward
/// the rows' measured total while open (zero when closed) under the shared
/// [`TOOL_FOLD`] spec. Each sibling body row clips to `budget - prefix`, so
/// the group opens/closes exactly like an ordinary tool fold's single
/// overflow-hidden container — including reversals, which seed `from` with
/// the budget in flight.
pub fn compact_body_height(fold: FoldState, total: f32, reduce_motion: bool, now: Instant) -> f32 {
    let target = if fold.open.unwrap_or(false) {
        total
    } else {
        0.0
    };
    let Some(at) = fold.toggled_at.filter(|_| !reduce_motion) else {
        return target;
    };
    let t = TOOL_FOLD
        .curve
        .eval(now.saturating_duration_since(at).as_secs_f32() / TOOL_FOLD.total().as_secs_f32());
    motion::lerp(fold.from, target, t)
}

/// Clip geometry for one compact-fold body row: `(prefix, own, total)`
/// natural heights over the contiguous `work_id` run containing `ix`.
/// Unmeasured rows report 0 until their prepaint probe lands — they stay
/// clipped for that one pass instead of flashing open at full height.
pub fn compact_fold_geometry(
    rows: &[Row],
    heights: &HashMap<SharedString, Rc<Cell<f32>>>,
    work_id: &SharedString,
    ix: usize,
) -> (f32, f32, f32) {
    let mut start = ix;
    while start > 0 && rows[start - 1].compact_fold.as_ref() == Some(work_id) {
        start -= 1;
    }
    let mut prefix = 0.0;
    let mut own = 0.0;
    let mut total = 0.0;
    for (j, row) in rows.iter().enumerate().skip(start) {
        if row.compact_fold.as_ref() != Some(work_id) {
            break;
        }
        let height = heights.get(&row.id).map_or(0.0, |cell| cell.get());
        if j < ix {
            prefix += height;
        } else if j == ix {
            own = height;
        }
        total += height;
    }
    (prefix, own, total)
}

/// BoardUI's measured recipe: a 300%-wide repeating gradient moves from 200%
/// to -100%. Its 38→50→62% highlight maps to a 36%-of-title shoulder around
/// each peak; adjacent copies sit three title-widths apart. Sampling this by
/// x-coordinate lets the paint clips reproduce the continuous pattern.
pub fn tool_title_shimmer_amount(x: f32, phase: f32) -> f32 {
    let primary_center = -2.5 + phase.clamp(0.0, 1.0) * 6.0;
    (-2..=2)
        .map(|copy| primary_center + copy as f32 * 3.0)
        .map(|center| (1.0 - (x - center).abs() / TOOL_GROUP_SHIMMER_HALF_WIDTH).clamp(0.0, 1.0))
        .fold(0.0, f32::max)
}

pub fn tool_title_shimmer_phase(start: Instant, now: Instant) -> f32 {
    let elapsed = now.checked_duration_since(start).unwrap_or_default();
    (elapsed.as_secs_f32() / TOOL_GROUP_SHIMMER_DURATION.as_secs_f32()).fract()
}

/// Viewport compensation paired with a long user-message collapse. While the
/// row loses height, this scrolls upward by the same eased distance so a
/// bottom-pinned viewport keeps the collapsing bubble in view.
pub struct UserCollapseScroll {
    pub started_at: Instant,
    pub duration_ms: u64,
    pub height_delta: f32,
    pub row_ix: usize,
    pub initial_top: f32,
    pub target_top: f32,
}

/// A locally-sent turn reserves the viewport below its prompt. The last row
/// has a minimum height, so streaming content and the working trailer consume
/// or release space in the same layout pass. Only changes to the preceding
/// rows require a post-layout refinement. Wheel input releases the automatic
/// glide/hold while preserving the reservation; overflow hands off to the
/// ordinary bottom spring. Chat switches restore the reservation released.
#[derive(Clone, Debug)]
pub struct OwnTurnAnchor {
    pub chat_id: String,
    pub message_id: SharedString,
    /// The step still owns the viewport (glide → hold). Any wheel/touch
    /// input releases it — the reservation stays behind as plain scrollable
    /// space, and the ordinary escape/restick rules apply from then on.
    pub held: bool,
    /// The entry glide has landed; the hold now re-asserts the prompt's
    /// position absolutely after every layout (glue- and lag-proof — the
    /// exact mechanism the shipped first-send anchor used).
    pub positioned: bool,
    /// A fresh send may install the anchor one notification before its echo.
    /// Once the prompt has appeared, its later disappearance is terminal
    /// (failed echo or removed entry) and the runway must retire.
    pub seen_prompt: bool,
}

impl OwnTurnAnchor {
    fn released_for_restore(mut self) -> Self {
        self.held = false;
        self.positioned = false;
        self.seen_prompt = true;
        self
    }

    pub fn observe_prompt(&mut self, exists: bool) -> bool {
        if exists {
            self.seen_prompt = true;
        }
        exists || !self.seen_prompt
    }
}

/// Locally-authored queue rows whose stable ids have not appeared in the
/// transcript yet. Registration is deliberately inert: adding a queue row
/// must leave the currently-visible turn and its runway untouched. Once a
/// matching prompt materializes, the newest match becomes the own-turn anchor.
#[derive(Default)]
pub struct PendingQueuedTurns {
    pub items: VecDeque<(String, SharedString)>,
}

impl PendingQueuedTurns {
    pub fn register(&mut self, chat_id: String, message_id: String) {
        self.items
            .retain(|(chat, id)| chat != &chat_id || id.as_ref() != message_id);
        self.items
            .push_back((chat_id, SharedString::from(message_id)));
        while self.items.len() > MAX_PENDING_QUEUED_TURNS {
            self.items.pop_front();
        }
    }

    /// Consume every candidate from this chat that is now present and return
    /// the newest one. Multiple rows can land in one doc frame; the last send
    /// owns the runway, matching consecutive immediate sends.
    pub fn take_latest_materialized(&mut self, chat_id: &str, rows: &[Row]) -> Option<String> {
        let mut latest = None;
        self.items.retain(|(chat, message_id)| {
            let materialized = chat == chat_id
                && rows
                    .iter()
                    .any(|row| row.turn_start && row.entry_id == message_id.as_ref());
            if materialized {
                latest = Some(message_id.to_string());
            }
            !materialized
        });
        latest
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// A stable per-chat viewport anchor. Row identity is preferred over its old
/// index because async replay can insert or remove rows while a chat is away.
#[derive(Clone, Debug)]
pub struct ViewportAnchor {
    pub row_id: SharedString,
    pub entry_id: SharedString,
    pub fallback_ix: usize,
    pub offset_in_row: Pixels,
}

impl ViewportAnchor {
    pub fn capture(rows: &[Row], scroll_top: ListOffset) -> Option<Self> {
        let fallback_ix = scroll_top.item_ix.min(rows.len().checked_sub(1)?);
        let row = &rows[fallback_ix];
        Some(Self {
            row_id: row.id.clone(),
            entry_id: row.entry_id.clone(),
            fallback_ix,
            offset_in_row: scroll_top.offset_in_item,
        })
    }

    fn resolve_exact(&self, rows: &[Row]) -> Option<ListOffset> {
        let item_ix = rows.iter().position(|row| row.id == self.row_id)?;
        Some(ListOffset {
            item_ix,
            offset_in_item: self.offset_in_row,
        })
    }

    pub fn resolve(&self, rows: &[Row]) -> Option<ListOffset> {
        if let Some(offset) = self.resolve_exact(rows) {
            return Some(offset);
        }

        // A row can disappear when a streaming block is reshaped. Stay in the
        // same message entry, choosing the surviving row nearest the old
        // location; the intra-row offset is no longer meaningful in that case.
        let item_ix = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.entry_id == self.entry_id)
            .min_by_key(|(ix, _)| ix.abs_diff(self.fallback_ix))
            .map(|(ix, _)| ix)
            .unwrap_or_else(|| self.fallback_ix.min(rows.len().saturating_sub(1)));
        (!rows.is_empty()).then_some(ListOffset {
            item_ix,
            offset_in_item: px(0.0),
        })
    }
}

/// Session-local viewport state. Chats that were following their tail keep
/// following it; only user-owned viewports restore a concrete row anchor.
#[derive(Clone, Debug)]
pub enum SavedViewport {
    FollowTail,
    Anchored {
        anchor: ViewportAnchor,
        distance_from_bottom: f32,
        /// Preserve the runway that made a short active turn scrollable.
        /// Navigation releases its automatic hold, so revisiting restores the
        /// viewport without immediately following new output to the bottom.
        own_turn: Option<OwnTurnAnchor>,
    },
}

pub struct RestoredViewport {
    pub offset: ListOffset,
    pub distance_from_bottom: f32,
    pub own_turn: Option<OwnTurnAnchor>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewportFinalizeToken {
    pub generation: u64,
    pub layout_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranscriptReplayState {
    Pending,
    Empty,
    Populated,
}

impl TranscriptReplayState {
    pub fn authoritative_empty(self) -> bool {
        self == Self::Empty
    }

    pub fn allows_fallback(self) -> bool {
        self == Self::Populated
    }
}

impl ViewportFinalizeToken {
    pub fn still_current(self, generation: u64) -> bool {
        self.generation == generation
    }

    pub fn layout_settled(self, layout_revision: u64) -> bool {
        self.layout_revision == layout_revision
    }
}

impl SavedViewport {
    pub fn capture(
        rows: &[Row],
        scroll_top: ListOffset,
        pinned: bool,
        distance_from_bottom: f32,
        own_turn: Option<&OwnTurnAnchor>,
    ) -> Option<Self> {
        if rows.is_empty() {
            return None;
        }
        if pinned {
            return Some(Self::FollowTail);
        }
        Some(Self::Anchored {
            anchor: ViewportAnchor::capture(rows, scroll_top)?,
            distance_from_bottom,
            own_turn: own_turn.cloned(),
        })
    }

    /// Before the opening reset arrives, rows may contain only optimistic
    /// echoes. In that gap an exact row is safe, but entry/index fallbacks
    /// would mistake an unrelated echo for the authoritative transcript.
    pub fn resolve(&self, rows: &[Row], allow_fallback: bool) -> Option<RestoredViewport> {
        let Self::Anchored {
            anchor,
            distance_from_bottom,
            own_turn,
        } = self
        else {
            return None;
        };
        let offset = if allow_fallback {
            anchor.resolve(rows)?
        } else {
            anchor.resolve_exact(rows)?
        };
        let own_turn = own_turn
            .clone()
            .filter(|turn| {
                rows.iter()
                    .any(|row| row.turn_start && row.entry_id == turn.message_id)
            })
            .map(OwnTurnAnchor::released_for_restore);
        Some(RestoredViewport {
            offset,
            distance_from_bottom: *distance_from_bottom,
            own_turn,
        })
    }
}

#[derive(Default)]
pub struct SavedViewportCache {
    pub by_chat: HashMap<String, SavedViewport>,
    pub recency: VecDeque<String>,
}

impl SavedViewportCache {
    pub fn insert(&mut self, chat_id: String, viewport: SavedViewport) {
        if self.by_chat.contains_key(&chat_id) {
            self.recency.retain(|candidate| candidate != &chat_id);
        }
        self.recency.push_back(chat_id.clone());
        self.by_chat.insert(chat_id, viewport);
        while self.by_chat.len() > MAX_SAVED_VIEWPORTS {
            let Some(evicted) = self.recency.pop_front() else {
                break;
            };
            self.by_chat.remove(&evicted);
        }
    }

    pub fn get_cloned_and_touch(&mut self, chat_id: &str) -> Option<SavedViewport> {
        let viewport = self.by_chat.get(chat_id).cloned()?;
        self.recency.retain(|candidate| candidate != chat_id);
        self.recency.push_back(chat_id.to_string());
        Some(viewport)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn len(&self) -> usize {
        self.by_chat.len()
    }
}

pub struct TranscriptView<M: TranscriptMedia> {
    pub host: Rc<dyn TranscriptHost>,
    pub list: ListState,
    pub rows: Vec<Row>,
    pub last_source: Option<(Option<String>, TranscriptReplayState, u64)>,
    pub chat_id: Option<String>,
    /// The shell may retain this already-laid-out view briefly for its exit.
    /// Cleared as soon as the exit is invisible; never used for another chat.
    pub retain_on_deselect: bool,
    /// `Some(doc_id)` pins this instance to a SUBAGENT doc: rows come from
    /// the host's entries for `doc_id` instead of the selected chat, and
    /// the instance is READ-ONLY — no echoes, no own-turn hold, and no global
    /// attachment protection (that set is shared with the primary transcript
    /// and overwritten wholesale).
    pub doc_override: Option<String>,
    /// Whether an override instance watches a LIVE doc (`for_doc(follow)`):
    /// only then may the working trailer render — a frozen snapshot must
    /// never spin, whatever its entries claim.
    pub doc_live: bool,
    /// Memory-only viewport state for primary chats visited in this window.
    /// A transcript instance is shared across tabs, so the active ListState is
    /// reset on every attach and cannot retain these positions by itself.
    pub saved_viewports: SavedViewportCache,
    /// An anchored viewport waiting for the selected chat's async replay.
    pub pending_viewport: Option<SavedViewport>,
    /// Generation of the selected chat, guarding post-layout restoration
    /// callbacks across rapid A→B→A navigation.
    pub viewport_generation: u64,
    /// A restored item anchor needs one post-layout refresh of distance-based
    /// UI state; programmatic list scrolling never invokes `handle_scroll`.
    pub viewport_finalize_pending: bool,
    pub viewport_finalize_scheduled: bool,
    /// Bumped whenever sync or own-turn logic invalidates measured rows. The
    /// post-restore finalizer waits until one layout completes without another
    /// invalidation, avoiding a stale jump-button decision.
    pub viewport_layout_revision: u64,
    /// One-shot "open at the latest content" for UNPINNED (frozen) override
    /// instances: rows land ASYNC after the tab opens (watch replay / blob
    /// fetch), so the end-scroll fires on the first non-empty sync, then
    /// never again — landing at the end and FOLLOWING it are different
    /// states, and the user owns the viewport from there. Pinned instances
    /// don't need it (the pin branch already opens at the end).
    pub land_end_pending: bool,
    pub row_cache: HashMap<String, CachedRows>,
    pub live_parsers: HashMap<String, IncrementalParser>,
    pub tree_cache: HashMap<String, (usize, Arc<BlockTree>)>,
    pub folds: HashMap<SharedString, FoldState>,
    /// Entrance state follows stable groups through completion so fast calls
    /// finish revealing. Replay rows have no entrance timestamps.
    pub tool_group_reveals: HashMap<SharedString, ToolGroupReveal>,
    pub last_replay_baseline: Option<Arc<openagents_doc::TranscriptBaseline>>,
    /// Parsed historical prefixes, used to seed text before a coalesced live
    /// suffix is painted. The wire watermark contains lengths, not text.
    pub historical_markdown: HashMap<SharedString, Row>,
    /// Detail folds (output/diff) per chip, keyed `"{row_id}#d{ix}"` — full
    /// [`FoldState`]s so detail bodies tween open/closed exactly like the
    /// group fold. Render-local like `folds` — never part of the row
    /// fingerprint.
    pub tool_details: HashMap<SharedString, FoldState>,
    /// Expand/collapse state for user bubbles past [`USER_COLLAPSED_LINES`],
    /// keyed by row id. Render-local like `folds` — never part of the row
    /// fingerprint, so toggling one costs a repaint, not a rebuild.
    pub user_folds: HashMap<SharedString, FoldState>,
    /// Full laid-out text heights for long user bubbles. The text's paint
    /// canvas writes these cells without notifying or mutating the transcript;
    /// click handlers read them as exact endpoints for the RESIZE tween. This
    /// preserves smooth layout motion without a paint → notify feedback loop.
    pub user_heights: HashMap<SharedString, Rc<Cell<f32>>>,
    /// Pending long-press toggle. A single task is enough because only one
    /// pointer can own a hold gesture at a time; a token invalidates stale
    /// timers when the pointer is released or moves into a text selection.
    pub user_hold_task: Option<Task<()>>,
    pub user_hold_token: u64,
    pub user_collapse_scroll: Option<UserCollapseScroll>,
    /// Tracks the queued frame, even when its animation is canceled/replaced.
    /// Only that callback clears it, so rapid input cannot fork frame drivers.
    pub user_collapse_scroll_scheduled: bool,
    /// Streaming fade veils, one per live markdown row (dropped on completion).
    pub veils: HashMap<SharedString, Rc<RefCell<RowVeil>>>,
    /// Live rows present in the transcript's REPLAY after (re)attaching to a
    /// chat: their veils are created pre-seeded, so text that was already
    /// streamed before the switch never fades in — only appends after it do
    /// (mugen's `FadePainter.attach` baseline; user report: switching back to
    /// a streaming session dissolved the entire reply).
    pub veil_baseline: std::collections::HashSet<SharedString>,
    /// Armed at attach, disarmed on the first sync whose transcript is
    /// non-empty: the baseline must be captured from the doc REPLAY frame,
    /// not the attach-time sync — selection clears the transcript and the
    /// replay lands async, so capturing at attach seeded nothing and the
    /// still-streaming reply faded in whole on every session switch (user
    /// report, round 2).
    pub veil_attach_pending: bool,
    /// Cross-frame flatten/shape-input cache (see [`RenderCache`]): fade
    /// frames reuse settled blocks' text+runs; the incremental parser's stable
    /// boundary invalidates only the live tail per commit.
    pub render_cache: Rc<RefCell<RenderCache>>,
    pub workspace_link: Option<render::LinkUi>,
    /// File-link roots per linking chat, valid for one
    /// the host's link-roots revision: every rendered row asks for them.
    pub file_link_roots: (
        u64,
        HashMap<SharedString, Rc<Vec<crate::workspace_links::FileLinkRoot>>>,
    ),
    /// Inline code spans that name an existing file, rewritten into the
    /// Markdown links they stand for per text part (see
    /// [`crate::markdown::inline_code_links`]). Reset with the same
    /// link-roots revision that resets `file_link_roots`.
    pub inline_code_links: crate::markdown::inline_code_links::InlineCodeLinkCache,
    pub rendered_rows: HashSet<SharedString>,
    /// Last UI typography generation reflected in `list` item measurements.
    /// Family and size changes can alter prose wrapping without changing row
    /// identity, so the virtual list must explicitly discard cached heights.
    pub typography_generation: u32,
    pub content_width: f32,
    /// Width held free at the column's right for a docked side card; the
    /// reading column centers in what remains.
    pub right_reserve: f32,
    /// Last global code-fence layout generation applied to this transcript.
    /// Each instance owns separate scroll handles and list measurements, so
    /// every one must reset itself after a global Fit-mode transition.
    pub code_fences_generation: u64,
    /// Compact mode as last applied to this instance's row split. Render
    /// polls the setting each frame; a flip rebuilds every row.
    pub compact_mode: bool,
    /// Compact work-group entry ids that were live this session — the
    /// "Worked for" subtitle fades in only for those, not historical rows.
    pub compact_live_entries: HashSet<SharedString>,
    /// Last live elapsed (seconds) per assistant entry, used if `duration_ms`
    /// has not landed on the doc yet when the turn settles.
    pub compact_last_elapsed: HashMap<String, i64>,
    /// When a compact work group first settled this session, so "Worked for"
    /// can fade in without replaying on later paints.
    pub compact_worked_fade_at: HashMap<SharedString, Instant>,
    /// Natural heights of compact-fold body rows, written by each row's
    /// prepaint probe without notifying. The work fold's TOOL_FOLD tween
    /// clips the sibling rows against one shared budget without knowing
    /// their layout up front.
    pub compact_fold_heights: HashMap<SharedString, Rc<Cell<f32>>>,
    /// Post-tween sweep that drops a closed work fold's retained body rows.
    pub compact_fold_settle: Option<Task<()>>,
    pub highlights: HighlightStore,
    pub show_jump_button: bool,
    /// Distance from the bottom at the last observation (wheel event or spring
    /// tick) — restick and escape are direction-aware
    /// (see [`Transcript::should_restick`]).
    pub last_scroll_distance: f32,
    /// The stick-to-bottom pin. Broken only by user input (wheel/touch up);
    /// re-engaged inside the 70px band, after an own-send first overflows, and
    /// on the jump button.
    pub pinned: bool,
    /// A locally-sent prompt currently held near the viewport top while its
    /// reply grows into the empty space below it.
    pub own_turn: Option<OwnTurnAnchor>,
    /// Queue rows authored in this window. They become own-turn anchors only
    /// after the host promotes their stable id into a transcript message.
    pub pending_queued_turns: PendingQueuedTurns,
    /// A layout-affecting change needs one post-layout own-turn measurement.
    pub own_turn_kick: bool,
    /// One own-turn `on_next_frame` callback in flight at most.
    pub own_turn_scheduled: bool,
    /// Wall-clock of the previous entry-glide tick (`None` = not gliding).
    pub own_turn_last_tick: Option<Instant>,
    pub spring: StickSpring,
    /// Wall-clock of the previous spring tick (`None` = parked).
    pub spring_last_tick: Option<Instant>,
    /// When the spring last landed on the bottom (settle-grace bookkeeping).
    pub spring_settled_at: Option<Instant>,
    /// A doc commit / wake happened before layout measured it — run at least
    /// one spring tick even though the pre-layout distance still reads 0.
    pub spring_kick: bool,
    /// One `on_next_frame` callback in flight at most.
    pub spring_scheduled: bool,
    pub scroll_anim: Option<Task<()>>,
    /// Last pointer sample while markdown selection owns a left-button drag.
    pub selection_drag_position: Option<Point<Pixels>>,
    /// One-shot timer rescheduled only while the pointer remains in an edge
    /// zone. Dropping it on mouse-up stops all selection scroll work.
    pub selection_scroll_task: Option<Task<()>>,
    /// MessageRail width gate (set by the shell from the container width).
    pub rail_enabled: bool,
    /// Height of the shell's composer/status/terminal stack overlaying the
    /// transcript's bottom (measured last frame): the last row pads past it
    /// so pinned content rests above the glass chrome it scrolls under.
    pub bottom_clearance: f32,
    /// How a host without the desktop's window chrome lays the list out.
    pub host_layout: Option<HostLayout>,
    /// Hovered rail tick (grows + shows the preview card).
    pub rail_hover: Option<usize>,
    pub rail_cache: Option<crate::rail::RailCache>,
    /// `(row id, entry id)` under the pointer — reveals the entry's timestamp
    /// strip (openagents chat-view.tsx `group-hover`; the rows report hover
    /// themselves). Keyed by ROW so a row→row move within one entry can't
    /// clear the reveal when the old row's leave event arrives after the new
    /// row's enter (enter/leave order across rows is not guaranteed).
    pub hovered_entry: Option<(SharedString, SharedString)>,
    /// Code block showing "Copied" feedback: `(row id, block ix)`, cleared by
    /// the companion task after ~1.2s.
    pub copied_code: Option<(SharedString, usize)>,
    pub copied_clear: Option<Task<()>>,
    /// Per-visible-fence horizontal offsets and scrollbar hover/drag state.
    /// Keys use the transcript's stable row identity, so streaming → settled
    /// rerenders keep their local scroll position without leaking state for
    /// blocks no longer present in the selected chat.
    pub code_fences: HashMap<SharedString, render::CodeFenceRuntime>,
    /// Entry whose hover action is showing transient copied-check feedback.
    pub copied_message: Option<SharedString>,
    pub copied_message_clear: Option<Task<()>>,
    /// Sidecar blob fetches keyed by doc ref (`chatId/partId[.diff]`,
    /// chat2-sync A3). `Ready` holds the UPGRADED detail, built once on
    /// arrival — render swaps it in per chip; rows never rebuild for it.
    /// Deliberately NOT cleared on chat switch: refs are chat-qualified and a
    /// fetched blob stays valid.
    pub blob_details: HashMap<SharedString, BlobFetch>,
    /// Monotonic fetch order per blob ref: when a tool has BOTH a diff and
    /// an output blob fetched, the chip shows the one requested most
    /// recently (click "Show full output" after a diff → see the output).
    pub blob_fetch_order: HashMap<SharedString, u64>,
    pub blob_fetch_counter: u64,
    pub _host_watch: Vec<Subscription>,
    /// This transcript's pictures and other media.
    pub media: M,
}

/// One sidecar blob fetch's lifecycle.
pub enum BlobFetch {
    Loading(#[allow(dead_code)] Task<()>),
    /// Failed with the affordance re-armed as a retry.
    Failed,
    Ready(Arc<ToolDetail>),
}

/// Shell-facing events (the transcript itself hosts no surfaces).
#[derive(Debug, Clone)]
pub enum TranscriptEvent {
    /// A spawn chip's "Open subagent" affordance: open the subagent's
    /// transcript as a right-pane tab. `chat_id` is the doc the chip lives
    /// in (the frozen blob is keyed `{chat_id}/{doc_id}`); `frozen` means
    /// the subagent finished — try the blob before watching the doc.
    OpenSubagent {
        chat_id: String,
        doc_id: String,
        title: String,
        frozen: bool,
    },
}

impl<M: TranscriptMedia> gpui::EventEmitter<TranscriptEvent> for TranscriptView<M> {}

impl<M: TranscriptMedia> TranscriptView<M> {
    pub fn set_workspace_link_handler(&mut self, handler: render::LinkUi) {
        self.workspace_link = Some(handler);
    }

    pub fn link_ui(&mut self, cx: &mut Context<Self>) -> Option<render::LinkUi> {
        let source = self
            .workspace_link
            .as_ref()
            .and_then(|link| link.source_session.clone())
            .or_else(|| self.chat_id.clone())?;
        self.link_ui_for(&SharedString::from(source), cx)
    }

    /// The workspace-link handler bound to `source_chat_id`: the linking
    /// chat's own checkout resolves first, then its parent's and this
    /// device's project roots, and the roots come along for the trailing
    /// open glyph and the file menu.
    fn link_ui_for(
        &mut self,
        source_chat_id: &SharedString,
        cx: &mut Context<Self>,
    ) -> Option<render::LinkUi> {
        let roots = self.file_link_roots(source_chat_id, cx);
        let source_local = self.host.chat_is_local(source_chat_id, cx);
        self.workspace_link.clone().map(|mut link| {
            link.source_session = Some(source_chat_id.to_string());
            link.source_local = source_local;
            link.file_roots = Some(roots);
            link
        })
    }

    /// The ordered checkouts a file link from `chat_id` may open against:
    /// the chat's own, its parent's, then this device's project roots. The
    /// memo keeps row rendering from rebuilding them every frame; a state
    /// change that can move a root clears it.
    fn file_link_roots(
        &mut self,
        chat_id: &SharedString,
        cx: &gpui::App,
    ) -> Rc<Vec<crate::workspace_links::FileLinkRoot>> {
        let host = self.host.clone();
        let (revision, memo) = &mut self.file_link_roots;
        let current = host.link_roots_revision(cx);
        if *revision != current {
            *revision = current;
            memo.clear();
        }
        memo.entry(chat_id.clone())
            .or_insert_with(|| Rc::new(host.file_link_roots(chat_id, cx)))
            .clone()
    }

    /// The row's text with every inline code span that names an existing
    /// file rewritten into the Markdown link it stands for (see
    /// [`crate::markdown::inline_code_links`]). The walk is memoized per part
    /// and link-roots revision; a revision change also drops the flatten
    /// cache, whose entries were shaped with the previous roots' styling.
    fn inline_code_tree(
        &mut self,
        tree: &Arc<BlockTree>,
        ui: Option<&render::LinkUi>,
        cx: &gpui::App,
    ) -> Arc<BlockTree> {
        let Some(ui) = ui else {
            return tree.clone();
        };
        let Some(roots) = ui.file_roots.as_deref() else {
            return tree.clone();
        };
        let revision = self.host.link_roots_revision(cx);
        if self.inline_code_links.set_revision(revision) {
            self.render_cache.borrow_mut().clear();
        }
        self.inline_code_links
            .linked_tree(tree, roots, ui.source_local)
    }

    /// File badges on tool chips open through the same workspace-link route
    /// as Markdown file links, so only chats with a link handler get
    /// clickable badges.
    fn tool_file_opener(&mut self, cx: &mut Context<Self>) -> Option<ToolFileOpener> {
        self.link_ui(cx).map(|link| ToolFileOpener { link })
    }

    pub fn new(host: Rc<dyn TranscriptHost>, cx: &mut Context<Self>) -> Self {
        Self::build(host, None, true, cx)
    }

    /// A read-only transcript over one SUBAGENT doc (right-pane tab). The
    /// caller starts the feed (`watch_subagent_doc` or the frozen snapshot);
    /// this instance only renders whatever lands under `doc_id`. `follow` =
    /// the doc is live: engage the end-follow pin from the start. Either
    /// way the tab OPENS at the latest content — a frozen transcript lands
    /// at the end once, unpinned, and free-scrolls from there.
    pub fn for_doc(
        host: Rc<dyn TranscriptHost>,
        doc_id: String,
        follow: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(host, Some(doc_id), follow, cx)
    }

    fn build(
        host: Rc<dyn TranscriptHost>,
        doc_override: Option<String>,
        follow: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        // FollowMode stays Normal: the tail pin is ours (a per-frame spring),
        // not the list's per-layout hard snap.
        //
        // Override instances align TOP: a subagent transcript reads like a
        // fresh notes page — entries anchored at the top, streaming growing
        // into the empty space below, never rising from the pane's bottom.
        // Top alignment gets that structurally (a short list rests at the
        // top with no reservation pad), and the PIN machinery still runs on
        // top of it for end-follow: the spring is purely distance-based, and
        // the glue trap it was built around is Bottom-only — layout
        // materializes a Top list's past-end offset to a CONCRETE position
        // every frame (gpui list.rs: only `Bottom` re-glues to the `None`
        // sentinel), so a parked spring can't re-glue and hard-track growth.
        let alignment = if doc_override.is_some() {
            ListAlignment::Top
        } else {
            ListAlignment::Bottom
        };
        let list = ListState::new(0, alignment, px(OVERDRAW_PX));
        let weak = cx.weak_entity();
        list.set_scroll_handler(move |event: &ListScrollEvent, _window, cx| {
            weak.update(cx, |this: &mut Self, cx| this.handle_scroll(event, cx))
                .ok();
        });
        let host_watch = {
            let weak = cx.weak_entity();
            host.clone().watch(
                Rc::new(move |doc_id, cx| {
                    weak.update(cx, |this, cx| this.host_changed(doc_id, cx))
                        .ok();
                }),
                cx,
            )
        };
        cx.on_release(|this: &mut Self, cx| M::released(this, cx))
            .detach();
        // The rail is sized for the conversation column; a narrow right-pane
        // tab has no width gate driving it, so override instances skip it.
        let rail_enabled = doc_override.is_none();
        // `follow` is the initial pin: the primary transcript always opens
        // pinned; an override instance pins only while its doc is LIVE (a
        // frozen transcript reads top-down, free-scrolling). Short content
        // is at-end by definition (distance 0), so the pin is invisible
        // until streaming overflows the pane — then it follows, releases on
        // wheel-up, and resticks/jumps exactly like the main transcript.
        let pinned = follow;
        let mut this = Self {
            host,
            list,
            rows: Vec::new(),
            last_source: None,
            // Pre-set so `sync` never sees an attach edge — an override
            // instance must not reset (or re-pin) on selection changes.
            chat_id: doc_override.clone(),
            retain_on_deselect: false,
            land_end_pending: doc_override.is_some() && !follow,
            doc_live: doc_override.is_some() && follow,
            doc_override,
            saved_viewports: SavedViewportCache::default(),
            pending_viewport: None,
            viewport_generation: 0,
            viewport_finalize_pending: false,
            viewport_finalize_scheduled: false,
            viewport_layout_revision: 0,
            row_cache: HashMap::new(),
            live_parsers: HashMap::new(),
            tree_cache: HashMap::new(),
            folds: HashMap::new(),
            tool_group_reveals: HashMap::new(),
            last_replay_baseline: None,
            historical_markdown: HashMap::new(),
            tool_details: HashMap::new(),
            user_folds: HashMap::new(),
            user_heights: HashMap::new(),
            user_hold_task: None,
            user_hold_token: 0,
            user_collapse_scroll: None,
            user_collapse_scroll_scheduled: false,
            veils: HashMap::new(),
            veil_baseline: std::collections::HashSet::new(),
            veil_attach_pending: true,
            render_cache: Rc::new(RefCell::new(RenderCache::default())),
            workspace_link: None,
            file_link_roots: Default::default(),
            inline_code_links: Default::default(),
            rendered_rows: HashSet::new(),
            typography_generation: crate::prefs::type_generation(cx),
            content_width: crate::prefs::transcript_width(cx),
            right_reserve: 0.0,
            code_fences_generation: crate::prefs::code_fences_generation(cx),
            compact_mode: crate::prefs::transcript_compact_mode(cx),
            compact_live_entries: HashSet::new(),
            compact_last_elapsed: HashMap::new(),
            compact_worked_fade_at: HashMap::new(),
            compact_fold_heights: HashMap::new(),
            compact_fold_settle: None,
            highlights: HighlightStore::default(),
            show_jump_button: false,
            last_scroll_distance: 0.0,
            pinned,
            own_turn: None,
            pending_queued_turns: PendingQueuedTurns::default(),
            own_turn_kick: false,
            own_turn_scheduled: false,
            own_turn_last_tick: None,
            spring: StickSpring::new(),
            spring_last_tick: None,
            spring_settled_at: None,
            spring_kick: false,
            spring_scheduled: false,
            scroll_anim: None,
            selection_drag_position: None,
            selection_scroll_task: None,
            rail_enabled,
            bottom_clearance: 0.0,
            host_layout: None,
            rail_hover: None,
            rail_cache: None,
            hovered_entry: None,
            copied_code: None,
            copied_clear: None,
            code_fences: HashMap::new(),
            copied_message: None,
            copied_message_clear: None,
            blob_details: HashMap::new(),
            blob_fetch_order: HashMap::new(),
            blob_fetch_counter: 0,
            _host_watch: host_watch,
            media: M::new(cx),
        };
        this.sync(cx);
        this
    }

    // ---- rail plumbing (rendering lives in crate::rail) ----

    /// Shell-driven width gate: the rail hides below 48rem of container width.
    /// Shell-driven: hold `reserve` px free on the right of the reading
    /// column (a docked side card), so the column and card center together.
    pub fn set_right_reserve(&mut self, reserve: f32, cx: &mut Context<Self>) {
        let reserve = reserve.max(0.0);
        if (self.right_reserve - reserve).abs() > 0.25 {
            self.right_reserve = reserve;
            // Rows rewrap at the new width, including virtual ones offscreen.
            self.list.remeasure();
            if self.pinned {
                self.wake_spring();
            }
            cx.notify();
        }
    }

    pub fn set_rail_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.rail_enabled != enabled {
            self.rail_enabled = enabled;
            cx.notify();
        }
    }

    pub fn rail_enabled(&self) -> bool {
        self.rail_enabled
    }

    /// Lay the list out for a host without the desktop's titlebar,
    /// composer overlay, and wide column; `None` keeps the desktop's.
    pub fn set_host_layout(&mut self, layout: Option<HostLayout>, cx: &mut Context<Self>) {
        if self.host_layout != layout {
            self.host_layout = layout;
            self.list.remeasure();
            cx.notify();
        }
    }

    /// Shell-driven: the measured height of the bottom chrome stack the
    /// transcript scrolls under. Sub-pixel jitter is ignored so steady-state
    /// frames don't re-notify.
    pub fn set_bottom_clearance(&mut self, height: f32, cx: &mut Context<Self>) {
        if (self.bottom_clearance - height).abs() > 0.5 {
            self.bottom_clearance = height;
            if self.own_turn.is_some() {
                self.remeasure_last_row();
                self.own_turn_kick = true;
            }
            cx.notify();
        }
    }

    pub fn rail_hover(&self) -> Option<usize> {
        self.rail_hover
    }

    pub fn set_rail_hover(&mut self, hover: Option<usize>) {
        self.rail_hover = hover;
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn list_state(&self) -> &ListState {
        &self.list
    }

    /// Snapshot the outgoing primary chat before its rows and ListState are
    /// reset. Empty rows never overwrite an older snapshot: during a rapid
    /// A→B→A switch, B's replay may not have arrived before leaving it again.
    fn remember_current_viewport(&mut self) {
        // Rows can already contain optimistic echoes while an older snapshot
        // is still waiting for the authoritative replay. Leaving again in
        // that window must preserve the older snapshot, not replace it with
        // the partial echo-only viewport.
        if self.pending_viewport.is_some() {
            return;
        }
        let Some(chat_id) = self.chat_id.clone() else {
            return;
        };
        let distance_from_bottom = if self.pinned {
            0.0
        } else {
            self.distance_from_bottom()
        };
        let Some(viewport) = SavedViewport::capture(
            &self.rows,
            self.list.logical_scroll_top(),
            self.pinned,
            distance_from_bottom,
            self.own_turn.as_ref(),
        ) else {
            return;
        };
        self.saved_viewports.insert(chat_id, viewport);
    }

    /// Restore an exact optimistic row while replay is pending, enable stable
    /// fallbacks only after a populated reset, and retire snapshots proven
    /// absent by an empty reset. `scroll_to` remains valid while the virtual
    /// list measures restored rows on the following layout pass.
    fn restore_pending_viewport(&mut self, replay: TranscriptReplayState) -> bool {
        if self.pending_viewport.is_none() {
            return false;
        }
        if !self.rows.is_empty()
            && let Some(restored) = self
                .pending_viewport
                .as_ref()
                .and_then(|saved| saved.resolve(&self.rows, replay.allows_fallback()))
        {
            self.pending_viewport = None;
            self.list.scroll_to(restored.offset);
            self.own_turn = restored.own_turn;
            self.own_turn_kick = self.own_turn.is_some();
            self.own_turn_last_tick = None;
            if self.own_turn.is_some() {
                // Replay readiness can change while echo rows stay identical,
                // so the no-diff path may install a runway without splicing.
                self.remeasure_last_row();
            }
            self.last_scroll_distance = restored.distance_from_bottom;
            self.show_jump_button = restored.distance_from_bottom > SCROLL_BUTTON_THRESHOLD_PX;
            self.viewport_finalize_pending = true;
            return true;
        }

        if !replay.authoritative_empty() {
            return false;
        }
        // The reset's document rows, not the combined rows, define
        // authoritative emptiness. A matching optimistic row above remains
        // valid, but an unrelated echo must never become an index fallback
        // for old history.
        self.discard_pending_viewport();
        if self.own_turn.is_none() {
            self.pinned = true;
            self.last_scroll_distance = 0.0;
            self.show_jump_button = false;
            self.list.scroll_to_end();
        }
        true
    }

    /// Explicit user/navigation intent supersedes a replay-delayed restore.
    /// Replace its cache entry with tail-follow until current rows can be
    /// snapshotted normally on the next chat switch.
    pub fn discard_pending_viewport(&mut self) {
        if self.pending_viewport.take().is_some()
            && let Some(chat_id) = self.chat_id.clone()
        {
            self.saved_viewports
                .insert(chat_id, SavedViewport::FollowTail);
        }
    }

    /// The app this transcript draws from.
    pub fn host(&self) -> &Rc<dyn TranscriptHost> {
        &self.host
    }

    /// The host's answers changed: everything, or only `doc_id`'s text.
    pub fn host_changed(&mut self, doc_id: Option<&str>, cx: &mut Context<Self>) {
        if let Some(doc_id) = doc_id {
            let shown = self
                .doc_override
                .as_deref()
                .or_else(|| self.host.selected_chat(cx));
            if shown != Some(doc_id) {
                return;
            }
        }
        self.sync(cx);
    }

    /// Hand viewport ownership to explicit rail/navigation input before its
    /// reduced-motion or animated branch moves the list.
    pub fn begin_scroll_navigation(&mut self) {
        self.discard_pending_viewport();
        self.cancel_user_hold();
        self.user_collapse_scroll = None;
        self.stop_automatic_scrolling();
    }

    fn stop_automatic_scrolling(&mut self) {
        // Navigation and selection within the session release the hold but keep the
        // runway (user spec: only leaving and revisiting the session clears
        // it) — scrolling back down re-arms the hold like any restick.
        self.release_own_turn_hold();
        self.pinned = false;
        self.spring.reset();
        self.spring_last_tick = None;
        self.spring_settled_at = None;
        self.spring_kick = false;
        self.scroll_anim = None;
    }

    /// Store the animation after [`Self::begin_scroll_navigation`].
    pub fn set_scroll_task(&mut self, task: Task<()>) {
        self.scroll_anim = Some(task);
    }

    /// Give the viewport to the user/navigation without dropping the
    /// reservation: the pad stays, the hold stands down until a restick.
    fn release_own_turn_hold(&mut self) {
        if let Some(anchor) = self.own_turn.as_mut() {
            anchor.held = false;
        }
        self.own_turn_last_tick = None;
    }

    fn remeasure_last_row(&mut self) {
        if let Some(last) = self.rows.len().checked_sub(1) {
            self.list.remeasure_items(last..last + 1);
            self.viewport_layout_revision = self.viewport_layout_revision.wrapping_add(1);
        }
    }

    pub fn distance_from_bottom(&self) -> f32 {
        let max = f32::from(self.list.max_offset_for_scrollbar().y);
        let cur = f32::from(self.list.scroll_px_offset_for_scrollbar().y);
        (max + cur).max(0.0)
    }

    /// Whether a user scroll should re-engage the bottom pin: inside the 70px
    /// stick band *and* moving toward the bottom. Direction matters — a small
    /// wheel-up notch near the bottom stays inside the band, and re-sticking
    /// on it would snap the view straight back, making the pin unbreakable.
    pub fn should_restick(distance: f32, previous_distance: f32) -> bool {
        distance <= STICK_THRESHOLD_PX && distance < previous_distance
    }

    fn handle_scroll(&mut self, _event: &ListScrollEvent, cx: &mut Context<Self>) {
        // Cancel synchronously, before a queued animation frame can undo the
        // wheel/touch input. Neither operation reads the borrowed ListState.
        self.user_collapse_scroll = None;
        self.cancel_user_hold();
        let released_own_turn = self.own_turn.as_ref().is_some_and(|anchor| anchor.held);
        self.release_own_turn_hold();
        if self.own_turn.is_some() {
            // Cancel any tail spring synchronously too; the deferred input
            // decision below may re-engage it after reading the new offset.
            self.pinned = false;
            self.spring.reset();
            self.spring_last_tick = None;
        }
        // The list invokes this handler ONLY from its wheel/touch input path
        // (programmatic scroll_by/scroll_to never re-enter it), while holding
        // its internal RefCell borrow — reading the ListState back
        // synchronously panics with "already mutably borrowed". Defer to the
        // end of the effect cycle, after the list has released its borrow.
        let this = cx.weak_entity();
        cx.defer(move |cx| {
            this.update(cx, |this: &mut Self, cx| {
                M::scrolled_near_top(this, cx);
                this.discard_pending_viewport();
                // Input owns the viewport immediately, including wheel-down
                // after background streaming. A held turn can be stale while
                // frame callbacks are paused; reasserting its old prompt here
                // made scrolling down impossible until an upward gesture.
                if this.own_turn.is_some() {
                    let distance = this.distance_from_bottom();
                    let previous = this.last_scroll_distance;
                    this.last_scroll_distance = distance;
                    // Reaching the end preserves normal tail-follow intent
                    // without reasserting a possibly stale prompt hold.
                    this.pinned =
                        distance <= AT_BOTTOM_PX || Self::should_restick(distance, previous);
                    this.spring.reset();
                    this.spring_last_tick = None;
                    // Re-stick only when returning to a short turn's actual
                    // hold. An off-screen prompt belongs to an overflowing
                    // reply, even if reservation refinement hasn't run yet.
                    let at_hold = this.own_turn_anchor_ix().is_some_and(|ix| {
                        this.list.bounds_for_item(ix).is_some_and(|bounds| {
                            f32::from(bounds.top() - this.list.viewport_bounds().top())
                                >= Self::own_send_inset(ix) - OWN_SEND_SCROLL_SLACK_PX - 2.0
                        })
                    });
                    if !released_own_turn && at_hold && Self::should_restick(distance, previous) {
                        if let Some(anchor) = this.own_turn.as_mut() {
                            anchor.held = true;
                            anchor.positioned = false;
                        }
                        this.pinned = false;
                        this.own_turn_kick = true;
                    }
                    if this.pinned {
                        this.wake_spring();
                    }
                    this.show_jump_button = jump_visibility(this.show_jump_button, distance)
                        && !this.own_turn.as_ref().is_some_and(|a| a.held);
                    cx.notify();
                    return;
                }
                let distance = this.distance_from_bottom();
                let previous = this.last_scroll_distance;
                this.last_scroll_distance = distance;
                if distance > previous + 1.0 && distance > AT_BOTTOM_PX {
                    // User input moving away from the bottom breaks the pin.
                    // Content growth never lands here — it doesn't fire the
                    // scroll handler (mugen §1e: interrupt from input, not
                    // scrollbar position).
                    this.pinned = false;
                    this.spring.reset();
                    this.spring_last_tick = None;
                } else if distance <= AT_BOTTOM_PX || Self::should_restick(distance, previous) {
                    // Returning toward the bottom inside the 70px band (or
                    // arriving at it) re-engages the pin with a glide.
                    if !this.pinned {
                        this.pinned = true;
                        this.wake_spring();
                    }
                }
                let show = jump_visibility(this.show_jump_button, distance) && !this.pinned;
                if show != this.show_jump_button {
                    this.show_jump_button = show;
                }
                cx.notify();
            })
            .ok();
        });
    }

    fn on_selection_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() || !crate::markdown::selection::is_dragging() {
            self.stop_selection_scroll();
            return;
        }
        self.selection_drag_position = Some(event.position);
        if render::update_drag_at(event.position) {
            cx.notify();
        }
        self.schedule_selection_scroll(cx);
    }

    fn on_selection_mouse_down(
        &mut self,
        event: &gpui::MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Text listeners claim the drag before it bubbles here. Stop following
        // immediately: a stream commit can otherwise virtualize the anchor
        // before the first mouse move. Keep the user bubble's long-press timer.
        if !crate::markdown::selection::is_dragging() {
            return;
        }
        self.selection_drag_position = Some(event.position);
        self.discard_pending_viewport();
        self.user_collapse_scroll = None;
        self.stop_automatic_scrolling();
        self.materialize_scroll_anchor();
        cx.notify();
    }

    fn on_selection_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_selecting = self.selection_drag_position.is_some();
        self.stop_selection_scroll();
        if let Some(_text) = crate::markdown::selection::end_active_drag() {
            // X11 middle-click paste parity, including the case where the
            // anchor row has virtualized away and cannot receive mouse-up.
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            cx.write_to_primary(ClipboardItem::new_string(_text));
        }
        if was_selecting {
            self.last_scroll_distance = self.distance_from_bottom();
            self.show_jump_button =
                jump_visibility(self.show_jump_button, self.last_scroll_distance);
            cx.notify();
        }
    }

    fn stop_selection_scroll(&mut self) {
        self.selection_drag_position = None;
        self.selection_scroll_task = None;
    }

    fn schedule_selection_scroll(&mut self, cx: &mut Context<Self>) {
        if self.selection_scroll_task.is_some() || !crate::markdown::selection::is_dragging() {
            return;
        }
        let Some(position) = self.selection_drag_position else {
            return;
        };
        if selection_scroll_step(self.list.viewport_bounds(), position) == 0.0 {
            return;
        }
        self.selection_scroll_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(SELECTION_SCROLL_TICK_MS))
                .await;
            let _ = this.update(cx, |transcript, cx| {
                transcript.selection_scroll_task = None;
                transcript.step_selection_scroll(cx);
            });
        }));
    }

    fn step_selection_scroll(&mut self, cx: &mut Context<Self>) {
        if !crate::markdown::selection::is_dragging() {
            self.stop_selection_scroll();
            return;
        }
        let Some(position) = self.selection_drag_position else {
            return;
        };
        let step = selection_scroll_step(self.list.viewport_bounds(), position);
        if step == 0.0 {
            return;
        }

        // Resolve against the registry painted after the previous step before
        // moving it again. This is what lets a stationary edge pointer consume
        // successive virtualized rows.
        render::update_drag_at(position);
        self.begin_scroll_navigation();
        self.list.scroll_by(px(step));
        self.last_scroll_distance = self.distance_from_bottom();
        self.show_jump_button = jump_visibility(self.show_jump_button, self.last_scroll_distance);
        cx.notify();
        self.schedule_selection_scroll(cx);
    }

    /// Reserve the reply's space below a locally-sent prompt — EVERY send,
    /// not just the first (a steer or a post-turn send used to collapse the
    /// previous reservation and drop the messages back down — user report).
    /// [`Self::step_own_turn`] sizes the reservation and eases the prompt to
    /// its top inset. Replacing a previous anchor starts a new glide.
    pub fn on_own_send(&mut self, chat_id: String, message_id: String, cx: &mut Context<Self>) {
        self.user_collapse_scroll = None;
        self.cancel_user_hold();
        self.discard_pending_viewport();
        self.pinned = false;
        self.show_jump_button = false;
        self.spring.reset();
        self.spring_last_tick = None;
        self.spring_settled_at = None;
        self.spring_kick = false;
        self.scroll_anim = None;
        // A glued offset re-snaps to the end on EVERY layout — the pad would
        // land and the viewport hard-track its bottom in the same frame,
        // skipping the glide entirely (rig-traced). Pin the offset to a
        // CONCRETE visible item first; the pad then reads as scrollable
        // distance for the glide to cover.
        self.materialize_scroll_anchor();
        let prompt_ix = self
            .rows
            .iter()
            .position(|row| row.turn_start && row.entry_id == message_id.as_str());
        self.own_turn = Some(OwnTurnAnchor {
            chat_id,
            message_id: SharedString::from(message_id),
            held: true,
            positioned: false,
            seen_prompt: prompt_ix.is_some(),
        });
        self.own_turn_last_tick = None;
        self.own_turn_kick = true;
        self.remeasure_last_row();
        cx.notify();
    }

    /// Remember a locally-authored queue row without touching the active
    /// runway. If host promotion won the race with the QueueMessage reply, the
    /// matching prompt is already present and can be anchored immediately.
    pub fn on_own_queued_send(
        &mut self,
        chat_id: String,
        message_id: String,
        cx: &mut Context<Self>,
    ) {
        let materialized = self.chat_id.as_deref() == Some(chat_id.as_str())
            && self
                .rows
                .iter()
                .any(|row| row.turn_start && row.entry_id == message_id.as_str());
        if materialized {
            self.on_own_send(chat_id, message_id, cx);
        } else {
            self.pending_queued_turns.register(chat_id, message_id);
        }
    }

    /// Promote a queued row only after its real transcript bubble exists.
    /// Materializations first observed while attaching to a chat are consumed
    /// without taking viewport ownership: navigation must not create a hidden
    /// auto-follow merely because queued work ran while the chat was away.
    fn promote_materialized_queued_turn(&mut self, attached: bool, cx: &mut Context<Self>) {
        let Some(chat_id) = self.chat_id.clone() else {
            return;
        };
        let Some(message_id) = self
            .pending_queued_turns
            .take_latest_materialized(&chat_id, &self.rows)
        else {
            return;
        };
        if !attached {
            self.on_own_send(chat_id, message_id, cx);
        }
    }

    /// Convert a glued scroll offset (`None`/past-the-end — layout re-snaps
    /// it to the end each frame) into a concrete `{item, offset}` anchored at
    /// the first visible row, which layout holds still.
    fn materialize_scroll_anchor(&mut self) {
        if !self.is_glued() {
            return;
        }
        let vp_top = f32::from(self.list.viewport_bounds().top());
        for ix in 0..self.rows.len() {
            if let Some(bounds) = self.list.bounds_for_item(ix)
                && f32::from(bounds.bottom()) > vp_top + 0.5
            {
                self.list.scroll_to(ListOffset {
                    item_ix: ix,
                    offset_in_item: px(vp_top - f32::from(bounds.top())),
                });
                return;
            }
        }
        // Bottom-aligned short lists expose no item bounds. Materialize
        // their actual end position using the measured height tree instead.
        // Preserve a negative first-row offset for the blank space above a
        // short chat; clamping it to zero would jump as the minimum is added.
        if !self.rows.is_empty() {
            let viewport_height = f32::from(self.list.viewport_bounds().size.height);
            self.list.scroll_by(px(-1.0));
            let content_height = -f32::from(self.list.scroll_px_offset_for_scrollbar().y) + 1.0;
            if content_height < viewport_height {
                self.list.scroll_to(ListOffset {
                    item_ix: 0,
                    offset_in_item: px(content_height - viewport_height),
                });
            } else {
                self.list.scroll_by(px(1.0 - viewport_height));
            }
        }
    }

    /// The held prompt's top offset from the viewport top. Row 0 already
    /// carries the titlebar chrome inside its own box (the first row's
    /// top gap), so the hold adds nothing — adding the inset on top parked
    /// a new chat's first prompt a double-chrome ~66px low (user report).
    fn own_send_inset(anchor_ix: usize) -> f32 {
        if anchor_ix == 0 {
            0.0
        } else {
            OWN_SEND_TOP_INSET_PX
        }
    }

    fn own_turn_anchor_ix(&self) -> Option<usize> {
        let anchor = self.own_turn.as_ref()?;
        self.rows
            .iter()
            .position(|row| row.turn_start && row.entry_id == anchor.message_id)
    }

    fn reconcile_own_turn_prompt(&mut self) {
        let Some(message_id) = self
            .own_turn
            .as_ref()
            .map(|anchor| anchor.message_id.clone())
        else {
            return;
        };
        let exists = self
            .rows
            .iter()
            .any(|row| row.turn_start && row.entry_id == message_id);
        let keep = self
            .own_turn
            .as_mut()
            .is_some_and(|anchor| anchor.observe_prompt(exists));
        if keep {
            return;
        }

        self.own_turn = None;
        self.own_turn_kick = false;
        self.own_turn_last_tick = None;
        self.remeasure_last_row();
        self.last_scroll_distance = self.distance_from_bottom();
        self.show_jump_button = jump_visibility(self.show_jump_button, self.last_scroll_distance);
        self.viewport_finalize_pending = true;
    }

    /// Install the reservation before list layout. Its height follows the
    /// current viewport in that same pass, including window resizes.
    fn update_runway_minimum(&mut self, cx: &gpui::App) {
        if let Some(ix) = self.own_turn_anchor_ix() {
            // Revealing the prompt adds reading space; only reply growth
            // consumes the runway. Include the same expansion tween in the
            // reservation before layout, including its reverse on Show less.
            let expansion = self.user_folds.get(&self.rows[ix].id).map_or(0.0, |fold| {
                let target = if fold.open == Some(true) {
                    fold.user_expansion_height
                } else {
                    0.0
                };
                match fold.toggled_at {
                    Some(at) if !motion::reduced_motion(cx) && fold.duration_ms > 0 => {
                        let raw = (at.elapsed().as_secs_f32() * 1000.0 / fold.duration_ms as f32)
                            .clamp(0.0, 1.0);
                        let progress = user_resize_spec(fold.user_expansion_height).progress(raw);
                        motion::lerp(fold.user_expansion_height - target, target, progress)
                    }
                    _ => target,
                }
            });
            self.list.set_tail_reservation(Some((
                ix,
                px(Self::own_send_inset(ix) - OWN_SEND_SCROLL_SLACK_PX - expansion),
            )));
        } else if self.own_turn.is_none() {
            self.list.set_tail_reservation(None);
        }
    }

    fn scroll_own_turn_by(&self, delta: f32) {
        let offset = self.list.logical_scroll_top();
        if offset.item_ix == 0 && offset.offset_in_item < px(0.0) {
            self.list.scroll_to(ListOffset {
                item_ix: 0,
                offset_in_item: offset.offset_in_item + px(delta),
            });
        } else {
            self.list.scroll_by(px(delta));
        }
    }

    /// Advance the prompt glide or hand a filled reservation to tail-follow.
    /// Reservation sizing happens in the list layout, never in this callback.
    fn step_own_turn(&mut self, cx: &mut Context<Self>) {
        if self.route_exit_pending(cx) {
            return;
        }
        self.own_turn_kick = false;
        // Layout moves the bottom too (pad refinement, streaming growth):
        // refresh the wheel handler's escape baseline every frame so only a
        // WHEEL's own delta registers as user intent. Without this, the pad
        // growing at turn-completion between two wheel events read as
        // "scrolled away" and silently released the hold — the next wheels
        // then sank unopposed deep into the runway blank (rig-traced).
        self.last_scroll_distance = self.distance_from_bottom();
        let Some(anchor_ix) = self.own_turn_anchor_ix() else {
            // The optimistic echo may arrive on the next state notification.
            return;
        };
        if let Some(anchor) = self.own_turn.as_mut() {
            anchor.seen_prompt = true;
        }
        let viewport = self.list.viewport_bounds();
        let viewport_height = f32::from(viewport.size.height);
        if viewport_height <= 0.0 {
            self.own_turn_kick = true;
            cx.notify();
            return;
        }
        let inset = Self::own_send_inset(anchor_ix);
        if self.is_glued() && self.own_turn.as_ref().is_some_and(|anchor| anchor.held) {
            self.list.scroll_by(px(-viewport_height));
        }
        let anchor_bounds = self.list.bounds_for_item(anchor_ix);
        // The list consumes the reservation in the same layout that measures
        // new rows. The height tree remains available when the prompt or tail
        // is outside the viewport, so neither can block the handoff.
        if self.list.tail_reservation_filled() {
            let held = self.own_turn.take().is_some_and(|a| a.held);
            self.own_turn_last_tick = None;
            self.list.set_tail_reservation(None);
            if held
                || self.pinned
                || (self.selection_drag_position.is_none()
                    && self.distance_from_bottom() <= AT_BOTTOM_PX)
            {
                self.engage_pin(cx);
            } else {
                cx.notify();
            }
            return;
        }

        // ---- entry glide, then absolute hold -------------------------------
        let (held, positioned) = self
            .own_turn
            .as_ref()
            .map_or((false, false), |a| (a.held, a.positioned));
        if !held {
            return;
        }
        if positioned {
            // Landed: re-assert the prompt's position after every layout.
            // scroll_to is absolute and bounds-independent, so neither glue
            // re-snaps, pad-sizing lag, nor a splice's unmeasured flicker can
            // carry the view off the prompt (each broke the spring-held
            // variants of this — rig-traced). ONE-SIDED: only upward drift
            // (view above the hold) is corrected. The scroll slack under the
            // reservation is legal resting space — wheel-down sinks into it
            // and stops hard at the list's own clamp; snapping back up from
            // there made the bottom bounce/stutter on every scroll event
            // (user report). Way-below-slack (impossible short of a bug)
            // still re-asserts.
            let moved = match anchor_bounds {
                Some(b) => {
                    let err = f32::from(b.top()) - (f32::from(viewport.top()) + inset);
                    // The legal rest zone below the hold is the epsilon plus
                    // rounding; anything deeper is a transient-collision sink
                    // and rubber-bands back.
                    err > 0.5 || err < -(OWN_SEND_SCROLL_SLACK_PX + 2.0)
                }
                // Bounds vanish in the glued representation (dissolved
                // above, so at most for this one frame) and through splice
                // flicker. Near the stop that is dead-band space — no
                // assert (asserting on None here was the bottom bounce);
                // far from it the position is unknowable flicker: re-assert.
                None => self.distance_from_bottom() > OWN_SEND_SCROLL_SLACK_PX + 8.0,
            };
            if moved {
                // Correct with the entry glide's ease, not a snap: the only
                // in-band escapes are one-frame commit transients and splice
                // flicker, and an eased ~200ms return reads as native
                // rubber-banding where an instant re-assert read as stutter
                // (user report). Bounds-less flicker still snaps — there is
                // nothing to ease against.
                match anchor_bounds {
                    Some(b) => {
                        let err = f32::from(b.top()) - (f32::from(viewport.top()) + inset);
                        let now = Instant::now();
                        let frames = match self.own_turn_last_tick {
                            Some(last) => (now.duration_since(last).as_secs_f32() * 1000.0
                                / SPRING_FRAME_MS)
                                .min(SPRING_MAX_CATCHUP_FRAMES),
                            None => 1.0,
                        };
                        self.own_turn_last_tick = Some(now);
                        let ease = 1.0 - OWN_SEND_GLIDE_RETAIN.powf(frames);
                        if err.abs() <= OWN_SEND_GLIDE_SNAP_PX {
                            self.list.scroll_by(px(err));
                            self.own_turn_last_tick = None;
                        } else {
                            self.scroll_own_turn_by(err * ease);
                        }
                        self.own_turn_kick = true;
                    }
                    None => {
                        self.list.scroll_to(ListOffset {
                            item_ix: anchor_ix,
                            offset_in_item: px(-inset),
                        });
                        self.own_turn_last_tick = None;
                    }
                }
                cx.notify();
            } else {
                self.own_turn_last_tick = None;
            }
            return;
        }
        let now = Instant::now();
        let frames = match self.own_turn_last_tick {
            Some(last) => (now.duration_since(last).as_secs_f32() * 1000.0 / SPRING_FRAME_MS)
                .min(SPRING_MAX_CATCHUP_FRAMES),
            None => 1.0,
        };
        self.own_turn_last_tick = Some(now);
        let ease = 1.0 - OWN_SEND_GLIDE_RETAIN.powf(frames);
        // Prefer the prompt geometry. When it is being remeasured but is
        // already the scroll anchor, its logical offset is equally exact.
        // Otherwise approach through the unmeasured rows, capping every step
        // at the prompt so the provisional minimum can never cause overshoot.
        let (err, anchored) = match anchor_bounds {
            Some(bounds) => (
                f32::from(bounds.top()) - (f32::from(viewport.top()) + inset),
                true,
            ),
            None if self.list.logical_scroll_top().item_ix == anchor_ix => (
                -f32::from(self.list.logical_scroll_top().offset_in_item) - inset,
                true,
            ),
            None => {
                // Remeasurement retains preceding row heights as hints. Read
                // the prompt's coordinate from that same height tree rather
                // than aiming at the provisional minimum's (larger) bottom.
                let current = f32::from(self.list.scroll_px_offset_for_scrollbar().y);
                let target = -f32::from(self.list.offset_for_item(anchor_ix)) + inset;
                (current - target, false)
            }
        };
        let glide_max = GLIDE_MAX_VIEWPORTS * viewport_height;
        let err = if err > glide_max {
            self.list.scroll_by(px(err - glide_max));
            glide_max
        } else {
            err
        };
        let land = |list: &ListState| {
            list.scroll_to(ListOffset {
                item_ix: anchor_ix,
                offset_in_item: px(-inset),
            });
        };
        if motion::reduced_motion(cx) {
            land(&self.list);
            if let Some(anchor) = self.own_turn.as_mut() {
                anchor.positioned = true;
            }
            self.own_turn_last_tick = None;
        } else if anchored
            && err <= OWN_SEND_GLIDE_SNAP_PX
            && err >= -(OWN_SEND_SCROLL_SLACK_PX + 2.0)
        {
            // At the hold — or resting inside the slack under it (a restick
            // that fired at the true bottom): land WITHOUT pulling the view
            // up. Only a still-above position gets the snap.
            if err > 0.5 {
                land(&self.list);
            }
            if let Some(anchor) = self.own_turn.as_mut() {
                anchor.positioned = true;
            }
            self.own_turn_last_tick = None;
        } else if !anchored && err <= OWN_SEND_GLIDE_SNAP_PX {
            // The height hints put us at the prompt. Land by row identity
            // so its final measurement cannot leave us in the reservation.
            land(&self.list);
            if let Some(anchor) = self.own_turn.as_mut() {
                anchor.positioned = true;
            }
            self.own_turn_last_tick = None;
        } else {
            self.scroll_own_turn_by(err * ease);
            if own_turn_glide_crossed(self.list.logical_scroll_top(), anchor_ix, inset) {
                land(&self.list);
            }
        }
        self.own_turn_kick = true;
        cx.notify();
    }

    /// Whether the transcript is currently pinned to the bottom.
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    /// Whether the shell should float the "Scroll to bottom" pill (scrolled
    /// more than [`SCROLL_BUTTON_THRESHOLD_PX`] off the end, unpinned).
    pub fn jump_button_shown(&self) -> bool {
        self.show_jump_button
    }

    /// The scroll-to-bottom pill's click: glide back to the end and re-pin.
    pub fn jump_to_bottom(&mut self, cx: &mut Context<Self>) {
        self.user_collapse_scroll = None;
        self.cancel_user_hold();
        self.discard_pending_viewport();
        // An expanded prompt can be taller than the viewport. Its reservation
        // is retained, but jumping should reveal the reply below that prompt.
        if self.own_turn_anchor_ix().is_some_and(|ix| {
            self.user_folds
                .get(&self.rows[ix].id)
                .is_some_and(|fold| fold.open == Some(true))
        }) {
            self.release_own_turn_hold();
            self.engage_pin(cx);
            return;
        }
        // With a live runway, "bottom" IS the held position (the reservation
        // makes prompt-at-top and pad-bottom the same place): re-arm the hold
        // and glide back instead of destroying the runway (user spec — only
        // navigating away and back clears it).
        if let Some(anchor) = self.own_turn.as_mut() {
            anchor.held = true;
            anchor.positioned = false;
            self.own_turn_last_tick = None;
            self.own_turn_kick = true;
            self.show_jump_button = false;
            cx.notify();
            return;
        }
        self.engage_pin(cx);
    }

    /// Re-engage the bottom pin with a glide. Long jumps teleport to within
    /// [`GLIDE_MAX_VIEWPORTS`] of the end first (mugen `springToBottom`);
    /// reduced motion snaps.
    fn engage_pin(&mut self, cx: &mut Context<Self>) {
        self.pinned = true;
        self.show_jump_button = false;
        if motion::reduced_motion(cx) {
            self.list.scroll_to_end();
            cx.notify();
            return;
        }
        let viewport = f32::from(self.list.viewport_bounds().size.height);
        let distance = self.distance_from_bottom();
        let glide_max = GLIDE_MAX_VIEWPORTS * viewport;
        if viewport > 0.0 && distance > glide_max {
            self.list.scroll_by(px(distance - glide_max));
        }
        self.wake_spring();
        cx.notify();
    }

    /// Arm the per-frame spring driver — `render` schedules the next frame
    /// while [`Self::spring_should_run`].
    fn wake_spring(&mut self) {
        if self.spring_settled_at.is_some_and(|settled| {
            settled.elapsed() >= Duration::from_millis(SPRING_SETTLE_GRACE_MS)
        }) {
            self.spring.reset();
            self.spring_last_tick = None;
        }
        self.spring_settled_at = None;
        self.spring_kick = true;
    }

    /// A layout kick needs one observation; otherwise only unfinished motion
    /// needs another frame. The settle grace retains state without repainting.
    fn spring_should_run(&self) -> bool {
        self.spring_kick || StickSpring::needs_frame(self.distance_from_bottom())
    }

    /// Whether the scroll offset is in a bottom-glued representation (`None`
    /// or anchored past the end) — states where the next layout hard-snaps to
    /// the new end instead of holding a pixel position.
    pub fn is_glued(&self) -> bool {
        self.list.logical_scroll_top().item_ix >= self.rows.len()
    }

    /// One spring frame: observe target growth, step the stepper, apply the
    /// delta, and park on landing. Runs from `window.on_next_frame`,
    /// i.e. after layout — measurements are fresh.
    fn step_spring(&mut self, cx: &mut Context<Self>) {
        if self.route_exit_pending(cx) {
            return;
        }
        self.spring_kick = false;
        if !self.pinned {
            self.spring_last_tick = None;
            return;
        }
        let now = Instant::now();
        if self.spring_settled_at.is_some_and(|settled| {
            now.duration_since(settled) >= Duration::from_millis(SPRING_SETTLE_GRACE_MS)
        }) {
            self.spring.reset();
            self.spring_last_tick = None;
            self.spring_settled_at = None;
        }
        let frames = match self.spring_last_tick {
            Some(last) => (now.duration_since(last).as_secs_f32() * 1000.0 / SPRING_FRAME_MS)
                .min(SPRING_MAX_CATCHUP_FRAMES),
            None => 1.0,
        };
        self.spring_last_tick = Some(now);

        let target = f32::from(self.list.max_offset_for_scrollbar().y);
        let mut distance = self.distance_from_bottom();
        // Long jumps (chat switch mid-history, huge pastes) teleport first.
        let viewport = f32::from(self.list.viewport_bounds().size.height);
        let glide_max = GLIDE_MAX_VIEWPORTS * viewport;
        if viewport > 0.0 && distance > glide_max {
            self.list.scroll_by(px(distance - glide_max));
            distance = glide_max;
        }
        let pos = target - distance;
        let next = self.spring.step(pos, target, frames);
        if next > pos {
            self.list.scroll_by(px(next - pos));
        }
        self.last_scroll_distance = (target - next).max(0.0);

        if target - next <= 0.5 {
            // Land on the final item, not the scrollbar's estimated pixel
            // total. Remeasuring virtual rows can otherwise move that total
            // after every landing and restart the glide indefinitely.
            self.list.scroll_to_end();
            self.spring_settled_at.get_or_insert(now);
        } else {
            self.spring_settled_at = None;
        }
        // A stationary spring used to repaint throughout the 500ms grace.
        // Repeated layout kicks kept that loop alive for entire streams even
        // at distance=0, velocity=0. Preserve the final movement's paint and
        // every moving frame; a settled spring wakes on the next layout kick.
        if next > pos || StickSpring::needs_frame(self.last_scroll_distance) {
            cx.notify();
        }
    }

    pub fn retain_for_route_exit(&mut self) {
        self.retain_on_deselect = true;
    }

    pub fn route_exit_pending(&self, cx: &gpui::App) -> bool {
        self.retain_on_deselect
            && self.doc_override.is_none()
            && self.host.selected_chat(cx).is_none()
            && self.chat_id.is_some()
    }

    pub fn finish_route_exit(&mut self, cx: &mut Context<Self>) {
        if self.host.selected_chat(cx).is_none() && self.chat_id.is_some() {
            self.retain_on_deselect = false;
            self.sync(cx);
            self.retain_on_deselect = true;
        }
    }

    /// Rebuild rows from app state; splice minimal ranges into the list.
    pub fn sync(&mut self, cx: &mut Context<Self>) {
        if self.retain_on_deselect
            && self.doc_override.is_none()
            && self.host.selected_chat(cx).is_none()
            && self.chat_id.is_some()
        {
            // A quick return can reuse this entity before the exit finishes.
            // Its next snapshot is still a replay, not newly arriving tools.
            self.veil_attach_pending = true;
            return;
        }
        let (selected, replay) = {
            let host = &self.host;
            match &self.doc_override {
                // Pinned to a subagent doc: `selected` equals `chat_id` by
                // construction, so the attach/reset branch below never fires,
                // and echoes stay empty (nothing is ever sent from here).
                Some(doc_id) => (Some(doc_id.clone()), TranscriptReplayState::Populated),
                None => {
                    let replay = if !host.transcript_replayed(cx) {
                        TranscriptReplayState::Pending
                    } else if host.entries(None, cx).is_empty() {
                        TranscriptReplayState::Empty
                    } else {
                        TranscriptReplayState::Populated
                    };
                    (host.selected_chat(cx).map(str::to_owned), replay)
                }
            }
        };

        let source = (selected.clone(), replay, self.host.transcript_revision(cx));
        if self.last_source.as_ref() == Some(&source) {
            return;
        }
        self.last_source = Some(source);

        let attached = selected != self.chat_id;
        // Arm the replay baseline before classifying tool arrivals. Selection
        // and replay may arrive in one sync; a retained same-chat entity can
        // also see a fresh pending subscription without changing chat_id.
        if attached || replay == TranscriptReplayState::Pending {
            self.veil_baseline.clear();
            self.veil_attach_pending = true;
        }
        if attached {
            // Read the incoming snapshot before inserting the outgoing one:
            // a full bounded cache may evict its oldest entry, which can be
            // exactly the chat the user is reopening.
            let saved_viewport = selected
                .as_ref()
                .and_then(|chat_id| self.saved_viewports.get_cloned_and_touch(chat_id));
            self.remember_current_viewport();
            let keep_own_turn = self
                .own_turn
                .as_ref()
                .is_some_and(|anchor| selected.as_deref() == Some(anchor.chat_id.as_str()));
            if !keep_own_turn {
                self.own_turn = None;
                self.own_turn_kick = false;
                self.own_turn_last_tick = None;
            }
            self.chat_id = selected;
            self.rows.clear();
            M::chat_changed(self, cx);
            self.row_cache.clear();
            self.live_parsers.clear();
            self.tree_cache.clear();
            self.folds.clear();
            self.tool_group_reveals.clear();
            self.last_replay_baseline = None;
            self.historical_markdown.clear();
            self.user_folds.clear();
            self.user_heights.clear();
            self.compact_fold_heights.clear();
            self.compact_fold_settle = None;
            self.user_hold_token = self.user_hold_token.wrapping_add(1);
            self.user_hold_task = None;
            self.user_collapse_scroll = None;
            self.veils.clear();
            self.render_cache.borrow_mut().clear();
            self.highlights.entries.clear();
            self.copied_message = None;
            self.copied_message_clear = None;
            self.list.reset(0);
            self.pending_viewport = None;
            self.viewport_generation = self.viewport_generation.wrapping_add(1);
            self.viewport_finalize_pending = false;
            if self.own_turn.is_some() {
                // A kept own-turn hold (send-created chat) owns the viewport.
                self.pinned = false;
                self.last_scroll_distance = 0.0;
                self.show_jump_button = false;
            } else if let Some(SavedViewport::Anchored {
                anchor,
                distance_from_bottom,
                own_turn,
            }) = saved_viewport
            {
                // Keep a possible runway pending until replay confirms that
                // its optimistic prompt still exists. Installing it on this
                // empty attach frame can leave a failed send's stale anchor
                // intercepting scroll-to-bottom forever.
                self.pinned = false;
                self.last_scroll_distance = distance_from_bottom;
                self.show_jump_button = distance_from_bottom > SCROLL_BUTTON_THRESHOLD_PX;
                self.pending_viewport = Some(SavedViewport::Anchored {
                    anchor,
                    distance_from_bottom,
                    own_turn,
                });
            } else {
                // New chats and chats that were following their tail retain
                // the existing open-at-bottom behavior.
                self.pinned = true;
                self.last_scroll_distance = 0.0;
                self.show_jump_button = false;
            }
            self.spring.reset();
            self.spring_last_tick = None;
            self.spring_settled_at = None;
            self.spring_kick = false;
            self.scroll_anim = None;
            self.stop_selection_scroll();
        }

        let mut new_rows: Vec<Row> = Vec::new();
        // Borrow the transcript only while deriving rows. Cloning the entity
        // handle lets rows_for mutate our caches without copying every text
        // and tool payload on each app-state notification.
        let (entries_empty, tail_streaming) = {
            let host = self.host.clone();
            let entries = host.entries(self.doc_override.as_deref(), cx);
            let prepared = self.chat_id.as_ref().and_then(|id| host.prepared(id, cx));
            for entry in entries {
                if !self.compact_mode
                    && let Some(rows) = prepared.and_then(|p| p.rows.get(&entry.id))
                {
                    new_rows.extend(rows.iter().cloned());
                } else {
                    new_rows.extend(self.rows_for(entry, false));
                }
            }
            if self.doc_override.is_none() {
                for echo in host.pending_echoes(cx) {
                    new_rows.extend(self.rows_for(echo, true));
                }
            }
            (
                entries.is_empty(),
                entries
                    .last()
                    .is_some_and(|e| e.status == Some(MessageStatus::Streaming)),
            )
        };

        let baseline = self
            .chat_id
            .as_deref()
            .and_then(|id| self.host.baseline(id, cx))
            .cloned();
        let baseline_changed = baseline.as_ref().is_some_and(|baseline| {
            self.last_replay_baseline
                .as_ref()
                .is_none_or(|previous| !Arc::ptr_eq(previous, baseline))
        });
        let mut historical_tools: HashMap<SharedString, HashSet<String>> = HashMap::new();
        let mut fully_historical: HashSet<SharedString> = HashSet::new();
        if baseline_changed {
            let baseline = baseline.as_ref().unwrap();
            let host = self.host.clone();
            let entries = host.entries(self.doc_override.as_deref(), cx);
            let previous_markdown = std::mem::take(&mut self.historical_markdown);
            let prepared = self.chat_id.as_ref().and_then(|id| host.prepared(id, cx));
            let mut historical_rows = Vec::new();
            for entry in entries {
                let covered = prepared.map_or_else(
                    || baseline.covers(entry),
                    |p| {
                        Arc::ptr_eq(baseline, &p.navigation_baseline)
                            || p.fully_historical.contains(&entry.id)
                    },
                );
                if covered {
                    fully_historical.insert(entry.id.clone().into());
                    continue;
                }
                if !self.compact_mode
                    && let Some(rows) = self
                        .chat_id
                        .as_ref()
                        .and_then(|id| host.prepared(id, cx))
                        .and_then(|p| p.historical.get(&entry.id))
                {
                    historical_rows.extend(rows.iter().cloned());
                    continue;
                }
                let Some(historical) = baseline.historical_entry(entry) else {
                    continue;
                };
                historical_rows.extend(rows_for_entry(
                    &historical,
                    false,
                    self.compact_mode,
                    &mut |_, text| Arc::new(parse_full(text)),
                ));
            }
            // A normal opening snapshot is entirely historical. Share its
            // parsed trees instead of parsing the whole transcript twice;
            // only an entry mixing historical and live parts needs a prefix.
            historical_rows.extend(
                new_rows
                    .iter()
                    .filter(|row| {
                        fully_historical.contains(&row.entry_id)
                            && matches!(row.kind, RowKind::LiveMarkdown { .. })
                    })
                    .cloned(),
            );
            for row in historical_rows {
                match &row.kind {
                    RowKind::ToolGroup { tools, .. }
                        if !fully_historical.contains(&row.entry_id) =>
                    {
                        historical_tools
                            .entry(row.entry_id.clone())
                            .or_default()
                            .extend(tools.iter().map(|tool| tool.part_id.clone()));
                    }
                    RowKind::LiveMarkdown { .. } => {
                        if previous_markdown
                            .get(&row.id)
                            .is_none_or(|old| old.version != row.version)
                        {
                            self.veils.remove(&row.id);
                        }
                        self.historical_markdown.insert(row.id.clone(), row);
                    }
                    _ => {}
                }
            }
            self.last_replay_baseline = Some(baseline.clone());
        }

        // Give only rows that ARRIVE after the replay baseline an entrance.
        // The first populated frame after a chat attach may already contain a
        // live tool group; treating it as history prevents a whole existing
        // task tree from reanimating on every chat switch.
        let replay_baseline = self.veil_attach_pending && !entries_empty;
        if replay_baseline {
            // Retain explicit user pins, but never resume an old arrival or
            // closing animation when revisiting the retained transcript.
            self.tool_group_reveals.clear();
            for fold in self.folds.values_mut() {
                fold.toggled_at = None;
                fold.disclosure_at = None;
            }
        }
        let previous_tools: HashMap<SharedString, HashMap<String, Option<Instant>>> = self
            .rows
            .iter()
            .filter(|row| !fully_historical.contains(&row.entry_id))
            .filter_map(|row| match &row.kind {
                RowKind::ToolGroup { tools, .. } => {
                    let reveal = self.tool_group_reveals.get(&row.id);
                    Some((
                        row.id.clone(),
                        tools
                            .iter()
                            .enumerate()
                            .map(|(ix, tool)| {
                                (
                                    tool.part_id.clone(),
                                    reveal.and_then(|r| r.starts.get(ix).copied().flatten()),
                                )
                            })
                            .collect(),
                    ))
                }
                _ => None,
            })
            .collect();
        let now = Instant::now();
        let mut live_tool_groups = HashSet::new();
        for row in &new_rows {
            let RowKind::ToolGroup { tools, .. } = &row.kind else {
                continue;
            };
            // Agent/spawn groups are standalone cards, not task trees —
            // except under compact mode, where they collapse like the rest.
            if !self.compact_mode && !tool_group_collapses(tools) {
                continue;
            }
            live_tool_groups.insert(row.id.clone());
            let historical = historical_tools.get(&row.entry_id);
            let whole_group_historical =
                fully_historical.contains(&row.entry_id) || (replay_baseline && baseline.is_none());
            let is_historical = |tool: &ToolItem| {
                whole_group_historical || historical.is_some_and(|ids| ids.contains(&tool.part_id))
            };
            let historical_count = if whole_group_historical {
                tools.len()
            } else {
                tools.iter().filter(|tool| is_historical(tool)).count()
            };
            let previous = previous_tools.get(&row.id);
            let is_new_group = historical_count == 0 && previous.is_none();
            let reveal = self.tool_group_reveals.entry(row.id.clone()).or_default();
            if baseline_changed && historical_count > 0 {
                reveal.rendered_open = None;
                if let Some(fold) = self.folds.get_mut(&row.id) {
                    fold.toggled_at = None;
                    fold.disclosure_at = None;
                }
            }
            reveal.shimmer_started_at.get_or_insert(now);
            if is_new_group {
                reveal.header_started_at.get_or_insert(now);
            }
            if baseline_changed && historical_count == tools.len() {
                reveal.header_started_at = None;
            }
            let first_row_delay = is_new_group.then_some(TOOL_FIRST_ROW_DELAY_MS).unwrap_or(0);
            let mut arrival_ix = 0;
            if whole_group_historical {
                reveal.starts.clear();
                reveal.starts.resize(tools.len(), None);
                continue;
            }
            reveal.starts = tools
                .iter()
                .map(|tool| {
                    if is_historical(tool) {
                        return None;
                    }
                    if let Some(start) = previous.and_then(|tools| tools.get(&tool.part_id)) {
                        return *start;
                    }
                    let start = now
                        + Duration::from_millis(first_row_delay + arrival_ix * TOOL_ROW_STAGGER_MS);
                    arrival_ix += 1;
                    Some(start)
                })
                .collect();
        }
        self.tool_group_reveals
            .retain(|row_id, _| live_tool_groups.contains(row_id));

        // Runtime scroll handles follow the stable code rows exactly. A live
        // block keeps its handle through completion; deleted/reindexed tail
        // blocks and the previous chat cannot accumulate stale handles.
        let active_code_fences: HashSet<SharedString> = new_rows
            .iter()
            .flat_map(|row| match &row.kind {
                RowKind::Markdown { tree, block_ix } | RowKind::LiveMarkdown { tree, block_ix } => {
                    tree.blocks
                        .get(*block_ix)
                        .map(|top| {
                            render::code_block_indices(&top.block, *block_ix)
                                .into_iter()
                                .map(|ix| format!("{}#code{ix}", row.id).into())
                                .collect()
                        })
                        .unwrap_or_default()
                }
                _ => Vec::new(),
            })
            .collect();
        self.code_fences
            .retain(|key, _| active_code_fences.contains(key));
        M::rows_built(self, &new_rows);

        // Text already streamed before this (re)attach is the veil BASELINE:
        // its rows' veils seed instead of fading (render creates them from
        // this set), so only post-switch appends animate. Captured from the
        // first NON-EMPTY transcript after attach — the replay frame — never
        // the attach-time sync, whose transcript is still empty (selection
        // clears it; the doc watch refills it async).
        if self.veil_attach_pending
            && (!entries_empty || replay.authoritative_empty() || baseline_changed)
        {
            self.veil_attach_pending = false;
            self.veil_baseline = new_rows
                .iter()
                .filter(|r| baseline.is_none() && matches!(r.kind, RowKind::LiveMarkdown { .. }))
                .map(|r| r.id.clone())
                .collect();
        }

        // Veils live exactly as long as their live row — drop them on the
        // live→complete flip (any mid-fade chunk snaps to full, matching the
        // row's version splice).
        let active_markdown: HashSet<&SharedString> = new_rows
            .iter()
            .filter(|row| matches!(row.kind, RowKind::LiveMarkdown { .. }))
            .map(|row| &row.id)
            .collect();
        self.veils.retain(|id, _| active_markdown.contains(id));
        self.veil_baseline.retain(|id| active_markdown.contains(id));
        self.historical_markdown
            .retain(|id, _| active_markdown.contains(id));

        // Capture this before the row splice changes the list's measured end.
        // When the user is truly live-following, retaining the end anchor
        // keeps the in-flow working trailer at the same viewport position as
        // transcript lines grow above it. Nothing about the trailer's layout
        // or coordinates changes.
        let live_following =
            should_anchor_live_stream(self.pinned, self.distance_from_bottom(), tail_streaming);
        let was_empty = self.rows.is_empty();
        let old_last = self.rows.len().checked_sub(1);
        match diff_rows(&self.rows, &new_rows) {
            None => {
                self.rows = new_rows;
                M::rows_changed(self, cx);
                self.reconcile_own_turn_prompt();
                // Replay readiness is independent of row content: an empty
                // reset (or one identical to optimistic rows) still resolves
                // or retires the pending viewport.
                if self.restore_pending_viewport(replay) {
                    cx.notify();
                }
                self.promote_materialized_queued_turn(attached, cx);
                return;
            }
            Some((old_range, count)) => {
                // Any replaced row's cached flatten results are stale — and
                // because live replies splice only the rows whose content hash
                // changed (the tail), this is O(changed rows) per commit, never
                // O(reply).
                for row in &self.rows[old_range.clone()] {
                    self.render_cache.borrow_mut().invalidate_row(&row.id);
                }
                if old_range.len() == count {
                    // In-place content change, same row count — notably the
                    // live→complete flip, where EVERY row of the streamed
                    // message changes version (streaming bit, tool auto_open,
                    // timestamp bit) with identical ids. `splice` would reset
                    // those items to hint-less Unmeasured (heights read 0
                    // until the next paint) and, when the viewport-top item is
                    // inside the range, clobber the scroll anchor to the range
                    // start — the end-of-turn up/down jump the spring then has
                    // to walk back. `remeasure_items` keeps old sizes as hints
                    // and holds the anchor across the remeasure.
                    self.list.remeasure_items(old_range);
                } else {
                    self.list.splice(old_range, count);
                }
                self.viewport_layout_revision = self.viewport_layout_revision.wrapping_add(1);
            }
        }
        self.rows = new_rows;
        if old_last != self.rows.len().checked_sub(1) {
            if let Some(ix) = old_last.filter(|&ix| ix < self.rows.len()) {
                // Bottom chrome moves to the new tail too.
                self.list.remeasure_items(ix..ix + 1);
            }
            if was_empty && self.own_turn.is_some() && !self.rows.is_empty() {
                // There was no concrete row to materialize at send time.
                // Start the echo at the bottom edge before adding its runway.
                self.list.scroll_to(ListOffset {
                    item_ix: 0,
                    offset_in_item: -self.list.viewport_bounds().size.height,
                });
            }
        }
        M::rows_changed(self, cx);
        self.reconcile_own_turn_prompt();
        self.restore_pending_viewport(replay);
        self.promote_materialized_queued_turn(attached, cx);
        if self.land_end_pending && !self.rows.is_empty() {
            // First content for an unpinned override tab: land at the end.
            // `scroll_to_end` is ITEM-anchored (past-the-end offset that the
            // next layout materializes) — a pixel scroll off `max_offset`
            // would land short here, since the freshly-spliced rows are
            // still unmeasured. Short content clamps back to the top under
            // Top alignment, so "end" and "top" coincide there.
            self.land_end_pending = false;
            self.list.scroll_to_end();
        }
        if self.own_turn.is_some() {
            self.own_turn_kick = true;
        }
        if self.pinned {
            if live_following || baseline_changed {
                self.list.scroll_to_end();
                self.spring.reset();
                self.spring_last_tick = None;
                self.spring_settled_at = None;
                self.spring_kick = false;
                self.last_scroll_distance = 0.0;
            } else {
                if motion::reduced_motion(cx) || was_empty {
                    // First fill (chat open) lands at the bottom instantly
                    // (mugen initialScroll:'bottom'); reduced motion snaps.
                    self.list.scroll_to_end();
                } else if self.is_glued() {
                    // A glued offset (`None` / anchored past the end) makes
                    // the upcoming layout hard-snap to the new end — the
                    // per-commit stutter. Materialize a pixel anchor a hair
                    // above the bottom so layout holds position and the
                    // spring glides the growth.
                    self.list.scroll_by(px(-0.75));
                }
                self.spring_kick = true;
            }
        }
        cx.notify();
    }

    fn compact_worked_secs_for(&self, entry: &SessionMessageEntry) -> Option<i64> {
        if !self.compact_mode || entry.role != MessageRole::Assistant {
            return None;
        }
        if entry.status == Some(MessageStatus::Streaming) {
            return None;
        }
        if let Some(ms) = entry.duration_ms {
            return (ms > 0).then_some((ms / 1000).max(1));
        }
        self.compact_last_elapsed
            .get(&entry.id)
            .copied()
            .filter(|&secs| secs > 0)
    }

    /// Cached row build for one entry (streaming entries bypass the cache).
    fn rows_for(&mut self, entry: &SessionMessageEntry, pending: bool) -> Vec<Row> {
        let streaming = entry.status == Some(MessageStatus::Streaming);
        // Live entries always rebuild; don't allocate a fingerprint that the
        // streaming path cannot use.
        // The mode is part of the row split, so it keys the cache alongside
        // the entry's own content (the high bit — `entry_fingerprint` never
        // sets it: its accumulator is small relative to 2^63).
        let fingerprint = if streaming {
            0
        } else {
            entry_fingerprint(entry, pending) ^ ((self.compact_mode as u64) << 63)
        };
        let mut rows = if !streaming
            && let Some(cached) = self.row_cache.get(&entry.id)
            && cached.fingerprint == fingerprint
        {
            cached.rows.clone()
        } else {
            let live_parsers = &mut self.live_parsers;
            let tree_cache = &mut self.tree_cache;
            let mut parse = |key: &str, text: &str| -> Arc<BlockTree> {
                // Render-cache invalidation rides on the row diff in `sync` (only
                // rows whose content hash changed are spliced — the reparsed tail).
                parse_for_row(streaming, key, text, live_parsers, tree_cache).0
            };
            rows_for_entry(entry, pending, self.compact_mode, &mut parse)
        };
        if let Some(secs) = self.compact_worked_secs_for(entry) {
            for row in &mut rows {
                if let RowKind::ToolGroup {
                    worked_secs,
                    compact_shell: true,
                    ..
                } = &mut row.kind
                {
                    *worked_secs = Some(secs);
                }
            }
        }

        if !streaming {
            self.row_cache.insert(
                entry.id.clone(),
                CachedRows {
                    fingerprint,
                    rows: rows.clone(),
                },
            );
        }
        rows.retain(|row| match &row.compact_fold {
            None => true,
            // A closing fold keeps its body mounted through the TOOL_FOLD
            // tween; the shell's settle sweep drops the rows afterwards.
            Some(id) => self.folds.get(id).is_some_and(|fold| {
                fold.open == Some(true)
                    || fold
                        .toggled_at
                        .is_some_and(|at| at.elapsed() < FOLD_TWEEN_WINDOW)
            }),
        });
        rows
    }

    /// Fetch a sidecar blob (full tool output or diff) and build its upgraded
    /// [`ToolDetail`] once, off the render path. Re-entry while Loading/Ready
    /// is a no-op; Failed re-arms as a retry (the affordance label says so).
    fn spawn_blob_fetch(&mut self, blob_ref: SharedString, cx: &mut Context<Self>) {
        // Rank BEFORE the already-fetched guard: clicking a Ready ref is the
        // "show me this one again" toggle (recency bump + repaint, no
        // re-fetch) — with both a diff and an output fetched, the two
        // affordances must be able to trade places forever.
        self.blob_fetch_counter += 1;
        self.blob_fetch_order
            .insert(blob_ref.clone(), self.blob_fetch_counter);
        match self.blob_details.get(&blob_ref) {
            Some(BlobFetch::Ready(_)) => {
                cx.notify();
                return;
            }
            Some(BlobFetch::Loading(_)) => return,
            Some(BlobFetch::Failed) | None => {}
        }
        let Some(rpc) = self.host.rpc(cx) else {
            return;
        };
        let is_diff = blob_ref.ends_with(".diff");
        let ref_key = blob_ref.clone();
        let task = cx.spawn(async move |this, cx| {
            let reply = crate::rpc::call_with_timeout(
                &rpc,
                cx.background_executor(),
                FETCH_TOOL_BLOB,
                serde_json::json!({ "blobRef": ref_key.as_ref() }),
                Duration::from_secs(20),
            )
            .await;
            let fetched = match reply {
                Ok(value) => {
                    let text = value
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or_default();
                    blob_detail(text, is_diff)
                        .map(|d| BlobFetch::Ready(Arc::new(d)))
                        .unwrap_or(BlobFetch::Failed)
                }
                Err(_) => BlobFetch::Failed,
            };
            this.update(cx, |this, cx| {
                this.blob_details.insert(ref_key, fetched);
                cx.notify();
            })
            .ok();
        });
        self.blob_details.insert(blob_ref, BlobFetch::Loading(task));
    }

    /// Expand/collapse one long user bubble. Heights come from the text's
    /// passive paint cache, never from transcript state, so this changes
    /// render-local fold state only and does not rebuild or splice rows.
    fn toggle_user_fold(
        &mut self,
        row_id: SharedString,
        row_ix: usize,
        collapsed_h: f32,
        full_h: f32,
        reduced_motion: bool,
    ) {
        let duration_ms = user_resize_duration_ms(full_h - collapsed_h);
        // A fold owns the viewport just like explicit navigation. Release
        // the sent-turn hold as well as the spring: otherwise growing beyond
        // the reserved space hands the still-held turn back to the bottom
        // pin and hides the beginning of the newly expanded prompt.
        self.begin_scroll_navigation();
        let entry = self.user_folds.entry(row_id).or_default();
        let currently_open = entry.open.unwrap_or(false);
        entry.from = if currently_open { full_h } else { collapsed_h };
        entry.open = Some(!currently_open);
        entry.epoch += 1;
        entry.toggled_at = Some(Instant::now());
        entry.duration_ms = duration_ms;
        entry.user_expansion_height = (full_h - collapsed_h).max(0.0);

        // Capture a screen-space anchor for the clicked row. We do not subtract
        // the removed height: that is only correct when the row's top is
        // exactly at the viewport bottom. At the top or middle of a long
        // message it overscrolls past the collapsed bubble. The same anchor is
        // used for expansion, with the full target height, so a bottom-pinned
        // prompt reveals its beginning below the top fade instead of above it.
        let Some(item_bounds) = self.list.bounds_for_item(row_ix) else {
            return;
        };
        let viewport = self.list.viewport_bounds();
        let initial_top = f32::from(item_bounds.top());
        // Keep a newly revealed bubble below the transcript's top fade band. A
        // 12px inset alone still leaves the first lines washed into the edge
        // fade when the expanded row started above view.
        let viewport_top = f32::from(viewport.top()) + Theme::TRANSCRIPT_FADE_BAND + 28.0;
        let target_height = if currently_open { collapsed_h } else { full_h };
        let viewport_bottom = f32::from(viewport.bottom()) - target_height - 12.0;
        let target_top = if viewport_bottom >= viewport_top {
            initial_top.clamp(viewport_top, viewport_bottom)
        } else {
            viewport_top
        };
        let needs_scroll = (target_top - initial_top).abs() > 0.5;

        if needs_scroll {
            if reduced_motion {
                if let Some(current) = self.list.bounds_for_item(row_ix) {
                    self.list
                        .scroll_by(px(f32::from(current.top()) - target_top));
                }
            } else {
                self.user_collapse_scroll = Some(UserCollapseScroll {
                    started_at: Instant::now(),
                    duration_ms,
                    height_delta: (full_h - collapsed_h).max(0.0),
                    row_ix,
                    initial_top,
                    target_top,
                });
            }
        }
    }

    fn cancel_user_hold(&mut self) {
        self.user_hold_token = self.user_hold_token.wrapping_add(1);
        self.user_hold_task = None;
    }

    /// Arm a long-press toggle instead of using double-click. Releasing before
    /// the threshold preserves an ordinary click/selection gesture; moving
    /// cancels the timer so drag selection never unexpectedly toggles the
    /// message.
    fn arm_user_hold(
        &mut self,
        row_id: SharedString,
        row_ix: usize,
        collapsed_h: f32,
        measured_h: Rc<Cell<f32>>,
        selection_key: Arc<str>,
        cx: &mut Context<Self>,
    ) {
        const USER_HOLD_DELAY: Duration = Duration::from_millis(360);
        self.cancel_user_hold();
        self.user_hold_token = self.user_hold_token.wrapping_add(1);
        let token = self.user_hold_token;
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(USER_HOLD_DELAY).await;
            this.update(cx, |this, cx| {
                if this.user_hold_token != token {
                    return;
                }
                this.user_hold_task = None;
                crate::markdown::selection::clear_if_owner(&selection_key);
                this.toggle_user_fold(
                    row_id,
                    row_ix,
                    collapsed_h,
                    measured_h.get().max(collapsed_h),
                    motion::reduced_motion(cx),
                );
                cx.notify();
            })
            .ok();
        });
        self.user_hold_task = Some(task);
    }

    fn step_user_collapse_scroll(&mut self, cx: &mut Context<Self>) {
        let Some(scroll) = self.user_collapse_scroll.as_ref() else {
            return;
        };
        let started_at = scroll.started_at;
        let duration_ms = scroll.duration_ms;
        let height_delta = scroll.height_delta;
        let row_ix = scroll.row_ix;
        let initial_top = scroll.initial_top;
        let target_top = scroll.target_top;
        let raw =
            (started_at.elapsed().as_secs_f32() / (duration_ms as f32 / 1000.0)).clamp(0.0, 1.0);
        let spec = user_resize_spec(height_delta);
        let progress = spec.progress(raw);
        let desired_top = motion::lerp(initial_top, target_top, progress);
        if let Some(current) = self.list.bounds_for_item(row_ix) {
            // `scroll_by(+x)` moves content up, so correcting current minus
            // desired keeps the row on the interpolated screen-space path.
            let correction = f32::from(current.top()) - desired_top;
            if correction.abs() > 0.1 {
                self.list.scroll_by(px(correction));
            }
        }
        if raw >= 1.0 {
            self.user_collapse_scroll = None;
            self.last_scroll_distance = self.distance_from_bottom();
            self.show_jump_button =
                jump_visibility(self.show_jump_button, self.last_scroll_distance);
        }
        cx.notify();
    }

    pub fn toggle_fold(
        &mut self,
        row_id: SharedString,
        open_height: f32,
        auto_open: bool,
        cx: &mut Context<Self>,
    ) {
        let shell_ix = self.rows.iter().position(|row| {
            row.id == row_id
                && matches!(
                    row.kind,
                    RowKind::ToolGroup {
                        compact_shell: true,
                        ..
                    }
                )
        });
        // A compact shell tweens its sibling body rows, so the tween opens
        // from the body's CURRENT rendered height — including mid-tween
        // reversals — not the header's analytic chip height.
        let body_height = shell_ix.map(|ix| {
            let total = self.compact_fold_total(&row_id, ix);
            let fold = self.folds.get(&row_id).copied().unwrap_or_default();
            compact_body_height(fold, total, motion::reduced_motion(cx), Instant::now())
        });
        let now_open = {
            let entry = self.folds.entry(row_id.clone()).or_default();
            let currently_open = entry.open.unwrap_or(auto_open);
            entry.from = body_height.unwrap_or(if currently_open { open_height } else { 0.0 });
            entry.open = Some(!currently_open);
            entry.epoch += 1;
            entry.toggled_at = Some(Instant::now());
            entry.disclosure_at = entry.toggled_at;
            !currently_open
        };
        if shell_ix.is_none() {
            return;
        }
        let reduce = motion::reduced_motion(cx);
        if now_open || reduce {
            if reduce && !now_open {
                // Instant transition — no tween, so nothing retains the body.
                if let Some(entry) = self.folds.get_mut(&row_id) {
                    entry.toggled_at = None;
                }
            }
            // Mount the folded rows so the tween has content to reveal —
            // or, under reduced motion, drop them immediately on close.
            self.last_source = None;
            self.sync(cx);
        } else {
            // The body stays mounted through the close tween; sweep it out
            // of the row list once the retention window elapses.
            self.compact_fold_settle = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(FOLD_TWEEN_WINDOW + Duration::from_millis(50))
                    .await;
                this.update(cx, |this, cx| {
                    this.last_source = None;
                    this.sync(cx);
                    cx.notify();
                })
                .ok();
            }));
        }
    }

    /// A folded body row's natural height as measured by its prepaint probe;
    /// zero until the row's first paint lands the measurement.
    fn compact_fold_height(&self, row_id: &SharedString) -> f32 {
        self.compact_fold_heights
            .get(row_id)
            .map_or(0.0, |cell| cell.get())
    }

    /// Total natural height of the contiguous body rows following a compact
    /// shell header (`shell_ix` is the header's row index).
    fn compact_fold_total(&self, work_id: &SharedString, shell_ix: usize) -> f32 {
        self.rows
            .iter()
            .skip(shell_ix + 1)
            .take_while(|row| row.compact_fold.as_ref() == Some(work_id))
            .map(|row| self.compact_fold_height(&row.id))
            .sum()
    }

    // ---- attachment read-back (user-attachments.tsx + transcript cache) ----

    /// The inside of a user bubble: the prompt text, clipped to
    /// [`USER_COLLAPSED_LINES`] until expanded, plus the expander chevron for
    /// prompts past the cap. Returns the bubble's children in order.
    ///
    /// The collapsed form clips a normally-laid-out text element at exactly
    /// five line boxes. Do not use gpui's `line_clamp` here: on an auto-width
    /// flex item it answers intrinsic-width probes with the truncated layout,
    /// collapsing the bubble to min-content width (one character per line).
    /// A plain height clip preserves the original bubble width calculation and
    /// never feeds measured layout back into the virtualized list.
    fn render_user_body(
        &mut self,
        row_id: &SharedString,
        row_ix: usize,
        text: SharedString,
        mentions: Arc<Vec<crate::composer_mentions::SentMentionSpan>>,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let chip_overlays = M::sent_chip_overlays(self, row_id, &mentions, theme, cx);
        let fold = self.user_folds.get(row_id).copied().unwrap_or_default();
        let expanded = fold.open.unwrap_or(false);
        let line_height =
            f32::from(crate::typography::ui_rems(USER_LINE_HEIGHT).to_pixels(window.rem_size()));
        let collapsed_text_h = USER_COLLAPSED_LINES as f32 * line_height;
        // Include the continuation line in the resize endpoints so removing
        // it on expansion does not make the bubble jump by a line.
        let collapsed_h = collapsed_text_h + line_height;
        let measured_h = self
            .user_heights
            .entry(row_id.clone())
            .or_insert_with(|| Rc::new(Cell::new(0.0)))
            .clone();
        let measured = measured_h.get();
        let collapsible = text.lines().count() > USER_COLLAPSED_LINES
            || (measured > 0.0 && measured > collapsed_text_h + 0.5)
            || (measured == 0.0 && user_message_needs_collapse(&text));
        let full_h = measured_h.get().max(collapsed_h);
        if let Some(fold) = self.user_folds.get_mut(row_id) {
            // Wrapping can change with the window width while expanded.
            fold.user_expansion_height = (full_h - collapsed_h).max(0.0);
        }

        let hold_key = row_id.clone();
        let hold_height = measured_h.clone();
        let hold_selection: Arc<str> = format!("{row_id}:u").into();
        let body = div()
            .id(SharedString::from(format!("{row_id}-body")))
            // A long press toggles instead of double-click. A normal release
            // remains available for text selection, and pointer movement
            // cancels the pending toggle before a drag can select text.
            .when(collapsible, |el| {
                let down_key = hold_key.clone();
                let down_height = hold_height.clone();
                let down_selection = hold_selection.clone();
                el.on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.arm_user_hold(
                            down_key.clone(),
                            row_ix,
                            collapsed_h,
                            down_height.clone(),
                            down_selection.clone(),
                            cx,
                        );
                    }),
                )
                .on_mouse_up(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, _, _, _| {
                        this.cancel_user_hold();
                    }),
                )
                .on_mouse_up_out(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, _, _, _| {
                        this.cancel_user_hold();
                    }),
                )
                .on_mouse_move(cx.listener(|this, _, _, _| {
                    this.cancel_user_hold();
                }))
            })
            .child(user_bubble_text(
                row_id,
                text,
                mentions,
                theme,
                measured_h.clone(),
                cx.entity_id(),
                chip_overlays,
                Rc::new(cx.listener(|this, path: &SharedString, window, cx| {
                    M::open_sent_chip_image(this, path, window, cx);
                })),
            ));
        // Height motion uses the same ease-out curve as sidebars, tool folds,
        // and pane transitions, with duration scaled to travel distance. The
        // full text remains laid out behind the clip; only the viewport over it
        // changes, so glyph wrapping never shifts.
        let duration_ms = fold
            .duration_ms
            .max(user_resize_duration_ms(full_h - collapsed_h));
        let animating = collapsible
            && fold.epoch > 0
            && fold
                .toggled_at
                .is_some_and(|at| at.elapsed() < Duration::from_millis(duration_ms + 200))
            && !motion::reduced_motion(cx);
        let ellipsis = || div().h(px(line_height)).child("...");
        let body: AnyElement = if animating {
            let from = fold.from;
            let to = if expanded { full_h } else { collapsed_h };
            let resize = user_resize_spec(full_h - collapsed_h);
            let ellipsis_h = if expanded { 0.0 } else { line_height };
            div()
                .child(div().overflow_hidden().child(body).with_animation(
                    SharedString::from(format!("{row_id}-user-resize-{}", fold.epoch)),
                    resize.animation(),
                    move |el, t| el.h(px((motion::lerp(from, to, t) - ellipsis_h).max(0.0))),
                ))
                .when(!expanded, |el| el.child(ellipsis()))
                .into_any_element()
        } else if collapsible && !expanded {
            div()
                .child(div().h(px(collapsed_text_h)).overflow_hidden().child(body))
                .child(ellipsis())
                .into_any_element()
        } else {
            body.into_any_element()
        };
        div()
            .relative()
            .child(body)
            .when(collapsible, |el| {
                el.child(self.render_user_expander(
                    row_id,
                    row_ix,
                    expanded,
                    collapsed_h,
                    measured_h,
                    theme,
                    cx,
                ))
            })
            .into_any_element()
    }

    /// A plain text link aligned with the message's left edge, following the
    /// continuation ellipsis when collapsed. No pill, border, or button wash.
    fn render_user_expander(
        &mut self,
        row_id: &SharedString,
        row_ix: usize,
        expanded: bool,
        collapsed_h: f32,
        measured_h: Rc<Cell<f32>>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let toggle_key = row_id.clone();
        let glyph = if expanded {
            crate::icons::ALT_ARROW_UP
        } else {
            crate::icons::ALT_ARROW_DOWN
        };
        let label = if expanded { "Show less" } else { "Show more" };
        let button = div()
            .id(SharedString::from(format!("{row_id}-expander")))
            .group("user-message-toggle")
            .role(gpui::Role::Button)
            .aria_label(if expanded {
                "Collapse message"
            } else {
                "Expand message"
            })
            .aria_expanded(expanded)
            .flex()
            .items_center()
            .gap(px(5.0))
            .text_size(crate::typography::ui_rems(14.0))
            .line_height(crate::typography::ui_rems(USER_LINE_HEIGHT))
            .text_color(theme.text_muted)
            .cursor_pointer()
            .hover(|s| s.text_color(theme.text))
            .child(label)
            .child(
                crate::icons::icon(glyph)
                    .size(px(12.0))
                    .text_color(theme.text_muted)
                    .group_hover("user-message-toggle", |s| s.text_color(theme.text)),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_user_fold(
                    toggle_key.clone(),
                    row_ix,
                    collapsed_h,
                    measured_h.get().max(collapsed_h),
                    motion::reduced_motion(cx),
                );
                cx.notify();
            }));
        div()
            .mt(px(USER_TOGGLE_GAP))
            .flex()
            .items_start()
            .child(button)
            .into_any_element()
    }

    /// A diagram replaced its source (or the reverse): remeasure the rows
    /// painting it and let the bottom pin and the own-turn runway absorb the
    /// height change, exactly like any other layout-affecting row update.
    pub fn diagram_layout_changed(&mut self, rows: &[SharedString], cx: &mut Context<Self>) {
        let mut changed = false;
        for (ix, row) in self.rows.iter().enumerate() {
            if rows.contains(&row.id) {
                self.list.remeasure_items(ix..ix + 1);
                changed = true;
            }
        }
        if !changed {
            return;
        }
        self.viewport_layout_revision = self.viewport_layout_revision.wrapping_add(1);
        if self.pinned {
            self.wake_spring();
        }
        if self.own_turn.is_some() {
            self.own_turn_kick = true;
        }
        cx.notify();
    }

    // ---- rendering ----

    /// The working loader, INSIDE the conversation flow: appended under the
    /// last row while the run is live (moved out of the shell's status strip
    /// — user request), so it reads as part of the streaming reply and
    /// scrolls away with it. The spinner drives this entity's frames, which
    /// keeps the elapsed timer ticking through delta-quiet tool runs.
    /// The failed-send retry (trailer affordance): re-kick every delivery
    /// road engine-side (fresh chat2 socket, host nudge, delivery escorts)
    /// and restart the grace clock so the trailer returns to Sending/Queued
    /// while the retry runs.
    fn retry_send(&mut self, cx: &mut Context<Self>) {
        let Some(chat_id) = self.chat_id.clone() else {
            return;
        };
        self.host.clone().retry_send(&chat_id, cx);
    }

    fn render_working_trailer(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let now = chrono::Utc::now();
        // `turn` (the turn's start, ms) keys the label and timer animations
        // so each turn starts fresh.
        let (sending, queued, elapsed_secs, turn) = if let Some(doc_id) = &self.doc_override {
            // A subagent doc has no Session row — `indicator_for` would read
            // the PARENT chat's live state into this tab. Liveness rides the
            // doc itself instead: the sink's assistant entry streams until
            // the subagent settles (run teardown finalizes abandoned sinks),
            // and a trailing USER entry is a steer still awaiting its reply
            // segment. Frozen snapshots never spin, whatever they claim.
            if !self.doc_live {
                return None;
            }
            let last = self.host.entries(Some(doc_id), cx).last()?;
            let live =
                last.status == Some(MessageStatus::Streaming) || last.role == MessageRole::User;
            if !live {
                return None;
            }
            let elapsed = turn_elapsed_secs(Some(last.created_at), None, now.timestamp_millis());
            (false, false, elapsed, last.created_at)
        } else {
            let chat_id = self.chat_id.clone()?;
            // Failed-send state first: past the grace window the trailer IS
            // the retry affordance, whatever the indicator fell back to.
            if self.host.send_undelivered(&chat_id, now, cx) {
                let theme = Theme::of(cx).clone();
                return Some(
                    div()
                        .id("undelivered-retry")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Theme::SPACE_SM))
                        .pt(px(Theme::SPACE_LG))
                        .text_size(crate::typography::ui_rems(12.0))
                        .text_color(theme.danger)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.retry_send(cx)))
                        .child(SharedString::from("Not delivered — click to retry"))
                        .into_any_element(),
                );
            }
            let activity = self.host.turn_activity(&chat_id, now, cx)?;
            (
                activity.sending,
                activity.queued,
                activity.elapsed_secs,
                activity.turn_ms,
            )
        };
        if self.compact_mode
            && !sending
            && !queued
            && let Some(elapsed_secs) = elapsed_secs.filter(|secs| *secs > 0)
        {
            let entry_id = if let Some(doc_id) = &self.doc_override {
                self.host
                    .entries(Some(doc_id), cx)
                    .last()
                    .map(|e| e.id.clone())
            } else {
                self.host
                    .entries(None, cx)
                    .iter()
                    .rev()
                    .find(|e| e.role == MessageRole::Assistant)
                    .map(|e| e.id.clone())
            };
            if let Some(entry_id) = entry_id {
                self.compact_last_elapsed.insert(entry_id, elapsed_secs);
            }
        }
        // OpenAgents (#11969): name the running tool and time it.
        let tool = (!sending && !queued && self.doc_override.is_none())
            .then(|| self.host.running_tool(chrono::Utc::now(), cx))
            .flatten();
        let elapsed_secs = tool
            .as_ref()
            .map_or(elapsed_secs, |tool| Some(tool.elapsed_secs));
        let word = if queued {
            "Queued — will send automatically"
        } else if sending {
            "Sending"
        } else if let Some(tool) = &tool {
            tool.label.as_str()
        } else {
            "Working"
        };
        let theme = Theme::of(cx).clone();
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Theme::SPACE_SM))
                .pt(px(Theme::SPACE_LG))
                .text_size(crate::typography::ui_rems(11.0))
                .child(crate::loaders::gradient_spinner(
                    "working-indicator",
                    &theme,
                    2.5,
                    cx.entity_id(),
                    cx,
                ))
                .child(
                    // Animate transitions between delivery and working states.
                    div()
                        .text_size(crate::typography::ui_rems(12.0))
                        .text_color(if queued {
                            theme.warning
                        } else {
                            theme.text_muted
                        })
                        .child(crate::roll_text::roll_text(
                            format!("working-word-{}-{turn}", cx.entity_id()),
                            SharedString::from(if queued {
                                word.to_string()
                            } else {
                                format!("{word}…")
                            }),
                            cx.reduce_motion(),
                        )),
                )
                .when_some(elapsed_secs.filter(|_| !sending), |el, elapsed_secs| {
                    // The timer ticks every second; its digits roll in place
                    // (the unit suffix holds).
                    el.child(
                        div()
                            .relative()
                            .top(px(1.0))
                            .text_color(theme.text_faint)
                            .child(crate::roll_text::roll_text(
                                format!("working-timer-{}-{turn}", cx.entity_id()),
                                SharedString::from(format_elapsed(elapsed_secs)),
                                cx.reduce_motion(),
                            )),
                    )
                })
                .into_any_element(),
        )
    }

    fn render_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(row) = self.rows.get(ix).cloned() else {
            return gpui::Empty.into_any_element();
        };
        self.rendered_rows.insert(row.id.clone());
        let theme = Theme::of(cx).clone();
        let side_gutter = self.host_layout.map_or(48.0, |layout| layout.side);
        let workspace_root = {
            self.host
                .chat_place(self.chat_id.as_deref(), cx)
                .and_then(|place| place.cwd)
                .map(SharedString::from)
        };
        // The viewport spans the full window (under the titlebar): the first
        // row's gap adds the titlebar's height so a top-scrolled transcript
        // rests below the chrome it fades under. The right pane already pads
        // for the titlebar — an override instance's first row keeps only the
        // ordinary turn gap, or the content sits double-chrome low.
        let top_gap = if ix == 0 {
            if let Some(layout) = self.host_layout {
                layout.top
            } else if self.doc_override.is_some() {
                Theme::SPACE_LG
            } else {
                Theme::TITLEBAR_HEIGHT + Theme::SPACE_LG + 10.0
            }
        } else {
            top_gap_for(ix.checked_sub(1).and_then(|i| self.rows.get(i)), &row)
        };
        // The last row must clear the composer/status stack the transcript
        // scrolls under PLUS the fade band above it, or the timestamp strip
        // (the row's lowest content) renders half-faded (or hidden) when the
        // transcript is pinned to the bottom.
        let is_last = ix + 1 == self.rows.len();
        let bottom_pad = if is_last {
            match self.host_layout {
                Some(layout) => layout.bottom,
                None => self.bottom_clearance + Theme::TRANSCRIPT_FADE_BAND + 8.0,
            }
        } else {
            0.0
        };
        // Live-run loader rides under the LAST row's content (above its
        // clearance pad), so it sits right beneath the working reply.
        let trailer = (ix + 1 == self.rows.len())
            .then(|| self.render_working_trailer(cx))
            .flatten();

        let inner: AnyElement = match &row.kind {
            RowKind::User {
                text,
                mentions,
                attachments,
                badges,
                pending,
            } => {
                let attachments = attachments.clone();
                let badges = badges.clone();
                let text = text.clone();
                let mentions = mentions.clone();
                let pending = *pending;
                // Attachment thumbnails ride ABOVE the bubble, right-aligned
                // (chat-view.tsx RowView: UserAttachmentStrip then the text
                // HStack); image-only sends show no bubble at all.
                let mut column = div().w_full().flex().flex_col();
                if !attachments.is_empty() {
                    column =
                        column.child(M::user_attachments(self, &row.id, &attachments, window, cx));
                }
                if !badges.is_empty() {
                    column = column.child(
                        div()
                            .w_full()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .justify_end()
                            .items_center()
                            .gap(px(6.0))
                            .pb(px(6.0))
                            .children(badges.iter().enumerate().map(|(bix, badge)| {
                                crate::badges::render(
                                    SharedString::from(format!("{}#badge{bix}", row.id)),
                                    badge,
                                    &theme,
                                )
                            })),
                    );
                }
                if !text.is_empty() {
                    // `min_w_0` is load-bearing: gpui text answers min/max-content
                    // probes with its UNWRAPPED width, so without it the bubble's
                    // automatic min-size is the full single-line width — the flex
                    // item can't shrink, `justify_end` pushes the overflow off the
                    // left edge, and long prompts render as one clipped line
                    // instead of wrapping inside the 80% column cap.
                    column = column.child(
                        div().w_full().flex().justify_end().child(
                            div()
                                .min_w_0()
                                .max_w(px(self.content_width * 0.8))
                                .bg(if self.host_layout.is_some_and(|l| l.plates) {
                                    user_plate_bg()
                                } else {
                                    crate::theme::user_bubble_bg()
                                })
                                .rounded(px(Theme::BUBBLE_RADIUS))
                                .px(px(16.0))
                                .py(px(10.0))
                                .text_size(crate::typography::ui_rems(14.0))
                                .line_height(crate::typography::ui_rems(USER_LINE_HEIGHT))
                                .text_color(theme.text)
                                .when(pending, |el| el.opacity(0.65))
                                .child(self.render_user_body(
                                    &row.id, ix, text, mentions, &theme, window, cx,
                                )),
                        ),
                    );
                }
                column.into_any_element()
            }
            RowKind::Markdown { tree, block_ix } => {
                let link = self.link_ui(cx);
                let tree = self.inline_code_tree(tree, link.as_ref(), cx);
                let Some(top) = tree.blocks.get(*block_ix) else {
                    return gpui::Empty.into_any_element();
                };
                let code = self.code_uis_for(&row.id, &top.block, *block_ix, cx);
                let opts = RenderOptions {
                    tasks: None,
                    media: M::markdown_media(self, cx),
                    row_key: row.id.clone(),
                    veil: None,
                    cache: (!render_cache_disabled()).then(|| self.render_cache.clone()),
                    now: Instant::now(),
                    copy: Some(self.copy_ui_for(&row.id, cx)),
                    link,
                    workspace_root: workspace_root.clone(),
                    code,
                };
                let highlight = self.code_highlight_for(&row.id, &tree, Some(*block_ix), cx);
                render::render_block(
                    &top.block,
                    *block_ix,
                    *block_ix,
                    &opts,
                    &theme,
                    window,
                    highlight
                        .get(block_ix)
                        .and_then(|o| o.as_deref())
                        .map(|document| document.lines.as_slice()),
                )
            }
            RowKind::LiveMarkdown { tree, block_ix } => {
                let link = self.link_ui(cx);
                let tree = self.inline_code_tree(tree, link.as_ref(), cx);
                let Some(top) = tree.blocks.get(*block_ix) else {
                    return gpui::Empty.into_any_element();
                };
                let code = self.code_uis_for(&row.id, &top.block, *block_ix, cx);
                // Per-appended-chunk fade veil (opacity only — layout commits
                // instantly). Reduced motion renders with no veil at all.
                // Baseline rows (text already streamed when the transcript
                // attached) start seeded: the existing reply must not fade in
                // on a session switch — only fresh appends animate.
                let reduced_motion = motion::reduced_motion(cx);
                if reduced_motion {
                    // Motion can resume mid-stream (background pause, or the OS
                    // setting flipping back). Text painted meanwhile is already
                    // on screen, so the next veil seeds it rather than
                    // dissolving the reply in again.
                    self.veils.remove(&row.id);
                    self.veil_baseline.insert(row.id.clone());
                }
                let seed_history = !self.veils.contains_key(&row.id)
                    && self.historical_markdown.contains_key(&row.id);
                let veil = (!reduced_motion).then(|| {
                    self.veils
                        .entry(row.id.clone())
                        .or_insert_with(|| {
                            if seed_history || self.veil_baseline.contains(&row.id) {
                                Rc::new(RefCell::new(RowVeil::seeded()))
                            } else {
                                Rc::default()
                            }
                        })
                        .clone()
                });
                // A streaming fence may still be growing, so only blocks the
                // reply has already moved past draw diagrams: a later row of
                // the same entry proves this block is complete. The tail
                // keeps its source until the next block or completion, and
                // per-token commits never start a render.
                let settled = self
                    .rows
                    .get(ix + 1)
                    .is_some_and(|next| next.entry_id == row.entry_id);
                let opts = RenderOptions {
                    tasks: None,
                    media: settled.then(|| M::markdown_media(self, cx)).flatten(),
                    row_key: row.id.clone(),
                    veil: veil.clone(),
                    cache: (!render_cache_disabled()).then(|| self.render_cache.clone()),
                    now: Instant::now(),
                    copy: Some(self.copy_ui_for(&row.id, cx)),
                    link,
                    workspace_root: workspace_root.clone(),
                    code,
                };
                if seed_history && let Some(veil) = &veil {
                    let historical = &self.historical_markdown[&row.id];
                    if let RowKind::LiveMarkdown { tree, block_ix } = &historical.kind
                        && let Some(top) = tree.blocks.get(*block_ix)
                    {
                        // Use the renderer's own nested element keys and text
                        // flattening, but never cache/paint the historical tree.
                        let seed_opts = RenderOptions {
                            cache: None,
                            link: None,
                            // Seed fences as source text, never request diagrams.
                            media: None,
                            ..opts.clone()
                        };
                        let _ = render::render_block(
                            &top.block, *block_ix, *block_ix, &seed_opts, &theme, window, None,
                        );
                        veil.borrow_mut().finish_seeding();
                    }
                }
                let highlight = self.code_highlight_for(&row.id, &tree, Some(*block_ix), cx);
                let timer = frame_stats_enabled().then(Instant::now);
                let el = render::render_block(
                    &top.block,
                    *block_ix,
                    *block_ix,
                    &opts,
                    &theme,
                    window,
                    highlight
                        .get(block_ix)
                        .and_then(|o| o.as_deref())
                        .map(|document| document.lines.as_slice()),
                );
                if let Some(start) = timer {
                    record_live_frame_us(start.elapsed().as_micros() as u64);
                }
                // The attach pass for this row is done (every element rendered
                // above seeded its baseline synchronously): elements appearing
                // from the NEXT pass on are newly streamed and fade normally.
                if let Some(veil) = &veil {
                    veil.borrow_mut().finish_seeding();
                }
                // Share the loaders' bounded clock. A display-frame callback
                // here would pin the transcript to 60/120Hz for the whole
                // stream, bypassing the clock even with no loader mounted.
                if veil.is_some_and(|v| v.borrow().is_fading()) {
                    motion::pulse_lease(cx.entity_id(), cx);
                }
                el
            }
            RowKind::ToolGroup {
                tools,
                auto_open,
                summary,
                worked_secs,
                compact_shell,
            } => self.render_tool_group(
                &row.id,
                tools,
                summary,
                *auto_open,
                *worked_secs,
                *compact_shell,
                &theme,
                cx,
            ),
            RowKind::InputChip { header, resolved } => {
                input_chip(header.clone(), *resolved, &theme)
            }
            RowKind::GeneratedImage {
                owner,
                path,
                name,
                mime_type,
            } => M::generated_image(self, &row.id, owner, path, name, mime_type, cx),
            RowKind::Visual { owner, path, title } => {
                M::visual(self, &row.id, owner, path, title, cx)
            }
            RowKind::ErrorChip { message } => error_chip(message.clone(), &theme),
            RowKind::ForkMarker {
                source_chat_id,
                source_title,
            } => fork_marker(source_chat_id.clone(), source_title.clone(), &theme),
        };
        // Diagram fences this row just requested start rendering after layout.
        M::after_row(self, cx);

        // The app's own content for the entry, under its last row.
        let after_entry = row
            .timestamp
            .and_then(|_| M::after_entry(self, &row.entry_id, window, cx));

        // Hover-revealed metadata strip: a RESERVED 32px lane under the
        // entry's last row. Timestamp, copy action, and copied feedback only
        // flip visibility/content, so none of them shifts the virtualizer.
        // User entries align end (under the bubble), assistant entries start.
        // Both read timestamp first, then the copy action.
        let is_user_row = matches!(row.kind, RowKind::User { .. });
        let hovered = self
            .hovered_entry
            .as_ref()
            .is_some_and(|(_, entry)| entry == &row.entry_id);
        let copied_message = self.copied_message.as_ref() == Some(&row.entry_id);
        let copy_text = row.copy_text.clone();
        let copy_entry_id = row.entry_id.clone();
        let strip = row.timestamp.map(|ms| {
            let timestamp = div()
                .text_size(crate::typography::ui_rems(12.0))
                .text_color(theme.text_muted.opacity(0.55))
                // A host without message times (the web's site chats) leaves
                // the time out and keeps the copy action.
                .child(if ms > 0 {
                    SharedString::from(format_timestamp(ms, &chrono::Local))
                } else {
                    SharedString::default()
                });
            let copy = copy_text.map(|text| {
                let entry_id = copy_entry_id.clone();
                let fade_key = format!("copy-message-hover-{entry_id}");
                div()
                    .id(SharedString::from(format!("copy-message-{entry_id}")))
                    .size(px(Theme::SPACE_MD * 2.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(Theme::CONTROL_RADIUS))
                    .cursor_pointer()
                    // Same quiet icon-button treatment as the copy action
                    // over transcript code blocks.
                    .bg(motion::hover_blend(
                        &fade_key,
                        gpui::transparent_black(),
                        crate::theme::ink(0.08),
                    ))
                    .on_hover(motion::hover_listener(fade_key))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.copy_message(entry_id.clone(), text.clone(), cx)
                    }))
                    .tooltip(crate::tooltip::text_tooltip("Copy message"))
                    .child(
                        crate::icons::icon(if copied_message {
                            crate::icons::CHECK
                        } else {
                            crate::icons::COPY
                        })
                        .size(px(14.0))
                        .text_color(theme.text_muted),
                    )
            });
            let metadata = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Theme::SPACE_SM));
            let metadata = metadata.child(timestamp).children(copy);
            div()
                .h(px(Theme::SPACE_SM + Theme::SPACE_MD * 2.0))
                .pt(px(Theme::SPACE_SM))
                .w_full()
                .flex()
                .items_center()
                // No horizontal inset: the original's `px-1` netted out flush
                // because its message text was inset by the same amount (group
                // padding 4 + inner VStack 4 = 8 = group 4 + px-1 4). Here the
                // markdown text / user bubble sit AT the content column edges,
                // so the label must too — assistant label's left edge on the
                // text's first-character x, user label's right edge on the
                // bubble's right edge (user-reported 4px drift).
                .when(is_user_row, |el| el.justify_end())
                .when(hovered, |el| {
                    el.child(motion::fade_quick(
                        SharedString::from(format!("meta-{}", row.id)),
                        metadata,
                    ))
                })
        });
        // On a host with plates, a reply's rows join into one plate: the
        // gap above a row of the same reply moves inside it, and only the
        // reply's first and last rows round their corners.
        let plated = self.host_layout.is_some_and(|layout| layout.plates)
            && !matches!(row.kind, RowKind::User { .. });
        let joins_above = plated
            && ix
                .checked_sub(1)
                .and_then(|i| self.rows.get(i))
                .is_some_and(|above| above.entry_id == row.entry_id);
        let joins_below = plated
            && self
                .rows
                .get(ix + 1)
                .is_some_and(|below| below.entry_id == row.entry_id);
        let (outer_top, plate_top) = if joins_above {
            (0.0, top_gap)
        } else {
            (top_gap, 0.0)
        };
        let entry_id = row.entry_id.clone();
        let row_id = row.id.clone();
        let column = div()
            .w_full()
            .max_w(px(self.content_width))
            .min_w_0()
            .child(inner)
            .children(after_entry)
            .children(strip)
            .children(trailer);
        let column = if plated {
            let radius = px(Theme::BUBBLE_RADIUS);
            div()
                .w_full()
                .max_w(px(self.content_width))
                .min_w_0()
                .bg(reply_plate_bg())
                .px(px(PLATE_PAD_X))
                .pt(px(plate_top + if joins_above { 0.0 } else { PLATE_PAD_Y }))
                .when(!joins_below, |plate| plate.pb(px(PLATE_PAD_Y)))
                .when(!joins_above, |plate| {
                    plate.rounded_tl(radius).rounded_tr(radius)
                })
                .when(!joins_below, |plate| {
                    plate.rounded_bl(radius).rounded_br(radius)
                })
                .child(column)
        } else {
            column
        };
        let outer = div()
            .id(row.id.clone())
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    let next = Some((row_id.clone(), entry_id.clone()));
                    if this.hovered_entry != next {
                        let entry_changed = this
                            .hovered_entry
                            .as_ref()
                            .is_none_or(|(_, entry)| entry != &entry_id);
                        this.hovered_entry = next;
                        if entry_changed {
                            cx.notify();
                        }
                    }
                } else if this
                    .hovered_entry
                    .as_ref()
                    .is_some_and(|(row, _)| row == &row_id)
                {
                    // Only the row that OWNS the current reveal may clear it —
                    // a stale leave from an earlier row must not blank the
                    // strip the newly entered row just lit.
                    this.hovered_entry = None;
                    cx.notify();
                }
            }))
            .w_full()
            .flex()
            .justify_center()
            .pt(px(outer_top))
            .pb(px(bottom_pad))
            // Keep side gutters as the configurable column shrinks to fit;
            // a docked side card's room comes off the right.
            .pl(px(side_gutter))
            .pr(px(side_gutter + self.right_reserve))
            .child(column);
        let Some(work_id) = row.compact_fold.clone() else {
            return outer.into_any_element();
        };

        // Compact work-fold body row: the shell header tweens ONE shared
        // height budget across these sibling rows — each row clips to
        // `budget - prefix`, reproducing an ordinary fold's single
        // overflow-hidden container. A prepaint probe keeps the row's
        // natural height current so the tween never needs its layout ahead
        // of time.
        let probe = self
            .compact_fold_heights
            .entry(row.id.clone())
            .or_insert_with(|| Rc::new(Cell::new(0.0)))
            .clone();
        let body = div().relative().flex_none().w_full().child(outer).child(
            canvas(
                move |bounds, _, _| {
                    let height = f32::from(bounds.size.height);
                    if (probe.get() - height).abs() > 0.5 {
                        probe.set(height);
                    }
                },
                |_, _, _, _| (),
            )
            .absolute()
            .inset_0(),
        );
        let fold = self.folds.get(&work_id).copied().unwrap_or_default();
        let open = fold.open.unwrap_or(false);
        let reduce_motion = motion::reduced_motion(cx);
        let now = Instant::now();
        let animating = !reduce_motion
            && fold
                .toggled_at
                .is_some_and(|at| at.elapsed() < TOOL_FOLD.total());
        if !animating {
            // Settled states render plain — open rows untouched, closed rows
            // empty until the settle sweep drops them from the row list.
            return if open {
                body.into_any_element()
            } else {
                gpui::Empty.into_any_element()
            };
        }
        let (prefix, own, total) =
            compact_fold_geometry(&self.rows, &self.compact_fold_heights, &work_id, ix);
        let clip = (compact_body_height(fold, total, reduce_motion, now) - prefix).clamp(0.0, own);
        if own > 0.0 && clip >= own {
            return body.into_any_element();
        }
        if own > 0.0 && clip <= 0.0 {
            return gpui::Empty.into_any_element();
        }
        let view = cx.entity_id();
        div()
            .relative()
            .w_full()
            .overflow_hidden()
            .h(px(clip))
            .child(body)
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        window.on_next_frame(move |_, cx| cx.notify(view));
                    },
                )
                .absolute()
                .inset_0(),
            )
            .into_any_element()
    }

    fn copy_message(&mut self, entry_id: SharedString, text: SharedString, cx: &mut Context<Self>) {
        cx.stop_propagation();
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        self.copied_message = Some(entry_id);
        self.copied_message_clear = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1200))
                .await;
            this.update(cx, |this, cx| {
                this.copied_message = None;
                this.copied_message_clear = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Copy-button wiring for one row's code blocks ([`render::CopyUi`]):
    /// click writes the block's code to the clipboard and shows a transient
    /// "Copied" check on that block for ~1.2s (overlay — no layout shift).
    fn copy_ui_for(&self, row_id: &SharedString, cx: &mut Context<Self>) -> render::CopyUi {
        let copied_ix = self
            .copied_code
            .as_ref()
            .filter(|(id, _)| id == row_id)
            .map(|(_, ix)| *ix);
        let row_key = row_id.clone();
        let entity = cx.weak_entity();
        let handler: Rc<dyn Fn(usize, SharedString, &mut Window, &mut gpui::App)> =
            Rc::new(move |ix, code, _window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()));
                let row_key = row_key.clone();
                entity
                    .update(cx, |this, cx| {
                        this.copied_code = Some((row_key, ix));
                        this.copied_clear = Some(cx.spawn(async move |this, cx| {
                            cx.background_executor()
                                .timer(Duration::from_millis(1200))
                                .await;
                            this.update(cx, |this, cx| {
                                this.copied_code = None;
                                this.copied_clear = None;
                                cx.notify();
                            })
                            .ok();
                        }));
                        cx.notify();
                    })
                    .ok();
            });
        render::CopyUi { handler, copied_ix }
    }

    /// Interactive layout/scroll plumbing for one agent Markdown fence. The
    /// persisted Fit choice is global, while each block retains only its own
    /// ephemeral horizontal offset and hover/drag state.
    fn code_ui_for(
        &mut self,
        row_id: &SharedString,
        block_ix: usize,
        cx: &mut Context<Self>,
    ) -> render::CodeUi {
        let key: SharedString = format!("{row_id}#code{block_ix}").into();
        let runtime = self.code_fences.entry(key.clone()).or_default();
        render::code_ui_for(
            key,
            crate::prefs::current(cx).code_fences_fit_content,
            runtime,
            cx.weak_entity(),
            |transcript| &mut transcript.code_fences,
        )
    }
    fn code_uis_for(
        &mut self,
        row_id: &SharedString,
        block: &Block,
        block_ix: usize,
        cx: &mut Context<Self>,
    ) -> Option<HashMap<usize, render::CodeUi>> {
        let indices = render::code_block_indices(block, block_ix);
        (!indices.is_empty()).then(|| {
            indices
                .into_iter()
                .map(|ix| (ix, self.code_ui_for(row_id, ix, cx)))
                .collect()
        })
    }

    /// Request highlights for the code blocks of a tree. `only` limits to one
    /// block index (split rows); `None` covers the whole tree (live rows).
    fn code_highlight_for(
        &mut self,
        row_id: &SharedString,
        tree: &Arc<BlockTree>,
        only: Option<usize>,
        cx: &mut Context<Self>,
    ) -> HashMap<usize, Option<Arc<openagents_syntax::HighlightedDocument>>> {
        let mut out = HashMap::new();
        for (ix, top) in tree.blocks.iter().enumerate() {
            if only.is_some_and(|o| o != ix) {
                continue;
            }
            if let Block::CodeBlock { language, code } = &top.block
                && let Some(lang) = language
                    .as_deref()
                    .and_then(openagents_syntax::language_for_alias)
            {
                out.insert(
                    ix,
                    self.highlights.request(row_id.clone(), ix, lang, code, cx),
                );
            }
        }
        out
    }

    fn tool_diff_highlight_for(
        &mut self,
        row_id: &SharedString,
        tool_ix: usize,
        detail: &ToolDetail,
        cx: &mut Context<Self>,
    ) -> Option<Arc<crate::diff::DiffHighlights>> {
        let ToolDetail::Diff {
            file,
            old_text,
            new_text,
        } = detail
        else {
            return None;
        };
        let cache_row: SharedString = format!("{row_id}#tool-diff-{tool_ix}").into();
        let old = match old_text {
            Some(source) => {
                let path = file.old_path.as_deref().unwrap_or(&file.path);
                let lang = openagents_syntax::language_for_path(path)?;
                Some(
                    self.highlights
                        .request(cache_row.clone(), 0, lang, source, cx)?,
                )
            }
            None => None,
        };
        let new = match new_text {
            Some(source) => {
                let lang = openagents_syntax::language_for_path(&file.path)?;
                Some(self.highlights.request(cache_row, 1, lang, source, cx)?)
            }
            None => None,
        };
        Some(Arc::new(crate::diff::DiffHighlights { old, new }))
    }

    pub fn render_tool_group(
        &mut self,
        row_id: &SharedString,
        tools: &Arc<Vec<ToolItem>>,
        summary: &SharedString,
        auto_open: bool,
        worked_secs: Option<i64>,
        compact_shell: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut fold = self.folds.get(row_id).copied().unwrap_or_default();
        let file_opener = self.tool_file_opener(cx);
        // Agent/spawn chips never fold: they are their own row, always open,
        // no "Called N tools" header — a running subagent stays visible.
        // Compact mode is the exception: EVERYTHING sits under the one work
        // accordion, spawn chips included.
        let collapses = self.compact_mode || tool_group_collapses(tools);
        let arrival_pending = !cx.reduce_motion()
            && self.tool_group_reveals.get(row_id).is_some_and(|reveal| {
                reveal.starts.iter().flatten().any(|start| {
                    Instant::now()
                        .checked_duration_since(*start)
                        .unwrap_or_default()
                        < TOOL_CONNECTOR_REVEAL.total()
                })
            });
        // Compact mode never auto-opens — not for the streaming tail, and not
        // to show chips arriving mid-reveal. Collapsed-by-default is the mode.
        let effective_auto_open = auto_open || (arrival_pending && !self.compact_mode);
        let open = !collapses || fold.open.unwrap_or(effective_auto_open);
        // Compact shells only change `open` through `toggle_fold`, which
        // already seeds `from` with the body's measured height — the
        // rendered-height reset below must not clobber it with the shell's
        // own (unused) body tracking.
        if collapses && !compact_shell {
            let reveal = self.tool_group_reveals.entry(row_id.clone()).or_default();
            if reveal
                .rendered_open
                .is_some_and(|previous| previous != open)
            {
                fold.from = reveal.rendered_height;
                fold.toggled_at = Some(Instant::now());
                fold.disclosure_at = fold.toggled_at;
                self.folds.insert(row_id.clone(), fold);
            }
            reveal.rendered_open = Some(open);
        }
        // The title shimmer reads "working" even under the collapsed compact
        // fold — any unresolved chip (a running tool, a live thought) keeps it
        // pulsing until the turn's work settles. Compute this before the
        // collapsed-body skip, which may empty `tools`.
        let active =
            collapses && (auto_open || (self.compact_mode && tools.iter().any(|t| !t.resolved)));
        if self.compact_mode && active {
            self.compact_live_entries.insert(row_id.clone());
        }
        let worked_secs = worked_secs.filter(|_| self.compact_mode);
        if worked_secs.is_some() && self.compact_live_entries.remove(row_id) {
            self.compact_worked_fade_at
                .insert(row_id.clone(), Instant::now());
        }
        let animate_worked = self
            .compact_worked_fade_at
            .get(row_id)
            .is_some_and(|at| at.elapsed() < motion::FADE_IN.total());
        let worked_fade_t = match worked_secs {
            Some(_) if animate_worked && !cx.reduce_motion() => self
                .compact_worked_fade_at
                .get(row_id)
                .map(|at| {
                    motion::FADE_IN.progress(
                        at.elapsed().as_secs_f32() / motion::FADE_IN.total().as_secs_f32(),
                    )
                })
                .unwrap_or(1.0),
            Some(_) => 1.0,
            None => 0.0,
        };

        // A settled collapsed group has no visible body. Do not construct or
        // format thousands of hidden chips merely to clip them to zero height.
        // Keep the body during closing so the existing fold animation survives.
        let body_visible = open
            || (!cx.reduce_motion()
                && fold
                    .toggled_at
                    .is_some_and(|at| at.elapsed() < TOOL_FOLD.total()));
        let tools = if body_visible {
            tools.as_slice()
        } else {
            M::release_tool_images(self, row_id, cx);
            &[]
        };
        // Chips render their EFFECTIVE detail: the precomputed doc-resident
        // one, upgraded in place by a fetched sidecar blob (chat2-sync A3).
        // Resolved per paint (a HashMap probe per chip) so fetched content
        // needs no row rebuild — arrival is a cx.notify, like a fold toggle.
        let details: Vec<Option<Arc<ToolDetail>>> = tools
            .iter()
            .map(|tool| {
                // Spawn chips never expand — the subagent doc is the record
                // of what the tool did, and an inline body would only repeat
                // it. The whole chip is the "open that doc" click instead.
                if is_spawn_link(tool) {
                    return None;
                }
                // Among fetched blobs, the most recently REQUESTED one wins —
                // a tool can carry both a diff and an output ref, and the
                // user's last click decides which upgrade is showing.
                let mut best: Option<(u64, Arc<ToolDetail>)> = None;
                for blob_ref in [&tool.diff_ref, &tool.output_ref].into_iter().flatten() {
                    if let Some(BlobFetch::Ready(detail)) = self.blob_details.get(blob_ref) {
                        let order = self.blob_fetch_order.get(blob_ref).copied().unwrap_or(0);
                        if best.as_ref().is_none_or(|(o, _)| order > *o) {
                            best = Some((order, detail.clone()));
                        }
                    }
                }
                best.map(|(_, d)| d).or_else(|| tool.detail.clone())
            })
            .collect();
        // Full-invocation blocks — with them, EVERY chip expands: the click
        // always answers "what exactly was this call?", output or not.
        let invocations: Vec<Option<Arc<ToolDetail>>> = tools
            .iter()
            .map(|tool| tool.invocation.clone().filter(|_| !is_spawn_link(tool)))
            .collect();
        // Fetch affordance under each open detail whose full payload is still
        // sidecar-only: `(ref, label)`. Diff offered first (the richer
        // upgrade), then the output — a fetched ref hands the affordance to
        // the NEXT unfetched one instead of retiring it (both must stay
        // reachable when a tool has both).
        let affordances: Vec<Option<ChipAffordance>> = tools
            .iter()
            .map(|tool| {
                // The currently-displayed ref (same recency rule as
                // `details` above): its affordance is spent; any OTHER
                // Ready ref stays offered as a no-fetch toggle.
                let shown: Option<&SharedString> = {
                    let mut best: Option<(u64, &SharedString)> = None;
                    for blob_ref in [&tool.diff_ref, &tool.output_ref].into_iter().flatten() {
                        if matches!(self.blob_details.get(blob_ref), Some(BlobFetch::Ready(_))) {
                            let order = self.blob_fetch_order.get(blob_ref).copied().unwrap_or(0);
                            if best.is_none_or(|(o, _)| order > o) {
                                best = Some((order, blob_ref));
                            }
                        }
                    }
                    best.map(|(_, r)| r)
                };
                let candidates = [
                    (tool.diff_ref.as_ref(), "diff", None),
                    (tool.output_ref.as_ref(), "output", tool.output_bytes),
                ];
                for (blob_ref, what, bytes) in candidates {
                    let Some(blob_ref) = blob_ref else { continue };
                    let label = match self.blob_details.get(blob_ref) {
                        Some(BlobFetch::Ready(_)) => {
                            if shown == Some(blob_ref) {
                                continue;
                            }
                            format!("Show full {what}")
                        }
                        Some(BlobFetch::Loading(_)) => format!("Loading full {what}…"),
                        Some(BlobFetch::Failed) => {
                            format!("Couldn't load full {what} — tap to retry")
                        }
                        None => match bytes {
                            Some(b) => format!("Show full {what} ({})", format_kb(b)),
                            None => format!("Show full {what}"),
                        },
                    };
                    return Some(ChipAffordance {
                        blob_ref: blob_ref.clone(),
                        label: SharedString::from(label),
                    });
                }
                None
            })
            .collect();
        // Which chips have their detail block open (render-local, analytic —
        // the FINAL state; a mid-tween detail already counts as its target).
        let detail_folds: Vec<FoldState> = details
            .iter()
            .zip(&invocations)
            .enumerate()
            .map(|(ix, (detail, invocation))| {
                if detail.is_none() && invocation.is_none() && !chip_has_images(&tools[ix]) {
                    return FoldState::default();
                }
                self.tool_details
                    .get(&SharedString::from(format!("{row_id}#d{ix}")))
                    .copied()
                    .unwrap_or_default()
            })
            .collect();
        let detail_opens: Vec<bool> = details
            .iter()
            .zip(&invocations)
            .zip(&detail_folds)
            .zip(tools.iter())
            .map(|(((detail, invocation), fold), tool)| {
                // A STREAMING thought chip defaults open (the live thinking
                // is the point); settled chips default closed. A user toggle
                // overrides either way — and compact mode overrides all of
                // it: nothing inside the fold opens itself.
                let default_open =
                    tool.kind == ToolItemKind::Thought && !tool.resolved && !self.compact_mode;
                (detail.is_some() || invocation.is_some() || chip_has_images(tool))
                    && fold.open.unwrap_or(default_open)
            })
            .collect();
        // Image previews exist only while their chip's body is mounted (open,
        // or tweening shut). Every other chip releases its pixels.
        let chip_images =
            M::hold_tool_images(self, row_id, tools, &detail_opens, &detail_folds, cx);
        let detail_highlights: Vec<Option<Arc<crate::diff::DiffHighlights>>> = details
            .iter()
            .enumerate()
            .map(|(ix, detail)| {
                detail
                    .as_deref()
                    .filter(|_| detail_opens[ix])
                    .and_then(|detail| self.tool_diff_highlight_for(row_id, ix, detail, cx))
            })
            .collect();
        let base_row_height = if collapses {
            TOOL_TREE_ROW_HEIGHT
        } else {
            CHIP_HEIGHT
        };
        let mut motion_active = false;
        let row_heights: Vec<f32> = details
            .iter()
            .zip(&invocations)
            .zip(&affordances)
            .zip(&detail_opens)
            .zip(&detail_folds)
            .zip(tools.iter())
            .map(
                |(((((detail, invocation), affordance), open), fold), tool)| {
                    let target = if *open {
                        base_row_height
                            + invocation.as_deref().map_or(0.0, detail_height)
                            + if chip_has_images(tool) {
                                TOOL_IMAGE_STRIP_HEIGHT
                            } else {
                                0.0
                            }
                            + detail.as_deref().map_or(0.0, detail_height)
                            + if affordance.is_some() {
                                BLOB_AFFORDANCE_HEIGHT
                            } else {
                                0.0
                            }
                    } else {
                        base_row_height
                    };
                    if !cx.reduce_motion() {
                        if let Some(at) = fold.toggled_at {
                            let t = TOOL_FOLD
                                .curve
                                .eval(at.elapsed().as_secs_f32() / TOOL_FOLD.total().as_secs_f32());
                            if t < 1.0 {
                                motion_active = true;
                            }
                            return motion::lerp(
                                fold.from + base_row_height - CHIP_CARD_HEIGHT,
                                target,
                                t,
                            );
                        }
                    }
                    target
                },
            )
            .collect();
        let reduce_motion = cx.reduce_motion();
        let now = Instant::now();
        let reveal_progress: Vec<f32> = (0..tools.len())
            .map(|ix| {
                let start = self
                    .tool_group_reveals
                    .get(row_id)
                    .and_then(|reveal| reveal.starts.get(ix))
                    .copied()
                    .flatten();
                tool_row_reveal_progress(start, now, reduce_motion)
            })
            .collect();
        let connector_progress: Vec<f32> = (0..tools.len())
            .map(|ix| {
                let start = self
                    .tool_group_reveals
                    .get(row_id)
                    .and_then(|reveal| reveal.starts.get(ix))
                    .copied()
                    .flatten();
                tool_connector_reveal_progress(start, now, reduce_motion)
            })
            .collect();
        let header_reveal = tool_row_reveal_progress(
            self.tool_group_reveals
                .get(row_id)
                .and_then(|reveal| reveal.header_started_at),
            now,
            reduce_motion,
        );
        if header_reveal < 1.0
            || reveal_progress.iter().any(|progress| *progress < 1.0)
            || connector_progress.iter().any(|progress| *progress < 1.0)
        {
            motion_active = true;
        }
        let revealed_height = CHIPS_TOP_PAD
            + row_heights
                .iter()
                .zip(&reveal_progress)
                .map(|(height, progress)| height * progress)
                .sum::<f32>();
        let viewport_height = revealed_height;
        let target = if open { viewport_height } else { 0.0 };
        let shimmer_phase = if active && !reduce_motion {
            motion::pulse_lease(cx.entity_id(), cx);
            self.tool_group_reveals
                .get(row_id)
                .and_then(|reveal| reveal.shimmer_started_at)
                .map(|start| tool_title_shimmer_phase(start, now))
        } else {
            None
        };
        let disclosure_progress = if reduce_motion {
            if open { 1.0 } else { 0.0 }
        } else {
            tool_disclosure_progress(open, fold, now)
        };

        // The summary rolls as the group grows ("Thought 2 times" → "Thought
        // 3 times · Ran 1 command"), like the working indicator's word.
        let rolled_summary = crate::roll_text::rolling(
            format!("tool-group-title-{row_id}"),
            summary.clone(),
            reduce_motion,
        );
        let toggle_id = row_id.clone();
        // A quiet summary sits above the activity rail; its chevron occupies
        // the same gutter as the rounded task-tree elbows below it.
        let header = div()
            .id(SharedString::from(format!("{row_id}-hdr")))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .pr(px(4.0))
            .h(px(TOOL_GROUP_HEADER_HEIGHT))
            .cursor_pointer()
            .text_size(px(TOOL_LABEL_SIZE))
            .line_height(px(TOOL_LABEL_LINE_HEIGHT))
            // Quiet even when children failed: agents routinely have failed
            // probes mid-work, and a red HEADER read as "this whole step
            // broke" (user report). Failures still show on the individual
            // chips (destructive tint, openagents tool-chip.tsx) and in the
            // summary's "· N failed" count.
            .text_color(theme.text_muted)
            .hover(|s| s.text_color(theme.text))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_fold(toggle_id.clone(), viewport_height, effective_auto_open, cx);
                cx.notify();
            }))
            .child(
                div()
                    // Keep the title adjacent to its disclosure affordance.
                    .w(px(22.0))
                    .h(px(18.0))
                    .flex_none()
                    .relative()
                    .child(
                        crate::icons::icon(crate::icons::ALT_ARROW_DOWN)
                            .absolute()
                            .left(px(ACTIVITY_TRUNK_X - 7.0))
                            .top(px(2.0))
                            .size(px(14.0))
                            .with_transformation(gpui::Transformation::rotate(gpui::radians(
                                -std::f32::consts::FRAC_PI_2 * (1.0 - disclosure_progress),
                            )))
                            .text_color(theme.text_muted),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .h(px(TOOL_LABEL_LINE_HEIGHT))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .child(match worked_secs {
                        Some(secs) => compact_work_title(
                            summary.clone(),
                            SharedString::from(worked_for_label(secs)),
                            worked_fade_t,
                            shimmer_phase,
                            theme,
                        ),
                        None => rolled_summary.unwrap_or_else(|| {
                            tool_group_title(summary.clone(), shimmer_phase, theme)
                        }),
                    }),
            );

        if compact_shell {
            let view = cx.entity_id();
            // The sibling body rows tween under TOOL_FOLD — keep frames (and
            // the chevron's disclosure rotation) pumping until it lands.
            if !reduce_motion
                && fold
                    .toggled_at
                    .is_some_and(|at| at.elapsed() < TOOL_FOLD.total())
            {
                motion_active = true;
            }
            return div()
                .relative()
                .flex()
                .flex_col()
                .font_family(theme.font_sans_fixed.clone())
                .child(header)
                .when(motion_active || animate_worked, |group| {
                    group.child(
                        canvas(
                            |_, _, _| (),
                            move |_, _, window, _| {
                                window.on_next_frame(move |_, cx| cx.notify(view));
                            },
                        )
                        .absolute()
                        .inset_0(),
                    )
                })
                .into_any_element();
        }

        let chips = div()
            .pt(px(CHIPS_TOP_PAD))
            .flex()
            .flex_col()
            .gap(px(CHIP_GAP))
            .children(tools.iter().enumerate().map(|(ix, tool)| {
                let reveal = reveal_progress[ix];
                let connector_reveal = connector_progress[ix];
                let content_reveal = tool_connector_parts(connector_reveal, ix > 0).1;
                let continuation_reveal =
                    tool_connector_continuation(connector_progress.get(ix + 1).copied());
                let row_height = row_heights[ix];
                // Spawn chips are LINKS, not accordions: the click opens the
                // subagent's transcript as a right-pane tab (the shell hosts
                // the surface — the chip only announces which doc it indexes).
                if let Some(doc_id) = tool.subagent_ref.clone().filter(|_| is_spawn_link(tool)) {
                    let chat_id = self.chat_id.clone().unwrap_or_default();
                    let title = subagent_tab_title(&tool.call);
                    let frozen = matches!(
                        tool.subagent_status,
                        Some(SubagentStatus::Done) | Some(SubagentStatus::Failed)
                    );
                    return subagent_chip(
                        tool,
                        SharedString::from(format!("{row_id}#s{ix}")),
                        cx.listener(move |_, _, _, cx| {
                            cx.emit(TranscriptEvent::OpenSubagent {
                                chat_id: chat_id.clone(),
                                doc_id: doc_id.to_string(),
                                title: title.to_string(),
                                frozen,
                            });
                        }),
                        collapses,
                        theme,
                        cx.entity_id(),
                        cx,
                    );
                }
                let detail = details[ix].clone();
                let invocation = invocations[ix].clone();
                if detail.is_none() && invocation.is_none() && !chip_has_images(tool) {
                    return reveal_tool_row(
                        tool_chip(
                            tool,
                            collapses,
                            ix > 0,
                            ix + 1 < tools.len(),
                            content_reveal,
                            connector_reveal,
                            continuation_reveal,
                            file_opener.as_ref(),
                            theme,
                            cx.entity_id(),
                            cx,
                        ),
                        row_height,
                        reveal,
                    );
                }
                let affordance = affordances[ix].clone();
                let open = detail_opens[ix];
                let dfold = detail_folds[ix];
                let key = SharedString::from(format!("{row_id}#d{ix}"));
                // Ordinary tools expand into muted text along the same column.
                // Subagent fallbacks retain their card; explicit heights keep
                // the row and group fold animations in sync.
                let animating = dfold.epoch > 0
                    && dfold
                        .toggled_at
                        .is_some_and(|at| at.elapsed() < FOLD_TWEEN_WINDOW);
                let toggle_key = key.clone();
                let mut card = div()
                    .my(px((base_row_height - CHIP_CARD_HEIGHT) / 2.0))
                    .when(collapses, |el| el.ml(px(ACTIVITY_TEXT_GAP)))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .when(!collapses, |card| {
                        card.rounded(px(9.0))
                            .border_1()
                            .border_color(crate::theme::hairline(0.07))
                            .bg(crate::theme::ink(0.03))
                    })
                    .child(
                        div()
                            .id(key.clone())
                            .h(px(if collapses {
                                CHIP_CARD_HEIGHT
                            } else {
                                CHIP_HEADER_HEIGHT
                            }))
                            .flex_none()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                let entry =
                                    this.tool_details.entry(toggle_key.clone()).or_default();
                                let currently_open = entry.open.unwrap_or(open);
                                entry.from = row_height - base_row_height + CHIP_CARD_HEIGHT;
                                entry.open = Some(!currently_open);
                                entry.epoch += 1;
                                entry.toggled_at = Some(Instant::now());
                                cx.notify();
                            }))
                            .child(chip_header(
                                tool,
                                open,
                                file_opener.as_ref(),
                                theme,
                                cx.entity_id(),
                                cx,
                            )),
                    );
                // The body stays mounted while the close tween shrinks over it.
                // Invocation first (what was asked), then output/diff (what
                // came back), separated by a small gap.
                if open || animating {
                    let mut panel = div()
                        .flex_none()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .overflow_hidden();
                    if let Some(invocation) = invocation.as_deref() {
                        panel = panel
                            .child(
                                div()
                                    .h(px(DETAIL_SEPARATOR))
                                    .flex_none()
                                    .when(!collapses, |line| line.bg(crate::theme::hairline(0.06))),
                            )
                            .child(detail_body::<M>(
                                invocation,
                                None,
                                &format!("{key}-call"),
                                theme,
                            ));
                    }
                    if let Some(images) = chip_images[ix].clone() {
                        panel = panel.child(M::tool_image_strip(
                            self, &key, images, collapses, theme, cx,
                        ));
                    }
                    if let Some(detail) = detail.as_deref() {
                        panel = panel
                            .child(
                                div()
                                    .h(px(DETAIL_SEPARATOR))
                                    .flex_none()
                                    .when(!collapses, |line| line.bg(crate::theme::hairline(0.06))),
                            )
                            .child(detail_body::<M>(
                                detail,
                                detail_highlights[ix].clone(),
                                &format!("{key}-out"),
                                theme,
                            ));
                    }
                    if let Some(ChipAffordance { blob_ref, label }) = affordance {
                        let loading = matches!(
                            self.blob_details.get(&blob_ref),
                            Some(BlobFetch::Loading(_))
                        );
                        let mut row = div()
                            .id(SharedString::from(format!("{key}-blob")))
                            .h(px(BLOB_AFFORDANCE_HEIGHT))
                            .flex_none()
                            .flex()
                            .items_center()
                            .text_size(px(TOOL_TEXT_SIZE))
                            .text_color(theme.text_faint)
                            .child(label);
                        if !loading {
                            row = row
                                .cursor_pointer()
                                .hover(|s| s.text_color(theme.text_muted))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.spawn_blob_fetch(blob_ref.clone(), cx);
                                    cx.notify();
                                }));
                        }
                        panel = panel.child(row);
                    }
                    card = card.child(panel);
                }
                let card = card.h(px(row_height - base_row_height + CHIP_CARD_HEIGHT));
                let card = div().min_w_0().flex_1().child(card);
                let row = div()
                    .w_full()
                    .flex_none()
                    .flex()
                    .flex_row()
                    // Stretch the line alongside the expanded text.
                    .when(collapses, |row| {
                        row.child(activity_rail(
                            tool,
                            ix > 0,
                            ix + 1 < tools.len(),
                            connector_reveal,
                            continuation_reveal,
                            base_row_height,
                            theme,
                        ))
                    })
                    .child(card.when(collapses && content_reveal < 1.0, |card| {
                        card.relative()
                            .top(px(4.0 * (1.0 - content_reveal)))
                            .opacity(content_reveal)
                    }))
                    .into_any_element();
                reveal_tool_row(row, row_height, reveal)
            }));

        let chips = chips.into_any_element();

        // Evaluate the group on the same clock as its disclosure. This also
        // gives completion a gentle close after the last arrival finishes,
        // without restarting an element animation when the list remounts it.
        let body_height = if !reduce_motion {
            fold.toggled_at
                .map(|at| {
                    let t = TOOL_FOLD.curve.eval(
                        now.saturating_duration_since(at).as_secs_f32()
                            / TOOL_FOLD.total().as_secs_f32(),
                    );
                    if t < 1.0 {
                        motion_active = true;
                    }
                    motion::lerp(fold.from, target, t)
                })
                .unwrap_or(target)
        } else {
            target
        };
        if let Some(reveal) = self.tool_group_reveals.get_mut(row_id) {
            reveal.rendered_height = body_height;
        }
        let body: AnyElement = if !collapses {
            chips.into_any_element()
        } else {
            div()
                .overflow_hidden()
                .h(px(body_height))
                .child(chips)
                .into_any_element()
        };

        let view = cx.entity_id();
        div()
            .relative()
            .flex()
            .flex_col()
            // Tool summaries and cards are code-adjacent chrome. Detail bodies
            // retain their explicit mono/diff typography below this boundary.
            .font_family(theme.font_sans_fixed.clone())
            .when(collapses, |el| {
                el.child(reveal_tool_row(
                    header.into_any_element(),
                    TOOL_GROUP_HEADER_HEIGHT,
                    header_reveal,
                ))
            })
            .child(body)
            .when(motion_active || animate_worked, |group| {
                group.child(
                    canvas(
                        |_, _, _| (),
                        move |_, _, window, _| {
                            window.on_next_frame(move |_, cx| cx.notify(view));
                        },
                    )
                    .absolute()
                    .inset_0(),
                )
            })
            .into_any_element()
    }
}

/// A sent message's text with its file-mention chips. The same recipe as the
/// markdown renderer's inline code (`flat_text_element`): chip ranges shape in
/// the mono font at the spectrum's `code_text`, [`StyledText`] supplies wrapped glyph
/// geometry through its layout handle, and a canvas paints the rounded
/// `code_wash` *beneath* the glyphs — so chips wrap, clip, and scroll exactly
/// like the text they decorate.
///
/// Per-frame cost while an assistant message streams below: shaping hits
/// gpui's line-layout cache (identical text + runs ⇒ reuse) and the underlay
/// repaints O(chips) quads — no layout work, no re-projection (spans were
/// computed once in [`rows_for_entry`]).
/// The user bubble's text: runs split at mention-chip boundaries (one plain
/// run when there are none), with the same selection machinery as rendered
/// markdown — the element registers into the frame's document-ordered
/// registry, so drags select, span into adjacent rows, and Cmd+C copies.
/// Keep routing instructions in the stored prompt for agents, but show a
/// concise attribution in the human transcript (including existing messages).
pub fn agent_message_display(text: &str) -> String {
    let Some(rest) = text
        .strip_prefix("[Message from OpenAgents chat ")
        .or_else(|| text.strip_prefix("[Message from Zeron chat "))
    else {
        return text.to_owned();
    };
    let Some((header, body)) = rest.split_once("]\n\n") else {
        return text.to_owned();
    };
    let Some((label, id)) = header
        .rsplit_once(". Reply to it with the OpenAgents `send_message` tool, chat ")
        .or_else(|| header.rsplit_once(". Reply to it with the Zeron `send_message` tool, chat "))
        .or_else(|| {
            header.rsplit_once(
                ". Reply only if it asks you something or you need something from it, with the OpenAgents `send_message` tool, chat ",
            )
        })
    else {
        return text.to_owned();
    };
    let Some(id) = id.strip_suffix('.') else {
        return text.to_owned();
    };
    let suffix = format!(" ({id})");
    let name = label.strip_suffix(&suffix).unwrap_or(label);
    format!("Message from {name}\n\n{body}")
}

#[cfg(test)]
#[test]
fn previous_agent_messages_keep_their_display() {
    for brand in ["OpenAgents", "Zeron"] {
        let prompt = format!(
            "[Message from {brand} chat Helper (chat-1). Reply to it with the {brand} `send_message` tool, chat chat-1.]\n\nDone."
        );
        assert_eq!(
            agent_message_display(&prompt),
            "Message from Helper\n\nDone."
        );
    }
    let prompt = "[Message from OpenAgents chat Helper (chat-1). Reply only if it asks you something or you need something from it, with the OpenAgents `send_message` tool, chat chat-1.]\n\nDone.";
    assert_eq!(
        agent_message_display(prompt),
        "Message from Helper\n\nDone."
    );
}

/// The bubble text's runs: body text in the sans font, chips as labels in it
/// with their padding pinned to [`crate::composer_chips::CHIP_PAD_FAMILY`], and an
/// agent attribution's name in bold.
pub fn user_bubble_runs(
    text: &str,
    mentions: &[crate::composer_mentions::SentMentionSpan],
    theme: &Theme,
) -> Vec<TextRun> {
    // Split runs at chip boundaries (spans are in order). Size and line
    // height flow from the bubble's div like every text child.
    let body_run = |len: usize| TextRun {
        len,
        font: gpui::font(theme.font_sans.clone()),
        color: theme.text,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut runs = Vec::with_capacity(mentions.len() * 2 + 1);
    let mut at = 0;
    for span in mentions.iter() {
        if at < span.range.start {
            runs.push(body_run(span.range.start - at));
        }
        // Chips are labels in the body font, like the composer's, with their
        // padding pinned to the face its insets are tuned for.
        let [lead, trail] = crate::composer_chips::chip_pad_ranges(&span.range);
        for (len, pad) in [
            (lead.len(), true),
            (trail.start - lead.end, false),
            (trail.len(), true),
        ] {
            if len > 0 {
                let mut run = body_run(len);
                if pad {
                    run.font.family = crate::composer_chips::CHIP_PAD_FAMILY.into();
                }
                runs.push(run);
            }
        }
        at = span.range.end;
    }
    if at < text.len() {
        runs.push(body_run(text.len() - at));
    }
    // Attribution names are bold sans text, never Markdown/italic. Split
    // existing runs so file-mention styling and selection offsets stay intact.
    if let Some(rest) = text.strip_prefix("Message from ")
        && let Some((name, _)) = rest.split_once("\n\n")
    {
        let bold = "Message from ".len().."Message from ".len() + name.len();
        let mut offset = 0;
        runs = runs
            .into_iter()
            .flat_map(|run| {
                let end = offset + run.len;
                let mut pieces = Vec::new();
                while offset < end {
                    let in_name = bold.contains(&offset);
                    let next = if offset < bold.start {
                        end.min(bold.start)
                    } else if in_name {
                        end.min(bold.end)
                    } else {
                        end
                    };
                    let mut piece = run.clone();
                    piece.len = next - offset;
                    if in_name {
                        piece.font.weight = gpui::FontWeight::BOLD;
                    }
                    pieces.push(piece);
                    offset = next;
                }
                pieces
            })
            .collect();
    }
    runs
}

pub fn user_bubble_text(
    row_id: &SharedString,
    text: SharedString,
    mentions: Arc<Vec<crate::composer_mentions::SentMentionSpan>>,
    theme: &Theme,
    measured_h: Rc<Cell<f32>>,
    entity_id: gpui::EntityId,
    overlays: Vec<ChipOverlay>,
    open_image: Rc<dyn Fn(&SharedString, &mut Window, &mut gpui::App)>,
) -> AnyElement {
    let runs = user_bubble_runs(&text, &mentions, theme);
    let styled = StyledText::new(text.clone()).with_runs(runs);
    let layout = styled.layout().clone();
    let chip_layout = layout.clone();
    // A chip showing a status paints it over its icon well instead.
    let chip_icons: Vec<Option<crate::composer_chips::ChipIcon>> = mentions
        .iter()
        .map(|span| {
            (!overlays
                .iter()
                .any(|overlay| overlay.status.is_some() && overlay.range == span.range))
            .then(|| crate::composer_chips::chip_icon(span.kind, &span.path, theme.appearance))
        })
        .collect();
    let sel_key: std::sync::Arc<str> = format!("{row_id}:u").into();
    let sel_theme = theme.clone();
    let underlay = canvas(
        |bounds, window, _| window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal),
        move |_, hitbox, window, cx| {
            for (span, icon) in mentions.iter().zip(&chip_icons) {
                // Centered on the label like the composer's chips.
                let label = crate::composer_chips::chip_pad_ranges(&span.range)[0].end;
                for (row, rect) in render::range_rects(&layout, &span.range, 0.0, 2.0)
                    .into_iter()
                    .enumerate()
                {
                    let row_top = rect.origin.y - px(2.0);
                    let offset = crate::composer_chips::text_chip_label_offset(
                        &layout,
                        label,
                        row_top,
                        px(2.0),
                        window,
                    );
                    let rect = Bounds::new(rect.origin + point(px(0.0), offset), rect.size);
                    let icon = icon.as_ref().filter(|_| row == 0);
                    crate::composer_chips::paint_chip(window, rect, icon, &sel_theme, cx);
                }
            }
            render::paint_text_selection(window, hitbox, &sel_key, &text, &layout, &sel_theme);
            // Passive geometry cache only: no entity update and no notify.
            // `bounds().height` can be the collapsed clip height, so derive
            // the full text height from the wrapped line layouts instead. The
            // click handler reads this exact value as the RESIZE endpoint,
            // while idle layout never feeds back into the transcript.
            let line_count: usize = layout
                .line_layouts()
                .iter()
                .map(|line| line.wrap_boundaries.len() + 1)
                .sum();
            let next_h = (line_count.max(1) as f32) * f32::from(layout.line_height());
            if (measured_h.get() - next_h).abs() > 0.5 {
                measured_h.set(next_h);
                // The first layout is the source of truth for soft wrapping.
                // Invalidate the transcript once so the expander and clip are
                // present even when glyph widths make a short-looking string
                // exceed five visual lines.
                cx.notify(entity_id);
            }
        },
    )
    .absolute()
    .size_full();
    // Same wrapper as the assistant markdown: user-bubble text is
    // selectable (paint_text_selection above), so it gets the I-beam too.
    let text = render::selectable_text_wrap()
        .child(underlay)
        .child(styled)
        .into_any_element();
    if overlays.is_empty() {
        return text;
    }
    ChipOverlays {
        id: format!("{row_id}-chip-overlays").into(),
        child: text,
        layout: chip_layout,
        overlays,
        open: open_image,
        well: crate::file_icons::well_bg(theme),
    }
    .into_any_element()
}

/// The upload status painted in a sent chip's icon well.
pub const CHIP_STATUS_SIZE: f32 = 14.0;

/// One sent chip's extras, laid over its pill: a status in place of its icon
/// and, for a loaded image, a click target opening it full size.
pub struct ChipOverlay {
    pub range: Range<usize>,
    pub open: Option<SharedString>,
    pub status: Option<AnyElement>,
}

/// Wraps a sent bubble's text with its chips' overlays. They follow the
/// shaped text, so a chip that wraps gets a click target per row; its status
/// sits on the first row, where the icon would be.
pub struct ChipOverlays {
    pub id: SharedString,
    pub child: AnyElement,
    pub layout: gpui::TextLayout,
    pub overlays: Vec<ChipOverlay>,
    pub open: Rc<dyn Fn(&SharedString, &mut Window, &mut gpui::App)>,
    pub well: gpui::Hsla,
}

impl IntoElement for ChipOverlays {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for ChipOverlays {
    type RequestLayoutState = ();
    type PrepaintState = Vec<AnyElement>;

    fn id(&self) -> Option<gpui::ElementId> {
        Some(self.id.clone().into())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> (gpui::LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> Vec<AnyElement> {
        self.child.prepaint(window, cx);
        let mut targets = Vec::new();
        for (index, overlay) in self.overlays.iter_mut().enumerate() {
            let rects = render::range_rects(&self.layout, &overlay.range, 0.0, 2.0);
            if let (Some(status), Some(first)) = (overlay.status.take(), rects.first()) {
                // Matches `paint_chip`'s icon well.
                let side = first.size.height - px(2.0);
                let mut well = div()
                    .size(side)
                    .rounded(px(4.0))
                    .bg(self.well)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(status)
                    .into_any_element();
                well.prepaint_as_root(
                    first.origin + point(px(1.0), px(1.0)),
                    size(side, side).map(gpui::AvailableSpace::Definite),
                    window,
                    cx,
                );
                targets.push(well);
            }
            let Some(path) = overlay.open.clone() else {
                continue;
            };
            for (part, rect) in rects.into_iter().enumerate() {
                let open = self.open.clone();
                let path = path.clone();
                let mut target = div()
                    .id(SharedString::from(format!("{}-{index}-{part}", self.id)))
                    .w(rect.size.width)
                    .h(rect.size.height)
                    .cursor_pointer()
                    .role(gpui::Role::Button)
                    .aria_label(format!(
                        "Preview {}",
                        crate::attachments::attachment_display_name(
                            path.rsplit(['/', '\\']).next().unwrap_or(&path),
                        )
                    ))
                    .on_click(move |event, window, cx| {
                        // A drag that selects text across the chip is not a click.
                        if event.click_count() == 1
                            && crate::markdown::selection::selected_text().is_none()
                        {
                            open(&path, window, cx);
                        }
                    })
                    .into_any_element();
                target.prepaint_as_root(
                    rect.origin,
                    rect.size.map(gpui::AvailableSpace::Definite),
                    window,
                    cx,
                );
                targets.push(target);
            }
        }
        targets
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        targets: &mut Vec<AnyElement>,
        window: &mut Window,
        cx: &mut gpui::App,
    ) {
        self.child.paint(window, cx);
        for target in targets {
            target.paint(window, cx);
        }
    }
}

/// A sent message's non-image attachment: the file's icon and name on the
/// same soft pill the transcript uses for file badges. Its bytes are never
/// read back to the transcript, so there is no thumbnail to wait for.
pub fn user_file_pill(name: &str, theme: &Theme) -> AnyElement {
    let name = crate::attachments::attachment_display_name(name);
    div()
        .h(px(28.0))
        .max_w(px(260.0))
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(6.0))
        .rounded(px(7.0))
        .bg(theme.ink(0.06))
        .pl(px(2.0))
        .pr(px(10.0))
        .text_size(px(13.0))
        .text_color(theme.text.opacity(0.85))
        .child(
            div()
                .size(px(24.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.0))
                .bg(crate::file_icons::well_bg(theme))
                .child(
                    crate::file_icons::icon(
                        crate::file_icons::FileIconIdentity::file(name),
                        theme.appearance,
                    )
                    .size(px(16.0)),
                ),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .child(SharedString::from(name.to_owned())),
        )
        .into_any_element()
}

/// The transcript ErrorChip — the shared [`notice_chip`] in its tile
/// treatment (a port of openagents chat-view.tsx `ErrorChip`, restacked for long
/// payloads: header row with the red-washed tile and the medium "Error"
/// label, then the human message below). Unlike the web port, the message
/// WRAPS instead of truncating: startup-crash errors carry the agent's exit
/// status and stderr, and a one-line ellipsis was exactly what made
/// zeronsh/comet#95 undiagnosable from the screenshot.
pub fn error_chip(message: SharedString, theme: &Theme) -> AnyElement {
    div()
        .py(px(4.0))
        .w_full()
        .child(
            notice_chip(theme, false, "Error", message, Tile)
                .overflow_hidden()
                .w_full(),
        )
        .into_any_element()
}

/// A quiet fork seam. The source gets its own constrained line so long
/// titles cannot widen a narrow side-chat pane. No message metadata lane.
pub fn fork_marker(
    source_chat_id: SharedString,
    source_title: SharedString,
    theme: &Theme,
) -> AnyElement {
    let rule = || div().flex_1().min_w_0().h(px(1.0)).bg(theme.border_strong);
    div()
        .py(px(14.0))
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(rule())
                .child(
                    div()
                        .flex_none()
                        .text_size(crate::typography::ui_rems(12.0))
                        .text_color(theme.text_muted.opacity(0.7))
                        .child("Forked from"),
                )
                .child(rule()),
        )
        .child(
            // Opens the chat this one was forked from (#11963).
            div()
                .id(SharedString::from(format!("fork-source-{source_chat_id}")))
                .debug_selector(|| "fork-source".into())
                .role(gpui::Role::Link)
                .aria_label(SharedString::from(format!("Open {source_title}")))
                .w_full()
                .min_w_0()
                .truncate()
                .text_center()
                .text_size(crate::typography::ui_rems(13.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text_muted)
                .cursor_pointer()
                .hover(|style| style.text_color(theme.text).underline())
                .on_click(move |_, window, cx| {
                    window.dispatch_action(Box::new(OpenForkSource(source_chat_id.to_string())), cx)
                })
                .child(source_title),
        )
        .into_any_element()
}

/// A passive one-line chip marking a question the agent asked — the
/// interactive controls live in the composer (chat-view.tsx `InputChip`):
/// 34px row, `rounded-[10px] border-white/[0.08] bg-white/[0.045] px-2
/// text-[12px]`, a 20px `bg-white/[0.09]` icon tile with a 12px
/// ChatRoundLine, the medium "Question" label, then the truncating value —
/// the first question's header once resolved, "Awaiting your answer…" while
/// pending. Neutral tones throughout; resolution never recolors the chip.
pub fn input_chip(header: SharedString, resolved: bool, theme: &Theme) -> AnyElement {
    let value: SharedString = if resolved {
        header
    } else {
        "Awaiting your answer…".into()
    };
    div()
        .py(px(4.0))
        .w_full()
        .child(
            div()
                .h(px(34.0))
                .w_full()
                .flex()
                .items_center()
                .gap(px(8.0))
                .overflow_hidden()
                .rounded(px(10.0))
                .border_1()
                .border_color(crate::theme::hairline(0.08))
                .bg(crate::theme::ink(0.045))
                .px(px(8.0))
                .text_size(px(12.0))
                .child(
                    div()
                        .flex_none()
                        .size(px(20.0))
                        .rounded(px(6.0))
                        .bg(crate::theme::ink(0.09))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            crate::icons::icon(crate::icons::CHAT_ROUND_LINE)
                                .size(px(12.0))
                                .text_color(theme.text_muted),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.text_muted)
                        .child(SharedString::from("Question")),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_color(theme.text.opacity(0.9))
                        .child(value),
                ),
        )
        .into_any_element()
}

/// A small glyph standing in for the tool's icon (openagents uses an icon set; a
/// quiet monochrome character keeps the tile without shipping SVGs).
/// The glyph for a tool call (openagents tool-chip.tsx `toolIcon`, Solar set).
pub fn tool_icon_path(call: &ToolCall) -> &'static str {
    match call {
        ToolCall::Exec { .. } => crate::icons::TERMINAL,
        ToolCall::ReadFile { .. } | ToolCall::ApplyPatch { .. } => crate::icons::DOCUMENT,
        ToolCall::WriteFile { .. } => crate::icons::DOCUMENT_ADD,
        ToolCall::EditFile { .. } => crate::icons::PEN,
        ToolCall::Search { .. } => crate::icons::MAGNIFER,
        ToolCall::Glob { .. } => crate::icons::FOLDER_WITH_FILES,
        ToolCall::WebFetch { .. } | ToolCall::WebSearch { .. } => crate::icons::GLOBAL,
        ToolCall::Todo { .. } => crate::icons::CHECKLIST,
        call if is_agent_call(call) => crate::icons::BOT,
        ToolCall::Unknown { name, .. } if name == "Wait for agents" => crate::icons::BOT,
        ToolCall::Mcp { .. } | ToolCall::Unknown { .. } => crate::icons::WIDGET,
    }
}

/// Compact file-action chips show only the final path component. The full
/// path remains on the tool call (and in expanded diagnostics) for identity
/// and disambiguation. Accept both separator styles because remote tools can
/// report Windows paths even when the UI is running elsewhere.
pub fn file_badge_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|component| !component.is_empty())
        .unwrap_or(path)
}

/// The body of an expanded chip card, under the header's separator. Diffs
/// render through the changes pane's section body — the real component, with
/// hunk headers, dual line-number gutters, accent bars, row washes, and
/// syntax runs — so an inline tool diff is indistinguishable from the
/// checkout diff sidebar. Output renders as a code block: verbatim mono
/// lines, indentation intact, counted-tail truncation.
pub fn detail_body<M: TranscriptMedia>(
    detail: &ToolDetail,
    diff_highlights: Option<Arc<crate::diff::DiffHighlights>>,
    selection_key: &str,
    theme: &Theme,
) -> AnyElement {
    let body = div().w_full().min_w_0().flex().flex_col().overflow_hidden();
    // Every line joins the transcript's selection, so paths, commands and
    // output can be drag-selected (across lines too) and copied. Lines clip
    // rather than ellipsize: selection maps the pointer onto the full text.
    let selectable = |line_ix: usize, text: SharedString, runs: Vec<TextRun>| {
        div()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .child(render::selectable_text_element(
                format!("{selection_key}-{line_ix}").into(),
                text,
                runs,
                theme.selection,
            ))
    };
    let plain = |text: &SharedString, color: gpui::Hsla| {
        vec![TextRun {
            len: text.len(),
            font: gpui::font(theme.font_mono.clone()),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        }]
    };
    match detail {
        // No comment layer: an inline tool diff is a record of what the
        // agent already did, not a review surface.
        ToolDetail::Diff { file, .. } => body
            .child(M::diff_body(file, diff_highlights, theme))
            .into_any_element(),
        ToolDetail::Stats { stats } => body
            .py(px(6.0))
            .font_family(theme.font_mono.clone())
            .text_size(px(TOOL_TEXT_SIZE))
            .children(stats.iter().enumerate().map(|(stat_ix, stat)| {
                let path = SharedString::from(stat.path.clone());
                let runs = plain(&path, theme.text_faint);
                div()
                    .h(px(OUTPUT_LINE_HEIGHT))
                    .w_full()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        crate::file_icons::icon(
                            crate::file_icons::FileIconIdentity::file(&stat.path),
                            theme.appearance,
                        )
                        .size(px(14.0))
                        .flex_none(),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(selectable(stat_ix, path, runs)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_color(theme.success)
                            .child(SharedString::from(format!("+{}", stat.additions))),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_color(theme.danger)
                            .child(SharedString::from(format!("−{}", stat.deletions))),
                    )
            }))
            .into_any_element(),
        ToolDetail::Output {
            lines,
            truncated_by,
        } => body
            .py(px(6.0))
            .font_family(theme.font_mono.clone())
            .text_size(px(TOOL_TEXT_SIZE))
            .children(lines.iter().enumerate().map(|(line_ix, line)| {
                let row = div()
                    .h(px(OUTPUT_LINE_HEIGHT))
                    .w_full()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .text_color(theme.text_faint);
                if line.is_empty() {
                    return row;
                }
                row.child(selectable(
                    line_ix,
                    line.clone(),
                    plain(line, theme.text_faint),
                ))
            }))
            .when(*truncated_by > 0, |block| {
                block.child(more_lines_row(*truncated_by, theme))
            })
            .into_any_element(),
        ToolDetail::Thought {
            lines,
            truncated_by,
        } => body
            .py(px(6.0))
            .text_size(px(TOOL_TEXT_SIZE))
            .children(lines.iter().enumerate().map(|(line_ix, line)| {
                let row = div()
                    .h(px(OUTPUT_LINE_HEIGHT))
                    .w_full()
                    .min_w_0()
                    .flex()
                    .items_center();
                let Some((text, runs)) = thought_line_text(line, theme) else {
                    return row; // blank separator row
                };
                row.child(selectable(line_ix, text, runs))
            }))
            .when(*truncated_by > 0, |block| {
                block.child(more_lines_row(*truncated_by, theme))
            })
            .into_any_element(),
    }
}

/// The counted-tail row under a truncated Output/Thought detail.
pub fn more_lines_row(truncated_by: usize, theme: &Theme) -> gpui::Div {
    div()
        .h(px(OUTPUT_LINE_HEIGHT))
        .flex()
        .items_center()
        .text_size(px(TOOL_TEXT_SIZE))
        .text_color(theme.text_faint)
        .child(SharedString::from(format!("… {truncated_by} more lines")))
}

/// Shape one flattened thought line into gpui text runs — the detail-body
/// palette: faint foreground prose, semibold for bold, mono for code,
/// underlined links (NOT clickable — a thought is a record, not a surface).
pub fn thought_line_text(
    line: &[InlineRun],
    theme: &Theme,
) -> Option<(SharedString, Vec<TextRun>)> {
    let mut text = String::new();
    let mut runs: Vec<TextRun> = Vec::new();
    for run in line {
        if run.text.is_empty() {
            continue;
        }
        let mut f = if run.style.code {
            gpui::font(theme.font_mono.clone())
        } else {
            gpui::font(theme.font_sans_fixed.clone())
        };
        if run.style.bold {
            f.weight = gpui::FontWeight::SEMIBOLD;
        }
        if run.style.italic {
            f.style = gpui::FontStyle::Italic;
        }
        runs.push(TextRun {
            len: run.text.len(),
            font: f,
            color: theme.text_faint,
            background_color: None,
            underline: run.style.link.is_some().then_some(gpui::UnderlineStyle {
                color: Some(theme.text_faint),
                thickness: px(1.0),
                wavy: false,
            }),
            strikethrough: run.style.strikethrough.then_some(gpui::StrikethroughStyle {
                thickness: px(1.0),
                color: Some(theme.text_faint),
            }),
        });
        text.push_str(&run.text);
    }
    if text.trim().is_empty() {
        return None;
    }
    Some((text.into(), runs))
}

/// The folded-narration chip's header detail — the first non-blank line of
/// the text, already flattened by [`thought_lines`] (inline markers became
/// styling, so they don't leak `**` glyphs into the one-liner).
pub fn note_chip_detail(tool: &ToolItem) -> String {
    let Some(ToolDetail::Thought { lines, .. }) = tool.detail.as_deref() else {
        return String::new();
    };
    for line in lines {
        let text: String = line.iter().map(|run| run.text.as_str()).collect();
        let text = single_line(text.trim());
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

/// The trailing tile on a chip header, when it has one.
pub enum ChipTrail {
    /// Expand/collapse chevron — flipped while the detail body is open.
    Chevron { open: bool },
    /// Top-right "opens elsewhere" arrow — the spawn chip's link to its
    /// subagent tab.
    OpenArrow,
}

/// Opens a tool chip's file in the files panel through the transcript's
/// workspace-link handler.
#[derive(Clone)]
pub struct ToolFileOpener {
    pub link: render::LinkUi,
}

impl ToolFileOpener {
    /// Only paths the link handler resolves — inside one of the chat's roots,
    /// or an absolute host file it opens read-only — get a clickable badge;
    /// the rest stay inert instead of looking clickable and doing nothing.
    pub fn opens(&self, path: &str) -> bool {
        self.link.file_link(path).is_some()
    }

    fn open(&self, path: &str, window: &mut Window, cx: &mut gpui::App) {
        render::activate_link(
            render::LinkTarget::new(file_badge_name(path), path),
            render::LinkAction::Primary,
            Some(&self.link),
            window,
            cx,
        );
    }
}

/// The chip's content row: icon tile + label + detail line (+ trailing tile
/// when the chip expands or links out). Shared between the plain chip, the
/// header of an expandable chip card, and the spawn link chip.
///
/// Spawn chips carry their subagent's lifecycle VISUALLY, in the chip's own
/// language: while running the mini working spinner (the sidebar's) pulses
/// at the right of the ordinary static detail; done is the ordinary quiet
/// chip; failed takes the danger tint — no status words, no live text (a
/// header rewriting itself per stream delta read as noise — user report).
pub fn chip_header_row(
    tool: &ToolItem,
    trail: Option<ChipTrail>,
    file_opener: Option<&ToolFileOpener>,
    theme: &Theme,
    view: gpui::EntityId,
    cx: &mut gpui::App,
) -> gpui::Div {
    let (label, detail) = match tool.kind {
        ToolItemKind::Thought => ("Thought process", String::new()),
        ToolItemKind::Note => ("Wrote", note_chip_detail(tool)),
        ToolItemKind::Call => tool_chip_content(&tool.call),
    };
    let activity = !is_agent_tool(tool);
    let file_path = match &tool.call {
        ToolCall::ReadFile { path }
        | ToolCall::WriteFile { path, .. }
        | ToolCall::EditFile { path, .. }
        | ToolCall::ApplyPatch { path: Some(path) } => Some(path.as_str()),
        _ => None,
    };
    let open_file = file_path
        .zip(file_opener)
        .filter(|(path, opener)| opener.opens(path))
        .map(|(path, opener)| (path.to_owned(), opener.clone()));
    let running = tool.subagent_ref.is_some()
        && matches!(tool.subagent_status, Some(SubagentStatus::Running));
    let failed = tool.is_error
        || (tool.subagent_ref.is_some()
            && matches!(tool.subagent_status, Some(SubagentStatus::Failed)));
    // Text resolves its color during layout, so group-hover text needs stable
    // child IDs under the keyed, expandable header to retain hover state.
    let hover_text = activity && trail.is_some() && !failed;
    let tint = if failed {
        theme.danger
    } else {
        theme.text_muted
    };
    div()
        .group("tool-header")
        .h(px(if activity {
            CHIP_CARD_HEIGHT
        } else {
            CHIP_HEADER_HEIGHT
        }))
        .w_full()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .px(px(if activity { 0.0 } else { 8.0 }))
        .text_size(px(TOOL_LABEL_SIZE))
        .line_height(px(TOOL_LABEL_LINE_HEIGHT))
        .when(!activity, |row| {
            row.child(
                // Subagent icon tile (`size-[18px] rounded-[5px] bg-white/[0.08]`,
                // icon size-3).
                div()
                    .size(px(18.0))
                    .flex_none()
                    .rounded(px(5.0))
                    .bg(crate::theme::ink(0.08))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        crate::icons::icon(match tool.kind {
                            ToolItemKind::Thought => crate::icons::CHAT_ROUND_LINE,
                            ToolItemKind::Note => crate::icons::PEN,
                            ToolItemKind::Call => tool_icon_path(&tool.call),
                        })
                        .size(px(12.0))
                        .text_color(theme.text_muted),
                    ),
            )
        })
        .child(
            div()
                .flex_none()
                .h(px(TOOL_LABEL_LINE_HEIGHT))
                .flex()
                .items_center()
                .when(!activity, |label| {
                    label.font_weight(gpui::FontWeight::MEDIUM)
                })
                .text_color(tint)
                .child(SharedString::from(label))
                .map(|label| {
                    if hover_text {
                        label
                            .id("tool-label")
                            .group_hover("tool-header", |style| style.text_color(theme.text))
                            .into_any_element()
                    } else {
                        label.into_any_element()
                    }
                }),
        )
        .child(
            div()
                .when(!activity, |detail| detail.flex_1())
                .min_w_0()
                .h(px(if file_path.is_some() {
                    22.0
                } else {
                    TOOL_LABEL_LINE_HEIGHT
                }))
                .flex()
                .when(activity && detail.is_empty(), |detail| detail.hidden())
                .items_center()
                .truncate()
                .text_color(if failed {
                    theme.danger
                } else if activity {
                    theme.text_muted
                } else {
                    theme.text.opacity(0.85)
                })
                .child(if let Some(path) = file_path {
                    let badge = div()
                        .min_w_0()
                        .h(px(22.0))
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .gap(px(6.0))
                        .rounded(px(5.0))
                        .bg(theme.ink(0.06))
                        .pl(px(1.0))
                        .pr(px(6.0))
                        .text_color(if failed {
                            theme.danger
                        } else {
                            theme.text.opacity(0.85)
                        })
                        .child(
                            div()
                                .size(px(20.0))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.0))
                                .bg(crate::file_icons::well_bg(theme))
                                .child(
                                    crate::file_icons::icon(
                                        crate::file_icons::FileIconIdentity::file(path),
                                        theme.appearance,
                                    )
                                    .size(px(14.0)),
                                ),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .child(SharedString::from(file_badge_name(path).to_owned())),
                        )
                        .map(|badge| {
                            if !hover_text && open_file.is_none() {
                                return badge.into_any_element();
                            }
                            badge
                                .id("tool-file-badge")
                                .when(hover_text, |badge| {
                                    badge.group_hover("tool-header", |style| {
                                        style.text_color(theme.text)
                                    })
                                })
                                // The badge is its own target: opening the
                                // file must not also toggle the chip's
                                // accordion underneath it.
                                .when_some(open_file, |badge, (path, opener)| {
                                    badge
                                        .debug_selector(|| "tool-file-badge".into())
                                        .cursor_pointer()
                                        .hover(|style| {
                                            style.bg(theme.ink(0.1)).text_color(theme.text)
                                        })
                                        .on_click(move |_, window, cx| {
                                            cx.stop_propagation();
                                            opener.open(&path, window, cx);
                                        })
                                })
                                .into_any_element()
                        });
                    crate::frost::frosted(5.0, 16.0, badge).into_any_element()
                } else {
                    div()
                        .min_w_0()
                        .truncate()
                        .child(SharedString::from(detail))
                        .into_any_element()
                })
                .map(|detail| {
                    if hover_text {
                        detail
                            .id("tool-detail")
                            .group_hover("tool-header", |style| style.text_color(theme.text))
                            .into_any_element()
                    } else {
                        detail.into_any_element()
                    }
                }),
        )
        .when_some(tool.call.subagent_model(), |row, model| {
            // Which model the child runs on, when the spawn named one.
            //
            // In the trailing slot rather than suffixed onto the detail: the
            // detail is the truncating slot, and the model is exactly what a
            // reader scanning a fan-out of spawns wants left once the
            // descriptions are cut.
            //
            // Bare faint text, NOT a filled pill: the tiles either side of it
            // are AFFORDANCES (the spinner means running, the arrow opens the
            // subagent), so giving a passive label the same chrome made the
            // trailing edge read as three buttons — the loudest thing in the
            // row was the one thing you cannot click.
            row.child(
                div()
                    .flex_none()
                    .h(px(18.0))
                    .flex()
                    .items_center()
                    .text_size(px(11.0))
                    .text_color(theme.text_faint)
                    .child(SharedString::from(model.to_owned())),
            )
        })
        .when(running, |row| {
            // The sidebar working-row spinner, in the chip's trailing slot —
            // paint-local (fixed footprint), so it never moves the layout.
            row.child(div().flex_none().child(crate::loaders::mini_glyph_spinner(
                format!(
                    "subagent-chip-{}",
                    tool.subagent_ref.as_deref().unwrap_or_default()
                ),
                2.0,
                theme.glyph,
                view,
                cx,
            )))
        })
        .when_some(trail, |row, trail| {
            // Trailing tile matching the group header's: a chevron for the
            // output/diff accordion, or the open-arrow for spawn chips.
            let tile = div()
                .size(px(18.0))
                .flex_none()
                .when(activity, |tile| {
                    tile.opacity(0.0)
                        .group_hover("tool-header", |style| style.opacity(1.0))
                })
                .when(!activity, |tile| {
                    tile.rounded(px(5.0)).bg(crate::theme::ink(0.06))
                })
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.text_muted.opacity(0.8));
            row.child(match trail {
                ChipTrail::Chevron { open } => tile.child(
                    crate::icons::icon(if open {
                        crate::icons::ALT_ARROW_DOWN
                    } else {
                        crate::icons::ALT_ARROW_RIGHT
                    })
                    .size(px(12.0))
                    .text_color(theme.text_faint)
                    .when(activity, |caret| {
                        caret.group_hover("tool-header", |style| {
                            style.text_color(if failed { theme.danger } else { theme.text })
                        })
                    }),
                ),
                ChipTrail::OpenArrow => tile.child(
                    crate::icons::icon(crate::icons::ARROW_UP_RIGHT)
                        .size(px(11.0))
                        .text_color(theme.text_muted.opacity(0.8)),
                ),
            })
        })
}

/// The header row of an expandable chip card.
pub fn chip_header(
    tool: &ToolItem,
    open: bool,
    file_opener: Option<&ToolFileOpener>,
    theme: &Theme,
    view: gpui::EntityId,
    cx: &mut gpui::App,
) -> gpui::Div {
    chip_header_row(
        tool,
        Some(ChipTrail::Chevron { open }),
        file_opener,
        theme,
        view,
        cx,
    )
}

/// Max chars a subagent tab title keeps. The strip chip is fixed-width and
/// truncates visually, but the derived title also rides drag ghosts and any
/// future pickers — cap it at the source.
pub const SUBAGENT_TITLE_MAX: usize = 40;

/// First line of `text`, trimmed, capped at `max` chars with an ellipsis.
pub fn title_line(text: &str, max: usize) -> Option<String> {
    let line = text.lines().find(|l| !l.trim().is_empty())?.trim();
    let mut out: String = line.chars().take(max).collect();
    if line.chars().count() > max {
        out.push('…');
    }
    Some(out)
}

/// Drop a leading "Agent"/"Task" genus (with its `:` and spacing) from a
/// spawn-title candidate. Only a real word boundary strips — "Taskmaster"
/// keeps its name. A bare "Agent"/"Task" strips to "" (no context at all).
pub fn strip_spawn_prefix(text: &str) -> &str {
    let t = text.trim();
    for prefix in ["agent", "task"] {
        if t.len() >= prefix.len()
            && t.is_char_boundary(prefix.len())
            && t[..prefix.len()].eq_ignore_ascii_case(prefix)
        {
            let rest = &t[prefix.len()..];
            if rest.is_empty() {
                return "";
            }
            if rest.starts_with(':') || rest.starts_with(char::is_whitespace) {
                return rest.trim_start_matches(':').trim();
            }
        }
    }
    t
}

/// Tab title for a spawn chip's subagent surface: the BARE task description
/// ("verify the marker pipeline"). The chip keeps the tool's fuller name —
/// a fixed-width tab spent on "Agent: " never shows the task, so the genus
/// is stripped here and the call input's description/prompt fields back up
/// a bare name (older docs); "Subagent" only as the last resort.
pub fn subagent_tab_title(call: &ToolCall) -> SharedString {
    let (name, input) = match call {
        ToolCall::Unknown { name, input } => (name.as_str(), input.as_ref()),
        ToolCall::Mcp { tool, input, .. } => (tool.as_str(), input.as_ref()),
        _ => return "Subagent".into(),
    };
    let candidates = [
        Some(name),
        input.and_then(|i| i.get("description")?.as_str()),
        input.and_then(|i| i.get("prompt")?.as_str()),
    ];
    for text in candidates.into_iter().flatten() {
        if let Some(title) = title_line(strip_spawn_prefix(text), SUBAGENT_TITLE_MAX) {
            return title.into();
        }
    }
    "Subagent".into()
}

/// Clip one newly appended task row to its committed height. Fade and lift are
/// applied to the row content itself, leaving the connector at full contrast
/// while it draws; fading the whole row made the path animation imperceptible.
pub fn reveal_tool_row(row: AnyElement, height: f32, progress: f32) -> AnyElement {
    if progress >= 1.0 {
        return row;
    }
    div()
        .w_full()
        .h(px(height * progress))
        .flex_none()
        .overflow_hidden()
        .child(row)
        .into_any_element()
}

/// BoardUI-style task tree with OpenAgents's tool glyph restored at each branch tip.
/// The previous row draws the first leg of a new arrival to its lower boundary;
/// this row then continues down, rounds the elbow, and finally reveals the icon.
/// One paint owns every segment in a row, avoiding alpha-darkened joints.
pub fn activity_rail(
    tool: &ToolItem,
    has_predecessor: bool,
    continues: bool,
    reveal: f32,
    continuation_reveal: f32,
    row_height: f32,
    theme: &Theme,
) -> gpui::Div {
    let color = theme.hairline(0.12);
    let tint = if tool.is_error {
        theme.danger
    } else {
        theme.text_muted
    };
    let (incoming_reveal, branch_reveal) = tool_connector_parts(reveal, has_predecessor);
    div()
        .relative()
        .w(px(ACTIVITY_GUTTER_WIDTH))
        .flex_none()
        .child(
            canvas(
                move |_, _, _| (),
                move |bounds, _, window, _| {
                    let x = bounds.origin.x + px(ACTIVITY_TRUNK_X);
                    let branch_y = bounds.origin.y + px(row_height / 2.0);
                    let bend_y = branch_y - px(ACTIVITY_BEND_RADIUS);
                    // The web draws the tree from quads, square elbows and
                    // all: path tessellation is a large part of a module.
                    #[cfg(target_family = "wasm")]
                    {
                        let mut line = |origin: Point<Pixels>, size: gpui::Size<Pixels>| {
                            window.paint_quad(gpui::fill(gpui::Bounds::new(origin, size), color));
                        };
                        if incoming_reveal > 0.0 {
                            let mut bottom = bounds.origin.y
                                + px((row_height / 2.0 - ACTIVITY_BEND_RADIUS) * incoming_reveal);
                            if incoming_reveal >= 1.0 && continues && continuation_reveal > 0.0 {
                                let continuation_height = (f32::from(bounds.size.height)
                                    - (row_height / 2.0 - ACTIVITY_BEND_RADIUS))
                                    .max(0.0);
                                bottom = bend_y + px(continuation_height * continuation_reveal);
                            }
                            line(
                                point(x - px(0.5), bounds.origin.y),
                                gpui::size(px(1.0), bottom - bounds.origin.y),
                            );
                        }
                        if branch_reveal > 0.0 {
                            let down = (branch_y - bend_y) * branch_reveal.min(0.5) * 2.0;
                            line(point(x - px(0.5), bend_y), gpui::size(px(1.0), down));
                            let across = (ACTIVITY_BRANCH_END_X - ACTIVITY_TRUNK_X)
                                * ((branch_reveal - 0.5).max(0.0) * 2.0);
                            if across > 0.0 {
                                line(
                                    point(x - px(0.5), branch_y - px(0.5)),
                                    gpui::size(px(across + 0.5), px(1.0)),
                                );
                            }
                        }
                    }
                    // Union the ribbons before painting. Stroke tessellation
                    // blends intersections twice, even in a single path.
                    #[cfg(not(target_family = "wasm"))]
                    let mut tree = PathBuilder::fill().with_style(gpui::PathStyle::Fill(
                        gpui::FillOptions::default().with_fill_rule(gpui::FillRule::NonZero),
                    ));
                    #[cfg(not(target_family = "wasm"))]
                    if incoming_reveal > 0.0 {
                        let mut bottom = point(
                            x,
                            bounds.origin.y
                                + px((row_height / 2.0 - ACTIVITY_BEND_RADIUS) * incoming_reveal),
                        );
                        if incoming_reveal >= 1.0 && continues && continuation_reveal > 0.0 {
                            let continuation_height = (f32::from(bounds.size.height)
                                - (row_height / 2.0 - ACTIVITY_BEND_RADIUS))
                                .max(0.0);
                            bottom =
                                point(x, bend_y + px(continuation_height * continuation_reveal));
                        }
                        activity_ribbon(&mut tree, &[point(x, bounds.origin.y), bottom]);
                    }
                    #[cfg(not(target_family = "wasm"))]
                    if branch_reveal > 0.0 {
                        let points: Vec<_> = activity_branch_points(branch_reveal)
                            .into_iter()
                            .map(|p| point(x + px(p.x), bend_y + px(p.y)))
                            .collect();
                        activity_ribbon(&mut tree, &points);
                    }
                    #[cfg(not(target_family = "wasm"))]
                    if let Ok(path) = tree.build() {
                        window.paint_path(path, color);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(
            crate::icons::icon(match tool.kind {
                ToolItemKind::Thought => crate::icons::CHAT_ROUND_LINE,
                ToolItemKind::Note => crate::icons::PEN,
                ToolItemKind::Call => tool_icon_path(&tool.call),
            })
            .absolute()
            .left(px(ACTIVITY_ICON_LEFT))
            .top(px(row_height / 2.0 - ACTIVITY_ICON_SIZE / 2.0))
            .size(px(ACTIVITY_ICON_SIZE))
            .opacity(branch_reveal)
            .text_color(tint),
        )
}

/// A clockwise ribbon contour. Nonzero fill unions intersecting contours,
/// preserving one alpha contribution at the fork on transparent surfaces.
#[cfg(not(target_family = "wasm"))]
pub fn activity_ribbon(path: &mut PathBuilder, points: &[Point<Pixels>]) {
    let mut left = Vec::with_capacity(points.len());
    let mut right = Vec::with_capacity(points.len());
    for (ix, p) in points.iter().enumerate() {
        let a = points[ix.saturating_sub(1)];
        let b = points[(ix + 1).min(points.len() - 1)];
        let dx = f32::from(b.x - a.x);
        let dy = f32::from(b.y - a.y);
        let length = dx.hypot(dy).max(0.0001);
        let normal = point(px(-dy / length * 0.5), px(dx / length * 0.5));
        left.push(*p + normal);
        right.push(*p - normal);
    }
    path.move_to(left[0]);
    for p in left.iter().skip(1).chain(right.iter().rev()) {
        path.line_to(*p);
    }
    path.close();
}

/// A plain activity row, or a card for a subagent without a linked document.
pub fn tool_chip(
    tool: &ToolItem,
    rail: bool,
    has_predecessor: bool,
    continues: bool,
    content_reveal: f32,
    connector_reveal: f32,
    continuation_reveal: f32,
    file_opener: Option<&ToolFileOpener>,
    theme: &Theme,
    view: gpui::EntityId,
    cx: &mut gpui::App,
) -> AnyElement {
    let row_height = if rail {
        TOOL_TREE_ROW_HEIGHT
    } else {
        CHIP_HEIGHT
    };
    div()
        .h(px(row_height))
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .when(rail, |row| {
            row.child(activity_rail(
                tool,
                has_predecessor,
                continues,
                connector_reveal,
                continuation_reveal,
                row_height,
                theme,
            ))
        })
        .child(
            div()
                .when(rail, |el| el.ml(px(ACTIVITY_TEXT_GAP)))
                .my(px((row_height - CHIP_CARD_HEIGHT) / 2.0))
                .h(px(CHIP_CARD_HEIGHT))
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .overflow_hidden()
                .when(!rail, |card| {
                    card.rounded(px(9.0))
                        .border_1()
                        .border_color(crate::theme::hairline(0.07))
                        .bg(crate::theme::ink(0.03))
                })
                .when(rail && content_reveal < 1.0, |card| {
                    card.relative()
                        .top(px(4.0 * (1.0 - content_reveal)))
                        .opacity(content_reveal)
                })
                .child(chip_header_row(tool, None, file_opener, theme, view, cx)),
        )
        .into_any_element()
}

/// A spawn chip: same card as [`tool_chip`], but the WHOLE card is the
/// "open the subagent tab" click (open-arrow tile in the trailing slot).
/// No accordion — an inline body would only repeat the subagent's own
/// transcript. The group guide rail is omitted for agent-only rows (no
/// collapse header for it to hang from).
pub fn subagent_chip(
    tool: &ToolItem,
    id: SharedString,
    on_open: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    rail: bool,
    theme: &Theme,
    view: gpui::EntityId,
    cx: &mut gpui::App,
) -> AnyElement {
    div()
        .h(px(CHIP_HEIGHT))
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .when(rail, |row| {
            row.child(
                div()
                    .ml(px(12.0))
                    .h_full()
                    .w(px(1.0))
                    .flex_none()
                    .bg(crate::theme::ink(0.08)),
            )
        })
        .child(
            div()
                .id(id)
                .when(rail, |el| el.ml(px(12.0)))
                .h(px(CHIP_CARD_HEIGHT))
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .overflow_hidden()
                .rounded(px(9.0))
                .border_1()
                .border_color(crate::theme::hairline(0.07))
                .bg(crate::theme::ink(0.03))
                .cursor_pointer()
                .hover(|s| s.bg(crate::theme::ink(0.05)))
                .on_click(on_open)
                .child(chip_header_row(
                    tool,
                    Some(ChipTrail::OpenArrow),
                    None,
                    theme,
                    view,
                    cx,
                )),
        )
        .into_any_element()
}

pub fn entry_fingerprint(entry: &SessionMessageEntry, pending: bool) -> u64 {
    let mut acc: Vec<u8> = Vec::with_capacity(entry.parts.len() * 8 + 16);
    acc.extend_from_slice(entry.id.as_bytes());
    acc.extend_from_slice(entry.device_id.as_bytes());
    acc.push(match entry.status {
        None => 0,
        Some(MessageStatus::Streaming) => 1,
        Some(MessageStatus::Complete) => 2,
        Some(MessageStatus::Aborted) => 3,
    });
    acc.push(pending as u8);
    acc.extend_from_slice(&entry.duration_ms.unwrap_or(0).to_le_bytes());
    for part in &entry.parts {
        acc.extend_from_slice(part.id().as_bytes());
        acc.extend_from_slice(&(part.byte_len() as u64).to_le_bytes());
        if let MessagePart::Tool {
            is_error,
            resolved,
            subagent_ref,
            subagent_status,
            subagent_tail,
            ..
        } = part
        {
            acc.push(*is_error as u8 | (*resolved as u8) << 1);
            // Subagent lifecycle mutates a COMPLETED entry in place (eager-
            // done: the spawn resolves while the subagent runs on) and
            // `byte_len` above doesn't cover these fields — hash them or the
            // cached rows never refresh on status/tail changes.
            acc.push(
                subagent_ref.is_some() as u8
                    | match subagent_status {
                        None => 0,
                        Some(SubagentStatus::Running) => 1 << 1,
                        Some(SubagentStatus::Done) => 2 << 1,
                        Some(SubagentStatus::Failed) => 3 << 1,
                    },
            );
            if let Some(tail) = subagent_tail {
                acc.extend_from_slice(tail.as_bytes());
            }
        }
        if let MessagePart::Image {
            path,
            name,
            mime_type,
            ..
        } = part
        {
            for field in [path, name, mime_type] {
                acc.extend_from_slice(field.as_bytes());
                acc.push(0);
            }
        }
        if let MessagePart::Visual { path, title, .. } = part {
            for field in [path, title] {
                acc.extend_from_slice(field.as_bytes());
                acc.push(0);
            }
        }
        if let MessagePart::Input { resolved, .. } = part {
            acc.push(0x10 | *resolved as u8);
        }
    }
    fnv1a(&acc)
}

impl<M: TranscriptMedia> Render for TranscriptView<M> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if record_view_frame("transcript") {
            tracing::warn!(
                distance = self.distance_from_bottom(),
                spring = self.spring_should_run(),
                velocity = self.spring.velocity,
                target_velocity = self.spring.target_vel,
                own_turn = self.own_turn.is_some(),
                veils = self.veils.len(),
                "transcript motion state"
            );
        }
        self.render_cache
            .borrow_mut()
            .retain_rows(&self.rendered_rows);
        self.rendered_rows.clear();
        let compact_mode = crate::prefs::transcript_compact_mode(cx);
        if self.compact_mode != compact_mode {
            self.compact_mode = compact_mode;
            // The row SPLIT differs by mode (one work fold vs per-segment
            // groups + narration rows) — the fingerprint above already keys
            // on it, so a fresh sync rebuilds everything.
            self.last_source = None;
            self.sync(cx);
        }
        let code_fences_generation = crate::prefs::code_fences_generation(cx);
        if self.code_fences_generation != code_fences_generation {
            self.code_fences_generation = code_fences_generation;
            // Horizontal positions are ephemeral. Reset every block owned by
            // this Transcript even when the toggle originated in another one.
            for runtime in self.code_fences.values() {
                runtime.scroll.set_offset(Point::default());
            }
            // Fit changes every code row from analytic to measured height (or
            // back), including virtual rows outside the current viewport.
            self.list.remeasure();
            if self.pinned {
                self.wake_spring();
            }
            if self.own_turn.is_some() {
                self.own_turn_kick = true;
            }
        }
        let content_width = crate::prefs::transcript_width(cx);
        if self.content_width != content_width {
            self.content_width = content_width;
            // The outer list viewport may not resize when only max-width
            // changes. Invalidate virtual row heights explicitly, retaining
            // their anchors and all live animation/provenance state.
            self.list.remeasure();
            if self.pinned {
                self.wake_spring();
            }
            if self.own_turn.is_some() {
                self.own_turn_kick = true;
            }
        }
        let typography_generation = crate::prefs::type_generation(cx);
        if self.typography_generation != typography_generation {
            self.typography_generation = typography_generation;
            // `refresh_windows` re-lays out visible rows, but ListState keeps
            // measured heights for virtualized rows outside the viewport.
            // Mark every row unmeasured while retaining height hints and a
            // proportional scroll anchor; GPUI will refresh each measurement
            // as the row enters its layout range.
            self.list.remeasure();
        }
        // Media follow the reading column (diagrams rasterize for it).
        let list_width = f32::from(self.list.viewport_bounds().size.width);
        let column = if list_width > 0.0 {
            self.content_width
                .min((list_width - self.right_reserve).max(1.0))
        } else {
            self.content_width
        };
        M::begin_frame(self, column, window, cx);
        // Own-turn driver: measurements are only authoritative after layout,
        // so reservation sizing, the send glide, and the outgrown-handoff
        // each advance at most once per requested frame. Scheduled on every
        // frame while an anchor is live (not just on kicks) so viewport
        // resizes and streaming growth re-derive the reservation; the step
        // only notifies on change, so a settled hold schedules no next frame.
        if !self.route_exit_pending(cx)
            && (self.own_turn.is_some() || self.own_turn_kick)
            && !self.own_turn_scheduled
        {
            self.own_turn_scheduled = true;
            let entity = cx.weak_entity();
            window.on_next_frame(move |_, cx| {
                entity
                    .update(cx, |this: &mut Self, cx| {
                        this.own_turn_scheduled = false;
                        this.step_own_turn(cx);
                    })
                    .ok();
            });
        }
        // Spring driver: one on_next_frame callback at a time; each tick
        // notifies, which re-enters render and schedules the next frame until
        // the spring parks. Reduced motion never schedules (sync snaps).
        if !self.route_exit_pending(cx)
            && self.pinned
            && !motion::reduced_motion(cx)
            && !self.spring_scheduled
            && self.spring_should_run()
        {
            self.spring_scheduled = true;
            let entity = cx.weak_entity();
            window.on_next_frame(move |_, cx| {
                entity
                    .update(cx, |this: &mut Self, cx| {
                        this.spring_scheduled = false;
                        this.step_spring(cx);
                    })
                    .ok();
            });
        }
        // Programmatic `scroll_to` does not invoke the list's user-scroll
        // handler. Refresh distance-derived state once layout has measured the
        // replay, guarded so a stale A callback cannot mutate B (or a newer A).
        if self.viewport_finalize_pending && !self.viewport_finalize_scheduled {
            self.viewport_finalize_scheduled = true;
            let token = ViewportFinalizeToken {
                generation: self.viewport_generation,
                layout_revision: self.viewport_layout_revision,
            };
            let entity = cx.weak_entity();
            window.on_next_frame(move |_, cx| {
                entity
                    .update(cx, |this: &mut Self, cx| {
                        this.viewport_finalize_scheduled = false;
                        if !token.still_current(this.viewport_generation) {
                            if this.viewport_finalize_pending {
                                cx.notify();
                            }
                            return;
                        }
                        let distance = this.distance_from_bottom();
                        this.last_scroll_distance = distance;
                        this.show_jump_button = jump_visibility(this.show_jump_button, distance)
                            && !this.pinned
                            && !this.own_turn.as_ref().is_some_and(|turn| turn.held);
                        if token.layout_settled(this.viewport_layout_revision) {
                            this.viewport_finalize_pending = false;
                        }
                        cx.notify();
                    })
                    .ok();
            });
        }
        // A long-message collapse near the bottom owns the viewport for the
        // duration of its height tween. Advance the matching upward scroll once
        // per frame so the bubble stays visible instead of shrinking above the
        // fixed viewport while the bottom content remains on screen.
        if self.user_collapse_scroll.is_some() && !self.user_collapse_scroll_scheduled {
            self.user_collapse_scroll_scheduled = true;
            let entity = cx.weak_entity();
            window.on_next_frame(move |_, cx| {
                entity
                    .update(cx, |this: &mut Self, cx| {
                        this.user_collapse_scroll_scheduled = false;
                        this.step_user_collapse_scroll(cx);
                    })
                    .ok();
            });
        }
        let rail = self.render_rail(cx);
        // The scroll-to-bottom pill is rendered by the SHELL (conversation
        // region overlay): it must float just above the composer and paint
        // OVER the bottom fade gradient, which is a later sibling of this
        // outlet — an overlay here would be tinted by the fade.
        self.update_runway_minimum(cx);
        let list_el = crate::overscroll::overscroll(
            "transcript-overscroll",
            crate::overscroll::OverscrollSource::List(self.list.clone()),
            list(self.list.clone(), cx.processor(Self::render_row))
                .size_full()
                .with_sizing_behavior(
                    if self.host_layout.is_some_and(|layout| layout.fit_content) {
                        gpui::ListSizingBehavior::Infer
                    } else {
                        gpui::ListSizingBehavior::Auto
                    },
                ),
        );
        let content: AnyElement = if self.doc_override.is_some() {
            // The primary transcript's fade lives on the SHELL's outlet
            // wrapper (it spans the titlebar/composer chrome); an override
            // instance owns its own — top edge only (nothing overlays the
            // pane's bottom), gated on real overflow so a short top-anchored
            // transcript shows no fade. Gated here rather than at paint via
            // a ScrollHandle (the list isn't one); scrolls re-render this
            // entity, so the flag can't go stale.
            let scrolled_under_top = {
                let max = f32::from(self.list.max_offset_for_scrollbar().y);
                max - self.distance_from_bottom() > 1.0
            };
            crate::edge_fade::edge_faded(
                Theme::TRANSCRIPT_FADE_BAND,
                scrolled_under_top,
                false,
                list_el,
            )
            .into_any_element()
        } else {
            list_el.into_any_element()
        };
        let fit_content = self.host_layout.is_some_and(|layout| layout.fit_content);
        let root = div()
            .relative()
            .when(!fit_content, |root| root.size_full())
            .when(fit_content, |root| {
                root.w_full().flex().flex_col().flex_shrink(1.)
            })
            .min_h_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(Self::on_selection_mouse_down),
            )
            .on_mouse_move(cx.listener(Self::on_selection_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_selection_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_selection_mouse_up))
            // FIRST child ⇒ paints first: clears this transcript's slice of the
            // frame's markdown text-selection registry before any row's text
            // elements re-register (document paint order = selection order;
            // see markdown/render.rs). Keyed by entity so a side chat or
            // subagent tab painted in the same frame can't wipe it.
            .child(crate::markdown::render::selection_frame_reset_for(
                cx.entity_id().as_u64(),
            ))
            .child(content)
            .child(rail);
        root.children(M::overlays(self, window, cx))
    }
}

// The retained desktop integration tests exercise the shared transcript.
#[cfg(feature = "test-support")]
impl<M: TranscriptMedia> TranscriptView<M> {
    #[doc(hidden)]
    pub fn test_release_own_turn_hold(&mut self) {
        self.release_own_turn_hold();
    }
    #[doc(hidden)]
    pub fn test_remeasure_last_row(&mut self) {
        self.remeasure_last_row();
    }
    #[doc(hidden)]
    pub fn test_handle_scroll(&mut self, event: &ListScrollEvent, cx: &mut Context<Self>) {
        self.handle_scroll(event, cx);
    }
    #[doc(hidden)]
    pub fn test_own_send_inset(ix: usize) -> f32 {
        Self::own_send_inset(ix)
    }
    #[doc(hidden)]
    pub fn test_own_turn_anchor_ix(&self) -> Option<usize> {
        self.own_turn_anchor_ix()
    }
    #[doc(hidden)]
    pub fn test_step_own_turn(&mut self, cx: &mut Context<Self>) {
        self.step_own_turn(cx);
    }
    #[doc(hidden)]
    pub fn test_step_spring(&mut self, cx: &mut Context<Self>) {
        self.step_spring(cx);
    }
    #[doc(hidden)]
    pub fn test_toggle_user_fold(
        &mut self,
        row_id: SharedString,
        row_ix: usize,
        collapsed: f32,
        full: f32,
        reduced: bool,
    ) {
        self.toggle_user_fold(row_id, row_ix, collapsed, full, reduced);
    }
    #[doc(hidden)]
    pub fn test_step_user_collapse_scroll(&mut self, cx: &mut Context<Self>) {
        self.step_user_collapse_scroll(cx);
    }
    #[doc(hidden)]
    pub fn test_render_row(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_row(ix, window, cx)
    }
}
