use std::collections::VecDeque;
use std::pin::pin;
use std::sync::Arc;

use async_stream::stream;
use tokio_stream::{Stream, StreamExt};

use super::decoder::{StreamDecoder, StreamTokenizer};
use super::inference::{
    CompletionUsage, FinishReason, InferenceChunk, InferenceFinishReason, PromptUsage,
};
use super::logprobs::TokenLogprob;
use super::output_parser::{
    ChunkKind, OutputAction, OutputActionSegment, OutputParser, ParsingOptions,
};
use super::{ChunkGenerator, OutputChunk, StreamError};

/// Parse inference chunks and produce protocol response chunks.
pub struct StreamProcessor<G> {
    generator: G,
    options: ParsingOptions,
    decoder: Option<StreamDecoder>,
}

impl<G> StreamProcessor<G>
where
    G: ChunkGenerator,
{
    /// Combine a protocol chunk generator with output parsing options.
    pub fn new(generator: G, options: ParsingOptions) -> Self {
        Self {
            generator,
            options,
            decoder: None,
        }
    }

    /// Decode the token IDs of [`InferenceChunk::Tokens`] with `tokenizer`.
    ///
    /// Each special token, as [`StreamTokenizer::is_special_token`] reports
    /// it, is decoded as its own piece, so that the output parser recognizes
    /// `</think>` and the DSML tags only as tokens; the same text spelled with
    /// plain tokens is content. Text chunks carry no token boundaries, so
    /// their markers match by spelling.
    ///
    /// Token IDs that contribute text count as completion tokens, including the
    /// token IDs buffered while a multi-token character was incomplete. Token
    /// IDs still buffered at the end of input contribute neither text nor
    /// completion usage. Without a tokenizer, a `Tokens` chunk fails
    /// the stream with [`StreamError::MissingTokenizer`]. In thinking mode, the
    /// tokenizer also counts the reasoning tokens of the emitted reasoning
    /// content.
    pub fn with_tokenizer(mut self, tokenizer: impl StreamTokenizer + 'static) -> Self {
        self.decoder = Some(StreamDecoder::new(Box::new(tokenizer)));
        self
    }

    /// Consume inference chunks until a finish chunk, a matched stop sequence,
    /// or the end of `inference`, and produce protocol response chunks.
    ///
    /// A ready chunk received before output starts supplies the initial prompt
    /// usage and fingerprint. Without it, the start chunk uses zero prompt usage
    /// and no fingerprint. Later ready chunks do not update emitted metadata.
    /// Completion usage accumulates each processed chunk's token count, including
    /// the entire chunk containing a stop sequence; later chunks are not read.
    ///
    /// # Errors
    ///
    /// The stream yields one [`StreamError`] and ends, without a finish chunk,
    /// when a `Tokens` chunk arrives without a tokenizer
    /// ([`StreamError::MissingTokenizer`]), the tokenizer fails to decode
    /// ([`StreamError::Decode`]), or a `Tokens` chunk carries a log probability
    /// count that differs from its token ID count
    /// ([`StreamError::LogprobCountMismatch`]).
    pub fn into_protocol_stream(
        self,
        inference: impl Stream<Item = InferenceChunk> + Send,
    ) -> impl Stream<Item = Result<G::Chunk, StreamError>> + Send
    where
        G::Chunk: Send,
    {
        let mut generator = self.generator;
        let options = self.options;
        let thinking_mode = options.thinking_initial_stage.is_some();
        let mut decoder = self.decoder;
        stream! {
            let mut inference = pin!(inference);
            let mut parser = OutputParser::new(options);
            let mut pending = PendingOutput::new();

            let mut started = false;
            let mut prompt_usage = PromptUsage::default();
            let mut completion_usage = CompletionUsage::default();
            let mut backend_finish_reason = None;
            // The finish reason is decided after the parser's final flush.
            // Record whether a tool call was named by the time the backend
            // finished, so that what the flush emits afterwards does not turn
            // `stop` into `tool_calls`.
            let mut had_tool_calls_at_finish = false;
            let mut stop_sequence = None;

            while let Some(chunk) = inference.next().await {
                let pieces = match chunk {
                    InferenceChunk::Ready {
                        system_fingerprint,
                        prompt_usage: ready_usage,
                    } => {
                        prompt_usage = ready_usage;
                        if !started {
                            let start = OutputChunk::Start { system_fingerprint, prompt_usage };
                            for protocol_chunk in generator.generate(start).await {
                                yield Ok(protocol_chunk);
                            }
                            started = true;
                        }
                        continue;
                    }
                    InferenceChunk::Finish { finish_reason } => {
                        if !started {
                            let start = OutputChunk::Start {
                                system_fingerprint: None,
                                prompt_usage,
                            };
                            for protocol_chunk in generator.generate(start).await {
                                yield Ok(protocol_chunk);
                            }
                            started = true;
                        }
                        backend_finish_reason = Some(finish_reason);
                        had_tool_calls_at_finish = pending.has_tool_calls;
                        break;
                    }
                    InferenceChunk::Text {
                        content,
                        token_count,
                    } => vec![OutputPiece {
                        content,
                        token_count,
                        logprobs: None,
                        kind: ChunkKind::Text,
                    }],
                    InferenceChunk::Tokens { token_ids, logprobs } => {
                        let Some(decoder) = decoder.as_mut() else {
                            yield Err(StreamError::MissingTokenizer);
                            return;
                        };
                        match decoder.decode_incrementally(&token_ids, logprobs) {
                            Ok(decoded) => decoded
                                .into_iter()
                                .map(|decoded| OutputPiece {
                                    content: decoded.content,
                                    token_count: decoded.token_count,
                                    logprobs: decoded.logprobs.map(Arc::from),
                                    kind: if decoded.is_special_token {
                                        ChunkKind::SpecialToken
                                    } else {
                                        ChunkKind::PlainTokens
                                    },
                                })
                                .collect(),
                            Err(error) => {
                                yield Err(error);
                                return;
                            }
                        }
                    }
                };
                if pieces.is_empty() {
                    continue;
                }
                if !started {
                    let start = OutputChunk::Start { system_fingerprint: None, prompt_usage };
                    for protocol_chunk in generator.generate(start).await {
                        yield Ok(protocol_chunk);
                    }
                    started = true;
                }
                for piece in pieces {
                    completion_usage.completion_tokens += piece.token_count;
                    let actions = parser.feed(&piece.content, piece.kind);
                    pending.push(piece.content, piece.logprobs);
                    for protocol_chunk in pending.apply_actions(actions, &mut generator).await {
                        yield Ok(protocol_chunk);
                    }
                    stop_sequence = pending.take_stop_sequence();
                    if stop_sequence.is_some() {
                        break;
                    }
                }
                if stop_sequence.is_some() {
                    break;
                }
            }

            if !started {
                let start = OutputChunk::Start { system_fingerprint: None, prompt_usage };
                for protocol_chunk in generator.generate(start).await {
                    yield Ok(protocol_chunk);
                }
            }
            let actions = parser.finish();
            for protocol_chunk in pending.apply_actions(actions, &mut generator).await {
                yield Ok(protocol_chunk);
            }
            let finish_reason = if stop_sequence.is_some() {
                FinishReason::StopSequence
            } else {
                match backend_finish_reason {
                    Some(InferenceFinishReason::Stop) if had_tool_calls_at_finish => {
                        FinishReason::ToolCalls
                    }
                    Some(InferenceFinishReason::Stop) => FinishReason::Stop,
                    Some(InferenceFinishReason::Length) => FinishReason::Length,
                    Some(InferenceFinishReason::ContentFilter) => FinishReason::ContentFilter,
                    None => FinishReason::EndOfStream,
                }
            };
            if thinking_mode {
                completion_usage.reasoning_tokens = decoder.as_ref().and_then(|decoder| {
                    decoder.tokenizer().count_tokens(&pending.reasoning_content)
                });
            }
            let finish = OutputChunk::Finish {
                finish_reason,
                stop_sequence,
                completion_usage,
            };
            for protocol_chunk in generator.generate(finish).await {
                yield Ok(protocol_chunk);
            }
        }
    }
}

/// One piece of model output ready for parsing: the text of one text chunk,
/// of the plain tokens of one token chunk, or of one special token.
struct OutputPiece {
    content: String,
    token_count: usize,
    logprobs: Option<Arc<[TokenLogprob]>>,
    kind: ChunkKind,
}

/// Output text waiting for its parsing actions, with the log probabilities of
/// the text it was split from.
struct BufferedChunk {
    content: String,
    logprobs: Option<Arc<[TokenLogprob]>>,
}

/// Output text that the parser has not fully assigned yet, and the state
/// gathered while applying actions to it.
struct PendingOutput {
    chunks: VecDeque<BufferedChunk>,
    /// The last segment of the previous [`Self::apply_actions`] call, kept
    /// open so that a following segment with the same action extends it.
    pending_segment: Option<OutputActionSegment>,
    tool_name_buffer: String,
    /// The action of the previously taken piece. A segment that spans several
    /// buffered chunks is taken in several pieces, and the actions that write
    /// a label or a tool call boundary act only on the first piece. `Skip`
    /// is the initial value because it differs from every action that checks it.
    previous_piece_action: OutputAction,
    stop_sequence: Option<String>,
    has_tool_calls: bool,
    /// The emitted reasoning content.
    reasoning_content: String,
}

impl PendingOutput {
    fn new() -> Self {
        Self {
            chunks: VecDeque::new(),
            pending_segment: None,
            tool_name_buffer: String::new(),
            previous_piece_action: OutputAction::Skip,
            stop_sequence: None,
            has_tool_calls: false,
            reasoning_content: String::new(),
        }
    }

    fn push(&mut self, content: String, logprobs: Option<Arc<[TokenLogprob]>>) {
        self.chunks.push_back(BufferedChunk { content, logprobs });
    }

    fn take_stop_sequence(&mut self) -> Option<String> {
        self.stop_sequence.take()
    }

    /// Take the first `segment.byte_len` bytes of the front buffered chunk and
    /// convert them by the segment's action. `segment.byte_len` must not exceed
    /// the length of the front chunk. Text split off from a chunk keeps the
    /// chunk's log probabilities.
    fn take_piece(&mut self, segment: OutputActionSegment) -> Option<OutputChunk> {
        let BufferedChunk {
            content: mut piece,
            logprobs,
        } = self.chunks.pop_front()?;
        if segment.byte_len < piece.len() {
            let remaining = piece.split_off(segment.byte_len);
            self.chunks.push_front(BufferedChunk {
                content: remaining,
                logprobs: logprobs.clone(),
            });
        }
        let is_first_piece = self.previous_piece_action != segment.action;
        self.previous_piece_action = segment.action;

        match segment.action {
            OutputAction::Content => Some(OutputChunk::Content {
                content: piece,
                logprobs,
            }),
            OutputAction::Reasoning => {
                self.reasoning_content.push_str(&piece);
                Some(OutputChunk::Reasoning {
                    content: piece,
                    logprobs,
                })
            }
            OutputAction::ToSpace => Some(OutputChunk::Content {
                content: " ".to_string(),
                logprobs,
            }),
            OutputAction::Skip | OutputAction::SkipInvalid { .. } => None,
            OutputAction::StopSequence => {
                self.stop_sequence
                    .get_or_insert_with(String::new)
                    .push_str(&piece);
                None
            }
            OutputAction::ToolCallBegin => is_first_piece.then_some(OutputChunk::ToolCallBegin),
            OutputAction::ToolName => {
                self.tool_name_buffer.push_str(&piece);
                None
            }
            OutputAction::ToolNameEnd => is_first_piece.then(|| {
                self.has_tool_calls = true;
                OutputChunk::ToolCallNamed {
                    tool_name: std::mem::take(&mut self.tool_name_buffer),
                }
            }),
            OutputAction::LabelToolCallArguments(label) => {
                is_first_piece.then(|| OutputChunk::ToolCallArgumentsDelta {
                    arguments: label.as_str().to_string(),
                })
            }
            OutputAction::ToolCallArgumentsText { is_string_value } => {
                let arguments = if is_string_value {
                    escape_json_string(piece)
                } else {
                    piece
                };
                Some(OutputChunk::ToolCallArgumentsDelta { arguments })
            }
            OutputAction::ToolCallArgumentsEnd { closing_text } => {
                is_first_piece.then(|| OutputChunk::ToolCallArgumentsDelta {
                    arguments: closing_text.to_string(),
                })
            }
            OutputAction::Label(label) => is_first_piece.then(|| OutputChunk::Content {
                content: label.to_string(),
                logprobs,
            }),
        }
    }

    /// Take every buffered chunk that `segment` covers completely, and reduce
    /// `segment` by their length.
    fn apply_whole_chunks(
        &mut self,
        segment: &mut OutputActionSegment,
        outputs: &mut Vec<OutputChunk>,
    ) {
        while let Some(front_len) = self
            .chunks
            .front()
            .map(|chunk| chunk.content.len())
            .filter(|&front_len| front_len <= segment.byte_len)
        {
            if let Some(chunk) =
                self.take_piece(OutputActionSegment::new(segment.action, front_len))
            {
                outputs.push(chunk);
            }
            segment.byte_len -= front_len;
        }
    }

    /// Apply the parser's action segments to the buffered chunks and generate
    /// protocol chunks. The last segment stays pending, after taking the
    /// chunks it covers completely, because the next parser output can extend
    /// it.
    async fn apply_actions<G>(
        &mut self,
        actions: Vec<OutputActionSegment>,
        generator: &mut G,
    ) -> Vec<G::Chunk>
    where
        G: ChunkGenerator,
    {
        let mut outputs = Vec::new();
        let mut pending_segment = self.pending_segment.take();
        for segment in actions {
            if let Some(pending_segment) = &mut pending_segment {
                if segment.action == pending_segment.action {
                    pending_segment.byte_len += segment.byte_len;
                } else {
                    self.apply_whole_chunks(pending_segment, &mut outputs);
                    if pending_segment.byte_len > 0
                        && let Some(chunk) = self.take_piece(pending_segment.clone())
                    {
                        outputs.push(chunk);
                    }
                    *pending_segment = segment;
                }
            } else {
                pending_segment = Some(segment);
            }
        }
        if let Some(pending_segment) = &mut pending_segment {
            self.apply_whole_chunks(pending_segment, &mut outputs);
        }
        self.pending_segment = pending_segment;

        let mut chunks = Vec::new();
        for output in outputs {
            chunks.extend(generator.generate(output).await);
        }
        chunks
    }
}

fn escape_json_string(content: String) -> String {
    match serde_json::to_string(&content) {
        Ok(escaped) if escaped.len() > 2 => escaped[1..escaped.len() - 1].to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use tokenizers::Tokenizer;

    use super::*;
    use crate::stream::{InferenceTokenLogprob, ThinkingInitialStage, TopLogprob};

    /// A chunk generator whose protocol chunks are the parsed output chunks.
    struct Passthrough;

    impl ChunkGenerator for Passthrough {
        type Chunk = OutputChunk;

        async fn generate(&mut self, chunk: OutputChunk) -> Vec<OutputChunk> {
            vec![chunk]
        }
    }

    /// The answer the model gives before the tool-call block in the reference
    /// scenarios.
    const PREAMBLE: &str = "Let's use get_current_weather to get weather🤭:";

    /// The tool-call block of the reference scenarios: an empty `invoke`, then
    /// one with a `string="true"` value that holds JSON text and a
    /// `string="false"` value that holds a JSON object.
    const TOOL_CALLS: &str = r#"<｜DSML｜tool_calls>
<｜DSML｜invoke name="get_current_weather">
</｜DSML｜invoke>
<｜DSML｜invoke name="get_current_weather">
<｜DSML｜parameter name="location" string="true">{"value": "Paris"}</｜DSML｜parameter>
<｜DSML｜parameter name="unit" string="false">{"value": "celsius"}</｜DSML｜parameter>
</｜DSML｜invoke>
</｜DSML｜tool_calls>"#;

    /// The tool calls that [`TOOL_CALLS`] parses into.
    const PARSED_TOOL_CALLS: [(&str, &str); 2] = [
        ("get_current_weather", "{}"),
        (
            "get_current_weather",
            r#"{"location": "{\"value\": \"Paris\"}", "unit": {"value": "celsius"}}"#,
        ),
    ];

    fn tokenizer() -> Tokenizer {
        // Downstream change: the 6 MB tokenizer is not vendored. Point
        // DEEPSEEK_RECIPE_V41_TOKENIZER at the checkpoint's tokenizer.json.
        let path = std::env::var("DEEPSEEK_RECIPE_V41_TOKENIZER")
            .expect("DEEPSEEK_RECIPE_V41_TOKENIZER names the V4.1 tokenizer.json");
        Tokenizer::from_file(path).expect("the V4.1 tokenizer loads")
    }

    fn encode(tokenizer: &Tokenizer, text: &str) -> Vec<u32> {
        tokenizer
            .encode(text, false)
            .expect("the text encodes")
            .get_ids()
            .to_vec()
    }

    fn reasoning_options() -> ParsingOptions {
        ParsingOptions {
            thinking_initial_stage: Some(ThinkingInitialStage::ReasoningStart),
            ..ParsingOptions::default()
        }
    }

    fn text(content: &str) -> InferenceChunk {
        InferenceChunk::Text {
            content: content.to_string(),
            token_count: 1,
        }
    }

    fn finish(finish_reason: InferenceFinishReason) -> InferenceChunk {
        InferenceChunk::Finish { finish_reason }
    }

    fn logprob(token_id: u32) -> InferenceTokenLogprob {
        InferenceTokenLogprob {
            logprob: -1.0,
            top_logprobs: vec![(token_id, -1.0)],
        }
    }

    fn processor(options: ParsingOptions) -> StreamProcessor<Passthrough> {
        StreamProcessor::new(Passthrough, options)
    }

    /// Run `chunks` through `processor` and return everything the stream
    /// yields.
    async fn run(
        processor: StreamProcessor<Passthrough>,
        chunks: Vec<InferenceChunk>,
    ) -> Vec<Result<OutputChunk, StreamError>> {
        processor
            .into_protocol_stream(tokio_stream::iter(chunks))
            .collect()
            .await
    }

    /// Run `chunks` through `processor`, expecting no stream error.
    async fn parse(
        processor: StreamProcessor<Passthrough>,
        chunks: Vec<InferenceChunk>,
    ) -> Vec<OutputChunk> {
        run(processor, chunks)
            .await
            .into_iter()
            .map(|chunk| chunk.expect("the stream succeeds"))
            .collect()
    }

    /// Parse the text chunks `contents` followed by a `Stop` finish.
    async fn parse_texts(contents: &[&str], options: ParsingOptions) -> Vec<OutputChunk> {
        let chunks = contents
            .iter()
            .map(|content| text(content))
            .chain([finish(InferenceFinishReason::Stop)])
            .collect();
        parse(processor(options), chunks).await
    }

    /// Parse one text chunk per character of `content`, as the reference
    /// implementation's mock backend produces, followed by a `Stop` finish.
    async fn parse_characters(content: &str, options: ParsingOptions) -> Vec<OutputChunk> {
        let characters: Vec<String> = content.chars().map(String::from).collect();
        let contents: Vec<&str> = characters.iter().map(String::as_str).collect();
        parse_texts(&contents, options).await
    }

    fn content_chunks(chunks: &[OutputChunk]) -> Vec<&str> {
        chunks
            .iter()
            .filter_map(|chunk| match chunk {
                OutputChunk::Content { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect()
    }

    fn reasoning_chunks(chunks: &[OutputChunk]) -> Vec<&str> {
        chunks
            .iter()
            .filter_map(|chunk| match chunk {
                OutputChunk::Reasoning { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect()
    }

    fn content(chunks: &[OutputChunk]) -> String {
        content_chunks(chunks).concat()
    }

    fn reasoning(chunks: &[OutputChunk]) -> String {
        reasoning_chunks(chunks).concat()
    }

    /// Return the tool calls as `(tool_name, arguments)` pairs, each opened by
    /// a `ToolCallBegin` chunk, named once, and assembled from its deltas.
    fn tool_calls(chunks: &[OutputChunk]) -> Vec<(String, String)> {
        let mut calls: Vec<(String, String)> = Vec::new();
        for chunk in chunks {
            match chunk {
                OutputChunk::ToolCallBegin => calls.push((String::new(), String::new())),
                OutputChunk::ToolCallNamed { tool_name } => {
                    let (name, _) = calls.last_mut().expect("a tool call is open");
                    assert!(name.is_empty(), "a tool call is named once");
                    name.clone_from(tool_name);
                }
                OutputChunk::ToolCallArgumentsDelta { arguments } => {
                    let (name, call_arguments) = calls.last_mut().expect("a tool call is open");
                    assert!(!name.is_empty(), "arguments follow the tool name");
                    call_arguments.push_str(arguments);
                }
                _ => {}
            }
        }
        calls
    }

    fn assert_tool_calls(chunks: &[OutputChunk], expected: &[(&str, &str)]) {
        let expected: Vec<(String, String)> = expected
            .iter()
            .map(|(name, arguments)| (name.to_string(), arguments.to_string()))
            .collect();
        assert_eq!(tool_calls(chunks), expected);
    }

    fn finish_chunk(chunks: &[OutputChunk]) -> (FinishReason, Option<&str>, CompletionUsage) {
        match chunks.last() {
            Some(OutputChunk::Finish {
                finish_reason,
                stop_sequence,
                completion_usage,
            }) => (*finish_reason, stop_sequence.as_deref(), *completion_usage),
            other => panic!("the stream ends with a finish chunk, not {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_stop_sequence_spanning_chunks_ends_output() {
        let options = ParsingOptions {
            stop_sequences: vec!["o worl".to_string()],
            ..ParsingOptions::default()
        };
        // The stop sequence completes in `ld`; `!` is not read.
        let chunks = parse_texts(&["he", "llo", " ", "wor", "ld", "!"], options).await;
        assert_eq!(content_chunks(&chunks), ["he", "ll"]);
        let (finish_reason, stop_sequence, completion_usage) = finish_chunk(&chunks);
        assert_eq!(finish_reason, FinishReason::StopSequence);
        assert_eq!(stop_sequence, Some("o worl"));
        assert_eq!(completion_usage.completion_tokens, 5);
    }

    #[tokio::test]
    async fn a_failed_stop_sequence_prefix_match_backtracks() {
        // After `abab`, the `c` completes `abc` from the second `ab`.
        let options = ParsingOptions {
            stop_sequences: vec!["abc".to_string()],
            ..ParsingOptions::default()
        };
        let chunks = parse_texts(&["ababc"], options).await;
        assert_eq!(content_chunks(&chunks), ["ab"]);
        assert_eq!(finish_chunk(&chunks).1, Some("abc"));
    }

    #[tokio::test]
    async fn json_output_is_recognized_in_fences_and_bare() {
        let options = || ParsingOptions {
            parse_json_output: true,
            ..ParsingOptions::default()
        };
        assert_eq!(
            content(&parse_characters("```json\n{}\n```", options()).await),
            "{}"
        );
        assert_eq!(
            content(&parse_characters("```\n{}\n```", options()).await),
            "{}"
        );
        // Text before bare JSON becomes one space per source chunk.
        assert_eq!(
            content(&parse_characters("json{}", options()).await),
            "    {}"
        );
        assert_eq!(
            content(&parse_characters("json[]", options()).await),
            "    []"
        );
    }

    #[tokio::test]
    async fn tool_call_arguments_become_a_json_object() {
        let block = r#"<｜DSML｜tool_calls>
<｜DSML｜invoke name="get_current_weather">
<｜DSML｜parameter name="location" string="true">Tokyo</｜DSML｜parameter>
<｜DSML｜parameter name="unit" string="true">celsius✅</｜DSML｜parameter>
</｜DSML｜invoke>
<｜DSML｜invoke name="get_current_weather">
<｜DSML｜parameter name="location" string="true"></｜DSML｜parameter>
<｜DSML｜parameter name="unit" string="true">celsius❌</｜DSML｜parameter>
</｜DSML｜invoke>
</｜DSML｜tool_calls>"#;
        let output = format!("{PREAMBLE}\n\n{block}");
        let chunks = parse_characters(&output, ParsingOptions::default()).await;
        assert_eq!(content(&chunks), PREAMBLE);
        assert_tool_calls(
            &chunks,
            &[
                (
                    "get_current_weather",
                    r#"{"location": "Tokyo", "unit": "celsius✅"}"#,
                ),
                (
                    "get_current_weather",
                    r#"{"location": "", "unit": "celsius❌"}"#,
                ),
            ],
        );
        assert_eq!(finish_chunk(&chunks).0, FinishReason::ToolCalls);
    }

    #[tokio::test]
    async fn spaced_tags_raw_json_values_and_empty_invokes_are_parsed() {
        let block = r#"<｜DSML｜ calls>
<｜DSML｜ invoke name="get_current_weather">
<｜DSML｜ parameter name="location" string="true">Tokyo</｜DSML｜ parameter>
<｜DSML｜ parameter name="unit" string="false">{"value": "celsius"}</｜DSML｜ parameter>
</｜DSML｜ invoke>
<｜DSML｜ invoke name="get_current_weather">
</｜DSML｜ invoke>
</｜DSML｜ calls>"#;
        let output = format!("{PREAMBLE}\n\n{block}");
        let chunks = parse_characters(&output, ParsingOptions::default()).await;
        assert_eq!(content(&chunks), PREAMBLE);
        assert_tool_calls(
            &chunks,
            &[
                (
                    "get_current_weather",
                    r#"{"location": "Tokyo", "unit": {"value": "celsius"}}"#,
                ),
                ("get_current_weather", "{}"),
            ],
        );
        assert_eq!(finish_chunk(&chunks).0, FinishReason::ToolCalls);
    }

    #[tokio::test]
    async fn json_text_in_a_string_value_is_escaped() {
        let output = format!("{PREAMBLE}\n\n{TOOL_CALLS}");
        let chunks = parse_characters(&output, ParsingOptions::default()).await;
        assert_eq!(content(&chunks), PREAMBLE);
        assert_tool_calls(&chunks, &PARSED_TOOL_CALLS);
    }

    #[tokio::test]
    async fn tool_calls_follow_reasoning_content() {
        // `</think>` directly before the tool-call block.
        let output = format!("{PREAMBLE}</think>\n\n{TOOL_CALLS}");
        let chunks = parse_characters(&output, reasoning_options()).await;
        assert_eq!(reasoning(&chunks), PREAMBLE);
        assert_eq!(content(&chunks), "");
        assert_tool_calls(&chunks, &PARSED_TOOL_CALLS);
        assert_eq!(finish_chunk(&chunks).0, FinishReason::ToolCalls);

        // Answer content between `</think>` and the tool-call block.
        let output = format!("{PREAMBLE}</think>{{some content}}\n\n{TOOL_CALLS}");
        let chunks = parse_characters(&output, reasoning_options()).await;
        assert_eq!(reasoning(&chunks), PREAMBLE);
        assert_eq!(content(&chunks), "{some content}");
        assert_tool_calls(&chunks, &PARSED_TOOL_CALLS);

        // The tool-call block ends reasoning content without `</think>`.
        let output = format!("{PREAMBLE}\n\n{TOOL_CALLS}");
        let chunks = parse_characters(&output, reasoning_options()).await;
        assert_eq!(reasoning(&chunks), PREAMBLE);
        assert_eq!(content(&chunks), "");
        assert_tool_calls(&chunks, &PARSED_TOOL_CALLS);
    }

    #[tokio::test]
    async fn a_second_thinking_end_token_is_skipped() {
        let output =
            format!("{PREAMBLE}</think>{{some content}}</think>{{some content}}\n\n{TOOL_CALLS}");
        let chunks = parse_characters(&output, reasoning_options()).await;
        assert_eq!(reasoning(&chunks), PREAMBLE);
        assert_eq!(content(&chunks), "{some content}{some content}");
        assert_tool_calls(&chunks, &PARSED_TOOL_CALLS);
    }

    #[tokio::test]
    async fn reasoning_content_ends_at_the_thinking_end_token() {
        let chunks = parse_texts(&["he", "llo", "</think>", "world"], reasoning_options()).await;
        assert_eq!(reasoning_chunks(&chunks), ["he", "llo"]);
        assert_eq!(content_chunks(&chunks), ["world"]);
    }

    #[tokio::test]
    async fn newlines_around_reasoning_content_are_dropped() {
        // The leading newlines of the reasoning content and of the answer
        // content and the newline before `</think>` are dropped; the newline
        // inside the reasoning content stays.
        let chunks = parse_texts(
            &["\n", "\nhe\n", "llo\n", "</think>", "\n", "\nworld"],
            reasoning_options(),
        )
        .await;
        assert_eq!(reasoning(&chunks), "he\nllo");
        assert_eq!(content(&chunks), "world");
    }

    #[tokio::test]
    async fn a_content_continuation_keeps_its_leading_newline() {
        let options = ParsingOptions {
            thinking_initial_stage: Some(ThinkingInitialStage::ContentContinuation),
            ..ParsingOptions::default()
        };
        let chunks = parse_texts(&["\nhe", "llo", "world"], options).await;
        assert_eq!(content_chunks(&chunks), ["\nhe", "llo", "world"]);
    }

    #[cfg_attr(
        not(feature = "upstream-tokenizer-tests"),
        ignore = "needs the V4.1 tokenizer in DEEPSEEK_RECIPE_V41_TOKENIZER"
    )]
    #[tokio::test]
    async fn a_log_probability_count_mismatch_fails_the_stream() {
        let tokenizer = tokenizer();
        let ids = encode(&tokenizer, "Hello world");
        assert_eq!(ids.len(), 2);
        let chunks = vec![
            InferenceChunk::Tokens {
                logprobs: Some(vec![logprob(ids[0])]),
                token_ids: ids,
            },
            finish(InferenceFinishReason::Stop),
        ];
        let processor = processor(ParsingOptions::default()).with_tokenizer(tokenizer);
        assert_eq!(
            run(processor, chunks).await,
            [Err(StreamError::LogprobCountMismatch {
                detail: "1 log probabilities for 2 token IDs".to_string(),
            })]
        );
    }

    #[cfg_attr(
        not(feature = "upstream-tokenizer-tests"),
        ignore = "needs the V4.1 tokenizer in DEEPSEEK_RECIPE_V41_TOKENIZER"
    )]
    #[tokio::test]
    async fn a_character_split_across_token_chunks_is_emitted_once_complete() {
        let tokenizer = tokenizer();
        // The V4.1 tokenizer splits U+1F92D (`f0 9f a4 ad`) into a token for
        // its first three bytes and a token for its last byte.
        let ids = encode(&tokenizer, "🤭");
        assert_eq!(ids.len(), 2);
        let chunks = ids
            .iter()
            .map(|&id| InferenceChunk::Tokens {
                token_ids: vec![id],
                logprobs: Some(vec![logprob(id)]),
            })
            .chain([finish(InferenceFinishReason::Stop)])
            .collect();
        let processor = processor(ParsingOptions::default()).with_tokenizer(tokenizer);
        let output = parse(processor, chunks).await;
        let [
            OutputChunk::Start { .. },
            OutputChunk::Content {
                content,
                logprobs: Some(logprobs),
            },
            OutputChunk::Finish {
                completion_usage, ..
            },
        ] = output.as_slice()
        else {
            panic!("one content chunk carries the character, not {output:?}");
        };
        assert_eq!(content, "🤭");
        let tokens: Vec<(&str, &[u8])> = logprobs
            .iter()
            .map(|logprob| (logprob.token.as_str(), logprob.bytes.as_slice()))
            .collect();
        assert_eq!(
            tokens,
            [
                ("\\xf0\\x9f\\xa4", &[0xf0, 0x9f, 0xa4][..]),
                ("\\xad", &[0xad][..]),
            ]
        );
        assert_eq!(completion_usage.completion_tokens, 2);
    }

    #[cfg_attr(
        not(feature = "upstream-tokenizer-tests"),
        ignore = "needs the V4.1 tokenizer in DEEPSEEK_RECIPE_V41_TOKENIZER"
    )]
    #[tokio::test]
    async fn top_log_probabilities_decode_their_token_ids() {
        let tokenizer = tokenizer();
        let ids = encode(&tokenizer, "Hello world");
        assert_eq!(ids.len(), 2);
        let (hello, world) = (ids[0], ids[1]);
        let chunks = vec![
            InferenceChunk::Tokens {
                token_ids: vec![hello],
                logprobs: Some(vec![InferenceTokenLogprob {
                    logprob: -0.1,
                    top_logprobs: vec![(hello, -0.1), (world, -2.0)],
                }]),
            },
            finish(InferenceFinishReason::Stop),
        ];
        let processor = processor(ParsingOptions::default()).with_tokenizer(tokenizer);
        let output = parse(processor, chunks).await;
        let Some(OutputChunk::Content {
            content,
            logprobs: Some(logprobs),
        }) = output.get(1)
        else {
            panic!("the token decodes into one content chunk, not {output:?}");
        };
        assert_eq!(content, "Hello");
        assert_eq!(
            logprobs.as_ref(),
            [TokenLogprob {
                token: "Hello".to_string(),
                logprob: -0.1,
                bytes: b"Hello".to_vec(),
                top_logprobs: vec![
                    TopLogprob {
                        token: "Hello".to_string(),
                        logprob: -0.1,
                        bytes: b"Hello".to_vec(),
                    },
                    TopLogprob {
                        token: " world".to_string(),
                        logprob: -2.0,
                        bytes: b" world".to_vec(),
                    },
                ],
            }]
        );
    }

    #[tokio::test]
    async fn output_truncated_inside_a_tool_call_keeps_the_length_finish_reason() {
        let block = "<｜DSML｜tool_calls>\n<｜DSML｜invoke name=\"f\">\n\
                     <｜DSML｜parameter name=\"a\" string=\"true\">tru";
        let chunks = vec![text(block), finish(InferenceFinishReason::Length)];
        let output = parse(processor(ParsingOptions::default()), chunks).await;
        assert_tool_calls(&output, &[("f", r#"{"a": "tru"#)]);
        assert_eq!(finish_chunk(&output).0, FinishReason::Length);

        // Truncated inside the tool name, the call is opened but never named.
        let block = "<｜DSML｜tool_calls>\n<｜DSML｜invoke name=\"get_cur";
        let chunks = vec![text(block), finish(InferenceFinishReason::Length)];
        let output = parse(processor(ParsingOptions::default()), chunks).await;
        assert_eq!(
            output,
            [
                OutputChunk::Start {
                    system_fingerprint: None,
                    prompt_usage: PromptUsage::default(),
                },
                OutputChunk::ToolCallBegin,
                OutputChunk::Finish {
                    finish_reason: FinishReason::Length,
                    stop_sequence: None,
                    completion_usage: CompletionUsage {
                        completion_tokens: 1,
                        reasoning_tokens: None,
                    },
                },
            ]
        );
    }

    #[tokio::test]
    async fn the_backend_finish_reason_is_reported() {
        // Without a finish chunk, the end of the inference stream is reported.
        let output = parse(processor(ParsingOptions::default()), vec![text("hi")]).await;
        assert_eq!(content(&output), "hi");
        assert_eq!(finish_chunk(&output).0, FinishReason::EndOfStream);

        let chunks = vec![text("hi"), finish(InferenceFinishReason::ContentFilter)];
        let output = parse(processor(ParsingOptions::default()), chunks).await;
        assert_eq!(finish_chunk(&output).0, FinishReason::ContentFilter);
    }

    #[cfg_attr(
        not(feature = "upstream-tokenizer-tests"),
        ignore = "needs the V4.1 tokenizer in DEEPSEEK_RECIPE_V41_TOKENIZER"
    )]
    #[tokio::test]
    async fn reasoning_tokens_are_counted_only_with_a_tokenizer() {
        let contents = ["think", "</think>", "answer"];
        let chunks = parse_texts(&contents, reasoning_options()).await;
        assert_eq!(
            finish_chunk(&chunks).2,
            CompletionUsage {
                completion_tokens: 3,
                reasoning_tokens: None,
            }
        );

        let tokenizer = tokenizer();
        let reasoning_token_count = encode(&tokenizer, "think").len();
        let chunks = contents
            .iter()
            .map(|content| text(content))
            .chain([finish(InferenceFinishReason::Stop)])
            .collect();
        let processor = processor(reasoning_options()).with_tokenizer(tokenizer);
        let output = parse(processor, chunks).await;
        assert_eq!(reasoning(&output), "think");
        assert_eq!(
            finish_chunk(&output).2,
            CompletionUsage {
                completion_tokens: 3,
                reasoning_tokens: Some(reasoning_token_count),
            }
        );
    }

    /// Encode `text` as plain tokens: the pieces are encoded separately so that
    /// a special-token spelling in `text` never becomes the special token.
    fn encode_plain(tokenizer: &Tokenizer, pieces: &[&str]) -> Vec<u32> {
        pieces
            .iter()
            .flat_map(|piece| encode(tokenizer, piece))
            .collect()
    }

    fn special_token_id(tokenizer: &Tokenizer, token: &str) -> u32 {
        let id = tokenizer
            .token_to_id(token)
            .expect("the special token exists");
        assert!(tokenizer.is_special_token(id));
        id
    }

    #[cfg_attr(
        not(feature = "upstream-tokenizer-tests"),
        ignore = "needs the V4.1 tokenizer in DEEPSEEK_RECIPE_V41_TOKENIZER"
    )]
    #[tokio::test]
    async fn only_a_thinking_end_token_ends_reasoning_content_in_token_output() {
        let tokenizer = tokenizer();
        let thinking_end = special_token_id(&tokenizer, "</think>");
        let think = encode(&tokenizer, "think");
        let answer = encode(&tokenizer, " answer");

        // The token ends reasoning content; its log probability belongs to the
        // marker, not to the reasoning content.
        let mut token_ids = think.clone();
        token_ids.push(thinking_end);
        token_ids.extend(&answer);
        let chunks = vec![
            InferenceChunk::Tokens {
                logprobs: Some(token_ids.iter().map(|&id| logprob(id)).collect()),
                token_ids,
            },
            finish(InferenceFinishReason::Stop),
        ];
        let with_tokens = processor(reasoning_options()).with_tokenizer(tokenizer.clone());
        let output = parse(with_tokens, chunks).await;
        assert_eq!(reasoning(&output), "think");
        assert_eq!(content(&output), " answer");
        let reasoning_logprobs: usize = output
            .iter()
            .filter_map(|chunk| match chunk {
                OutputChunk::Reasoning { logprobs, .. } => logprobs.as_ref().map(|l| l.len()),
                _ => None,
            })
            .sum();
        assert_eq!(reasoning_logprobs, think.len());
        assert_eq!(
            finish_chunk(&output).2.completion_tokens,
            think.len() + 1 + answer.len()
        );

        // The same text spelled with plain tokens stays reasoning content.
        let plain = encode_plain(&tokenizer, &["think", "</", "think>", " answer"]);
        assert!(!plain.contains(&thinking_end));
        let chunks = vec![
            InferenceChunk::Tokens {
                token_ids: plain,
                logprobs: None,
            },
            finish(InferenceFinishReason::Stop),
        ];
        let with_tokens = processor(reasoning_options()).with_tokenizer(tokenizer);
        let output = parse(with_tokens, chunks).await;
        assert_eq!(reasoning(&output), "think</think> answer");
        assert_eq!(content(&output), "");
    }

    #[cfg_attr(
        not(feature = "upstream-tokenizer-tests"),
        ignore = "needs the V4.1 tokenizer in DEEPSEEK_RECIPE_V41_TOKENIZER"
    )]
    #[tokio::test]
    async fn only_dsml_tokens_open_a_tool_call_in_token_output() {
        let tokenizer = tokenizer();
        let dsml = special_token_id(&tokenizer, "｜DSML｜");
        let call = |dsml_ids: &dyn Fn() -> Vec<u32>| {
            let mut ids = encode(&tokenizer, "hi\n<");
            ids.extend(dsml_ids());
            ids.extend(encode(&tokenizer, "tool_calls>\n<"));
            ids.extend(dsml_ids());
            ids.extend(encode(&tokenizer, "invoke name=\"f\">\n</"));
            ids.extend(dsml_ids());
            ids.extend(encode(&tokenizer, "invoke>\n</"));
            ids.extend(dsml_ids());
            ids.extend(encode(&tokenizer, "tool_calls>"));
            vec![
                InferenceChunk::Tokens {
                    token_ids: ids,
                    logprobs: None,
                },
                finish(InferenceFinishReason::Stop),
            ]
        };

        let with_tokens = processor(ParsingOptions::default()).with_tokenizer(tokenizer.clone());
        let output = parse(with_tokens, call(&|| vec![dsml])).await;
        assert_eq!(content(&output), "hi");
        assert_tool_calls(&output, &[("f", "{}")]);
        assert_eq!(finish_chunk(&output).0, FinishReason::ToolCalls);

        // The DSML spelling in plain tokens is content, not markup.
        let plain_dsml = encode_plain(&tokenizer, &["｜DS", "ML｜"]);
        assert!(!plain_dsml.contains(&dsml));
        let chunks = call(&|| plain_dsml.clone());
        let with_tokens = processor(ParsingOptions::default()).with_tokenizer(tokenizer);
        let output = parse(with_tokens, chunks).await;
        assert!(content(&output).starts_with("hi\n<｜DSML｜tool_calls>"));
        assert_tool_calls(&output, &[]);
        assert_eq!(finish_chunk(&output).0, FinishReason::Stop);
    }

    #[cfg_attr(
        not(feature = "upstream-tokenizer-tests"),
        ignore = "needs the V4.1 tokenizer in DEEPSEEK_RECIPE_V41_TOKENIZER"
    )]
    #[tokio::test]
    async fn an_incomplete_character_before_a_special_token_is_flushed() {
        let tokenizer = tokenizer();
        let thinking_end = special_token_id(&tokenizer, "</think>");
        let emoji = encode(&tokenizer, "🤭");
        assert!(emoji.len() > 1);
        // Only the first token of the character precedes the thinking end
        // token, so its bytes are decoded as the replacement character.
        let chunks = vec![
            InferenceChunk::Tokens {
                token_ids: vec![emoji[0], thinking_end],
                logprobs: None,
            },
            finish(InferenceFinishReason::Stop),
        ];
        let with_tokens = processor(reasoning_options()).with_tokenizer(tokenizer);
        let output = parse(with_tokens, chunks).await;
        assert_eq!(reasoning(&output), "\u{FFFD}");
        assert_eq!(content(&output), "");
        assert_eq!(finish_chunk(&output).2.completion_tokens, 2);
    }
}
