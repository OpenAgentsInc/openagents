# Everglade medieval refactor: handoff

P9 resumes on October 7, 2026, from the saved
`wip/everglade-medieval-p9` commit `423c18519a`. Its public-pack cleanup
removes 13 replaced stand-ins and their far levels, updates the licensed-kit
documentation, and clears lamps and plaza furniture from doorways and
walks. The Verse Everglade tests pass (46 tests). The zone library passes
251 tests; its only failure is the stale public-pack digest, which the
queue regenerates. The world-tree snapshot and standing-point routes pass
without a snapshot change. The public pack lands
through `openagents artifact submit everglade-pack`; its verification is
recorded below after the queue runs.

The saved `wip/everglade-b2-doc` commit `52ec66fe15` holds one documentation
commit on top of B2, whose code landed as `0e0d8afd37`.

Status as of October 7, 2026. The plan, its decisions, and the "As built"
notes are in [Everglade medieval refactor](everglade-medieval-refactor.md).
The umbrella issue is #10903.

## What landed

| Phase | Issue | Commits | State |
| --- | --- | --- | --- |
| Decisions recorded | #10903 | `46a89d3587` | Done |
| P1: export and archive | #10894 | `89328cdd2a` | Closed; vendor archive uploaded October 7 |
| P2: kit pack, loader, proxies | #10895 | `8f6136a533`, first repin `ea39fa5cf2` | Closed |
| P3: materials and levels | #10896 | grade in `0a44257462`, tooling `50dca3b658` | Open: far levels and coplanar check |
| P4: kit houses and breaking | #10897 | `0a44257462` | Closed |
| P5: Stoop Lane | #10898 | `0a44257462`, `ea39fa5cf2` | Closed |
| P6: Main Street, the plaza, Market Row | #10899 | `611df73ca7`, repin `6b37d8ee57` | Closed |
| P7: the rest of the vernacular | #10900 | `19cc9a02c1` | Closed |
| P7b: the first town | #10927 | `c9c75765b5`, fix `7e77971050` | Closed |
| P8: web and phone | #10901 | `df3f5fa15e` | Closed; pack uploaded, Cloud Build grant in place |
| P9: cleanup | #10902 | recovered from `423c18519a` | Verse tests pass; public-pack queue pending |
| B1: offline baker | #10905 | `0360046732`, `983432c41f` | Closed |
| B2: lightmap layers | #10906 | by its subagent | See its issue |
| B3: time of day and destruction | #10907 | none | Not started |
| B4: tiers and measurement | #10908 | none | Not started |

The town has 65 medieval kit houses: Stoop Lane, Main Street, the Fountain
Plaza, Market Row, the Lantern Quarter, Well Square, the Knowledge,
Foundry, and Creative Districts, and the first town around the Commons.
The landmarks, Brownstone Row, the farm, the cabins in Walden Woods, the
smithy, the Boardwalk Cafés, the round Music Hall, and the beekeeper's hut
keep their models. The Greco-futurism district and the workshop hall are
unchanged.

## What's open

- **P3 (#10896).** Two pieces remain. First, far levels: the kit pack has
  no `lod/kit.<id>` models. The compiler accepts `<id>.far.gltf`, so the
  build script needs to write decimated far levels for the heavy pieces:
  the fountain (7,752 triangles), the lamps (2,976), and the stalls and
  carts. Second, the coplanar check: run `scripts/blender/coplanar.py`
  over a built house (bands over walls, floors over plinth tops) and fix
  the recipe in `layout/kit_house.rs`, not the pieces. Also compare a
  merged house with the demo's, side by side.
- **B2 (#10906).** Its issue holds what its subagent landed and what
  remains.
- **B3 (#10907).** Not started. It needs B2's layers. Kit lamps glow
  (their materials are `Emit_…`) but light nothing until B3.
- **B4 (#10908).** Not started. It needs B2 and P8. One 512 px kit pack
  serves every tier today (10.2 MB to transfer, about 28 MiB decoded),
  which is over the phone's 8 MiB transfer budget. A phone tier is part of
  B4.

## Commands

Run every Cargo command through the build lease, one at a time:

```sh
openagents lease build --keep-target-dir -- cargo test -p verse-zone-everglade --lib
openagents lease build --keep-target-dir -- cargo test -p verse --lib zones::everglade
openagents lease build --keep-target-dir -- cargo check -p everglade-web --target wasm32-unknown-unknown
openagents lease build --keep-target-dir -- cargo check -p coder-mobile
openagents lease build --keep-target-dir -- cargo test -p openagents-web --lib everglade
```

After a layout change, regenerate the world tree, then check that only
standing points and doors moved:

```sh
WORLD_TREE_WRITE=1 openagents lease build --keep-target-dir -- cargo test -p verse-zone-everglade --lib world_tree
```

Offscreen captures, with and without the kit:

```sh
VERSE_KIT_PACK=~/.openagents/verse/private/medieval-town/packs/<KIT_SHA256>.vtp \
  cargo run --release -p verse --example everglade_capture -- OUT.png city-stoop
cargo run --release -p verse --features dev-destruction --example town_capture -- OUT_DIR stoop
```

`VERSE_KIT_UNPINNED=1` lets an offline tool load an unpinned kit pack,
for example to compare grades.

## The kit pipeline

1. **Export.** `scripts/unreal/medieval_town_export.py` runs Unreal
   headless on the vault copy and writes
   `~/.openagents/verse/private/medieval-town/export/`. It normalizes the
   output, so two runs produce the same bytes.
   `scripts/unreal/medieval_town_archive.py` digests, compares, and
   archives exports.
2. **Private store.** The bucket is `openagentsgemini-verse-private-assets`.
   Vendor files and digests go under `vendor/medieval-town/`, and compiled
   packs under `packs/`. Cloud Build's account
   (`oa-mvp-automation@…`) may read `packs/` only.
3. **Build.** `scripts/unreal/medieval_kit_build.py` reads the committed
   recipe `scripts/unreal/medieval_kit_recipe.json` (our IDs to vendor mesh
   names) and writes one glTF per piece to
   `~/.openagents/verse/private/medieval-town/kit-build/`.
4. **Compile.** The `everglade_kit` example (`compile::kit`, under
   `Limits::KIT`) grades the images (`everglade_pack::kit::grade`) and
   writes `packs/<sha>.vtp` and the desktop zone cache. Every piece must
   keep within 0.1 m of its committed box in `kit::PIECES`.
5. **Pin.** `openagents artifact submit everglade-kit` lands a branch and
   repins `KIT_SHA256` and `KIT_BYTES` in
   `crates/verse-zone-everglade/src/zones/everglade_pack/kit.rs`. Never
   push a repin directly. A new digest also needs an upload to the
   bucket's `packs/` before the next web deploy.
6. **Serve.** `crates/openagents-web/cloudbuild.yaml` copies the pinned
   pack into the image, and the site serves `/everglade/kit/<sha>.vtp`. The
   browser fetches it beside the Everglade pack. The desktop and the phone
   download it once into the zone cache, by digest.
7. **Fallback.** `kit::install` puts the kit's models into the decoded
   Everglade pack, or a committed proxy for each piece when the kit is
   missing or a model leaves its box.

## Known issues and gotchas

- **The look.** The kit still reads greyer and more realistic than
  Everglade's painted houses. The grade's four constants are
  `GRADE_DETAIL` (0.6), `GRADE_SATURATION` (1.2), `GRADE_GAMMA` (0.85),
  and `GRADE_WARMTH` in `everglade_pack/kit.rs`. Changing them changes the
  pack, so a repin goes through the queue. The before and after captures
  are listed under Captures below.
- **Lamps.** Kit lamps glow at night, but nothing is lit by them until B3.
  Their glass reads cool white.
- **Proxies.** Without the kit, houses draw as grey boxes with dark window
  panes, which is coherent but plain. Collision always comes from the
  proxies, so walking matches with and without the kit.
- **Stories.** A story is 4.5 m floor to floor (a 4 m wall and the kit's
  0.5 m band), on a plinth sunk 1.25 m, so the floor is 0.75 m up the
  steps.
- **Breaking.** Kit pieces are carved one block each and merge into the
  static cells; they don't draw as instances.
- **Test edits.** The tall-ruin test lets a chunk lodge up to 4.5 m high
  in a rubble heap, up from 3 m. The instancing test counts merged kit
  pieces separately. The far-level count dropped from more than 600 to
  more than 300. The town's demolition tests now break the beekeeper's
  hut, which is the last village-kit house. The verse test's list of
  placed generated models no longer names the replaced stand-ins or the
  fountain.
- **Other users of `KitHouse`.** The Meteor Showcase
  (`crates/verse/src/zones/meteor_showcase.rs`) builds kit houses too, so
  a new field there breaks the verse build.

## Owner steps left

Only the 60 frames per second checks on the reference laptop and a phone,
after the openagents.com deploy that serves the kit (`NEEDS_OWNER.md`).

## Captures

Captures are offscreen and private, in `/private/tmp/claude-501/medieval/`.
That directory is cleared on reboot, so copies of the later ones are in the
session's scratch directory. The P5 to P7 captures are `p5-stoop-*.png`,
`p6-*.png`, `p7-*.png`, `p7b-approach.png`, `p4-house/*.png` (a kit house
breaking), and `p3-grade-before.png` and `p3-grade-after.png`.
