//! A hosted zone as data (`docs/verse/ice-cream-man-live.md`): The Ice Cream
//! Man's first, and The Haunted House's (`docs/verse/games/the-haunted-house.md`).
//!
//! The zone is a livehost tenant. Its manifest maps paths to content hashes,
//! and `zone.json`, one of those paths, says what the paths are: the shell
//! (the page, its glue, and the game's drawing module), the logic module,
//! the tuning, and the mesh chunks with where each one sits in the house.
//! This crate reads and plans; it never fetches. The shell (`icecream-web`),
//! the Grid (`everglade-web`), and the deploy tool (`icecream-live`) share
//! it, so they agree on what loads first and what a deploy changed.

pub mod audio;
pub mod chunk;
pub mod image;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// `zone.json`'s schema.
pub const SCHEMA: &str = "openagents.icecream.zone.v1";
/// The tenant's name on livehost.
pub const TENANT: &str = "icecream";
/// The path of the zone index in the manifest.
pub const INDEX: &str = "zone.json";

/// What `zone.json` holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub schema: String,
    pub tenant: String,
    /// The game state version the logic module must read
    /// (`icecream_rules::live::STATE_VERSION`).
    pub state_version: u32,
    /// The house every player in the zone shares; the chunks are baked
    /// from it.
    pub seed: u64,
    /// Where a run starts, in metres (x, z).
    pub spawn: [f32; 2],
    /// The page, then the files it loads before anything else.
    pub shell: Vec<String>,
    /// The rules as a WebAssembly module; empty for a zone whose rules are
    /// in its shell.
    #[serde(default)]
    pub logic: String,
    /// The numbers a run plays by; empty when the zone has none.
    #[serde(default)]
    pub tuning: String,
    /// Small data files needed before the first frame, such as a house
    /// document's global settings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<String>,
    pub chunks: Vec<Chunk>,
    pub budget: Budget,
}

/// One mesh chunk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    pub path: String,
    pub kind: Kind,
    /// A room's centre in metres (x, z); models have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[f32; 2]>,
    /// For a room, its index in the house.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<usize>,
    /// For an area, the asset chunks it draws with: they load with it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
    /// Uncompressed bytes.
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A room's walls, furniture and ceiling lights.
    Room,
    /// A character model, needed before the first frame.
    Model,
    /// A part of a house document (an area), placed like a room.
    Area,
    /// An asset an area draws with (a mesh, an image, or a sound); it
    /// loads with the first area that needs it.
    Asset,
}

impl Kind {
    /// Whether chunks of this kind have a place and stream by distance.
    #[must_use]
    pub fn placed(self) -> bool {
        matches!(self, Self::Room | Self::Area)
    }
}

/// The first-play budget.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    /// Rooms whose centre is within this many metres of the spawn load
    /// before play; the rest stream in by distance.
    pub near_m: f32,
    /// The most uncompressed bytes the first play may need.
    pub first_play_bytes: u64,
    /// The time to first play the zone promises on a local connection, ms.
    pub first_play_ms: u32,
}

impl Zone {
    /// Reads `zone.json`.
    ///
    /// # Errors
    /// It isn't JSON, or not this schema.
    pub fn parse(text: &str) -> Result<Self, String> {
        let zone: Zone = serde_json::from_str(text).map_err(|e| format!("zone.json: {e}"))?;
        if zone.schema != SCHEMA {
            return Err(format!(
                "zone.json has schema {}, not {SCHEMA}",
                zone.schema
            ));
        }
        Ok(zone)
    }

    /// The paths a player needs before the first frame, in the order to
    /// fetch them: the shell, the logic and the tuning, the models, and the
    /// rooms near the spawn, nearest first.
    #[must_use]
    pub fn first_play(&self) -> Vec<String> {
        let mut paths: Vec<String> = self.shell.clone();
        for path in [&self.logic, &self.tuning] {
            if !path.is_empty() {
                paths.push(path.clone());
            }
        }
        paths.extend(self.data.iter().cloned());
        paths.extend(
            self.chunks
                .iter()
                .filter(|c| c.kind == Kind::Model)
                .map(|c| c.path.clone()),
        );
        let spawn = self.spawn;
        let near: Vec<&Chunk> = self
            .rooms_by_distance(spawn)
            .into_iter()
            .take_while(|(d, _)| *d <= self.budget.near_m)
            .map(|(_, c)| c)
            .collect();
        paths.extend(near.iter().map(|c| c.path.clone()));
        for chunk in near {
            for need in &chunk.needs {
                if !paths.contains(need) {
                    paths.push(need.clone());
                }
            }
        }
        paths
    }

    /// The asset chunks a placed chunk needs that aren't `loaded` yet.
    #[must_use]
    pub fn needs_of(&self, path: &str, loaded: &BTreeSet<String>) -> Vec<String> {
        self.chunks
            .iter()
            .find(|c| c.path == path)
            .map(|c| {
                c.needs
                    .iter()
                    .filter(|n| !loaded.contains(*n))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The rooms nearest `at` first, with their distances.
    #[must_use]
    pub fn rooms_by_distance(&self, at: [f32; 2]) -> Vec<(f32, &Chunk)> {
        let mut rooms: Vec<(f32, &Chunk)> = self
            .chunks
            .iter()
            .filter(|c| c.kind.placed())
            .filter_map(|c| c.at.map(|p| ((p[0] - at[0]).hypot(p[1] - at[1]), c)))
            .collect();
        rooms.sort_by(|a, b| a.0.total_cmp(&b.0));
        rooms
    }

    /// The next rooms to stream for a player at `at`, nearest first,
    /// skipping the ones already `loaded`.
    #[must_use]
    pub fn stream_order(&self, at: [f32; 2], loaded: &BTreeSet<String>) -> Vec<String> {
        self.rooms_by_distance(at)
            .into_iter()
            .filter(|(_, c)| !loaded.contains(&c.path))
            .map(|(_, c)| c.path.clone())
            .collect()
    }
}

/// A manifest's objects: path to SHA-256. Every livehost manifest has them.
pub type Objects = BTreeMap<String, String>;

/// Reads the `objects` of a livehost manifest.
///
/// # Errors
/// It isn't a manifest.
pub fn objects(manifest: &str) -> Result<Objects, String> {
    #[derive(Deserialize)]
    struct Manifest {
        objects: Objects,
    }
    serde_json::from_str::<Manifest>(manifest)
        .map(|m| m.objects)
        .map_err(|e| format!("not a manifest: {e}"))
}

/// The URL livehost serves an object at: its hash, and the path's
/// extension for its type.
#[must_use]
pub fn object_url(path: &str, hash: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() => format!("/_o/{hash}.{ext}"),
        _ => format!("/_o/{hash}"),
    }
}

/// What a deploy changed for a player already in the zone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Update {
    /// The logic module changed: swap it at the next step.
    pub logic: bool,
    /// The tuning changed: retune at the next step.
    pub tuning: bool,
    /// The zone index changed.
    pub index: bool,
    /// Chunks whose hash changed or that are new.
    pub chunks: Vec<String>,
    /// The shell changed: the page must load again (the state survives
    /// the reload).
    pub shell: bool,
    /// Every path whose object is new to the player, so the bytes to fetch.
    pub fetch: BTreeSet<String>,
}

/// What moving from `old` to `new` means for a player with `zone`'s
/// layout, by comparing each path's hash. An unchanged hash is never
/// fetched again.
#[must_use]
pub fn update(zone: &Zone, old: &Objects, new: &Objects) -> Update {
    let changed =
        |path: &str| !path.is_empty() && new.get(path).is_some_and(|h| old.get(path) != Some(h));
    let mut update = Update {
        logic: changed(&zone.logic),
        tuning: changed(&zone.tuning),
        index: changed(INDEX),
        shell: zone.shell.iter().any(|p| changed(p)),
        ..Update::default()
    };
    let seen: BTreeSet<&String> = old.values().collect();
    for (path, hash) in new {
        if old.get(path) != Some(hash) && !seen.contains(hash) {
            update.fetch.insert(path.clone());
        }
    }
    update.chunks = new
        .keys()
        .filter(|p| p.starts_with("chunks/") && changed(p))
        .cloned()
        .collect();
    update
}

pub mod budget {
    //! The first-play budget check a deploy runs before it publishes.

    use super::Zone;

    /// What the check measured.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Report {
        /// Uncompressed bytes before the first frame.
        pub first_play_bytes: u64,
        pub budget_bytes: u64,
        /// Files before the first frame.
        pub first_play_files: usize,
        /// Every byte of the zone.
        pub total_bytes: u64,
    }

    impl Report {
        #[must_use]
        pub fn within(&self) -> bool {
            self.first_play_bytes <= self.budget_bytes
        }

        /// One line for the deploy's output.
        #[must_use]
        pub fn line(&self) -> String {
            format!(
                "first play {} KB in {} files of a {} KB budget ({} KB in all)",
                self.first_play_bytes / 1024,
                self.first_play_files,
                self.budget_bytes / 1024,
                self.total_bytes / 1024
            )
        }
    }

    /// Checks a zone's first play against its budget. `index` is the
    /// text of `zone.json`, `size` gives a path's uncompressed bytes (a
    /// file's length, or an object's), and `all` lists every path, for the
    /// total.
    ///
    /// # Errors
    /// The index doesn't read, a first-play file is missing, or the first
    /// play is over budget; the message says which.
    pub fn check<'a>(
        index: &str,
        size: impl Fn(&str) -> Option<u64>,
        all: impl IntoIterator<Item = &'a String>,
    ) -> Result<Report, String> {
        let zone = Zone::parse(index)?;
        let first = zone.first_play();
        let mut first_play_bytes = 0;
        for path in &first {
            first_play_bytes += size(path).ok_or_else(|| format!("{path} is missing"))?;
        }
        let report = Report {
            first_play_bytes,
            budget_bytes: zone.budget.first_play_bytes,
            first_play_files: first.len(),
            total_bytes: all.into_iter().filter_map(|p| size(p)).sum(),
        };
        if report.within() {
            Ok(report)
        } else {
            Err(format!("over the first-play budget: {}", report.line()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone() -> Zone {
        let room = |i: usize, x: f32| Chunk {
            path: format!("chunks/room-{i}.bin"),
            kind: Kind::Room,
            at: Some([x, 5.0]),
            room: Some(i),
            needs: Vec::new(),
            bytes: 100,
        };
        Zone {
            schema: SCHEMA.into(),
            tenant: TENANT.into(),
            state_version: 1,
            seed: 7,
            spawn: [5.0, 5.0],
            shell: vec!["index.html".into(), "icecream_web_bg.wasm".into()],
            logic: "logic.wasm".into(),
            tuning: "tuning.json".into(),
            data: Vec::new(),
            chunks: vec![
                room(2, 25.0),
                room(0, 5.0),
                room(1, 15.0),
                Chunk {
                    path: "chunks/man.bin".into(),
                    kind: Kind::Model,
                    at: None,
                    room: None,
                    needs: Vec::new(),
                    bytes: 50,
                },
            ],
            budget: Budget {
                near_m: 12.0,
                first_play_bytes: 1000,
                first_play_ms: 1500,
            },
        }
    }

    #[test]
    fn first_play_is_the_shell_logic_models_and_near_rooms() {
        assert_eq!(
            zone().first_play(),
            [
                "index.html",
                "icecream_web_bg.wasm",
                "logic.wasm",
                "tuning.json",
                "chunks/man.bin",
                "chunks/room-0.bin",
                "chunks/room-1.bin",
            ]
        );
    }

    #[test]
    fn rooms_stream_nearest_first_and_skip_loaded_ones() {
        let loaded = BTreeSet::from(["chunks/room-1.bin".to_owned()]);
        assert_eq!(
            zone().stream_order([26.0, 5.0], &loaded),
            ["chunks/room-2.bin", "chunks/room-0.bin"]
        );
    }

    #[test]
    fn an_update_fetches_only_new_hashes() {
        let z = zone();
        let old = Objects::from([
            ("logic.wasm".into(), "a".into()),
            ("tuning.json".into(), "b".into()),
            ("chunks/room-0.bin".into(), "c".into()),
            ("chunks/room-1.bin".into(), "d".into()),
            ("index.html".into(), "e".into()),
        ]);
        let mut new = old.clone();
        new.insert("tuning.json".into(), "f".into());
        new.insert("chunks/room-1.bin".into(), "g".into());
        // A room that now looks like another one costs nothing to fetch.
        new.insert("chunks/room-0.bin".into(), "d".into());
        let update = update(&z, &old, &new);
        assert!(update.tuning && !update.logic && !update.shell);
        assert_eq!(update.chunks, ["chunks/room-0.bin", "chunks/room-1.bin"]);
        assert_eq!(
            update.fetch,
            BTreeSet::from(["tuning.json".to_owned(), "chunks/room-1.bin".to_owned()])
        );
    }

    #[test]
    fn the_budget_counts_the_first_play() {
        let index = serde_json::to_string(&zone()).unwrap();
        let all: Vec<String> = [
            "index.html",
            "icecream_web_bg.wasm",
            "logic.wasm",
            "tuning.json",
        ]
        .into_iter()
        .map(String::from)
        .chain(zone().chunks.iter().map(|c| c.path.clone()))
        .collect();
        let report = budget::check(&index, |_| Some(100), &all).unwrap();
        assert_eq!(report.first_play_bytes, 700);
        assert_eq!(report.total_bytes, 800);
        assert!(budget::check(&index, |_| Some(200), &all).is_err());
        assert!(budget::check(&index, |p| (p != "logic.wasm").then_some(1), &all).is_err());
    }

    #[test]
    fn a_house_zone_loads_its_near_areas_with_their_assets() {
        let area = |id: &str, z: f32, needs: &[&str]| Chunk {
            path: format!("chunks/areas/{id}.json"),
            kind: Kind::Area,
            at: Some([8.0, z]),
            room: None,
            needs: needs.iter().map(|s| (*s).to_owned()).collect(),
            bytes: 10,
        };
        let asset = |name: &str| Chunk {
            path: name.into(),
            kind: Kind::Asset,
            at: None,
            room: None,
            needs: Vec::new(),
            bytes: 10,
        };
        let zone = Zone {
            schema: SCHEMA.into(),
            tenant: "haunt".into(),
            state_version: 0,
            seed: 0,
            spawn: [8.0, 4.0],
            shell: vec!["templates/index.html".into()],
            logic: String::new(),
            tuning: String::new(),
            data: vec!["house.json".into()],
            chunks: vec![
                area(
                    "yard",
                    10.0,
                    &["chunks/meshes/tree.bin", "chunks/meshes/pumpkin.bin"],
                ),
                area(
                    "hall",
                    26.0,
                    &["chunks/meshes/pumpkin.bin", "chunks/meshes/portrait.bin"],
                ),
                area("kitchen", 62.0, &["chunks/cards/cat.ici"]),
                asset("chunks/meshes/tree.bin"),
                asset("chunks/meshes/pumpkin.bin"),
                asset("chunks/meshes/portrait.bin"),
                asset("chunks/cards/cat.ici"),
            ],
            budget: Budget {
                near_m: 24.0,
                first_play_bytes: 1000,
                first_play_ms: 1500,
            },
        };
        assert_eq!(
            zone.first_play(),
            [
                "templates/index.html",
                "house.json",
                "chunks/areas/yard.json",
                "chunks/areas/hall.json",
                "chunks/meshes/tree.bin",
                "chunks/meshes/pumpkin.bin",
                "chunks/meshes/portrait.bin",
            ]
        );
        // Assets never stream on their own; areas bring them.
        assert_eq!(
            zone.stream_order([8.0, 4.0], &BTreeSet::new()),
            [
                "chunks/areas/yard.json",
                "chunks/areas/hall.json",
                "chunks/areas/kitchen.json"
            ]
        );
        assert_eq!(
            zone.needs_of("chunks/areas/kitchen.json", &BTreeSet::new()),
            ["chunks/cards/cat.ici"]
        );
        // A zone without logic or tuning never reports them changed.
        let old = Objects::from([("house.json".into(), "a".into())]);
        let new = Objects::from([("house.json".into(), "b".into())]);
        let up = update(&zone, &old, &new);
        assert!(!up.logic && !up.tuning);
        assert_eq!(up.fetch, BTreeSet::from(["house.json".to_owned()]));
    }

    #[test]
    fn object_urls_keep_the_extension() {
        assert_eq!(object_url("chunks/room-1.bin", "ab"), "/_o/ab.bin");
        assert_eq!(object_url("LICENSE", "ab"), "/_o/ab");
    }
}
