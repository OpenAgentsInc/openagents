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

Status as of October 8, 2026. The plan, its decisions, and the "As built"
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
| P3: materials and levels | #10896 | grade in `0a44257462`, tooling `50dca3b658` | CPU acceptance passes; visual acceptance and publication remain open |
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

- **P3 (#10896).** Checkpointed on October 8 on `codex/everglade-p3`;
  the claim is released and the board returns to Todo for the usage stop.
  All 65 actual houses pass strict selected budgets: near 3,156–10,000
  triangles/10–12 draws, middle 2,999–3,000/one, and far 799–800/one.
  The real candidate pack admits all 53 pieces, five prop far models, and
  131 house models. Every house and the reduced market-hall near model
  pass the unchanged coplanar detector without exemptions. Focused
  transform/paint, compiler/admission, selector/damage/restore, and walking
  checks pass; the rebased headless consumer check also passes. Six
  source-bound geometry/LOD captures are accepted: cabin at horizontal
  10/50/120 m and market exterior/gable/interior. Near favors the roof;
  interior lighting and rain remain limitations. Remaining acceptance is
  the same exported demo actor before/after merge and grade, grade captures,
  far damage/repeated damage/restore captures, and any reviewed near facade
  supplement. Rebase and check integration against current main before
  artifact-queue publication, private upload, and dependent deployment.
  No P3 artifact or main commit has landed. Exact source/input/candidate
  identities, captures, limitations, and resume commands follow below.
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
`far-inspection-clamped.jsonl`. The supplied demo reference is
`export/maps/Maps__medieval_town.json`, with its actor/component transforms,
GLBs, material tables, and textures; no matching retained screenshot is
available. Render that original input offscreen on the remote machine and
label it "exported Unreal demo input," not an Unreal-engine screenshot.
`demo-reference/exported-demo-input.blend` and `reference.json` preserve
actor `BP_residential_house_15`, its 87 original components, 18 material
identities, and source transforms. That actor differs from the canonical
width/depth/door bays, plinth height, and floor spacing; compare the same
demo actor before/after merge and grade, and report the canonical gap.
Private `export/`, `kit-build/`, and `packs/`
remain under `~/.openagents/verse/private/medieval-town/`.

Historical stages are retained for provenance; their admission and budget
failures are resolved by the final CPU reports and candidate acceptance
below. `house-check-actual.log` first measures 3,156–11,988 triangles,
26–70 draws, and 10–12 materials at every distance. Renderer checkpoint
`f41cc66779` reduces original near submissions to 10–12 and passes two
selector/destruction regressions plus three layout/walking tests. At that
stage the market hall exceeds 10,000 triangles and middle/far still use
original pieces. Integration `3314ae5773` adds private levels, editable
fallback ranges, inverse transforms, and atlases; three transform/paint,
five compiler, and seven admission tests pass, with a passing palette
fixture. Initial prop far exports have 127/255/800/192/256 triangles for
awning/cart/fountain/lamp/stall. The market pilot has 10,000/3,000/800
triangles with all original parts. Its normalized admission is pending
at that historical stage and passes in the later complete candidate.

The historical export source is `3314ae5773a09aab3d160b84874d9f90a1e61e0b`.
`raw-kit-p3-identities.txt` records recipe SHA-256
`fd798e88b6425306b72b7952df099e2e3c7b4a2a887715936553d4093eadd78e`,
export-manifest SHA-256
`b2edbdd659f7860c24935fe00d115cab75ac5f05a945eca66169f0b86c18cee0`,
and build-report SHA-256
`5362a674915f8cbabdd2503a6a12e874b7cfb2630eaa0ffb556fa3b2b9cea2cc`.
`raw-kit-p3.log` confirms 53 pieces, zero skips, and 28 textures. The approved
compiled export writes `house-check-p3-source/` and its log, including a
normalized recipe manifest for the corrected CPU atlas batches. It passes
in 244 seconds and records all 65 houses with 10–12 actual near draws and
3,156–11,988 triangles. The manifest has 65 unique recipes and SHA-256
`6c035e6b195f0a960c988e05c39aa3e2b6ff49f9a6f6230dfb25b5a4d36deb15`.
`house-check-p3-source-report.json` retains the extracted distance report.
All 33 corrected CPU atlas batches complete. `house-level-inventory.json`
records 131 private models: market-hall near is 10,000 triangles in 12
primitives; every middle model is 2,999–3,000 triangles in one primitive;
every far model is 799–800 triangles in one primitive. The build has 61
textures, including 33 shared 512 px atlases. Three far models lose a
0.131907 m negative-Z roof extremum: `house-c1ceda6765b05298`,
`house-e48501b0455065eb`, and `house-a06a4e4efaba17de`. The bounded generator
correction fits actual indexed vertex positions along that axis to its
source span and leaves UVs and materials intact. The first fixture passes,
but the rerun exposes loose decimator vertices that do not belong to any
rendered triangle; measuring all vertices cannot prove the rendered box.
The extended fixture includes that case and passes. Reruns of batches 19,
25, and 29 at `a230f7a331` restore all 131 models within 5.95 mm of indexed
source bounds. Only the three far glTF/bin pairs change; 289 other files
stay byte-identical. UVs, materials, and indices stay unchanged, with a
maximum real vertex displacement of 0.131907 m. Exact pre/post boxes are in
`house-extrema-receipt.json`; `house-level-inventory-corrected.json` passes.
These historical source-file counts precede the final pack admission and
distance-selected acceptance recorded below.

The post-gap coplanar reports cover all 65 houses in
`coplanar-p3-source-*.jsonl`: zero inter-piece recipe overlaps, and eight
intra-mesh face-pair overlaps per house, about 0.0277 m², in the supplied
roof-end `T_trim_wood_01_BC`/`T_trim_wood_02_BC` seams. The exact raw-piece
inspection in `coplanar-roof-input.log` corrects the earlier wood/plaster
label. No exemption or suppression hides these seams;
correction and visual assessment are pending at that historical stage.
The subsequent CPU reports and accepted gable capture resolve the geometry
gap. The bounded prototype
offsets only `MI_trim_wood_02_01` by +3 mm X before mirroring in the two
derived roof-end exports. Recipe offsets are limited to 5 mm; original
vendor GLBs remain untouched. Three synthetic tests pass, including the
mirrored gap and recipe validation. The actual private preview passes the
unmodified detector for both pieces: 255 triangles and six primitives each,
44 moved vertices each, and maximum displacement 0.00300002 m. UV, color,
normal, index, and material arrays remain unchanged. Actual boxes remain
within the committed piece bounds. `roof-finish-preview-receipt.json`
records the source, recipe, GLB, and output byte identities.

Full CPU regeneration uses source
`c17428fc306692538147937b2d635461cca52682`, recipe SHA-256
`269ac649621b7738d31eb07e0da61d93569df8b07e2d262ac9d721e88f5b0d50`,
and build-report SHA-256
`31bd436846596d6cf1e742dfc7d7c9b793b4c4cd8d2e28c83c1dbb38bb75f11b`.
The original roof GLB remains
`4757fdd45e3ca4cf44108f3e144d5093def055967fc82cf679a2e63ca5d51bea`.
`raw-kit-p3-roof-identities.txt` also pins the compiled export test binary:
`483da388fa8287311321371bab54b349a7c53c351419a7946121c7f7b923ba2e`
(Rust source `3314ae5773`). The raw rebuild passes with 53 pieces, zero
skips, and 28 textures. `kit-build-p3-before-roof/` retains the preceding
private build. The current 65-house source export writes
`house-check-p3-roof/` and passes in 242 seconds. All 33 corrected CPU
atlas batches pass; `house-level-inventory-roof.json` verifies 131 models,
61 textures, 33 atlases, and indexed bounds within 5.95 mm of the source.
`coplanar-p3-roof-*.jsonl` covers all 65 houses with zero recipe or intrinsic
overlap pairs; `coplanar-market-near-roof.log` confirms the reduced near
model also passes the unchanged detector.

`house-check-p3-integrated.log` compiles and admits the real private pack,
then verifies actual distance-selected batches for every canonical house;
the compiled test passes in 266 seconds. The extracted
`house-check-p3-integrated-report.json` has SHA-256
`db97503fbff90dc7157aa641b5021dfa26ef37b662d4849322b9c448c05e53be`.
All 65 houses meet the plan: near 3,156–10,000 triangles/10–12 draws,
middle 2,999–3,000/one, and far 799–800/one. Use the raw-source manifest in
`house-check-p3-roof/` for regeneration; the integrated report already
selects the reduced near model and is not a generator input.

Private generation remains frozen at `c17428fc30`. The public feature
branch is rebased onto `2c6af8ad347f80b2e9f950445ee9adacd734db51`; it preserves
the scene-lit particle changes and `NEEDS_OWNER.md`. Source
`1d38e17a75ba88f77a585d628d49a46641325329` passes scoped formatting and the
composed headless Verse consumer check in 13.71 seconds. Its Cargo JSON
SHA-256 is `67fca990da0724378b691dfca0af630218596fe2626a6f48183285ffcf31fa68`.
The strict checker option `VERSE_KIT_HOUSE_VERIFY=1` writes protected
candidate pack bytes and `candidate.json` without changing the pin. Its
filtered no-run passes in 45.22 seconds at source `2b8eade165`; executable
SHA-256 is `e1c17be0b155103d73ddf47acba0a2372e99bf2a06b0931158019d00afc79bac`
and Cargo JSON SHA-256 is
`9fa9841aa9cfc92b2fcad89ac09abec7e5e5774acbf66067c9ca153d7e7986a4`.
The existing capture example has an ignored, environment-driven wrapper
that requires a private output directory. Its first headless no-run
exposes desktop-only Alice workshop references; `1d38e17a75` gates only
those capture paths and preserves desktop behavior. The repaired no-run
passes in 5.15 seconds; executable SHA-256 is
`2abadd71e5e2af581a4bfc7377efdc68b8c386a8f2de21359a74afab22d6b60d`
and Cargo JSON SHA-256 is
`0ac05bd77dfcc54b60a6a14a35136c034d9ac840f74b7eac9a09d4224f645059`.
The toolbox retains `p3-candidate-no-run*`, `p3-capture-no-run*` (including
the failed attempt), and `p3-composed-headless-check*` with full commands,
logs, and executable paths. The team Cargo slot is released after this
batch. The compiled strict checker passes in 262.43 seconds and writes
candidate `c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e`
to `house-check-p3-candidate/`: 21,467,658 bytes, 189 models, 283,800
triangles, and 63,438,848 decoded texture bytes. Its hash and `0600` file
permissions pass verification. `p3-candidate-cpu-report.json` has SHA-256
`a1923f8f0407986ea77f923fd08913f8d0e69fa7d907d39753224a29c3bbb380`.
All 65 strict distance-selected budgets still pass. The 60.5 MiB desktop
texture payload does not establish the B4 phone tier.

Six serial remote quiet/GPU captures pass in 57.75–61.22 seconds with
the immutable capture executable above and candidate `c559955403b4`.
The coordinator accepts them as geometry and LOD evidence. The near cabin
view favors the roof and gives limited facade/palette evidence; middle and
far retain recognizable massing, roof bounds, and palette. Market exterior
and gable remain coherent; the interior retains floor, ceiling, walls,
and windows. Its darkness and existing rain-through-ceiling limitation
do not establish lighting acceptance. Every caption states horizontal
selector distance, full eye/target coordinates, and Euclidean eye-to-floor
anchor distance. The private `p3-raster/receipt-index.json`, SHA-256
`6c3f4b3545c98a9d922c64a8082f0a7436a7800e131ae7a528df375b934717c1`, binds
the six PNG hashes, exact poses, commands, sources, and lease receipts:

| Private PNG in `p3-raster/` | Horizontal distance (m) | Euclidean distance (m) | SHA-256 |
| --- | ---: | ---: | --- |
| `cabin-h10.png` | 10 | 19.939 | `22eda700eaa7a304511bae18cdf93adb7046fab35a27d1409252e5f0b2a01d91` |
| `cabin-h50.png` | 50 | 53.578 | `17ff6a18e8ac83eebad5c1bdf7a4dd9bd78b1a52453f279c14b258b4f8cfeb2e` |
| `cabin-h120.png` | 120 | 123.513 | `8283a0ea9d543197457bccd2e50c1575f35299c645dccfc94fc77bbd1487f2a3` |
| `market-exterior.png` | 32 | 37.344 | `79ea3df40b97c906e337a45dac3ce3f9293b12d09998f263c4900959ab3fd4fa` |
| `market-roof.png` | 25 | 33.467 | `a6444a1741794bb4f8c6378cdbb6ab827aeebfc4be4b1eaf217ed12d389e28f5` |
| `market-interior.png` | 3 | 3.250 | `a28901d6d0e877e9764285d63ab024610bc49c0ef15f05fbd4cb1db4dd615580` |

The remote directory is
`/home/christopherdavid/.openagents/scratch/process-2159833/medieval-p3-tools/p3-raster/`.
The Mac copy, including receipts, is
`/Users/christopherdavid/.openagents/scratch/codex-01a119ad-8a81-7882-9d04-97d83523f0c8/p3-tools/p3-raster/`.
No P3 artifact submission, repin, upload, deployment, or main landing has run.
Remaining acceptance is the accurately labeled same exported demo actor
before/after merge and grade, matching grade captures, far damage/repeated
damage/restore captures, and any reviewed near facade supplement. Then
rebase onto current main, preserve independent changes, obtain focused
composition checks, and submit through `everglade-kit` before private upload
and the dependent web deploy. The claim is released for the usage stop;
these are resumable steps, not an external blocker or owner-only work.

Resume only after a fresh claim/status audit and coordinator resource window.
The retained private helper checks the immutable executable and candidate
hashes and takes quiet/GPU leases. To repeat one approved capture, replace
`cabin-h50` with another name from the table:

```sh
openagents lease run --class bench --place remote:coderos-4080 --priority normal -- sh -c '
  cd "$HOME/.openagents/scratch/process-2159833/medieval-p3-tools/source" &&
  OPENAGENTS_BUILD_LEASES=1 OPENAGENTS_SLOT_FREE_GB=25 OPENAGENTS_LEASE_PRIORITY=normal \
    python3 ../p3-raster-capture.py cabin-h50'
```

To repeat the CPU candidate acceptance, use its retained source/build
identities and output outside Git:

```sh
openagents lease run --class build --place remote:coderos-4080 --priority normal -- sh -c '
  cd "$HOME/.openagents/scratch/process-2159833/medieval-p3-tools/source" &&
  export CARGO_TARGET_DIR="$HOME/work/openagents-target-agent11" CARGO_BUILD_JOBS=4 RUST_TEST_THREADS=4 \
    OPENAGENTS_BUILD_LEASES=1 OPENAGENTS_SLOT_FREE_GB=25 OPENAGENTS_LEASE_PRIORITY=normal \
    VERSE_KIT_HOUSE_BUILD="$HOME/.openagents/verse/private/medieval-town/kit-build" \
    VERSE_KIT_HOUSE_OUTPUT="$HOME/.openagents/scratch/process-2159833/medieval-p3-tools/house-check-p3-candidate" \
    VERSE_KIT_HOUSE_VERIFY=1 &&
  unset VERSE_KIT_PACK VERSE_KIT_BAKE &&
  openagents lease build --keep-target-dir --priority normal -- \
    timeout 600 "$CARGO_TARGET_DIR/debug/examples/kit_house_check-d503a2e1635f07a5" \
    --ignored --exact tests::export_private_house_acceptance --nocapture'
```

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
- **Breaking.** Kit pieces are carved one block each. The P3 checkpoint
  groups intact house submissions and retains editable per-piece ranges
  for damage fallback and restore; those pieces do not draw as instances.
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
