# Animation Lab A1 verification

Issue [#12966](https://github.com/OpenAgentsInc/openagents/issues/12966).
The unlisted realm is `/worlds/animation-lab`. Shared playback and inspection
uploads are in `aethyr/anim/`; the stations are in `verse/animlab/`.

## Built

The rig station draws the skeleton over the skinned mesh, selects joints
and reports their names and parents, and colours vertices by the selected
joint's normalized weight. All eight influence slots are retained. Shape
selection and a zero-to-one slider drive the admitted sparse morphs. The
count lines report joints, influence slots, shapes, face bones and materials.

The clip station has play/pause, single-frame steps, a scrubbable timeline,
event ticks and plots for the selected joint's quaternion components or a
face curve. Plot samples are cached until the selection changes. Faded
skeleton ghosts show earlier/later poses at spacings 4, 8 or 16 frames.
The stage has orbit and zoom controls, keyboard controls and phone buttons.
Deep links select a station, rig and clip. A missing private bundle is named
on screen while the built-in mannequin remains usable.

The default character is an original block mesh on the existing CC0
65-joint mannequin skeleton, with an authored wave. The private source
bundle loads Echo and all 24 body and 20 Slay face clips through the same
`anim.rig.v1` / `anim.clip.v1` readers. Every source face clip was selected
and sampled by the headless check; the active face curve is plotted.

## Checks

- Station/shared playback suite: **242/242** checks.
- Bundle table authority: **66 killed, zero surviving mutants**.
- Exact renderer/memory adoption authority: **55 killed, zero survivors**.
- Headless harness/shared frame-report checks: **136/136**.
- Realm generator: **25 tests, 244 expectations, zero failures**; drift check passes.
- Generated shader drift and the no-DOM-UI check pass.
- No whole-repository gate was run.

The A0 document admission gates remain unchanged from `42920e800b` and
`71413e680e`; their 2,372 killed mutants are not rerun here.

The private bundle has 334 joints and 44 clips, **65,546,718 bytes**, SHA-256
`671ba58db50b48447423ccf56c5a9f90f3c20e920fe7f52c4abeb52243470a69`.
Payloads, source documents and images remain private. Only counts, hashes,
code, original made test data and test logs are committed.

The release Wasm is **231,906 bytes** (budget 290,000), SHA-256
`5bb795e4ffef2b77cc68f487a5d5105dec18e5c216945bfcb418e2826cfdbd68`.
The Coax-generated shader is
`e2b936aaaf2692a45702bae5488d84df7f8f0b54f2f1f9705284594e70115e04`.

## Fixed frames and recovery

Muted headless Chrome on this Mac's Apple Metal GPU renders both stations
on desktop (1280 × 800) and in a phone viewport (390 × 844). Two runs compare
all full GPU frame hashes exactly. Body station captures hold frame 18;
the face capture holds frame 12. The first face clip is also captured after
destroying and replacing the actual WebGPU device. Its pixels, selected
rig, clip and frame remain identical. All twenty face clips are sampled.
Expected telemetry from the deliberate device loss is counted separately;
there are no unexpected browser errors.

| View | Desktop SHA-256 | Phone viewport SHA-256 |
| --- | --- | --- |
| Mannequin rig | `9728fb63992e9fc3c2f48d79703833420d261392351f4b75d68d5a96eb0ef9fa` | `0a875fc8159000dfbc5cc6fbfa4aa8ce2c2832a7615f05d6b319222c77aeb377` |
| Mannequin clip | `43b667a10fcd181eaae1edb4a6f38a1e78c1a96cb1c874a3b8391144d99e1a9f` | `a47e897ac7c4c2c09a25f634ca003745d6125472e6b026f29a6ec99a28ab4639` |
| Echo rig | `03fada589d044ebe78db2263bbefd12caad6750e095ebdfca1e822a2e7846c2c` | `c9be3ac76982feed5209eacc62f85d67d5d9ffdebe17974de1c3b164b6116281` |
| Echo clip | `a7960994659338da38d63de4215c1ffbdbb8cb1a44f40ae59f62c3362c838ef3` | `aa064bbcee1d4ef10599ad0ba7d8656e76d3231cad7f6230f036fda419ba7de5` |
| Face, before and after GPU replacement | `03ff8420dc5dd56258d3a08efc46c701aca2168f33dce59ac072ddd9a63d5196` | `29e3209667b4f9ebdb003c22aca50213efb4fda0c424fd672d644e81819852a5` |

The browser ABI was also instantiated independently. A valid private bundle
was admitted, then malformed replacement bytes were written through its real
asset exports. The new input was refused, the active bank stayed unchanged,
and the retained bundle's complete SHA-256 remained equal to the original.
This verifies the two-bank storage lifetime across refusal, not only a copy
of parsed metadata.

## Budgets and limits

A frame measurement includes one Coax frame, GPU encoding and completion of
its submission. Each run measures 60 held-source frames after warm-up.
The budget is **33.3 ms** on desktop and phone. Exact timing and device
replacement durations are in the raw receipts alongside this record.

| Run | Median ms | p95 ms | GPU recovery ms |
| --- | ---: | ---: | ---: |
| Desktop first | 1.925 | 2.340 | 630 |
| Desktop second | 1.920 | 2.395 | 627 |
| Phone viewport first | 1.890 | 2.460 | 636 |
| Phone viewport second | 1.885 | 2.475 | 629 |

The inspector keeps a 64 MiB bundle admission limit, 64 clips per rig and a
64 MiB GPU mesh upload limit. Two source banks ensure a refused replacement
cannot overwrite admitted bytes. The browser and linker use the same
Coax-owned 256 MiB memory ceiling and 4 MiB stack for this inspector. Other
pages keep 64 MiB. Its renderer is explicitly tracked as an isolated
inspection renderer; this is not shared-renderer adoption qualification.

The material is neutral inspection shading; source textures and groom are
not included. Ghosts are skeletons to bound draw cost on smaller devices.
ALB1 currently packages one rig and one immutable binary with its clips;
these transport bounds do not narrow the underlying document formats.

The realm generator initially exposed an upstream metadata mismatch: Valley
World's page had five detail assets absent from its canonical table row.
The row now retains those existing assets, and regeneration changes only
line wrapping on that page. Valley's runtime and visible behavior are
unchanged by A1. #12962 remains open.

## Owner and deployment

Headless phone viewport results use the Mac GPU. A physical iPhone run is
still needed and is recorded, with screens and taps, in
[workspace NEEDS_OWNER.md](https://github.com/AtlantisPleb/workspace/blob/main/NEEDS_OWNER.md)
(commit `f04d645cb9`). It does not hold the implementation issue open.
No production deployment was performed.
