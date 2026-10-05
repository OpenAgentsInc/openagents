# Cooked static content streaming

`traversal.json` records the final offline Lambert bake and a 1,200-frame,
1280×720 Vulkan traversal on an RTX 4080 (driver 595.71.05). The original dataset
has 48 terrain chunks and one shared image: 42,485,280 encoded bytes. Its compact
manifest, rendered PNG, source and executable fingerprints, and raw logs are
retained beside the report. Earlier runs precede the pixel assertion and lighting
bake; their filenames identify those stages. The run before the latest controller integration
is retained under `before-controller` names.

The final run reserves 4 MiB for CPU payloads, 3 MiB for GPU payloads, two source
jobs, 128 KiB of uploads per frame, and a 2 ms soft pump deadline. High water is
3,555,488 CPU bytes and 2,670,592 GPU bytes. It records 43 evictions, two stale
results, one refused oversized view, one real device recreation, and 303,925
textured terrain pixels. No pump exceeds 2 ms or upload exceeds 128 KiB. Steady
pump CPU p95 is 0.143 ms; encode/submit p95 is 0.499 ms. Total frame CPU p95 is
0.521 ms. The 99.790 ms device recreation remains visible in the total-frame
maximum of 100.013 ms. These are CPU observations, not GPU execution or display
latency measurements. The first 120 frames are retained separately as startup.

`partial-rows-rebuild.log` verifies four-byte upload steps across a 3×2 image,
both depth conventions, and identical pixels on a replacement GPU device after
source files have been deleted. Portable tests cover dependency sharing,
cancellation reservations, capacity refusal, malformed data, and stale tickets.

Reproduce with the pinned toolchain and a warm Cargo target directory:

```sh
export CARGO_TARGET_DIR="$HOME/work/openagents-target-agent1"
cargo build -p verse --no-default-features --example streaming_traversal
WGPU_BACKEND=vulkan "$CARGO_TARGET_DIR/debug/examples/streaming_traversal" /tmp/traversal.json 1200
cargo test -p verse -p verse-engine --no-default-features --lib
cargo test -p verse --no-default-features --lib \
  streaming::tests::gpu_partial_uploads_draw_and_rebuild_without_source_files \
  -- --ignored --nocapture
```

Use a temporary `HOME` and `XDG_RUNTIME_DIR` and unset display variables for
headless runs. The fixture opens no owner host, relay, engine, or credentials.
Its files are procedural original content. Work is paced at 60 Hz; it can show a
loading gap while the next view becomes resident. Managed payload accounting
excludes metadata, render targets, fixed shader resources, driver padding, and
readback copies, queue staging, and in-flight driver resource retirement. These
cache budgets do not bound driver backlog; V18 retains that work. Offline cooking
and source configuration precede the frame loop. A driver call or filesystem syscall already running cannot be
preempted. Static unlit/baked geometry and single-level images are the implemented
profile; animated packs, full physical materials, and phone/browser streaming
remain separate acceptance work.

`library-tests.log` records 565 Verse tests and 131 engine tests passing, with 13
GPU/platform tests ignored by default. The explicit GPU test above passes.
`consumers.log` checks the desktop and mobile Rust consumers; `web-check.log`
checks the browser build. The first broad checks found stale Everglade and Grid
packs. Their regeneration logs are retained; Everglade is repinned. The controller
rebase preserves main’s newer generated Grid pack. The subsequent Meteor Swarm
merge changes a Grid source fingerprint, so that pack is regenerated again. A desktop initializer also needed the newly introduced optional
presence name. No Clippy, release gate, live owner host, or phone suite ran.

The measurement precedes the later Meteor Swarm merge. Its receipt records a
public owner-source base commit and exact implementation file hashes; the local
implementation commit was rebased before publication. Focused streaming,
rendering, and Grid pack checks after that merge are retained separately. Recorded
frame measurements apply to their source revision. The measured fixture source
is retained because its manifest-writing expression is formatted before the push.
Changed Rust files pass formatting checks.
