use glam::{DVec3, Vec3};
use physics::fluid::{Fluid, Settings, Tank};

use super::{GpuFluid, Snapshot, headless};

fn dam_break(scale: f64) -> Fluid {
    let tank = Tank {
        min: DVec3::ZERO,
        max: DVec3::new(0.4, 0.4, 0.15) * DVec3::new(scale, 1.0, scale),
    };
    let mut f = Fluid::new(Settings::default(), Some(tank));
    f.add_block(
        DVec3::ZERO,
        DVec3::new(0.125 * scale, 0.25, 0.15 * scale),
        DVec3::ZERO,
    );
    f
}

/// The CPU's [`physics::fluid::Stats`] measures, on a GPU snapshot.
#[derive(Clone, Copy, Debug)]
struct Measures {
    density_error: f64,
    surface: f64,
    com: DVec3,
}

fn measure(positions: &[DVec3], densities: &[f64], fluid: &Fluid) -> Measures {
    let rho0 = fluid.settings.rest_density;
    let mut heights: Vec<f64> = positions.iter().map(|p| p.y).collect();
    heights.sort_by(f64::total_cmp);
    let surface = heights[(heights.len() as f64 * 0.95) as usize] + fluid.settings.radius;
    let h = fluid.kernel_radius();
    let deep: Vec<f64> = positions
        .iter()
        .zip(densities)
        .filter(|(p, _)| p.y < surface - h)
        .map(|(_, d)| (d / rho0 - 1.0).abs())
        .collect();
    Measures {
        density_error: deep.iter().sum::<f64>() / deep.len().max(1) as f64,
        surface,
        com: positions.iter().copied().sum::<DVec3>() / positions.len() as f64,
    }
}

fn gpu_measures(snapshot: &Snapshot, fluid: &Fluid) -> Measures {
    let positions: Vec<DVec3> = snapshot.positions.iter().map(|p| p.as_dvec3()).collect();
    let densities: Vec<f64> = snapshot.densities.iter().map(|d| f64::from(*d)).collect();
    measure(&positions, &densities, fluid)
}

#[test]
fn the_gpu_dam_break_matches_the_cpu_reference() {
    let Some((device, queue, adapter)) = headless() else {
        eprintln!("no GPU adapter; skipped");
        return;
    };
    let mut cpu = dam_break(1.0);
    let gpu = GpuFluid::new(&device, &cpu);
    // Sample times in steps of 1/240 s: the collapse, the run-up on the
    // far wall, the slosh back, and the settled tank.
    let samples = [60u32, 120, 240, 480, 960];
    let mut done = 0;
    for &at in &samples {
        gpu.step(&device, &queue, at - done);
        while cpu.tick < u64::from(at) {
            cpu.step(None);
        }
        done = at;
        let c = measure(cpu.positions(), cpu.densities(), &cpu);
        let g = gpu_measures(&gpu.read(&device, &queue), &cpu);
        eprintln!(
            "{adapter} t {:.2}: cpu {c:?} gpu {g:?}",
            f64::from(at) / 240.0
        );
        let settled = at >= 960;
        // While the wave runs the centres of mass may be up to 1.5 cm apart
        // (the tank is 40 cm long); settled, within 5 mm. On an M5 Max they
        // stay within 1 mm throughout.
        let com = if settled { 0.005 } else { 0.015 };
        assert!((c.com - g.com).length() < com, "t {at}: {c:?} vs {g:?}");
        assert!(
            (c.density_error - g.density_error).abs() < 0.005,
            "t {at}: {c:?} vs {g:?}"
        );
        if settled {
            assert!(g.density_error < 0.01, "{g:?}");
            assert!((c.surface - g.surface).abs() < 0.005, "{c:?} vs {g:?}");
        }
    }
}

#[test]
#[ignore = "explicit GPU timing; prints the step time for 50k+ particles"]
fn fifty_thousand_particles_step_inside_a_60_hz_frame() {
    let Some((device, queue, adapter)) = headless() else {
        eprintln!("no GPU adapter; skipped");
        return;
    };
    // The dam break, five times longer and wider: about 55k particles.
    let fluid = dam_break(5.0);
    let gpu = GpuFluid::new(&device, &fluid);
    assert!(gpu.len() >= 50_000, "{}", gpu.len());
    gpu.step(&device, &queue, 30);
    let frames = 300;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        gpu.step(&device, &queue, 1);
    }
    let per_frame = start.elapsed().as_secs_f64() / f64::from(frames);
    let snapshot = gpu.read(&device, &queue);
    assert!(snapshot.positions.iter().all(|p: &Vec3| p.is_finite()));
    eprintln!(
        "{adapter}: {} particles, {} iterations, one step per frame: {:.2} ms per frame ({:.0} fps)",
        gpu.len(),
        fluid.settings.iterations,
        per_frame * 1e3,
        1.0 / per_frame
    );
    assert!(per_frame < 1.0 / 60.0, "{per_frame} s per frame");
}

#[test]
fn the_liquid_shaders_validate() {
    for (name, source) in [("pbf", super::SHADER), ("render", super::render::SHADER)] {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    }
}

/// A zone's tank (the Water Lab's, away from the origin) breaks its dam
/// and stays a liquid in its tank: finite, inside, and settling.
#[test]
fn a_lab_tank_stays_in_its_tank() {
    for round in [0, 1] {
        let tank = super::LiquidTank {
            min: Vec3::new(6.7, 1.2, 11.7),
            max: Vec3::new(8.3, 1.9, 12.3),
            radius: 0.025,
            column: 0.3,
            height: 0.55,
            round,
        };
        let mut fluid = tank.fluid();
        assert!(fluid.len() > 1000, "{} particles", fluid.len());
        let steps = (4.0 / fluid.settings.dt) as usize;
        for step in 0..steps {
            fluid.step(None);
            let lost = fluid
                .positions()
                .iter()
                .filter(|p| {
                    !p.is_finite()
                        || p.y < f64::from(tank.min.y) - 0.05
                        || p.y > f64::from(tank.max.y) + 0.3
                })
                .count();
            assert_eq!(
                lost, 0,
                "round {round}: {lost} particles lost by step {step}"
            );
        }
    }
}
