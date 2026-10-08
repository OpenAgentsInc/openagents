use super::*;

fn run(r: &mut Ripples, from: f32, seconds: f32, eye: Vec2, sources: &[Source]) -> f32 {
    let mut t = from;
    let dt = 1.0 / 60.0;
    while t < from + seconds {
        t += dt;
        r.advance(t, eye, sources, None);
    }
    t
}

/// A splash on the wave-equation tiers spreads as a ring at about the
/// wave speed.
#[test]
fn a_pulse_spreads_as_a_ring_at_the_wave_speed() {
    let mut r = Ripples::for_tier(Tier::Low);
    r.advance(0.0, Vec2::ZERO, &[], None);
    r.advance(
        1.0 / 60.0,
        Vec2::ZERO,
        &[Source::impact(Vec2::ZERO, 0.3, 0.05)],
        None,
    );
    let t = run(&mut r, 1.0 / 60.0, 2.0, Vec2::ZERO, &[]);
    // Where the ring's disturbance is largest along +x.
    let (peak, _) = (1..60)
        .map(|i| {
            let x = i as f32 * 0.1;
            (x, r.height_at(x, 0.0).abs())
        })
        .fold((0.0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
    let expected = WAVE_SPEED * t;
    assert!(
        (peak - expected).abs() < 0.8,
        "ring at {peak} m, expected about {expected}"
    );
}

/// Both kernels lose energy after a splash and never blow up.
#[test]
fn the_field_settles_after_a_splash_on_every_tier() {
    for tier in [Tier::Low, Tier::Medium, Tier::High] {
        let mut r = Ripples::for_tier(tier);
        r.advance(0.0, Vec2::ZERO, &[], None);
        r.advance(
            0.04,
            Vec2::ZERO,
            &[Source::impact(Vec2::new(0.4, -0.3), 0.4, 0.06)],
            None,
        );
        run(&mut r, 0.04, 0.5, Vec2::ZERO, &[]);
        let early = r.energy();
        assert!(early > 0.0, "{tier:?}");
        run(&mut r, 0.54, 10.0, Vec2::ZERO, &[]);
        let late = r.energy();
        assert!(
            late.is_finite() && late < early * 0.05,
            "{tier:?}: {early} → {late}"
        );
    }
}

/// iWave's kernel answers each wavenumber with about that wavenumber, the
/// vertical derivative of a plane wave, so long waves outrun short ones.
#[test]
fn the_iwave_kernel_is_the_vertical_derivative() {
    let kernel = iwave_kernel(4);
    let sum: f32 = kernel.iter().sum();
    assert!(sum.abs() < 1e-4, "{sum}");
    for q in [0.6_f32, 0.8, 1.0, 1.2] {
        let response = kernel_response(&kernel, q);
        assert!((response - q).abs() < 0.35 * q, "q {q}: {response}");
    }
    // Positive and rising up to the grid's shortest wave, so no mode grows.
    let mut last = 0.0;
    for k in 1..=31 {
        let response = kernel_response(&kernel, k as f32 * 0.1);
        assert!(response > last, "q {}: {response}", k as f32 * 0.1);
        last = response;
    }
    // ω = √(g R(q)): over the visible ripples the phase speed ω / q falls
    // as q grows, so short waves run slower than long ones.
    let speed = |q: f32| kernel_response(&kernel, q).sqrt() / q;
    assert!(speed(0.8) > speed(2.0));
}

/// A stress scenario: a hundred movers and impacts at once. The field
/// takes at most its tier's budget, nearest first.
#[test]
fn sources_stay_within_each_tier_s_budget() {
    let mut sources = Vec::new();
    for i in 0..100 {
        let a = i as f32 * 0.7;
        let at = Vec2::new(a.cos(), a.sin()) * (1.0 + i as f32 * 0.2);
        sources.push(if i % 3 == 0 {
            Source::impact(at, 0.3, 0.04)
        } else {
            Source::mover(at, Vec2::new(a.sin(), -a.cos()) * 1.5, 0.4)
        });
    }
    for (tier, budget) in [(Tier::Low, 8), (Tier::Medium, 16), (Tier::High, 32)] {
        let mut r = Ripples::for_tier(tier);
        let mut t = 0.0;
        for _ in 0..30 {
            t += 1.0 / 60.0;
            r.advance(t, Vec2::ZERO, &sources, None);
            let s = r.stats();
            assert_eq!(s.offered, 100);
            assert_eq!(s.applied, budget, "{tier:?}");
            assert!(s.wakes <= budget);
            assert!(s.steps <= MOST_STEPS);
        }
        assert!(r.energy().is_finite());
        assert_eq!(r.plan().sources, budget);
    }
}

/// The window follows the eye in whole cells; what is in the water stays
/// where it was in the world.
#[test]
fn scrolling_keeps_the_ripples_in_place() {
    let mut r = Ripples::for_tier(Tier::Medium);
    r.advance(0.0, Vec2::ZERO, &[], None);
    r.advance(
        1.0 / 60.0,
        Vec2::ZERO,
        &[Source::impact(Vec2::new(2.0, 1.0), 0.5, 0.05)],
        None,
    );
    let before = (r.height_at(2.0, 1.0), r.foam_at(2.0, 1.0));
    let corner = r.corner();
    // The eye moves 3.3 m east; nothing steps (same time).
    r.advance(1.0 / 60.0, Vec2::new(3.3, 0.0), &[], None);
    assert_ne!(r.corner(), corner);
    let cell = r.plan().cell;
    let moved = (r.corner() - corner) / cell;
    assert_eq!(moved, moved.round());
    let after = (r.height_at(2.0, 1.0), r.foam_at(2.0, 1.0));
    assert!((before.0 - after.0).abs() < 1e-6 && (before.1 - after.1).abs() < 1e-6);
    assert!(before.1 > 0.0);
}

fn pond(x: f32, z: f32) -> Option<[f32; 2]> {
    (x * x + z * z < 9.0).then_some([0.0, 0.0])
}

fn stream(x: f32, _z: f32) -> Option<[f32; 2]> {
    (x.abs() < 50.0).then_some([0.8, 0.0])
}

/// Dry ground holds no ripple and no foam; foam on a current drifts
/// downstream and fades.
#[test]
fn dry_cells_hold_still_and_foam_drifts_with_the_current() {
    let mut r = Ripples::for_tier(Tier::Low);
    r.advance(0.0, Vec2::ZERO, &[], Some(Wet(pond)));
    let mut t = 0.0;
    for _ in 0..120 {
        t += 1.0 / 60.0;
        r.advance(
            t,
            Vec2::ZERO,
            &[Source::impact(Vec2::new(2.5, 0.0), 0.6, 0.05)],
            Some(Wet(pond)),
        );
    }
    assert_eq!(r.height_at(5.0, 0.0), 0.0);
    assert_eq!(r.foam_at(5.0, 0.0), 0.0);
    assert!(r.foam_at(2.5, 0.0) > 0.0);

    let mut r = Ripples::for_tier(Tier::Low);
    r.advance(0.0, Vec2::ZERO, &[], Some(Wet(stream)));
    r.advance(
        1.0 / 60.0,
        Vec2::ZERO,
        &[Source::impact(Vec2::ZERO, 0.3, 0.05)],
        Some(Wet(stream)),
    );
    let start = r.foam_at(0.0, 0.0);
    let mut t = 1.0 / 60.0;
    for _ in 0..90 {
        t += 1.0 / 60.0;
        r.advance(t, Vec2::ZERO, &[], Some(Wet(stream)));
    }
    // 1.5 s at 0.8 m/s: the foam is about 1.2 m downstream.
    let there = r.foam_at(1.2, 0.0);
    assert!(there > r.foam_at(0.0, 0.0) * 2.0, "{there}");
    assert!(there < start, "it fades");
}

/// A mover draws a V behind it: the foam of its drawn wedge peaks across
/// its path at about the Kelvin angle.
#[test]
fn a_mover_draws_a_kelvin_wedge() {
    let mut r = Ripples::for_tier(Tier::High);
    let velocity = Vec2::new(1.5, 0.0);
    let mover = Source::mover(Vec2::ZERO, velocity, 0.3);
    r.advance(0.0, Vec2::ZERO, &[mover], None);
    r.advance(1.0 / 30.0, Vec2::ZERO, &[mover], None);
    assert_eq!(r.stats().wakes, 1);
    let texels = r.texels();
    let n = r.plan().size;
    let cell = r.plan().cell;
    let corner = r.corner();
    let foam = |x: f32, z: f32| {
        let i = ((x - corner.x) / cell) as usize;
        let j = ((z - corner.y) / cell) as usize;
        half::f16::from_bits(texels[j * n + i][3]).to_f32()
    };
    let behind = 3.0;
    let peak = (0..40)
        .map(|k| k as f32 * 0.05)
        .max_by(|a, b| foam(-behind, *a).total_cmp(&foam(-behind, *b)))
        .unwrap();
    let expected = behind * KELVIN.tan();
    assert!(
        (peak - expected).abs() < 0.35,
        "arm at {peak}, expected {expected}"
    );
    // Nothing ahead of it.
    assert_eq!(foam(1.0, 0.0), 0.0);
    // A still body draws none.
    r.advance(
        2.0 / 30.0,
        Vec2::ZERO,
        &[Source::mover(Vec2::ZERO, Vec2::ZERO, 0.3)],
        None,
    );
    assert_eq!(r.stats().wakes, 0);
}

/// A boat rowed across still water leaves a foam trail that lasts behind
/// it, with ripples of a few centimeters, on every kernel.
#[test]
fn a_mover_leaves_a_lasting_foam_trail() {
    for tier in [Tier::Low, Tier::High] {
        let mut r = Ripples::for_tier(tier);
        let mut t = 0.0;
        let mut at = Vec2::new(-3.0, 0.0);
        let v = Vec2::new(1.5, 0.0);
        for _ in 0..240 {
            t += 1.0 / 60.0;
            at += v / 60.0;
            r.advance(t, Vec2::ZERO, &[Source::mover(at, v, 0.9)], None);
        }
        let tex = r.texels();
        let f = |k: usize| half::f16::from_bits(tex[k][3]).to_f32();
        let h = |k: usize| half::f16::from_bits(tex[k][0]).to_f32();
        let s = |k: usize| half::f16::from_bits(tex[k][1]).to_f32().abs();
        let n = tex.len();
        let maxf = (0..n).map(f).fold(0.0, f32::max);
        let maxh = (0..n).map(h).map(f32::abs).fold(0.0, f32::max);
        let maxs = (0..n).map(s).fold(0.0, f32::max);
        let cover = (0..n).filter(|&k| f(k) > 0.3).count() as f32 * r.plan().cell * r.plan().cell;
        assert!(maxf > 0.5, "{tier:?}: foam {maxf}");
        assert!(cover > 2.0, "{tier:?}: trail {cover} m²");
        assert!(
            r.foam_at(at.x - 2.0, 0.0) > 0.3,
            "{tier:?}: foam 2 m behind"
        );
        assert!((0.005..0.3).contains(&maxh), "{tier:?}: height {maxh}");
        assert!(maxs > 0.01, "{tier:?}: slope {maxs}");
    }
}

/// Skipping an empty kernel preserves its clock phase and later impulses.
#[test]
fn empty_fields_skip_work_and_resume_on_a_pulse() {
    for tier in Tier::ALL {
        let mut field = Ripples::for_tier(tier);
        field.advance(0.0, Vec2::ZERO, &[], None);
        field.advance(0.041, Vec2::ZERO, &[], None);
        assert_eq!(field.stats().steps, 0);
        assert!(field.is_quiet());
        let dt = 1.0 / field.plan.rate;
        let mut expected_clock = 0.0;
        while expected_clock + dt <= 0.041 {
            expected_clock += dt;
        }
        assert_eq!(field.clock, Some(expected_clock));
        let mut reference = field.clone();
        let source = Source::impact(Vec2::ZERO, 0.3, 0.05);
        reference.inject(&source, dt);
        reference.step(dt);
        field.advance(expected_clock + dt, Vec2::ZERO, &[source], None);
        assert_eq!(field.stats().steps, 1);
        assert_eq!(field.h, reference.h);
        assert_eq!(field.prev, reference.prev);
        assert_eq!(field.foam, reference.foam);
        assert!(field.energy() > 0.0);
        // A zero crossing can retain velocity/history and must keep stepping.
        field.h.fill(0.0);
        field.foam.fill(0.0);
        field.prev[field.plan.size * field.plan.size / 2] = 0.001;
        field.advance(field.clock.unwrap() + dt, Vec2::ZERO, &[], None);
        assert!(field.stats().steps > 0);
    }
}
