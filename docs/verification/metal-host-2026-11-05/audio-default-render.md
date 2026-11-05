# Mixed default audio fixture

`metal-audio.coax --default --sdk SDK --producer PRODUCER` now records a CPU-only
render through the production native audio adapter. The SDK is the qualified
audio adoption at `802ceffdbc`; the unchanged frozen packet-capacity producer is
used only to expose its authored room WAV bank. No world GPU or audio device is
opened. The real browser AudioWorklet subsequently passes the same fixture
at the unchanged sample bound, as recorded below.

Immutable inputs and outputs live in
`~/.openagents/aethyrforge/metal-host-12868/audio-default-render/`:

- `plan.json` contains every exact command and render boundary.
- `audio.json` contains command results, seven actual input PCM identities,
  source WAV/Ogg paths, shapes and actual Coax observations.
- `inputs/` retains the actual adapter PCM and exact source bytes.
- `sources/` freezes the SDK observer and Coax fixture generator.
- `render-artifacts.sha256` binds the four outputs and executed SDK image.

Commands execute before the named 1,024-frame block. Rate is 48 kHz; all
outputs are interleaved stereo little-endian F32, with no alignment adjustment.

| Block | Effects before rendering | Output |
| --- | --- | --- |
| 0 | Room WAV, bus 0.75, ambient-e1 bed, picked SDK fire-cast-e2/fire-hit-e3 voices | `first.f32`, 4 blocks |
| 4 | Required companion frost-hit-e2/ice-e3 voices | `voices.f32`, 12 blocks |
| 16 | Bed gain/pan update, required companion splash-e1 | `bed-update.f32`, 16 blocks |
| 32 | Stop five IDs, silence bed | `stop-tail.f32`, 96 blocks |

All voices carry the eight mixer fields: name, gain, pan, delay, filter, rate,
wet and ID. The plan retains each numeric spelling. The fire-hit-e3 voice has
already completed by block 32, so stopping ID 1102 explicitly returns -1; the
other stops return 128. Actual final observations are 131,072 Coax frames,
66 room partitions, 14 accepted commands, one refused completed-ID stop,
three SDK decodes and three companion decodes. Callback/device frames stay zero.

The independent browser workstream consumes these actual source identities
and command schedule through the real OfflineAudioContext/AudioWorklet. All
262,144 samples pass the unchanged 2^-20 bound, with zero nonfinite values,
no shifts and no masks. Maximum errors for the four ranges are respectively
4.470348358154297e-8, 2.561137080192566e-8, 7.450580596923828e-9 and
3.725290298461914e-9. Actual callbacks total1,024, rendered/device frames
131,072, accepted commands14, refused1 and room partitions66. The source joins
and complete receipts are published at `573be05473` in the
[common default-audio record](../host-contract-2026-11-05/browser-opus.md).
The eleven synth/DSP fixtures retain their separate scope. Current primary,
graphics and default-host admission remain separate.
