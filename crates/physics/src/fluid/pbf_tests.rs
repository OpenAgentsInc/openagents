use glam::DVec3;

use super::{Fluid, Settings, Tank};
use crate::{Body, Uniform, World};

fn dam_break() -> Fluid {
    let tank = Tank {
        min: DVec3::ZERO,
        max: DVec3::new(0.4, 0.4, 0.15),
    };
    let mut f = Fluid::new(Settings::default(), Some(tank));
    f.add_block(DVec3::ZERO, DVec3::new(0.125, 0.25, 0.15), DVec3::ZERO);
    f
}

/// The mean over `d`-wide columns of the highest particle centre, plus `r`:
/// the free surface where `keep` holds.
fn surface(f: &Fluid, keep: impl Fn(DVec3) -> bool) -> f64 {
    let d = 2.0 * f.settings.radius;
    let mut tops = std::collections::BTreeMap::new();
    for p in f.positions().iter().filter(|p| keep(**p)) {
        let column = ((p.x / d).floor() as i64, (p.z / d).floor() as i64);
        let top = tops.entry(column).or_insert(f64::MIN);
        *top = p.y.max(*top);
    }
    tops.values().sum::<f64>() / tops.len() as f64 + f.settings.radius
}

fn bits(f: &Fluid) -> Vec<u64> {
    f.positions()
        .iter()
        .chain(f.velocities())
        .flat_map(|v| v.to_array())
        .map(f64::to_bits)
        .collect()
}

#[test]
fn a_dam_break_settles_at_rest_density_with_a_steady_fill_height() {
    let mut f = dam_break();
    assert!(f.len() <= 5_000 && f.len() > 2_000, "{}", f.len());
    let mut moved = 0.0_f64;
    let mut late = Vec::new();
    for step in 0..960 {
        f.step(None);
        if step == 60 {
            moved = f.stats().center_of_mass.x;
        }
        if step >= 720 && step % 24 == 0 {
            late.push(f.stats());
        }
    }
    // The column collapsed and ran across the tank.
    assert!(moved > 0.1, "{moved}");
    let last = f.stats();
    assert!(last.density_error < 0.01, "{last:?}");
    let surfaces: Vec<f64> = late.iter().map(|s| s.surface).collect();
    let (lo, hi) = surfaces
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), s| (a.min(*s), b.max(*s)));
    // In the last second the surface moves by less than a particle and
    // stands within 10% of the fill height the volume implies.
    assert!(hi - lo < 2.0 * f.settings.radius, "{surfaces:?}");
    let level = surface(&f, |_| true);
    assert!(
        (level / last.fill_height - 1.0).abs() < 0.1,
        "{level} vs {}",
        last.fill_height
    );
    for s in &late {
        assert!(s.density_error < 0.01, "{s:?}");
    }
}

#[test]
fn pressure_in_a_resting_column_grows_at_rho_g() {
    let tank = Tank {
        min: DVec3::ZERO,
        max: DVec3::new(0.15, 0.3, 0.1),
    };
    let mut f = Fluid::new(Settings::default(), Some(tank));
    f.add_block(DVec3::ZERO, DVec3::new(0.15, 0.2, 0.1), DVec3::ZERO);
    for _ in 0..360 {
        f.step(None);
    }
    let fill = f.stats().fill_height;
    let h = f.kernel_radius();
    // Below the surface layer and above the floor's, a straight line.
    let points: Vec<(f64, f64)> = f
        .positions()
        .iter()
        .zip(f.pressure())
        .filter(|(p, _)| p.y > h && p.y < fill - h)
        .map(|(p, q)| (p.y, *q))
        .collect();
    let n = points.len() as f64;
    let (my, mp) = points
        .iter()
        .fold((0.0, 0.0), |(a, b), (y, p)| (a + y / n, b + p / n));
    let slope = points.iter().map(|(y, p)| (y - my) * (p - mp)).sum::<f64>()
        / points.iter().map(|(y, _)| (y - my).powi(2)).sum::<f64>();
    let expected = -1000.0 * 9.81;
    assert!((slope / expected - 1.0).abs() < 0.05, "dp/dy {slope}");
    // The line reaches zero within a particle of the surface.
    let zero = my - mp / slope;
    assert!(
        (zero - fill).abs() < 2.0 * f.settings.radius,
        "{zero} vs {fill}"
    );
}

#[test]
fn a_floating_box_reaches_its_archimedes_draft() {
    for density in [250.0, 500.0, 750.0] {
        let s = Settings::default();
        let side = 0.3;
        let tank = Tank {
            min: DVec3::ZERO,
            max: DVec3::new(side, side, side),
        };
        let mut f = Fluid::new(s, Some(tank));
        f.add_block(DVec3::ZERO, DVec3::new(side, 0.1, side), DVec3::ZERO);
        assert!(f.len() <= 5_000, "{}", f.len());
        let mut world = World::new(s.dt);
        let half = DVec3::new(0.06, 0.0375, 0.06);
        let mass = density * 8.0 * half.x * half.y * half.z;
        let start = DVec3::new(side / 2.0, 0.1 + half.y + 0.01, side / 2.0);
        let id = world.add(Body::new(mass, Body::box_inertia(mass, half * 2.0), start));
        f.couple_box(id, half);
        for _ in 0..720 {
            f.step(Some(&mut world));
            world.step(&Uniform(s.gravity));
        }
        let body = world.bodies()[id.0 as usize];
        // The water level away from the box and the walls.
        let level = surface(&f, |p| {
            (p.x - side / 2.0).abs().max((p.z - side / 2.0).abs()) > 0.1
                && p.x.min(p.z) > 0.025
                && p.x.max(p.z) < side - 0.025
        });
        let draft = level - (body.pos.y - half.y);
        let expected = density / 1000.0 * 2.0 * half.y;
        assert!(
            (draft / expected - 1.0).abs() < 0.05,
            "{density} kg/m^3: draft {draft} m, expected {expected} m"
        );
        assert!(body.vel.length() < 0.01, "{:?}", body.vel);
        assert!(body.orientation.to_axis_angle().1 < 0.1);
    }
}

#[test]
fn two_droplets_touch_and_coalesce_into_one_round_drop() {
    let settings = Settings {
        gravity: DVec3::ZERO,
        surface_tension: 0.2,
        ..Settings::default()
    };
    let mut f = Fluid::new(settings, None);
    let radius = 0.04;
    let gap = 0.016;
    f.add_ball(
        DVec3::new(-(radius + gap / 2.0), 0.0, 0.0),
        radius,
        DVec3::ZERO,
    );
    f.add_ball(
        DVec3::new(radius + gap / 2.0, 0.0, 0.0),
        radius,
        DVec3::ZERO,
    );
    let link = 1.5 * 2.0 * settings.radius;
    assert_eq!(clusters(f.positions(), link), 2);
    for _ in 0..720 {
        f.step(None);
    }
    assert_eq!(clusters(f.positions(), link), 1);
    let p = f.positions();
    let extent = |a: usize| {
        let (lo, hi) = p.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| {
            (lo.min(q[a]), hi.max(q[a]))
        });
        hi - lo
    };
    let (x, y, z) = (extent(0), extent(1), extent(2));
    // One drop about as wide as it is long: the pair started 2.2 times as
    // long as wide.
    assert!(x / y.min(z) < 1.25 && x < 0.11, "{x} {y} {z}");
    // Without surface tension the same pair never joins up.
    let mut loose = Fluid::new(
        Settings {
            surface_tension: 0.0,
            ..settings
        },
        None,
    );
    loose.add_ball(
        DVec3::new(-(radius + gap / 2.0), 0.0, 0.0),
        radius,
        DVec3::ZERO,
    );
    loose.add_ball(
        DVec3::new(radius + gap / 2.0, 0.0, 0.0),
        radius,
        DVec3::ZERO,
    );
    for _ in 0..240 {
        loose.step(None);
    }
    assert!(clusters(loose.positions(), link) > 2);
}

fn clusters(p: &[DVec3], link: f64) -> usize {
    let mut parent: Vec<usize> = (0..p.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for i in 0..p.len() {
        for j in i + 1..p.len() {
            if (p[i] - p[j]).length() < link {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    (0..p.len()).filter(|&i| root(&mut parent, i) == i).count()
}

#[test]
fn a_run_is_bit_for_bit_the_same_on_one_thread_and_many() {
    let run = |serial: bool| {
        let mut f = dam_break();
        f.settings.surface_tension = 0.05;
        crate::parallel::SERIAL.store(serial, std::sync::atomic::Ordering::Relaxed);
        for _ in 0..90 {
            f.step(None);
        }
        crate::parallel::SERIAL.store(false, std::sync::atomic::Ordering::Relaxed);
        bits(&f)
    };
    let a = run(false);
    assert_eq!(a, run(false), "two runs differ");
    assert_eq!(a, run(true), "threads change the result");
}

fn plate(f: &mut Fluid, below: i32) {
    let d = 2.0 * f.settings.radius;
    let mut points = Vec::new();
    for i in -10..=10 {
        for j in -below..=10 {
            points.push(DVec3::new(f64::from(i) * d, f64::from(j) * d, 0.0));
        }
    }
    f.add_boundary(points);
}

#[test]
fn adhesion_draws_a_drop_onto_a_wall() {
    // A drop just off a plate, without gravity: adhesion pulls it on.
    let gap = |adhesion: f64| {
        let settings = Settings {
            adhesion,
            surface_tension: 0.2,
            gravity: DVec3::ZERO,
            ..Settings::default()
        };
        let mut f = Fluid::new(settings, None);
        plate(&mut f, 10);
        f.add_ball(DVec3::new(0.0, 0.0, 0.045), 0.025, DVec3::ZERO);
        for _ in 0..240 {
            f.step(None);
        }
        f.positions().iter().map(|p| p.z).fold(f64::MAX, f64::min)
    };
    let (free, held) = (gap(0.0), gap(20.0));
    assert!(
        held < 0.02 && held < free - 0.004,
        "nearest {held} m with adhesion, {free} m without"
    );
}

#[test]
fn a_no_slip_wall_slows_a_film_running_down_it() {
    let fall = |boundary_viscosity: f64| {
        let settings = Settings {
            adhesion: 20.0,
            surface_tension: 0.2,
            boundary_viscosity,
            ..Settings::default()
        };
        let mut f = Fluid::new(settings, None);
        plate(&mut f, 40);
        let d = 2.0 * settings.radius;
        f.add_block(
            DVec3::new(-0.04, 0.0, 0.0105),
            DVec3::new(0.04, 0.05, 0.0105 + 2.0 * d),
            DVec3::ZERO,
        );
        let start = f.stats().center_of_mass.y;
        for _ in 0..60 {
            f.step(None);
        }
        start - f.stats().center_of_mass.y
    };
    let (slides, held) = (fall(0.0), fall(0.5));
    assert!(
        held < 0.5 * slides,
        "no-slip {held} m, free-slip {slides} m"
    );
}
