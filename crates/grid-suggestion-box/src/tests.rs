#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use verse_engine::assets::{Pack, Texture};
use verse_engine::inventory::{Asset, AssetId, Binding, Inventory, License, Origin};

use super::*;

fn tint(tone: u8) -> [f32; 3] {
    [f32::from(tone) / 3.0; 3]
}

/// A pack with only the Grid's white texel, as the Grid's start has it.
fn bare_pack() -> Pack {
    let digest = "0".repeat(64);
    Pack {
        inventory: Some(Inventory {
            version: 1,
            compiler: AssetId::new("verse:compiler:grid").unwrap(),
            compiler_revision: digest.clone(),
            assets: vec![
                Asset {
                    id: AssetId::new("verse:source:project").unwrap(),
                    binding: Binding::Source {
                        origin: Origin {
                            creator: "OpenAgents contributors".into(),
                            license: License::Apache2,
                            revision: digest.clone(),
                            format: "rust".into(),
                        },
                    },
                    sha256: digest.clone(),
                    bytes: 1,
                    dependencies: vec![],
                },
                Asset {
                    id: AssetId::new("verse:texture:grid/0").unwrap(),
                    binding: Binding::Texture { slot: 0 },
                    sha256: digest.clone(),
                    bytes: 1,
                    dependencies: vec![AssetId::new("verse:source:project").unwrap()],
                },
            ],
        }),
        version: 1,
        source_revision: digest.clone(),
        models: BTreeMap::new(),
        textures: vec![Texture {
            file: "verse-flat-white.png".into(),
            sha256: digest,
            width: 1,
            height: 1,
        }],
        placements: vec![],
    }
}

#[test]
fn the_box_is_admitted_small_and_unlit() {
    let mut pack = bare_pack();
    add(&mut pack, 0, "verse:texture:grid/0", &tint).unwrap();
    pack.validate().unwrap();
    assert_eq!(pack.models.len(), 1);
    assert_eq!(pack.placements.len(), 1);
    let model = &pack.models[MODEL];
    assert!(model.surfaces.iter().all(|s| s.unlit && !s.emissive));
    let vertices: usize = model.surfaces.iter().map(|s| s.vertices.len()).sum();
    assert!(vertices < 1_000, "{vertices}");
    // A pack without the white texel is refused.
    assert!(add(&mut bare_pack(), 0, "verse:texture:grid/9", &tint).is_err());
}

fn flat(p: [f64; 3]) -> [f64; 2] {
    [p[0], p[2]]
}

fn apart(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// The distance from `p` to the segment `a`–`b`.
fn from_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
    apart(p, [a[0] + t * dx, a[1] + t * dz])
}

/// The plaza's places that live outside this crate's dependencies, from
/// their own constants: the spawn (`verse_core::world::SPAWN`), the spawn
/// area (`verse_world::social::arrival::SpawnArea::GRID`), the Game Lab
/// arch (`verse::zones::game_place`), and the walk-in arches
/// (`verse::zones::gate`, side and forward of the spawn): Everglade, the
/// hidden Lagrange 1, Lagrange 2, RITUAL, and every hosted arch the Grid
/// may show (ICE CREAM MAN, then each 7 m outboard, up to six).
const SPAWN: [f64; 2] = [0.0, -10.0];
const SPAWN_AREA: ([f64; 2], f64) = ([0.0, -14.0], 6.0);
const GAME_LAB: [f64; 2] = [-8.0, 3.0];
const GAME_LAB_REACH: f64 = 4.5;
const ARCHES: [[f64; 2]; 4] = [[9.0, 1.0], [-9.0, 1.0], [-20.0, 9.0], [0.0, 16.0]];
const HOSTED: ([f64; 2], f64, usize) = ([20.0, 9.0], 7.0, 6);
/// The room an arch's approach takes in front of it, m.
const ARCH_ROOM: f64 = 6.0;
/// What a spawn keeps clear of each reach (`spawn_guard::MARGIN`), m.
const SPAWN_MARGIN: f64 = 2.0;

#[test]
fn the_box_stands_on_the_landing_plaza_clear_of_everything_else() {
    let at = flat(POSITION);
    let stand = flat(stand());
    // Near the spawn, and seen from it, but out of the spawn area's way:
    // nobody arrives inside its reach.
    let spawn = apart(at, SPAWN);
    assert!((8.0..16.0).contains(&spawn), "{spawn}");
    assert!(apart(stand, SPAWN_AREA.0) > SPAWN_AREA.1 + LEAVE_RANGE + SPAWN_MARGIN);
    // The arches and their approaches.
    let hosted = (0..HOSTED.2).map(|i| [HOSTED.0[0] + HOSTED.1 * i as f64, HOSTED.0[1]]);
    for arch in ARCHES.into_iter().chain(hosted) {
        assert!(apart(at, arch) > ARCH_ROOM + 2.0, "{arch:?}");
    }
    assert!(apart(stand, GAME_LAB) > GAME_LAB_REACH + LEAVE_RANGE + 4.0);
    // The help desk, and where people stand to ask there.
    let desk = flat(grid_help_desk::POSITION);
    assert!(apart(at, desk) > 10.0);
    let ask = flat(grid_help_desk::world([0.0, 0.0, grid_help_desk::STAND]));
    assert!(apart(stand, ask) > grid_help_desk::LEAVE_RANGE + LEAVE_RANGE + 4.0);
    // The terminal and the Foreman's walk between its home and the terminal.
    let terminal = flat(pylons_view::terminal::POSITION);
    assert!(apart(at, terminal) > 9.0);
    assert!(!pylons_view::terminal::in_reach(super::stand(), true));
    let home = flat(grid_foreman::HOME);
    let reach = grid_foreman::TALK_LEAVE + LEAVE_RANGE;
    assert!(
        from_segment(stand, home, terminal) > reach,
        "the Foreman's walk"
    );
    // The build billboard and the room in front of it.
    let board = deploy_board::layout::SITE;
    assert!(apart(at, board) > 10.0);
    let front = deploy_board::layout::FACES;
    assert!(from_segment(at, board, front) > 8.0);
    // The Pylon Bay's pad, with room to walk around it.
    let [u0, u1, v0, v1] = pylons_view::bay::PAD;
    let (a, b) = (
        pylons_view::bay::world(u0, v0),
        pylons_view::bay::world(u1, v1),
    );
    let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
    let (z0, z1) = (a[2].min(b[2]), a[2].max(b[2]));
    let outside = at[0] < x0 - 2.5 || at[0] > x1 + 2.5 || at[1] < z0 - 2.5 || at[1] > z1 + 2.5;
    assert!(outside, "{at:?} is on the Bay ({x0}..{x1}, {z0}..{z1})");
    // Where the visitor posts is open floor in front of the slot, toward
    // the spawn.
    assert!(apart(stand, SPAWN) < spawn);
}

#[test]
fn the_prompt_shows_in_reach_with_hysteresis() {
    let at = stand();
    assert!(in_reach(at, false));
    let edge = world([0.0, 0.0, BOX_HALF[1] + STAND + 2.8]);
    assert!(!in_reach(edge, false));
    assert!(in_reach(edge, true));
    assert!(!in_reach(
        world([0.0, 0.0, BOX_HALF[1] + STAND + 3.4]),
        true
    ));
    assert!(!in_reach([at[0], 5.0, at[2]], true));
    assert!(!in_reach([f64::NAN, 0.0, 0.0], true));
    assert!(!in_reach([SPAWN[0], 0.0, SPAWN[1]], true));
    // The prompt and the label float over the roof.
    assert!(prompt_at()[1] > BOX_TOP + ROOF_RISE);
    assert!(label_at()[1] > prompt_at()[1]);
}

#[test]
fn the_words_are_plain() {
    for text in [LABEL, PROMPT_KEYS, PROMPT_TOUCH] {
        assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
    }
}
