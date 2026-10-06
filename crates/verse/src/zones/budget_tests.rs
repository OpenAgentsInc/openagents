//! Destruction against the quality budgets: tearing down every building in
//! Everglade, then its debris, and the Grove's tower again and again, keeps
//! every frame's geometry within each tier's budget, so the renderer never
//! has to trim a frame, and nothing in the runtime turns a budget into a
//! stop.

use crate::controller::InputState;
use crate::render::{fit_frame, mesh_resources};
use crate::runtime::WorldRuntime;
use crate::ui::UiBatch;
use crate::zones::everglade::demolition::town::{POOL_BYTES, Town};
use crate::zones::everglade::height;
use crate::zones::grove::layout::TOWER;
use glam::Vec3;
use verse_engine::quality::{Resources, Tier};

const DT: f32 = 1.0 / 60.0;

fn town_mut(runtime: &mut WorldRuntime) -> &mut Town {
    runtime
        .zone_state
        .everglade
        .as_mut()
        .and_then(|glade| glade.town_mut())
        .expect("a destructible town")
}

/// The most geometry a frame and the world have held so far.
#[derive(Default)]
struct Peak {
    pool: usize,
    frame: u64,
    frames: usize,
}

/// Ticks `runtime` for `seconds`, checking the town's pool every frame and
/// the whole frame against every tier's budget every few frames.
fn run(runtime: &mut WorldRuntime, base: Resources, seconds: f32, peak: &mut Peak) {
    for frame in 0..(seconds / DT).round() as usize {
        runtime.tick(&InputState::default(), DT);
        let pool = town_mut(runtime).geometry_bytes();
        assert!(pool <= POOL_BYTES, "the town holds {pool} bytes");
        peak.pool = peak.pool.max(pool);
        if frame % 15 == 0 {
            let dynamic = runtime.dynamic_mesh();
            let frame_bytes = mesh_resources(&dynamic).unwrap().geometry_bytes;
            peak.frame = peak.frame.max(frame_bytes);
            peak.frames += 1;
            for tier in Tier::ALL {
                let quality = tier.quality();
                let budget = quality.budget();
                assert!(
                    frame_bytes <= budget.dynamic_geometry_bytes,
                    "{}: the frame's {frame_bytes} bytes pass the reserve",
                    tier.name()
                );
                let (trimmed, _) = fit_frame(base, quality, &dynamic, &UiBatch::default())
                    .expect("a frame never stops for its budget");
                assert!(
                    trimmed.is_none(),
                    "{}: the frame had to be trimmed",
                    tier.name()
                );
            }
        }
    }
}

/// The world's resources, checked to leave each tier's reserve free.
fn world_base(runtime: &WorldRuntime) -> Resources {
    let base = mesh_resources(&runtime.world.mesh).unwrap();
    for tier in Tier::ALL {
        let budget = tier.quality().budget();
        assert!(
            base.geometry_bytes <= budget.static_geometry_bytes(),
            "{}: the world's {} bytes leave no room for destruction",
            tier.name(),
            base.geometry_bytes
        );
    }
    base
}

#[test]
fn destroying_every_everglade_building_and_its_debris_stays_within_every_tier_budget() {
    let mut runtime = crate::zones::everglade_tests::entered();
    let base = world_base(&runtime);
    let targets: Vec<Vec3> = town_mut(&mut runtime)
        .buildings()
        .iter()
        .filter(|b| b.destructible())
        .map(|b| {
            let ([cx, cz], _) = b.rect;
            Vec3::new(cx, height(cx, cz) + 2.0, cz)
        })
        .collect();
    assert!(targets.len() > 50, "{} buildings", targets.len());
    let mut peak = Peak::default();
    // Twice over: the whole town, then again into its debris and what
    // regrew.
    for _ in 0..2 {
        for group in targets.chunks(4) {
            for &at in group {
                town_mut(&mut runtime).blast(at, 9.0, 400, Vec3::ZERO);
            }
            run(&mut runtime, base, 0.75, &mut peak);
        }
    }
    run(&mut runtime, base, 5.0, &mut peak);
    // Restoring the town frees the debris' buffers.
    town_mut(&mut runtime).restore();
    run(&mut runtime, base, 0.5, &mut peak);
    assert_eq!(town_mut(&mut runtime).geometry_bytes(), 0);
    assert!(peak.pool > 0 && peak.frames > 100);
    eprintln!(
        "Everglade: world {} MiB, peak town {} MiB, peak frame {} MiB",
        base.geometry_bytes >> 20,
        peak.pool >> 20,
        peak.frame >> 20
    );
}

#[test]
fn breaking_the_grove_tower_again_and_again_stays_within_every_tier_budget() {
    let mut runtime = WorldRuntime::new();
    runtime.install_grove(crate::zones::everglade_tests::pack());
    runtime
        .set_spawn(Vec3::new(TOWER[0], 0.0, TOWER[1] - 20.0), 0.0)
        .unwrap();
    let base = world_base(&runtime);
    let mut peak = Peak::default();
    let ground = height(TOWER[0], TOWER[1]);
    for round in 0..4 {
        for level in 0..8 {
            let side = if (round + level) % 2 == 0 { -1.0 } else { 1.0 };
            let at = Vec3::new(
                TOWER[0] + side * 2.0,
                ground + 1.0 + 3.0 * level as f32,
                TOWER[1],
            );
            town_mut(&mut runtime).blast(at, 6.0, 400, Vec3::X * side);
            run(&mut runtime, base, 0.5, &mut peak);
        }
        run(&mut runtime, base, 3.0, &mut peak);
        town_mut(&mut runtime).restore();
        run(&mut runtime, base, 0.25, &mut peak);
        assert_eq!(town_mut(&mut runtime).geometry_bytes(), 0, "round {round}");
    }
    assert!(peak.pool > 0);
    eprintln!(
        "Grove: world {} MiB, peak tower {} MiB, peak frame {} MiB",
        base.geometry_bytes >> 20,
        peak.pool >> 20,
        peak.frame >> 20
    );
}

/// No runtime source can turn a budget into a stop: the budget types have
/// no refusing check, and no budget refusal message is left in the
/// renderer, the zones, or the browser and phone hosts.
#[test]
fn no_runtime_path_returns_a_fatal_budget_error() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let crates = [
        "crates/verse/src",
        "crates/verse-engine/src",
        "crates/verse-pbr/src",
        "crates/verse-core/src",
        "crates/verse-zone-everglade/src",
        "crates/verse-zone-grove/src",
        "crates/everglade-web/src",
        "crates/coder-mobile/src",
    ];
    let refusals = [
        "admitted quality budget",
        "admitted instance budget",
        "exceeds retained GPU geometry",
        "exceeds its physical geometry bounds",
        ".admit(resources)",
    ];
    let mut found = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = crates.iter().map(|c| root.join(c)).collect();
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                stack.push(entry.unwrap().path());
            }
        } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("budget_tests.rs")
        {
            let text = std::fs::read_to_string(&path).unwrap();
            for refusal in refusals {
                if text.contains(refusal) {
                    found.push(format!("{}: {refusal}", path.display()));
                }
            }
        }
    }
    assert!(found.is_empty(), "budget refusals remain: {found:#?}");
    // An over-budget world and frame are measured, never refused.
    let budget = Tier::Low.quality().budget();
    let over = budget.overrun(Resources {
        geometry_bytes: budget.geometry_bytes * 2,
        ..Resources::default()
    });
    assert!(!over.fits());
}
