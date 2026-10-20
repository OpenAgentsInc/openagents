//! Special tokens used in prompt rendering and output parsing.
//!
//! [`SpecialToken`] names each token, and [`SpecialTokens`] maps each name to
//! its spelling: the string written into the prompt and matched in model
//! output. [`DefaultSpecialTokens`] holds the spellings of V4 and V4.1; callers
//! can substitute their own mapping.

use std::sync::Arc;

/// A kind of special token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SpecialToken {
    /// Marks the start of the prompt.
    Bos,
    /// Ends an assistant message.
    Eos,
    /// Starts the reasoning content of an assistant turn.
    ThinkingStart,
    /// Ends the reasoning content of an assistant turn.
    ThinkingEnd,
    /// Starts a system message.
    System,
    /// Starts a user message.
    User,
    /// Starts an assistant message.
    Assistant,
    /// Starts the latest reminder message.
    LatestReminder,
    /// Token written before each DSML tag name, as in `<｜DSML｜invoke>`.
    Dsml,
    /// Image token, written into the prompt as the placeholder of each image
    /// source.
    Image,
    /// Opens a grounding reference in assistant content or reasoning content.
    Ref,
    /// Closes a grounding reference in assistant content or reasoning content.
    RefEnd,
    /// Opens a grounding box in assistant content or reasoning content.
    Box,
    /// Closes a grounding box in assistant content or reasoning content.
    BoxEnd,
    /// Opens a grounding point in assistant content or reasoning content.
    Point,
    /// Closes a grounding point in assistant content or reasoning content.
    PointEnd,
    /// Opens a grounding polygon in assistant content or reasoning content.
    Polygon,
    /// Closes a grounding polygon in assistant content or reasoning content.
    PolygonEnd,
}

impl SpecialToken {
    /// Return the spelling of this token in V4 and V4.1.
    ///
    /// This is the value [`DefaultSpecialTokens`] returns and the default value
    /// of the corresponding [`SpecialTokensMap`] field.
    pub const fn default_str(self) -> &'static str {
        match self {
            Self::Bos => "<｜begin▁of▁sentence｜>",
            Self::Eos => "<｜end▁of▁sentence｜>",
            Self::ThinkingStart => "<think>",
            Self::ThinkingEnd => "</think>",
            Self::System => "<｜System｜>",
            Self::User => "<｜User｜>",
            Self::Assistant => "<｜Assistant｜>",
            Self::LatestReminder => "<｜latest_reminder｜>",
            Self::Dsml => "｜DSML｜",
            Self::Image => "<｜image｜>",
            Self::Ref => "<｜ref｜>",
            Self::RefEnd => "<｜/ref｜>",
            Self::Box => "<｜box｜>",
            Self::BoxEnd => "<｜/box｜>",
            Self::Point => "<｜point｜>",
            Self::PointEnd => "<｜/point｜>",
            Self::Polygon => "<｜polygon｜>",
            Self::PolygonEnd => "<｜/polygon｜>",
        }
    }
}

/// A mapping from each [`SpecialToken`] to its spelling in prompts and model
/// output.
///
/// Implement this to replace the whole mapping. [`DefaultSpecialTokens`] and
/// the default [`SpecialTokensMap`] supply the V4 and V4.1 spellings.
///
/// # Model differences
///
/// V4 rendering writes no system token regardless of the mapping, so
/// overriding [`System`](SpecialToken::System) affects V4.1 rendering only.
pub trait SpecialTokens: std::fmt::Debug + Send + Sync {
    /// Return the spelling of `token`.
    fn token(&self, token: SpecialToken) -> &str;
}

/// The special token mapping with the V4 and V4.1 spellings.
#[derive(Debug, Default)]
pub struct DefaultSpecialTokens;

impl SpecialTokens for DefaultSpecialTokens {
    fn token(&self, token: SpecialToken) -> &str {
        token.default_str()
    }
}

impl SpecialTokens for Arc<dyn SpecialTokens> {
    fn token(&self, token: SpecialToken) -> &str {
        self.as_ref().token(token)
    }
}

/// Return the default special token mapping.
pub fn default_special_tokens() -> Arc<dyn SpecialTokens> {
    Arc::new(DefaultSpecialTokens)
}

/// A [`SpecialTokens`] mapping with one string field per token.
///
/// Every field defaults to the corresponding [`DefaultSpecialTokens`] value, so
/// a caller can override only the tokens it needs.
#[derive(Debug, Clone)]
pub struct SpecialTokensMap {
    pub bos: String,
    pub eos: String,
    pub thinking_start: String,
    pub thinking_end: String,
    pub system: String,
    pub user: String,
    pub assistant: String,
    pub latest_reminder: String,
    pub dsml: String,
    pub image: String,
    pub r#ref: String,
    pub ref_end: String,
    pub r#box: String,
    pub box_end: String,
    pub point: String,
    pub point_end: String,
    pub polygon: String,
    pub polygon_end: String,
}

impl Default for SpecialTokensMap {
    fn default() -> Self {
        Self {
            bos: SpecialToken::Bos.default_str().to_string(),
            eos: SpecialToken::Eos.default_str().to_string(),
            thinking_start: SpecialToken::ThinkingStart.default_str().to_string(),
            thinking_end: SpecialToken::ThinkingEnd.default_str().to_string(),
            system: SpecialToken::System.default_str().to_string(),
            user: SpecialToken::User.default_str().to_string(),
            assistant: SpecialToken::Assistant.default_str().to_string(),
            latest_reminder: SpecialToken::LatestReminder.default_str().to_string(),
            dsml: SpecialToken::Dsml.default_str().to_string(),
            image: SpecialToken::Image.default_str().to_string(),
            r#ref: SpecialToken::Ref.default_str().to_string(),
            ref_end: SpecialToken::RefEnd.default_str().to_string(),
            r#box: SpecialToken::Box.default_str().to_string(),
            box_end: SpecialToken::BoxEnd.default_str().to_string(),
            point: SpecialToken::Point.default_str().to_string(),
            point_end: SpecialToken::PointEnd.default_str().to_string(),
            polygon: SpecialToken::Polygon.default_str().to_string(),
            polygon_end: SpecialToken::PolygonEnd.default_str().to_string(),
        }
    }
}

impl SpecialTokens for SpecialTokensMap {
    fn token(&self, token: SpecialToken) -> &str {
        match token {
            SpecialToken::Bos => &self.bos,
            SpecialToken::Eos => &self.eos,
            SpecialToken::ThinkingStart => &self.thinking_start,
            SpecialToken::ThinkingEnd => &self.thinking_end,
            SpecialToken::System => &self.system,
            SpecialToken::User => &self.user,
            SpecialToken::Assistant => &self.assistant,
            SpecialToken::LatestReminder => &self.latest_reminder,
            SpecialToken::Dsml => &self.dsml,
            SpecialToken::Image => &self.image,
            SpecialToken::Ref => &self.r#ref,
            SpecialToken::RefEnd => &self.ref_end,
            SpecialToken::Box => &self.r#box,
            SpecialToken::BoxEnd => &self.box_end,
            SpecialToken::Point => &self.point,
            SpecialToken::PointEnd => &self.point_end,
            SpecialToken::Polygon => &self.polygon,
            SpecialToken::PolygonEnd => &self.polygon_end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_TOKENS: [SpecialToken; 18] = [
        SpecialToken::Bos,
        SpecialToken::Eos,
        SpecialToken::ThinkingStart,
        SpecialToken::ThinkingEnd,
        SpecialToken::System,
        SpecialToken::User,
        SpecialToken::Assistant,
        SpecialToken::LatestReminder,
        SpecialToken::Dsml,
        SpecialToken::Image,
        SpecialToken::Ref,
        SpecialToken::RefEnd,
        SpecialToken::Box,
        SpecialToken::BoxEnd,
        SpecialToken::Point,
        SpecialToken::PointEnd,
        SpecialToken::Polygon,
        SpecialToken::PolygonEnd,
    ];

    #[test]
    fn default_str_matches_expected_values() {
        let expected = [
            (SpecialToken::Bos, "<｜begin▁of▁sentence｜>"),
            (SpecialToken::Eos, "<｜end▁of▁sentence｜>"),
            (SpecialToken::ThinkingStart, "<think>"),
            (SpecialToken::ThinkingEnd, "</think>"),
            (SpecialToken::System, "<｜System｜>"),
            (SpecialToken::User, "<｜User｜>"),
            (SpecialToken::Assistant, "<｜Assistant｜>"),
            (SpecialToken::LatestReminder, "<｜latest_reminder｜>"),
            (SpecialToken::Dsml, "｜DSML｜"),
            (SpecialToken::Image, "<｜image｜>"),
            (SpecialToken::Ref, "<｜ref｜>"),
            (SpecialToken::RefEnd, "<｜/ref｜>"),
            (SpecialToken::Box, "<｜box｜>"),
            (SpecialToken::BoxEnd, "<｜/box｜>"),
            (SpecialToken::Point, "<｜point｜>"),
            (SpecialToken::PointEnd, "<｜/point｜>"),
            (SpecialToken::Polygon, "<｜polygon｜>"),
            (SpecialToken::PolygonEnd, "<｜/polygon｜>"),
        ];
        for (token, expected) in expected {
            assert_eq!(token.default_str(), expected);
            assert_eq!(DefaultSpecialTokens.token(token), expected);
        }
    }

    #[test]
    fn map_defaults_match_the_default_impl() {
        let map = SpecialTokensMap::default();
        let tokens = DefaultSpecialTokens;
        for token in ALL_TOKENS {
            assert_eq!(map.token(token), tokens.token(token));
        }
    }

    #[test]
    fn map_overrides_single_token() {
        let map = SpecialTokensMap {
            user: "<user>".to_string(),
            ..SpecialTokensMap::default()
        };
        assert_eq!(map.token(SpecialToken::User), "<user>");
        assert_eq!(
            map.token(SpecialToken::Assistant),
            SpecialToken::Assistant.default_str()
        );
    }
}
