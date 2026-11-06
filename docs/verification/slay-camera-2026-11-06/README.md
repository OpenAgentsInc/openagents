# Slay's authored handheld camera

This follow-up to [#12962](https://github.com/OpenAgentsInc/openagents/issues/12962)
imports the source TF0290 camera template. The issue remains open for its
cinematic rendering target and mobile memory/performance qualification.

## Source and conversion

The private SDK reader loads the shot and its camera template as data. It
exports all nine channels, their base values, the two property-scale channels,
and all 200 exact `OuterToInnerTransform` sample times. It does not play a
sequence, load a world, render in Unreal or save source packages. The
[SDK build](sdk-build-green.log) and [read](sdk-read.log) succeeded; the committed
reader is byte-identical to the built private plugin. No exported payload is
in this repository. [identity.json](identity.json) records its hash.

The template is additive from its authored base. Coax subtracts that base in
f64 before applying the source property scale and adding to the outer camera.
Only then does it narrow to the existing f32 camera metadata. The near −90°
rest rotation is not added as motion. The packer refuses a changed channel,
frame or scale profile instead of silently falling back to a static camera.
The exact SDK time mapping retains the source float time scale; it is not
rounded to an assumed rational phase.

The [pure Coax camera unit](../../../aethyr/tools/slay-camera.coax) contains
the conversion and coordinate-basis checks. JSON and filesystem effects stay
in the packer. The [independent oracle](../../../aethyr/tools/slay_camera_oracle.py)
uses SDK channels and a separate double-precision rotation-matrix construction.

## Results

| Check | Result |
| --- | --- |
| Old fixed-camera pack against the new oracle | [Fails](oracle-red.log), as expected |
| All 200 packed source cameras | [Pass](oracle-green.log): position error 0 m; largest basis-component error 3.10 × 10⁻⁷, below 2 × 10⁻⁶ |
| Pure camera math mutation | [33/33 killed](math-mutate.log), zero survivors |
| Complete-pack admission mutation | [20/20 killed](admit-mutate.log), zero survivors |
| Full Canyon page suite and dependencies | [464/464 pass](page-tests.log) |
| Wasm | [91,959 bytes](wasm.log), within the unchanged [96,000-byte budget](budget.log) |
| Source body and other metadata | [Unchanged](unchanged-source.json): all 27,000 body-local records and every non-camera metadata field |

The [test-first composition failure](red.log) and
[test-first new-digest refusal](admit-red.log) are retained. No equivalent
mutants are waived. The camera mutation count covers the pure math unit;
the independent native oracle covers the actual JSON-to-packed-camera path.
It does not claim mutation coverage of the packer's filesystem entry point.

The private bundle remains 794,477,568 bytes, now with SHA-256
`5e0fa638d37aacecbb9870d0a25a6b9a9f3e136e49eb849d5201063a8cfa47da`.
Admission also retains the exact, previously verified fixed-camera identity;
it accepts no arbitrary pack of that size. The versioned private alias
`valley-slay-handheld.pack` leaves the existing `valley-slay.pack` installation
in place. Use `/worlds/valley-canyon?pack=valley-slay-handheld` with the new build.

## Physical observation and remaining cost

Muted headless Chromium observed the actual new Wasm and private bundle on
November 6, 2026, approximately 07:21–07:24 Chicago time (CST). Adjacent source
frames differ; stepping back restores identical GPU pixels. Body, face and
camera advance together; ambient locomotion and the phone viewport pass.
The [source frame](shot1125.png) and [phone viewport](shot-phone.png) were
visually inspected. The phone viewport is a Mac observation, not phone hardware.

The [concurrent run](shot-concurrent-red.log), made while the Everglade drift
suite was running, failed the ordinary 10,000 ms recovery deadline. After
drift exited, the [isolated run](shot-isolated.log) recovered in **9,849 ms**,
with identical state and GPU pixels and exactly one pack request, despite
making the source unavailable after its first download. The deadline was
unchanged in both runs. These observations show little recovery headroom;
they do not qualify recovery under concurrent GPU pressure.

The existing 35-layer 4K texture profile still reserves about 3.13 GB of GPU
storage. A mobile/source-detail profile and real mobile hardware measurements
remain necessary. Source lighting, skin and eyes, strands, orb deformation,
flare, material-time phase, exact reference-frame selection and scenery still
prevent claiming cinematic parity. No production deploy or owner sign-off
was performed.

Reproduce the packer with private source directories:

```sh
coax/web/tool.sh aethyr/tools/slay-shot-pack.coax GLTF_DIR SDK_DIR OUTPUT_DIR CAMERA_SDK_DIR
python3 aethyr/tools/slay_camera_oracle.py SDK_DIR CAMERA_SDK_DIR/tf0290-camera-template.json OUTPUT_DIR/echo.shot
```
