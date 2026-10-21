//! The load screen between the Grid and a zone (#12438).
//!
//! Going through a Grid portal builds the zone, then opens the zone's
//! renderer on the canvas; coming back opens the Grid's engine again. The
//! screen covers the canvas the whole way, its bar moving as each step
//! finishes, and lifts once the first frame of the new place is drawn, so
//! the player never sees a still or half-drawn canvas.
//!
//! The steps are the real ones the page waits on: the zone's world, the
//! GPU device, the renderer with the world uploaded, and the first frame.
//! The page reports each through [`Load::reach`]. The analytics hooks are
//! the window's `verse-zone-load` events, whose detail [`Load::event`]
//! writes: `start` when the screen goes up, then `ready` or `failed`.
//!
//! Plain data, tested natively; `web.rs` draws it.

/// Which way the player is going.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    /// Through a Grid portal into a zone.
    In,
    /// Back to the Grid from a zone.
    Back,
}

impl Way {
    fn name(self) -> &'static str {
        match self {
            Self::In => "enter",
            Self::Back => "return",
        }
    }
}

/// A finished step, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// The screen is up; nothing has finished yet.
    Started,
    /// The place's world is built.
    World,
    /// The GPU device is open on the canvas.
    Device,
    /// The renderer is ready, with the world on the GPU.
    Renderer,
    /// The first frame of the new place is drawn.
    Drawn,
}

impl Stage {
    /// The bar's fill once this step has finished.
    #[must_use]
    pub fn fraction(self) -> f32 {
        match self {
            Self::Started => 0.05,
            Self::World => 0.4,
            Self::Device => 0.6,
            Self::Renderer => 0.9,
            Self::Drawn => 1.0,
        }
    }
}

/// How long the screen takes to fade out once the place is drawn, ms.
pub const FADE_MS: f64 = 350.0;

/// One trip between the Grid and a zone.
#[derive(Clone, Debug, PartialEq)]
pub struct Load {
    pub way: Way,
    /// The page's name for the zone (`lagrange2`), the event's `zone`.
    pub zone: &'static str,
    /// The place's name on screen (`Lagrange 2`, `the Grid`).
    pub label: String,
    pub stage: Stage,
    /// When the screen went up, ms.
    pub since: f64,
    /// A line that takes the detail's place, such as why the zone didn't
    /// open on the way back.
    pub note: Option<String>,
}

/// What a `verse-zone-load` event reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Start,
    Ready,
    Failed,
}

impl Load {
    #[must_use]
    pub fn new(way: Way, zone: &'static str, label: &str, now: f64) -> Self {
        Self {
            way,
            zone,
            label: label.to_owned(),
            stage: Stage::Started,
            since: now,
            note: None,
        }
    }

    /// `stage` finished. Steps only move forward; returns whether this one
    /// moved the bar.
    pub fn reach(&mut self, stage: Stage) -> bool {
        if stage > self.stage {
            self.stage = stage;
            true
        } else {
            false
        }
    }

    /// Whether the new place is drawn and the screen can lift.
    #[must_use]
    pub fn done(&self) -> bool {
        self.stage == Stage::Drawn
    }

    /// The heading.
    #[must_use]
    pub fn title(&self) -> String {
        match self.way {
            Way::In => format!("Entering {}", self.label),
            Way::Back => format!("Back to {}", self.label),
        }
    }

    /// The line under the bar: what the page is doing now.
    #[must_use]
    pub fn detail(&self) -> String {
        if let Some(note) = &self.note {
            return note.clone();
        }
        match (self.stage, self.way) {
            (Stage::Started, Way::In) => format!("Building {}…", self.label),
            (Stage::Started, Way::Back) => "Getting the Grid ready…".into(),
            (Stage::World, _) => "Starting the graphics…".into(),
            (Stage::Device, _) => "Setting up the scene…".into(),
            (Stage::Renderer, _) => "Almost there…".into(),
            (Stage::Drawn, _) => "Ready".into(),
        }
    }

    /// The bar's fill, 0 to 1.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        self.stage.fraction()
    }

    /// The bar's value as a whole percent, for `aria-valuenow`.
    #[must_use]
    pub fn percent(&self) -> u32 {
        (self.fraction() * 100.0).round() as u32
    }

    /// The detail of a `verse-zone-load` event at `now` (ms), as JSON:
    /// `{phase, zone, direction, ms}`, plus `error` for a failure.
    #[must_use]
    pub fn event(&self, event: Event, now: f64, error: Option<&str>) -> String {
        let phase = match event {
            Event::Start => "start",
            Event::Ready => "ready",
            Event::Failed => "failed",
        };
        let mut detail = serde_json::json!({
            "phase": phase,
            "zone": self.zone,
            "direction": self.way.name(),
            "ms": (now - self.since).max(0.0).round(),
        });
        if let Some(error) = error {
            detail["error"] = error.chars().take(240).collect::<String>().into();
        }
        detail.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_moves_forward_with_each_real_step_to_the_first_frame() {
        let mut load = Load::new(Way::In, "lagrange2", "Lagrange 2", 1_000.0);
        assert_eq!(load.title(), "Entering Lagrange 2");
        assert_eq!(load.detail(), "Building Lagrange 2…");
        let mut last = load.fraction();
        for stage in [Stage::World, Stage::Device, Stage::Renderer, Stage::Drawn] {
            assert!(!load.done());
            assert!(load.reach(stage));
            assert!(load.fraction() > last, "{stage:?}");
            last = load.fraction();
        }
        assert!(load.done());
        assert_eq!(load.percent(), 100);
        // A step reported late never moves the bar back.
        assert!(!load.reach(Stage::Device));
        assert_eq!(load.stage, Stage::Drawn);
    }

    #[test]
    fn the_way_back_names_the_grid() {
        let load = Load::new(Way::Back, "grid", "the Grid", 0.0);
        assert_eq!(load.title(), "Back to the Grid");
        assert_eq!(load.detail(), "Getting the Grid ready…");
        let mut failed = load.clone();
        failed.note = Some("We couldn't open Lagrange 2. You're back on the Grid.".into());
        assert_eq!(
            failed.detail(),
            "We couldn't open Lagrange 2. You're back on the Grid."
        );
    }

    #[test]
    fn events_carry_the_phase_zone_direction_and_time() {
        let load = Load::new(Way::In, "lagrange2", "Lagrange 2", 1_000.0);
        let start: serde_json::Value =
            serde_json::from_str(&load.event(Event::Start, 1_000.0, None)).unwrap();
        assert_eq!(
            start,
            serde_json::json!({"phase": "start", "zone": "lagrange2", "direction": "enter", "ms": 0.0})
        );
        let ready: serde_json::Value =
            serde_json::from_str(&load.event(Event::Ready, 2_234.4, None)).unwrap();
        assert_eq!(ready["phase"], "ready");
        assert_eq!(ready["ms"], 1_234.0);
        let failed: serde_json::Value =
            serde_json::from_str(&load.event(Event::Failed, 1_500.0, Some("no adapter"))).unwrap();
        assert_eq!(failed["phase"], "failed");
        assert_eq!(failed["error"], "no adapter");
        let back = Load::new(Way::Back, "grid", "the Grid", 0.0);
        assert!(
            back.event(Event::Start, 0.0, None)
                .contains("\"direction\":\"return\"")
        );
    }

    #[test]
    fn the_words_are_plain() {
        let mut load = Load::new(Way::In, "lagrange2", "Lagrange 2", 0.0);
        let mut copy = vec![load.title(), load.detail()];
        for stage in [Stage::World, Stage::Device, Stage::Renderer, Stage::Drawn] {
            load.reach(stage);
            copy.push(load.detail());
        }
        let back = Load::new(Way::Back, "grid", "the Grid", 0.0);
        copy.extend([back.title(), back.detail()]);
        for text in copy {
            assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
        }
    }
}
