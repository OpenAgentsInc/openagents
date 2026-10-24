> Imported unchanged from Aethyrforge `docs/audits/2026-10-24-consolidate-into-openagents.md` at `2b1ccb7ef5` (2026-10-24). Relative links point into that repository at that commit. Decisions here are superseded by [`docs/adr/`](../../adr/README.md); this is background.

# Consolidating Aethyrforge into the openagents monorepo

Date: 2026-10-24. Status: planning audit. Nothing has been moved, deleted, or
relicensed. Repositories inspected: `OpenAgentsInc/aethyrforge` at `0a33298`
(full history, 277 commits on all branches) and `OpenAgentsInc/openagents` at
`60622d40`. The private `cakbags` repository was read only for its
`aethyrforge-migration-plan.md`; nothing from it is quoted beyond the boundary
it describes.

The owner's instruction (2026-10-24): "We are open-sourcing everything except
the private cakbags repo." The private Aethyrforge engine therefore moves into
the public openagents monorepo. This audit lists what moves where, what must not
be published, which openagents rules conflict, how the builds fit together, one
reconciled set of engine decision records, and a phased plan that keeps the live
Kiki MVP working.

Conventions in this document:

- **Measured** means a count or result produced by a command recorded here.
- **Inference** marks a conclusion drawn from reading code or docs without
  running it.
- **Recommendation** is a proposal awaiting the owner where marked
  **Owner decision**.

## Summary

- **Recommendation:** Aethyrforge becomes *the* OpenAgents engine. Its Rust
  crates move to a separate Cargo workspace at `crates/aethyrforge/` (excluded
  from the root workspace, like `crates/psionic`), its TypeScript web app to
  `apps/aethyrforge/`, and its specification to `docs/engine/`, with one set of
  14 decision records in `docs/engine/decisions/`.
- **Measured:** the full history contains no credential patterns, no licensed
  vendor binaries, and no deleted files. The only binaries ever committed are
  six original starter-scene buffers and two original PNGs. History can be
  published as-is, provided the owner accepts four low-risk items listed in
  [history scan](#history-scan): a 220 KB digest of a third party's X posts,
  17 absolute home-directory paths in one results file, an X archiving tool, and
  the owner's commit email (already public in openagents).
- **Policy:** the engine is mostly TypeScript (500 `.ts`, 18 `.tsx`, 104 `.mjs`
  files) and Python (125 files). openagents allows TypeScript only in named
  exceptions. The owner must grant one engine web exception that replaces the
  three existing ones (`apps/haunt-preview`, `apps/verse-selector`,
  `apps/verse-ui`) over time.
- **Duplicates:** there are five renderers for overlapping worlds today
  (Aethyrforge TypeScript WebGPU, Aethyrforge Rust render lab, Verse v1 on
  `wgpu`, the iOS `medieval_town` `wgpu` Metal renderer, and the
  `apps/haunt-preview` TypeScript WebGPU renderer), plus two universe selectors,
  two simulation kernels, two world authorities, two semantic building models,
  and two asset compilers. The decision records below pick one of each and mark
  the rest legacy with an exit condition.
- **Simplification:** once the engine is public, the public/private host
  protocol split stops being an intellectual-property boundary. The
  `aethyrforge-host` protocol remains as the security boundary between the
  trusted OpenAgents page and the untrusted engine/preview origins, but the
  engine can link the SDK at build time instead of fetching an operator-pinned
  cross-origin SDK.
- **Kiki MVP continuity:** the live routes run pinned Cloud Run releases
  selected by JSON files in `crates/openagents-web/content/demos/`. No pin
  changes until a release built from the monorepo passes the same browser
  checks. Old release tags stay deployed.

## Inventory and disposition

Sizes are tracked bytes and lines from `git ls-files` at `0a33298` (measured).
Untracked local directories (`node_modules/` 244 MB, `dist/` 452 KB,
`target/`, and the untracked `kiki-tmp.mjs` capture script) are not part of the
import.

Dispositions: **move** (path in openagents), **merge** (combine with an existing
openagents component), **replace**, or **drop**.

### Rust crates

| Aethyrforge path | Size | Contents | Disposition |
| --- | --- | --- | --- |
| `crates/kernel` (`af-kernel`) | 17 files, 419 KB, 11.6k lines | Player/camera, fixed step, collision, typed scene edits, battle crowd; builds the browser `kernel.wasm` | **Move** to `crates/aethyrforge/kernel`. Later **merge** overlapping simulation with `crates/verse-engine` (schedule, entity storage, animation) per [ADR 0009](#reconciled-decision-records) |
| `crates/mmorpg-core` | 8 files, 77 KB, 2.3k lines | Shared Quick MMORPG simulation rules | **Move** to `crates/aethyrforge/mmorpg-core` |
| `crates/server` (`af-world-server`) | 3 files, 20 KB, 495 lines | Axum/Tokio WebSocket world server for Open Ground | **Move** to `crates/aethyrforge/world-server`; later **merge** with `crates/verse-world` and `crates/verse-host` into one play authority |
| `crates/gpu` (`af-gpu`) | 5 files, 44 KB, 1.2k lines | `webgpu.h`-shaped GPU interface, browser backend, native `bindgen` backend | **Move** to `crates/aethyrforge/gpu`. Becomes the only GPU interface for new engine code |
| `crates/render-lab` | 16 files, 163 KB, 4.0k lines | Rust renderer lab for decisions 0006–0008 (work in progress as of `0a33298`) | **Move** to `crates/aethyrforge/render-lab`; it is the seed of the Rust renderer |

The workspace (`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` pinning
1.98.1 with `wasm32-unknown-unknown`) moves with the crates. Crate names keep
their `af-` prefixes at import so that builds stay byte-comparable; a rename to
`aethyrforge-*` can follow.

### TypeScript packages and apps

| Aethyrforge path | Size | Contents | Disposition |
| --- | --- | --- | --- |
| `packages/renderer` | 135 files, 1.71 MB, 47.5k lines | Current production TypeScript WebGPU renderer | **Move** to `apps/aethyrforge/packages/renderer`. Stays the production path until each Rust pass reaches parity ([ADR 0004](#reconciled-decision-records)); then **replace** pass by pass |
| `packages/runtime` | 44 files, 395 KB, 12.4k lines | Worker runtime, battle views, agent contract | **Move** to `apps/aethyrforge/packages/runtime` |
| `packages/editor-services` | 32 files, 257 KB, 7.8k lines | Effect 4 services, authoring permits, agent client | **Move** to `apps/aethyrforge/packages/editor-services` |
| `packages/assets` | 5 files, 32 KB, 1.1k lines | Runtime asset loading | **Move** |
| `packages/boids` | 5 files, 55 KB, 1.6k lines | Boids Lab | **Move** (lab) |
| `packages/universe` | 4 files, 29 KB, 584 lines | Universe selector, ported from openagents `apps/verse-selector` at `9fd3263494` (Apache-2.0 header) | **Merge**: one universe selector. Keep the engine's version and retire `apps/verse-selector` once `/new-lander/verse` uses it |
| `apps/editor` | 35 files, 194 KB, 4.2k lines | React DOM, Effect Atom, shadcn/ui editor | **Move** to `apps/aethyrforge/apps/editor`; later **merge** with `apps/verse-ui` (same stack, same owner authorization) |
| `apps/preview` | 3 files, 183 KB, 5.2k lines | Isolated preview page and worker | **Move** |
| `apps/player` | 5 files, 19 KB, 516 lines | Release player shell | **Move** |
| `apps/universe` | 3 files, 15 KB, 495 lines | Universe editor/preview entry | **Move**; merge as above |
| `apps/render-lab` | 3 files, 31 KB, 662 lines | Browser host and WebGPU import table for the Rust lab | **Move**; becomes the web platform host of [ADR 0004](#reconciled-decision-records) |
| `apps/boids`, `apps/job-lab` | 3 files, 30 KB | Labs (`job-lab` is a 1-line stub) | **Move** `boids`; **drop** `job-lab` unless it is in active use |
| Root HTML, `vite.config.ts`, `tsconfig.json`, `postcss.config.mjs`, `package.json`, `package-lock.json` | 11 HTML entries | Vite multi-page build | **Move** to `apps/aethyrforge/` |
| `public/` | 13 files, 111 KB | Original starter courtyard buffers and `mural.png`, favicon | **Move**; confirm provenance of `mural.png` before publishing ([docs/starter-game.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/starter-game.md) states the courtyard is generated) |
| `LICENSE.shadcn` | MIT notice | shadcn/ui component origin | **Move** beside the components |

### Tests, tools, and deployment

| Aethyrforge path | Size | Contents | Disposition |
| --- | --- | --- | --- |
| `tests/` | 226 files, 1.85 MB, 56.2k lines (224 `.ts`) | `tsx --test` unit and contract tests | **Move** to `apps/aethyrforge/tests` |
| `tools/*.mjs` (96 files, 47.4k lines) | Playwright verifiers, benchmarks, recorders | Browser checks run through `openagents browser run` | **Move** to `apps/aethyrforge/tools`. Same pattern as openagents `scripts/check-*.mjs` |
| `tools/assets/` | 114 files, 1.42 MB; 19.2k lines Python, 5.2k lines TypeScript | Private-asset import and compile pipeline, including Unreal exporters | **Move** as a transitional exception, then **merge** into the Rust compiler in `crates/verse-content` (`compiler` feature) and `crates/verse-bake` ([ADR 0013](#reconciled-decision-records)) |
| `tools/reference/` | 27 files, 290 KB, 4.1k lines Python | Charts measured through a locally installed Unreal 5.8 editor | **Move** under `apps/aethyrforge/tools/reference`, or **drop** if the owner does not want Unreal-dependent tooling in public |
| `tools/film/`, `tools/fixtures/` | 11 files, 74 KB | Film capture, synthetic fixtures | **Move** |
| `tools/check_spec.py` | 16 KB | Link, schema, and abstract-model checks for the specification | **Move** to `docs/engine/check_spec.py` or `scripts/engine/`; port to Rust later |
| `tools/host-fixture.py` | 2 KB | Synthetic host conformance fixture | **Merge** with `crates/aethyrforge-host/examples` |
| `tools/archive_x.py`, `tools/test_archive_x.py` | 16 KB | X post archiver (needs `X_BEARER_TOKEN` from the environment) | **Drop**. It is research tooling, not engine code |
| `tools/build-c-town.mjs` | 8 KB | Builds the C experiment | **Drop** with the C code |
| `tools/deploy/` | 4 files, 36 KB | Release packaging, Nginx/CSP generation, Cloud Build config, release log | **Move** to `scripts/deploy/aethyrforge/` and **merge** its README into openagents `docs/verse/aethyrforge-deployment.md` |

### Documentation, contracts, and experiments

| Aethyrforge path | Size | Disposition |
| --- | --- | --- |
| `docs/*.md` (43 specification files) | 68 files under `docs/`, 1.64 MB, 19.6k lines total | **Move** to `docs/engine/`. Remove "proprietary" and "private repository" wording in [openagents-host.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/openagents-host.md), [integration.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/integration.md), and [tools/deploy/README.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/tools/deploy/README.md) |
| `docs/decisions/0001`–`0008` | 8 records, 459 lines | **Replace** with the reconciled set below; keep the originals in `docs/engine/decisions/history/` for traceability |
| `INVARIANTS.md` | 13 invariants | **Move** to `docs/engine/invariants.md` with IDs renamed `ENG-INV-01`…`13`; link it from openagents `INVARIANTS.md` instead of adding rows there |
| `AGENTS.md` | Contributor contract | **Merge** a short engine section into openagents `AGENTS.md`; drop the sentence that openagents language rules do not apply |
| `docs/requirements.json`, `contracts/` (6 files, 37 KB), `examples/` (9 files, 24 KB) | **Move** to `docs/engine/` |
| `docs/references/` (4 files, 76 KB) | Medieval Town visual target, shot guide, gallery JSON with `fab.com` listing and preview URLs | **Move**. They describe a licensed product with public links; they hold no vendor files |
| `docs/research/` (10 files, 359 KB) | **Move** to `docs/engine/research/`, except `gdechichi-tweets-2026.md` (220 KB of a third party's posts) and `x-archive-tool.md`: **drop** (or move to `cakbags`) |
| `docs/media/boids-lab-2026-10-23.png` | 626 KB original capture | **Move** |
| `experiments/c-town/` | 42 files, 516 KB, 6.0k lines (C, Rust, TypeScript) | **Drop** the C code (a third product language with a recorded negative result). **Move** its README and results into `docs/engine/research/c-town/` |
| `experiments/boids/results/` | 3 JSON files, 201 KB | **Move** to `docs/engine/research/data/` |
| `experiments/kaigen/` | 8 JSON files, 1.80 MB | **Move** after replacing 17 absolute `/Users/...` scratch paths in `smooth-diagnostics-2026-10-23.json`, or **drop** and keep only the research summary |
| `third_party/` | 5 directories, 23 KB | **Move** `quick-mmorpg` (MIT) and `simplex-noise` notices. **Replace** the three `openagents-*` notices with in-repo references: they credit openagents source that will live in the same repository |

### Duplicates between the repositories

| Concern | Aethyrforge | openagents | Recommendation |
| --- | --- | --- | --- |
| Browser renderer | `packages/renderer` (TS, 47.5k lines); `crates/render-lab` + `crates/gpu` (Rust) | `crates/verse`, `crates/verse-gfx`, `crates/verse-pbr` (95.7k + 2.7k + 44.6k lines, `wgpu` 30.0.1), browser via `crates/everglade-web` (10.9k lines, WebGPU or WebGL2); `apps/haunt-preview` (TS WebGPU, 11.5k lines) | One Rust renderer over `af-gpu` ([ADR 0004, 0005](#reconciled-decision-records)). The TS renderer stays production until parity. `apps/haunt-preview` is **dropped** once `/new-lander/house` no longer needs its fallback. Verse v1 rendering is legacy |
| Native town | Not present | `apps/openagents/crates/mobile/src/medieval_town` (6 files, 3.8k lines, own `town.wgsl`, `wgpu` on Metal) | Interim native distribution, frozen; replaced by the engine core's native build ([ADR 0003](#reconciled-decision-records)) |
| Universe selector | `packages/universe`, `apps/universe`, pinned by `content/demos/universe.json` | `apps/verse-selector` (1.2k lines) built into `crates/openagents-web/src/pages/new_lander.js` | Keep one, in the engine |
| Editor UI | `apps/editor` (React, Effect Atom, shadcn/ui) | `apps/verse-ui` (same stack, 2.3k lines) built into `content/verse-ui/ui.js` | One web UI package ([ADR 0011](#reconciled-decision-records)) |
| Simulation kernel | `af-kernel`, `af-mmorpg-core` | `crates/verse-engine` (17.8k), `crates/verse-world` (90.6k), `crates/physics` (29.8k) | Converge on one engine core; physics stays a shared crate ([ADR 0009](#reconciled-decision-records)) |
| World authority | `af-world-server` | `crates/verse-world`, `crates/verse-host` | One Rust play authority |
| Semantic buildings | Semantic world model ([docs/world-model.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/world-model.md)) | `crates/world-tree` (`openagents.verse-world-tree.v1`: districts, buildings, rooms) | One model; `world-tree` becomes an input adapter ([ADR 0008](#reconciled-decision-records)) |
| Asset compiler | `tools/assets` (Python/TS) | `crates/verse-content` `compiler` feature, `crates/verse-bake` | One Rust compiler ([ADR 0013](#reconciled-decision-records)) |
| Character pipeline | Python importer and TS animation pass reimplemented from `verse-zone-everglade` (see `third_party/openagents-character`) | `crates/verse-zone-everglade` pack format, `crates/verse-content` | Use the Rust original directly once in one repository |
| Host protocol split | `docs/openagents-host.md`, private adapter in `packages/editor-services` | `crates/aethyrforge-host` (53 files, 14.4k lines, wire v1–v7 schemas) | Keep the protocol as a security boundary; drop the IP-boundary machinery ([ADR 0012](#reconciled-decision-records)) |

**Inference:** the host SDK's split exists because the engine was private. Its
docs say so directly ("Aethyrforge remains proprietary", "Public builds never
fetch or link that implementation"). The origin separation it enforces is
still required by invariant INV-09 (untrusted content never receives editor
authority), so the protocol stays.

## What must not be published

### Never in Git, before or after the move

- Licensed vendor sources and derived data: Modular Medieval Town (Fab
  standard license), Fantasy Props MegaKit, Unreal project exports, recovered
  metadata, textures, the compiled runtime asset sets (`assetsets/<sha256>/`),
  Mixamo-derived Quick MMORPG character exports, and the Everglade `.vtp`
  packs. Aethyrforge already keeps all of these outside Git (measured below).
- Private captures of licensed scenes (the openagents rule: "Licensed scene
  captures stay in scratch storage").
- Credentials: Cloud Build/automation service-account configuration,
  `CLOUDSDK_CONFIG` contents, the test-account token file, `X_BEARER_TOKEN`.
  openagents `AGENTS.md`: "Never put an API key in source, a log line, a test
  fixture, or an issue."
- Anything from `cakbags`, including its Kiki review packets, product briefs,
  and the 25-page screen packet referenced by the MVP qualification record.

### Present in the tree and needing review

- **Vendor mesh identifiers.** 52 `SM_*` names and other vendor identifiers
  appear in 20 files, mostly `tools/assets/*.py` and tests (measured with
  `git grep -oE '\bSM_[A-Za-z0-9_]+'`). The openagents licensed-kit rule allows
  "the recipe that maps our IDs to the vendor's mesh names". The stricter
  private-character rule allows no name at all. **Owner decision:** confirm
  the licensed-kit rule covers Medieval Town and Fantasy Props identifiers in
  engine code, or move the mapping into the private asset manifests.
- **Unreal tooling defaults.** Seven exporters default `--engine` to
  `/Users/Shared/Epic Games/UE_5.8`. Harmless, but replace with a required
  argument for portability.
- **Release identities.** `tools/deploy/README.md` records Cloud Run revision
  names, image digests, and asset-set SHA-256 values. These are already public
  in openagents (`docs/verse/aethyrforge-mvp-qualification.json`), so no new
  exposure.
- **Live private-asset URL in public code (existing).** openagents
  `apps/openagents/crates/mobile/src/medieval_town/mod.rs` hard-codes a
  `aethyrforge-preview` Cloud Run URL and SHA-256 for a compiled native town
  manifest. **Inference:** that serves compiled licensed geometry to any
  client, which matches the licensed-kit rule (compiled packs are served to
  every client) but should be recorded in `docs/verse/private-assets.md`.
- **Third-party text.** `docs/research/gdechichi-tweets-2026.md` (220,866
  bytes) reproduces excerpts of up to 25 words from 952 posts by one X account.
  Recommendation: do not publish it; move it to `cakbags` or delete it.

### History scan

Commands, run on a full clone after `git fetch --unshallow` and fetching every
remote branch (`main` and `codex/kiki-mobile-touch`, 277 commits):

```sh
git log --all -p > history.patch                      # 17.1 MB
grep -E 'AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{35}|sk-[A-Za-z0-9_-]{20,}|sk-ant-|ghp_|gho_|github_pat_|xox[bpas]-|BEGIN [A-Z ]*PRIVATE KEY|"private_key"|client_secret|Bearer [A-Za-z0-9._-]{24,}' history.patch
git rev-list --objects --all | git cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(rest)' | awk '$1=="blob" && $3>200000'
git log --all --name-only --format= | sort -u | grep -iE '\.(glb|gltf|fbx|png|jpe?g|ktx2|exr|hdr|bin|wasm|mp4|webm|vtp|zip|uasset|umap|tga|dds|obj|blend|mov|wav|ogg|mp3)$'
git log --all --diff-filter=D --name-only --format=
grep -cE '[A-Za-z0-9+/]{400,}={0,2}' history.patch
```

Measured results:

| Check | Result |
| --- | --- |
| Credential patterns | None |
| Environment secret names | `X_BEARER_TOKEN` (read from the environment by `tools/archive_x.py`; no value), `UNRELATED_OWNER_SECRET` (synthetic test value) |
| Blobs over 200 KB | Seven: `boids-lab` PNG (626 KB), Kaigen results JSON (up to 603 KB), the tweet digest (221 KB), `packages/renderer/renderer.ts` (213 KB) |
| Binary or model files ever committed | `docs/media/boids-lab-2026-10-23.png`, `public/starter/*.bin` (5), `public/starter/mural.png`; all original |
| Deleted files | None. Every path ever committed is still in the tree, so a HEAD review covers history |
| Embedded base64 payloads | None |
| Cloud Run, `googleapis`, `gs://`, `openagentsgemini` references in history | None |
| Absolute home paths | 17 `/Users/christopherdavid/.openagents/scratch/...` paths in the Kaigen results; 8 `/Users/Shared/Epic Games` defaults |
| Commit authors | `AtlantisPleb <chris@openagents.com>` (12), `Codex <codex@openai.com>` (263), `Claude <noreply@anthropic.com>` (2) |

**Conclusion:** the history can be published as-is. Because no file was ever
deleted, everything the HEAD cleanup removes still appears in history. Only the
tweet digest, the home paths, and the X archiver would remain visible there.

**Owner decision:** either

1. **Preserve history (recommended).** Import with an unrelated-history merge
   so that every Aethyrforge commit keeps its SHA. openagents records already
   cite engine commits (`private_engine: 0a28e7d…` in
   `docs/verse/aethyrforge-mvp-qualification.json`; the C experiment cites
   `ae4ac9a…`). These links keep working. Accept the three items above in
   public history.
2. **Filter history.** Run `git filter-repo` to drop the tweet digest and the X
   archiver and rewrite the 17 paths. Every SHA changes, so publish an old-to-new
   SHA map in `docs/engine/history.md` and update the qualification records.
3. **Squash.** Import one snapshot commit and archive the old repository. The
   cited SHAs then resolve only in that archive.

Before any option, run a second scanner (`gitleaks detect` or `trufflehog git`)
over the same clone. This audit used regular expressions only.

## Policy conflicts with openagents `AGENTS.md`

| Rule in openagents | Aethyrforge today | Resolution |
| --- | --- | --- |
| "Product code in this workspace is Rust. Do not add TypeScript." Named exceptions: `apps/haunt-preview`, `apps/verse-selector`, `apps/verse-ui` | 500 `.ts` (146k lines), 18 `.tsx`, 11 `.mts`, 104 `.mjs` (50k lines); React DOM, Effect Atom, Effect 4, shadcn/ui, Vite, Tailwind | **Owner decision:** grant one named exception, `apps/aethyrforge`, for the engine's web host, editor, labs, tests, and Node verification tools. It absorbs the `apps/verse-ui` exception and retires the `apps/haunt-preview` and `apps/verse-selector` exceptions. Per [ADR 0004](#reconciled-decision-records), the renderer and runtime move to Rust over time, so the TypeScript surface shrinks to UI and web host glue |
| Python allowed only as "retained training and acceptance tooling" | 125 Python files (24.4k lines), 19.2k in `tools/assets` (product asset compilation) | **Owner decision:** a transitional tooling exception for `apps/aethyrforge/tools/assets`, with the Rust compiler ([ADR 0013](#reconciled-decision-records)) as the exit. Acceptance tools such as `check_spec.py` fit the existing exception |
| No C product code | `experiments/c-town` (3 `.c`, 2 `.h`) | **Drop** the code, keep the results |
| "No GitHub workflows or GitHub-billed automation" | Aethyrforge has none; deploy uses Cloud Build with an authorized service account | No change |
| "Never put an API key in source" | None found | No change; add the scan to the import checklist |
| Licensed asset rules in `docs/verse/private-assets.md` | Same intent, different paths (`$HOME/.openagents/verse/private/aethyrforge*`) and env names (`AETHYRFORGE_PRIVATE_ASSETS`, …) | Keep paths; document them as an engine asset location in `private-assets.md`. Resolve the vendor-identifier question above |
| User-facing copy: no machine talk (`oa-copy` guard) | Engine UI strings were not written against this guard | Run `oa_copy::violations` over engine UI text before any engine UI ships on openagents.com from the monorepo; **inference:** some strings (for example "candidate", "generation", "admitted" in diagnostics) will need rewording |
| Velocity: no new `INVARIANTS.md` rows or long docs | Engine spec is 19.6k lines with its own invariants | Keep the engine specification in `docs/engine/` with its own invariants file; do not add rows to the root `INVARIANTS.md` |
| Commit attribution names the actual contributor | Engine history has Codex and Claude authors | Compatible |
| License: Apache-2.0 (`LICENSE` at root) | `Cargo.toml` says `license = "MIT"`; no `LICENSE` file; [integration.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/integration.md#repository-licensing-and-contribution-gate) proposes Apache-2.0 but none was adopted | **Owner decision:** Apache-2.0 for engine code, matching openagents. Change the workspace `license` field at import. `packages/universe` is already Apache-2.0 |

## Build and workspace integration

### Cargo workspace placement

- **Measured:** openagents root pins Rust 1.97.1 (`rust-version = "1.97.1"`,
  edition 2024). `apps/openagents` pins 1.98.1 in its own workspace for GPUI.
  Aethyrforge pins 1.98.1 with `wasm32-unknown-unknown` and declares
  `rust-version = "1.98"`.
- **Recommendation:** `crates/aethyrforge/` as its own workspace, added to the
  root `exclude` list with a comment, the same way as `crates/psionic` and
  `apps/openagents`. It keeps its own `Cargo.lock`, `rust-toolchain.toml`
  (1.98.1, `wasm32-unknown-unknown`), and release profile (`lto = "thin"`,
  `panic = "abort"`). Reasons: no root toolchain bump, no change to the root's
  shared-target incremental list, and the engine's `naga` 30, `axum` 0.8.9 and
  `tokio` pins stay independent.
- **Consumers:** the website (`openagents-web`, root workspace) does not link
  engine crates; it embeds releases. The phone app (`apps/openagents`,
  1.98.1) will link the engine core when the native build exists; a path
  dependency between two 1.98.1 workspaces works. `crates/aethyrforge-host`
  stays in the root workspace because the website links it; the engine's
  build links it by path ([ADR 0012](#reconciled-decision-records)), so it must
  keep building on 1.97.1.
- **Shared target:** add `~/work/openagents-target-1-engine` to the shared
  target list in `AGENTS.md`.

### Node packages

- `apps/aethyrforge/package.json` keeps its exact pins (React 19.3.1, Effect
  4.0.6, Vite 8.3.5, TypeScript 7.0.2, Playwright 1.64.1). `apps/verse-ui`
  pins the same stack; merge them into one npm workspace when the UIs merge
  so there is one lockfile for this stack.
- Built outputs that the Rust website embeds (`new_lander.js`,
  `content/verse-ui/ui.js`) remain checked-in generated files until those
  surfaces move into the engine's release, after which they are deleted.

### Verification

- Keep the existing local commands, rooted at the new paths: `npm run check`,
  `npm test`, `npm run build`, `cargo test` in `crates/aethyrforge`, and
  `python3 docs/engine/check_spec.py`.
- **Measured:** `check_spec.py` currently fails on a pre-existing broken link
  (`experiments/c-town/README.md` → `measurement.md`). Fix it during the move,
  since the C experiment directory changes anyway.
- Browser checks run through `openagents browser run`, as openagents already
  does with `scripts/check-haunted-house.mjs`, `scripts/check-medieval-town.mjs`,
  and the `scripts/check-verse-*.mjs` family.

### Deployment and release pinning

Today (from [tools/deploy/README.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/tools/deploy/README.md) and openagents
`docs/verse/aethyrforge-deployment.md`):

1. Build the public SDK from a clean openagents revision and pin its Wasm SHA-256.
2. Build the engine from a clean private revision with `VITE_OPENAGENTS_PROTOCOL_URL`
   pointing at the editor origin's `/sdk/<sha256>/aethyrforge_host.js`.
3. Package two contexts (`aethyrforge-editor`, `aethyrforge-preview`), build
   with Cloud Build, deploy by digest with `--no-traffic` and a fresh tag.
4. Mount the private asset set read-only into the preview service.
5. Update `crates/openagents-web/content/demos/*.json` (or
   `WEB_MEDIEVAL_RELEASE_JSON` / `OPENAGENTS_HAUNT_RELEASE`) and promote the
   website.

After consolidation:

- One source revision builds both the SDK and the engine. The release record
  names one openagents commit instead of a public/private pair.
- The cross-origin SDK fetch and its operator-pinned URL are no longer needed
  for new releases: the engine links `aethyrforge-host` at build time.
  Pinned old releases keep loading their old SDK URLs.
- Source maps can be published. The "no source maps, no source tree" rule in
  the deploy contexts becomes optional; asset privacy remains.
- What stays: separate editor and preview origins, CSP, sandboxed preview, the
  private asset bucket and its read-only mount, immutable release tags,
  HTTP/2 to the container (the town's 45.68 MB PNG exceeds Cloud Run's HTTP/1
  limit), and the "never test on the owner's account" rule.
- Host protocol versions (`crates/aethyrforge-host/schema/host-v1.json` …
  `host-v7.json`, `VERSION = 7` in `protocol.rs`) stay frozen per pinned
  release. **Measured:** the protocol document still describes the version 5
  handshake and the version 6 schema while the code is at 7; fix that during
  the docs move. Retire a frozen schema only when no deployed tag uses it.

## Reconciled decision records

**Recommendation:** put the unified records in `docs/engine/decisions/`, with an
index in `docs/engine/decisions/README.md`. Move Aethyrforge 0001–0008 to
`docs/engine/decisions/history/` unchanged so their links and reasoning remain
readable. Each new record states which older record or document it supersedes.

### Contradictions resolved

| Contradiction | Resolution |
| --- | --- |
| Aethyrforge 0001 puts graphics in a thin TypeScript WebGPU host; 0006 moves the renderer into Rust | 0006 wins ([ADR 0004](#reconciled-decision-records)). The TypeScript renderer remains production until per-pass parity, as 0006 already says |
| openagents audit `docs/audits/2026-10-22-owned-renderer-webgpu-and-effect.md` recommends a TypeScript runtime and renderer with Rust only for measured bottlenecks | Superseded by ADR 0004. The C/Wasm Town results (CPU preparation 1.9–4.0 ms median against a 30–42 ms HDR pass; [results](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/research/c-medieval-town-results-2026-10-23.md)) show language is not the frame-rate lever, so portability decides |
| openagents Verse v1 (`docs/verse/engine/architecture.md`) uses `wgpu` directly on every platform, including WebGL2 in the browser | ADR 0005: new engine code calls `af-gpu`. `wgpu` remains the fallback native backend behind that interface. Verse v1 is legacy; no new Verse v1 renderer features. WebGL2 is not an engine target ([docs/platform-support.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/platform-support.md) selects no WebGL renderer); the Everglade WebGL2 path stays only until Everglade is ported |
| openagents "Rust only" against the React/Effect editor | ADR 0011: one named web UI exception; domain authority stays in Rust |
| iOS `medieval_town` is a second, native renderer of the same town with its own WGSL | ADR 0003: an interim native distribution, like the WebView option 0006 allows, frozen and replaced by the engine core's native build |
| Two "engine" names: Verse Engine and Aethyrforge | ADR 0001: the engine is Aethyrforge; Verse is the product built on it |
| Closed-source engine behind a public host SDK (cakbags plan, [openagents-host.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/openagents-host.md)) | ADR 0002 and 0012: open source; the protocol remains only as a security boundary |

### The records

1. **0001: One engine, Aethyrforge, in the openagents monorepo.** Aethyrforge is
   the OpenAgents engine: world model, compiler, runtime, renderer, editor, and
   release format. Verse is the metaverse product built on it, and Kiki's
   universe, house, and personal planets are Verse content. The name "Verse
   Engine" retires; the `verse-*` crates are the Verse v1 runtime, kept working
   in maintenance until each zone runs on the engine, with no new engine
   features added there. Supersedes the naming and ownership sections of
   openagents `docs/verse/engine/architecture.md` and makes
   `docs/verse/engine/roadmap.md` historical.

2. **0002: Open source code, private licensed assets.** Engine code is
   Apache-2.0 under the openagents root license. Licensed vendor files, their
   converted geometry and textures, compiled private asset sets, and captures
   of licensed scenes never enter Git. Committed code may name licensed pieces
   only through mapping recipes allowed by `docs/verse/private-assets.md`.
   Every build without private assets runs on original or CC0 fixtures.
   Supersedes the closed-source boundary in `cakbags/aethyrforge-migration-plan.md`
   and the licensing gate in Aethyrforge [integration.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/integration.md).

3. **0003: Web first, then mobile, then desktop.** The web ships first. The
   first mobile delivery is the web player in mobile browsers; native iOS and
   Android follow from the same engine core, inside the one OpenAgents mobile
   app (`apps/openagents`), then desktop. Each native target is a separately
   qualified profile. The existing iOS `medieval_town` renderer is an interim
   native distribution: kept working, not extended, and removed when the
   engine core's iOS build reaches parity. Keeps the target order of Aethyrforge
   0006.

4. **0004: One Rust engine core, including the renderer.** Runtime,
   simulation, render graph, visibility, batching, resource lifetimes, and GPU
   command recording are Rust crates that build for `wasm32-unknown-unknown`
   and native targets. TypeScript keeps only the web UI (ADR 0011) and web host
   glue: canvas and worker bootstrap, input, lifecycle, and the WebGPU import
   table. Native hosts supply surface, input, and lifecycle through a C ABI. The
   TypeScript renderer stays the production path until each Rust pass reaches
   image parity and equal or better measurements on pinned profiles. Supersedes
   Aethyrforge 0001's graphics placement and the openagents 2026-10-22
   owned-renderer audit; keeps Aethyrforge 0006's core decision.

5. **0005: One GPU interface shaped like `webgpu.h`.** All engine GPU code calls
   `af-gpu`. Its web backend calls the browser's WebGPU; its native backend
   links Dawn; `wgpu` (wgpu-native) is the fallback native backend if Dawn
   fails qualification on a target. No backend-specific types cross the
   interface. There is no WebGL renderer. Verse v1 code that calls `wgpu`
   directly (`verse`, `verse-gfx`, `verse-pbr`, `everglade-web`, mobile
   `medieval_town`) is legacy and ports onto `af-gpu` as zones move.
   Supersedes the `wgpu` dependency choice in openagents
   `docs/verse/engine/architecture.md`; keeps Aethyrforge 0006's backend choice.

6. **0006: GPU-driven rendering on every platform.** GPU-resident scene
   tables, compute visibility with indirect draws, a visibility buffer for
   opaque geometry, temporal anti-aliasing and upscaling as the default,
   cluster LOD for dense static geometry, crowd tiers, and capability-selected
   fast paths with baseline fallbacks. Each step is kept only if it measurably
   reduces GPU time on pinned profiles. Keeps Aethyrforge 0007 unchanged.

7. **0007: Engine shaders in Slang, shipped as WGSL.** Engine shaders are
   authored in Slang and compiled offline to WGSL validated by both Tint and
   Naga. One WGSL artifact serves every platform. Creator materials stay WGSL
   through the existing material hook. Verse v1 WGSL and GLES variants move
   when their passes port. Keeps Aethyrforge 0008.

8. **0008: Semantic worlds compiled incrementally into complete
   generations.** Editable buildings are structural graphs; graphics,
   collision, navigation, selection, and streaming data derive from one
   document digest and activate together. openagents `crates/world-tree`
   (`openagents.verse-world-tree.v1`) becomes an import adapter into this
   model, not a second building model. Keeps Aethyrforge 0002 and invariants
   INV-03, INV-04, and INV-08.

9. **0009: One authority model for authoring, play, and releases.**
   Authoring commits validated transactions against a branch head; play runs in
   a separate Rust simulation authority pinned to a release; releases are
   immutable and channels advance by compare-and-swap. The engine converges on
   one play-authority service: `af-world-server` and `af-kernel` merge with
   `verse-world`, `verse-engine`, and `verse-host` rather than running beside
   them. OpenAgents identity, NIP-HOST grants, and `coder-sync` rooms remain
   the account and session layer. `crates/physics` stays the shared physics
   crate. Keeps Aethyrforge 0003.

10. **0010: Metered creator behaviors.** A documented TypeScript syntax subset
    compiles to owned bytecode run by a metered Rust/Wasm VM in an isolated
    worker, with capability-checked host calls. This is creator content, not
    product code, so it needs no language exception. Keeps Aethyrforge 0004;
    the feasibility experiment is still required.

11. **0011: One web UI stack as a named exception.** Engine and Verse web UI use
    React DOM, TypeScript, Effect Atom, and Effect 4, in one package under
    `apps/aethyrforge`, as the single named TypeScript exception for this area.
    It absorbs `apps/verse-ui`; `apps/verse-selector` and `apps/haunt-preview`
    retire. Rust keeps marketing pages, routes, accounts, projects,
    permissions, publication, invitations, voice, and payments. The engine frame
    loop stays independent of React renders (INV-13). Native apps keep GPUI,
    UIKit, and Kotlin shells with Rust Native cards. Keeps Aethyrforge 0005 and
    amends the openagents `AGENTS.md` exceptions.

12. **0012: The host protocol is a security boundary.** The trusted
    openagents.com page embeds the engine in a separate-origin frame through
    `aethyrforge-host`; untrusted creator previews run on a third origin. The
    protocol keeps exact-origin handshakes, scoped ports, bounded messages, and
    server-side authorization. The engine links the crate at build time; the
    operator-pinned cross-origin SDK URL, "private adapter" wording, and
    source-map prohibitions retire for new releases. Frozen schemas stay
    available while any deployed release uses them. Updates
    `docs/verse/aethyrforge-host-protocol.md` and Aethyrforge
    [openagents-host.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/openagents-host.md).

13. **0013: One asset compiler, in Rust.** Asset admission and compilation
    converge on one deterministic Rust compiler built from `crates/verse-content`
    (`compiler` feature) and `crates/verse-bake`, with Aethyrforge's asset
    identity, rights, and generation rules ([asset-compilation.md](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/asset-compilation.md)).
    The Python and TypeScript tools in `tools/assets` are a transitional
    exception: no new pipelines are added in Python, and each tool is retired
    when its Rust replacement produces identical output hashes on the pinned
    asset sets.

14. **0014: Builds, verification, and releases.** The engine is its own Cargo
    workspace at `crates/aethyrforge/` (Rust 1.98.1, `wasm32-unknown-unknown`),
    excluded from the root workspace. Checks run locally or on non-GitHub
    infrastructure. A release is built from one pushed openagents commit, packaged
    into immutable editor and preview images deployed by digest, and selected by
    the website through `crates/openagents-web/content/demos/*.json`. Rollback
    selects a previous release JSON. Keeps the deployment rules of Aethyrforge
    `tools/deploy/README.md` and openagents `docs/verse/aethyrforge-deployment.md`,
    which become one document.

### Supersession map

| Existing record or document | Fate |
| --- | --- |
| Aethyrforge [0001](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0001-hybrid-engine.md) | Superseded by 0004 and 0011 |
| Aethyrforge [0002](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0002-incremental-world.md) | Carried into 0008 |
| Aethyrforge [0003](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0003-authority-and-publishing.md) | Carried into 0009 |
| Aethyrforge [0004](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0004-safe-behaviors.md) | Carried into 0010 |
| Aethyrforge [0005](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0005-react-effect-frontend.md) | Carried into 0011 |
| Aethyrforge [0006](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0006-cross-platform-engine-core.md) | Split into 0003, 0004, and 0005 |
| Aethyrforge [0007](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0007-gpu-driven-rendering.md) | Carried into 0006 |
| Aethyrforge [0008](https://github.com/OpenAgentsInc/aethyrforge/blob/2b1ccb7ef5599138464cff1bd6407f074fa65971/docs/decisions/0008-shader-toolchain.md) | Carried into 0007 |
| openagents `docs/verse/engine/architecture.md` | Naming and `wgpu` choice superseded by 0001 and 0005; rest kept as Verse v1 documentation |
| openagents `docs/verse/engine/roadmap.md` | Historical |
| openagents `docs/audits/2026-10-22-owned-renderer-webgpu-and-effect.md` | Superseded by 0004 (kept as a dated audit) |
| openagents `docs/verse/browser-editor-implementation-plan.md` | Superseded by 0011 and the engine delivery plan |
| openagents `docs/verse/aethyrforge-host-protocol.md` | Kept, revised under 0012 |
| openagents `docs/verse/aethyrforge-deployment.md` | Kept, merged with Aethyrforge `tools/deploy/README.md` under 0014 |
| openagents `docs/verse/private-assets.md` | Kept; referenced by 0002 |
| openagents `docs/verse/mobile.md`, `docs/verse/zones.md` | Kept as Verse product docs; renderer statements fall under 0005 |
| `cakbags/aethyrforge-migration-plan.md` | Its closed-source boundary is superseded by 0002 and 0012; its product journey stays with the owner |

## Phased migration plan

Order is by dependency. Each phase ends with the Kiki MVP routes unchanged:
`https://openagents.com/new-lander`, `/new-lander/verse`, and
`/new-lander/house`.

### What keeps the MVP working

**Measured from openagents source:**

- `/new-lander` is Rust/Maud (`pages/new_lander.rs`) plus the built
  `content/verse-ui/ui.js` and `ui.css`.
- `/new-lander/verse` embeds the engine release pinned in
  `content/demos/universe.json` (`z12580ue8d9551374` on the
  `aethyrforge-editor` service) and the built selector `new_lander.js`.
- `/new-lander/house` embeds the release pinned in
  `content/demos/haunted-house.json` (`z12580he8d9551374`); personal planets use
  `content/demos/runtimes/personal-planet.json` (protocol 7). A fallback to
  `apps/haunt-preview` exists when no house release is configured.

The rule for every phase: the website keeps serving the current pins and the
Cloud Run tags they name stay deployed. A pin changes only after a monorepo-built
release passes `scripts/check-haunted-house.mjs`,
`scripts/check-verse-universe.mjs`, the other `scripts/check-verse-*.mjs`
checks used in the MVP qualification, and `scripts/check-medieval-town.mjs`, in
candidate mode against a no-traffic tag.

### Phase 0: decisions and freeze point

- Owner answers the decisions listed in the [next section](#decisions-for-the-owner).
- Agree a cutover commit on Aethyrforge `main`. Another session is active in
  this repository today; announce the cutover so in-flight work lands first.
  Decide whether to merge or drop `codex/kiki-mobile-touch` (2 commits ahead).

### Phase 1: pre-publication cleanup in Aethyrforge

- Remove or relocate the tweet digest, X archiver, and C experiment code; scrub
  or drop the Kaigen results; fix the broken `measurement.md` link.
- Add the Apache-2.0 license and change the workspace `license` field.
- Replace "proprietary", "private repository", and "closed source" wording.
- Run `gitleaks` or `trufflehog` over the full clone.
- Apply the chosen history option.
- Risk: low. Exit check: `python3 tools/check_spec.py`, `npm run check`,
  `npm test`, `cargo test`.

### Phase 2: import without behavior change

- Bring the tree in at the mapped paths in one merge.
- Add the workspace exclude, the `AGENTS.md` engine section and language
  exception, and `docs/engine/` with the 14 records.
- Build the engine from the monorepo and compare `kernel.wasm` and bundle
  hashes with the same commit built in the old repository. Equal hashes prove
  the move changed nothing.
- Risk: path assumptions in tools (`ROOT`, `public/kernel.wasm`, Vite inputs)
  and in `tools/deploy/render.py`. Exit check: identical artifacts.

### Phase 3: release from the monorepo

- Move `tools/deploy` to `scripts/deploy/aethyrforge/` and merge the two
  deployment documents.
- Deploy a monorepo-built release to the existing `aethyrforge-editor` and
  `aethyrforge-preview` services with `--no-traffic` and new tags.
- Run the candidate checks, then update the three demo JSON pins in one website
  release, keeping the previous JSON for rollback.
- Risk: the MVP qualification records name a public/private SHA pair; record
  the new single-repository identity in a new qualification entry instead of
  editing the old one.
- Archive the Aethyrforge repository with a README pointing to the monorepo.
  **Owner decision:** archive it as public or keep it private.

### Phase 4: simplify the host boundary

- Link `aethyrforge-host` into the engine build, remove
  `VITE_OPENAGENTS_PROTOCOL_URL` for new releases, and update the protocol
  document to version 7.
- Retire frozen schemas only after no deployed tag uses them.
- Risk: a release that drops a protocol version still pinned by a project
  release would break shared links. Check `content/demos` and stored project
  releases before retiring anything.

### Phase 5: remove duplicates, one at a time

Each step has its own qualification and can stop independently:

1. One universe selector: switch `/new-lander/verse` fully to the engine
   universe, then delete `apps/verse-selector` and `new_lander.js`.
2. One web UI: merge `apps/verse-ui` into the engine UI package.
3. Retire `apps/haunt-preview` and the `haunt.rs` fallback once the house pin
   has been stable for an agreed period.
4. Use the Rust character pipeline directly; drop the Python reimplementation.
5. Port asset compilers to Rust per ADR 0013.
6. Converge simulation and authority per ADR 0009, starting with shared
   physics and the world tree adapter.
7. Rust renderer passes reach parity per ADR 0004; then the iOS app links the
   engine core and `medieval_town` is removed per ADR 0003.
8. Port Verse v1 zones onto the engine per ADR 0005; drop WebGL2 with the last
   zone that needs it.

Risks across phase 5: scope. Verse v1 is about 408k lines of Rust and WGSL
across the 30 `crates/verse*` crates, plus 29.8k in `crates/physics`
(measured; the largest are `verse` 95.7k, `verse-world` 90.6k, `verse-pbr`
44.6k, and `verse-engine` 17.8k). It ships in the
desktop app and the phone app's Verse tab. **Inference:** porting it is a
multi-month program; ADR 0001 keeps it working rather than forcing a deadline.

## Decisions for the owner

1. **License:** Apache-2.0 for engine code (recommended), and whether the
   archived Aethyrforge repository becomes public.
2. **History:** preserve full history (recommended), filter it, or squash; and
   whether the third-party tweet digest may remain in public history.
3. **TypeScript exception:** one `apps/aethyrforge` exception for the web UI,
   web host, labs, tests, and Node verifiers, replacing the three current
   exceptions over time.
4. **Python exception:** a transitional exception for `tools/assets` until the
   Rust compiler replaces it.
5. **Workspace:** a separate engine workspace at 1.98.1 (recommended) or a
   root toolchain bump to 1.98.1.
6. **Names:** "Aethyrforge" as the engine name and the retirement of "Verse
   Engine".
7. **Verse v1:** legacy maintenance with gradual porting (recommended), and
   whether WebGL2 support ends with it.
8. **iOS native town:** keep it frozen as an interim distribution
   (recommended) or remove it now.
9. **Vendor identifiers:** whether licensed-kit mesh names may stay in public
   engine code under the licensed-kit rule.
10. **Unreal-dependent tools:** publish `tools/reference` and the Unreal
    exporters, or drop them.
11. **Branch and cutover:** merge or drop `codex/kiki-mobile-touch`, and the
    cutover date for work on Aethyrforge `main`.
