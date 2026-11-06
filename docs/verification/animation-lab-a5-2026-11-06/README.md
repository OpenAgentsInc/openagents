# Animation Lab A5 verification

Issue: [#12970](https://github.com/OpenAgentsInc/openagents/issues/12970).
Scope: admitted scene projects, Scene/Cut stations, the private Guardian
study, headless CLI films and JSON diagnostics. No production deployment.

## Checks

The qualified compiler is Coax 0.52, SHA-256
`23dfc150fa0d21dd199b2da382b9c2dd0762e8183e3f576657fe1341d327117e`.
The affected CLI was generated from `apps/openagents-cli/main.coax` and
linked with its existing native hosts, without building dictation or running
whole-repository gates. The resulting `openagents` binary ran the checks and
films below.

| Gate | Result |
| --- | --- |
| `project-policy.coax`, root mutation | 74 killed, 0 survived |
| `cut-playback.coax`, root mutation | 56 killed, 0 survived |
| `voice-source.coax`, root mutation | 94 killed, 0 survived |
| `verse/animlab/project.coax`, two merged root shards | 317 killed, 0 survived |
| `apps/openagents-cli/anim-args.coax`, root mutation | 160 killed, 0 survived |
| New/changed authority total | **701 killed, 0 survived** |
| Station suite | **392 checks passed** |
| Native play/helper suite | **357 checks passed** |
| Scene diagnostic suite | **316 checks passed** |
| Original native file-selection harness | Performance, scene, cut and project admitted with their expected durations/rates |
| Targeted generated Animation Lab WGSL check | Passed |
| `tools-check` | 1,843 tracked tool files; 0 new files without a reason |

The staged tests cover malformed component returns, previous-bank retention,
duplicate names, second-library resolution, two speech/WAV spans, the last
performance/voice slots, draw limits, complete canonical audio, and explicit
canyon/crossfade refusal. A separate station test and original headless GPU
probe cover inspection-to-film geometry invalidation.

The final Wasm is **348,092 bytes**, below its 420,000-byte budget. Its stack
is 8 MiB; maximum imported memory remains 256 MiB. Source and geometry
windows remain 64 MiB. Engine SHA-256:
`d4ab9f4fe1cab89a44f8f21f868467c4f2feefba5106cb55d394942e12194a40`.
Shader SHA-256:
`1871a76b794c3e3a738cffb79cd9e1ab20a729e3c02d4cdb0bc319e995366b66`.

## Guardian film

The source is the qualified private A0 Valley walk, retargeted through A2's
53 paired anchors to the original 65-joint mannequin. The study generator
creates 901 samples at 30 Hz over 30 seconds, original arm/recoil keys,
three source-derived foot contacts, a hand-bound original wand, a jaw morph
and a separate original prop rig. The hero has nine performance layers;
the two hooded stand-ins have an original recoil performance. The bird is
procedural Coax geometry. Eight speech-aligned visemes drive the stand-in
mouth; the offline stock voice remains private.

Both films were rendered by:

```sh
OPENAGENTS_ANIM_ORIGIN=http://127.0.0.1:8945 openagents labs anim play PRIVATE_DIR/project.json PRIVATE_DIR/guardian-releaseN.mp4
```

Chrome 157.0.8103.39 ran headless and muted with WebGPU/Metal on the local
Apple Silicon machine. Each film has **720 frames at 24 fps, 960 × 540**.
Both H.264 and AAC streams have a **30.000-second** duration. Every frame
hash matches; all 720 hashes are distinct. Neither run reported a browser
or GPU error.

| Measurement | Run 1 | Run 2 |
| --- | --- | --- |
| Render wall time | 48.753 s | 48.639 s |
| Total including mix/encode | 49.450 s | 50.188 s |
| Render seconds per film second | 1.6251 | 1.6213 |
| Capture median | 24.680 ms | 25.560 ms |
| Capture p95 | 25.960 ms | 30.065 ms |

Render wall time includes browser startup, asset loading, GPU readback and
PNG writes. Capture timing covers held-frame stepping/readback, hashing and
presentation; it excludes CDP PNG encoding and file writes. These are local
measurements, not a cross-device frame-rate guarantee.

Project SHA-256:
`245eceb2d713afc607b4bbf92119e0bfa1c8870156aa2636b0a26d6ac61e92d5`.
SHA-256 of the newline-separated 720 RGBA hashes, with a trailing newline:
`ee2d3c3045e28685c978ecea357a36ca242dc56b0a453ffbbbbc20def3252d4a`.
Both complete MP4s also have the same SHA-256:
`757b303ee54828a0c6b9949132c6d328871fb7c43a84cc6b1b711c6ed6cdf967`.

| Shot start | Frame | RGBA SHA-256 |
| --- | --- | --- |
| S1 | 0 | `1da36400f9d883ab528db8f0c31fa301233c550c7003915ff49e34bd5a0a17ff` |
| S2 | 120 | `677eee2e39be6ebb1bff495a64afdc12a94787f0fc825ca69e0d6a30b770a984` |
| S3 | 216 | `b4859754de3cbb5e26b311c7ea25fe5684be20af6ef660a09c932f9d043f63fb` |
| S4 | 288 | `1cc3c6c01917e824c4cff8bd897ec5e8845121cbd08d2f7ab2cda017c97f3a5f` |
| S5 | 432 | `ed61ca945d6f8e15a8f39a1f468f59341f643c3d4eeb709b6c6335a36b45215d` |
| S6 | 600 | `ac9137d41a5f6e1fb274762bbeb5f976413358b561d3d7105477afced6aa4492` |

The mixed WAV is stereo PCM16, 48,000 Hz, 1,440,000 frames. Nonzero audio
runs from 9.4000 to 10.37721 seconds, inside the authored voice cue. Its peak
absolute PCM value is 22,215. Audio is mixed from edit time and muxed after
Verse capture.

## Numerical diagnostics

`openagents labs anim check PRIVATE_DIR/project.json` emitted one JSON
object, SHA-256:
`1fa93169a4066516bda787b505d26bf9fe3e1336d99d3eeb369d82ca698b3a9e`.

| Check | Result |
| --- | --- |
| S1 maximum cumulative planted-foot travel | **0.0000537110 cm**, pass below 1 cm |
| S2 maximum cumulative planted-foot travel | **0.0092006363 cm**, pass below 1 cm |
| Every cut, S1→S2 through S5→S6 | **0° / 0 cm pose jump**; camera eye/look jumps reported as authored cuts |
| Raw/composite face range violations | **0 / 0**; no source facial curves in this stand-in |
| Canonical elbow/knee screening violations | **0** |
| Character capsule pair overlaps | **0** |
| Repeated CPU palette/face bits | **Identical** |
| Hero head framing | **901/901 samples inside the active camera** |

The report states its models: 170° canonical elbow/knee screening, and
upright character-root capsules of radius 0.3 m and height 1.8 m. It does
not claim authored joint limits, self-collision or prop collision shapes.
Foot contacts use 120 Hz; pose/face/capsule/framing samples use 30 Hz.

All complete reports, hash lists, WAVs, PNGs and MP4s remain under
`~/.openagents/aethyrforge/animation-lab/a5-scenes/`. No licensed payload or
capture is committed or sent for image/model analysis. The original fixture
captures contain only geometry made in this repository.

## Limits and remaining work

The [A5 profile](../../verse/anim-scenes-v1.md) records the implementation
choices: studio/courtyard, hard cuts and incoming fades, at most 60 seconds
per capture. Canyon and image crossfades are refused. Weather/time and focus
remain metadata; non-guardian effect kinds use markers. Live seek is silent;
offline audio samples the correct cue offset. The study uses simple original
bodies and a mouth stand-in. A6 owns finished shading/depth of field, and A8
owns original motion/face/voice captures before publication.

A5 requires no owner-only step. It changes no Valley Lab file or visible
behavior. #12962 stays open for exact rendering; #12983 stays open for the
separate A4 re-import qualification. Work stops after A5.
