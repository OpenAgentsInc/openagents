//! The Haunted House as a livehost tenant, `haunt`
//! (`docs/verse/games/the-haunted-house.md`, "Hosting and code layout").
//!
//! The house is one `haunt.v1` document. A bake checks it, then writes the
//! tenant directory: the shell (the page and `haunt-web`'s module), the
//! house's global settings (`house.json`), one chunk per area (that area's
//! part of the document, placed at its middle), and the asset chunks the
//! areas draw with: a mesh chunk per asset (a part per shape) and a card
//! image per creature. Each area names the asset chunks it needs, so the
//! first play is the settings, the areas near the arrival, and their
//! assets. A deploy that changes one area changes one chunk.
//!
//! The house a deploy publishes lives at `WORK/haunt/house.json`, made from
//! version 1 the first time; Game Lab's on-save hook writes its versions
//! there.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use haunt_rules::House;
use icecream_zone::chunk::{self, Part};
use icecream_zone::{Budget, Chunk, Kind, SCHEMA, Zone};

/// The tenant's name on livehost.
pub const TENANT: &str = "haunt";
/// The shell's files: the page (a livehost template), the glue, and the
/// module.
pub const SHELL: [&str; 3] = ["templates/index.html", "haunt_web.js", "haunt_web_bg.wasm"];
/// The house's global settings in the tenant.
pub const HOUSE: &str = "house.json";
/// Areas whose middle is within this many metres of the arrival load
/// before the first frame: the yard and the entry hall.
pub const NEAR_M: f32 = 24.0;
/// The first-play budget: 3.5 MB uncompressed and 1.5 s, as for The Ice
/// Cream Man.
pub const FIRST_PLAY_BYTES: u64 = 3_500_000;
pub const FIRST_PLAY_MS: u32 = 1500;

const SITE: &str = r#"{
  "tenant": "haunt",
  "routes": [{ "path": "/", "template": "index.html" }]
}
"#;

/// The page. `data-live-tenant` puts the shell in live mode; and
/// `data-livehost-self` tells livehost's injected client that the page
/// applies updates itself.
pub const PAGE: &str = r#"<!doctype html>
<html lang="en" data-livehost-self>
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover">
    <title>The Haunted House</title>
    <style>
      html, body { margin: 0; width: 100%; height: 100%; overflow: hidden; background: #2B3346; touch-action: none; }
      #haunt-canvas { display: block; width: 100vw; height: 100vh; outline: none; touch-action: none; }
    </style>
  </head>
  <body>
    <canvas id="haunt-canvas" tabindex="0" data-live-tenant="haunt" aria-label="The Haunted House"></canvas>
    <script type="module">
      import init from "{{asset haunt_web.js}}";
      init({ module_or_path: "{{asset haunt_web_bg.wasm}}" }).catch((error) => {
        console.error("The house couldn't start. Reload the page to try again.", error);
      });
    </script>
  </body>
</html>
"#;

/// The house a deploy publishes: `WORK/haunt/house.json`, made from
/// version 1 when it isn't there yet.
///
/// # Errors
/// The file can't be read or written.
pub fn house_path(work: &Path) -> Result<PathBuf> {
    let path = work.join("haunt/house.json");
    if !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap_or(work))?;
        std::fs::write(&path, haunt_rules::doc::V1_JSON)?;
    }
    Ok(path)
}

/// Reads and checks a house.
///
/// # Errors
/// It doesn't read, or a check refuses it; the message is the refusal.
pub fn read_house(path: &Path) -> Result<House> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let house = House::parse(&text).map_err(anyhow::Error::msg)?;
    haunt_rules::check(&house, haunt_assets::catalog()).map_err(|r| anyhow::anyhow!(r.message))?;
    Ok(house)
}

fn mesh_chunk(asset: &str) -> Option<Vec<u8>> {
    let a = haunt_assets::catalog().asset(asset)?;
    let mut parts = Vec::new();
    let first = haunt_assets::meshes::build(asset, &a.states[0])?;
    parts.push(Part {
        name: format!("{asset}#"),
        data: first.data,
    });
    for state in haunt_assets::meshes::shaped_states(asset) {
        if let Some(mesh) = haunt_assets::meshes::build(asset, state) {
            parts.push(Part {
                name: format!("{asset}#{state}"),
                data: mesh.data,
            });
        }
    }
    Some(chunk::encode(&parts))
}

/// Writes the tenant directory `out` afresh for `house`, with the shell
/// built in `shell` (`scripts/build-haunt-web.sh`'s output).
///
/// # Errors
/// The house doesn't pass its checks, the shell isn't built, or a file
/// can't be written.
pub fn bake(house: &House, shell: &Path, out: &Path) -> Result<Zone> {
    let catalog = haunt_assets::catalog();
    haunt_rules::check(house, catalog).map_err(|r| anyhow::anyhow!(r.message))?;
    if out.exists() {
        std::fs::remove_dir_all(out)?;
    }
    for dir in ["templates", "chunks/areas", "chunks/meshes", "chunks/cards"] {
        std::fs::create_dir_all(out.join(dir))?;
    }
    let write = |path: &str, bytes: &[u8]| -> Result<u64> {
        let target = out.join(path);
        std::fs::write(&target, bytes).with_context(|| format!("writing {}", target.display()))?;
        Ok(bytes.len() as u64)
    };
    for name in &SHELL[1..] {
        let bytes = std::fs::read(shell.join(name)).with_context(|| {
            format!("the shell build has no {name}; run icecream-live shell --game haunt")
        })?;
        write(name, &bytes)?;
    }
    write(SHELL[0], PAGE.as_bytes())?;
    write("site.json", SITE.as_bytes())?;
    let mut head = house.clone();
    head.areas.clear();
    write(HOUSE, &serde_json::to_vec_pretty(&head)?)?;

    // Each area and the assets it draws with.
    let mut chunks = Vec::new();
    let mut assets: BTreeMap<String, u64> = BTreeMap::new();
    let mut asset_chunks = Vec::new();
    for area in &house.areas {
        let mut placed = haunt_web::scene::props(area, catalog);
        placed.extend(haunt_web::scene::yard_scenery(area));
        let mut needs = BTreeSet::new();
        for p in &placed {
            let path = match p.kind {
                haunt_assets::Kind::Mesh => format!("chunks/meshes/{}.bin", p.asset),
                haunt_assets::Kind::Card => format!("chunks/cards/{}.ici", p.asset),
                haunt_assets::Kind::Effect => continue,
            };
            if !assets.contains_key(&path) {
                let bytes = match p.kind {
                    haunt_assets::Kind::Mesh => mesh_chunk(&p.asset),
                    _ => haunt_assets::cards::draw(&p.asset)
                        .map(|image| icecream_zone::image::encode(&image)),
                };
                let Some(bytes) = bytes else { continue };
                let size = write(&path, &bytes)?;
                assets.insert(path.clone(), size);
                asset_chunks.push(Chunk {
                    path: path.clone(),
                    kind: Kind::Asset,
                    at: None,
                    room: None,
                    needs: Vec::new(),
                    bytes: size,
                });
            }
            needs.insert(path);
        }
        let path = format!("chunks/areas/{}.json", area.id);
        let size = write(&path, &serde_json::to_vec_pretty(area)?)?;
        chunks.push(Chunk {
            path,
            kind: Kind::Area,
            at: Some(area.centre()),
            room: None,
            needs: needs.into_iter().collect(),
            bytes: size,
        });
    }
    chunks.extend(asset_chunks);
    let zone = Zone {
        schema: SCHEMA.into(),
        tenant: TENANT.into(),
        state_version: 0,
        seed: 0,
        spawn: house
            .arrival_point()
            .context("the house has no arrival point")?,
        shell: SHELL.iter().map(|s| (*s).to_owned()).collect(),
        logic: String::new(),
        tuning: String::new(),
        data: vec![HOUSE.into()],
        chunks,
        budget: Budget {
            near_m: NEAR_M,
            first_play_bytes: FIRST_PLAY_BYTES,
            first_play_ms: FIRST_PLAY_MS,
        },
    };
    write(icecream_zone::INDEX, &serde_json::to_vec_pretty(&zone)?)?;
    if zone.first_play().len() < 4 {
        bail!("the first play is empty");
    }
    Ok(zone)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(dir: &Path) -> PathBuf {
        let shell = dir.join("shell");
        std::fs::create_dir_all(&shell).unwrap();
        for name in &SHELL[1..] {
            std::fs::write(shell.join(name), vec![0u8; 900_000]).unwrap();
        }
        shell
    }

    fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let rel = path
                        .strip_prefix(dir)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    out.insert(rel, std::fs::read(&path).unwrap());
                }
            }
        }
        out
    }

    #[test]
    fn the_first_play_is_the_yard_and_the_entry_hall_with_their_assets_within_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("out");
        let zone = bake(&haunt_rules::v1(), &shell(tmp.path()), &out).unwrap();
        let first = zone.first_play();
        assert!(first.contains(&"chunks/areas/yard.json".to_owned()));
        assert!(first.contains(&"chunks/areas/entry-hall.json".to_owned()));
        assert!(!first.contains(&"chunks/areas/kitchen.json".to_owned()));
        assert!(first.contains(&"chunks/meshes/jack-o-lantern.bin".to_owned()));
        assert!(
            !first.contains(&"chunks/cards/black-cat.ici".to_owned()),
            "the kitchen's cat streams later"
        );
        let all = files(&out);
        let report = icecream_zone::budget::check(
            std::str::from_utf8(&all["zone.json"]).unwrap(),
            |p| all.get(p).map(|b| b.len() as u64),
            all.keys(),
        )
        .unwrap();
        assert!(report.within(), "{}", report.line());
        // Every chunk the zone lists is there, and every area reads back.
        for c in &zone.chunks {
            assert!(all.contains_key(&c.path), "{}", c.path);
            if c.kind == Kind::Area {
                let _: haunt_rules::doc::Area = serde_json::from_slice(&all[&c.path]).unwrap();
            }
        }
        let head = House::parse(std::str::from_utf8(&all[HOUSE]).unwrap()).unwrap();
        assert!(head.areas.is_empty());
    }

    #[test]
    fn a_light_change_in_one_area_changes_one_chunk() {
        let tmp = tempfile::tempdir().unwrap();
        let shell = shell(tmp.path());
        let out = tmp.path().join("out");
        let house = haunt_rules::v1();
        bake(&house, &shell, &out).unwrap();
        let before = files(&out);
        let mut lit = house.clone();
        lit.area_mut("parlor").unwrap().light.preset = "jack-o-glow".into();
        bake(&lit, &shell, &out).unwrap();
        let after = files(&out);
        // The index lists every chunk's size, so it changes too.
        let changed: Vec<&String> = before.keys().filter(|k| before[*k] != after[*k]).collect();
        assert_eq!(changed, ["chunks/areas/parlor.json", "zone.json"]);
        // And the volume changes only the settings.
        let mut quiet = house.clone();
        quiet.audio.music = 0.2;
        bake(&quiet, &shell, &out).unwrap();
        let after = files(&out);
        let changed: Vec<&String> = before.keys().filter(|k| before[*k] != after[*k]).collect();
        assert_eq!(changed, [HOUSE]);
    }

    #[test]
    fn a_house_that_fails_its_checks_is_not_baked() {
        let tmp = tempfile::tempdir().unwrap();
        let mut house = haunt_rules::v1();
        house.area_mut("kitchen").unwrap().rect[0] = 1.0;
        let error = bake(&house, &shell(tmp.path()), &tmp.path().join("out")).unwrap_err();
        assert_eq!(error.to_string(), "The kitchen overlaps the back hall.");
    }
}
