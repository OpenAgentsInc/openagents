use crate::{Hsla, theme};

use super::theme as viz_theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeTone {
    Neutral,
    Live,
    Warning,
    Error,
    TrackPgolf,
    TrackHomegolf,
    TrackXtrain,
}

pub fn tone_color(tone: BadgeTone) -> Hsla {
    match tone {
        BadgeTone::Neutral => theme::text::MUTED,
        BadgeTone::Live => viz_theme::state::LIVE,
        BadgeTone::Warning => viz_theme::state::WARNING,
        BadgeTone::Error => viz_theme::state::ERROR,
        BadgeTone::TrackPgolf => viz_theme::track::PGOLF,
        BadgeTone::TrackHomegolf => viz_theme::track::HOMEGOLF,
        BadgeTone::TrackXtrain => viz_theme::track::XTRAIN,
    }
}

#[cfg(test)]
mod tests {
    use super::{BadgeTone, tone_color};

    #[test]
    fn badge_tones_map_to_distinct_semantics() {
        assert_ne!(
            tone_color(BadgeTone::TrackPgolf),
            tone_color(BadgeTone::TrackHomegolf)
        );
        assert_ne!(tone_color(BadgeTone::Warning), tone_color(BadgeTone::Error));
    }
}
