//! Incrementally recognize reasoning content, tool-call markup, JSON output,
//! and stop sequences.

use std::sync::Arc;

use deepseek_recipe_core::special_tokens::{SpecialToken, SpecialTokens, default_special_tokens};

const JSON_FENCE_OPEN_MARKERS: [&str; 2] = ["```json\n", "```\n"];

/// The stage where parsing begins in thinking mode.
#[derive(Debug, Clone, Copy)]
pub enum ThinkingInitialStage {
    /// Begin the reasoning content of a new assistant turn, discarding leading
    /// newlines.
    ReasoningStart,
    /// Continue the reasoning content of a prefix continuation, preserving
    /// leading newlines.
    ReasoningContinuation,
    /// Continue the answer content of a prefix continuation whose reasoning
    /// content has ended, preserving leading newlines.
    ContentContinuation,
}

impl ThinkingInitialStage {
    /// Return whether parsing begins in reasoning content.
    pub fn starts_in_reasoning(&self) -> bool {
        matches!(self, Self::ReasoningStart | Self::ReasoningContinuation)
    }
}

const BARE_JSON_OPEN_MARKERS: [&str; 2] = ["{", "["];
const JSON_FENCE_CLOSE_MARKER: &str = "\n```";

/// Settings for parsing inference output.
#[derive(Debug, Clone)]
pub struct ParsingOptions {
    /// Parse model-specific tool-call markup.
    pub parse_tool_calls: bool,
    /// Begin parsing inside the tool-call block that the prompt opened, as for
    /// a required tool choice. Takes precedence over
    /// [`thinking_initial_stage`](Self::thinking_initial_stage).
    pub starts_in_tool_calls: bool,
    /// Recognize JSON output: a fenced block that starts with ```` ```json ````
    /// or ```` ``` ```` and a newline, or bare JSON that starts at the first `{`
    /// or `[`. Text outside the JSON becomes one space for each source chunk it
    /// spans. This does not validate JSON syntax.
    pub parse_json_output: bool,
    /// The stage where parsing begins in thinking mode. `None` means thinking
    /// mode is off: parsing begins in answer content and discards leading
    /// newlines. `Some` also makes the stream processor report reasoning
    /// tokens.
    pub thinking_initial_stage: Option<ThinkingInitialStage>,
    /// Match stop sequences in answer content and detected JSON output.
    /// Reasoning content, tool-call markup, and the text around JSON output are
    /// excluded. The parser discards empty sequences during construction.
    pub stop_sequences: Vec<String>,
    /// Mapping used to recognize the thinking-end token and tool-call markup.
    pub special_tokens: Arc<dyn SpecialTokens>,
    /// Keep newlines at the start of answer content instead of discarding
    /// them, so that replaying the answer reproduces the generated tokens.
    ///
    /// Downstream change for Psionic; `false` keeps the upstream behavior.
    pub preserves_answer_whitespace: bool,
}

impl Default for ParsingOptions {
    fn default() -> Self {
        Self {
            parse_tool_calls: true,
            starts_in_tool_calls: false,
            parse_json_output: false,
            thinking_initial_stage: None,
            stop_sequences: Vec::new(),
            special_tokens: default_special_tokens(),
            preserves_answer_whitespace: false,
        }
    }
}

/// The origin of a chunk of model output, which decides whether the
/// special-token segments of a marker match it.
///
/// A marker such as `</think>` or `<｜DSML｜invoke name="` consists of plain
/// segments and special-token segments. Model output that spells a special
/// token with plain tokens is content, not markup, so a special-token segment
/// matches only a decoded special token. Text supplied without token
/// boundaries has no such distinction, so its markers match by spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChunkKind {
    /// Text without token boundaries, such as [`InferenceChunk::Text`]: every
    /// segment matches its spelling.
    ///
    /// [`InferenceChunk::Text`]: super::InferenceChunk::Text
    Text,
    /// Text decoded from plain tokens: only plain segments match.
    PlainTokens,
    /// The text of one decoded special token: only special-token segments
    /// match.
    SpecialToken,
}

/// An incremental parser of model output. Each segment it returns assigns an
/// [`OutputAction`] to a number of input bytes.
pub(super) struct OutputParser {
    options: ParsingOptions,
    state: State,
}

impl OutputParser {
    pub(super) fn new(mut options: ParsingOptions) -> Self {
        options
            .stop_sequences
            .retain(|sequence| !sequence.is_empty());
        let stage = if options.starts_in_tool_calls {
            Stage::ToolCalls
        } else if let Some(initial_stage) = &options.thinking_initial_stage {
            match initial_stage {
                ThinkingInitialStage::ReasoningStart => Stage::Reasoning {
                    strips_leading_newlines: true,
                },
                ThinkingInitialStage::ReasoningContinuation => Stage::Reasoning {
                    strips_leading_newlines: false,
                },
                ThinkingInitialStage::ContentContinuation => Stage::Content {
                    strips_leading_newlines: false,
                },
            }
        } else {
            Stage::Content {
                strips_leading_newlines: !options.preserves_answer_whitespace,
            }
        };
        let state = State::new(stage, &options);
        Self { options, state }
    }

    /// Parse the next source chunk, retaining a partial marker match for later
    /// input. `kind` decides whether the special-token segments of a marker
    /// match the chunk.
    pub(super) fn feed(&mut self, content: &str, kind: ChunkKind) -> Vec<OutputActionSegment> {
        self.state.feed(content, kind, &self.options)
    }

    /// Return one segment for the bytes still buffered at the end of input,
    /// with the current stage's action for unmatched text.
    pub(super) fn finish(self) -> Vec<OutputActionSegment> {
        self.state.finish().1
    }
}

/// The kind of unexpected output that an [`OutputAction::SkipInvalid`] segment
/// skips.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum InvalidTokenKind {
    /// A thinking-end token outside reasoning content.
    ExtraEndOfThinking,
    /// Output after a stop sequence or the end of the tool-call block.
    ContentAfterFinished,
}

/// A label that the parser writes into tool call arguments while it turns
/// DSML parameters into a JSON object.
///
/// The variants that write equal text, `StringOpen` and `StringClose`, remain
/// distinct actions. The stream processor merges adjacent segments with equal
/// actions and writes a label once per segment, so an empty string value needs
/// two distinct actions to produce both of its quotes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ArgumentsLabel {
    /// `{` before the first argument.
    ObjectOpen,
    /// `, ` before each later argument.
    ArgumentSeparator,
    /// `: ` between an argument name and its value.
    KeyValueSeparator,
    /// `"` before a string value.
    StringOpen,
    /// `"` after a string value.
    StringClose,
}

impl ArgumentsLabel {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::ObjectOpen => "{",
            Self::ArgumentSeparator => ", ",
            Self::KeyValueSeparator => ": ",
            Self::StringOpen | Self::StringClose => "\"",
        }
    }
}

/// What the stream processor does with the bytes of one segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum OutputAction {
    /// Emit the bytes as answer content.
    Content,
    /// Emit one space for the bytes of each source chunk, for text around JSON
    /// output.
    ToSpace,
    /// Skip markup that the parser consumed.
    Skip,
    /// Skip unexpected output of the given kind.
    SkipInvalid { kind: InvalidTokenKind },
    /// Record the bytes as the matched stop sequence.
    StopSequence,
    /// An `invoke` tag opened a tool call.
    ToolCallBegin,
    /// Append the bytes to the tool name.
    ToolName,
    /// The tool name is complete.
    ToolNameEnd,
    /// Write a label into the tool call arguments, once per segment.
    LabelToolCallArguments(ArgumentsLabel),
    /// Write the bytes into the tool call arguments, escaped as JSON string
    /// content when `is_string_value` is set.
    ToolCallArgumentsText { is_string_value: bool },
    /// Close the tool call arguments with `closing_text`, once per segment.
    ToolCallArgumentsEnd { closing_text: &'static str },
    /// Emit the bytes as reasoning content.
    Reasoning,
    /// Emit a label as answer content, once per segment, such as the `{` that
    /// starts bare JSON output.
    Label(&'static str),
}

/// An action and the number of input bytes it applies to.
#[derive(Debug, Clone)]
pub(super) struct OutputActionSegment {
    pub(super) action: OutputAction,
    pub(super) byte_len: usize,
}

impl OutputActionSegment {
    pub(super) fn new(action: OutputAction, byte_len: usize) -> Self {
        Self { action, byte_len }
    }
}

#[derive(Debug, Clone, Copy)]
enum Stage {
    /// Answer content. `strips_leading_newlines` registers a branch that
    /// discards newlines before the first other byte.
    Content { strips_leading_newlines: bool },
    /// Inside JSON output.
    Json,
    /// After the closing fence of JSON output.
    AfterJson,
    /// Inside the tool-call block, between `invoke` tags.
    ToolCalls,
    /// The tool name inside an `invoke` tag, before its closing quote.
    ToolName,
    /// Inside an `invoke` tag, before a `parameter` tag or the closing tag.
    /// `is_first_argument` holds until the first argument is written.
    ToolCallArguments { is_first_argument: bool },
    /// The quoted argument name of a `parameter` tag.
    ToolCallArgumentName,
    /// The `string="true|false">` attribute of a `parameter` tag.
    ToolCallArgumentStringAttribute,
    /// The value of a `parameter` tag.
    ToolCallArgumentValue { is_string_value: bool },
    /// Reasoning content. See [`Stage::Content`] for `strips_leading_newlines`.
    Reasoning { strips_leading_newlines: bool },
    /// After a stop sequence or the end of the tool-call block.
    Finished,
}

#[derive(Debug)]
struct MatchBranch {
    state: MatchState,
    next_stage: Stage,
    action_on_matched: ActionOnMatched,
}

#[derive(Debug)]
struct MatchBranches(Vec<MatchBranch>);

impl MatchBranches {
    fn new() -> Self {
        Self(Vec::new())
    }
    /// Register a branch with any match pattern.
    fn add_branch(
        &mut self,
        pattern: MatchPattern,
        next_stage: Stage,
        action_on_matched: ActionOnMatched,
    ) {
        self.0.push(MatchBranch {
            state: pattern.new_state(),
            next_stage,
            action_on_matched,
        });
    }
    /// Register a marker given as text; every byte is a plain segment.
    fn add_literal_branch<T: ToString + ?Sized>(
        &mut self,
        marker: &T,
        next_stage: Stage,
        action_on_matched: ActionOnMatched,
    ) {
        self.add_branch(
            MatchPattern::Marker(plain_pattern(&marker.to_string())),
            next_stage,
            action_on_matched,
        );
    }
    /// Register a marker given as plain and special-token segments.
    fn add_segmented_branch(
        &mut self,
        marker: &[Segment<'_>],
        next_stage: Stage,
        action_on_matched: ActionOnMatched,
    ) {
        self.add_branch(
            MatchPattern::Marker(pattern(marker)),
            next_stage,
            action_on_matched,
        );
    }
    /// Register one branch per marker in `markers`, each given as text.
    fn add_literal_branches<T: ToString>(
        &mut self,
        markers: &[T],
        next_stage: Stage,
        action_on_matched: ActionOnMatched,
    ) {
        markers
            .iter()
            .for_each(|marker| self.add_literal_branch(marker, next_stage, action_on_matched));
    }
    /// Register the opening marker of the tool-call block, which newlines
    /// can precede.
    fn add_tool_calls_open_branch(&mut self, dsml_open_marker: &[Segment<'_>]) {
        self.add_branch(
            MatchPattern::NewlinesAndMarker(pattern(dsml_open_marker)),
            Stage::ToolCalls,
            ActionOnMatched::Skip,
        );
    }
}

/// One part of a marker: template text, or the spelling of a special token.
#[derive(Debug, Clone, Copy)]
enum Segment<'a> {
    Plain(&'a str),
    Special(&'a str),
}

/// A marker as the matcher compares it: each byte with whether it belongs to a
/// special-token segment.
type Pattern = Vec<(u8, bool)>;

fn pattern(segments: &[Segment<'_>]) -> Pattern {
    segments
        .iter()
        .flat_map(|segment| {
            let (text, is_special) = match segment {
                Segment::Plain(text) => (*text, false),
                Segment::Special(text) => (*text, true),
            };
            text.bytes().map(move |byte| (byte, is_special))
        })
        .collect()
}

fn plain_pattern(text: &str) -> Pattern {
    pattern(&[Segment::Plain(text)])
}

/// Return whether `byte` from a chunk of `kind` matches one byte of a pattern.
fn unit_matches((pattern_byte, is_special): (u8, bool), byte: u8, kind: ChunkKind) -> bool {
    pattern_byte == byte
        && match kind {
            ChunkKind::Text => true,
            ChunkKind::PlainTokens => !is_special,
            ChunkKind::SpecialToken => is_special,
        }
}

#[derive(Debug)]
struct State {
    stage: Stage,
    /// The markers of this stage, in priority order: when several branches
    /// match at the same byte, the first one registered wins.
    match_branches: MatchBranches,
    action_on_unmatched: ActionOnUnmatched,
    /// Number of fed bytes not yet assigned to a segment.
    buffered_len: usize,
}

type ActionOnMatched = OutputAction;
type ActionOnUnmatched = OutputAction;

impl State {
    fn new(stage: Stage, options: &ParsingOptions) -> Self {
        let dsml = options.special_tokens.token(SpecialToken::Dsml);
        let dsml_open_marker = [Segment::Plain("<"), Segment::Special(dsml)];
        let dsml_close_marker = [Segment::Plain("</"), Segment::Special(dsml)];
        let tool_calls_close_markers = [
            [
                Segment::Plain("</"),
                Segment::Special(dsml),
                Segment::Plain("tool_calls>"),
            ],
            [
                Segment::Plain("</"),
                Segment::Special(dsml),
                Segment::Plain(" calls>"),
            ],
        ];
        let thinking_end_marker = [Segment::Special(
            options.special_tokens.token(SpecialToken::ThinkingEnd),
        )];

        let mut match_branches = MatchBranches::new();
        let mut add_stop_sequence_branches = || {
            match_branches.add_literal_branches(
                &options.stop_sequences,
                Stage::Finished,
                ActionOnMatched::StopSequence,
            );
        };
        let action_on_unmatched = match stage {
            Stage::Content {
                strips_leading_newlines,
            } => {
                if options.parse_json_output {
                    match_branches.add_literal_branches(
                        &JSON_FENCE_OPEN_MARKERS,
                        Stage::Json,
                        ActionOnMatched::Skip,
                    );
                    for marker in BARE_JSON_OPEN_MARKERS {
                        match_branches.add_literal_branch(
                            marker,
                            Stage::Json,
                            ActionOnMatched::Label(marker),
                        );
                    }
                } else {
                    add_stop_sequence_branches();
                }
                if options.parse_tool_calls {
                    match_branches.add_tool_calls_open_branch(&dsml_open_marker);
                }
                if strips_leading_newlines {
                    match_branches.add_branch(
                        MatchPattern::LeadingNewline,
                        Stage::Content {
                            strips_leading_newlines: true,
                        },
                        ActionOnMatched::Skip,
                    );
                }
                match_branches.add_segmented_branch(
                    &thinking_end_marker,
                    Stage::Content {
                        strips_leading_newlines: false,
                    },
                    ActionOnMatched::SkipInvalid {
                        kind: InvalidTokenKind::ExtraEndOfThinking,
                    },
                );
                if options.parse_json_output {
                    ActionOnUnmatched::ToSpace
                } else {
                    ActionOnUnmatched::Content
                }
            }
            Stage::Json => {
                add_stop_sequence_branches();
                match_branches.add_literal_branch(
                    JSON_FENCE_CLOSE_MARKER,
                    Stage::AfterJson,
                    ActionOnMatched::Skip,
                );
                ActionOnUnmatched::Content
            }
            Stage::AfterJson => {
                if options.parse_tool_calls {
                    match_branches.add_tool_calls_open_branch(&dsml_open_marker);
                }
                ActionOnUnmatched::ToSpace
            }
            Stage::ToolCalls => {
                match_branches.add_literal_branch(
                    "invoke name=\"",
                    Stage::ToolName,
                    ActionOnMatched::ToolCallBegin,
                );
                for marker in &tool_calls_close_markers {
                    match_branches.add_segmented_branch(
                        marker,
                        Stage::Finished,
                        ActionOnMatched::Skip,
                    );
                }
                ActionOnUnmatched::Skip
            }
            Stage::ToolName => {
                match_branches.add_literal_branch(
                    "\"",
                    Stage::ToolCallArguments {
                        is_first_argument: true,
                    },
                    ActionOnMatched::ToolNameEnd,
                );
                ActionOnUnmatched::ToolName
            }
            Stage::ToolCallArguments { is_first_argument } => {
                match_branches.add_literal_branch(
                    "parameter name=",
                    Stage::ToolCallArgumentName,
                    if is_first_argument {
                        ActionOnMatched::LabelToolCallArguments(ArgumentsLabel::ObjectOpen)
                    } else {
                        ActionOnMatched::LabelToolCallArguments(ArgumentsLabel::ArgumentSeparator)
                    },
                );
                match_branches.add_segmented_branch(
                    &dsml_close_marker,
                    Stage::ToolCalls,
                    if is_first_argument {
                        ActionOnMatched::ToolCallArgumentsEnd { closing_text: "{}" }
                    } else {
                        ActionOnMatched::ToolCallArgumentsEnd { closing_text: "}" }
                    },
                );
                ActionOnUnmatched::Skip
            }
            Stage::ToolCallArgumentName => {
                match_branches.add_literal_branch(
                    " ",
                    Stage::ToolCallArgumentStringAttribute,
                    ActionOnMatched::LabelToolCallArguments(ArgumentsLabel::KeyValueSeparator),
                );
                ActionOnUnmatched::ToolCallArgumentsText {
                    is_string_value: false,
                }
            }
            Stage::ToolCallArgumentStringAttribute => {
                match_branches.add_literal_branch(
                    "true\">",
                    Stage::ToolCallArgumentValue {
                        is_string_value: true,
                    },
                    ActionOnMatched::LabelToolCallArguments(ArgumentsLabel::StringOpen),
                );
                match_branches.add_literal_branch(
                    "false\">",
                    Stage::ToolCallArgumentValue {
                        is_string_value: false,
                    },
                    ActionOnMatched::Skip,
                );
                ActionOnUnmatched::Skip
            }
            Stage::ToolCallArgumentValue { is_string_value } => {
                match_branches.add_segmented_branch(
                    &dsml_close_marker,
                    Stage::ToolCallArguments {
                        is_first_argument: false,
                    },
                    if is_string_value {
                        ActionOnMatched::LabelToolCallArguments(ArgumentsLabel::StringClose)
                    } else {
                        ActionOnMatched::Skip
                    },
                );
                ActionOnUnmatched::ToolCallArgumentsText { is_string_value }
            }
            Stage::Reasoning {
                strips_leading_newlines,
            } => {
                if options.parse_tool_calls {
                    match_branches.add_tool_calls_open_branch(&dsml_open_marker);
                }
                if strips_leading_newlines {
                    match_branches.add_branch(
                        MatchPattern::LeadingNewline,
                        Stage::Reasoning {
                            strips_leading_newlines,
                        },
                        ActionOnMatched::Skip,
                    );
                }
                match_branches.add_branch(
                    MatchPattern::NewlinesAndMarker(pattern(&thinking_end_marker)),
                    Stage::Content {
                        strips_leading_newlines: !options.preserves_answer_whitespace,
                    },
                    ActionOnMatched::Skip,
                );
                ActionOnUnmatched::Reasoning
            }
            Stage::Finished => ActionOnUnmatched::SkipInvalid {
                kind: InvalidTokenKind::ContentAfterFinished,
            },
        };
        Self {
            stage,
            match_branches,
            action_on_unmatched,
            buffered_len: 0,
        }
    }

    fn feed(
        &mut self,
        content: &str,
        kind: ChunkKind,
        options: &ParsingOptions,
    ) -> Vec<OutputActionSegment> {
        let mut output_actions = Vec::new();
        for byte in content.bytes() {
            self.buffered_len += 1;
            let mut matched_branch = None;
            for branch in self.match_branches.0.iter_mut() {
                if branch.state.feed(byte, kind) {
                    matched_branch = Some((
                        branch.state.matching_len(),
                        branch.next_stage,
                        branch.action_on_matched,
                    ));
                    break;
                }
            }
            if let Some((matching_len, next_stage, action_on_matched)) = matched_branch {
                if matching_len < self.buffered_len {
                    output_actions.push(OutputActionSegment::new(
                        self.action_on_unmatched,
                        self.buffered_len - matching_len,
                    ));
                }
                if matching_len > 0 {
                    output_actions.push(OutputActionSegment::new(action_on_matched, matching_len));
                }
                *self = State::new(next_stage, options);
            }
        }
        let matching_len = self
            .match_branches
            .0
            .iter()
            .map(|branch| branch.state.matching_len())
            .max()
            .unwrap_or(0);
        if matching_len < self.buffered_len {
            output_actions.push(OutputActionSegment::new(
                self.action_on_unmatched,
                self.buffered_len - matching_len,
            ));
            self.buffered_len = matching_len;
        }
        output_actions
    }

    fn finish(self) -> (Stage, Vec<OutputActionSegment>) {
        let mut output_actions = Vec::new();
        if self.buffered_len > 0 {
            output_actions.push(OutputActionSegment::new(
                self.action_on_unmatched,
                self.buffered_len,
            ));
        }

        (self.stage, output_actions)
    }
}

enum MatchPattern {
    /// A marker.
    Marker(Pattern),
    /// A marker that any number of newlines can precede; the newlines belong
    /// to the match.
    NewlinesAndMarker(Pattern),
    /// A newline as the first byte of the stage.
    LeadingNewline,
}

impl MatchPattern {
    fn new_state(self) -> MatchState {
        match self {
            Self::Marker(pattern) => {
                let kmp_table = kmp_table(&pattern);
                MatchState::Marker {
                    pattern,
                    kmp_table,
                    matched_len: 0,
                }
            }
            Self::NewlinesAndMarker(pattern) => {
                let kmp_table = kmp_table(&pattern);
                MatchState::NewlinesAndMarker {
                    pattern,
                    kmp_table,
                    matched_len: 0,
                    newline_count: 0,
                }
            }
            Self::LeadingNewline => MatchState::LeadingNewline {
                is_first_byte: true,
                matched: false,
            },
        }
    }
}

type KmpTable = Vec<usize>;

fn kmp_table<T: PartialEq>(pattern: &[T]) -> KmpTable {
    let mut table = vec![0; pattern.len()];
    let mut len = 0;
    let mut i = 1;
    while i < pattern.len() {
        if pattern[i] == pattern[len] {
            len += 1;
            table[i] = len;
            i += 1;
        } else if len != 0 {
            len = table[len - 1];
        } else {
            table[i] = 0;
            i += 1;
        }
    }
    table
}

#[derive(Debug)]
enum MatchState {
    Marker {
        pattern: Pattern,
        kmp_table: KmpTable,
        matched_len: usize,
    },
    NewlinesAndMarker {
        pattern: Pattern,
        kmp_table: KmpTable,
        matched_len: usize,
        newline_count: usize,
    },
    LeadingNewline {
        is_first_byte: bool,
        matched: bool,
    },
}

/// Advance a KMP match of `pattern` by one byte of a chunk of `kind`.
///
/// The failure table compares pattern bytes together with their special-token
/// flag, while a byte of a [`ChunkKind::Text`] chunk matches a pattern byte
/// regardless of the flag. The markers of this parser have no border whose
/// flags differ from the pattern start, so the table stays valid for text
/// chunks.
fn kmp_next(
    pattern: &[(u8, bool)],
    kmp_table: &[usize],
    mut matched_len: usize,
    byte: u8,
    kind: ChunkKind,
) -> usize {
    let matches = |matched_len: usize| {
        pattern
            .get(matched_len)
            .is_some_and(|&unit| unit_matches(unit, byte, kind))
    };
    while matched_len > 0 && !matches(matched_len) {
        matched_len = kmp_table[matched_len - 1];
    }
    if matches(matched_len) {
        matched_len += 1;
    }
    matched_len
}

impl MatchState {
    fn feed(&mut self, byte: u8, kind: ChunkKind) -> bool {
        match self {
            MatchState::Marker {
                pattern,
                kmp_table,
                matched_len,
            } => {
                *matched_len = kmp_next(pattern, kmp_table, *matched_len, byte, kind);
                *matched_len == pattern.len()
            }
            MatchState::NewlinesAndMarker {
                pattern,
                kmp_table,
                matched_len,
                newline_count,
            } => {
                if *matched_len == 0 && byte == b'\n' {
                    *newline_count += 1;
                } else {
                    let next = kmp_next(pattern, kmp_table, *matched_len, byte, kind);
                    if next <= *matched_len {
                        *newline_count = 0
                    }
                    *matched_len = next;
                }
                *matched_len == pattern.len()
            }
            MatchState::LeadingNewline {
                is_first_byte,
                matched,
            } => {
                if *is_first_byte {
                    *matched = byte == b'\n';
                }
                *is_first_byte = false;
                *matched
            }
        }
    }
    fn matching_len(&self) -> usize {
        match self {
            MatchState::Marker { matched_len, .. } => *matched_len,
            MatchState::NewlinesAndMarker {
                matched_len,
                newline_count,
                ..
            } => *matched_len + *newline_count,
            MatchState::LeadingNewline { matched, .. } => {
                if *matched {
                    1
                } else {
                    0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use deepseek_recipe_core::special_tokens::SpecialTokensMap;

    use super::*;

    fn custom_options() -> ParsingOptions {
        ParsingOptions {
            thinking_initial_stage: Some(ThinkingInitialStage::ReasoningStart),
            special_tokens: Arc::new(SpecialTokensMap {
                thinking_end: "<\\end>".to_string(),
                dsml: "!DSML!".to_string(),
                ..SpecialTokensMap::default()
            }),
            ..ParsingOptions::default()
        }
    }

    fn actions(options: ParsingOptions, text: &str) -> Vec<OutputAction> {
        let mut parser = OutputParser::new(options);
        parser
            .feed(text, ChunkKind::Text)
            .into_iter()
            .map(|segment| segment.action)
            .collect()
    }

    #[test]
    fn custom_reasoning_end_label_ends_reasoning() {
        let actions = actions(custom_options(), "reasoning<\\end>answer");
        assert!(actions.contains(&OutputAction::Reasoning));
        assert!(actions.contains(&OutputAction::Content));
        assert!(!actions.contains(&OutputAction::SkipInvalid {
            kind: InvalidTokenKind::ExtraEndOfThinking
        }));
    }

    #[test]
    fn default_reasoning_end_label_is_not_recognized_after_override() {
        let actions = actions(custom_options(), "</think>answer");
        assert!(actions.contains(&OutputAction::Reasoning));
        assert!(!actions.contains(&OutputAction::Content));
    }

    #[test]
    fn custom_dsml_label_enters_tool_call_parsing() {
        let actions = actions(custom_options(), "<!DSML!invoke name=\"f\"");
        assert!(actions.contains(&OutputAction::ToolCallBegin));
    }

    #[test]
    fn default_dsml_label_is_not_recognized_after_override() {
        let actions = actions(custom_options(), "<｜DSML｜invoke name=\"f\"");
        assert!(!actions.contains(&OutputAction::ToolCallBegin));
    }
}
