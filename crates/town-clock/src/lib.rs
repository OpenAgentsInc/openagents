//! Everglade's town clock (`docs/verse/generative-agents.md`, phase C1).
//!
//! Town time is a pure function of Unix time: every device that agrees on
//! the time of day in the real world derives the same town day, hour, and
//! phase, with no network message. The apps run the cycle
//! ([`Clock::RUNNING`], which [`Clock::from_settings`] gives with no
//! setting): a town day passes in [`DAY_REAL_SECONDS`] of real time,
//! counted from [`EPOCH_UNIX`], so a short visit sees the town's routines
//! turn over. The day doesn't pass evenly: [`PACE`] gives daylight most of
//! the real hour and dusk and night a few minutes each, so a visitor
//! usually arrives by day. The wall-clock mode follows real hours instead.
//! [`Clock::DAYTIME`], the library's default and the apps' off switch,
//! holds [`DAYTIME_HOUR`], late morning. An hour pin fixes the time of day
//! for captures and debugging while the day count keeps running.
//!
//! The crate reads no clock and has no dependencies: the caller passes the
//! Unix time, so a render loop passes the real time and a test passes a
//! fixed instant. Everglade's sky, the world tree, the townsfolk's routines,
//! and Alice's day plans all read town time from here, and nothing else
//! keeps it.

use std::fmt;

/// Real seconds in one compressed town day: one town day an hour.
pub const DAY_REAL_SECONDS: u32 = 3_600;

/// Seconds in a town day.
pub const TOWN_DAY_SECONDS: u32 = 86_400;

/// The town's epoch, Unix seconds: 2026-10-01T00:00:00Z, midnight at the
/// start of town day zero.
pub const EPOCH_UNIX: i64 = 1_790_812_800;

/// How town time follows real time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// One town day in `day_seconds` real seconds, from the epoch.
    Compressed { day_seconds: u32 },
    /// The real time of day, at `utc_offset_minutes` from UTC: one town
    /// day a real day.
    WallClock { utc_offset_minutes: i32 },
}

/// The compressed day's pace: each stretch's first town hour, and the
/// minutes of a 60-minute real day it takes to reach the next stretch's
/// hour. Daylight, 07:00 to 17:00, takes 42 minutes; dawn takes 3, dusk 7,
/// and the night 7.5, so the Sun is up for three quarters of a visit.
/// The shares add to 60.
pub const PACE: [(f64, f64); 5] = [
    (0.0, 3.0),
    (5.0, 3.0),
    (7.0, 42.5),
    (17.0, 7.0),
    (20.0, 4.5),
];

/// The stretch of [`PACE`] that holds `fraction` of a day, measured in real
/// time when `by_real` is set and in town time otherwise: its start and
/// span in town time, then in real time, as fractions of a day.
fn stretch(fraction: f64, by_real: bool) -> (f64, f64, f64, f64) {
    let (mut town, mut real) = (0.0, 0.0);
    for (i, &(hour, share)) in PACE.iter().enumerate() {
        let next = PACE.get(i + 1).map_or(24.0, |p| p.0);
        let (town_span, real_span) = ((next - hour) / 24.0, share / 60.0);
        let end = if by_real {
            real + real_span
        } else {
            town + town_span
        };
        if fraction < end || i + 1 == PACE.len() {
            return (town, town_span, real, real_span);
        }
        town += town_span;
        real += real_span;
    }
    (0.0, 1.0, 0.0, 1.0)
}

/// The fraction of a compressed town day gone when `real` of its real time
/// has passed: [`PACE`]'s piecewise-linear map, increasing from 0 at 0 to 1
/// at 1.
#[must_use]
pub fn town_fraction(real: f64) -> f64 {
    let real = real.clamp(0.0, 1.0);
    let (town, town_span, start, real_span) = stretch(real, true);
    town + (real - start) / real_span * town_span
}

/// The inverse of [`town_fraction`]: the fraction of the real day gone when
/// `town` of the town day has.
#[must_use]
pub fn real_fraction(town: f64) -> f64 {
    let town = town.clamp(0.0, 1.0);
    let (start, town_span, real, real_span) = stretch(town, false);
    real + (town - start) / town_span * real_span
}

/// Town seconds per real second at `hours` into a compressed day of
/// [`DAY_REAL_SECONDS`]: about 14 by day and 53 to 100 at night.
#[must_use]
pub fn town_per_real(hours: f64) -> f64 {
    let fraction = if hours.is_finite() {
        hours.rem_euclid(24.0) / 24.0
    } else {
        0.0
    };
    let (_, town_span, _, real_span) = stretch(fraction, false);
    town_span / real_span * f64::from(TOWN_DAY_SECONDS) / f64::from(DAY_REAL_SECONDS)
}

impl Mode {
    /// The default: one town day every [`DAY_REAL_SECONDS`].
    pub const COMPRESSED: Self = Self::Compressed {
        day_seconds: DAY_REAL_SECONDS,
    };

    /// Reads a mode setting: `compressed`, `compressed:SECONDS` (real
    /// seconds a town day), `wall` (UTC), or `wall:MINUTES` (an offset from
    /// UTC, such as `wall:-300`).
    ///
    /// # Errors
    ///
    /// Returns a message naming the accepted forms.
    pub fn parse(text: &str) -> Result<Self, String> {
        let usage = || {
            format!(
                "a town clock is compressed, compressed:SECONDS, wall, or wall:MINUTES, got {text}"
            )
        };
        let (name, value) = match text.trim().split_once(':') {
            Some((name, value)) => (name, Some(value)),
            None => (text.trim(), None),
        };
        match (name, value) {
            ("compressed", None) => Ok(Self::COMPRESSED),
            ("compressed", Some(v)) => match v.trim().parse::<u32>() {
                Ok(day_seconds) if day_seconds > 0 => Ok(Self::Compressed { day_seconds }),
                _ => Err(usage()),
            },
            ("wall", None) => Ok(Self::WallClock {
                utc_offset_minutes: 0,
            }),
            ("wall", Some(v)) => match v.trim().parse::<i32>() {
                Ok(m) if m.abs() <= 14 * 60 => Ok(Self::WallClock {
                    utc_offset_minutes: m,
                }),
                _ => Err(usage()),
            },
            _ => Err(usage()),
        }
    }
}

/// Whether the cycle runs, as the apps' setting reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// The cycle stopped at [`DAYTIME_HOUR`]: [`Clock::DAYTIME`].
    Off,
    /// The cycle running in a mode.
    On(Mode),
}

impl Setting {
    /// Reads a cycle setting: `off` (late-morning daylight), `on` (the
    /// compressed day), or any [`Mode::parse`] form.
    ///
    /// # Errors
    ///
    /// Returns a message naming the accepted forms.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "off" => Ok(Self::Off),
            "on" => Ok(Self::On(Mode::COMPRESSED)),
            other => Mode::parse(other).map(Self::On).map_err(|_| {
                format!(
                    "a town clock is off, on, compressed, compressed:SECONDS, wall, or \
                     wall:MINUTES, got {text}"
                )
            }),
        }
    }
}

/// The hour the stopped clock holds: late morning, the light Everglade
/// had before it had a clock.
pub const DAYTIME_HOUR: f64 = 10.5;

/// The town clock: an epoch, a mode, and an optional hour pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    /// Unix seconds at the start of town day zero.
    pub epoch_unix: i64,
    pub mode: Mode,
    /// A fixed time of day, whole seconds into the day, for captures and
    /// debugging. The day count still follows the mode.
    pub pinned_second: Option<u32>,
}

/// The library's default is [`Clock::DAYTIME`], so a test or a capture that
/// names no time looks the same at any hour; the apps run the cycle through
/// [`Clock::from_settings`].
impl Default for Clock {
    fn default() -> Self {
        Self::DAYTIME
    }
}

impl Clock {
    /// The stopped clock: the compressed day from [`EPOCH_UNIX`] with its
    /// time of day pinned at [`DAYTIME_HOUR`]. The day count still runs.
    pub const DAYTIME: Self = Self {
        epoch_unix: EPOCH_UNIX,
        mode: Mode::COMPRESSED,
        pinned_second: Some((DAYTIME_HOUR * 3_600.0) as u32),
    };

    /// The running cycle: the compressed day from [`EPOCH_UNIX`], unpinned.
    pub const RUNNING: Self = Self {
        pinned_second: None,
        ..Self::DAYTIME
    };

    /// The clock the settings give: the running cycle unless `setting`
    /// stops it or names another mode, pinned at `hour` when one is given.
    #[must_use]
    pub fn from_settings(setting: Option<Setting>, hour: Option<f64>) -> Self {
        let clock = match setting {
            None => Self::RUNNING,
            Some(Setting::Off) => Self::DAYTIME,
            Some(Setting::On(mode)) => Self::RUNNING.with_mode(mode),
        };
        if hour.is_some() {
            clock.pinned(hour)
        } else {
            clock
        }
    }

    /// This clock in `mode`.
    #[must_use]
    pub const fn with_mode(self, mode: Mode) -> Self {
        Self { mode, ..self }
    }

    /// This clock with its time of day pinned at `hour`, wrapped into a
    /// day and rounded to the second; `None`, or an hour that isn't a
    /// number, unpins it.
    #[must_use]
    pub fn pinned(self, hour: Option<f64>) -> Self {
        Self {
            pinned_second: hour
                .filter(|h| h.is_finite())
                .map(|h| ((h.rem_euclid(24.0) * 3_600.0).round() as u32) % TOWN_DAY_SECONDS),
            ..self
        }
    }

    /// The pinned hour, if the clock is pinned.
    #[must_use]
    pub fn pinned_hour(&self) -> Option<f64> {
        self.pinned_second.map(|s| f64::from(s) / 3_600.0)
    }

    /// Town seconds since the epoch at `unix_seconds`, before any pin.
    /// Monotonic in `unix_seconds`. A compressed day keeps [`PACE`].
    #[must_use]
    pub fn town_seconds(&self, unix_seconds: f64) -> f64 {
        let real = unix_seconds - self.epoch_unix as f64;
        match self.mode {
            Mode::Compressed { day_seconds } => {
                let length = f64::from(day_seconds.max(1));
                let day = (real / length).floor();
                let into = ((real - day * length) / length).clamp(0.0, 1.0);
                (day + town_fraction(into)) * f64::from(TOWN_DAY_SECONDS)
            }
            Mode::WallClock { utc_offset_minutes } => real + f64::from(utc_offset_minutes) * 60.0,
        }
    }

    /// Town time at `unix_seconds`, fractional seconds allowed.
    #[must_use]
    pub fn at(&self, unix_seconds: f64) -> TownTime {
        let total = self.town_seconds(unix_seconds);
        let day_length = f64::from(TOWN_DAY_SECONDS);
        let day = (total / day_length).floor();
        let second = match self.pinned_second {
            Some(second) => f64::from(second),
            None => (total - day * day_length).clamp(0.0, day_length - 1e-6),
        };
        TownTime {
            day: day as i64,
            second,
        }
    }

    /// Town time at whole Unix seconds.
    #[must_use]
    pub fn at_unix(&self, unix_seconds: i64) -> TownTime {
        self.at(unix_seconds as f64)
    }

    /// Real seconds until the town clock next reaches `hour`, from
    /// `unix_seconds`. A pinned clock never moves, so it returns `None`.
    #[must_use]
    pub fn real_seconds_until(&self, unix_seconds: f64, hour: f64) -> Option<f64> {
        if self.pinned_second.is_some() || !hour.is_finite() {
            return None;
        }
        let now = self.at(unix_seconds).hours();
        let target = hour.rem_euclid(24.0);
        Some(match self.mode {
            Mode::Compressed { day_seconds } => {
                let (from, to) = (real_fraction(now / 24.0), real_fraction(target / 24.0));
                (to - from).rem_euclid(1.0) * f64::from(day_seconds.max(1))
            }
            Mode::WallClock { .. } => (target - now).rem_euclid(24.0) * 3_600.0,
        })
    }

    /// Town seconds per real second at `hours` into the day: [`PACE`]'s
    /// rate on a compressed day, 1 on the wall clock, and 0 when pinned.
    #[must_use]
    pub fn town_per_real(&self, hours: f64) -> f64 {
        if self.pinned_second.is_some() {
            return 0.0;
        }
        match self.mode {
            Mode::Compressed { day_seconds } => {
                town_per_real(hours) * f64::from(DAY_REAL_SECONDS) / f64::from(day_seconds.max(1))
            }
            Mode::WallClock { .. } => 1.0,
        }
    }
}

/// A moment of town time: the day since the epoch and the second of that
/// day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TownTime {
    /// Town days since the epoch; negative before it.
    pub day: i64,
    /// Seconds into the day, `0.0..86_400.0`.
    pub second: f64,
}

impl TownTime {
    /// The town time at `hour` of `day`, for tests and fixed scenes.
    #[must_use]
    pub fn at_hour(day: i64, hour: f64) -> Self {
        Self {
            day,
            second: hour.rem_euclid(24.0) * 3_600.0,
        }
    }

    /// Hours into the day, fractional: `0.0..24.0`.
    #[must_use]
    pub fn hours(&self) -> f64 {
        self.second / 3_600.0
    }

    /// The hour on a 24-hour clock, `0..=23`.
    #[must_use]
    pub fn hour(&self) -> u8 {
        (self.second / 3_600.0).floor().clamp(0.0, 23.0) as u8
    }

    /// The minute of the hour, `0..=59`.
    #[must_use]
    pub fn minute(&self) -> u8 {
        ((self.second / 60.0).floor() % 60.0).clamp(0.0, 59.0) as u8
    }

    /// The fraction of the day gone, `0.0..1.0`.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        self.second / f64::from(TOWN_DAY_SECONDS)
    }

    /// The phase of the day this moment falls in.
    #[must_use]
    pub fn phase(&self) -> Phase {
        Phase::at(self.hours())
    }
}

impl fmt::Display for TownTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "day {}, {:02}:{:02} ({})",
            self.day,
            self.hour(),
            self.minute(),
            self.phase().name()
        )
    }
}

/// A phase of the town day. The phases cover the day without gaps: each
/// starts at its [`Phase::start`] hour and runs to the next one's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phase {
    Dawn,
    Morning,
    Noon,
    Afternoon,
    Dusk,
    Night,
}

impl Phase {
    /// Every phase, in the order the day passes through them from dawn.
    pub const ALL: [Self; 6] = [
        Self::Dawn,
        Self::Morning,
        Self::Noon,
        Self::Afternoon,
        Self::Dusk,
        Self::Night,
    ];

    /// The hour the phase starts. The Sun rises at 06:00 and sets at 18:00,
    /// in the middle of dawn and of dusk.
    #[must_use]
    pub const fn start(self) -> f64 {
        match self {
            Self::Dawn => 5.0,
            Self::Morning => 7.0,
            Self::Noon => 11.0,
            Self::Afternoon => 13.0,
            Self::Dusk => 17.0,
            Self::Night => 20.0,
        }
    }

    /// The phase at `hours` into the day, wrapped into a day.
    #[must_use]
    pub fn at(hours: f64) -> Self {
        let h = if hours.is_finite() {
            hours.rem_euclid(24.0)
        } else {
            0.0
        };
        Self::ALL
            .iter()
            .rev()
            .copied()
            .find(|p| h >= p.start())
            .unwrap_or(Self::Night)
    }

    /// The phase's lowercase display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dawn => "dawn",
            Self::Morning => "morning",
            Self::Noon => "noon",
            Self::Afternoon => "afternoon",
            Self::Dusk => "dusk",
            Self::Night => "night",
        }
    }
}

/// Reads an hour setting for a pin: `18`, `18.5`, or `18:30`.
///
/// # Errors
///
/// Returns a message when the text isn't an hour from 0 through 24.
pub fn parse_hour(text: &str) -> Result<f64, String> {
    let bad = || format!("a town hour is H, H.F, or HH:MM from 0 to 24, got {text}");
    let text = text.trim();
    let hour = match text.split_once(':') {
        Some((h, m)) => {
            let h: u8 = h.parse().map_err(|_| bad())?;
            let m: u8 = m.parse().map_err(|_| bad())?;
            if m >= 60 {
                return Err(bad());
            }
            f64::from(h) + f64::from(m) / 60.0
        }
        None => text.parse::<f64>().map_err(|_| bad())?,
    };
    if !(0.0..=24.0).contains(&hour) {
        return Err(bad());
    }
    Ok(hour.rem_euclid(24.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed instant: 2026-10-07T12:34:56Z.
    const INSTANT: i64 = 1_791_376_496;

    #[test]
    fn the_apps_run_the_cycle_and_off_is_daytime() {
        // The library's default stays still, so tests don't depend on the
        // hour; the apps' default setting runs.
        let day = Clock::default().at_unix(INSTANT);
        assert_eq!((day.hour(), day.minute()), (10, 30));
        assert_eq!(Clock::DAYTIME.pinned_hour(), Some(DAYTIME_HOUR));
        assert_eq!(Clock::from_settings(None, None), Clock::RUNNING);
        assert_eq!(Clock::from_settings(None, None).pinned_hour(), None);
        assert_eq!(
            Clock::from_settings(Some(Setting::Off), None),
            Clock::DAYTIME
        );
        assert_eq!(
            Clock::from_settings(Some(Setting::parse("on").unwrap()), None),
            Clock::RUNNING
        );
        let wall = Clock::from_settings(Some(Setting::parse("wall").unwrap()), Some(18.5));
        assert_eq!(wall.pinned_hour(), Some(18.5));
        assert!(matches!(wall.mode, Mode::WallClock { .. }));
        for setting in [None, Some(Setting::Off)] {
            let pinned = Clock::from_settings(setting, Some(6.0));
            assert_eq!(pinned.pinned_hour(), Some(6.0));
            let (a, b) = (pinned.at_unix(INSTANT), pinned.at_unix(INSTANT + 1_234));
            assert_eq!((a.hour(), a.second), (6, b.second));
        }
        assert_eq!(Setting::parse(" off "), Ok(Setting::Off));
        assert!(Setting::parse("dusk").is_err());
    }

    #[test]
    fn the_pace_gives_daylight_most_of_the_hour() {
        assert!((PACE.iter().map(|p| p.1).sum::<f64>() - 60.0).abs() < 1e-9);
        assert!(PACE.windows(2).all(|w| w[0].0 < w[1].0));
        // The real share of the hour from sunrise to sunset.
        let lit = real_fraction(18.0 / 24.0) - real_fraction(6.0 / 24.0);
        assert!(lit > 0.75, "{lit}");
        // Lamps burn from about 18:30 to 05:30; that is under a fifth.
        let dark = 1.0 - (real_fraction(18.5 / 24.0) - real_fraction(5.5 / 24.0));
        assert!(dark < 0.2, "{dark}");
        for step in 0..=1_000 {
            let real = f64::from(step) / 1_000.0;
            assert!((real_fraction(town_fraction(real)) - real).abs() < 1e-9);
        }
        assert_eq!((town_fraction(0.0), town_fraction(1.0)), (0.0, 1.0));
        assert!((town_per_real(12.0) - 600.0 / 42.5).abs() < 1e-9);
        assert!(town_per_real(2.0) > 4.0 * town_per_real(12.0));
        assert_eq!(Clock::DAYTIME.town_per_real(12.0), 0.0);
    }

    #[test]
    fn two_devices_at_one_instant_agree() {
        let a = Clock::RUNNING.with_mode(Mode::COMPRESSED);
        let b = Clock::RUNNING;
        assert_eq!(a.at_unix(INSTANT), b.at_unix(INSTANT));
        assert_eq!(a.at(INSTANT as f64 + 0.25), b.at(INSTANT as f64 + 0.25));
    }

    #[test]
    fn the_epoch_is_midnight_of_day_zero() {
        let t = Clock::RUNNING.at_unix(EPOCH_UNIX);
        assert_eq!((t.day, t.hour(), t.minute()), (0, 0, 0));
        assert_eq!(t.phase(), Phase::Night);
    }

    #[test]
    fn a_compressed_day_passes_in_an_hour() {
        let clock = Clock::RUNNING;
        // Night and dawn take 6 minutes, then each daylight hour 4.25.
        let morning = clock.at_unix(EPOCH_UNIX + 360);
        assert_eq!((morning.hour(), morning.minute()), (7, 0));
        let noon = clock.at_unix(EPOCH_UNIX + 1_635);
        assert_eq!((noon.day, noon.hour(), noon.minute()), (0, 12, 0));
        let next = clock.at_unix(EPOCH_UNIX + i64::from(DAY_REAL_SECONDS));
        assert_eq!((next.day, next.hour()), (1, 0));
        // By day a town minute is 4.25 real seconds.
        let minute = clock.at(EPOCH_UNIX as f64 + 1_635.0 + 4.3);
        assert_eq!((minute.hour(), minute.minute()), (12, 1));
    }

    #[test]
    fn town_time_is_monotonic() {
        for clock in [
            Clock::RUNNING,
            Clock::RUNNING.with_mode(Mode::WallClock {
                utc_offset_minutes: -300,
            }),
        ] {
            let mut last = f64::NEG_INFINITY;
            let mut last_total = f64::NEG_INFINITY;
            for step in 0..20_000_i64 {
                let unix = EPOCH_UNIX - 50_000 + step * 37;
                let total = clock.town_seconds(unix as f64);
                assert!(total > last_total);
                last_total = total;
                let t = clock.at_unix(unix);
                let absolute = t.day as f64 * f64::from(TOWN_DAY_SECONDS) + t.second;
                assert!(absolute > last, "{t} after {last}");
                assert!((0.0..f64::from(TOWN_DAY_SECONDS)).contains(&t.second));
                last = absolute;
            }
        }
    }

    #[test]
    fn the_wall_clock_follows_real_hours() {
        let clock = Clock::RUNNING.with_mode(Mode::WallClock {
            utc_offset_minutes: 0,
        });
        let t = clock.at_unix(INSTANT);
        assert_eq!((t.day, t.hour(), t.minute()), (6, 12, 34));
        let east = clock
            .with_mode(Mode::WallClock {
                utc_offset_minutes: 120,
            })
            .at_unix(INSTANT);
        assert_eq!(east.hour(), 14);
    }

    #[test]
    fn the_phases_cover_the_day_in_order() {
        let mut seen = Vec::new();
        for minute in 0..24 * 60 {
            let phase = TownTime::at_hour(0, f64::from(minute) / 60.0).phase();
            if seen.last() != Some(&phase) {
                seen.push(phase);
            }
        }
        // Midnight is night; the day runs dawn through night and back.
        assert_eq!(
            seen,
            [
                Phase::Night,
                Phase::Dawn,
                Phase::Morning,
                Phase::Noon,
                Phase::Afternoon,
                Phase::Dusk,
                Phase::Night
            ]
        );
        for phase in Phase::ALL {
            assert_eq!(Phase::at(phase.start()), phase);
        }
        assert!(Phase::ALL.windows(2).all(|w| w[0].start() < w[1].start()));
        assert_eq!(Phase::at(f64::NAN), Phase::Night);
    }

    #[test]
    fn a_pin_fixes_the_hour_and_keeps_the_day() {
        let clock = Clock::RUNNING.pinned(Some(18.5));
        let a = clock.at_unix(INSTANT);
        let b = clock.at_unix(INSTANT + 7_200);
        assert_eq!((a.hour(), a.minute(), a.phase()), (18, 30, Phase::Dusk));
        assert_eq!(a.second, b.second);
        assert_eq!(b.day, a.day + 2);
        assert_eq!(Clock::RUNNING.pinned(Some(-1.0)).pinned_hour(), Some(23.0));
        assert_eq!(
            Clock::RUNNING.pinned(Some(23.9999999)).pinned_second,
            Some(0)
        );
        assert_eq!(Clock::RUNNING.pinned(Some(f64::NAN)).pinned_hour(), None);
        assert_eq!(clock.real_seconds_until(INSTANT as f64, 6.0), None);
    }

    #[test]
    fn real_seconds_until_an_hour() {
        let clock = Clock::RUNNING;
        let wait = clock
            .real_seconds_until(EPOCH_UNIX as f64, 6.0)
            .expect("an unpinned clock moves");
        assert!((wait - 270.0).abs() < 1e-6);
        let t = clock.at(EPOCH_UNIX as f64 + wait);
        assert_eq!((t.hour(), t.minute()), (6, 0));
    }

    #[test]
    fn settings_parse() {
        assert_eq!(parse_hour("18"), Ok(18.0));
        assert_eq!(parse_hour("6:30"), Ok(6.5));
        assert_eq!(parse_hour("24"), Ok(0.0));
        assert!(parse_hour("25").is_err());
        assert!(parse_hour("12:60").is_err());
        assert!(parse_hour("noon").is_err());
        assert_eq!(Mode::parse("compressed"), Ok(Mode::COMPRESSED));
        assert_eq!(
            Mode::parse("compressed:600"),
            Ok(Mode::Compressed { day_seconds: 600 })
        );
        assert_eq!(
            Mode::parse("wall:-300"),
            Ok(Mode::WallClock {
                utc_offset_minutes: -300
            })
        );
        assert!(Mode::parse("compressed:0").is_err());
        assert!(Mode::parse("lunar").is_err());
    }

    #[test]
    fn display_names_the_day_and_phase() {
        assert_eq!(
            TownTime::at_hour(3, 18.25).to_string(),
            "day 3, 18:15 (dusk)"
        );
    }
}
