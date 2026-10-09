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

P9 (#10902) is closed. Deploy record `26bb7ef2ab` completes the requested
`openagents-web:d3f3ad546c` deployment. The later component catalog deploy
preserves its game assets; the P3 deployment below now serves P9's smaller
public pack and the reviewed private kit. The terminal retention
fix (#10909) is closed at `98df2423e3`, with its full-hour release soak
and consumer check passing.

P3 (#10896) lands as `682435a6c1`; the `everglade-kit` artifact queue
repins the reviewed pack in `5415c483de`. Registry prerequisite
`14437ac3c2` includes whole-house level generation in the queue's
regeneration and consistency check. All 65 houses pass the unsuppressed
coplanar detector and the compiled budgets: near levels have at most
10,000 triangles and 12 draws, middle levels at most 3,000 triangles and
one draw, and far levels at most 800 triangles and one draw.

Capture source `b903f52e2b` completes the matching exported-demo and grade
views, plus first and repeated far damage and exact restoration. The six
geometry views and fresh scoped checks are recorded below. The reviewed
pack is `c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e`,
21,467,658 bytes, with 63,438,848 decoded texture bytes (60.5 MiB).
P3 is code-complete and deployed on
`coder-web-p3-d2fb95d33d-20261008140843`. Staging with no traffic and
production each pass 33 HTTP checks, both hardware browser render paths,
and 30 catalog assertions. This footprint needs B4's web and phone tier work.
No licensed geometry or captures enter Git.

B2 (#10906) verifies and publishes the landed per-vertex implementation
from `0e0d8afd37`; its queue pin is `d138fad182` and streamed delivery is
`a80bde9114`. Second-UV storage and denser lamp probes remain work for
B4 and umbrella #10903. B3 (#10907) closed on October 9 (`720904b4c2`).
B4 (#10908) and umbrella #10903 remain open. New delegation remains
stopped; the coordinator continues directly after fresh claim audits.
Recheck each issue and claim before starting its work. The sections below retain
the original October 7 plan.

## What landed

| Phase | Issue | Commits | State |
| --- | --- | --- | --- |
| Decisions recorded | #10903 | `46a89d3587` | Done |
| P1: export and archive | #10894 | `89328cdd2a` | Closed; vendor archive uploaded October 7 |
| P2: kit pack, loader, proxies | #10895 | `8f6136a533`, first repin `ea39fa5cf2` | Closed |
| P3: materials and levels | #10896 | `682435a6c1`, repin `5415c483de` | Closed; source, artifact, capture, and website checks pass |
| P4: kit houses and breaking | #10897 | `0a44257462` | Closed |
| P5: Stoop Lane | #10898 | `0a44257462`, `ea39fa5cf2` | Closed |
| P6: Main Street, the plaza, Market Row | #10899 | `611df73ca7`, repin `6b37d8ee57` | Closed |
| P7: the rest of the vernacular | #10900 | `19cc9a02c1` | Closed |
| P7b: the first town | #10927 | `c9c75765b5`, fix `7e77971050` | Closed |
| P8: web and phone | #10901 | `df3f5fa15e` | Closed; pack uploaded, Cloud Build grant in place |
| P9: cleanup | #10902 | `f008baa090`, repin `4b9ad70748` | Closed; both suites and pack consistency pass |
| B1: offline baker | #10905 | `0360046732`, `983432c41f` | Closed |
| B2: lightmap layers | #10906 | `0e0d8afd37`, verification on `codex/everglade-b2-verification` | Per-vertex implementation verified and deployed; UV storage and denser lamp probes remain deferred |
| B3: time of day and destruction | #10907 | `720904b4c2` | Closed October 9: blended suns, relight over the layers; the `codex/everglade-b3` checkpoint was not used |
| B4: tiers and measurement | #10908 | coordinator | Browser layer loading in progress |

The town has 65 medieval kit houses: Stoop Lane, Main Street, the Fountain
Plaza, Market Row, the Lantern Quarter, Well Square, the Knowledge,
Foundry, and Creative Districts, and the first town around the Commons.
The landmarks, Brownstone Row, the farm, the cabins in Walden Woods, the
smithy, the Boardwalk Cafés, the round Music Hall, and the beekeeper's hut
keep their models. The Greco-futurism district and the workshop hall are
unchanged.

## Bake status

- **B2 (#10906).** The landed per-vertex implementation is verified and
  deployed. Artifact submission `01791470792224747042-2390831-0` lands
  its completed output in `d138fad18276ec7942c6116bbf992c3062ae5841`.
  Recovery rebases the generated commit and runs `--layers --check`;
  it does not rebake when unrelated main changes arrive. The existing
  output is 51,682,623 bytes with SHA-256
  `14ae7f75e9ce4f81483f6f44369753545cb2cab892177607438b3077ebbbae23`,
  scene `a673374a9399f0f564e558bf50cc608a23227d39893bada078093fac021379df`,
  and bake key `5f9adbe288bb182e946991e934b169163e2dabfff2adeb9bcdd37f6b9627d207`.
  Five focused layer/fallback tests and scoped formatting pass. Four
  noon/night layered/fallback capture runs retain 15 PNGs; reports confirm
  the NVIDIA adapter, matching scene, active layers, and full night lamp
  intensity. Night doorway differences are subtle; these captures do not
  establish chart-seam acceptance. The original second-UV lightmaps and
  denser lamp probes remain deferred under B4 and umbrella #10903.
  Streaming fix `a80bde9114` passes two route tests, including a 33 MiB
  response. Cloud Build `b8bc150d-1be4-4e75-af79-e92713a5f3e4` adds only
  the existing layer file to the newer onboarding image, preserving its
  native server and browser assets. Revision
  `coder-web-b2-88cb5f7599-20261008174258` now serves all traffic. The
  zero-traffic `new` tag and production each pass complete digest/size
  downloads, route checks, and invalid-name refusals. Ten existing public
  responses match the preceding production image byte-for-byte. A native
  empty-cache download passes, verifies the digest, and decodes all four
  sun layers and 4,326,184 vertices. Private receipts and captures stay in
  the coordinator's `b2-verification/` scratch directory; no licensed
  bytes enter Git. The earlier buffered staging image and superseded
  server build never receive production traffic.
- **B3 (#10907).** Closed on October 9 with `720904b4c2`, written fresh
  on main rather than from the `codex/everglade-b3` checkpoint. The town
  blends the two baked suns either side of the hour and recombines on a
  worker in 1/64 steps; desktops relight what breaks over the layers and
  `R` restores them. Measurements are in the refactor plan's B3 entry and
  the issue.
- **B4 (#10908).** The coordinator holds the claim on
  `codex/everglade-b4-publication`. Main `984bca94e3` publishes the existing
  51,684,139-byte VLAY through the reuse-only artifact queue, with SHA-256
  `fc5414a1bfef9e730f3d7d779e4447f12cc86d4e571042eec42518abb30ef7c2`.
  Browser loading verifies that file before installing the town, skips the
  stepped bake, and reports `offline_light` under `?frames`.
  Exact reviewed Mac, Linux, and browser scene identities are bound to the
  artifact SHA, source recipe, and target recipe. The retained audit in
  `bench/verse/2026-10-08/layer-scene-compatibility/` proves identical
  topology, materials, UVs, vertex colors, and licensed images; positions
  differ by at most 0.031 mm, and nine procedural dirt pixels differ by
  at most one byte per channel. Unknown scenes and debug topology remain
  rejected. The compatibility, reuse, and private artifact tests pass.
  No new bake is needed when unrelated main commits land.
  Main `332a88c0f0` streams large browser modules; `c103b903f0` applies
  browser chrome tokens through the DOM style API under the site's strict
  CSP. The optimized production module is 28,450,183 bytes. Cloud Build
  `889f9249-7d6a-427e-8889-9178dba04e35` overlays the verified module and
  existing VLAY on the live image. Revision
  `coder-web-b4-dbd84fdb3d-20261008195645` now serves 100% of traffic.
  Staging WebGPU and WebGL2 render with `offline_light: true` and no
  browser errors; full staging and production file checks pass. Production also passes both browser backends with baked light active
  and no browser errors.
  Spatial tier reduction and budgets remain open. The 512 px kit still
  serves every tier (21,467,658 bytes transferred, 60.5 MiB decoded).
  B3's final repair commit needs a subsequent browser module refresh;
  preserve the immutable bake and the latest live native web image.

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
These captures precede the final P3 landing. The source and artifact pin
are now on main, and the reviewed pack is uploaded and deployed. The
completed checks and deployment records follow.

### Completed demo, grade, and far damage acceptance

Capture source `b903f52e2b833f345930cd19e0d0c98106264231` uses the
original exported `BP_residential_house_15`: 87 visible components and
their original transforms and material overrides. The base-color conversion
produces 12 materials and 19,633 triangles. The actual Rust merge preserves
bounds, triangles, and every matching rendered pixel (maximum delta zero).
These are exported Unreal demo inputs rendered by Verse, not Unreal-engine
screenshots; the canonical-house composition gap described above remains.
Applying the real kit grade changes 903,274 rendered channels while keeping
the camera and geometry fixed.

The 120 m cabin inspection uses one persistent GPU renderer and the actual
`Town` damage and `R` restoration paths. Before damage it submits one
800-triangle scene draw. First and repeated damage hide 15 and 20 pieces,
respectively, activate the original-piece fallback, and change the captured
pixels. Restoration clears every hidden piece and fallback and reproduces
the original far-house PNG exactly. This studio inspection moves fog beyond
120 m so it can show the model; it does not measure gameplay atmosphere.
The earlier 110 m studio-fog attempt is retained as rejected blank evidence.

The immutable capture executable has SHA-256
`9958ce2523e0ceb0b2954dbac940b1704816b94f2a3399135b2a0a2660629329`.
The demo report is `3bee5bd80bd79d2480159736143e50ec91da9225558b6f8dc15d288da44ef4a0`;
the damage report is `733e33fbf7ba4d491559fbf0a3bc4790923d116fca7128581a1550a806aab1b8`.
Both remote directories are in the same toolbox:
`p3-final-demo/captures-b903f52e2b83/` and
`p3-final-damage/captures-b903f52e2b83/`. The owner's local copies are
`/Users/christopherdavid/.openagents/scratch/codex-01a119ab-cb4c-7331-b0dc-8ddce4fb09a0/p3-visual/accepted-demo/`
and `accepted-damage/`. Every PNG has a hash in its private receipt.
The original/merged demo, grade views, and all four damage views are inspected.
No licensed captures or geometry enter Git.

Fresh leased checks pass: three house-recipe tests, seven compiler tests,
seven kit admission tests, two group selector/damage/restore tests, scoped
formatting, a headless capture compile, and the wasm32 web consumer check.
Logs remain in `p3-final-checks-64c25fecb1/`; the formatting-only successor
matches the checked source. The web consumer reports existing unused-import
and dead-field warnings. No broad gate or unrelated test suite ran.

P3 is complete. The retained private helper checks the immutable
executable and candidate hashes and takes quiet/GPU leases. To repeat one approved capture, replace
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
- **Breaking.** Kit pieces are carved one block each. P3
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

Only the physical reference laptop and phone checks remain
(`NEEDS_OWNER.md`, #10901). Repeat the tier measurements after B4; the
current P3 footprint exceeds the web and phone soft budgets.

## Captures

Captures are offscreen and private, in `/private/tmp/claude-501/medieval/`.
That directory is cleared on reboot, so copies of the later ones are in the
session's scratch directory. The P5 to P7 captures are `p5-stoop-*.png`,
`p6-*.png`, `p7-*.png`, `p7b-approach.png`, `p4-house/*.png` (a kit house
breaking), and `p3-grade-before.png` and `p3-grade-after.png`.

The requested web image `openagents-web:d3f3ad546c` was deployed in
`26bb7ef2ab`. Production now serves P3 revision
`coder-web-p3-d2fb95d33d-20261008140843`, with both current pack hashes and
older immutable URLs verified. See the October 8 P3 entry in
[Website deployment](../deployment/openagents-web.md).
Private browser receipts and inspected WebGPU framebuffer and WebGL2 views
are in
`/Users/christopherdavid/.openagents/scratch/codex-01a119ab-cb4c-7331-b0dc-8ddce4fb09a0/p3-web/production-browser/`.
The WebGPU view uses framebuffer readback because Linux headless
compositor screenshots are black on both preceding and new images; those
black screenshots are retained as rejected evidence. This check establishes
rendering and asset delivery, not the physical-device frame rate.
