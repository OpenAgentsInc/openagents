# Aethyrforge import provenance

Date: 2026-10-24. Issue: #12586, part of the Kiki rebuild epic (#12585). Plan:
[the Aethyr merge](merge.md#where-things-go).

## Source

- Repository: `OpenAgentsInc/aethyrforge`, branch `main`.
- Commit: `71c19bfe8b5de79811de5ca518e783876650d42f` ("Record native render lab
  results on an RTX 4080 and an M5 Max", 2026-10-24).
- Method: files were copied from `git archive` of that commit, not from a
  working tree. History was not imported; how history moves is an open owner
  decision ([merge plan](merge.md#decisions-for-the-owner), item 1).

The first copy was taken at `2b1ccb7ef5599138464cff1bd6407f074fa65971` (the
commit the [consolidation audit](imported/2026-10-24-consolidation-audit.md)
inspected). The 13 files changed by the three later commits (`22e749c`,
`370d6f5`, `71c19bf`: the native `webgpu.h` backend for `af-gpu`, the native
render lab, `tools/build-webgpu-native.sh`, `docs/research/render-lab.md`, and
`Cargo.lock`) were then synced, so every copied file matches `71c19bf`.

### Later syncs

- 2026-10-24, #12594: `f42c02cfad4d4df99f6269b4245f86200fd784b6` ("Optimize
  million-fighter rendering and enemy searches"), the newest Aethyrforge
  commit at the time; no later commit touches battle code. Its diff from
  `71c19bf` was applied with the path map below: 12 files under
  `apps/aethyr/` (renderer, tests, tools) and `docs/aethyr/engine/battle-crowd.md`.
  `battle-crowd.ts` and `renderer.ts` carried the Detail changes from #12589,
  and the diff applied over them without conflicts. Per the owner's decision,
  `tools/assets/compile-battle-impostors.mjs` came in alone, as
  `apps/aethyr/tools/assets/compile-battle-impostors.mjs`; the rest of
  `tools/assets/` stays out. Licensed sources, baked atlases, and captures
  stay out of Git.

  Changes beyond the source commit:
  - The baker's guard against writing atlases inside the checkout now
    resolves the openagents checkout root (four levels up) instead of the
    script's grandparent folder, which is `apps/aethyr` here.
  - The silhouette threshold follows Detail (ADR 0007): the
    `crowd_tier_distance` knob sets `RendererDetail.crowdSilhouettePixels`
    from 8 px at full to 16 px at the knob's floor, never above 16 px, and
    the renderer passes it to `BattleCrowd.setSilhouettePixels`. The shader
    reads it from the battle frame's unused `sun.w` instead of the constant
    `8.0`. At `Detail` 1 the behavior matches the source commit.

## Where things went

| Aethyrforge path | openagents path |
| --- | --- |
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` | `crates/aethyr/` (its own workspace, Rust 1.98.1 with `wasm32-unknown-unknown`; excluded from the root workspace) |
| `crates/{kernel,gpu,render-lab,mmorpg-core,server}` | `crates/aethyr/{kernel,gpu,render-lab,mmorpg-core,server}` (crate names unchanged: `af-kernel`, `af-gpu`, `af-render-lab`, `af-mmorpg-core`, `af-world-server`) |
| `packages/`, `apps/`, `tests/`, `public/`, `third_party/`, root HTML entries, `vite.config.ts`, `tsconfig.json`, `postcss.config.mjs`, `package.json`, `package-lock.json`, `LICENSE.shadcn`, `.gitignore` | `apps/aethyr/` |
| `README.md` | `apps/aethyr/README.md` |
| `tools/*.mjs`, `tools/*.ts`, `tools/*.sh`, `tools/film/`, `tools/fixtures/`, `tools/check_spec.py`, `tools/host-fixture.py`, `tools/compositor-*` | `apps/aethyr/tools/` |
| `tools/deploy/` | `scripts/deploy/aethyr/` |
| The TypeScript modules of `experiments/c-town/` that engine code, tools, or tests import (`config.ts`, `metrics.ts`, `simulation.ts`, `frame-core.ts`, `packed-frame.ts`, `resident-frame.ts`, `gpu-timer.ts`, `active-entities.ts`, `measurement.*`) | `apps/aethyr/experiments/c-town/` (same relative imports, so bundles are unchanged) |
| `experiments/kaigen/player-budgets.json` | `apps/aethyr/experiments/kaigen/` (read by `tools/build-player.ts`) |
| `docs/*.md`, `docs/requirements.json`, `docs/references/`, `docs/media/`, `docs/research/` | `docs/aethyr/engine/` |
| `docs/decisions/` | `docs/aethyr/engine/decisions-history/`, each with a deprecation note pointing at [`docs/adr/`](../adr/README.md) |
| `INVARIANTS.md`, `contracts/`, `examples/` | `docs/aethyr/engine/` |
| `experiments/c-town/README.md`, `MEASUREMENT.md`, `results/` | `docs/aethyr/engine/research/c-town/` |
| `experiments/boids/results/` | `docs/aethyr/engine/research/data/boids/` |

That is 829 files: 671 in `apps/aethyr`, 56 in `crates/aethyr`, 98 in
`docs/aethyr/engine`, and 4 in `scripts/deploy/aethyr`. No licensed asset,
credential, or `cakbags` content is included; a scan for key patterns and
home-directory paths found none.

Relative Markdown links were rewritten for the new layout. Links to files that
were not copied point at permalinks in Aethyrforge at `71c19bf`, except the
tweet digest, which is not linked. Paths written in prose and code spans inside
the engine documents still use Aethyrforge's layout (for example
`crates/kernel` means `crates/aethyr/kernel`, and `tools/` means
`apps/aethyr/tools/`). Two documents already imported unchanged,
`docs/audits/2026-10-24-consolidate-into-openagents.md` and
`docs/research/2026-10-24-gpu-native.md`, were not copied again; links to them
point at [`imported/`](imported/).

## Left out

| Aethyrforge path | Why |
| --- | --- |
| `experiments/c-town/c/`, `experiments/c-town/rust-control/`, `tools/build-c-town.mjs`, and the C-only harness (`benchmark.*`, `build-benchmark.mjs`, `build-frozen-authority.mjs`, `cpu-benchmark.ts`, `serve.*`, `summarize.*`, `verify-browser.*`) | C experiment code, dropped; its results are kept as documentation |
| `tests/c-town-{frame,kernel,simulation,benchmark,browser-verifier,server,summary}.test.ts` | Test the dropped C experiment |
| `tools/assets/` (114 files) | Python and TypeScript private-asset pipeline; owner decision pending (merge plan item 4) |
| `tests/{battle-cooker,gi-bounce-cli,ibl-cli,link-display,link-fog-ambient,link-lighting,native-material-batches}.test.ts` | Test `tools/assets` |
| `tools/reference/` (27 files, including its measured fixtures) | Unreal-dependent tools; owner decision pending (merge plan item 10) |
| `tests/fog-ambient.test.ts`, `tests/fog-ambient-fixtures.ts` | Read `tools/reference/fixtures` |
| `tools/archive_x.py`, `tools/test_archive_x.py`, `docs/research/x-archive-tool.md` | X post archiver; research tooling, not engine code |
| `docs/research/gdechichi-tweets-2026.md` | Third-party post digest |
| `experiments/kaigen/results/` | Raw run JSON with absolute home-directory paths; the summary is kept in `research/kaigen-analysis-2026-10-23.md` |
| `AGENTS.md` | Superseded by this repository's `AGENTS.md` |

## Changes made while copying

Each change is a path fix, a test fix for the new location, or wording that
stopped being true. None changes runtime behavior.

- `crates/aethyr/Cargo.toml`: members listed by name instead of `crates/*`.
  The root `Cargo.toml` excludes `crates/aethyr`.
- `apps/aethyr/tools/build-kernel.sh`, `build-render-lab.sh`, `build-server.sh`,
  and `build-webgpu-native.sh`: run Cargo in `crates/aethyr` (so its
  `rust-toolchain.toml` applies), write `kernel.wasm` to
  `apps/aethyr/public/`, and default the Cargo target and the wgpu-native
  install to folders beside the openagents checkout (`../aethyr-target`,
  `../webgpu-native`) instead of inside it.
- `apps/aethyr/package.json`: `build:kernel` runs the script with `bash`; it
  used `sh`, which fails on Linux, where `sh` is `dash`.
- `apps/aethyr/tools/check_spec.py`: checks `docs/aethyr/engine` and the
  imported code folders from the repository root. It now passes; at the
  source commit it failed on a broken link (`measurement.md`, now
  `MEASUREMENT.md`).
- `apps/aethyr/tools/benchmark-runtime.mjs`, `battle-evidence.mjs`, and
  `verify-material-editor.mjs`: source-pin paths for the crates, toolchain, and
  deploy scripts.
- `apps/aethyr/tests/runtime-modularity.test.ts`: matches module paths relative
  to the package, because the package itself now lives under `apps/`.
- `apps/aethyr/tests/color-temperature.test.ts`: skips the one test that reads a
  `tools/reference` fixture when the fixture is absent.
- `scripts/deploy/aethyr/render.py`: finds `apps/aethyr`, and the generated
  `NOTICE.txt` no longer calls the engine proprietary.
- Wording: `scripts/deploy/aethyr/README.md` (private repository and revision),
  `docs/aethyr/engine/openagents-host.md` (proprietary editor, private
  repository), and `docs/aethyr/engine/integration.md` (license gate in an
  empty repository; the license is now the open owner decision).

## Verification

Environment: Linux x86_64, Node 22.22.0, npm 10.9.4, Rust 1.98.1 from
`rust-toolchain.toml`, Binaryen 132 (the release binary, reporting its version
as `wasm-opt version 132` as the build script requires),
`CARGO_INCREMENTAL=0`.

| Check | Result |
| --- | --- |
| `npm ci` in `apps/aethyr` | Pass |
| `npm run check` | Pass |
| `npm test` | 1,321 tests: 1,318 pass, 0 fail, 3 skipped. Aethyrforge at the source commit on the same machine: 1,381 tests, 8 fail (all in the dropped C tests: no Homebrew `clang`, no C Wasm builds), 15 skipped. The 60 fewer tests are the left-out test files |
| `npm run build` | Pass |
| `python3 apps/aethyr/tools/check_spec.py` | Pass |
| `cargo test --locked --workspace` in `crates/aethyr` (default features; the `native` GPU feature needs wgpu-native and was not built) | Pass: 143 tests pass, 1 ignored (`af-kernel` 114, `af-mmorpg-core` 16, `af-render-lab` 10, `af-world-server` 3) |
| `cargo metadata --no-deps` at the repository root | Pass |

### Build comparison

The same source commit was built in a copy of Aethyrforge outside this
repository, with the same toolchain and Cargo target folder. Of 64 compared
files (`public/kernel.wasm` and every file in `dist/`), 56 are identical. All
JavaScript is byte-identical; the differences are:

| File | Aethyrforge `71c19bf` | openagents | Cause |
| --- | --- | --- | --- |
| `kernel.wasm` (and `dist/kernel.wasm`) | `581a3b43981766990a03ce7dd98b15c03170517905775d78178beaa23c1b86c8` (910,996 bytes) | `287e197bb1bcbaedf68852e5a3b077038c851effd3eff217e39a533f4ce06d66` (910,940 bytes) | Eight panic-location strings hold workspace-relative source paths: `crates/kernel/src/*.rs` and `crates/mmorpg-core/src/scenery.rs` became `kernel/src/*.rs` and `mmorpg-core/src/scenery.rs` (7 bytes shorter each). Rebuilding from this repository with `--remap-path-prefix=kernel/=crates/kernel/ --remap-path-prefix=mmorpg-core/=crates/mmorpg-core/` reproduces `581a3b43…` exactly, so nothing else differs. The default build keeps the true paths |
| `assets/editor-*.css` | `dcb61a99…` | `2dc9939d…` | Tailwind scans the build's working folder for class names. In Aethyrforge that included all documentation and `tools/assets`, which matched six unused utilities (`invisible!`, `shrink`, `text-input`, `lowercase`, `antialiased`, `shadow/reflection`) in prose and Python. The new CSS is the old CSS minus those six rules; no imported source uses them |
| `assets/universe-preview-*.css` | `31cb35e4…` | `0b9da362…` | Same six rules |
| `assets/editor-*.js`, `assets/universe-preview-*.js` | Same content (`e7100167…`, `f0e84446…`) | Same content | Only the file names changed, because they name the changed CSS |
| `index.html`, `universe-preview.html` | `3da85879…`, `2b227836…` | `d8f0b2d0…`, `2fb82c8a…` | Only the changed asset file names |

## Open follow-ups

- Owner decisions from the [merge plan](merge.md#decisions-for-the-owner): the
  license field (`crates/aethyr` still says `license = "MIT"` and
  `apps/aethyr/package.json` keeps the name `aethyrforge`), `tools/assets`,
  `tools/reference`, and history.
- Until `tools/assets` and `tools/reference` arrive or are replaced, these
  keep references to them and do not run: the `prepare:*` and `build:player`
  npm scripts (`tools/build-player.ts`), `tools/benchmark-runtime.mjs`,
  `tools/verify-material-editor.mjs`, `tools/verify-town-swarm.mjs`,
  `tools/verify-vignette.mjs`, and `tools/verify-source-refraction.mjs`.
  `tools/benchmark-suite.mjs` reads `experiments/kaigen/results/`, which was
  not imported.
- The C Town backends (`c`, `c-frame`) load Wasm that only the dropped C code
  builds; the default backends are unaffected.
- Confirm that `public/starter/mural.png` is original, as the audit's scan
  recorded.
- Prose paths inside `docs/aethyr/engine` still use Aethyrforge's layout.
