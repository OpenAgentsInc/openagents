//! Prints Everglade's world tree (`docs/verse/generative-agents.md`, item
//! 3): every node with its kind, standing point, affordances, and state at
//! a town hour, or, with `--known`, what a sample agent perceives on a
//! short walk.
//!
//! ```text
//! cargo run -p verse-zone-everglade --example world_tree -- [--hour HH] [--known]
//! ```
//!
//! The walk reads the committed pack for the town's blockers, so sight
//! stops at walls.

use std::path::Path;

use town_clock::TownTime;
use verse_world::social::sight::{Footprints, Open, Sight};
use verse_zone_everglade::zones::everglade::Everglade;
use verse_zone_everglade::zones::everglade::world_tree as everglade_tree;
use verse_zone_everglade::zones::everglade_pack::{self, ZonePack};
use world_tree::{Known, text};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let hour = args
        .iter()
        .position(|a| a == "--hour")
        .and_then(|i| args.get(i + 1))
        .and_then(|h| h.parse::<f64>().ok())
        .unwrap_or(21.0);
    let tree = everglade_tree::everglade();
    let states = everglade_tree::conditions::states(&tree, TownTime::at_hour(0, hour), None);
    if !args.iter().any(|a| a == "--known") {
        println!("town hour {hour:05.2}");
        print!("{}", text::dump(&tree, Some(&states)));
        return;
    }
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let world = ZonePack::load_local(&pack).and_then(|p| Everglade::world(&p));
    let blockers = world.map(|w| w.blockers).unwrap_or_default();
    let walls = Footprints {
        blocks: &blockers,
        tops: &[],
        default_top: 12.0,
        floor: 0.0,
    };
    let sight: &dyn Sight = if blockers.is_empty() { &Open } else { &walls };
    // A townsperson's first morning: up the approach path, into the
    // workshop hall, and along Main Street to the bakery's door.
    let walk = [
        ("the approach path", [0.0, -20.0]),
        ("the workshop hall", [0.0, 5.0]),
        ("Main Street by the bakery", [-25.0, 47.0]),
    ];
    let mut known = Known::new("bram", &tree);
    for (place, at) in walk {
        let learned = everglade_tree::perceive::perceive(&mut known, &tree, sight, at);
        println!("at {place} ({}, {}): learned {learned}", at[0], at[1]);
    }
    print!("{}", text::dump_known(&tree, &known, Some(&states)));
}
