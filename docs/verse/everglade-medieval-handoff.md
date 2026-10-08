# Everglade medieval refactor: handoff

P9 lands on October 7, 2026, from the saved
`wip/everglade-medieval-p9` commit `423c18519a`. The cleanup lands as
`f008baa090`, and the `everglade-pack` queue repins it in `4b9ad70748`.
It removes 13 replaced stand-ins and their far levels, updates the
licensed-kit documentation, and clears lamps and plaza furniture from
walks and doorways. The public pack drops from 12,303,751 to 10,636,202
bytes (13.55 percent), with digest
`a82df378ca7d06d9c755ae24076c89270d8a8097509c54a166d941da05f9de2f`.
The queue's regeneration and consistency check pass. Both final suites
pass on the landed pack: 256 zone-library tests and 46 Verse Everglade
tests. The scoped formatting check passes. The current world-tree snapshot
and every standing-point route pass; P9 changes no world-tree IDs or
places. No licensed asset files enter the repository.

The saved `wip/everglade-b2-doc` commit `52ec66fe15` holds one documentation
commit on top of B2, whose code landed as `0e0d8afd37`.

Status as of October 7, 2026. The plan, its decisions, and the "As built"
notes are in [Everglade medieval refactor](everglade-medieval-refactor.md).
The umbrella issue is #10903.

## October 8 coordination checkpoint

P9 (#10902) is closed. The web deploy record is `26bb7ef2ab`; production
serves `openagents-web:d3f3ad546c` and the pinned private kit with HTTP
`200`. That image predates P9's smaller public pack. The terminal retention
fix (#10909) is closed at `98df2423e3`, with its full-hour release soak
and consumer check passing.

P3 (#10896) remains open on the pushed branch
[`codex/everglade-p3`](https://github.com/OpenAgentsInc/openagents/tree/codex/everglade-p3).
Checkpoint `7d27c2b761` updates its handoff with the source, commands,
and private evidence paths. Capture source `1d38e17a75` passes the scoped checks.
All 65 houses pass the unsuppressed coplanar detector and the compiled
budgets: near levels have at most 10,000 triangles and 12 draws, middle
levels at most 3,000 triangles and one draw, and far levels at most 800
triangles and one draw. Six remote raster captures cover a cabin at
horizontal selector distances of 10, 50, and 120 m and the market's
exterior, roof, and interior. Their exact eye distances and hashes are in
the capture receipts. The near cabin view favors the roof; the interior
is dark and shows the existing rain-through-ceiling limitation.

The private candidate pack has digest
`c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e`,
21,467,658 bytes, and 63,438,848 decoded texture bytes. It is unpublished;
the larger decoded footprint also needs B4's phone-tier review. P3 still
needs matching exported-demo and grade comparisons, first and repeated
damage plus `R` restoration checks, fresh-main integration, and
`openagents artifact submit everglade-kit`, followed by the private upload
and web deploy. Do not repin directly or commit licensed files.

B2 (#10906) has code in `0e0d8afd37` but remains open for verification
and publication. B3 (#10907) has an unverified source checkpoint on
[`codex/everglade-b3`](https://github.com/OpenAgentsInc/openagents/tree/codex/everglade-b3),
commit `a2f4bbf7d3`; no bake, check, or capture ran for that checkpoint.
B4 (#10908) and umbrella #10903 remain open. This run stops new assignments
at 89 percent shared usage; active work saves resumable checkpoints.
Recheck each issue and claim before resuming. The sections below retain
the original October 7 plan.

## What landed

| Phase | Issue | Commits | State |
| --- | --- | --- | --- |
| Decisions recorded | #10903 | `46a89d3587` | Done |
| P1: export and archive | #10894 | `89328cdd2a` | Closed; vendor archive uploaded October 7 |
| P2: kit pack, loader, proxies | #10895 | `8f6136a533`, first repin `ea39fa5cf2` | Closed |
| P3: materials and levels | #10896 | grade in `0a44257462`, tooling `50dca3b658` | Open: house levels, private compilation, and visual acceptance |
| P4: kit houses and breaking | #10897 | `0a44257462` | Closed |
| P5: Stoop Lane | #10898 | `0a44257462`, `ea39fa5cf2` | Closed |
| P6: Main Street, the plaza, Market Row | #10899 | `611df73ca7`, repin `6b37d8ee57` | Closed |
| P7: the rest of the vernacular | #10900 | `19cc9a02c1` | Closed |
| P7b: the first town | #10927 | `c9c75765b5`, fix `7e77971050` | Closed |
| P8: web and phone | #10901 | `df3f5fa15e` | Closed; pack uploaded, Cloud Build grant in place |
| P9: cleanup | #10902 | `f008baa090`, repin `4b9ad70748` | Closed; both suites and pack consistency pass |
| B1: offline baker | #10905 | `0360046732`, `983432c41f` | Closed |
| B2: lightmap layers | #10906 | by its subagent | See its issue |
| B3: time of day and destruction | #10907 | checkpoint `a2f4bbf7d3` on `codex/everglade-b3` | Unverified; claim released while P3 takes priority |
| B4: tiers and measurement | #10908 | none | Not started |

The town has 65 medieval kit houses: Stoop Lane, Main Street, the Fountain
Plaza, Market Row, the Lantern Quarter, Well Square, the Knowledge,
Foundry, and Creative Districts, and the first town around the Commons.
The landmarks, Brownstone Row, the farm, the cabins in Walden Woods, the
smithy, the Boardwalk Cafés, the round Music Hall, and the beekeeper's hut
keep their models. The Greco-futurism district and the workshop hall are
unchanged.

## What's open

- **P3 (#10896).** Claimed on October 8 on `codex/everglade-p3`.
  The canonical report covers all 65 houses: 3,156–11,988 triangles,
  26–70 selected draws, and 10–12 materials at each of 10, 50, and 120 m.
  The market hall alone exceeds the near triangle budget. The renderer
  checkpoint `f41cc66779` groups original house submissions at one anchor,
  keeps their destruction ranges and actual world bounds, and selects one
  distance level with shared hysteresis. Its two focused regressions pass,
  including discontinuous jumps, damage at far distance, repeated range
  edits, and restore. Three layout and walking tests pass after 3 mm gaps
  separate floor/plinth and band/wall finish faces. The headless Verse
  consumer check passes; the default check stops on the remote runtime's
  missing ALSA development metadata.
  Public integration checkpoint `d5f04dc475` adds optional canonical house
  levels and damage fallback; it is unformatted and unverified. No P3
  artifact or main commit has landed. Private CPU decimation produces
  awning/cart/fountain/lamp/stall far levels of 127/255/800/192/256
  triangles, within budgets and with exact original bounds. Seven focused
  kit-admission tests pass. Those private models still need compilation
  and admission in the real pack.
  Remaining acceptance: private middle/far house shells and shared atlases,
  market-hall near reduction without removing visible interiors, actual
  selected batches within 10,000/3,000/800 triangles and 12/5/1 draws,
  post-gap coplanar checks, demo comparison, grade captures, and one house
  at 10/50/120 m. The initial coplanar scan finds genuine floor/band recipe
  overlaps and two tiny roof-end material seams inside licensed meshes;
  assess those seams visually rather than claiming zero failures.
- **B2 (#10906).** Its issue holds what its subagent landed and what
  remains.
- **B3 (#10907).** Checkpoint `a2f4bbf7d3` on `codex/everglade-b3` holds
  unverified blending, damage repair, private-layer preflight, and acceptance
  tools. Its handoff lists the exact remaining checks, private scene identity,
  captures, and timing plan. The claim is released while P3 takes priority;
  no B3 Cargo command, bake, capture, or measurement has run.
- **B4 (#10908).** Not started. It needs B2 and P8. One 512 px kit pack
  serves every tier today (10.2 MB to transfer, about 28 MiB decoded),
  which is over the phone's 8 MiB transfer budget. A phone tier is part of
  B4.

## P3 checkpoint inputs

Keep licensed inputs and captures outside Git. The verified private kit is
`dae1612d4c22438a933c27b406c1e18fe134b13eab8eb5240ddcf5506ffb0b93`
(10,238,689 bytes). On `coderos-4080`, the stable source checkout is
`~/.openagents/scratch/process-2159833/medieval-p3-tools/source`; its warm
Cargo target is `~/work/openagents-target-agent11`. The same toolbox keeps
`house-check-actual.log`, the 65 canonical exports in `house-check-actual/`,
`coplanar-house-00.log`, `coplanar-houses-01-04.log`, `far-clamped.log`, and
`far-inspection-clamped.jsonl`. Private `export/`, `kit-build/`, and `packs/`
remain under `~/.openagents/verse/private/medieval-town/`.

The portable headless CPU runtime is Blender 4.5.14 LTS. Its official archive
SHA-256 is `9ba871ff2ecd36526b77432745980b7e6664ecd0c7ca11c48849073dcfe06da3`.
The toolbox's `blender-cpu` wrapper supplies read-only Nix libraries and
bounds workers to four. House export is the filtered ignored
`export_private_house_acceptance` example test in `kit_house_check`:
precompile it under the build lease, then execute its test binary with
`VERSE_KIT_PACK` and `VERSE_KIT_HOUSE_OUTPUT` pointing outside the repository.
Do not run GPU compute, ray tracing, or bake regeneration on the Mac.

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

The requested web image `openagents-web:d3f3ad546c` serves the kit on
revision `coder-web-d3f3ad546c-20261008043217` at 100 percent traffic. The
deployment record is `26bb7ef2ab`. That image predates P9 and retains the
older public pack until a later image deploys the cleanup.
