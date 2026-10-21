//! What the page shows and how it steers in each place (#12438).
//!
//! A Grid page walks between the Grid and a zone behind one of its portals.
//! The Grid's own layers (the Pylon labels, pillars, and Pylon Bay on `/`)
//! belong to the Grid only, and each zone keeps the controls it has on the
//! desktop: Lagrange 1's EVA pack and Lagrange 2's factory fly the visitor
//! along the view, and their zone panel carries the zone's buttons.
//!
//! This is plain data over the zones' names, so it is tested natively;
//! `web.rs` names the zone and acts on the answers.

/// Where the player is, as the page's layers see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// On the Grid plaza.
    Grid,
    /// Between the Grid and a zone, while the load screen is up.
    Loading,
    /// Inside a zone.
    Zone,
}

impl Place {
    /// The name the page's `data-verse-place` attribute and its
    /// `verse-zone` event carry.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::Loading => "loading",
            Self::Zone => "zone",
        }
    }

    /// Where the player is: on the plaza, on the way somewhere, or in the
    /// zone named `zone`.
    #[must_use]
    pub fn of(zone: &str, loading: bool) -> Self {
        if loading {
            Self::Loading
        } else if zone == GRID {
            Self::Grid
        } else {
            Self::Zone
        }
    }
}

/// The Grid plaza's zone name.
pub const GRID: &str = "grid";

/// Whether the Grid's overlay (Pylon labels, pillars, the Pylon Bay, and
/// the walking hint) shows: on the Grid only, never over a zone or its load
/// screen.
#[must_use]
pub fn overlay_shown(place: Place) -> bool {
    place == Place::Grid
}

/// How the keys, the mouse, and the zone panel steer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    /// The Grid homepage (`/`): `A` and `D` strafe, as #12353 set.
    GridWalk,
    /// The shared controller as the desktop maps it: `A` and `D` turn.
    Walk,
    /// Lagrange 1's EVA pack or Lagrange 2's factory: the zone flies the
    /// visitor along the view (`W`/`S` along it, `A`/`D` turn, `Q`/`E`
    /// strafe, `Space` climbs, `Shift` is fast), the zone panel carries the
    /// zone's buttons, and `F` at an arch goes through it.
    Station,
}

/// The scheme in the zone named `zone`, on the Grid homepage when `home`.
#[must_use]
pub fn scheme(zone: &str, home: bool) -> Scheme {
    match zone {
        "lagrange1" | "lagrange2" => Scheme::Station,
        GRID if home => Scheme::GridWalk,
        _ => Scheme::Walk,
    }
}

impl Scheme {
    /// The key the shared controller reads for `code`
    /// (`KeyboardEvent.code`).
    #[must_use]
    pub fn key(self, code: &str) -> &str {
        match (self, code) {
            (Self::GridWalk, "KeyA") => "KeyQ",
            (Self::GridWalk, "KeyD") => "KeyE",
            _ => code,
        }
    }

    /// Whether the zone panel (the zone's caption and buttons) draws over
    /// the world.
    #[must_use]
    pub fn zone_panel(self) -> bool {
        self == Self::Station
    }

    /// Whether `F` goes through a nearby arch, or into Snap Station.
    #[must_use]
    pub fn arch_key(self) -> bool {
        self == Self::Station
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overlay_shows_on_the_grid_only() {
        assert!(overlay_shown(Place::of("grid", false)));
        for zone in ["lagrange1", "lagrange2", "everglade", "grove"] {
            assert!(!overlay_shown(Place::of(zone, false)), "{zone}");
        }
        // The load screen covers the way in and the way back.
        assert!(!overlay_shown(Place::of("grid", true)));
        assert!(!overlay_shown(Place::of("lagrange2", true)));
        assert_eq!(Place::of("grid", true).name(), "loading");
        assert_eq!(Place::of("lagrange2", false).name(), "zone");
        assert_eq!(Place::of("grid", false).name(), "grid");
    }

    #[test]
    fn the_lagrange_zones_fly_with_their_panel_and_the_grid_walks() {
        for zone in ["lagrange1", "lagrange2"] {
            for home in [true, false] {
                let scheme = scheme(zone, home);
                assert_eq!(scheme, Scheme::Station, "{zone}");
                assert!(scheme.zone_panel() && scheme.arch_key());
                // A and D turn the visitor, as on the desktop.
                assert_eq!(scheme.key("KeyA"), "KeyA");
                assert_eq!(scheme.key("KeyD"), "KeyD");
            }
        }
        let home = scheme("grid", true);
        assert_eq!(home, Scheme::GridWalk);
        assert_eq!(home.key("KeyA"), "KeyQ");
        assert_eq!(home.key("KeyD"), "KeyE");
        assert_eq!(home.key("KeyW"), "KeyW");
        assert!(!home.zone_panel());
        assert_eq!(scheme("grid", false), Scheme::Walk);
        assert_eq!(scheme("everglade", true), Scheme::Walk);
        assert!(!Scheme::Walk.zone_panel());
        assert_eq!(Scheme::Walk.key("KeyA"), "KeyA");
    }
}
