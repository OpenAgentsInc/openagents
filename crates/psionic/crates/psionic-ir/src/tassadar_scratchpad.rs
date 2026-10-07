use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable schema version for the bounded Tassadar scratchpad formatting lane.
pub const TASSADAR_SCRATCHPAD_FRAMEWORK_SCHEMA_VERSION: u16 = 1;

/// Scratchpad encoding admitted by the bounded executor-input pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarScratchpadEncoding {
    /// Plain prompt-plus-target sequence with no inserted scratchpad markers.
    FlatTrace,
    /// Duplicate target chunks behind explicit scratchpad delimiters before emitting them.
    DelimitedChunkScratchpad,
}

impl TassadarScratchpadEncoding {
    /// Returns the stable encoding label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FlatTrace => "flat_trace",
            Self::DelimitedChunkScratchpad => "delimited_chunk_scratchpad",
        }
    }
}

/// Controlled position-ID scheme admitted by the bounded executor-input pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarControlledPositionScheme {
    /// Plain monotonic absolute position IDs.
    AbsoluteMonotonic,
    /// Reset controlled positions at each scratchpad/output segment boundary.
    SegmentReset,
    /// Reset controlled positions and reserve low buckets for scratchpad markers.
    TraceSchemaBuckets,
}

impl TassadarControlledPositionScheme {
    /// Returns the stable scheme label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AbsoluteMonotonic => "absolute_monotonic",
            Self::SegmentReset => "segment_reset",
            Self::TraceSchemaBuckets => "trace_schema_buckets",
        }
    }
}

/// One token segment inside the scratchpad formatting framework.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarScratchpadSegmentKind {
    Prompt,
    Scratchpad,
    Output,
}

/// Public formatting config for one bounded scratchpad/position framework variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarScratchpadFormatConfig {
    pub schema_version: u16,
    pub encoding: TassadarScratchpadEncoding,
    pub position_scheme: TassadarControlledPositionScheme,
    pub scratchpad_segment_token_cap: u16,
}

impl TassadarScratchpadFormatConfig {
    /// Creates one bounded formatting config.
    #[must_use]
    pub fn new(
        encoding: TassadarScratchpadEncoding,
        position_scheme: TassadarControlledPositionScheme,
        scratchpad_segment_token_cap: u16,
    ) -> Self {
        Self {
            schema_version: TASSADAR_SCRATCHPAD_FRAMEWORK_SCHEMA_VERSION,
            encoding,
            position_scheme,
            scratchpad_segment_token_cap: scratchpad_segment_token_cap.max(1),
        }
    }
}

/// One formatted token under the scratchpad framework.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarScratchpadToken {
    pub token: String,
    pub segment_kind: TassadarScratchpadSegmentKind,
    pub absolute_position_id: u32,
    pub controlled_position_id: u32,
}

/// One fully formatted prompt-plus-target sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarScratchpadFormattedSequence {
    pub config: TassadarScratchpadFormatConfig,
    pub tokens: Vec<TassadarScratchpadToken>,
    pub position_reset_count: u32,
    pub sequence_digest: String,
}

impl TassadarScratchpadFormattedSequence {
    fn new(
        config: TassadarScratchpadFormatConfig,
        tokens: Vec<TassadarScratchpadToken>,
        position_reset_count: u32,
    ) -> Self {
        let mut sequence = Self {
            config,
            tokens,
            position_reset_count,
            sequence_digest: String::new(),
        };
        sequence.sequence_digest =
            stable_digest(b"tassadar_scratchpad_formatted_sequence|", &sequence);
        sequence
    }
}

/// Formats one prompt-plus-target sequence under the bounded scratchpad framework.
#[must_use]
pub fn format_tassadar_sequence_with_scratchpad(
    prompt_tokens: &[String],
    target_tokens: &[String],
    config: &TassadarScratchpadFormatConfig,
) -> TassadarScratchpadFormattedSequence {
    let mut raw_tokens = prompt_tokens
        .iter()
        .map(|token| (token.clone(), TassadarScratchpadSegmentKind::Prompt))
        .collect::<Vec<_>>();

    match config.encoding {
        TassadarScratchpadEncoding::FlatTrace => {
            raw_tokens.extend(
                target_tokens
                    .iter()
                    .map(|token| (token.clone(), TassadarScratchpadSegmentKind::Output)),
            );
        }
        TassadarScratchpadEncoding::DelimitedChunkScratchpad => {
            for chunk in target_tokens.chunks(config.scratchpad_segment_token_cap as usize) {
                raw_tokens.push((
                    String::from("<scratchpad_open>"),
                    TassadarScratchpadSegmentKind::Scratchpad,
                ));
                raw_tokens.extend(
                    chunk
                        .iter()
                        .map(|token| (token.clone(), TassadarScratchpadSegmentKind::Scratchpad)),
                );
                raw_tokens.push((
                    String::from("<emit>"),
                    TassadarScratchpadSegmentKind::Scratchpad,
                ));
                raw_tokens.extend(
                    chunk
                        .iter()
                        .map(|token| (token.clone(), TassadarScratchpadSegmentKind::Output)),
                );
                raw_tokens.push((
                    String::from("<scratchpad_close>"),
                    TassadarScratchpadSegmentKind::Scratchpad,
                ));
            }
        }
    }

    let mut tokens = Vec::with_capacity(raw_tokens.len());
    let mut controlled_position = 0_u32;
    let mut previous_segment: Option<TassadarScratchpadSegmentKind> = None;
    let mut position_reset_count = 0_u32;
    for (absolute_position_id, (token, segment_kind)) in raw_tokens.into_iter().enumerate() {
        let should_reset = match config.position_scheme {
            TassadarControlledPositionScheme::AbsoluteMonotonic => false,
            TassadarControlledPositionScheme::SegmentReset
            | TassadarControlledPositionScheme::TraceSchemaBuckets => {
                previous_segment.is_some() && previous_segment != Some(segment_kind)
            }
        };
        if should_reset {
            controlled_position = 0;
            position_reset_count = position_reset_count.saturating_add(1);
        }
        let controlled_position_id = match config.position_scheme {
            TassadarControlledPositionScheme::AbsoluteMonotonic => absolute_position_id as u32,
            TassadarControlledPositionScheme::SegmentReset => controlled_position,
            TassadarControlledPositionScheme::TraceSchemaBuckets => match segment_kind {
                TassadarScratchpadSegmentKind::Prompt => controlled_position,
                TassadarScratchpadSegmentKind::Scratchpad => controlled_position.saturating_add(64),
                TassadarScratchpadSegmentKind::Output => controlled_position.saturating_add(128),
            },
        };
        tokens.push(TassadarScratchpadToken {
            token,
            segment_kind,
            absolute_position_id: absolute_position_id as u32,
            controlled_position_id,
        });
        controlled_position = controlled_position.saturating_add(1);
        previous_segment = Some(segment_kind);
    }

    TassadarScratchpadFormattedSequence::new(config.clone(), tokens, position_reset_count)
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("scratchpad formatted sequence should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        format_tassadar_sequence_with_scratchpad, TassadarControlledPositionScheme,
        TassadarScratchpadEncoding, TassadarScratchpadFormatConfig, TassadarScratchpadSegmentKind,
    };

    #[test]
    fn scratchpad_segment_reset_framework_resets_controlled_positions() {
        let formatted = format_tassadar_sequence_with_scratchpad(
            &[String::from("<program>")],
            &[
                String::from("a"),
                String::from("b"),
                String::from("c"),
                String::from("d"),
                String::from("e"),
            ],
            &TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::DelimitedChunkScratchpad,
                TassadarControlledPositionScheme::SegmentReset,
                2,
            ),
        );

        assert!(formatted.position_reset_count > 0);
        let output_positions = formatted
            .tokens
            .iter()
            .filter(|token| token.segment_kind == TassadarScratchpadSegmentKind::Output)
            .map(|token| token.controlled_position_id)
            .collect::<Vec<_>>();
        assert_eq!(output_positions, vec![0, 1, 0, 1, 0]);
    }

    #[test]
    fn scratchpad_trace_schema_buckets_preserve_output_token_order() {
        let target_tokens = vec![String::from("x"), String::from("y"), String::from("z")];
        let formatted = format_tassadar_sequence_with_scratchpad(
            &[String::from("<program>")],
            target_tokens.as_slice(),
            &TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::DelimitedChunkScratchpad,
                TassadarControlledPositionScheme::TraceSchemaBuckets,
                2,
            ),
        );
        let outputs = formatted
            .tokens
            .iter()
            .filter(|token| token.segment_kind == TassadarScratchpadSegmentKind::Output)
            .map(|token| token.token.clone())
            .collect::<Vec<_>>();
        assert_eq!(outputs, target_tokens);
        assert!(formatted
            .tokens
            .iter()
            .any(|token| token.controlled_position_id >= 128));
    }
}
