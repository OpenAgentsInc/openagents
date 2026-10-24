# C/Wasm town experiment

This experimental slice compares C, packed Rust, pooled TypeScript, and the
current Rust/TypeScript runtime using the same Medieval Town and WebGPU renderer.
The [experiment review](../c-medieval-town-experiment-2026-10-23.md)
records the scope and interpretation. C owns movement, visibility, instance
compaction, and clustered-light preparation in its variant. The existing Rust
kernel still validates authoring commands. GPU programs and the editor stay shared.

## Current town and the frozen comparison

Current main includes a newer character controller and triangle walking surfaces.
Use `VITE_C_TOWN_BACKEND=c-frame` to keep that current Rust controller while C
prepares visibility, compacted instances, and clustered lights. The default
`baseline` backend keeps the current TypeScript preparation. Neither setting
replaces character movement with the historical C controller.

The four-way movement comparison below uses the frozen pre-character runtime at
`eac0437` (the earlier day timing run used `87ae58c`, before the capture-only fix).
Create a separate checkout at that revision to reproduce the full C player:

```sh
git worktree add ../aethyrforge-c-town-reproduction eac0437
cd ../aethyrforge-c-town-reproduction
```

Use the same private asset snapshot and quality settings for every candidate.
Current main rejects the earlier packed movement backends in the new character
worker; it does not silently substitute incompatible movement or ignore triangles.
The pinned commits remain ancestors of main. The full C movement experiment is
therefore runnable without reverting the newer town in the main checkout.

For independent controller conformance tests on current main, first build the
frozen Rust authority into its own artifact. This never replaces `kernel.wasm`:

```sh
node experiments/c-town/build-frozen-authority.mjs \
  --source-dir /absolute/private/scratch/new-frozen-authority-source
```

The source directory must be new, outside the checkout, with an existing parent.
The tool extracts only the pinned manifests, toolchain, and Rust kernel from
`ae4ac9a`, then writes `public/c-town-authority.wasm` and build evidence. The
simulation tests and CPU benchmark use this file, so rebuilding the current
product controller cannot change their comparison.

## Prepare the modules

Install the selected Rust toolchain, Node dependencies, LLVM Clang, and LLD.
The C build uses freestanding C17 and no downloaded SDK or Emscripten runtime.
Override `AF_C_CLANG` and `AF_C_WASM_LD` if the executables are elsewhere.

```sh
npm ci
npm run build:kernel
node tools/build-c-town.mjs --repeats 3
node experiments/c-town/build-frozen-authority.mjs --source-dir /absolute/private/scratch/new-frozen-authority-source
npx tsx --test tests/c-town-*.test.ts
```

The builder writes ignored files under `public/`: the C frame and simulation
modules, the matched Rust simulation control, and a C simulation candidate with
movement speed multiplied by 1.1. It records toolchain versions, flags, source
hashes, output hashes, and individual build times in `public/c-town-build.json`.
The candidate is for state-preserving code-reload verification, outside measured
frame intervals. It is not the default movement behavior.

The default executables on the measured Mac are
`/opt/homebrew/opt/llvm/bin/clang` and `/opt/homebrew/bin/wasm-ld`. The script runs
the linker explicitly. C and Rust use separate Wasm memories. The C frame arena
starts at 2 MiB and grows to 256 MiB; C movement reserves 16 MiB for bounded tables.
These are experimental capacities, not memory targets for the finished engine.

## Freeze the assets and browser builds

Copy the private prepared asset tree to a new scratch directory. Record a sorted
path, byte-length, and SHA-256 manifest before testing. Never publish that tree,
camera coordinates, captures, or raw browser traces. Use the same frozen copy for
every variant, and do not regenerate lighting during the comparison.

Build four production bundles from the same source revision. Give each variant
its own editor and preview ports. For example, build the baseline first:

```sh
export AF_EXPERIMENT_DIR="/absolute/private/scratch/c-town-experiment"
export AETHYRFORGE_PRIVATE_ASSETS="$AF_EXPERIMENT_DIR/assets"
VITE_C_TOWN_BACKEND=baseline VITE_C_TOWN_BENCHMARK=1 \
  VITE_EDITOR_ORIGIN=http://127.0.0.1:55210 \
  VITE_PREVIEW_URL=http://127.0.0.1:55211/preview.html \
  npx vite build --outDir "$AF_EXPERIMENT_DIR/build-baseline"
```

Repeat for `rust-packed`, `ts-packed`, and `c`, changing the backend, output
directory, and origin pair. The renderer and editor assets remain production
bundles; Vite development transformations are excluded from the frame comparison.
The C and packed controls still carry the authoring Rust kernel. Their complete
town download is not the size of the small experimental module alone.

Write a private server configuration. For example:

```json
[
  {
    "name": "baseline",
    "build": "/absolute/private/scratch/c-town-experiment/build-baseline",
    "assets": "/absolute/private/scratch/c-town-experiment/assets",
    "editorPort": 55210,
    "previewPort": 55211
  }
]
```

Add the other three pairs, then start one server process:

```sh
node experiments/c-town/serve.mjs --config "$AF_EXPERIMENT_DIR/servers.json"
```

The server binds loopback, restricts methods and origins, and checks resolved
paths before serving private files. Content-addressed resources use immutable
caching. Scene and control JSON use `no-store`; unhashed Wasm uses conditional
`no-cache`. Stop the process with SIGTERM when verification ends.

## Run the comparisons

Run CPU measurements before opening a GPU context:

```sh
npx tsx experiments/c-town/cpu-benchmark.ts \
  --assets "$AF_EXPERIMENT_DIR/assets" \
  --out "$AF_EXPERIMENT_DIR/cpu" --batches 15 --iterations 128
node experiments/c-town/build-benchmark.mjs "$AF_EXPERIMENT_DIR/builds"
```

The CPU tool checks actual day/night matrices, visibility, draw ranges, and light
storage against the current implementation across 64 recovered cameras per
scenario. It then measures per-call batch intervals. Simulation checks complete
C/Rust state and baseline player positions before timing. Its AABB stress profile
uses source bounds as synthetic blockers; it does not claim triangle collision
or the exact walkable-town collision mesh. The leaf-build tool changes a used
speed constant in private source copies, recompiles both languages, and records
five changing outputs each. It leaves the checkout untouched.

Set `AF_C_TOWN_URLS` to a JSON object mapping all four variant names to their
editor URLs. Run one isolated browser context at a time:

```sh
export AF_C_TOWN_URLS='{"baseline":"http://127.0.0.1:55210/","rust-packed":"http://127.0.0.1:55212/","ts-packed":"http://127.0.0.1:55216/","c":"http://127.0.0.1:55214/"}'
openagents browser run -- node experiments/c-town/benchmark.mjs \
  --out "$AF_EXPERIMENT_DIR/browser" --seconds 120 --warmup 10 \
  --trials 3 --route MT-09 --motion sway --trace false
```

The worker applies the same render-camera sway while simulation keeps stepping.
The harness rotates variant order between trials. It records fresh-context and
warm-navigation readiness, CPU frame intervals, available GPU timestamps, fixed
time images, and the worker's resource accounting. A timestamp for the main HDR
pass measures only that pass. CPU render submission time is not GPU execution
time. Browser readiness includes asynchronous uploads and setup; fresh contexts
do not flush operating-system file caches.

Use `--trace true` for a separate five-second diagnostic trace per variant,
excluded from reported timing intervals. Raw traces and screenshots stay in
private scratch. GPU queues are not forced idle for each live frame. Inspect
fixed-time captures and compare decoded pixels before claiming visual parity.

Create a public summary after reviewing the raw results:

```sh
node experiments/c-town/summarize.mjs \
  "$AF_EXPERIMENT_DIR/cpu/cpu-benchmark.json" \
  experiments/c-town/results/2026-10-23-cpu.json
node experiments/c-town/summarize.mjs \
  "$AF_EXPERIMENT_DIR/browser/benchmark.json" \
  experiments/c-town/results/2026-10-23-browser.json
```

The summarizer omits private paths, scene tables, camera poses, images, and traces.
It retains bounded distributions, scene/module hashes, counts, compiler evidence,
and resource estimates. Custom-section removal and gzip sizes are separate
payload analyses; the actual application still downloads its built modules.

## Shared engine baseline

The [representative suite](../../representative-benchmarks.md) extends these
tools with active 1k/10k/100k CPU populations, balanced instrumentation windows,
owned/Town routes, complete asset identities, and the common
[measurement contract](MEASUREMENT.md). Historical C results retain their pinned
source and controller; they are not relabeled as current-controller comparisons.
