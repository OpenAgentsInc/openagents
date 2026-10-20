/// Input text metrics: 14px type with a 22.75px line height.
pub const INPUT_LINE_HEIGHT: f32 = 22.75;
pub const INPUT_TEXT_SIZE: f32 = 14.0;
/// Fade only the overflowing edge of the input.
const INPUT_FADE_BAND: f32 = 12.0;
/// Drag-selection autoscroll cadence.
pub const DRAG_SCROLL_FRAME_MS: u64 = 16;

/// Caret blink half-period (standard textarea cadence: ~500ms on / 500ms off).
pub const CARET_BLINK_MS: u64 = 500;

/// Caret blink phase for a time since the last keystroke/caret move: solid
/// through the first half-period (typing bursts never blink — each keystroke
/// resets the phase), then alternating.
pub fn caret_visible(ms_since_activity: u64) -> bool {
    (ms_since_activity / CARET_BLINK_MS) % 2 == 0
}

/// Auto-grow: content height for a wrapped-line count.
pub fn input_content_height(wrapped_lines: usize) -> f32 {
    wrapped_lines.max(1) as f32 * INPUT_LINE_HEIGHT
}

fn input_max_scroll(content_height: f32, viewport_height: f32) -> f32 {
    (content_height - viewport_height).max(0.0)
}

/// Only settled overflow gets a scroll fade. The animated viewport can be
/// smaller for a few frames while an otherwise fitting draft grows into it.
fn input_overflow_edges(
    content_height: f32,
    settled_height: f32,
    visible_height: f32,
    scroll_top: f32,
) -> (bool, bool) {
    if input_max_scroll(content_height, settled_height) <= 1.0 {
        return (false, false);
    }
    let max_scroll = input_max_scroll(content_height, visible_height);
    (scroll_top > 1.0, scroll_top < max_scroll - 1.0)
}

/// During the reveal, stop at a complete row boundary instead of slicing
/// glyphs with a moving clip. Scrolling offsets the row grid inside the box.
fn input_reveal_height(visible: f32, scroll: f32, line_height: f32, resizing: bool) -> f32 {
    if !resizing {
        return visible;
    }
    let row_end = ((scroll + visible + 0.001) / line_height).floor() * line_height;
    (row_end - scroll).clamp(0.0, visible)
}

/// Apply GPUI's wheel delta to a top-origin input offset. Positive deltas mean
/// scrolling toward the start, matching gpui's built-in list/div behavior.
fn input_scroll_offset(
    current: f32,
    delta_y: f32,
    content_height: f32,
    viewport_height: f32,
) -> f32 {
    (current - delta_y).clamp(0.0, input_max_scroll(content_height, viewport_height))
}

/// Minimally adjust the viewport so the caret row is fully visible.
fn input_scroll_offset_for_cursor(
    current: f32,
    cursor_top: f32,
    cursor_height: f32,
    content_height: f32,
    viewport_height: f32,
    settled_height: Option<f32>,
) -> f32 {
    // Resize the reveal, not the scroll position: existing text stays fixed
    // relative to the input origin throughout the height animation.
    let viewport_height = settled_height.unwrap_or(viewport_height);
    let mut next = current;
    if cursor_top < next {
        next = cursor_top;
    } else if cursor_top + cursor_height > next + viewport_height {
        next = cursor_top + cursor_height - viewport_height;
    }
    next.clamp(0.0, input_max_scroll(content_height, viewport_height))
}

/// Pointer selection granularity; multi-click drags retain the initial unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PressIntent {
    Word,
    Line,
    ExtendSelection,
    PlaceCaret,
}

fn press_intent(click_count: usize, shift: bool) -> PressIntent {
    if click_count >= 3 {
        PressIntent::Line
    } else if click_count == 2 {
        PressIntent::Word
    } else if shift {
        PressIntent::ExtendSelection
    } else {
        PressIntent::PlaceCaret
    }
}

fn word_range(text: &str, offset: usize) -> Range<usize> {
    let offset = offset.min(text.len());
    text.split_word_bound_indices()
        .find(|(at, word)| *at <= offset && offset < at + word.len())
        .map(|(at, word)| at..at + word.len())
        .unwrap_or(offset..offset)
}

/// Per-frame drag-selection scroll. Distance increases speed, capped at one
/// text row per frame so crossing the input boundary never causes a jump.
fn input_drag_scroll_delta(
    pointer_y: f32,
    viewport_top: f32,
    viewport_bottom: f32,
    line_height: f32,
) -> f32 {
    let distance = if pointer_y < viewport_top {
        pointer_y - viewport_top
    } else if pointer_y > viewport_bottom {
        pointer_y - viewport_bottom
    } else {
        return 0.0;
    };
    distance.signum() * (distance.abs() * 0.2).clamp(1.0, line_height)
}

/// Staged-attachment strip metrics (openagents attachment-ui.tsx AttachmentStrip:
/// `flex flex-wrap gap-2 px-4 pt-3`, `size-14` thumbs).
pub const STRIP_THUMB: f32 = 56.0;
pub const STRIP_GAP: f32 = 8.0;
pub const STRIP_PAD_TOP: f32 = 12.0;
pub const STRIP_PAD_X: f32 = 16.0;

/// Height the wrap strip adds to the pill for `count` staged thumbnails at an
/// `inner_width` pill content width (0 when empty). Mirrors flex-wrap: as many
fn escape_dismisses_completion(key: &str, completion_open: bool) -> bool {
    key == "escape" && completion_open
}

macro_rules! define_input_actions {
    ($namespace:ident) => {
        actions!(
            $namespace,
            [
                Backspace,
                Delete,
                Left,
                Right,
                Up,
                Down,
                SelectLeft,
                SelectRight,
                SelectUp,
                SelectDown,
                SelectAll,
                Home,
                End,
                SelectHome,
                SelectEnd,
                DocStart,
                DocEnd,
                SelectDocStart,
                SelectDocEnd,
                WordLeft,
                WordRight,
                SelectWordLeft,
                SelectWordRight,
                DeleteWordLeft,
                DeleteWordRight,
                DeleteToLineStart,
                DeleteToLineEnd,
                Copy,
                Cut,
                Paste,
                Newline,
                MessageNewlineOrAccept,
                ModifiedSubmit,
                Submit,
                Undo,
                Redo,
                MentionTab,
                ToggleDictation,
                OutdentList,
            ]
        );
    };
}

/// How long a run of single-character edits keeps merging into one undo step.
/// A pause longer than this starts a fresh step, so undo rewinds in the
/// bursts the user actually typed rather than one character at a time.
const UNDO_COALESCE: Duration = Duration::from_millis(700);

/// Cap on retained undo steps — a long-lived composer must not grow forever.
const UNDO_LIMIT: usize = 200;

const MENTION_TOOLTIP_DELAY: Duration = Duration::from_millis(420);
const MENTION_TOOLTIP_HEIGHT: f32 = 24.0;
/// A private URI scheme keeps file mentions distinguishable from ordinary
/// Markdown links pasted into the composer.
use openagents_proto::file_mentions::{local_file_link, local_path_is_safe};

/// A restorable point in the input's history: text plus where the caret and
/// selection sat when the edit landed.
#[derive(Clone)]
struct EditSnapshot {
    content: String,
    selected_range: Range<usize>,
    selection_reversed: bool,
    caret_affinity: CaretAffinity,
}

/// Build the text inserted when a workspace item is dropped at an arbitrary
/// selection. Unlike completion, a drop does not necessarily happen at a
/// token boundary, so it supplies its own leading separator when needed.
/// A reference must not introduce whitespace before closing Markdown syntax.
/// Existing horizontal whitespace is reused; line endings stay on their row.
fn reference_suffix(next: Option<char>) -> (&'static str, usize) {
    match next {
        Some('\n' | '\r') => ("", 0),
        Some(ch) if ch.is_whitespace() => ("", ch.len_utf8()),
        Some(')' | ']' | '}' | '*' | '_' | '~' | ',' | '.' | ';' | ':' | '!' | '?') => ("", 0),
        _ => (" ", 0),
    }
}

/// Place a reference link at an arbitrary selection, supplying the separators
/// a completion would already have had around it.
fn dropped_reference(content: &str, range: Range<usize>, link: &str) -> Option<(String, usize)> {
    if range.start > range.end || !content.is_char_boundary(range.start) {
        return None;
    }
    let suffix = content.get(range.end..)?;
    let prefix = if range.start > 0
        && content[..range.start]
            .chars()
            .next_back()
            .is_some_and(|ch| {
                !ch.is_whitespace() && !matches!(ch, '(' | '[' | '{' | '*' | '_' | '~')
            }) {
        " "
    } else {
        ""
    };
    let (trailing, advance) = reference_suffix(suffix.chars().next());
    let inserted = format!("{prefix}{link}{trailing}");
    let cursor_advance = inserted.len() + advance;
    Some((inserted, cursor_advance))
}

/// A path alone is not enough: two identical relative paths can appear in a
/// draft, so the raw range remains part of the hover identity.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MentionTooltipTarget {
    range: Range<usize>,
    path: SharedString,
    /// Set for attachment chips. An image chip has no tooltip: a click opens
    /// the picture full size instead.
    attachment: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MentionTooltipPhase {
    Hidden,
    Waiting {
        target: MentionTooltipTarget,
        generation: u64,
    },
    Visible {
        target: MentionTooltipTarget,
        generation: u64,
    },
}

impl MentionTooltipPhase {
    fn target(&self) -> Option<&MentionTooltipTarget> {
        match self {
            Self::Hidden => None,
            Self::Waiting { target, .. } | Self::Visible { target, .. } => Some(target),
        }
    }
}

/// Pure tooltip lifecycle reducer. Motion within the same chip preserves both
/// waiting and visible phases, so normal pointer jitter cannot starve the
/// delay or flicker an already-visible tooltip.
fn mention_tooltip_reduce(
    phase: MentionTooltipPhase,
    pointer_target: Option<MentionTooltipTarget>,
    pointer_in_popup: bool,
    generation: u64,
) -> MentionTooltipPhase {
    match pointer_target {
        Some(target) if phase.target() == Some(&target) => phase,
        Some(target) => MentionTooltipPhase::Waiting { target, generation },
        None if pointer_in_popup && matches!(phase, MentionTooltipPhase::Visible { .. }) => phase,
        None => MentionTooltipPhase::Hidden,
    }
}

fn mention_tooltip_promote(
    phase: MentionTooltipPhase,
    generation: u64,
    target_is_live: bool,
) -> MentionTooltipPhase {
    match phase {
        MentionTooltipPhase::Waiting {
            target,
            generation: current,
        } if current == generation && target_is_live => MentionTooltipPhase::Visible {
            target,
            generation: current,
        },
        MentionTooltipPhase::Waiting {
            generation: current,
            ..
        } if current == generation => MentionTooltipPhase::Hidden,
        phase => phase,
    }
}

fn mention_tooltip_contains(in_chip: bool, in_popup: bool) -> bool {
    in_chip || in_popup
}

fn display_row_segments(
    range: Range<usize>,
    row_ends: impl IntoIterator<Item = usize>,
) -> Vec<(usize, usize, Range<usize>)> {
    let mut segments = Vec::new();
    let mut row_start = 0usize;
    for (row_ix, row_end) in row_ends.into_iter().enumerate() {
        let start = range.start.max(row_start);
        let end = range.end.min(row_end);
        if start < end {
            segments.push((row_ix, row_start, start..end));
        }
        row_start = row_end;
        if row_start >= range.end {
            break;
        }
    }
    segments
}

#[derive(Debug, Clone)]
struct MentionHit {
    target: MentionTooltipTarget,
    bounds: Bounds<Pixels>,
    anchor: Point<Pixels>,
}

/// GPUI's text wrapper permits breaks at Unicode spacer glyphs. Reference
/// chips need stronger boundaries than ordinary prose, without changing ZUI.
fn wrap_reference_chips(line: &mut WrappedLine, chips: &[Range<usize>], width: Pixels) {
    if chips.is_empty() {
        return;
    }
    let glyphs: Vec<_> = line
        .runs()
        .iter()
        .enumerate()
        .flat_map(|(run_ix, run)| {
            run.glyphs.iter().enumerate().map(move |(glyph_ix, glyph)| {
                (
                    glyph.index,
                    glyph.position.x,
                    gpui::WrapBoundary { run_ix, glyph_ix },
                )
            })
        })
        .collect();
    let inside = |index| {
        chips
            .iter()
            .any(|range| range.start < index && index < range.end)
    };
    // Keep complete references together; oversized references can still wrap.
    let mut boundaries = line.wrap_boundaries.clone();
    boundaries.clear();
    let mut row = 0;
    while row < glyphs.len() {
        let limit = glyphs[row].1 + width;
        let overflow = (row..glyphs.len()).find(|&i| {
            glyphs
                .get(i + 1)
                .map_or(line.unwrapped_layout.width, |g| g.1)
                > limit
        });
        let Some(overflow) = overflow else {
            break;
        };
        // Prefer whitespace or a complete chip boundary. If an individual
        // token is wider than the viewport, retain the usual emergency wrap.
        let preferred = (row + 1..=overflow).rev().find(|&i| {
            let index = glyphs[i].0;
            !inside(index)
                && (chips.iter().any(|r| r.start == index || r.end == index)
                    || line.text[..index]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace))
        });
        let at = preferred
            .or_else(|| (row + 1..=overflow).rev().find(|&i| !inside(glyphs[i].0)))
            .unwrap_or(overflow.max(row + 1));
        if at >= glyphs.len() {
            break;
        }
        boundaries.push(glyphs[at].2);
        row = at;
    }
    let layout = gpui::WrappedLineLayout {
        unwrapped_layout: line.unwrapped_layout.clone(),
        wrap_boundaries: boundaries,
        wrap_width: line.wrap_width,
    };
    *std::ops::DerefMut::deref_mut(line) = std::sync::Arc::new(layout);
}

/// Direction of the last edit — a run only merges with edits of its own kind.
#[derive(Clone, Copy, PartialEq)]
enum EditKind {
    Insert,
    Delete,
}

const GENERIC_COMPOSER_CONTEXT: &str = "Composer";
pub(crate) const MESSAGE_COMPOSER_CONTEXT: &str = "MessageComposer";
const PALETTE_SEARCH_CONTEXT: &str = "PaletteSearch";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageEnterBindingAction {
    Submit,
    ModifiedSubmit,
    NewlineOrAccept,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MessageEnterBinding {
    keystroke: String,
    action: MessageEnterBindingAction,
}

fn message_enter_bindings(
    behavior: ComposerSendBehavior,
    modifier_combo: &str,
) -> Vec<MessageEnterBinding> {
    match behavior {
        ComposerSendBehavior::Enter => vec![
            MessageEnterBinding {
                keystroke: "enter".into(),
                action: MessageEnterBindingAction::Submit,
            },
            MessageEnterBinding {
                keystroke: modifier_combo.into(),
                action: MessageEnterBindingAction::ModifiedSubmit,
            },
        ],
        ComposerSendBehavior::ModEnter => vec![
            MessageEnterBinding {
                keystroke: "enter".into(),
                action: MessageEnterBindingAction::NewlineOrAccept,
            },
            MessageEnterBinding {
                keystroke: modifier_combo.into(),
                action: MessageEnterBindingAction::ModifiedSubmit,
            },
        ],
    }
}

fn message_input_context(wizard_active: bool) -> &'static str {
    if wizard_active {
        GENERIC_COMPOSER_CONTEXT
    } else {
        MESSAGE_COMPOSER_CONTEXT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnterOutcome {
    AcceptCompletion,
    Submit,
    Newline,
}

fn enter_outcome(has_completion: bool, fallback: EnterOutcome) -> EnterOutcome {
    if has_completion {
        EnterOutcome::AcceptCompletion
    } else {
        fallback
    }
}

fn input_bindings(context: &'static str) -> Vec<KeyBinding> {
    let ctx = Some(context);
    let mut bindings = vec![
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("shift-backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
        // macOS line/document motion — a laptop keyboard has no home/end keys,
        // so Cmd+arrow is the only way users reach either edge.
        KeyBinding::new("cmd-left", Home, ctx),
        KeyBinding::new("cmd-right", End, ctx),
        KeyBinding::new("cmd-up", DocStart, ctx),
        KeyBinding::new("cmd-down", DocEnd, ctx),
        KeyBinding::new("shift-cmd-left", SelectHome, ctx),
        KeyBinding::new("shift-cmd-right", SelectEnd, ctx),
        KeyBinding::new("shift-cmd-up", SelectDocStart, ctx),
        KeyBinding::new("shift-cmd-down", SelectDocEnd, ctx),
        // Line-edge deletion (Cmd+Delete on macOS).
        KeyBinding::new("cmd-backspace", DeleteToLineStart, ctx),
        KeyBinding::new("cmd-delete", DeleteToLineEnd, ctx),
    ];
    for prefix in ["cmd", "ctrl"] {
        bindings.push(KeyBinding::new(&format!("{prefix}-z"), Undo, ctx));
        bindings.push(KeyBinding::new(&format!("shift-{prefix}-z"), Redo, ctx));
    }
    // Word-level editing: Option on macOS, Ctrl on Windows/Linux.
    let word_edit_prefix = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    bindings.push(KeyBinding::new(
        &format!("{word_edit_prefix}-backspace"),
        DeleteWordLeft,
        ctx,
    ));
    bindings.push(KeyBinding::new(
        &format!("{word_edit_prefix}-delete"),
        DeleteWordRight,
        ctx,
    ));
    bindings.push(KeyBinding::new(
        &format!("{word_edit_prefix}-left"),
        WordLeft,
        ctx,
    ));
    bindings.push(KeyBinding::new(
        &format!("{word_edit_prefix}-right"),
        WordRight,
        ctx,
    ));
    bindings.push(KeyBinding::new(
        &format!("shift-{word_edit_prefix}-left"),
        SelectWordLeft,
        ctx,
    ));
    bindings.push(KeyBinding::new(
        &format!("shift-{word_edit_prefix}-right"),
        SelectWordRight,
        ctx,
    ));
    for prefix in ["cmd", "ctrl"] {
        bindings.push(KeyBinding::new(&format!("{prefix}-a"), SelectAll, ctx));
        bindings.push(KeyBinding::new(&format!("{prefix}-c"), Copy, ctx));
        bindings.push(KeyBinding::new(&format!("{prefix}-x"), Cut, ctx));
        bindings.push(KeyBinding::new(&format!("{prefix}-v"), Paste, ctx));
    }
    if !matches!(context, "AgentLabInput" | "AgentLabSingleLine") {
        bindings.push(KeyBinding::new("tab", MentionTab, ctx));
        bindings.push(KeyBinding::new("shift-tab", OutdentList, ctx));
    }
    bindings
}

/// Bind the composer keymap. Call once at app boot.
pub fn init(cx: &mut App, send_behavior: ComposerSendBehavior) {
    let mut fields = input_bindings("AgentLabInput");
    fields.push(KeyBinding::new("enter", Newline, Some("AgentLabInput")));
    cx.bind_keys(fields);
    let mut fields = input_bindings("AgentLabSingleLine");
    fields.push(KeyBinding::new("enter", Submit, Some("AgentLabSingleLine")));
    cx.bind_keys(fields);
    let mut generic_bindings = input_bindings(GENERIC_COMPOSER_CONTEXT);
    generic_bindings.push(KeyBinding::new(
        "enter",
        Submit,
        Some(GENERIC_COMPOSER_CONTEXT),
    ));

    let mut message_bindings = input_bindings(MESSAGE_COMPOSER_CONTEXT);
    for binding in message_enter_bindings(send_behavior, &platform_combo("mod-enter")) {
        match binding.action {
            MessageEnterBindingAction::Submit => message_bindings.push(KeyBinding::new(
                &binding.keystroke,
                Submit,
                Some(MESSAGE_COMPOSER_CONTEXT),
            )),
            MessageEnterBindingAction::ModifiedSubmit => message_bindings.push(KeyBinding::new(
                &binding.keystroke,
                ModifiedSubmit,
                Some(MESSAGE_COMPOSER_CONTEXT),
            )),
            MessageEnterBindingAction::NewlineOrAccept => message_bindings.push(KeyBinding::new(
                &binding.keystroke,
                MessageNewlineOrAccept,
                Some(MESSAGE_COMPOSER_CONTEXT),
            )),
        }
    }

    let word_edit_prefix = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    // Palette-search context: TEXT-EDITING keys only. gpui dispatches matched
    // keybindings BEFORE raw key listeners (window.rs `dispatch_key_event`),
    // so anything bound here can never reach a palette's `on_key_down` —
    // navigation keys (up/down/left/right/enter) are deliberately unbound and
    // bubble to the palette frame instead.
    let palette = Some(PALETTE_SEARCH_CONTEXT);
    let mut palette_bindings = vec![
        KeyBinding::new("backspace", Backspace, palette),
        KeyBinding::new("shift-backspace", Backspace, palette),
        KeyBinding::new("delete", Delete, palette),
        KeyBinding::new("home", Home, palette),
        KeyBinding::new("end", End, palette),
        KeyBinding::new("shift-left", SelectLeft, palette),
        KeyBinding::new("shift-right", SelectRight, palette),
        // Modifier-qualified motion is safe here: the palette's own navigation
        // uses BARE arrows/enter, which stay unbound and bubble to its frame.
        KeyBinding::new("cmd-left", Home, palette),
        KeyBinding::new("cmd-right", End, palette),
        KeyBinding::new("shift-cmd-left", SelectHome, palette),
        KeyBinding::new("shift-cmd-right", SelectEnd, palette),
        KeyBinding::new("cmd-backspace", DeleteToLineStart, palette),
    ];
    palette_bindings.push(KeyBinding::new(
        &format!("{word_edit_prefix}-backspace"),
        DeleteWordLeft,
        palette,
    ));
    palette_bindings.push(KeyBinding::new(
        &format!("{word_edit_prefix}-delete"),
        DeleteWordRight,
        palette,
    ));
    palette_bindings.push(KeyBinding::new(
        &format!("{word_edit_prefix}-left"),
        WordLeft,
        palette,
    ));
    palette_bindings.push(KeyBinding::new(
        &format!("{word_edit_prefix}-right"),
        WordRight,
        palette,
    ));
    palette_bindings.push(KeyBinding::new(
        &format!("shift-{word_edit_prefix}-left"),
        SelectWordLeft,
        palette,
    ));
    palette_bindings.push(KeyBinding::new(
        &format!("shift-{word_edit_prefix}-right"),
        SelectWordRight,
        palette,
    ));
    for prefix in ["cmd", "ctrl"] {
        palette_bindings.push(KeyBinding::new(&format!("{prefix}-a"), SelectAll, palette));
        palette_bindings.push(KeyBinding::new(&format!("{prefix}-c"), Copy, palette));
        palette_bindings.push(KeyBinding::new(&format!("{prefix}-x"), Cut, palette));
        palette_bindings.push(KeyBinding::new(&format!("{prefix}-v"), Paste, palette));
        palette_bindings.push(KeyBinding::new(&format!("{prefix}-z"), Undo, palette));
        palette_bindings.push(KeyBinding::new(&format!("shift-{prefix}-z"), Redo, palette));
    }
    cx.bind_keys(palette_bindings);
    cx.bind_keys(generic_bindings);
    cx.bind_keys(message_bindings);
}

/// Events the composer wrapper listens for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerInputEvent {
    Submitted,
    ModifiedSubmitted,
    Edited,
    CursorMoved,
    ViewportChanged,
    MentionNavigate(isize),
    MentionAccept,
    MentionDismiss,
    /// Images pasted from the clipboard (screenshots / copied image data) —
    /// the wrapper stages them as attachments (use-attachments.ts onPaste).
    PastedImages(Vec<gpui::Image>),
    /// File paths pasted from the clipboard (a file manager "Copy").
    PastedPaths(Vec<PathBuf>),
    PastedText {
        range: Range<usize>,
        revision: u64,
    },
    /// An image chip was clicked: open its picture full size.
    OpenAttachment(u32),
}

/// Shaping inputs excluding mutable viewport and selection geometry.
#[derive(Clone, PartialEq)]
struct InputLayoutKey {
    width: Pixels,
    font: gpui::Font,
    font_size: Pixels,
    color: gpui::Hsla,
    chip_family: SharedString,
    chip_color: gpui::Hsla,
    syntax: crate::theme::SyntaxPalette,
    marked_range: Option<Range<usize>>,
    placeholder: SharedString,
    mentions_enabled: bool,
    active_line: Option<Range<usize>>,
}

/// A soft-wrap boundary is both the previous row's end and the next row's
/// start. Keep the visual side of the caret separately from its source offset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CaretAffinity {
    Upstream,
    #[default]
    Downstream,
}

/// Multiline input entity: content + selection + IME marked text + measured
/// layout (wrapped lines) for mouse mapping and auto-grow.
pub struct ComposerInput {
    /// Key context for the binding map ("Composer", or "PaletteSearch" for
    /// palette filters whose navigation keys must bubble).
    key_context: &'static str,
    /// Cmd+C with no input selection copies the transcript selection. Only
    /// the message composer keeps focus while the user reads the transcript;
    /// dialog and palette fields copy only their own selection. A flag rather
    /// than `key_context`, which the question wizard swaps while it borrows
    /// the message input.
    copies_transcript_selection: bool,
    accessibility_role: Role,
    focus_handle: FocusHandle,
    content: String,
    edit_revision: u64,
    pub(crate) read_only: bool,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    caret_affinity: CaretAffinity,
    marked_range: Option<Range<usize>>,
    /// Retained through vertical moves across short rows; other moves reset it.
    preferred_column: Option<Pixels>,
    is_selecting: bool,
    drag_position: Option<Point<Pixels>>,
    drag_generation: u64,
    drag_unit: Option<(PressIntent, Range<usize>)>,
    drag_autoscroll_active: bool,
    /// Vertical scroll inside the input once content exceeds the max height.
    scroll_top: f32,
    /// Visible content budget supplied by the animated composer.
    viewport_height: Option<f32>,
    /// Final content budget, excluding temporary overflow during a resize.
    settled_viewport_height: Option<f32>,
    resizing: bool,
    overflow_top_padding: f32,
    needs_measure: bool,
    last_layout_key: Option<InputLayoutKey>,
    last_notified_layout: Option<(Pixels, f32)>,
    max_ascent: f32,
    #[cfg(test)]
    layout_rebuilds: usize,
    /// The runs of the last rebuilt layout; the test text system shapes
    /// every run alike, so tests check the requested fonts here.
    #[cfg(test)]
    last_runs: Vec<TextRun>,
    /// Normally keeps the caret visible through edits and rewraps. Manual
    /// wheel scrolling pauses it until the next caret move or edit.
    follow_cursor: bool,
    text_size: f32,
    configured_line_height: f32,
    single_line: bool,
    scroll_left: f32,
    // -- measured state (written during layout/paint) --
    last_lines: Vec<WrappedLine>,
    line_starts: Vec<usize>,
    line_indents: Vec<Pixels>,
    last_bounds: Option<Bounds<Pixels>>,
    line_height: Pixels,
    content_height: f32,
    max_line_width: f32,
    last_width: f32,
    /// Raw Markdown → chip display projection from the last layout pass.
    projection: TextProjection,
    syntax_source: String,
    syntax_spans: Vec<openagents_syntax::HighlightSpan>,
    syntax_task: Option<Task<()>>,
    /// Inline completion preview: painted in faint ink after the text while
    /// the caret sits at the end (palette tab-completion). Owned by the
    /// wrapper — it recomputes and re-sets this on every render pass, so the
    /// input never has to know what the completion means.
    ghost: Option<SharedString>,
    /// File mentions are a composer feature, not a behavior of generic inputs
    /// (picker searches and rename fields also use this type).
    mentions_enabled: bool,
    /// Bumped once per `layout_text` pass — the flip logic uses it to apply at
    /// most one compact↔expanded flip per layout (a flip is only re-evaluated
    /// after the input has been measured in the new mode).
    layout_epoch: u64,
    display_is_placeholder: bool,
    /// Caret blink anchor: reset on every keystroke/caret move so the caret is
    /// solid while typing and blinks at [`CARET_BLINK_MS`] when idle.
    blink_anchor: Instant,
    /// Half-period repaint driver, alive only while the input is focused.
    blink_task: Option<Task<()>>,
    platform: input_host::State,
    // -- undo history --
    undo_stack: Vec<EditSnapshot>,
    redo_stack: Vec<EditSnapshot>,
    /// Kind, trailing offset, and time of the last edit — the merge test that
    /// decides whether the next edit extends the current undo step.
    last_edit: Option<(EditKind, usize, Instant)>,
    /// The wrapper owns mention state; this only redirects bound keys while a
    /// mention token is active, keeping input focus and native text editing.
    mention_open: bool,
    mention_has_selection: bool,
    /// Last prepainted chip bounds; the paint-phase pointer listener uses
    /// these instead of attempting to infer text geometry from the cursor.
    mention_hits: Vec<MentionHit>,
    /// The image chip a click started on, and where.
    chip_press: Option<(u32, Point<Pixels>)>,
    /// The attachments the draft holds, by chip number, with each image's
    /// picture. `None` outside the composer, where every attachment link is
    /// shown as a chip.
    attachment_chips: Option<HashMap<u32, ChipAttachment>>,
    /// The draft the attachment chips belong to; copied chips carry it so a paste
    /// into another draft can tell it must not reuse their numbers.
    attachment_scope: String,
    mention_tooltip: MentionTooltipPhase,
    mention_tooltip_generation: u64,
    mention_tooltip_popup: Option<Bounds<Pixels>>,
    mention_tooltip_task: Option<Task<()>>,
    /// Created once when Waiting promotes; retaining this entity preserves
    /// GPUI's global animation state across prepaint frames.
    mention_tooltip_view: Option<Entity<MentionPathTooltip>>,
}

impl ComposerInput {
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self::with_context(placeholder, GENERIC_COMPOSER_CONTEXT, cx)
    }

    /// An input in a custom KEY context — palettes use `"PaletteSearch"`,
    /// whose keymap binds only text-editing keys so navigation keys bubble to
    /// the surrounding frame (see `init`).
    pub fn with_context(
        placeholder: impl Into<SharedString>,
        key_context: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            key_context,
            copies_transcript_selection: false,
            accessibility_role: Role::MultilineTextInput,
            focus_handle: cx.focus_handle(),
            content: String::new(),
            edit_revision: 0,
            read_only: false,
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            caret_affinity: CaretAffinity::Downstream,
            marked_range: None,
            preferred_column: None,
            is_selecting: false,
            drag_position: None,
            drag_generation: 0,
            drag_unit: None,
            drag_autoscroll_active: false,
            scroll_top: 0.0,
            viewport_height: None,
            settled_viewport_height: None,
            resizing: false,
            overflow_top_padding: 0.0,
            needs_measure: true,
            last_layout_key: None,
            last_notified_layout: None,
            max_ascent: INPUT_TEXT_SIZE,
            #[cfg(test)]
            layout_rebuilds: 0,
            #[cfg(test)]
            last_runs: Vec::new(),
            follow_cursor: true,
            text_size: INPUT_TEXT_SIZE,
            configured_line_height: INPUT_LINE_HEIGHT,
            single_line: false,
            scroll_left: 0.0,
            last_lines: Vec::new(),
            line_starts: vec![0],
            line_indents: Vec::new(),
            last_bounds: None,
            line_height: px(INPUT_LINE_HEIGHT),
            content_height: INPUT_LINE_HEIGHT,
            max_line_width: 0.0,
            last_width: 0.0,
            projection: TextProjection::default(),
            syntax_source: String::new(),
            syntax_spans: Vec::new(),
            syntax_task: None,
            ghost: None,
            mentions_enabled: false,
            layout_epoch: 0,
            display_is_placeholder: true,
            blink_anchor: Instant::now(),
            blink_task: None,
            platform: Default::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit: None,
            mention_open: false,
            mention_has_selection: false,
            mention_hits: Vec::new(),
            chip_press: None,
            attachment_chips: None,
            attachment_scope: String::new(),
            mention_tooltip: MentionTooltipPhase::Hidden,
            mention_tooltip_generation: 0,
            mention_tooltip_popup: None,
            mention_tooltip_task: None,
            mention_tooltip_view: None,
        }
    }

    /// Override the text metrics for compact one-line surfaces such as
    /// toolbar searches without changing the main composer typography.
    pub fn with_text_metrics(mut self, text_size: f32, line_height: f32) -> Self {
        self.text_size = text_size;
        self.configured_line_height = line_height;
        self.line_height = px(line_height);
        self.content_height = line_height;
        self
    }

    /// Keep compact fields on one row and reveal the caret horizontally.
    /// Bound a standalone multiline composer while preserving caret scrolling.
    pub(crate) fn with_viewport_height(mut self, height: f32) -> Self {
        let height = height.max(self.configured_line_height);
        self.viewport_height = Some(height);
        self.settled_viewport_height = Some(height);
        self
    }

    pub fn with_single_line(mut self) -> Self {
        self.single_line = true;
        self
    }

    /// Join the window's Tab order, for a field the keyboard must reach.
    pub(crate) fn with_tab_stop(mut self) -> Self {
        self.focus_handle = self.focus_handle.tab_stop(true);
        self
    }

    fn set_key_context(&mut self, key_context: &'static str, cx: &mut Context<Self>) {
        if self.key_context != key_context {
            self.key_context = key_context;
            cx.notify();
        }
    }

    pub fn with_accessibility_role(mut self, role: Role) -> Self {
        self.accessibility_role = role;
        self
    }

    /// Reset the caret blink phase (solid again) — called on every edit and
    /// caret move, matching textarea behavior.
    fn reset_blink(&mut self) {
        self.blink_anchor = Instant::now();
    }

    /// Caret paint gate: focused input in an active window, in the "on" blink
    /// phase. Also (re)arms the half-period repaint driver while focused, and
    /// drops it on blur so an unfocused input schedules no frames.
    fn caret_shown(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let focused = self.focus_handle.is_focused(window);
        if !focused || !window.is_window_active() {
            self.blink_task = None;
            return false;
        }
        if self.blink_task.is_none() {
            self.blink_task = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(CARET_BLINK_MS))
                        .await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            }));
        }
        caret_visible(self.blink_anchor.elapsed().as_millis() as u64)
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn set_mention_controls(
        &mut self,
        open: bool,
        has_selection: bool,
        cx: &mut Context<Self>,
    ) {
        if self.mention_open == open && self.mention_has_selection == has_selection {
            return;
        }
        self.mention_open = open;
        self.mention_has_selection = has_selection;
        cx.notify();
    }

    fn enable_mentions(&mut self) {
        self.mentions_enabled = true;
        self.refresh_projection();
    }

    fn editing_source_range(&self) -> Range<usize> {
        let selected = self.selected_range.clone();
        let range = self
            .marked_range
            .as_ref()
            .map_or(selected.clone(), |marked| {
                selected.start.min(marked.start)..selected.end.max(marked.end)
            });
        self.line_range_at(range.start).start..self.line_range_at(range.end).end
    }

    fn refresh_projection(&mut self) {
        self.projection = if self.mentions_enabled {
            let live: Option<HashSet<u32>> = self
                .attachment_chips
                .as_ref()
                .map(|images| images.keys().copied().collect());
            TextProjection::rich(
                &self.content,
                Some(self.editing_source_range()),
                live.as_ref(),
            )
        } else {
            TextProjection {
                display: self.content.clone(),
                mentions: Vec::new(),
                mappings: Vec::new(),
            }
        };
    }

    /// Replace a completed `@query` token as one non-coalescing undo step.
    pub fn replace_mention(
        &mut self,
        range: Range<usize>,
        path: &str,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        if !local_path_is_safe(path) {
            return;
        }
        self.replace_mention_link(range, &local_file_link(path, is_dir), cx);
    }

    /// Replace a completed `@query` token with a canonical reference link.
    pub(crate) fn replace_mention_link(
        &mut self,
        range: Range<usize>,
        link: &str,
        cx: &mut Context<Self>,
    ) {
        if self.read_only || self.marked_range.is_some() {
            return;
        }
        self.invalidate_mention_tooltip();
        let (trailing, advance) = reference_suffix(self.content[range.end..].chars().next());
        let inserted = format!("{link}{trailing}");
        self.record_edit(&range, &inserted);
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.content =
            self.content[..range.start].to_owned() + &inserted + &self.content[range.end..];
        let cursor = range.start + inserted.len() + advance;
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
        self.refresh_projection();
        self.follow_cursor = true;
        self.reset_blink();
        self.needs_measure = true;
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
    }

    /// Insert a workspace reference at the current selection. Drag-and-drop
    /// uses the same strict local Markdown transport and projected chip as an
    /// `@` mention selected from completion.
    fn insert_dropped_mention(&mut self, path: &str, is_dir: bool, cx: &mut Context<Self>) -> bool {
        if !local_path_is_safe(path) {
            return false;
        }
        self.insert_reference(&local_file_link(path, is_dir), cx)
    }

    /// Delete every chip that mentions attachment `index`, along with one separator
    /// when the chip sat between spaces. Returns whether anything was removed.
    pub(crate) fn remove_attachment_chips(&mut self, index: u32, cx: &mut Context<Self>) -> bool {
        if self.read_only || self.marked_range.is_some() {
            return false;
        }
        let ranges: Vec<Range<usize>> =
            openagents_proto::attachment_mentions::attachment_mentions(&self.content)
                .into_iter()
                .filter(|mention| mention.index == index)
                .map(|mention| mention.range)
                .collect();
        if ranges.is_empty() {
            return false;
        }
        self.invalidate_mention_tooltip();
        for mut range in ranges.into_iter().rev() {
            let separated =
                range.start == 0 || self.content[..range.start].ends_with(char::is_whitespace);
            if separated && self.content[range.end..].starts_with(' ') {
                range.end += 1;
            }
            self.record_edit(&range, "");
            self.edit_revision = self.edit_revision.wrapping_add(1);
            self.content.replace_range(range.clone(), "");
            let shift = |offset: usize| {
                if offset >= range.end {
                    offset - range.len()
                } else {
                    offset.min(range.start)
                }
            };
            self.selected_range = shift(self.selected_range.start)..shift(self.selected_range.end);
        }
        self.last_edit = None;
        self.selection_reversed = false;
        self.preferred_column = None;
        self.refresh_projection();
        self.reset_blink();
        self.needs_measure = true;
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
        true
    }

    /// Tell the editor which staged attachments its chips may reference (`None`
    /// for one that is not an image). Chips for any other number show as plain
    /// text.
    pub(crate) fn set_attachment_chips(
        &mut self,
        scope: &str,
        attachments: HashMap<u32, Option<std::sync::Arc<gpui::Image>>>,
        cx: &mut Context<Self>,
    ) {
        let unchanged = self.attachment_scope == scope
            && self.attachment_chips.as_ref().is_some_and(|current| {
                current.len() == attachments.len()
                    && attachments.iter().all(|(index, image)| {
                        current.get(index).is_some_and(|known| {
                            known.image.as_ref().map(std::sync::Arc::as_ptr)
                                == image.as_ref().map(std::sync::Arc::as_ptr)
                        })
                    })
            });
        if unchanged {
            return;
        }
        self.attachment_scope = scope.to_string();
        self.attachment_chips = Some(
            attachments
                .into_iter()
                .map(|(index, image)| (index, ChipAttachment { image }))
                .collect(),
        );
        self.invalidate_mention_tooltip();
        self.refresh_projection();
        self.needs_measure = true;
        cx.notify();
    }

    /// Insert a canonical reference link at the selection, as one undo step.
    pub(crate) fn insert_reference(&mut self, link: &str, cx: &mut Context<Self>) -> bool {
        // The platform still owns the marked range during IME composition.
        // Inserting a chip into it would leave those offsets pointing inside
        // the new reference, and the next IME update could delete the chip.
        if self.read_only || self.marked_range.is_some() {
            return false;
        }
        let range = self.selected_range.clone();
        let Some((inserted, cursor_advance)) =
            dropped_reference(&self.content, range.clone(), link)
        else {
            return false;
        };
        self.invalidate_mention_tooltip();
        self.record_edit(&range, &inserted);
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.content =
            self.content[..range.start].to_owned() + &inserted + &self.content[range.end..];
        let cursor = range.start + cursor_advance;
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
        self.refresh_projection();
        self.follow_cursor = true;
        self.reset_blink();
        self.needs_measure = true;
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
        true
    }

    /// Replace a completed plain-text token (slash commands) as one
    /// non-coalescing undo step. Unlike [`Self::replace_mention`], the
    /// replacement is ordinary text — no link, no chip projection.
    pub fn replace_plain_token(
        &mut self,
        range: Range<usize>,
        replacement: &str,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let (trailing, advance) = reference_suffix(self.content[range.end..].chars().next());
        let inserted = format!("{replacement}{trailing}");
        self.record_edit(&range, &inserted);
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.content =
            self.content[..range.start].to_owned() + &inserted + &self.content[range.end..];
        let cursor = range.start + inserted.len() + advance;
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
        self.refresh_projection();
        self.follow_cursor = true;
        self.reset_blink();
        self.needs_measure = true;
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
    }

    fn remove_completion_token(&mut self, mut range: Range<usize>, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.content[..range.start].trim().is_empty()
            && self.content[range.end..].trim().is_empty()
        {
            range = 0..self.content.len();
        } else if (range.start == 0 || self.content[..range.start].ends_with(' '))
            && self.content[range.end..].starts_with(' ')
        {
            range.end += 1;
        }
        self.last_edit = None;
        self.record_edit(&range, "");
        self.last_edit = None;
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.content.replace_range(range.clone(), "");
        self.selected_range = range.start..range.start;
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
        self.refresh_projection();
        self.follow_cursor = true;
        self.needs_measure = true;
        self.reset_blink();
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Set (or clear) the inline completion preview. Only paints while the
    /// caret sits at the end of a non-empty draft — see the prepaint gate.
    pub fn set_ghost(&mut self, ghost: Option<SharedString>, cx: &mut Context<Self>) {
        if self.ghost == ghost {
            return;
        }
        self.ghost = ghost;
        cx.notify();
    }

    pub fn has_newline(&self) -> bool {
        self.content.contains('\n')
    }

    /// Unwrapped width of the widest line — feeds the compact/expanded flip.
    pub fn measured_text_width(&self) -> f32 {
        self.max_line_width
    }

    pub fn measured_content_height(&self) -> f32 {
        self.content_height
    }

    pub fn placeholder(&self) -> &SharedString {
        &self.placeholder
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.cancel_dictation();
        self.invalidate_mention_tooltip();
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.content = text.into();
        if self.single_line {
            self.content = self.content.replace(['\r', '\n'], " ");
        }
        let end = self.content.len();
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
        self.marked_range = None;
        self.scroll_top = 0.0;
        self.scroll_left = 0.0;
        self.refresh_projection();
        self.follow_cursor = true;
        // Programmatic replacement (draft load, clear-on-submit) is a new
        // document, not an edit — undo must not reach back past it.
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_edit = None;
        self.reset_blink();
        self.needs_measure = true;
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
    }

    fn invalidate_mention_tooltip(&mut self) {
        self.mention_tooltip_generation = self.mention_tooltip_generation.wrapping_add(1);
        self.mention_tooltip = MentionTooltipPhase::Hidden;
        self.mention_tooltip_popup = None;
        self.mention_tooltip_task = None;
        self.mention_tooltip_view = None;
    }

    fn set_mention_hits(&mut self, hits: Vec<MentionHit>) {
        self.mention_hits = hits;
        let live = self
            .mention_tooltip
            .target()
            .is_none_or(|target| self.mention_hits.iter().any(|hit| &hit.target == target));
        if !live {
            self.invalidate_mention_tooltip();
        }
    }

    fn start_mention_tooltip_wait(&mut self, target: MentionTooltipTarget, cx: &mut Context<Self>) {
        self.mention_tooltip_generation = self.mention_tooltip_generation.wrapping_add(1);
        let generation = self.mention_tooltip_generation;
        self.mention_tooltip = MentionTooltipPhase::Waiting { target, generation };
        self.mention_tooltip_popup = None;
        self.mention_tooltip_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(MENTION_TOOLTIP_DELAY).await;
            this.update(cx, |input, cx| {
                let live = input.mention_tooltip.target().is_some_and(|target| {
                    input.mention_hits.iter().any(|hit| &hit.target == target)
                });
                let next = mention_tooltip_promote(input.mention_tooltip.clone(), generation, live);
                if next != input.mention_tooltip {
                    input.mention_tooltip = next;
                    input.mention_tooltip_task = None;
                    if let MentionTooltipPhase::Visible { target, generation } =
                        &input.mention_tooltip
                    {
                        input.mention_tooltip_view = Some(cx.new(|_| MentionPathTooltip {
                            path: target.path.clone(),
                            activation: *generation,
                        }));
                    }
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    fn is_image_chip(&self, target: &MentionTooltipTarget) -> bool {
        target
            .attachment
            .and_then(|index| self.attachment_chips.as_ref()?.get(&index))
            .is_some_and(|chip| chip.image.is_some())
    }

    /// The number of the image chip under `position`, if any.
    fn image_chip_at(&self, position: Point<Pixels>) -> Option<u32> {
        self.mention_hits
            .iter()
            .find(|hit| hit.bounds.contains(&position))
            .filter(|hit| self.is_image_chip(&hit.target))
            .and_then(|hit| hit.target.attachment)
    }

    fn on_mention_pointer_move(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.invalidate_mention_tooltip();
            return;
        }
        let target = self
            .mention_hits
            .iter()
            .find(|hit| hit.bounds.contains(&position))
            .filter(|hit| !self.is_image_chip(&hit.target))
            .map(|hit| hit.target.clone());
        let in_popup = self
            .mention_tooltip_popup
            .is_some_and(|popup| popup.contains(&position));
        let next_generation = self.mention_tooltip_generation.wrapping_add(1);
        let next = mention_tooltip_reduce(
            self.mention_tooltip.clone(),
            target.clone(),
            in_popup,
            next_generation,
        );
        if next == self.mention_tooltip {
            return;
        }
        match next {
            MentionTooltipPhase::Waiting { target, .. } => {
                self.start_mention_tooltip_wait(target, cx)
            }
            _ => {
                self.invalidate_mention_tooltip();
                self.mention_tooltip = next;
                cx.notify();
            }
        }
    }

    fn visible_mention_tooltip(
        &self,
    ) -> Option<(
        MentionTooltipTarget,
        Point<Pixels>,
        u64,
        Entity<MentionPathTooltip>,
    )> {
        let MentionTooltipPhase::Visible { target, generation } = &self.mention_tooltip else {
            return None;
        };
        self.mention_hits
            .iter()
            .find(|hit| hit.target == *target)
            .and_then(|hit| {
                let view = self.mention_tooltip_view.clone()?;
                Some((target.clone(), hit.anchor, *generation, view))
            })
    }

    fn check_mention_tooltip_visibility(
        &mut self,
        popup: Bounds<Pixels>,
        pointer: Point<Pixels>,
    ) -> bool {
        let Some((target, _, _, _)) = self.visible_mention_tooltip() else {
            return false;
        };
        let in_chip = self
            .mention_hits
            .iter()
            .any(|hit| hit.target == target && hit.bounds.contains(&pointer));
        if mention_tooltip_contains(in_chip, popup.contains(&pointer)) {
            self.mention_tooltip_popup = Some(popup);
            true
        } else {
            self.invalidate_mention_tooltip();
            false
        }
    }

    // ---- undo history ----

    fn snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            content: self.content.clone(),
            selected_range: self.selected_range.clone(),
            selection_reversed: self.selection_reversed,
            caret_affinity: self.caret_affinity,
        }
    }

    /// Called with the range about to be replaced, BEFORE the content changes,
    /// so the pushed snapshot is the pre-edit state.
    fn record_edit(&mut self, range: &Range<usize>, new_text: &str) {
        input_host::note_input();
        self.cancel_dictation();
        let kind = if new_text.is_empty() {
            EditKind::Delete
        } else {
            EditKind::Insert
        };
        // A run merges only while it stays single-character, contiguous with
        // the previous edit, of the same kind, and inside the idle window. A
        // pause, a word break, a paste, or a caret jump all break the run so
        // undo lands on a boundary the user recognizes.
        let coalescible = match kind {
            EditKind::Insert => range.is_empty() && new_text.chars().count() == 1,
            EditKind::Delete => self
                .content
                .get(range.clone())
                .is_some_and(|text| text.graphemes(true).count() == 1),
        };
        let mergeable = coalescible
            && match (kind, &self.last_edit) {
                (EditKind::Insert, Some((EditKind::Insert, at, when))) => {
                    range.is_empty()
                        && range.start == *at
                        && new_text.chars().count() == 1
                        && !new_text.starts_with(['\n', ' ', '\t'])
                        && when.elapsed() < UNDO_COALESCE
                }
                (EditKind::Delete, Some((EditKind::Delete, at, when))) => {
                    range.end == *at && when.elapsed() < UNDO_COALESCE
                }
                _ => false,
            };
        if !mergeable {
            self.undo_stack.push(self.snapshot());
            if self.undo_stack.len() > UNDO_LIMIT {
                self.undo_stack.remove(0);
            }
        }
        // Any fresh edit invalidates the redo branch.
        self.redo_stack.clear();
        let tail = match kind {
            EditKind::Insert => range.start + new_text.len(),
            EditKind::Delete => range.start,
        };
        self.last_edit = coalescible.then(|| (kind, tail, Instant::now()));
    }

    fn restore(&mut self, snapshot: EditSnapshot, cx: &mut Context<Self>) {
        self.cancel_dictation();
        self.invalidate_mention_tooltip();
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.content = snapshot.content;
        self.selected_range = snapshot.selected_range;
        self.selection_reversed = snapshot.selection_reversed;
        self.caret_affinity = snapshot.caret_affinity;
        self.preferred_column = None;
        self.marked_range = None;
        self.refresh_projection();
        self.follow_cursor = true;
        // Never merge a subsequent edit into a step that undo just crossed.
        self.last_edit = None;
        self.reset_blink();
        self.needs_measure = true;
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.dictation_active() {
            self.dictation_changed(cx);
            cx.notify();
        }
        self.cancel_dictation();
        if self.read_only {
            return;
        }
        let Some(previous) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(self.snapshot());
        self.restore(previous, cx);
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.dictation_active() {
            self.dictation_changed(cx);
            cx.notify();
        }
        self.cancel_dictation();
        if self.read_only {
            return;
        }
        let Some(next) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(self.snapshot());
        self.restore(next, cx);
    }

    // ---- editing ops ----

    pub fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.cancel_dictation();
        self.last_edit = None;
        let offset = self.projection.normalize_range(offset..offset).start;
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
        self.follow_cursor = true;
        self.reset_blink();
        cx.emit(ComposerInputEvent::CursorMoved);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.cancel_dictation();
        self.last_edit = None;
        self.extend_selection(offset, cx);
    }

    // Deletion extends the selection internally without breaking a typing run.
    fn extend_selection(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
        let offset = self.projection.normalize_range(offset..offset).start;
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.follow_cursor = true;
        self.reset_blink();
        cx.emit(ComposerInputEvent::CursorMoved);
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        if let Some(boundary) = self.projection.previous_boundary(offset) {
            return boundary;
        }
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(ix, _)| (ix < offset).then_some(ix))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        if let Some(boundary) = self.projection.next_boundary(offset) {
            return boundary;
        }
        self.content
            .grapheme_indices(true)
            .find_map(|(ix, _)| (ix > offset).then_some(ix))
            .unwrap_or(self.content.len())
    }

    fn previous_word_boundary(&self, offset: usize) -> usize {
        if let Some(boundary) = self.projection.previous_boundary(offset) {
            return boundary;
        }
        self.content
            .split_word_bound_indices()
            .rev()
            .find_map(|(ix, word)| (ix < offset && !word.trim().is_empty()).then_some(ix))
            .unwrap_or(0)
    }

    fn next_word_boundary(&self, offset: usize) -> usize {
        if let Some(boundary) = self.projection.next_boundary(offset) {
            return boundary;
        }
        self.content
            .split_word_bound_indices()
            .find_map(|(ix, word)| {
                let end = ix + word.len();
                (end > offset && !word.trim().is_empty()).then_some(end)
            })
            .unwrap_or(self.content.len())
    }

    /// Byte range of the logical line containing `offset`.
    fn line_range_at(&self, offset: usize) -> Range<usize> {
        let mut offset = offset.min(self.content.len());
        while !self.content.is_char_boundary(offset) {
            offset -= 1;
        }
        let start = self.content[..offset]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let end = self.content[offset..]
            .find('\n')
            .map(|i| offset + i)
            .unwrap_or(self.content.len());
        start..end
    }

    /// Navigation stops before the whole line ending. The raw line range
    /// intentionally retains CR for full-line selection and block edits.
    fn line_content_end_at(&self, offset: usize) -> usize {
        let end = self.line_range_at(offset).end;
        if self.content.as_bytes().get(end) == Some(&b'\n')
            && end > 0
            && self.content.as_bytes()[end - 1] == b'\r'
        {
            end - 1
        } else {
            end
        }
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                return;
            }
            self.extend_selection(prev, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                return;
            }
            self.extend_selection(next, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            self.move_to(prev, cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.selected_range.end);
            self.move_to(next, cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if self.mention_has_selection {
            cx.emit(ComposerInputEvent::MentionNavigate(-1));
            return;
        }
        if let Some((ix, affinity, column)) = self.vertical_target(-1.0) {
            self.move_to(ix, cx);
            self.caret_affinity = affinity;
            self.preferred_column = Some(column);
        }
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if self.mention_has_selection {
            cx.emit(ComposerInputEvent::MentionNavigate(1));
            return;
        }
        if let Some((ix, affinity, column)) = self.vertical_target(1.0) {
            self.move_to(ix, cx);
            self.caret_affinity = affinity;
            self.preferred_column = Some(column);
        }
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((ix, affinity, column)) = self.vertical_target(-1.0) {
            self.select_to(ix, cx);
            self.caret_affinity = affinity;
            self.preferred_column = Some(column);
        }
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((ix, affinity, column)) = self.vertical_target(1.0) {
            self.select_to(ix, cx);
            self.caret_affinity = affinity;
            self.preferred_column = Some(column);
        }
    }

    /// Offset one wrapped line above/below the cursor, keeping its x column.
    /// Clamps to the document edges, matching the platform's behavior on the
    /// first and last line.
    fn vertical_target(&self, dir: f32) -> Option<(usize, CaretAffinity, Pixels)> {
        let current = self.cursor_point()?;
        let column = self.preferred_column.unwrap_or(current.x);
        let target_y = f32::from(current.y) + dir * f32::from(self.line_height);
        if target_y < 0.0 {
            return Some((0, CaretAffinity::Downstream, column));
        }
        if target_y >= self.content_height {
            return Some((self.content.len(), CaretAffinity::Upstream, column));
        }
        let (index, affinity) = self.caret_for_point(point(column, px(target_y)));
        Some((index, affinity, column))
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all_text(cx);
    }

    /// Select the whole draft — an inline rename opens with its name selected.
    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let line = self.line_range_at(self.cursor_offset());
        self.move_to(line.start, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_content_end_at(self.cursor_offset()), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        let line = self.line_range_at(self.cursor_offset());
        self.select_to(line.start, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.line_content_end_at(self.cursor_offset()), cx);
    }

    fn doc_start(&mut self, _: &DocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn doc_end(&mut self, _: &DocEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn select_doc_start(&mut self, _: &SelectDocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, cx);
    }

    fn select_doc_end(&mut self, _: &SelectDocEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.content.len(), cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let prev = self.previous_word_boundary(self.cursor_offset());
        self.move_to(prev, cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let next = self.next_word_boundary(self.cursor_offset());
        self.move_to(next, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let prev = self.previous_word_boundary(self.cursor_offset());
        self.select_to(prev, cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        let next = self.next_word_boundary(self.cursor_offset());
        self.select_to(next, cx);
    }

    /// Opt/Cmd + Delete family. With a live selection these delete the
    /// selection only (platform behavior) — the extend runs off the cursor.
    fn delete_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            if self.cursor_offset() == offset {
                return;
            }
            self.extend_selection(offset, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prev = self.previous_word_boundary(self.cursor_offset());
        self.delete_to(prev, window, cx);
    }

    fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next = self.next_word_boundary(self.cursor_offset());
        self.delete_to(next, window, cx);
    }

    fn delete_to_line_start(
        &mut self,
        _: &DeleteToLineStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = self.line_range_at(self.cursor_offset()).start;
        self.delete_to(start, window, cx);
    }

    fn delete_to_line_end(
        &mut self,
        _: &DeleteToLineEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let end = self.line_content_end_at(self.cursor_offset());
        self.delete_to(end, window, cx);
    }

    fn clipboard_selection(&self) -> Option<(String, String)> {
        if self.selected_range.is_empty() {
            return None;
        }
        let selected = self.projection.normalize_range(self.selected_range.clone());
        let raw = self.content[selected.clone()].to_string();
        let mut text = String::new();
        let mut at = selected.start;
        // Use the document's actual chips: parsing a selected substring alone
        // would activate literal examples selected from inside code or images.
        for (link, _) in &self.projection.mentions {
            if link.range.start < selected.start || link.range.end > selected.end {
                continue;
            }
            text.push_str(&self.content[at..link.range.start]);
            text.push_str(&openagents_proto::invocation::invocation_prompt(
                &openagents_proto::file_mentions::file_mention_prompt(
                    &self.content[link.range.clone()],
                ),
            ));
            at = link.range.end;
        }
        text.push_str(&self.content[at..selected.end]);
        Some((raw, text))
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((raw, text)) = self.clipboard_selection() {
            cx.write_to_clipboard(ClipboardItem::new_string_with_json_metadata(
                text.clone(),
                serde_json::json!({
                    "openagentsComposerV1": raw,
                    "text": text,
                    "openagentsAttachmentScope": self.attachment_scope,
                }),
            ));
        } else if self.copies_transcript_selection
            && let Some(text) = crate::markdown::selection::selected_text()
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some((raw, text)) = self.clipboard_selection() {
            cx.write_to_clipboard(ClipboardItem::new_string_with_json_metadata(
                text.clone(),
                serde_json::json!({
                    "openagentsComposerV1": raw,
                    "text": text,
                    "openagentsAttachmentScope": self.attachment_scope,
                }),
            ));

            self.last_edit = None;
            self.replace_text_in_range(None, "", window, cx);
            self.last_edit = None;
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        // Image data (or copied files) beats text — the original composer's
        // onPaste prevents the default text insert when `clipboardData.files`
        // is non-empty and stages the images instead.
        let mut images: Vec<gpui::Image> = Vec::new();
        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in &item.entries {
            match entry {
                ClipboardEntry::Image(image) => images.push(image.clone()),
                ClipboardEntry::ExternalPaths(files) => {
                    paths.extend(files.paths().iter().cloned());
                }
                ClipboardEntry::String(_) => {}
            }
        }
        if !images.is_empty() {
            cx.emit(ComposerInputEvent::PastedImages(images));
            return;
        }
        if !paths.is_empty() {
            cx.emit(ComposerInputEvent::PastedPaths(paths));
            return;
        }
        if let Some(mut text) = item.text() {
            if self.mentions_enabled {
                if let Some(value) = item
                    .metadata()
                    .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
                {
                    if value.get("text").and_then(|v| v.as_str()) == Some(text.as_str()) {
                        if let Some(raw) =
                            value.get("openagentsComposerV1").and_then(|v| v.as_str())
                        {
                            // Chip numbers belong to one draft: chips copied from
                            // another chat would name the wrong attachment, so
                            // they paste as their plain labels instead.
                            let from_this_draft = value
                                .get("openagentsAttachmentScope")
                                .and_then(|v| v.as_str())
                                .is_some_and(|scope| scope == self.attachment_scope);
                            if from_this_draft
                                || openagents_proto::attachment_mentions::attachment_mentions(raw)
                                    .is_empty()
                            {
                                text = raw.to_owned();
                            }
                        }
                    }
                }
            }
            // Clipboard operations remain separate undo steps, even a one-character paste.
            self.last_edit = None;
            self.replace_text_in_range(None, &text, window, cx);
            self.last_edit = None;
            if self.mentions_enabled && !self.single_line {
                let end = self.cursor_offset();
                cx.emit(ComposerInputEvent::PastedText {
                    range: end.saturating_sub(text.len())..end,
                    revision: self.edit_revision,
                });
            }
        }
    }

    fn apply_pasted_references(
        &mut self,
        original: &str,
        paste_end: usize,
        revision: u64,
        replacements: Vec<(Range<usize>, String)>,
        cx: &mut Context<Self>,
    ) {
        if self.read_only
            // A delayed paste lookup must not rewrite the range captured by
            // dictation and make its final transcript fail the draft guard.
            || self.dictation_active()
            || self.edit_revision != revision
            || self.text() != original
            || self.marked_range.is_some()
            || self.selected_range != (paste_end..paste_end)
        {
            return;
        }
        self.edit_revision = self.edit_revision.wrapping_add(1);
        let mut cursor = paste_end;
        for (range, link) in replacements.into_iter().rev() {
            cursor = cursor - range.len() + link.len();
            self.content.replace_range(range, &link);
        }
        // Canonicalization belongs to the paste's existing undo step.
        self.selected_range = cursor..cursor;
        self.refresh_projection();
        self.needs_measure = true;
        self.last_edit = None;
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.mentions_enabled && self.selected_range.is_empty() && self.marked_range.is_none() {
            if let Some((range, inserted)) =
                composer_markdown::newline_edit(&self.content, self.cursor_offset())
            {
                let range = self.range_to_utf16(&range);
                self.replace_text_in_range(Some(range), &inserted, window, cx);
                return;
            }
        }
        let line_end = self.line_range_at(self.cursor_offset()).end;
        let newline = if self.line_content_end_at(self.cursor_offset()) < line_end {
            "\r\n"
        } else {
            "\n"
        };
        self.replace_text_in_range(None, newline, window, cx);
    }

    fn message_newline_or_accept(
        &mut self,
        _: &MessageNewlineOrAccept,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match enter_outcome(self.mention_has_selection, EnterOutcome::Newline) {
            EnterOutcome::AcceptCompletion => cx.emit(ComposerInputEvent::MentionAccept),
            EnterOutcome::Newline => self.newline(&Newline, window, cx),
            EnterOutcome::Submit => unreachable!("newline action cannot submit"),
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        match enter_outcome(self.mention_has_selection, EnterOutcome::Submit) {
            EnterOutcome::AcceptCompletion => cx.emit(ComposerInputEvent::MentionAccept),
            EnterOutcome::Submit => cx.emit(ComposerInputEvent::Submitted),
            EnterOutcome::Newline => unreachable!("submit action cannot insert a newline"),
        }
    }

    fn modified_submit(&mut self, _: &ModifiedSubmit, _: &mut Window, cx: &mut Context<Self>) {
        match enter_outcome(self.mention_has_selection, EnterOutcome::Submit) {
            EnterOutcome::AcceptCompletion => cx.emit(ComposerInputEvent::MentionAccept),
            EnterOutcome::Submit => cx.emit(ComposerInputEvent::ModifiedSubmitted),
            EnterOutcome::Newline => unreachable!("submit action cannot insert a newline"),
        }
    }

    fn mention_tab(&mut self, _: &MentionTab, window: &mut Window, cx: &mut Context<Self>) {
        if self.mention_has_selection {
            cx.emit(ComposerInputEvent::MentionAccept);
        } else if !self.indent_list(false, window, cx) {
            cx.propagate();
        }
    }

    fn outdent_list(&mut self, _: &OutdentList, window: &mut Window, cx: &mut Context<Self>) {
        if self.mention_open || !self.indent_list(true, window, cx) {
            cx.propagate();
        }
    }

    fn indent_list(&mut self, outdent: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.mentions_enabled
            || self.read_only
            || self.marked_range.is_some()
            || self.mention_open
        {
            return false;
        }
        let selection = self.selected_range.clone();
        let start = self.line_range_at(selection.start).start;
        let last = if !selection.is_empty() && self.content[..selection.end].ends_with('\n') {
            selection.end - 1
        } else {
            selection.end
        };
        let end = self.line_range_at(last).end;
        let block = &self.content[start..end];
        let mut replacement = String::new();
        let mut changes = Vec::new();
        let mut at = start;
        for (ix, line) in block.split('\n').enumerate() {
            let Some(prefix) = composer_markdown::list_prefix(line) else {
                return false;
            };
            if in_code(&self.content, at + prefix.indent) {
                return false;
            }
            if ix > 0 {
                replacement.push('\n');
            }
            let removed = if outdent {
                line[prefix.indent_start..prefix.indent]
                    .chars()
                    .take(2)
                    .map(char::len_utf8)
                    .sum()
            } else {
                0
            };
            let added = if outdent { 0 } else { 2 };
            replacement.push_str(&line[..prefix.indent_start]);
            if !outdent {
                replacement.push_str("  ");
            }
            replacement.push_str(&line[prefix.indent_start + removed..]);
            changes.push((at + prefix.indent_start, removed, added));
            at += line.len() + 1;
        }
        let remap = |offset: usize| -> usize {
            let mut result = offset as isize;
            for &(at, removed, added) in &changes {
                if offset >= at {
                    result += added as isize - removed.min(offset - at) as isize;
                }
            }
            result.max(0) as usize
        };
        let next = remap(selection.start)..remap(selection.end);
        let reversed = self.selection_reversed;
        let edit = self.range_to_utf16(&(start..end));
        self.last_edit = None;
        self.replace_text_in_range(Some(edit), &replacement, window, cx);
        self.selected_range = next;
        self.selection_reversed = reversed;
        self.refresh_projection();
        self.last_edit = None;
        cx.notify();
        true
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && self.dictation_active() {
            self.cancel_dictation();
            self.dictation_changed(cx);
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if escape_dismisses_completion(&event.keystroke.key, self.mention_open) {
            cx.emit(ComposerInputEvent::MentionDismiss);
            cx.stop_propagation();
        }
    }

    // ---- geometry ----

    /// Content-local point for a byte index (y grows down from content top).
    fn point_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        self.point_for_display_index(self.projection.raw_to_display(index))
    }

    fn cursor_point(&self) -> Option<Point<Pixels>> {
        self.point_for_display_index_with_affinity(
            self.projection.raw_to_display(self.cursor_offset()),
            self.caret_affinity,
        )
    }

    /// Content-local point for a shaped projection byte index. The icon layer
    /// uses this to occupy its explicit projection slot without inventing a
    /// second coordinate system beside the custom text editor.
    fn point_for_display_index(&self, index: usize) -> Option<Point<Pixels>> {
        self.point_for_display_index_with_affinity(index, CaretAffinity::Downstream)
    }

    fn point_for_display_index_with_affinity(
        &self,
        index: usize,
        affinity: CaretAffinity,
    ) -> Option<Point<Pixels>> {
        for (line_ix, line) in self.last_lines.iter().enumerate() {
            let line_start = *self.line_starts.get(line_ix)?;
            let line_len = line.len();
            if index < line_start {
                continue;
            }
            if index <= line_start + line_len {
                let relative_index = index - line_start;
                let local = if affinity == CaretAffinity::Downstream {
                    line.wrap_boundaries()
                        .iter()
                        .position(|boundary| {
                            line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index
                                == relative_index
                        })
                        .map(|row| point(px(0.0), self.line_height * (row + 1)))
                        .or_else(|| line.position_for_index(relative_index, self.line_height))?
                } else {
                    line.position_for_index(relative_index, self.line_height)?
                };
                let y_offset: f32 = self
                    .last_lines
                    .iter()
                    .take(line_ix)
                    .map(|l| f32::from(l.size(self.line_height).height))
                    .sum();
                let indent = if local.y > px(0.0) {
                    self.line_indents.get(line_ix).copied().unwrap_or_default()
                } else {
                    px(0.0)
                };
                return Some(point(local.x + indent, local.y + px(y_offset)));
            }
        }
        None
    }

    /// [`chip_label_offset`] for the chip label starting at projected byte
    /// `label`, from the shaped line holding it.
    fn chip_label_offset(
        &self,
        label: usize,
        row_top: Pixels,
        room: Pixels,
        window: &Window,
    ) -> Pixels {
        let line_ix = self
            .line_starts
            .partition_point(|start| *start <= label)
            .saturating_sub(1);
        match (self.last_lines.get(line_ix), self.line_starts.get(line_ix)) {
            (Some(line), Some(start)) => chip_label_offset(
                &line.unwrapped_layout,
                label - start,
                row_top,
                self.line_height,
                room,
                window,
            ),
            _ => px(0.0),
        }
    }

    /// Content-local boxes occupied by a projected byte range, split at every
    /// soft wrap. A caret exactly at a wrap boundary belongs visually to both
    /// rows in GPUI; using the explicit wrap indices lets the range's first
    /// glyph start at x=0 on the new row instead of inheriting the old row's
    /// end caret (which previously caused mention washes to be discarded).
    fn bounds_for_display_range(&self, range: Range<usize>) -> Vec<Bounds<Pixels>> {
        let mut bounds = Vec::new();
        let mut y_offset = px(0.0);
        for (line_ix, line) in self.last_lines.iter().enumerate() {
            let line_start = self.line_starts.get(line_ix).copied().unwrap_or(0);
            let local_start = range.start.saturating_sub(line_start).min(line.len());
            let local_end = range.end.saturating_sub(line_start).min(line.len());
            if local_start >= local_end
                || range.end <= line_start
                || range.start >= line_start + line.len()
            {
                y_offset += line.size(self.line_height).height;
                continue;
            }

            let row_ends = line
                .wrap_boundaries()
                .iter()
                .map(|boundary| line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index)
                .chain(std::iter::once(line.len()));
            for (row_ix, row_start, segment) in
                display_row_segments(local_start..local_end, row_ends)
            {
                let row_y = y_offset + self.line_height * row_ix;
                let start_x = if segment.start == row_start {
                    px(0.0)
                } else {
                    line.position_for_index(segment.start, self.line_height)
                        .map(|point| point.x)
                        .unwrap_or(px(0.0))
                };
                if let Some(end_point) = line.position_for_index(segment.end, self.line_height)
                    && end_point.x > start_x
                {
                    bounds.push(Bounds::new(
                        point(
                            start_x
                                + if row_ix > 0 {
                                    self.line_indents.get(line_ix).copied().unwrap_or_default()
                                } else {
                                    px(0.0)
                                },
                            row_y,
                        ),
                        size(end_point.x - start_x, self.line_height),
                    ));
                }
            }
            y_offset += line.size(self.line_height).height;
        }
        bounds
    }

    /// Byte index and visual side closest to a content-local point.
    fn caret_for_point(&self, position: Point<Pixels>) -> (usize, CaretAffinity) {
        if self.display_is_placeholder {
            return (0, CaretAffinity::Downstream);
        }
        let mut y = f32::from(position.y);
        if y < 0.0 {
            return (0, CaretAffinity::Downstream);
        }
        for (line_ix, line) in self.last_lines.iter().enumerate() {
            let height = f32::from(line.size(self.line_height).height);
            let line_start = self.line_starts.get(line_ix).copied().unwrap_or(0);
            if y < height || line_ix + 1 == self.last_lines.len() {
                let indent = if y >= f32::from(self.line_height) {
                    self.line_indents.get(line_ix).copied().unwrap_or_default()
                } else {
                    px(0.0)
                };
                let local = point(position.x - indent, px(y.min(height - 1.0).max(0.0)));
                let ix = line
                    .closest_index_for_position(local, self.line_height)
                    .unwrap_or_else(|ix| ix);
                let affinity = line
                    .wrap_boundaries()
                    .iter()
                    .position(|boundary| {
                        line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index == ix
                    })
                    .filter(|row| local.y < self.line_height * (row + 1))
                    .map_or(CaretAffinity::Downstream, |_| CaretAffinity::Upstream);
                let mut raw = self
                    .projection
                    .display_to_raw((line_start + ix).min(self.projection.display.len()));
                // CRLF is one newline grapheme. A click past the text on its
                // row must land before CR, never between its two bytes.
                if raw > 0
                    && self.content.as_bytes().get(raw) == Some(&b'\n')
                    && self.content.as_bytes()[raw - 1] == b'\r'
                {
                    raw -= 1;
                }
                return (raw, affinity);
            }
            y -= height;
        }
        (self.content.len(), CaretAffinity::Upstream)
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        self.caret_for_mouse_position(position).0
    }

    fn caret_for_mouse_position(&self, position: Point<Pixels>) -> (usize, CaretAffinity) {
        let Some(bounds) = self.last_bounds else {
            return (0, CaretAffinity::Downstream);
        };
        let local = point(
            position.x - bounds.left() + px(self.scroll_left),
            position.y - bounds.top() + px(self.scroll_top),
        );
        self.caret_for_point(local)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_mention_tooltip();
        window.focus(&self.focus_handle, cx);
        let intent = press_intent(event.click_count, event.modifiers.shift);
        self.chip_press = (intent == PressIntent::PlaceCaret)
            .then(|| self.image_chip_at(event.position))
            .flatten()
            .map(|index| (index, event.position));
        self.is_selecting = true;
        self.drag_position = Some(event.position);
        self.drag_unit = None;
        self.drag_generation = self.drag_generation.wrapping_add(1);
        self.drag_autoscroll_active = false;
        match intent {
            PressIntent::Word | PressIntent::Line => {
                let index = self.index_for_mouse_position(event.position);
                let range = if intent == PressIntent::Word {
                    self.mention_hits
                        .iter()
                        .find(|hit| hit.bounds.contains(&event.position))
                        .map(|hit| hit.target.range.clone())
                        .unwrap_or_else(|| self.selection_unit(intent, index))
                } else {
                    self.selection_unit(intent, index)
                };
                self.move_to(range.start, cx);
                self.select_to(range.end, cx);
                self.drag_unit = Some((intent, range));
            }
            PressIntent::ExtendSelection => {
                let (index, affinity) = self.caret_for_mouse_position(event.position);
                self.select_to(index, cx);
                self.caret_affinity = affinity;
            }
            PressIntent::PlaceCaret => {
                let (index, affinity) = self.caret_for_mouse_position(event.position);
                self.move_to(index, cx);
                self.caret_affinity = affinity;
            }
        }
    }

    fn selection_unit(&self, intent: PressIntent, index: usize) -> Range<usize> {
        let range = if intent == PressIntent::Line {
            let mut range = self.line_range_at(index);
            if range.end < self.content.len() {
                range.end += 1;
            }
            range
        } else {
            // Hit testing snaps to chip edges; prefer the chip at that edge.
            if let Some((link, _)) = self
                .projection
                .mentions
                .iter()
                .find(|(link, _)| link.range.start <= index && index < link.range.end)
            {
                return link.range.clone();
            }
            word_range(&self.content, index)
        };
        self.projection.normalize_range(range)
    }

    fn drag_select_to(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some((intent, anchor)) = self.drag_unit.clone() {
            let range = self.selection_unit(intent, index);
            if range.start < anchor.start {
                self.move_to(anchor.end, cx);
                self.select_to(range.start, cx);
            } else {
                self.move_to(anchor.start, cx);
                self.select_to(range.end.max(anchor.end), cx);
            }
        } else {
            self.select_to(index, cx);
        }
    }

    fn drag_select_at(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let (index, affinity) = self.caret_for_mouse_position(position);
        self.drag_select_to(index, cx);
        if self.drag_unit.is_none() {
            self.caret_affinity = affinity;
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        // A click that starts and ends on an image chip opens the picture, as
        // the thumbnails did; a drag across it only selects.
        if let Some((index, down)) = self.chip_press.take()
            && self.image_chip_at(event.position) == Some(index)
            && (event.position - down).magnitude() <= 4.0
        {
            cx.emit(ComposerInputEvent::OpenAttachment(index));
        }
        self.is_selecting = false;
        self.drag_position = None;
        self.drag_generation = self.drag_generation.wrapping_add(1);
        self.drag_autoscroll_active = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        self.on_mention_pointer_move(event.position, cx);
        if self.is_selecting {
            self.drag_position = Some(event.position);
            let position = self.drag_selection_position(event.position);
            self.drag_select_at(position, cx);
            if self.drag_scroll_delta(event.position) != 0.0 && !self.drag_autoscroll_active {
                self.start_drag_autoscroll(cx);
            }
        }
    }

    fn start_drag_autoscroll(&mut self, cx: &mut Context<Self>) {
        self.drag_autoscroll_active = true;
        let generation = self.drag_generation;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(DRAG_SCROLL_FRAME_MS))
                    .await;
                let keep_running = this
                    .update(cx, |input, cx| input.drag_autoscroll_tick(generation, cx))
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }

    fn drag_selection_position(&self, position: Point<Pixels>) -> Point<Pixels> {
        let Some(bounds) = self.last_bounds else {
            return position;
        };
        point(
            position.x.clamp(bounds.left(), bounds.right() - px(0.5)),
            position.y.clamp(bounds.top(), bounds.bottom() - px(0.5)),
        )
    }

    fn drag_scroll_delta(&self, position: Point<Pixels>) -> f32 {
        let Some(bounds) = self.last_bounds else {
            return 0.0;
        };
        input_drag_scroll_delta(
            f32::from(position.y),
            f32::from(bounds.top()),
            f32::from(bounds.bottom()),
            f32::from(self.line_height),
        )
    }

    fn drag_autoscroll_tick(&mut self, generation: u64, cx: &mut Context<Self>) -> bool {
        if !self.is_selecting || self.drag_generation != generation {
            return false;
        }
        let (Some(position), Some(bounds)) = (self.drag_position, self.last_bounds) else {
            self.drag_autoscroll_active = false;
            return false;
        };
        let delta = self.drag_scroll_delta(position);
        if delta == 0.0 {
            self.drag_autoscroll_active = false;
            return false;
        }
        let next = (self.scroll_top + delta).clamp(
            0.0,
            input_max_scroll(
                self.content_height,
                self.settled_viewport_height
                    .unwrap_or(f32::from(bounds.size.height)),
            ),
        );
        if next == self.scroll_top {
            self.drag_autoscroll_active = false;
            return false;
        }
        self.scroll_top = next;
        let edge_position = self.drag_selection_position(position);
        self.drag_select_at(edge_position, cx);
        // Selection motion normally resumes caret following. During an edge
        // drag the autoscroll loop owns the viewport instead.
        self.follow_cursor = false;
        true
    }

    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.last_bounds else {
            return;
        };
        let viewport_height = self
            .settled_viewport_height
            .unwrap_or(f32::from(bounds.size.height));
        let delta_y = f32::from(event.delta.pixel_delta(self.line_height).y);
        let next = input_scroll_offset(
            self.scroll_top,
            delta_y,
            self.content_height,
            viewport_height,
        );
        if next == self.scroll_top {
            // Overscroll guard: when the input itself is scrollable (content
            // taller than the viewport), swallow the wheel event even at the
            // scroll boundary so it never chains into the outer transcript
            // list (the native equivalent of `overscroll-behavior: contain`).
            if delta_y != 0.0 && input_max_scroll(self.content_height, viewport_height) > 0.0 {
                cx.stop_propagation();
            }
            return;
        }
        self.invalidate_mention_tooltip();
        self.scroll_top = next;
        self.follow_cursor = false;
        cx.stop_propagation();
        cx.emit(ComposerInputEvent::ViewportChanged);
        cx.notify();
    }

    // ---- utf16 mapping (IME) ----

    fn offset_from_utf16(&self, offset: usize) -> usize {
        Self::utf8_offset(&self.content, offset)
    }

    fn utf8_offset(text: &str, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in text.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    /// Shape the text at a width; store measured layout; return content height.
    /// Called from the element's measured-layout closure.
    fn layout_text(
        &mut self,
        width: Pixels,
        style: &TextStyle,
        window: &mut Window,
        cx: &App,
    ) -> f32 {
        let theme = Theme::of(cx);
        let key = InputLayoutKey {
            width,
            font: style.font(),
            font_size: style.font_size.to_pixels(window.rem_size()),
            color: style.color,
            chip_family: theme.font_mono.clone(),
            chip_color: theme.code_text,
            syntax: theme.syntax.clone(),
            marked_range: self.marked_range.clone(),
            placeholder: self.placeholder.clone(),
            mentions_enabled: self.mentions_enabled,
            active_line: self.mentions_enabled.then(|| self.editing_source_range()),
        };
        // Height-only animation, scrolling, selection and caret blinking do
        // not change shaping. Reuse the entity's single retained layout,
        // including the parent's early measurement of this same edit.
        if !self.needs_measure && self.last_layout_key.as_ref() == Some(&key) {
            self.layout_epoch += 1;
            return self.content_height;
        }
        #[cfg(test)]
        {
            self.layout_rebuilds += 1;
        }
        // Rebuild this even for an empty draft. Otherwise deleting the final
        // mention can leave its previous paint geometry alive while the
        // placeholder is already being shaped, tinting "Do anything" for a
        // frame (or longer when no subsequent layout is requested).
        self.refresh_projection();
        let (display, is_placeholder) = if self.content.is_empty() {
            (self.placeholder.clone(), true)
        } else {
            (SharedString::from(self.projection.display.clone()), false)
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        self.line_height = px(self.configured_line_height);

        // Inline code: the markdown renderer's recipe (mono font + the
        // spectrum's `code_text`) over the rounded `code_wash` beneath.
        let (chip_font, chip_color) = {
            let theme = Theme::of(cx);
            (gpui::font(theme.font_mono.clone()), theme.code_text)
        };
        let run_for = |len: usize, underline: bool, code: bool| TextRun {
            len,
            font: if code {
                chip_font.clone()
            } else {
                style.font()
            },
            color: if code { chip_color } else { style.color },
            // Rounded mention washes are painted explicitly beneath the text;
            // TextRun backgrounds are square and can disappear in wrapped runs.
            background_color: None,
            underline: underline.then_some(UnderlineStyle {
                color: Some(style.color),
                thickness: px(1.0),
                wavy: false,
            }),
            strikethrough: None,
        };
        let raw_faces = if self.mentions_enabled && !is_placeholder {
            composer_markdown::faces(&self.content)
        } else {
            Vec::new()
        };
        let faces = if self.mentions_enabled && !is_placeholder {
            raw_faces
                .iter()
                .cloned()
                .map(|(range, face)| {
                    (
                        self.projection.raw_to_display(range.start)
                            ..self.projection.raw_to_display(range.end),
                        face,
                    )
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let syntax: Vec<_> = if self.mentions_enabled && self.syntax_source == self.content {
            self.syntax_spans
                .iter()
                .map(|span| {
                    (
                        self.projection.raw_to_display(span.range.start)
                            ..self.projection.raw_to_display(span.range.end),
                        span.kind,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let marked = self.marked_range.as_ref().map(|r| {
            self.projection.raw_to_display(r.start)..self.projection.raw_to_display(r.end)
        });
        let mut boundaries = vec![0, display.len()];
        for (range, _) in &syntax {
            boundaries.extend([range.start, range.end]);
        }
        for (range, _) in &faces {
            boundaries.extend([range.start, range.end]);
        }
        for (_, range) in &self.projection.mentions {
            boundaries.extend([range.start, range.end]);
            for pad in chip_pad_ranges(range) {
                boundaries.extend([pad.start, pad.end]);
            }
        }
        if let Some(range) = &marked {
            boundaries.extend([range.start, range.end]);
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        let mut face_events: Vec<_> = faces
            .iter()
            .flat_map(|(range, face)| {
                [
                    (range.start, *face as usize, 1isize),
                    (range.end, *face as usize, -1isize),
                ]
            })
            .collect();
        face_events.sort_by_key(|event| event.0);
        let mut face_event = 0;
        let mut face_depth = [0isize; 4];
        let runs: Vec<TextRun> = boundaries
            .windows(2)
            .filter(|r| r[1] > r[0])
            .map(|r| {
                while face_event < face_events.len() && face_events[face_event].0 <= r[0] {
                    let (_, face, delta) = face_events[face_event];
                    face_depth[face] += delta;
                    face_event += 1;
                }
                let mention_ix = self
                    .projection
                    .mentions
                    .partition_point(|(_, range)| range.end <= r[0]);
                let chip_range = self
                    .projection
                    .mentions
                    .get(mention_ix)
                    .map(|(_, range)| range)
                    .filter(|range| range.contains(&r[0]));
                let chip = chip_range.is_some();
                let code = face_depth[composer_markdown::Face::Code as usize] > 0;
                let mut run = run_for(
                    r[1] - r[0],
                    marked.as_ref().is_some_and(|range| range.contains(&r[0])),
                    code,
                );
                if chip_range.is_some_and(|range| {
                    chip_pad_ranges(range).iter().any(|pad| pad.contains(&r[0]))
                }) {
                    run.font.family = CHIP_PAD_FAMILY.into();
                }
                if !chip {
                    if face_depth[composer_markdown::Face::Bold as usize] > 0 {
                        run.font.weight = gpui::FontWeight::BOLD;
                    }
                    if face_depth[composer_markdown::Face::Italic as usize] > 0 {
                        run.font.style = gpui::FontStyle::Italic;
                    }
                    if code {
                        run.background_color = Some(theme.code_wash);
                    }
                    if face_depth[composer_markdown::Face::Strikethrough as usize] > 0 {
                        run.strikethrough = Some(gpui::StrikethroughStyle {
                            thickness: px(1.0),
                            color: Some(style.color),
                        });
                    }
                }
                if code && !chip {
                    run.color = style.color;
                    if let Some((_, kind)) = syntax
                        .get(syntax.partition_point(|(range, _)| range.end <= r[0]))
                        .filter(|(range, _)| range.contains(&r[0]))
                    {
                        run.color = Theme::of(cx).syntax.color(*kind);
                    }
                }
                run
            })
            .collect();
        #[cfg(test)]
        {
            self.last_runs = runs.clone();
        }

        // Each logical list line reserves its marker width for continuation
        // rows. Painting and hit testing apply the same continuation offset.
        let mut lines = Vec::new();
        let mut indents = Vec::new();
        let mut display_at = 0;
        let mut raw_at = 0;
        let mut run_at = 0;
        let run_ranges: Vec<_> = runs
            .iter()
            .map(|run| {
                let range = run_at..run_at + run.len;
                run_at = range.end;
                (range, run)
            })
            .collect();
        let code_ranges: Vec<_> = raw_faces
            .iter()
            .filter(|(_, face)| *face == composer_markdown::Face::Code)
            .map(|(range, _)| range)
            .collect();
        for text in display.split('\n') {
            let raw_line = self
                .content
                .get(raw_at..)
                .unwrap_or_default()
                .split('\n')
                .next()
                .unwrap_or_default();
            let mut indent = px(0.0);
            if self.mentions_enabled && !is_placeholder {
                if let Some(prefix) = composer_markdown::list_prefix(raw_line).filter(|prefix| {
                    let marker_at = raw_at + prefix.indent;
                    let code_ix = code_ranges.partition_point(|range| range.end <= marker_at);
                    !code_ranges
                        .get(code_ix)
                        .is_some_and(|range| range.contains(&marker_at))
                }) {
                    let end = self
                        .projection
                        .raw_to_display(raw_at + prefix.end)
                        .saturating_sub(display_at)
                        .min(text.len());
                    let marker: SharedString = text[..end].to_string().into();
                    let marker_run = run_for(marker.len(), false, false);
                    indent = window
                        .text_system()
                        .shape_line(marker, font_size, &[marker_run], None)
                        .width
                        .min(width * 0.4);
                }
            }
            let end = display_at + text.len();
            let first_run = run_ranges.partition_point(|(r, _)| r.end <= display_at);
            let line_runs: Vec<TextRun> = run_ranges[first_run..]
                .iter()
                .take_while(|(r, _)| r.start < end)
                .filter_map(|(range, run)| {
                    let len = range
                        .end
                        .min(end)
                        .saturating_sub(range.start.max(display_at));
                    (len > 0).then(|| TextRun {
                        len,
                        ..(*run).clone()
                    })
                })
                .collect();
            if let Ok(shaped) = window.text_system().shape_text(
                text.to_string().into(),
                font_size,
                &line_runs,
                (!self.single_line).then_some((width - indent).max(px(20.0))),
                None,
            ) {
                let chips: Vec<_> = self.projection.mentions[self
                    .projection
                    .mentions
                    .partition_point(|(_, r)| r.end <= display_at)..]
                    .iter()
                    .take_while(|(_, r)| r.start < end)
                    .filter(|(_, r)| r.start >= display_at && r.end <= end)
                    .map(|(_, r)| r.start - display_at..r.end - display_at)
                    .collect();
                for mut line in shaped {
                    if !self.single_line {
                        wrap_reference_chips(&mut line, &chips, (width - indent).max(px(20.0)));
                    }
                    lines.push(line);
                    indents.push(indent);
                }
            }
            display_at = end + 1;
            raw_at += raw_line.len() + 1;
        }
        self.line_indents = indents;

        // Logical line byte offsets (each shaped line covers one \n-split line).
        let mut line_starts = Vec::with_capacity(lines.len());
        let mut at = 0usize;
        for line in &lines {
            line_starts.push(at);
            at += line.len() + 1; // + '\n'
        }
        if line_starts.is_empty() {
            line_starts.push(0);
        }

        let content_height: f32 = lines
            .iter()
            .map(|l| f32::from(l.size(self.line_height).height))
            .sum();
        let max_line_width: f32 = lines
            .iter()
            .map(|l| f32::from(l.unwrapped_layout.width))
            .fold(0.0, f32::max);

        self.display_is_placeholder = is_placeholder;
        self.max_ascent = lines
            .iter()
            .map(|line| f32::from(line.unwrapped_layout.ascent))
            .fold(INPUT_TEXT_SIZE, f32::max);
        self.last_layout_key = Some(key);
        self.last_lines = lines;
        self.line_starts = line_starts;
        self.content_height = content_height.max(self.configured_line_height);
        self.max_line_width = if is_placeholder { 0.0 } else { max_line_width };
        self.last_width = f32::from(width);
        self.needs_measure = false;
        self.layout_epoch += 1;
        self.content_height
    }

    fn paint_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        let visible = f32::from(bounds.size.height);
        let top_overflow = input_overflow_edges(
            self.content_height,
            self.settled_viewport_height.unwrap_or(visible),
            visible,
            self.scroll_top,
        )
        .0;
        let top_padding = if top_overflow {
            self.overflow_top_padding
        } else {
            0.0
        };
        let height = input_reveal_height(
            visible,
            self.scroll_top,
            f32::from(self.line_height),
            self.resizing,
        );
        Bounds::new(
            point(bounds.left(), bounds.top() - px(top_padding)),
            size(bounds.size.width, px(height + top_padding)),
        )
    }

    /// Keep the cursor visible when content exceeds the element height.
    fn clamp_scroll(&mut self, element_height: f32) -> bool {
        if self.single_line {
            let previous = self.scroll_left;
            let width = (self.last_width - 2.0).max(1.0);
            if let Some(cursor) = self.cursor_point() {
                let x = f32::from(cursor.x);
                self.scroll_left = self.scroll_left.min(x).max(x - width).max(0.0);
            }
            self.scroll_left = self.scroll_left.min((self.max_line_width - width).max(0.0));
            self.scroll_top = 0.0;
            return self.scroll_left != previous;
        }
        let previous = self.scroll_top;
        if self.follow_cursor {
            if let Some(cursor) = self.cursor_point() {
                self.scroll_top = input_scroll_offset_for_cursor(
                    self.scroll_top,
                    f32::from(cursor.y),
                    f32::from(self.line_height),
                    self.content_height,
                    element_height,
                    self.settled_viewport_height,
                );
            }
        }
        self.scroll_top = self.scroll_top.clamp(
            0.0,
            input_max_scroll(
                self.content_height,
                self.settled_viewport_height.unwrap_or(element_height),
            ),
        );
        self.scroll_top != previous
    }
}

impl EventEmitter<ComposerInputEvent> for ComposerInput {}

impl Focusable for ComposerInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for ComposerInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self
            .projection
            .normalize_range(self.range_from_utf16(&range_utf16));
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content.get(range)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.selected_range = self.projection.normalize_range(self.selected_range.clone());
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.take().is_some() {
            self.refresh_projection();
            self.needs_measure = true;
            self.last_edit = None;
            cx.emit(ComposerInputEvent::CursorMoved);
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        self.cancel_dictation();
        let single_line_text;
        let new_text = if self.single_line {
            single_line_text = new_text.replace(['\r', '\n'], " ");
            single_line_text.as_str()
        } else {
            new_text
        };
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = self.projection.normalize_range(range);
        self.invalidate_mention_tooltip();
        // An IME commit is the tail of a composition whose pre-composition
        // snapshot was already taken (`replace_and_mark_text_in_range`);
        // recording here would pin undo to the half-composed text instead.
        if self.marked_range.is_none() {
            self.record_edit(&range, new_text);
        }
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.content =
            self.content[0..range.start].to_owned() + new_text + &self.content[range.end..];
        let cursor = range.start + new_text.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
        self.marked_range.take();
        self.refresh_projection();
        self.follow_cursor = true;
        self.reset_blink();
        self.needs_measure = true;
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        self.cancel_dictation();
        let single_line_text;
        let new_text = if self.single_line {
            single_line_text = new_text.replace(['\r', '\n'], " ");
            single_line_text.as_str()
        } else {
            new_text
        };
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = self.projection.normalize_range(range);
        self.invalidate_mention_tooltip();
        // First keystroke of a composition: snapshot the text as it stood
        // before any of it existed, so one undo drops the whole composition.
        if self.marked_range.is_none() {
            self.undo_stack.push(self.snapshot());
            if self.undo_stack.len() > UNDO_LIMIT {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
            self.last_edit = None;
        }
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.content =
            self.content[0..range.start].to_owned() + new_text + &self.content[range.end..];
        if new_text.is_empty() {
            self.marked_range = None;
        } else {
            self.marked_range = Some(range.start..range.start + new_text.len());
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|r| Self::utf8_offset(new_text, r.start)..Self::utf8_offset(new_text, r.end))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.selection_reversed = false;
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
        self.refresh_projection();
        self.follow_cursor = true;
        self.reset_blink();
        self.needs_measure = true;
        cx.emit(ComposerInputEvent::Edited);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self
            .projection
            .normalize_range(self.range_from_utf16(&range_utf16));
        let start = if range.is_empty() && range.start == self.cursor_offset() {
            self.cursor_point()?
        } else {
            self.point_for_index(range.start)?
        };
        let origin = point(
            bounds.left() + start.x - px(self.scroll_left),
            bounds.top() + start.y - px(self.scroll_top),
        );
        Some(Bounds::new(origin, size(px(2.0), self.line_height)))
    }

    fn character_index_for_point(
        &mut self,
        point_in_window: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let index = self.index_for_mouse_position(point_in_window);
        Some(self.offset_to_utf16(index))
    }
}

/// The custom element: measured auto-grow layout + shaped-line painting.
struct ComposerTextElement {
    input: Entity<ComposerInput>,
    /// Max content height before internal scrolling kicks in.
    max_content_height: f32,
}

struct MentionPathTooltip {
    path: SharedString,
    /// Stable for one `Waiting → Visible` promotion; a later activation gets
    /// a new key and therefore exactly one fresh fade-in.
    activation: u64,
}

impl Render for MentionPathTooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).for_popup();
        let card = crate::popover::popover_card(&theme)
            .max_w(px(480.0))
            .h(px(MENTION_TOOLTIP_HEIGHT))
            .flex()
            .items_center()
            .p_0()
            .px(px(8.0))
            .font_family(theme.font_mono.clone())
            .text_size(px(11.0))
            .text_color(theme.text_muted)
            .child(div().min_w_0().truncate().child(self.path.clone()));
        motion::fade_quick(
            ("file-mention-path-tooltip", self.activation),
            div().child(crate::frost::frosted(
                crate::popover::CARD_RADIUS,
                crate::frost::MENU_BLUR,
                card,
            )),
        )
    }
}

struct ComposerTextPrepaint {
    cursor: Option<PaintQuad>,
    /// Each chip row's bounds; the first row of a chip also carries its icon.
    mention_chips: Vec<(Bounds<Pixels>, Option<ChipIcon>)>,
    mention_hits: Vec<MentionHit>,
    /// Image chips take the pointing hand: a click opens them.
    image_chip_hitboxes: Vec<gpui::Hitbox>,
    selection_quads: Vec<PaintQuad>,
    /// Completion preview: window-space origin of the end-of-text caret plus
    /// the suffix to paint there (shaped at paint time — it never joins the
    /// content's own layout, so hit-testing and the caret ignore it).
    ghost: Option<(Point<Pixels>, SharedString)>,
}

impl IntoElement for ComposerTextElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for ComposerTextElement {
    type RequestLayoutState = ();
    type PrepaintState = ComposerTextPrepaint;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        let input = self.input.clone();
        let text_style = window.text_style();
        let max_content = self.max_content_height;
        let layout_id =
            window.request_measured_layout(style, move |known, available, window, cx| {
                let width = known.width.unwrap_or(match available.width {
                    gpui::AvailableSpace::Definite(width) => width,
                    _ => px(320.0),
                });
                let content_height = input.update(cx, |input, cx| {
                    input.layout_text(width, &text_style, window, cx)
                });
                size(width, px(content_height.min(max_content)))
            });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let text_style = window.text_style();
        self.input.update(cx, |input, cx| {
            // Intrinsic measurement may try several widths in one layout.
            // Only publish the resolved geometry, once: notifying for each
            // provisional width starts an endless measure/notify loop.
            input.layout_text(bounds.size.width, &text_style, window, cx);
            let layout = (bounds.size.width, input.content_height);
            let layout_changed = input.last_notified_layout != Some(layout);
            input.last_notified_layout = Some(layout);
            let scrolled = input.clamp_scroll(f32::from(bounds.size.height));
            input.last_bounds = Some(bounds);
            if scrolled || layout_changed {
                cx.emit(ComposerInputEvent::ViewportChanged);
            }
        });
        let input = self.input.read(cx);
        let paint_bounds = input.paint_bounds(bounds);
        let scroll = px(input.scroll_top);
        let origin = point(bounds.left() - px(input.scroll_left), bounds.top() - scroll);
        let selection_color = Theme::of(cx).selection;
        let caret_color = Theme::of(cx).caret;

        let mut mention_chips = Vec::new();
        let appearance = Theme::of(cx).appearance;
        let mut mention_hits = Vec::new();
        for (mention, display) in &input.projection.mentions {
            let mut first_row = true;
            let target = MentionTooltipTarget {
                range: mention.range.clone(),
                path: if mention.attachment.is_some() {
                    SharedString::from(mention.basename.clone())
                } else {
                    SharedString::from(format!(
                        "{}{}",
                        mention.path,
                        if mention.is_dir() { "/" } else { "" }
                    ))
                },
                attachment: mention.attachment,
            };
            // The path tooltip stays flush so the pointer can move onto it.
            let (tooltip_height, tooltip_gap) = (MENTION_TOOLTIP_HEIGHT, 1.0);
            let label = chip_pad_ranges(display)[0].end;
            for local_bounds in input.bounds_for_display_range(display.clone()) {
                let row_top = origin.y + local_bounds.origin.y;
                // Centered on the label (see `paint_chip`).
                let label_offset = input.chip_label_offset(label, row_top, px(1.5), window);
                let chip_bounds = Bounds::new(
                    point(
                        origin.x + local_bounds.origin.x,
                        row_top + px(1.5) + label_offset,
                    ),
                    size(local_bounds.size.width, local_bounds.size.height - px(3.0)),
                );
                mention_chips.push((
                    chip_bounds,
                    first_row.then(|| chip_icon(mention.kind, &mention.path, appearance)),
                ));
                first_row = false;
                let above_anchor = chip_bounds.top() - px(tooltip_height) - px(tooltip_gap);
                let anchor_y = if above_anchor >= px(0.0) {
                    above_anchor
                } else {
                    // GPUI positions at anchor + 1px; subtracting one keeps the
                    // below fallback flush so the pointer can enter the popup.
                    chip_bounds.bottom() - px(1.0)
                };
                let visible_bounds = chip_bounds.intersect(&paint_bounds);
                if visible_bounds.size.width == px(0.0) || visible_bounds.size.height == px(0.0) {
                    continue;
                }
                mention_hits.push(MentionHit {
                    target: target.clone(),
                    bounds: visible_bounds,
                    // The fixed-height popup starts at anchor + 1px. Moving
                    // the anchor above the chip therefore yields conventional
                    // above-target placement without cursor tracking.
                    anchor: point(chip_bounds.left(), anchor_y),
                });
            }
        }
        let mut selection_quads = Vec::new();
        let mut cursor = None;
        if input.selected_range.is_empty() || input.display_is_placeholder {
            if let Some(p) = input.cursor_point() {
                cursor = Some(fill(
                    Bounds::new(
                        point(origin.x + p.x, origin.y + p.y),
                        size(px(2.0), input.line_height),
                    ),
                    caret_color,
                ));
            } else if input.display_is_placeholder {
                cursor = Some(fill(
                    Bounds::new(origin, size(px(2.0), input.line_height)),
                    caret_color,
                ));
            }
        } else if let (Some(start), Some(end)) = (
            input.point_for_index(input.selected_range.start),
            input.point_for_display_index_with_affinity(
                input.projection.raw_to_display(input.selected_range.end),
                CaretAffinity::Upstream,
            ),
        ) {
            let lh = input.line_height;
            if start.y == end.y {
                selection_quads.push(fill(
                    Bounds::from_corners(
                        point(origin.x + start.x, origin.y + start.y),
                        point(origin.x + end.x, origin.y + start.y + lh),
                    ),
                    selection_color,
                ));
            } else {
                // First visual row, full middle rows, last visual row.
                selection_quads.push(fill(
                    Bounds::from_corners(
                        point(origin.x + start.x, origin.y + start.y),
                        point(bounds.right(), origin.y + start.y + lh),
                    ),
                    selection_color,
                ));
                if end.y > start.y + lh {
                    selection_quads.push(fill(
                        Bounds::from_corners(
                            point(origin.x, origin.y + start.y + lh),
                            point(bounds.right(), origin.y + end.y),
                        ),
                        selection_color,
                    ));
                }
                selection_quads.push(fill(
                    Bounds::from_corners(
                        point(origin.x, origin.y + end.y),
                        point(origin.x + end.x, origin.y + end.y + lh),
                    ),
                    selection_color,
                ));
            }
        }
        let tooltip = input.visible_mention_tooltip();
        if let Some((_target, anchor, _activation, view)) = tooltip {
            let view = view.into();
            let input = self.input.clone();
            window.set_tooltip(AnyTooltip {
                view,
                mouse_position: anchor,
                check_visible_and_update: Rc::new(move |popup, window, cx| {
                    input.update(cx, |input, _| {
                        input.check_mention_tooltip_visibility(popup, window.mouse_position())
                    })
                }),
            });
        }
        // The ghost only shows where accepting it would insert: a collapsed
        // caret at the end of real (non-placeholder, non-IME) text.
        let ghost = input
            .ghost
            .clone()
            .filter(|g| {
                !g.is_empty()
                    && !input.display_is_placeholder
                    && input.marked_range.is_none()
                    && input.selected_range.is_empty()
                    && input.cursor_offset() == input.content.len()
            })
            .and_then(|g| {
                input
                    .point_for_index(input.content.len())
                    .map(|p| (point(origin.x + p.x, origin.y + p.y), g))
            });
        let image_chip_hitboxes = mention_hits
            .iter()
            .filter(|hit| input.is_image_chip(&hit.target))
            .map(|hit| window.insert_hitbox(hit.bounds, gpui::HitboxBehavior::Normal))
            .collect();
        ComposerTextPrepaint {
            cursor,
            mention_chips,
            mention_hits,
            image_chip_hitboxes,
            selection_quads,
            ghost,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        self.input.update(cx, |input, _| {
            input.set_mention_hits(prepaint.mention_hits.clone())
        });
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let input = self.input.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble {
                input.update(cx, |input, cx| input.on_mouse_move(event, cx));
            }
        });
        for hitbox in &prepaint.image_chip_hitboxes {
            window.set_cursor_style(CursorStyle::PointingHand, hitbox);
        }

        // WrappedLine isn't Clone — temporarily take the shaped lines out of the
        // entity for painting, then put them back for mouse mapping.
        let (lines, indents, line_height, scroll, scroll_left) =
            self.input.update(cx, |input, _| {
                (
                    std::mem::take(&mut input.last_lines),
                    input.line_indents.clone(),
                    input.line_height,
                    input.scroll_top,
                    input.scroll_left,
                )
            });

        let paint_bounds = self.input.read(cx).paint_bounds(bounds);
        window.with_content_mask(
            Some(gpui::ContentMask {
                bounds: paint_bounds,
            }),
            |window| {
                let theme = Theme::of(cx).clone();
                for (chip, icon) in prepaint.mention_chips.drain(..) {
                    paint_chip(window, chip, icon.as_ref(), &theme, cx);
                }
                for quad in prepaint.selection_quads.drain(..) {
                    window.paint_quad(quad);
                }
                let mut y = bounds.top() - px(scroll);
                for (line_ix, line) in lines.iter().enumerate() {
                    let height = line.size(line_height).height;
                    let indent = indents.get(line_ix).copied().unwrap_or_default();
                    if indent > px(0.0) && !line.wrap_boundaries().is_empty() {
                        for row in 0..=line.wrap_boundaries().len() {
                            let row_bounds = Bounds::new(
                                point(bounds.left(), y + line_height * row),
                                size(bounds.size.width, line_height),
                            );
                            window.with_content_mask(
                                Some(gpui::ContentMask { bounds: row_bounds }),
                                |window| {
                                    let _ = line.paint(
                                        point(
                                            bounds.left() - px(scroll_left)
                                                + if row > 0 { indent } else { px(0.0) },
                                            y,
                                        ),
                                        line_height,
                                        gpui::TextAlign::Left,
                                        Some(bounds),
                                        window,
                                        cx,
                                    );
                                },
                            );
                        }
                    } else {
                        let _ = line.paint(
                            point(bounds.left() - px(scroll_left), y),
                            line_height,
                            gpui::TextAlign::Left,
                            Some(bounds),
                            window,
                            cx,
                        );
                    }
                    y += height;
                }
                if let Some((ghost_origin, ghost)) = prepaint.ghost.take() {
                    let style = window.text_style();
                    let font_size = style.font_size.to_pixels(window.rem_size());
                    let run = TextRun {
                        len: ghost.len(),
                        font: style.font(),
                        color: Theme::of(cx).text_faint,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let line = window
                        .text_system()
                        .shape_line(ghost, font_size, &[run], None);
                    // (Clipping comes from the surrounding content mask.)
                    let _ = line.paint(
                        ghost_origin,
                        line_height,
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
                // Caret only when this input is actually focused in an active
                // window (Electron hides it on window deactivation too), and only
                // in the "on" blink phase — solid while typing, ~500ms blink idle.
                if self
                    .input
                    .update(cx, |input, cx| input.caret_shown(window, cx))
                    && let Some(cursor) = prepaint.cursor.take()
                {
                    window.paint_quad(cursor);
                }
            },
        );
        self.input.update(cx, |input, _| {
            input.last_lines = lines;
        });
    }
}

impl Render for ComposerInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.mentions_enabled && self.syntax_source != self.content {
            self.syntax_source = self.content.clone();
            self.syntax_spans.clear();
            let source = self.syntax_source.clone();
            // Keep grammar loading and parsing off the input/paint thread. A
            // dropped task and source check prevent stale edits recoloring text.
            self.syntax_task = Some(cx.spawn(async move |input, cx| {
                let (source, spans) = cx
                    .background_executor()
                    .spawn(async move {
                        let spans = composer_markdown::syntax_spans(&source);
                        (source, spans)
                    })
                    .await;
                input
                    .update(cx, |input, cx| {
                        if input.content == source {
                            input.syntax_spans = spans;
                            input.needs_measure = true;
                            cx.notify();
                        }
                    })
                    .ok();
            }));
        }
        let theme = Theme::of(cx);
        let popup_theme = theme.for_popup();
        let theme = if self.key_context == "PaletteSearch"
            || self.accessibility_role == gpui::Role::SearchInput
        {
            &popup_theme
        } else {
            theme
        };
        let text_color = if self.content.is_empty() {
            theme.text_faint
        } else {
            theme.text
        };
        div()
            .id(("composer-input", cx.entity_id()))
            .role(self.accessibility_role)
            .aria_label(self.placeholder.clone())
            .aria_placeholder(self.placeholder.clone())
            .key_context(self.key_context)
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::doc_start))
            .on_action(cx.listener(Self::doc_end))
            .on_action(cx.listener(Self::select_doc_start))
            .on_action(cx.listener(Self::select_doc_end))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::mention_tab))
            .on_action(cx.listener(Self::outdent_list))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::delete_to_line_start))
            .on_action(cx.listener(Self::delete_to_line_end))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::message_newline_or_accept))
            .on_action(cx.listener(Self::modified_submit))
            .on_action(cx.listener(Self::submit))
            .map(|element| input_host::decorate(element, cx))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, window, _| {
                // Capture runs before the clicked control handles the press, so
                // another input can take focus normally during bubbling.
                if event.button == MouseButton::Left && this.focus_handle.is_focused(window) {
                    window.blur();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .w_full()
            .text_size(crate::typography::ui_rems(self.text_size))
            .line_height(crate::typography::ui_rems(self.configured_line_height))
            .text_color(text_color)
            .font_family(theme.font_sans.clone())
            .child({
                let input = cx.entity();
                let ascent = self.max_ascent;
                crate::edge_fade::edge_faded(
                    INPUT_FADE_BAND,
                    true,
                    true,
                    ComposerTextElement {
                        input: input.clone(),
                        max_content_height: self
                            .viewport_height
                            .unwrap_or(TEXTAREA_MAX - TEXTAREA_PAD_V),
                    },
                )
                // Fade through the existing top padding, like the transcript
                // scrolling under its chrome. Account for GPUI's baseline
                // sampling without consuming another inset inside the text box.
                .inset_top(ascent - self.overflow_top_padding)
                .fade_overflow_y_with(move |cx| {
                    let input = input.read(cx);
                    let visible_height = input
                        .last_bounds
                        .map_or(0.0, |bounds| f32::from(bounds.size.height));
                    input_overflow_edges(
                        input.content_height,
                        input
                            .settled_viewport_height
                            .unwrap_or(TEXTAREA_MAX - TEXTAREA_PAD_V),
                        visible_height,
                        input.scroll_top,
                    )
                })
            })
    }
}
