#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use verse_engine::assets::{Pack, Texture};
use verse_engine::inventory::{Asset, AssetId, Binding, Inventory, License, Origin};
use verse_engine::loading::Prepared;

use super::*;

const WHITE_FILE: &str = "verse-flat-white.png";
const WHITE: &[u8] = include_bytes!("../../../assets/verse/grid/verse-flat-white.png");

fn tint(tone: u8) -> [f32; 3] {
    [f32::from(tone) / 3.0; 3]
}

/// A pack with only the Grid's white texel, as the Grid's start has it.
fn bare_pack() -> Pack {
    let digest = sha(WHITE);
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
                    bytes: WHITE.len() as u64,
                    dependencies: vec![AssetId::new("verse:source:project").unwrap()],
                },
            ],
        }),
        version: 1,
        source_revision: digest.clone(),
        models: BTreeMap::new(),
        textures: vec![Texture {
            file: WHITE_FILE.into(),
            sha256: digest,
            width: 1,
            height: 1,
        }],
        placements: vec![],
    }
}

#[test]
fn the_desk_is_admitted_with_its_screens() {
    let mut pack = bare_pack();
    add(&mut pack, 0, "verse:texture:grid/0", &tint).unwrap();
    pack.validate().unwrap();
    assert_eq!(pack.models.len(), 3);
    assert_eq!(pack.placements.len(), 3);
    let mut files = vec![(WHITE_FILE, WHITE)];
    files.extend(files_list());
    let prepared = Prepared::from_bytes(pack, &files, Default::default()).unwrap();
    assert_eq!(prepared.textures().len(), 3);
    // A screen that isn't the admitted one is refused.
    let mut pack = bare_pack();
    add(&mut pack, 0, "verse:texture:grid/0", &tint).unwrap();
    let wrong = [
        (WHITE_FILE, WHITE),
        (MACBOOK_SCREEN_FILE, devices::IPHONE_SCREEN),
        (IPHONE_SCREEN_FILE, devices::IPHONE_SCREEN),
    ];
    assert!(Prepared::from_bytes(pack, &wrong, Default::default()).is_err());
}

fn files_list() -> Vec<(&'static str, &'static [u8])> {
    files().to_vec()
}

#[test]
fn the_devices_keep_their_budgets() {
    assert!(
        devices::PINNED.len() <= devices::MAX_BYTES,
        "{}",
        devices::PINNED.len()
    );
    for (_, bytes) in files() {
        assert!(bytes.len() <= devices::MAX_SCREEN_BYTES, "{}", bytes.len());
    }
    let devices = devices::pinned().unwrap();
    let names: Vec<_> = devices.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["macbook", "iphone"]);
    for device in &devices {
        assert!(device.triangles.len() <= devices::MAX_TRIANGLES);
        assert!(!device.edges.is_empty());
        assert!(device.screen_indices.len() >= 6);
        // The screen's coordinates span the image.
        let (lo, hi) = device
            .screen
            .iter()
            .fold((2.0f32, -1.0f32), |(lo, hi), (_, uv)| {
                (lo.min(uv[0]).min(uv[1]), hi.max(uv[0]).max(uv[1]))
            });
        assert!(
            lo > -0.01 && lo < 0.05 && hi > 0.95 && hi < 1.01,
            "{lo} {hi}"
        );
    }
    // The Grid digests every vertex at start: the desk stays small.
    let mut pack = bare_pack();
    add(&mut pack, 0, "verse:texture:grid/0", &tint).unwrap();
    let vertices: usize = pack
        .models
        .values()
        .flat_map(|m| &m.surfaces)
        .map(|s| s.vertices.len())
        .sum();
    assert!(vertices < 8_000, "{vertices}");
}

#[test]
fn a_truncated_or_padded_file_is_refused() {
    let bytes = devices::PINNED;
    assert!(devices::decode(&bytes[..bytes.len() - 1]).is_err());
    let mut padded = bytes.to_vec();
    padded.push(0);
    assert!(devices::decode(&padded).is_err());
    assert!(devices::decode(b"OAHD").is_err());
    let devices = devices::pinned().unwrap();
    assert_eq!(
        devices::decode(&devices::encode(&devices)).unwrap(),
        devices
    );
}

#[test]
fn the_devices_stand_on_the_counter_facing_the_visitor() {
    let placements = placements();
    let counter = &placements[0];
    let front = Quat::from_array(counter.rotation) * Vec3::Z;
    let toward = Vec3::new(
        (FACES[0] - POSITION[0]) as f32,
        0.0,
        (FACES[1] - POSITION[2]) as f32,
    )
    .normalize();
    assert!(front.dot(toward) > 0.999);
    for device in devices::pinned().unwrap() {
        let placement = placements
            .iter()
            .find(|p| p.model.ends_with(&device.name))
            .unwrap();
        let m = transform(placement);
        let lowest = device
            .corners
            .iter()
            .map(|p| m.transform_point3(Vec3::from_array(*p)).y)
            .fold(f32::MAX, f32::min);
        // On the counter (or its stand), never sunk into it.
        assert!(
            lowest >= art::TOP as f32 - 0.002 && lowest < art::TOP as f32 + 0.03,
            "{}: {lowest}",
            device.name
        );
        // The screen faces the visitor's side of the counter.
        let s = &device.screen;
        let [a, b, c] = [0, 1, 2].map(|i| {
            m.transform_point3(Vec3::from_array(s[usize::from(device.screen_indices[i])].0))
        });
        let normal = (b - a).cross(c - a).normalize();
        assert!(normal.dot(front) > 0.2, "{}: {normal}", device.name);
    }
}

fn flat(p: [f64; 3]) -> [f64; 2] {
    [p[0], p[2]]
}

fn apart(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// The plaza's places, from their own constants: the spawn
/// (`verse_core::world::SPAWN`), the terminal, the arches
/// (`verse::zones::gate`, side and forward of the spawn), the Game Lab
/// arch (`verse::zones::game_place`), and the plaza's middle.
const SPAWN: [f64; 2] = [0.0, -10.0];
const GAME_LAB: [f64; 2] = [-8.0, 3.0];
/// How far the Game Lab arch reaches, at most, m.
const GAME_LAB_REACH: f64 = 4.5;
const ARCHES: [[f64; 2]; 5] = [
    [-9.0, 1.0],
    [9.0, 1.0],
    [-20.0, 9.0],
    [0.0, 16.0],
    [20.0, 9.0],
];

#[test]
fn the_desk_stands_near_the_spawn_and_clear_of_the_plaza() {
    let at = flat(POSITION);
    let spawn = apart(at, SPAWN);
    assert!((6.0..10.0).contains(&spawn), "{spawn}");
    assert!(apart(at, [0.0, 0.0]) > 7.0);
    assert!(apart(at, flat(pylons_view::terminal::POSITION)) > 9.0);
    for arch in ARCHES {
        assert!(apart(at, arch) > 6.0, "{arch:?}");
    }
    // The Pylon Bay's pad.
    let [u0, u1, v0, v1] = pylons_view::bay::PAD;
    let (a, b) = (
        pylons_view::bay::world(u0, v0),
        pylons_view::bay::world(u1, v1),
    );
    let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
    assert!(at[0] + 3.0 < x0 || at[0] - 3.0 > x1);
    // The Game Lab arch: apart, and their reaches never meet.
    assert!(apart(at, GAME_LAB) > 7.0);
    let ask = flat(world([0.0, 0.0, STAND]));
    assert!(apart(ask, GAME_LAB) > GAME_LAB_REACH + LEAVE_RANGE);
    // The Gym's doorway is 36 m ahead of the spawn.
    assert!(at[1] < SPAWN[1] + 36.0 - 20.0);
    // Where the visitor asks is open floor, not behind the counter.
    let stand = flat(world([0.0, 0.0, STAND]));
    assert!(apart(stand, SPAWN) < spawn);
}

#[test]
fn the_prompt_shows_in_reach_with_hysteresis() {
    let stand = world([0.0, 0.0, STAND]);
    assert!(in_reach(stand, false));
    let edge = world([0.0, 0.0, STAND + 3.0]);
    assert!(!in_reach(edge, false));
    assert!(in_reach(edge, true));
    assert!(!in_reach(world([0.0, 0.0, STAND + 3.6]), true));
    assert!(!in_reach([stand[0], 5.0, stand[2]], true));
    assert!(!in_reach([f64::NAN, 0.0, 0.0], true));
    // Not from the spawn, nor from the terminal.
    assert!(!in_reach([SPAWN[0], 0.0, SPAWN[1]], true));
    assert!(!in_reach(pylons_view::terminal::POSITION, true));
}

#[test]
fn the_words_are_plain() {
    for text in [SIGN, PROMPT_KEYS, PROMPT_TOUCH] {
        assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
    }
}
