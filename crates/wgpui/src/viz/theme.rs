use crate::{Hsla, theme};

/// Binding rules for visualization tokens:
///
/// - charts should use `series::*`
/// - badges should use `state::*` and `track::*`
/// - panel chrome should use `surface::*`
pub mod surface {
    use crate::Hsla;

    pub const PANEL_BG: Hsla = Hsla::new(0.578, 0.553, 0.063, 0.96);
    pub const PANEL_TITLE_BG_ALPHA: f32 = 0.06;
    pub const PANEL_RULE_ALPHA: f32 = 0.12;
    pub const CHART_BG: Hsla = Hsla::new(0.57, 0.714, 0.039, 0.92);
}

pub mod state {
    use crate::{Hsla, theme};

    pub const LIVE: Hsla = theme::status::SUCCESS;
    pub const ACTIVE: Hsla = theme::status::RUNNING;
    pub const CACHED: Hsla = Hsla::new(0.52, 0.72, 0.54, 1.0);
    pub const STALE: Hsla = theme::status::WARNING;
    pub const WARNING: Hsla = theme::status::WARNING;
    pub const ERROR: Hsla = theme::status::ERROR;
    pub const UNAVAILABLE: Hsla = theme::text::MUTED;
}

pub mod track {
    use crate::Hsla;

    pub const PGOLF: Hsla = Hsla::new(0.549, 0.979, 0.715, 1.0);
    pub const HOMEGOLF: Hsla = Hsla::new(0.129, 0.864, 0.688, 1.0);
    pub const XTRAIN: Hsla = Hsla::new(0.386, 0.75, 0.725, 1.0);
    pub const EXPLORER: Hsla = Hsla::new(0.824, 0.918, 0.804, 1.0);
}

pub mod series {
    use crate::Hsla;

    pub const LOSS: Hsla = Hsla::new(0.987, 0.652, 0.651, 1.0);
    pub const OPTIMIZER: Hsla = Hsla::new(0.373, 0.756, 0.732, 1.0);
    pub const RUNTIME: Hsla = Hsla::new(0.549, 0.979, 0.715, 1.0);
    pub const HARDWARE: Hsla = Hsla::new(0.129, 0.864, 0.688, 1.0);
    pub const EVENTS: Hsla = LOSS;
    pub const PROVENANCE: Hsla = OPTIMIZER;
}

pub fn provider_accent(provider: &str) -> Hsla {
    if provider.eq_ignore_ascii_case("runpod") {
        track::HOMEGOLF
    } else if provider.eq_ignore_ascii_case("google")
        || provider.eq_ignore_ascii_case("google_cloud")
    {
        series::RUNTIME
    } else {
        theme::accent::PRIMARY
    }
}

#[cfg(test)]
mod tests {
    use super::{series, state, surface, track};

    #[test]
    fn viz_surface_tokens_are_non_transparent() {
        assert!(surface::PANEL_BG.a > 0.0);
        assert!(surface::CHART_BG.a > 0.0);
    }

    #[test]
    fn viz_state_tokens_remain_distinct() {
        assert_ne!(state::LIVE, state::ERROR);
        assert_ne!(state::STALE, state::UNAVAILABLE);
    }

    #[test]
    fn viz_track_tokens_cover_all_training_families() {
        assert_ne!(track::PGOLF, track::HOMEGOLF);
        assert_ne!(track::HOMEGOLF, track::XTRAIN);
        assert_ne!(series::LOSS, series::RUNTIME);
    }
}
