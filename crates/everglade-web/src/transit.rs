//! Going through a Grid portal in the browser.
//!
//! The runtime keeps a portal crossing for the page
//! (`WorldRuntime::defer_grid_portals`) rather than building the zone in
//! the frame the player walks through it. The page then says what it is
//! opening and builds the zone a couple of frames later, so the words are
//! on screen before the work starts; a zone this build can't open is
//! refused with a line saying so, and the player stays on the Grid. A zone
//! whose renderer fails to open sends the player back to the Grid with a
//! line saying that, instead of stopping the page.
//!
//! [`Transit`] is that state machine, apart from the browser (and over any
//! zone type) so it can be tested natively; `web.rs` says which zones the
//! web opens.

/// Frames between saying what loads and building it, so the words paint.
pub const SETTLE_FRAMES: u8 = 2;
/// How long a refusal or a failure stays on screen, ms.
pub const NOTICE_MS: f64 = 4_000.0;

/// What this build can do with a zone behind a Grid portal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    /// Generated here: it opens on the web.
    Web,
    /// Needs a pack this build doesn't load from the Grid, or a zone the
    /// web has no renderer for.
    DesktopOnly,
}

/// The line while a zone opens.
#[must_use]
pub fn loading_text(label: &str) -> String {
    format!("Loading {label}…")
}

/// The line for a zone the web can't open.
pub const DESKTOP_ONLY: &str = "This place is only in the desktop app.";

/// The line when a zone failed to open.
#[must_use]
pub fn failed_text(label: &str) -> String {
    format!("We couldn't open {label}. You're back on the Grid.")
}

/// Where a portal crossing stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Transit<ZoneId> {
    Idle,
    /// The line is up; the zone is built after `frames` more frames.
    Announced {
        zone: ZoneId,
        frames: u8,
    },
    /// The zone is built and its renderer is opening.
    Opening {
        zone: ZoneId,
        since: f64,
    },
    /// A refusal or a failure on screen until `until`.
    Notice {
        until: f64,
    },
}

/// What the page does this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Step<ZoneId> {
    Nothing,
    /// Put this line on screen.
    Say(String),
    /// Build the zone now.
    Build(ZoneId),
    /// Take the line down.
    Clear,
}

impl<ZoneId> Default for Transit<ZoneId> {
    fn default() -> Self {
        Self::Idle
    }
}

impl<ZoneId: Copy + PartialEq> Transit<ZoneId> {
    /// The player walked through the portal to `zone`, labeled `label`,
    /// which this build opens or not (`support`).
    pub fn ask(&mut self, zone: ZoneId, support: Support, label: &str, now: f64) -> Step<ZoneId> {
        if !matches!(self, Self::Idle | Self::Notice { .. }) {
            return Step::Nothing;
        }
        match support {
            Support::Web => {
                *self = Self::Announced {
                    zone,
                    frames: SETTLE_FRAMES,
                };
                Step::Say(loading_text(label))
            }
            Support::DesktopOnly => {
                *self = Self::Notice {
                    until: now + NOTICE_MS,
                };
                Step::Say(DESKTOP_ONLY.into())
            }
        }
    }

    /// One frame: `opening` is whether the renderer is still opening.
    pub fn frame(&mut self, now: f64, opening: bool) -> Step<ZoneId> {
        match self.clone() {
            Self::Announced { zone, frames } if frames > 0 => {
                *self = Self::Announced {
                    zone,
                    frames: frames - 1,
                };
                Step::Nothing
            }
            Self::Announced { zone, .. } => {
                *self = Self::Opening { zone, since: now };
                Step::Build(zone)
            }
            Self::Opening { .. } if !opening => {
                *self = Self::Idle;
                Step::Clear
            }
            Self::Notice { until } if now >= until => {
                *self = Self::Idle;
                Step::Clear
            }
            _ => Step::Nothing,
        }
    }

    /// The zone, or its renderer, failed to open; the page sends the player
    /// back to the Grid.
    pub fn failed(&mut self, label: &str, now: f64) -> Step<ZoneId> {
        *self = Self::Notice {
            until: now + NOTICE_MS,
        };
        Step::Say(failed_text(label))
    }

    /// Whether the page should hold the player still: a zone is on its way.
    #[must_use]
    pub fn busy(&self) -> bool {
        matches!(self, Self::Announced { .. } | Self::Opening { .. })
    }
}

/// Whether `step` is the refusal of a zone this build can't open: what the
/// page reports as `verse-zone-unavailable` (#12439).
#[must_use]
pub fn refused<ZoneId>(step: &Step<ZoneId>) -> bool {
    matches!(step, Step::Say(text) if text == DESKTOP_ONLY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Zone {
        Lagrange1,
        Lagrange2,
        Everglade,
    }
    use Support::{DesktopOnly, Web};

    #[test]
    fn a_web_zone_is_announced_built_then_cleared_once_drawn() {
        let mut transit = Transit::<Zone>::default();
        assert_eq!(
            transit.ask(Zone::Lagrange2, Web, "Lagrange 2", 0.0),
            Step::Say("Loading Lagrange 2…".into())
        );
        assert!(transit.busy());
        // A second crossing while one loads changes nothing.
        assert_eq!(
            transit.ask(Zone::Lagrange1, Web, "Lagrange 1", 1.0),
            Step::Nothing
        );
        for _ in 0..SETTLE_FRAMES {
            assert_eq!(transit.frame(16.0, false), Step::Nothing);
        }
        assert_eq!(transit.frame(32.0, false), Step::Build(Zone::Lagrange2));
        // Still opening: the line stays.
        assert_eq!(transit.frame(48.0, true), Step::Nothing);
        assert!(transit.busy());
        assert_eq!(transit.frame(64.0, false), Step::Clear);
        assert_eq!(transit, Transit::Idle);
    }

    #[test]
    fn a_zone_the_web_cannot_open_is_refused_and_the_line_goes() {
        let mut transit = Transit::<Zone>::default();
        assert_eq!(
            transit.ask(Zone::Everglade, DesktopOnly, "Everglade", 100.0),
            Step::Say(DESKTOP_ONLY.into())
        );
        assert!(refused(&Step::<Zone>::Say(DESKTOP_ONLY.into())));
        assert!(!refused(&transit.clone().ask(
            Zone::Lagrange2,
            Web,
            "Lagrange 2",
            101.0
        )));
        assert!(!transit.busy());
        assert_eq!(transit.frame(100.0 + NOTICE_MS - 1.0, false), Step::Nothing);
        assert_eq!(transit.frame(100.0 + NOTICE_MS, false), Step::Clear);
    }

    #[test]
    fn a_failed_zone_says_so_and_another_portal_can_follow() {
        let mut transit = Transit::<Zone>::default();
        transit.ask(Zone::Lagrange2, Web, "Lagrange 2", 0.0);
        for _ in 0..=SETTLE_FRAMES {
            transit.frame(0.0, false);
        }
        assert_eq!(
            transit.failed("Lagrange 2", 10.0),
            Step::Say("We couldn't open Lagrange 2. You're back on the Grid.".into())
        );
        assert!(!transit.busy());
        // A new crossing replaces the notice.
        assert!(matches!(
            transit.ask(Zone::Lagrange2, Web, "Lagrange 2", 20.0),
            Step::Say(_)
        ));
    }

    #[test]
    fn the_lines_are_plain() {
        for text in [
            loading_text("Lagrange 2"),
            DESKTOP_ONLY.to_owned(),
            failed_text("Lagrange 2"),
        ] {
            assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
        }
    }
}
