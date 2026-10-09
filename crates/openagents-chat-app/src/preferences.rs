//! The reading preferences a person sets in an OpenAgents app's Settings:
//! text size, reduced motion, notifications (#10021), sounds (#10474), and the
//! theme (#11028). Shared by the
//! desktop and the phones, so a size means the same drawn text on both.
//!
//! They live in the one settings file `openagents settings` and Coder read
//! (`coder::task::settings`, `~/.openagents/settings.json` on a computer),
//! as its `app` section beside `coder`:
//!
//! ```json
//! {
//!   "schema": "openagents.settings.v1",
//!   "app": { "text_size": "larger", "reduce_motion": true, "notifications": false, "sounds": true, "theme": "system" }
//! }
//! ```
//!
//! A missing file, section, or field means the default. A section that does
//! not parse (an unknown size, say) is read as the defaults: these are
//! presentation choices, and none of them opens anything up. The theme is
//! Coder Light or Coder Noir from the shared token table (`oa-tokens`);
//! `system` follows the computer's or phone's appearance.

pub use oa_tokens::ThemeChoice;
use rust_native::layout::{MarkdownMetrics, Metrics};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The settings file's section these live in.
pub const SECTION: &str = "app";

/// How large chats and pages draw their text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSize {
    Smaller,
    #[default]
    Default,
    Larger,
    Largest,
}

impl TextSize {
    /// Every size, smallest first.
    pub const ALL: [TextSize; 4] = [
        TextSize::Smaller,
        TextSize::Default,
        TextSize::Larger,
        TextSize::Largest,
    ];

    /// The name a person reads.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            TextSize::Smaller => "Smaller",
            TextSize::Default => "Default",
            TextSize::Larger => "Larger",
            TextSize::Largest => "Largest",
        }
    }

    /// The size as a percentage of the default.
    #[must_use]
    pub const fn percent(self) -> u16 {
        match self {
            TextSize::Smaller => 90,
            TextSize::Default => 100,
            TextSize::Larger => 115,
            TextSize::Largest => 130,
        }
    }

    /// `points` at this size, rounded to a whole point.
    #[must_use]
    pub fn scale(self, points: f32) -> f32 {
        (points * f32::from(self.percent()) / 100.0).round()
    }

    fn scale_u16(self, value: u16) -> u16 {
        ((u32::from(value) * u32::from(self.percent()) + 50) / 100) as u16
    }

    /// The chat transcript's metrics (the current scheme's
    /// [`crate::visual::Visual::transcript`], [`crate::visual::TRANSCRIPT`]
    /// in the dark scheme) at this size: text, line heights, and the gaps
    /// and padding around text grow together; the reading width and corner
    /// radii stay.
    #[must_use]
    pub fn transcript(self) -> Metrics {
        let base = crate::visual::current().transcript;
        let s = |value| self.scale_u16(value);
        Metrics {
            body_size: s(base.body_size),
            body_line_height: s(base.body_line_height),
            row_gap: s(base.row_gap),
            bubble_padding: s(base.bubble_padding),
            markdown: base.markdown.map(|markdown| MarkdownMetrics {
                headings: markdown.headings.map(|[size, line]| [s(size), s(line)]),
                code_size_half_points: s(markdown.code_size_half_points),
                code_line_height: s(markdown.code_line_height),
                code_header_height: s(markdown.code_header_height),
                code_label_size: s(markdown.code_label_size),
                code_padding_y: s(markdown.code_padding_y),
                ..markdown
            }),
            ..base
        }
    }
}

/// The preferences, as set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub text_size: TextSize,
    /// Keep moving pictures (the Grid behind the window) still, besides
    /// what the system's own "Reduce motion" asks.
    pub reduce_motion: bool,
    /// Show a notification when Coder asks for the person, finishes, or
    /// fails while the app is away.
    pub notifications: bool,
    /// Play a short sound ([`crate::cues`]) when Coder finishes, asks for
    /// the person, or fails. Off mutes every cue.
    pub sounds: bool,
    /// Coder Light, Coder Noir, or whichever the system's appearance is.
    pub theme: ThemeChoice,
}

impl Default for Preferences {
    fn default() -> Self {
        Preferences {
            text_size: TextSize::Default,
            reduce_motion: false,
            notifications: true,
            sounds: true,
            // The system's appearance (#11028). A surface that never sets
            // the theme seam (`visual::set_scheme`), the phones today,
            // still paints dark.
            theme: ThemeChoice::System,
        }
    }
}

/// One change a Settings screen makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    TextSize(TextSize),
    ReduceMotion(bool),
    Notifications(bool),
    Sounds(bool),
    Theme(ThemeChoice),
}

impl Preferences {
    /// Makes `change`; `true` when anything changed.
    pub fn apply(&mut self, change: Change) -> bool {
        let before = *self;
        match change {
            Change::TextSize(size) => self.text_size = size,
            Change::ReduceMotion(on) => self.reduce_motion = on,
            Change::Notifications(on) => self.notifications = on,
            Change::Sounds(on) => self.sounds = on,
            Change::Theme(choice) => self.theme = choice,
        }
        *self != before
    }

    /// The preferences in a settings file's top-level object: the defaults
    /// when the section is missing or does not parse.
    #[must_use]
    pub fn from_settings(settings: &Value) -> Preferences {
        settings
            .get(SECTION)
            .and_then(|section| serde_json::from_value(section.clone()).ok())
            .unwrap_or_default()
    }

    /// The preferences in a settings file's bytes (see
    /// [`Preferences::from_settings`]); the defaults when they are not JSON.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Preferences {
        serde_json::from_slice::<Value>(bytes)
            .map(|settings| Preferences::from_settings(&settings))
            .unwrap_or_default()
    }

    /// This section's value, for [`SECTION`] in the settings file.
    #[must_use]
    pub fn section(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_missing_or_broken_section_is_the_defaults() {
        let defaults = Preferences::default();
        assert!(defaults.notifications && defaults.sounds && !defaults.reduce_motion);
        assert_eq!(defaults.text_size, TextSize::Default);
        assert_eq!(defaults.theme, ThemeChoice::System, "follows the system");
        for settings in [
            json!({}),
            json!({"schema": "openagents.settings.v1", "coder": {}}),
            json!({"app": {"text_size": "gigantic"}}),
            json!({"app": 3}),
        ] {
            assert_eq!(Preferences::from_settings(&settings), defaults);
        }
        assert_eq!(Preferences::from_bytes(b"not json"), defaults);
        // A missing field keeps its default; the others are read.
        assert_eq!(
            Preferences::from_settings(&json!({"app": {"reduce_motion": true}})),
            Preferences {
                reduce_motion: true,
                ..defaults
            }
        );
    }

    #[test]
    fn a_section_reads_back_what_was_written() {
        let mut preferences = Preferences::default();
        assert!(preferences.apply(Change::TextSize(TextSize::Largest)));
        assert!(preferences.apply(Change::Notifications(false)));
        assert!(preferences.apply(Change::ReduceMotion(true)));
        assert!(!preferences.apply(Change::ReduceMotion(true)));
        assert!(preferences.apply(Change::Sounds(false)));
        assert!(!preferences.apply(Change::Sounds(false)));
        assert!(preferences.apply(Change::Theme(ThemeChoice::Light)));
        assert!(!preferences.apply(Change::Theme(ThemeChoice::Light)));
        let settings =
            json!({ "schema": "openagents.settings.v1", SECTION: preferences.section() });
        assert_eq!(Preferences::from_settings(&settings), preferences);
        assert_eq!(
            preferences.section(),
            json!({
                "text_size": "largest",
                "reduce_motion": true,
                "notifications": false,
                "sounds": false,
                "theme": "light"
            })
        );
    }

    #[test]
    fn every_size_scales_the_transcript_and_the_default_is_unchanged() {
        assert_eq!(TextSize::Default.transcript(), crate::visual::TRANSCRIPT);
        let mut last = 0;
        for size in TextSize::ALL {
            let metrics = size.transcript();
            assert!(metrics.body_size > last, "{size:?}");
            last = metrics.body_size;
            assert_eq!(
                metrics.reading_width,
                crate::visual::TRANSCRIPT.reading_width
            );
            // Every size is one the transcript layout admits.
            rust_native::layout::TranscriptLayout::new()
                .set_metrics(metrics)
                .expect("valid metrics");
        }
        assert_eq!(TextSize::Largest.transcript().body_size, 21);
        assert_eq!(TextSize::Smaller.scale(14.0), 13.0);
    }
}
