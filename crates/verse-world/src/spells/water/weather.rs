//! The weather's rules (`docs/verse/water.md`, Weather): SRD 5.2.1's
//! Heavy Precipitation and Strong Wind over the zone's schedule
//! (`physics::water::weather`), Call Lightning's storm bonus, and the
//! spells that make weather as bounded local [`Overlay`]s with a start
//! tick and a duration, which never change the schedule.

use glam::{DVec2, DVec3};
use physics::water::weather::{Overlay, OverlayKind, State, Weather};

use super::effects::{RAIN_CUBE, RAIN_TIME, VaporKind};
use super::{WaterSpells, lightning, ticks};
use crate::spells::FEET;

/// Sleet Storm's cylinder radius, m: 20 feet; it lasts up to a minute.
pub const SLEET_RADIUS: f64 = 20.0 * FEET;
pub const SLEET_TIME: f64 = 60.0;
/// Call Lightning's storm cloud radius, m: 60 feet; up to 10 minutes.
pub const CLOUD_RADIUS: f64 = 60.0 * FEET;
pub const CLOUD_TIME: f64 = 600.0;
/// Storm of Vengeance's cloud radius, m: 300 feet; up to a minute.
pub const VENGEANCE_RADIUS: f64 = 300.0 * FEET;
pub const VENGEANCE_TIME: f64 = 60.0;

/// What the weather at a place does under SRD 5.2.1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Conditions {
    /// Heavy rain or a storm: Heavy Precipitation.
    pub heavy_precipitation: bool,
    /// Wind of 20 mph (9 m/s) or more: Strong Wind.
    pub strong_wind: bool,
    /// Heavy Precipitation leaves the area Lightly Obscured.
    pub lightly_obscured: bool,
    /// Heavy Precipitation: Disadvantage on Wisdom (Perception) checks.
    pub perception_disadvantage: bool,
    /// Strong Wind: Disadvantage on ranged weapon attack rolls.
    pub ranged_disadvantage: bool,
    /// Either puts out open flames.
    pub flames_out: bool,
    /// Strong Wind disperses fog.
    pub fog_dispersed: bool,
    /// The zone's Storm, which Call Lightning takes control of.
    pub storm: bool,
}

/// The SRD's conditions under `weather`.
#[must_use]
pub fn conditions(weather: &Weather) -> Conditions {
    let heavy = weather.heavy_precipitation();
    let wind = weather.strong_wind();
    Conditions {
        heavy_precipitation: heavy,
        strong_wind: wind,
        lightly_obscured: heavy,
        perception_disadvantage: heavy,
        ranged_disadvantage: wind,
        flames_out: heavy || wind,
        fog_dispersed: wind,
        storm: weather.state == State::Storm,
    }
}

/// Call Lightning's d10s per bolt under `weather`: cast outdoors in a
/// storm, it takes control of the storm and deals 1d10 more.
#[must_use]
pub fn call_lightning_dice(weather: &Weather) -> u32 {
    lightning::call_lightning_dice(weather.state == State::Storm)
}

fn area(kind: OverlayKind, center: DVec3, radius: f64, start: u64, seconds: f64) -> Overlay {
    Overlay {
        kind,
        center: DVec2::new(center.x, center.z),
        radius,
        start,
        duration: ticks(seconds),
    }
}

/// Create Water's rain over its 30-foot cube, for its 6 s (ours).
#[must_use]
pub fn create_water(center: DVec3, start: u64) -> Overlay {
    area(OverlayKind::Rain, center, RAIN_CUBE * 0.5, start, RAIN_TIME)
}

/// Sleet Storm's cylinder for as long as it is held, at most a minute.
#[must_use]
pub fn sleet_storm(center: DVec3, start: u64, seconds: f64) -> Overlay {
    area(
        OverlayKind::Sleet,
        center,
        SLEET_RADIUS,
        start,
        seconds.min(SLEET_TIME),
    )
}

/// Fog Cloud's sphere of `radius` m for `seconds`.
#[must_use]
pub fn fog_cloud(center: DVec3, radius: f64, start: u64, seconds: f64) -> Overlay {
    area(OverlayKind::Fog, center, radius, start, seconds)
}

/// Call Lightning's storm cloud, at most 10 minutes.
#[must_use]
pub fn call_lightning(center: DVec3, start: u64, seconds: f64) -> Overlay {
    area(
        OverlayKind::Cloud,
        center,
        CLOUD_RADIUS,
        start,
        seconds.min(CLOUD_TIME),
    )
}

/// Storm of Vengeance's storm under a cloud `radius` m wide (the Grove
/// draws 60 feet of the SRD's 300), at most a minute.
#[must_use]
pub fn storm_of_vengeance(center: DVec3, radius: f64, start: u64, seconds: f64) -> Overlay {
    area(
        OverlayKind::Storm,
        center,
        radius.min(VENGEANCE_RADIUS),
        start,
        seconds.min(VENGEANCE_TIME),
    )
}

impl WaterSpells {
    /// The spell weather the water's spells hold at `tick`: Create
    /// Water's rains and the fog banks, as overlays on the schedule.
    #[must_use]
    pub fn overlays(&self, tick: u64) -> Vec<Overlay> {
        let rains = self.rains.iter().filter(|r| r.until > tick).map(|r| {
            let duration = ticks(RAIN_TIME);
            Overlay {
                kind: OverlayKind::Rain,
                center: DVec2::new(r.center.x, r.center.z),
                radius: r.half,
                start: r.until.saturating_sub(duration),
                duration,
            }
        });
        let fogs = self
            .vapors
            .iter()
            .filter(|v| v.kind == VaporKind::Fog && v.until > tick)
            .map(|v| Overlay {
                kind: OverlayKind::Fog,
                center: DVec2::new(v.center.x, v.center.z),
                radius: v.radius,
                start: 0,
                duration: v.until,
            });
        rains.chain(fogs).collect()
    }
}

#[cfg(test)]
mod tests {
    use physics::water::weather::{Climate, Schedule};

    use super::*;
    use crate::spells::water::Rain;

    #[test]
    fn heavy_rain_and_storms_are_heavy_precipitation() {
        let s = Schedule::new(Climate::TEMPERATE, 5);
        let storm = s.clone().pinned(State::Storm).sample(1_000_000);
        let c = conditions(&storm);
        assert!(c.heavy_precipitation && c.lightly_obscured && c.perception_disadvantage);
        assert!(c.flames_out && c.storm);
        assert_eq!(call_lightning_dice(&storm), 4);
        let clear = s.pinned(State::Clear).sample(1_000_000);
        assert_eq!(conditions(&clear), Conditions::default());
        assert_eq!(call_lightning_dice(&clear), 3);
        let gale = Weather {
            wind: DVec2::new(9.5, 0.0),
            ..Weather::CALM
        };
        let c = conditions(&gale);
        assert!(c.strong_wind && c.ranged_disadvantage && c.fog_dispersed && c.flames_out);
        assert!(!c.heavy_precipitation);
    }

    #[test]
    fn spell_weather_lies_over_the_schedule() {
        let s = Schedule::new(Climate::TEMPERATE, 5).pinned(State::Clear);
        let at = DVec3::new(4.0, 0.0, -3.0);
        let p = DVec2::new(4.0, -3.0);
        let mut spells = WaterSpells::default();
        spells.rains.push(Rain {
            center: at,
            half: RAIN_CUBE * 0.5,
            until: 1_000 + ticks(RAIN_TIME),
        });
        spells.fog(at, 6.0, 50_000);
        let overlays = spells.overlays(1_000 + ticks(3.0));
        assert_eq!(overlays.len(), 2);
        let w = s.sample_at(p, 1_000 + ticks(3.0), &overlays);
        assert!(w.rain > 0.6 && w.fog > 0.9, "{w:?}");
        assert!(spells.overlays(1_000 + ticks(RAIN_TIME)).len() == 1);
        // Storm of Vengeance makes Call Lightning stronger under it.
        let vengeance = storm_of_vengeance(at, 18.0, 0, 60.0);
        let w = s.sample_at(p, ticks(30.0), &[vengeance]);
        assert_eq!(call_lightning_dice(&w), 4);
        assert!(conditions(&w).strong_wind);
        let sleet = sleet_storm(at, 0, 300.0);
        assert_eq!(sleet.duration, ticks(SLEET_TIME));
        assert!(s.sample_at(p, ticks(10.0), &[sleet]).heavy_precipitation());
        let cloud = call_lightning(at, 0, 600.0);
        assert!(s.sample_at(p, ticks(10.0), &[cloud]).cloud > 0.99);
        let rain = create_water(at, 0);
        assert!(s.sample_at(p, ticks(3.0), &[rain]).rain > 0.6);
        let fog = fog_cloud(at, 6.0, 0, 3_600.0);
        assert!(s.sample_at(p, ticks(3.0), &[fog]).fog > 0.99);
    }
}
