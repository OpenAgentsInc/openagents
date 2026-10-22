//! Writes the zone's tenant directory: the game's own meshes baked into
//! chunks, the tuning, the logic module, the shell, and `zone.json`.
//!
//! The Ice Cream Man's art is made in code (`icecream_web::scene`): every
//! room of the house, its ceiling lights, the Ice Cream Man and the kid.
//! The bake runs that code for the zone's house and packs each room and
//! each character into its own chunk, so a deploy that changes one room or
//! one character changes one hash. The game has no texture or sound files
//! (its sounds are made in the page), so there are no texture or audio
//! chunks.
//!
//! The sources a deploy reads live in `crates/icecream-live/zone/`:
//!
//! - `level.json`: the house's seed, the near radius, and the first-play
//!   budget.
//! - `tuning.json`: the gameplay numbers that differ from the game's
//!   defaults, by name (Game Lab's names).
//! - `palette.json`: colours to draw differently, by chunk group (`man`,
//!   `kid`, `rooms`), as `"RRGGBB": "RRGGBB"`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use icecream_rules::layout::{House, centre};
use icecream_rules::{Tuning, live::STATE_VERSION};
use icecream_web::mesh::Mesh;
use icecream_web::scene;
use icecream_zone::chunk::{self, Part};
use icecream_zone::{Budget, Chunk, Kind, SCHEMA, TENANT, Zone};
use serde::Deserialize;

/// The shell's files, in the order the page loads them: the page (a
/// livehost template, served at `/t/icecream/`), the module's glue, and the
/// module. The page names the other two by content hash
/// (`{{asset PATH}}`), so a browser caches them for good and a prefetch
/// counts.
pub const SHELL: [&str; 3] = [
    "templates/index.html",
    "icecream_web.js",
    "icecream_web_bg.wasm",
];
/// livehost's description of the tenant: its one route.
const SITE: &str = r#"{
  "tenant": "icecream",
  "routes": [{ "path": "/", "template": "index.html" }]
}
"#;
/// The logic module's path in the tenant.
pub const LOGIC: &str = "logic.wasm";

#[derive(Deserialize)]
struct Level {
    seed: u64,
    near_m: f32,
    first_play_bytes: u64,
    first_play_ms: u32,
}

/// Colour swaps by chunk group.
type Palette = BTreeMap<String, BTreeMap<String, String>>;

/// The zone's sources.
pub struct Sources {
    pub dir: PathBuf,
}

impl Sources {
    /// The checked-in sources beside this crate.
    #[must_use]
    pub fn checked_in() -> Self {
        Self {
            dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("zone"),
        }
    }

    fn read<T: for<'de> Deserialize<'de>>(&self, name: &str) -> Result<T> {
        let path = self.dir.join(name);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))
    }

    /// The zone's tuning: the defaults with `tuning.json` on top.
    ///
    /// # Errors
    /// The file doesn't read, or a value is out of bounds.
    pub fn tuning(&self) -> Result<Tuning> {
        let values: BTreeMap<String, f64> = self.read("tuning.json")?;
        let mut tuning = Tuning::default();
        for (name, value) in values {
            tuning.set(&name, value).map_err(anyhow::Error::msg)?;
        }
        Ok(tuning)
    }
}

fn recolour(mesh: &Mesh, swaps: Option<&BTreeMap<String, String>>) -> Result<Vec<f32>> {
    let mut data = mesh.data.clone();
    let Some(swaps) = swaps.filter(|s| !s.is_empty()) else {
        return Ok(data);
    };
    let mut table = Vec::new();
    for (from, to) in swaps {
        let parse = |hex: &str| {
            u32::from_str_radix(hex.trim_start_matches('#'), 16)
                .with_context(|| format!("{hex} is not a colour"))
        };
        table.push((
            icecream_web::mesh::rgb(parse(from)?),
            icecream_web::mesh::rgb(parse(to)?),
        ));
    }
    let byte = |x: f32| (x * 255.0).round() as i32;
    for vertex in data.chunks_exact_mut(chunk::STRIDE) {
        let colour = &mut vertex[9..12];
        for (from, to) in &table {
            if (0..3).all(|k| byte(colour[k]) == byte(from[k])) {
                colour.copy_from_slice(to);
                break;
            }
        }
    }
    Ok(data)
}

fn part(name: &str, mesh: &Mesh, swaps: Option<&BTreeMap<String, String>>) -> Result<Part> {
    Ok(Part {
        name: name.into(),
        data: recolour(mesh, swaps)?,
    })
}

/// What the bake wrote.
pub struct Baked {
    pub zone: Zone,
}

/// Writes the tenant directory `out` afresh from `sources`, the shell build
/// in `shell` (the output of `scripts/build-icecream-web.sh`), and the logic
/// module at `logic`. livehost uploads only the files whose hash changed.
///
/// # Errors
/// A source doesn't read, or a file can't be written.
pub fn bake(sources: &Sources, shell: &Path, logic: &Path, out: &Path) -> Result<Baked> {
    let level: Level = sources.read("level.json")?;
    let palette: Palette = sources.read("palette.json")?;
    let tuning = sources.tuning()?;
    let house = House::of(level.seed, &tuning);
    // Start clean, so nothing a source no longer makes is deployed.
    if out.exists() {
        std::fs::remove_dir_all(out)?;
    }
    std::fs::create_dir_all(out.join("chunks"))?;
    std::fs::create_dir_all(out.join("templates"))?;
    let write = |path: &str, bytes: &[u8]| -> Result<u64> {
        let target = out.join(path);
        std::fs::write(&target, bytes).with_context(|| format!("writing {}", target.display()))?;
        Ok(bytes.len() as u64)
    };

    for name in &SHELL[1..] {
        let bytes = std::fs::read(shell.join(name)).with_context(|| {
            format!("the shell build has no {name}; run scripts/build-icecream-web.sh")
        })?;
        write(name, &bytes)?;
    }
    write(SHELL[0], PAGE.as_bytes())?;
    write("site.json", SITE.as_bytes())?;
    let logic_bytes =
        std::fs::read(logic).with_context(|| format!("reading {}", logic.display()))?;
    write(LOGIC, &logic_bytes)?;
    write("tuning.json", &serde_json::to_vec_pretty(&tuning)?)?;

    let mut chunks = Vec::new();
    let man = scene::man();
    let swaps = palette.get("man");
    let bytes = chunk::encode(&[
        part("body", &man.body, swaps)?,
        part("leg", &man.leg, swaps)?,
        part("arm", &man.arm, swaps)?,
        part("head", &man.head, swaps)?,
        part("forehead", &man.forehead, swaps)?,
    ]);
    chunks.push(model("chunks/man.bin", write("chunks/man.bin", &bytes)?));
    let kid = scene::avatar();
    let swaps = palette.get("kid");
    let bytes = chunk::encode(&[
        part("torso", &kid.torso, swaps)?,
        part("head", &kid.head, swaps)?,
        part("arm", &kid.arm, swaps)?,
        part("leg", &kid.leg, swaps)?,
    ]);
    chunks.push(model("chunks/kid.bin", write("chunks/kid.bin", &bytes)?));

    let swaps = palette.get("rooms");
    for (index, room) in house.rooms.iter().enumerate() {
        let path = format!("chunks/room-{index}.bin");
        let bytes = chunk::encode(&[
            part("room", &scene::room(&house, index, tuning.yellow), swaps)?,
            part(
                "lights",
                &scene::lights(&house, index, tuning.yellow),
                swaps,
            )?,
        ]);
        let size = write(&path, &bytes)?;
        let at = centre(room.cell);
        chunks.push(Chunk {
            path,
            kind: Kind::Room,
            at: Some([at.x, at.z]),
            room: Some(index),
            needs: Vec::new(),
            bytes: size,
        });
    }
    let zone = Zone {
        schema: SCHEMA.into(),
        tenant: TENANT.into(),
        state_version: STATE_VERSION,
        seed: level.seed,
        spawn: [house.spawn.x, house.spawn.z],
        shell: SHELL.iter().map(|s| (*s).to_owned()).collect(),
        logic: LOGIC.into(),
        tuning: "tuning.json".into(),
        data: Vec::new(),
        chunks,
        budget: Budget {
            near_m: level.near_m,
            first_play_bytes: level.first_play_bytes,
            first_play_ms: level.first_play_ms,
        },
    };
    write(icecream_zone::INDEX, &serde_json::to_vec_pretty(&zone)?)?;
    if zone.state_version != STATE_VERSION {
        bail!("the zone's state version is not the rules'");
    }
    Ok(Baked { zone })
}

fn model(path: &str, bytes: u64) -> Chunk {
    Chunk {
        path: path.into(),
        kind: Kind::Model,
        at: None,
        room: None,
        needs: Vec::new(),
        bytes,
    }
}

/// The zone's page. The canvas's `data-live-tenant` puts the game in live
/// mode: it loads the logic, the tuning, and the chunks from the tenant's
/// manifest and follows the channel the host serves. `data-livehost-self`
/// tells livehost's injected client that the page applies updates itself.
pub const PAGE: &str = r#"<!doctype html>
<html lang="en" data-livehost-self>
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover">
    <title>The Ice Cream Man</title>
    <style>
      html, body { margin: 0; width: 100%; height: 100%; overflow: hidden; background: #2b1a12; touch-action: none; }
      #icecream-status { position: fixed; left: 12px; bottom: 12px; color: #ffe4d0; font: 600 15px ui-rounded, system-ui, sans-serif; }
    </style>
  </head>
  <body>
    <canvas id="icecream-canvas" tabindex="0" data-live-tenant="icecream" aria-label="The Airbnb, where the Ice Cream Man is"></canvas>
    <div id="icecream-status">Loading the Ice Cream Man…</div>
    <script type="module">
      import init from "{{asset icecream_web.js}}";
      init({ module_or_path: "{{asset icecream_web_bg.wasm}}" }).catch((error) => {
        document.getElementById("icecream-status").textContent =
          "The game couldn't start. Reload the page to try again.";
        console.error(error);
      });
    </script>
  </body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn sources(dir: &Path, tuning: &str, palette: &str) -> Sources {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("level.json"),
            r#"{"seed":7,"near_m":16,"first_play_bytes":9000000,"first_play_ms":1500}"#,
        )
        .unwrap();
        std::fs::write(dir.join("tuning.json"), tuning).unwrap();
        std::fs::write(dir.join("palette.json"), palette).unwrap();
        Sources { dir: dir.into() }
    }

    fn shell(dir: &Path) -> (PathBuf, PathBuf) {
        let shell = dir.join("shell");
        std::fs::create_dir_all(&shell).unwrap();
        for name in &SHELL[1..] {
            std::fs::write(shell.join(name), name.as_bytes()).unwrap();
        }
        let logic = dir.join("logic.wasm");
        std::fs::write(&logic, b"\0asm").unwrap();
        (shell, logic)
    }

    fn hashes(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        for entry in std::fs::read_dir(dir.join("chunks")).unwrap() {
            let entry = entry.unwrap();
            out.insert(
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read(entry.path()).unwrap(),
            );
        }
        out
    }

    #[test]
    fn a_palette_change_changes_only_its_model() {
        let tmp = tempfile::tempdir().unwrap();
        let (shell, logic) = shell(tmp.path());
        let out = tmp.path().join("out");
        let a = sources(&tmp.path().join("a"), "{}", "{}");
        let baked = bake(&a, &shell, &logic, &out).unwrap();
        assert_eq!(baked.zone.chunks.len(), 2 + 49);
        let before = hashes(&out);
        let b = sources(
            &tmp.path().join("b"),
            "{}",
            r#"{"man":{"FFE4D0":"8FD3B4"}}"#,
        );
        bake(&b, &shell, &logic, &out).unwrap();
        let after = hashes(&out);
        let changed: Vec<_> = before.keys().filter(|k| before[*k] != after[*k]).collect();
        assert_eq!(changed, ["man.bin"]);
    }

    #[test]
    fn two_more_closets_change_only_their_rooms() {
        let tmp = tempfile::tempdir().unwrap();
        let (shell, logic) = shell(tmp.path());
        let out = tmp.path().join("out");
        bake(
            &sources(&tmp.path().join("a"), "{}", "{}"),
            &shell,
            &logic,
            &out,
        )
        .unwrap();
        let before = hashes(&out);
        // The default is two; two more.
        let tuning = r#"{"extra_closets": 4}"#;
        let baked = bake(
            &sources(&tmp.path().join("b"), tuning, "{}"),
            &shell,
            &logic,
            &out,
        )
        .unwrap();
        let after = hashes(&out);
        let changed = before.keys().filter(|k| before[*k] != after[*k]).count();
        assert_eq!(changed, 2, "one chunk for each room with a new closet");
        let report = icecream_zone::budget::check(
            &std::fs::read_to_string(out.join("zone.json")).unwrap(),
            |p| std::fs::metadata(out.join(p)).ok().map(|m| m.len()),
            &baked.zone.first_play(),
        )
        .unwrap();
        assert!(report.first_play_files > 6);
    }
}
