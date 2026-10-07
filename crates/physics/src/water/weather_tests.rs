use glam::DVec2;

use super::*;
use crate::water::tick_at;

const HZ: u64 = TICK_HZ;
const BLOCK: u64 = BLOCK_SECONDS * HZ;

fn everglade() -> Schedule {
    Schedule::new(Climate::TEMPERATE, 0xE7E5)
}

#[test]
fn two_clients_sample_the_same_weather_for_the_same_seed_and_tick() {
    // Two clients build their schedules on their own and read the tick
    // from clocks a few milliseconds apart within one step.
    let a = everglade();
    let b = Schedule::new(Climate::TEMPERATE, 0xE7E5);
    let ms = 1_791_000_000_000_u64;
    let (ta, tb) = (tick_at(ms), tick_at(ms + 3));
    assert_eq!(ta, tb);
    for k in 0..2_000 {
        let tick = ta + k * 977;
        assert_eq!(a.sample(tick), b.sample(tick));
        assert_eq!(a.ground(tick), b.ground(tick));
        assert_eq!(a.rise(tick).to_bits(), b.rise(tick).to_bits());
    }
    // Another seed is another sky.
    let c = Schedule::new(Climate::TEMPERATE, 0xE7E6);
    let differs = (0..200).any(|k| a.block(k) != c.block(k));
    assert!(differs);
}

#[test]
fn blocks_follow_the_climate() {
    let s = everglade();
    let mut counts = [0usize; 5];
    for k in 0..20_000 {
        counts[s.block(k) as usize] += 1;
    }
    let total: f64 = Climate::TEMPERATE.weights.iter().sum();
    for (i, w) in Climate::TEMPERATE.weights.iter().enumerate() {
        let share = counts[i] as f64 / 20_000.0;
        assert!((share - w / total).abs() < 0.02, "{i}: {share}");
    }
    // Everglade is mostly clear.
    assert!(counts[0] > counts.iter().skip(1).copied().max().unwrap());
}

#[test]
fn transitions_blend_over_sixty_seconds() {
    let s = everglade();
    let k = (1..10_000)
        .find(|&k| s.block(k) == State::Clear && s.block(k + 1) == State::Rain)
        .expect("a clear block followed by rain");
    let boundary = (k + 1) * BLOCK;
    // Before the boundary it is clear; the rain eases in over 60 s.
    assert_eq!(s.states(boundary - 1), (State::Clear, State::Clear, 1.0));
    let start = s.states(boundary);
    assert_eq!((start.0, start.1), (State::Clear, State::Rain));
    assert_eq!(start.2, 0.0);
    assert_eq!(s.sample(boundary).rain, 0.0);
    let half = s.sample(boundary + 30 * HZ);
    let full = s.sample(boundary + 60 * HZ);
    assert!(
        half.rain > 0.0 && half.rain < full.rain,
        "{half:?} {full:?}"
    );
    assert_eq!(s.states(boundary + 60 * HZ).2, 1.0);
    // The rain rises without a jump from tick to tick.
    let mut last = s.rain(boundary);
    for t in boundary..boundary + 60 * HZ {
        let now = s.rain(t);
        assert!((now - last).abs() < 0.01, "jump at {t}: {last} to {now}");
        last = now;
    }
    // The state flips halfway through the blend.
    assert_eq!(s.sample(boundary + 29 * HZ).state, State::Clear);
    assert_eq!(s.sample(boundary + 31 * HZ).state, State::Rain);
}

#[test]
fn a_pinned_state_overrides_the_schedule() {
    let tick = 9_000 * BLOCK + 123;
    for state in State::ALL {
        let s = everglade().pinned(state);
        assert_eq!(s.sample(tick).state, state);
        assert_eq!(s.states(tick), (state, state, 1.0));
    }
    // A pin from a tick on blends in from what the schedule had.
    let mut s = everglade();
    let at = 77 * BLOCK + 5 * HZ;
    let scheduled = s.sample(at - 1).state;
    let to = if scheduled == State::Storm {
        State::Clear
    } else {
        State::Storm
    };
    s.pin(at, to);
    assert_eq!(s.sample(at - 1).state, scheduled);
    assert_eq!(s.states(at).0, scheduled);
    assert_eq!(s.sample(at + 60 * HZ).state, to);
    assert_eq!(s.pinned_at(at + 1), Some(to));
    s.unpin();
    assert_eq!(s.pinned_at(at + 1), None);
}

#[test]
fn the_srd_effects_follow_rain_and_wind() {
    let tick = 5_000 * BLOCK;
    let storm = everglade().pinned(State::Storm).sample(tick);
    assert!(storm.heavy_precipitation());
    let clear = everglade().pinned(State::Clear).sample(tick);
    assert!(!clear.heavy_precipitation());
    assert!(!clear.strong_wind());
    assert_eq!(clear.rain, 0.0);
    // A coastal storm blows a Strong Wind, which disperses the fog.
    let coast = Schedule::new(Climate::COASTAL, 3).pinned(State::Storm);
    let windy = (0..400)
        .map(|k| coast.sample(tick + k * 997))
        .find(Weather::strong_wind);
    let windy = windy.expect("a strong wind in a coastal storm");
    assert!(windy.fog <= State::Storm.profile().fog * 0.2 + 1e-9);
}

#[test]
fn storms_flash_and_clear_skies_do_not() {
    let storm = everglade().pinned(State::Storm);
    let clear = everglade().pinned(State::Clear);
    let mut flashes = 0;
    for t in (0..120 * HZ).step_by(4) {
        if storm.sample(t).lightning > 0.5 {
            flashes += 1;
        }
        assert_eq!(clear.sample(t).lightning, 0.0);
    }
    assert!(flashes > 0);
}

#[test]
fn spell_weather_is_local_and_bounded_in_time() {
    let s = everglade().pinned(State::Clear);
    let rain = Overlay {
        kind: OverlayKind::Rain,
        center: DVec2::new(10.0, 0.0),
        radius: 4.5,
        start: 1_000,
        duration: 6 * HZ,
    };
    let inside = DVec2::new(10.0, 1.0);
    let w = s.sample_at(inside, 1_000 + 3 * HZ, &[rain]);
    assert!(w.rain >= 0.69, "{w:?}");
    assert_eq!(w.state, State::Rain);
    // Outside the area, before, and after: the schedule's own weather.
    assert_eq!(s.sample_at(DVec2::ZERO, 1_000 + 3 * HZ, &[rain]).rain, 0.0);
    assert_eq!(s.sample_at(inside, 999, &[rain]).rain, 0.0);
    assert_eq!(s.sample_at(inside, 1_000 + 6 * HZ, &[rain]).rain, 0.0);
    // The schedule itself never changes.
    assert_eq!(s.sample(1_000 + 3 * HZ).rain, 0.0);
    // Storm of Vengeance: a storm with strong wind under the cloud.
    let vengeance = Overlay {
        kind: OverlayKind::Storm,
        radius: 18.0,
        duration: 60 * HZ,
        ..rain
    };
    let w = s.sample_at(inside, 1_000 + 30 * HZ, &[vengeance]);
    assert_eq!(w.state, State::Storm);
    assert!(w.strong_wind() && w.heavy_precipitation());
    // Fog Cloud fogs; a strong wind would clear it.
    let fog = Overlay {
        kind: OverlayKind::Fog,
        ..rain
    };
    assert!(s.sample_at(inside, 1_000 + 3 * HZ, &[fog]).fog > 0.99);
}

#[test]
fn rising_water_is_bounded_and_deterministic() {
    // Pinned rain for hours: the level rises to at most 0.15 m.
    let wet = everglade().pinned(State::Storm);
    let dry = everglade().pinned(State::Clear);
    let t = 50 * BLOCK;
    assert!(wet.rise(t) <= MAX_RISE + 1e-12);
    assert!(wet.rise(t) > MAX_RISE * 0.6, "{}", wet.rise(t));
    assert_eq!(dry.rise(t), 0.0);
    assert!((wet.flow_gain(t) - 1.0) > 0.3);
    assert_eq!(dry.flow_gain(t), 1.0);
    // The schedule never raises water past the bound, and the rise moves
    // smoothly from tick to tick.
    let s = Schedule::new(Climate::COASTAL, 11);
    let mut last = s.rise(1_000 * BLOCK);
    for k in 1..3_000 {
        let tick = 1_000 * BLOCK + k * 120;
        let r = s.rise(tick);
        assert!((0.0..=MAX_RISE).contains(&r), "{r}");
        assert!((r - last).abs() < 0.002, "{last} to {r} at {tick}");
        last = r;
    }
    // It rises during rain and falls back after: rain pinned for 30
    // minutes, then clear.
    let mut s = everglade().pinned(State::Clear);
    let start = 300 * BLOCK;
    s.pin(start, State::Rain);
    s.pin(start + 1_800 * HZ, State::Clear);
    let peak = s.rise(start + 1_800 * HZ);
    assert!(peak > 0.05, "{peak}");
    assert!(s.rise(start + 3_600 * HZ + 60 * HZ) < 1e-9);
    // The same answer from a fresh schedule.
    let mut again = everglade().pinned(State::Clear);
    again.pin(start, State::Rain);
    again.pin(start + 1_800 * HZ, State::Clear);
    assert_eq!(again.rise(start + 1_800 * HZ).to_bits(), peak.to_bits());
}

#[test]
fn the_ground_wets_in_rain_and_dries_after() {
    let mut s = everglade().pinned(State::Clear);
    let start = 400 * BLOCK;
    assert_eq!(s.ground(start), Ground::default());
    s.pin(start, State::Rain);
    s.pin(start + 900 * HZ, State::Clear);
    let raining = s.ground(start + 600 * HZ);
    assert!(raining.wet > 0.9 && raining.puddles > 0.4, "{raining:?}");
    // Five minutes after the rain: still wet, puddles still standing.
    let after = s.ground(start + 1_200 * HZ);
    assert!(
        after.wet > 0.4 && after.wet < raining.wet + 0.1,
        "{after:?}"
    );
    assert!(after.puddles > 0.2, "{after:?}");
    // Long after: dry.
    let later = s.ground(start + 3_000 * HZ);
    assert_eq!(later, Ground::default());
}

#[test]
fn climates_validate_and_states_parse() {
    assert!(Climate::TEMPERATE.valid() && Climate::COASTAL.valid());
    assert!(
        !Climate {
            weights: [0.0; 5],
            wind: [0.0, 1.0]
        }
        .valid()
    );
    for state in State::ALL {
        assert_eq!(State::parse(state.name()), Some(state));
    }
    assert_eq!(State::parse(" RAIN "), Some(State::Rain));
    assert_eq!(State::Storm.next(), State::Clear);
}
