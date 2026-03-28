use crate::{Hsla, theme};

use super::theme as viz_theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProvenanceTone {
    Neutral,
    Evidence,
    Cached,
    Warning,
    Error,
}

pub fn tone_color(tone: ProvenanceTone) -> Hsla {
    match tone {
        ProvenanceTone::Neutral => theme::text::MUTED,
        ProvenanceTone::Evidence => viz_theme::series::PROVENANCE,
        ProvenanceTone::Cached => viz_theme::state::CACHED,
        ProvenanceTone::Warning => viz_theme::state::WARNING,
        ProvenanceTone::Error => viz_theme::state::ERROR,
    }
}
