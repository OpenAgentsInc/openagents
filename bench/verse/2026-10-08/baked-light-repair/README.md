# Baked-light repair evidence

This folder retains the evidence for
[#10907](https://github.com/OpenAgentsInc/openagents/issues/10907). The full
clock and blend measurements complete on continuation source
`f655bcc32c6df5a69ef278ba8a2e45e342178fab`, but its rebuild fails. The separate
clear destruction capture completes on source
`290c1590726189dbdabb5693125cfa0fbca8a779` under an explicit 600-second
diagnostic bound. The blend review accepts negligible cost under the
retained capture conditions. A separate particle-free inspection exposes
the ground, interior, and rubble for scoped visual acceptance. Each
component retains its own verbatim report and identity.

Source `9967c94cef19031ff91eac6d8cd222ab7c3024eb` builds scene
`f55a1e76fca2e2b96af777e296549aa9b51776cadbcfff6eaf1c77a8ad83b2ab` from the
current town and production kit `c5599554…`. The new CPU sidecar has SHA-256
`fc5414a1bfef9e730f3d7d779e4447f12cc86d4e571042eec42518abb30ef7c2` and
51,684,139 bytes. The historical, clock, and blend captures select it as a
verified local override through `VERSE_KIT_BAKE` with
`VERSE_KIT_UNPINNED=1`; those runs do not verify a published pin. The clear
destruction run enforces the updated published pins on the selected local
files. The licensed kit and VLAY remain outside this folder;
`blend-inputs.json` records their paths, sizes, and digests.

The published-sidecar preflight failure remains retained. That sidecar
expects scene `a673374a…`; the current town produces `f55a1e76…`, despite an
identical 4,326,184-vertex count. Commit `7afd440fe81caaeb1609fbe857c8d5f15c4437a6`
changes fountain placement order in `layout/city.rs`: previously, the town
excludes the plaza fountain from the instance table, then appends
`kit/fountain`; the current town extends every instance in table order. This
changes first-encounter mesh, material, image, and merged-vertex ordering,
which changes the scene digest. A cross-platform floating-point cause has
not been established.

The first full capture fails its selective repair hold after 3,600 frames
and 47.915 seconds: 203,297 of 618,035 targets are processed, with 414,738
still queued, no worker error, and no skipped targets. That frame cutoff
permits fewer polls than the 4,829 ideal 128-target batches required. The
180-second wall limit remains unexhausted. `historical-full-capture/` retains
the exact failure diagnostics, 120 progress records, commands, inputs, logs,
and leases, plus pristine, aftermath, and four clock-phase originals.
`historical-screenshot-inventory.json` records the path, size, and digest of
all 67 original screenshots in durable scratch. This run writes its raw clock
and paired records only after destruction, so those records are unavailable
after the failure; none are reconstructed.

The corrected hold on source `c08730c964a5bae38d3eaa6818213717202a35c7` also
fails repair completion. It processes 491,660 of 618,035 targets after
10,221 hold frames and 180.006 seconds, with 126,375 still queued and no
worker errors, skipped targets, or rejected patches. It applies both static
and chunk vertices but completes no generation. `historical-wall-capture/`
retains the checkpointed 10,441 clock records and 128 paired samples, all
repair progress, and selected original pixels. Its screenshot inventory
keeps every original scratch path, size, and digest. Neither failed run
establishes repair acceptance.

The continuation full run also fails, with `Everglade has no offensive
spells` during rebuild. `historical-continuation-full/` retains its exact
failed report, log, manifest, GPU lease, inputs, and preflight. The report
retains all 10,441 completed clock records and truthfully skips blend timing.
It does not retain the noon/night repair counters or hold progress before
the rebuild error; those missing records are not reconstructed from images
or their timestamps. Three occluded destruction originals remain retained,
and `historical-continuation-screenshot-inventory.json` records all 69
original screenshots in durable scratch.

That executable enables `capture` without `dev-destruction`, so its rebuild
gate is disabled. This is a capture configuration failure; the missing raw
repair counters prevent a diagnostic-completion claim. The corrected
destruction harness enables `capture,dev-destruction`, activates the runtime
development controls before installation, and checkpoints repair diagnostics
before restore. It also raises only the camera eye's Y coordinate to 28 m,
keeping its X/Z position, player ground origin, meteor origin, and target
selection unchanged.

The matched CPU bake uses 12 threads, 128 vertex rays, 256 probe rays, two
bounces, four sun rays, and seed 1,592,593,228. Its receipt records the scene,
settings, output digest, and timing. `matched-bake-command.json` and its
build lease retain the exact invocation. The earlier published preflight
uses the capture executable with `--preflight-only`; `matched-bake.py`
retains that command and its environment before regeneration.

The continuation uses separate raw reports for 128 paired blend samples and
10,441 clock records at 1920×1080 with High quality and TAA disabled. The
completed `blend-capture.json` and failed full report retain their
own commands and source/binary identities; no report rows are fabricated or
merged. Named clock phases warm the
scheduled sky bake before saving pixels. The 24-hour timeline renders every
step and extracts pixels only for selected frames. The paired comparison
uses fresh independent renderers with the identical bake replayed into both,
a fixed 09:00 scene, balanced alternating order, and every measured sample.
Only the four immutable sun weights differ. The measured interval includes
CPU fitting, encoding, submission, serial completion, mapping, and pixel
extraction; it excludes simulation and PNG writing. It does not measure the
selective repair worker's cost. Wall completion is not GPU duration. The
completed blend mean increment is −0.010874 ms, with an approximate 95% block
interval of [−0.278931, +0.257183] ms across all 128 pairs. The blend runs
under quiet and GPU leases. The full clock/repair run uses a GPU lease and
makes no timing claim.
The root review accepts the negligible blend increment from all 128 pairs
and the +0.257183 ms upper interval bound. This verdict covers the measured
serial wall interval; it does not measure GPU duration or selective-worker
cost.

`clock-records.jsonl` retains every clock row from that failed full run.
`clock/` retains the 12 named phase and boundary originals. The
[phase contact sheet](clock/phase-contact-sheet.png) places dawn, noon,
dusk, and night at their original 1920×1080 resolution. The four
`sun-*-boundary-crops.png` boards preserve the same 800×600 crop without
rescaling; `clock-review.json` records source hashes and crop coordinates.
Before and after images exist at every boundary. Exact-boundary pixels are
unavailable at 08:00, 15:30, and 17:30, so those boards label the gap.
The 12:00 center uses the separately warmed named noon capture.
The root review observes gradual changes in the retained before/after
images and recorded sun weights. The missing exact-boundary pixels remain
explicit.

[Clock preview](clock-preview.mp4) is a labeled lossy 960×540 H.264 preview
of the 50 saved timeline stills in numeric order, played at six stills per
second. It invents no intermediate simulation frames. `clock-preview.json`
retains the exact encoding command, input hashes, and output identity;
the original pixels and all 10,441 metadata rows remain the review evidence.
Run `python3 check.py --completed-clock` to verify this completed component
without claiming that the failed containing run completes repair or restore.

The clear destruction phase advances 960 frames at pinned noon. Bounded holds
poll production repair batches with zero simulation time, then retain the
completed geometry repair, a midnight clock repair on the same geometry,
and R restoration before and after polling. The continuation explicitly
permits a 600-second diagnostic hold; the default remains 180 seconds. Any
eventual completion records its full hold latency and does not establish
completion within the default bound. The capture records worker
errors, rejected patches, target counts, backlog, generations, and applied
static and chunk vertices. The noon hold processes and applies all 618,035
targets in 13,954 frames and 227.121 seconds. The night hold processes and
applies the same 618,035 targets in 25,480 frames and 378.710 seconds. Both
finish with zero backlog, skipped targets, worker errors, or rejected chunk
vertices. These latencies exceed the unchanged 180-second default; this
evidence establishes completion under the explicit diagnostic bound.
`clear-capture/` retains both full hold histories, the checkpoint before R,
the final repair verification, all eight original destruction stills, and
the exact command, inputs, log, and GPU lease. `repair-diagnostics.jsonl`
retains all 1,324 nested repair records. The run has no quiet lease and makes
no performance or GPU-duration claim. These diagnostic checks do not
establish visual quality by themselves.

The historical continuation destruction camera sits inside foreground foliage. Leaves cover
most of the targeted house and rubble in the pristine and frame-900 views,
so those images cannot establish the absence of floating shadows or verify
restored lighting. The clear camera uses eye `[-30, 28, 35]`, aim
`[0, 4, 6]`, and player ground origin `[-30, 0, 35]`. It exposes the target
house and several separated pieces. Smoke masks part of the rubble in
these 290 originals; the receiver inspection below exposes more contacts.
The retained
night image darkens the rubble, and R restores the intact house at noon;
those observations do not establish pristine/R pixel identity.
[Clear-camera crops](clear-camera-crops.png) compare native 720×720 regions
from pristine, repaired noon, repaired night, and R originals without
rescaling. `clear-camera-crops.json` records the source hashes and coordinates.

The supplementary receiver inspection completes on source
`51e1ab25192e95038edcee1f548355c11df5ff95` with its own executable and GPU
lease, without a quiet lease or timing claim.
It advances the same 960-frame scenario at High quality with the raised
camera and pinned noon, then renders copied meshes with only sprites,
ribbons, and glow removed. Geometry, chunks, lamps, lighting, and simulation
remain in the copies. The pristine, frame-900, and R views can expose ground
contacts through the smoke. This mode skips the long selective holds and
blend measurements and records `verified=false`; it does not establish
worker convergence. `receiver-inspection/` retains all nine original stills,
the verbatim report and checkpoints, inputs, preflight, runner, manifest,
log, and GPU lease. Its nine diagnostic records remain in
`receiver-inspection-diagnostics.jsonl`. These artifacts remain separate
from the completed 290 repair proof.

The same-state frame-900 copy removes 971 sprites and retains 511 rigid
instances with an identical lighting stage. Its dynamic mesh has zero lit
vertices; the uploaded static scene remains in the renderer. The root
review finds no disconnected old wall or roof shadow in the exposed ground,
interior, and rubble. Pristine and R views restore the house geometry.
Vegetation still occludes some areas, and color differences reflect
continuing weather and exposure. This verdict does not establish
pristine/R pixel identity or worker convergence in the inspection run.
[Receiver crops](receiver-camera-crops.png) preserve four native 720×720
regions. `receiver-camera-crops.json` records their source hashes and
coordinates; the two frame-900 panels share one simulation state.

The integrated CPU log passes 190 physics tests, 197 PBR tests, 313 Everglade
tests, and two capture-example tests. The log also retains private-render
and documentation checks. The supplementary native light-upload tests pass
on source `3e6406782163e2fe01e9beeef933dd1a39216169`; their identity and leases
remain separate from the full capture's source. Final-source native and CPU
checks remain historical evidence.

Continuation source `f655bcc32c` passes 190 physics tests, 198 PBR tests,
318 Everglade tests, 14 render tests, 10 GLES tests, four capture-example
tests, and the debug capture build. Six functional native tests cover motion,
reactive history, static light patches, and rigid light patches. Their exact
commands, executable digest, and GPU lease remain in
`current-native-manifest.json`. The current scene preflight verifies the
same `f55a1e76…` local sidecar identity.

The published integration check on source
`97e8b2b0937d9641b6c8760b2e781579c5ba83a8` passes 200 PBR tests,
318 Everglade tests, and three private-render tests. Source `290c159072…`
passes all 10 GLES translation tests. `published-check-commands.json`
records the exact Cargo arguments and source identities beside their build
logs and leases. The earlier wrong-package GLES invocation matches zero
tests and is not counted.

The two additional native light-upload tests pass on source
`340df670f658276c6074d33ec80ccdfc66dac7c3`. The PBR source tree is identical
between that commit and the clear capture's `290c159072…` commit; the only
changed file is the capture harness. `published-native-manifest.json`
retains all eight dispatched tests, while this folder retains the two light
logs and the GPU lease. The other six tests have separate temporal evidence;
native tests do not establish TAA visual acceptance.

The intermediate portable fallback checks on source
`eb8c50bd50ac5a25577251a881949a1d2ddf1f4c` pass 319 Everglade tests and the
`wasm32-unknown-unknown` library check. Their logs, exact Cargo arguments,
and build leases remain retained. The later browser lazy-start guard also
passes the Wasm library check on source
`51e1ab25192e95038edcee1f548355c11df5ff95`. That check proves compilation;
it does not exercise a browser runtime. The same source passes all four
capture-example tests. These portable changes do not relabel the completed
native destruction capture.

Run `python3 check.py` to verify the four separate components' input
identities, scene preflight, all paired and clock records, diagnostic
completion and restoration, PNG integrity, lease coverage, and the final
hash inventory. Add `--raw-root PATH` to compare retained copies with their
scratch sources or `--input-files` to hash the licensed local inputs without
copying them. The checker makes no visual or negligible-cost verdict.

The separate source `290c1590726189dbdabb5693125cfa0fbca8a779` preflight
admits the same local kit and VLAY with `VERSE_KIT_UNPINNED` removed. This
enforces the updated published SHA pins against the selected local files.
It does not exercise automatic download or cache resolution. Its manifest,
preflight, and inputs remain in the `published-pin-*` files; it does not
relabel the earlier unpinned captures.

Run `python3 check.py --recorded-failures` to verify the three earlier
failures and their current hash inventory. This mode reports historical
failed repair acceptance explicitly. `SHA256SUMS` covers every retained
file except itself.

Run `python3 check.py --completed-blend` to verify the completed 128-pair
measurement, its lease coverage, and the six continuation functional native
tests. Run `python3 check.py --completed-destruction` to verify the clear
destruction component separately. The checker makes no visual-quality or
negligible-cost verdict; those verdicts are recorded separately from the
artifact checks.
Run `python3 check.py --receiver-inspection` to verify the supplementary
inspection's same-state pairing and skipped holds separately.
