# Whole-Codebase Bloat, Architecture & Refactoring Audit

**Date:** 2026-11-20  
**Commit:** `855654fc81` (`origin/main`)  
**Scope:** Entire `openagents` repository — tracked footprint, git health, governance docs, allowlist debt (`coax/remote/counts.sh`), compiler/app/component structure, monoliths, and duplication.

> **Related audits:** This document is a whole-repository structural audit and complements narrower recent audits rather than replacing them — notably [2026-11-11-domain-driven-design-audit.md](2026-11-11-domain-driven-design-audit.md), [2026-11-19-openagents-desktop-consolidation-audit.md](2026-11-19-openagents-desktop-consolidation-audit.md), and [2026-11-20-openagents-com-live-surface-audit.md](2026-11-20-openagents-com-live-surface-audit.md).

---

## Executive Summary

At `855654fc81`, the repository tracks **38,383 files** totaling **2,189.2 MB (2.19 GB)** in the working tree and **9.4 GB** in `.git`. Active source code is substantial and predominantly Coax (**1,565,229 lines** across **8,309 `.coax` files** outside verification dumps), yet **only 6.0% of tracked bytes (131.23 MB)** are `.coax` source files.

The five highest-leverage findings:

| # | Category | Headline Measurement | Impact |
| --- | --- | --- | --- |
| 1 | **Verification & binary bloat in git** | `docs/verification/` + `psionic/docs/verification/` = **1,182.30 MB (54.0% of repo)** across **13,327 files (34.7% of all files)**; `.git` is **9.4 GB** | Slows every `git worktree add`, `git status`, `git grep`, and remote build sync; pollutes code search with stale generated `.c`/`.h`/`.coax` copies |
| 2 | **Asset packs & ML fixtures in git** | `apps/openagents-web/realms/` (**358.03 MB**) + `psionic/fixtures/` (**235.62 MB**) + `docs/research/` & `docs/anthropic/` (**73.92 MB**) | Another **30.5% of tracked bytes** is large binary packs (`.pack`, `.pack.gz`, `.f32`, `.mp4`, `.htm`), including a **93.14 MB** experimental pack |
| 3 | **Unreadable governance monoliths** | `INVARIANTS.md` = **8,805 lines (564 KB, 313 H2s)**; `NEEDS_OWNER.md` = **1,795 lines (119 KB, 110 H2s)** | Exceeds practical human and agent context budgets despite `AGENTS.md` requiring agents to read `INVARIANTS.md` before changes |
| 4 | **High-leverage architectural debt** | `tools-port` = **1,227**; `layer-context` = **30**; `sql-rules-routes` = **27**; `components-to-move` = **21**; `parity-known` = **114** | **16 of 30** `layer-context` violations come from generic codecs (`json`, `hex`, `base64`) living under `coax/lib/nostr/`; **168** active files still depend on `old/` and `legacy/` |
| 5 | **Three compilers & source monoliths** | Three Coax compilers (**164.7k LOC** combined); `coax/web/host.js` (**4,878 LOC**); `everglade.coax` (**5,298 LOC**); `SceneView.swift` (**2,049 LOC**) | Multiplies maintenance cost, slows iteration, and risks divergent behavior across compiler and host surfaces |

---

## 1. Repository & Git Bloat (2.19 GB Working Tree, 9.4 GB `.git`)

### 1.1 Tracked Footprint by Top-Level Directory

| Directory | Tracked Size | % of Repo | Files | Primary Contents |
| --- | ---: | ---: | ---: | --- |
| `docs/` | 1,179.24 MB | 53.9% | 13,626 | `docs/verification/` (1,079.27 MB), `docs/research/` (59.47 MB), `docs/anthropic/` (14.45 MB) |
| `apps/` | 400.25 MB | 18.3% | 3,393 | `apps/openagents-web/realms/` (358.03 MB of `.pack`/`.pack.gz`/fonts) |
| `psionic/` | 366.48 MB | 16.7% | 8,581 | `psionic/fixtures/` (235.62 MB), `psionic/docs/verification/` (103.02 MB) |
| `coax/` | 119.82 MB | 5.5% | 5,640 | `coax/web/goldens/` (31.31 MB), `coax/tests/` (37.42 MB), `coax/lib/` (23.54 MB) |
| `metaverse/` | 24.13 MB | 1.1% | 745 | `stage-resources/` (9.62 MB), `splice-bots/` (5.84 MB) |
| `bitcoin/` | 20.86 MB | 1.0% | 501 | `fixtures/` (15.06 MB), `book/` (4.40 MB) |
| `metaverse-engine/` | 20.41 MB | 0.9% | 2,273 | Engine modules, render, tests, host, book |
| `assets/` | 17.46 MB | 0.8% | 228 | `assets/registry/` (15.71 MB) |
| `nostr/` | 10.30 MB | 0.5% | 633 | `book/` (4.59 MB), `nips/` (2.98 MB), `examples/` |
| All other 16 top-level dirs | 30.25 MB | 1.4% | 2,763 | `everglade/`, `components/`, `proto/`, `evals/`, `rules/`, `coder/`, `tools/`, `os/`, `openagents/`, `scripts/`, `pay/`, `plans/`, etc. |
| **Total** | **2,189.20 MB** | **100.0%** | **38,383** | **`.git` directory: 9.4 GB** |

### 1.2 Verification Artifact Dumps (`1,182.30 MB`, `13,327` files)

Combined, `docs/verification/` (**1,079.27 MB**, **12,724 files**) and `psionic/docs/verification/` (**103.02 MB**, **603 files**) account for **54.0% of all tracked bytes** and **34.7% of all tracked files**:

- **Compressed archives:** `567` `.gz` files (**562.79 MB**), `18` `.xz` files (**130.80 MB**), `14` `.tar` files (**7.24 MB**) — e.g., `docs/verification/coax-retained-stores-B1-resume-2026-11-01/raw/proof.tar.gz` (58.12 MB) and six `docs/verification/data/aethyr-ch11-2026-11-02/**/*.tar.{gz,xz}` bundles totaling >200 MB.
- **Media & raw tensors:** `317` `.png` (116.93 MB), `180` `.f32` (93.20 MB), `138` `.jpg` (40.36 MB), `13` `.mp4` (30.01 MB), `23` `.bin` (11.56 MB).
- **Execution logs:** `3,102` `.log` files (14.72 MB), `1,093` `.txt` files (9.69 MB), plus `.stderr`, `.stdout`, and `.out` captures (over 2,300 files across the repo).
- **Stale source snapshots polluting search:** Verification folders contain `391` `.coax` files (16.44 MB), `111` `.c` files (14.76 MB), and `76` `.h` files (8.30 MB). In fact, the **two largest `.coax` files in the entire repository** (84,940 lines each) are archived copies of `checker.coax` inside `docs/verification/data/*/compiler-checker-qualified/inputs/coax/lib/compiler/checker.coax`, and **12 of the 15 largest `.c`/`.h` files** in the repo are archived verification outputs (e.g., 39,753-line `proto_importer.c` duplicated in `final-native/` and `final-native-v6/`, and 28,241-line `parakeet-metal-source.h` duplicated in `runtime/` and `optimization/`).

Because `AGENTS.md` mandates creating a fresh `git worktree` per task, every single agent worktree pays the cost of checking out 13,327 verification dump files (~1.18 GB of disk I/O), and `git grep` constantly returns false hits from stale snapshots.

### 1.3 Binary Asset Packs, Fixtures & Out-of-Scope Media

- **`apps/openagents-web/realms/` (358.03 MB, 1,621 files):**
  - `everglade-experimental/assets/glade.pack.gz`: **93.14 MB** — the single largest file in the repository, sitting in an experimental realm.
  - `everglade2/assets/everglade2.pack`: **59.54 MB**
  - `everglade/assets/everglade-zone.pack` (**33.59 MB**) + `everglade-avatar.pack` (**21.08 MB**)
  - `character-creator/assets/glade.pack` (**33.53 MB**) + `fantasy.pack` (**20.91 MB**) + `arkai_c.ttf` (**14.95 MB**)
- **`psionic/fixtures/` (235.62 MB, 4,908 files):**
  - `original-edit/` (62.16 MB, 1,162 files), `deepseek/` (58.81 MB, 256 files — including 24.47 MB `compressed-candidate.bin.gz`), `robotics/` (50.74 MB, 1,345 files), `harmony/` (12.98 MB), `text/` (12.89 MB).
- **`docs/research/` (59.47 MB) & `docs/anthropic/` (14.45 MB):**
  - `docs/research/ocean/slab-construction.mp4` (**28.31 MB**) and `ocean-atomics-reactor.mp4` (**18.46 MB**).
  - `docs/anthropic/s1/anthropicpbc-sx1ax1.htm` (**14.31 MB**) — an external SEC S-1 filing checked directly into git.
- **`coax/web/goldens/` (31.31 MB, 138 files):** Visual regression images tracked directly in git.

### 1.4 Exact Byte-for-Byte Duplicate Files (`79.13 MB` Wasted)

A SHA-256 scan of all tracked files `>= 4 KB` found **389 exact duplicate groups** wasting **79.13 MB**:

- `apps/openagents-web/demos/grid-crowd/assets/cast/Cast.bin` == `metaverse/stage-resources/Cast.bin` (2.32 MB wasted).
- `psionic/fixtures/verse-villain/candidates/{0..9}.bf16` — **10 identical copies** of the same 0.18 MB file (1.66 MB wasted).
- `docs/verification/data/aethyr-ch11-2026-11-01/.../checker.coax` == `docs/verification/data/coax-gpu-047-2026-11-01/.../checker.coax` (5.23 MB wasted).
- `docs/verification/data/aethyr-ch11-2026-11-01/native-media/sans-owned-filter-probe/canonical.bin` == `.../sans-owned-filter/canonical.bin` (5.50 MB wasted).
- `psionic/docs/verification/parakeet-2026-11-02/{runtime,optimization}/...` — duplicated `parakeet-metal-source.h` (2.09 MB) and duplicated `.f32` corpora (`fi`, `de`, `pt`, `ru`, etc., >10 MB wasted).
- Multiple duplicated `before.png` / `after.png` / `scene.png` screenshots across `docs/verification/character-*` directories (>6 MB wasted).

### 1.5 Tracked Generated Code

- **Generated C files (4 files):** `coax/web/api/nip98.generated.c` (**10,912 lines**), `coax/web/api/mesh-policy.generated.c` (**1,546 lines**), `coax/web/api/caller-policy.generated.c`, and `psionic/platform/spot-recovery.generated.c`.
- **Tracked WGSL shaders (69 active files, 107,084 lines, 4.32 MB):** 68 of them live directly in `apps/openagents-web/` (e.g., `slab.wgsl`, `everglade.wgsl`, `everglade5e.wgsl`, `fivemmo.wgsl` at ~320–340 KB each). Where these are deterministic Coax-emitted outputs checked by `wgsl-module --site --check`, evaluate whether build-time generation + hash pin can replace checking in 107k lines of generated shader text.

---

## 2. Active Codebase Scale & Language Mix

Excluding `docs/verification/`, `psionic/docs/verification/`, `*/third_party/*`, and `coax/tests/vectors/upstream/`:

| Language / Extension | Active Lines | Files | Notes |
| --- | ---: | ---: | --- |
| `.coax` | 1,565,229 | 8,309 | Core product, engine, realms, stdlib, compiler, ML, tests |
| `.py` | 191,800 | 1,209 | Tests, reference oracles, importers, evals (`713` in `tools-port.txt`) |
| `.c` | 161,338 | 1,103 | C hosts, API routes (`coax/web/api/`), `coax/bootstrap/`, generated C |
| `.wgsl` | 107,084 | 69 | Mostly generated shaders under `apps/openagents-web/` |
| `.sql` | 51,876 | 234 | Schema and migrations (`coax/web/api/`) |
| `.sh` | 32,151 | 510 | Build/remote/check orchestration (`349` in `tools-port.txt`) |
| `.cpp` | 25,775 | 51 | `coax0` bootstrap compiler (`coax/src/`) |
| `.swift` | 24,553 | 112 | macOS/iOS hosts (`openagents-desktop`, `everglade-desktop`, `shared`) |
| `.h` | 20,878 | 296 | C/ObjC headers |
| `.js` / `.ts` / `.cjs` | 28,519 | 308 | Browser host (`coax/web/host.js` = 4,878 LOC), tooling |
| `.proto` | 8,938 | 118 | Wire schemas (`proto/`) |
| `.md` (all repo) | 820,368 | 3,129 | Specs, books, audits, transcripts, governance |

---

## 3. Governance & Documentation Debt

1. **`INVARIANTS.md` is an 8,805-line (564 KB) monolith with 313 `##` sections.**  
   `AGENTS.md` instructs every agent to "Read `INVARIANTS.md` before changing anything it covers," but a 564 KB single file (~140k tokens) cannot be read in full. Agents either skip it or grep for keywords and miss related invariants.  
   **Recommendation:** Split `INVARIANTS.md` into per-context files (`coax/INVARIANTS.md`, `metaverse-engine/INVARIANTS.md`, `everglade/INVARIANTS.md`, `psionic/INVARIANTS.md`, `pay/INVARIANTS.md`, `apps/INVARIANTS.md`, `coax/web/api/INVARIANTS.md`) and reduce root `INVARIANTS.md` to a concise (<150-line) cross-cutting index.

2. **`NEEDS_OWNER.md` is 1,795 lines (119 KB, 110 `##` sections, 73 open checkboxes).**  
   Dates span `2026-10-29` through `2026-11-16`, with only 1 checked box and 1 heading marked done/resolved. An append-only owner inbox of 110 sections loses urgency and hides true blockers.  
   **Recommendation:** Triage into an active top-10 list at root and move historical/resolved requests to `docs/archive/needs-owner-2026-11.md`.

3. **`AGENTS.md` (32.4 KB, 351 lines) has stale/contradictory entries.**  
   In the "What is here" table, `components/` appears **twice** with contradicting descriptions:
   - Line 73 describes the layered component system (`core/`, `layout/`, `paint/`, `skin/`, `backend/`, `kit/`).
   - Line 78 says *"the spec's layers and skins are not built yet"* (which is now false — those directories hold 28,050 lines of `.coax`).

4. **Transcripts & plan sprawl.**  
   `docs/transcripts/` holds 314 files (6.28 MB, several >3,000 lines each), and `plans/` holds 8 root-level plan files outside `docs/`. Consolidate active plans under `docs/plans/` and decide whether raw episode transcripts belong in git.

---

## 4. Architectural & Allowlist Debt (`coax/remote/counts.sh`)

Snapshot of the ten tracked debt gates at `855654fc81`:

| Gate (`counts.sh show`) | Current Count | Source of Truth | Assessment |
| --- | ---: | --- | --- |
| `tools-port` | **1,227** | `coax/tools-port.txt` | High — 5 entries are already deleted on disk; 84% are in `coax/` (549) and `psionic/` (487) |
| `parity-known` | **114** | `everglade/features.coax` | Medium-high — Everglade parity backlog |
| `layer-context` | **30** | `metaverse-engine/layer-allowlist.txt` | **Quick win** — 16 of 30 are caused by generic codecs parked in `coax/lib/nostr/` |
| `sql-rules-routes` | **27** | `coax/web/api/*.c` + `schema.sql` | Medium — C API routes still invoking PL/pgSQL business rules |
| `components-to-move` | **21** | `coax/web/components-check.coax` | Medium — hand-rolled wrapping/markdown outside `components/` |
| `parity-gaps` | **4** | `everglade/features.coax` | Low — close to zero |
| `wire-allowlist` | **1** | `proto/wire-allowlist.txt` | Healthy (`external-format-fixtures`) |
| `layer-engine-up` / `everglade-up` / `tests` | **0 / 0 / 0** | `metaverse-engine/layer-allowlist.txt` | Clean |

### 4.1 `layer-context = 30`: Generic Codecs Misplaced in `coax/lib/nostr/`

Of the 30 context-boundary violations in `metaverse-engine/layer-allowlist.txt`, **19 import `coax/lib/nostr/*`**, and **16 of those 19** only want generic encoding/JSON primitives:

- `coax/lib/nostr/json.coax` (10 callers across `metaverse-engine/`, `coax/lib/access/`, `coax/lib/x402/`, `coax/web/api/`)
- `coax/lib/nostr/json-f64.coax` (`apps/metaverse-engine-desktop/chat.coax`)
- `coax/lib/nostr/hex.coax` (`metaverse-engine/session/seat-ticket.coax`, `coax/web/room/room-write.coax`)
- `coax/lib/nostr/base64.coax` (`coax/lib/access/phrase-gate-entry.coax`)
- `coax/lib/nostr/block/fi-base64url.coax` (`metaverse-engine/session/{resume-envelope,seat-ticket}.coax`)

**Refactor:** Promote these five generic codec modules into base `coax/lib/encoding/` (or `coax/lib/json/` + `coax/lib/encoding/`), re-export from `coax/lib/nostr/` if needed during transition, and update callers. That single refactor cuts `layer-context` from **30 → 14** (-53%).

### 4.2 `tools-port = 1,227`: Unported Scripts & Stale Allowlist Entries

Breakdown of `coax/tools-port.txt`:
- **By directory:** `coax/` (549), `psionic/` (487), `tools/` (65), `metaverse/` (43), `docs/` (23), `scripts/` (18), `metaverse-engine/` (16), `apps/` (13), `proto/` (7), `coder/` (6).
- **By extension:** `.py` (713), `.sh` (349), `.cjs` (86), `.ts` (76), `.js` (3).
- **Stale entries (5 files missing on disk)** left over from the `openagents-desktop` consolidation:
  - `apps/openagents-desktop/zeron/fixtures/fixture.py`
  - `apps/openagents-desktop/zeron/fixtures/psionic.py`
  - `apps/openagents-desktop/tests/check_zeron_defaults.py`
  - `apps/openagents-desktop/tests/lab-owner.py`
  - `apps/openagents-desktop/tests/zeron_source_extract.py`
- **Large Bash orchestration scripts** that carry complex logic (retry, parsing, scheduling, deployment) and are prime candidates for Coax tooling or decomposition:
  - `coax/remote/after.sh` (1,582 lines), `psionic/check.sh` (906 lines), `coax/remote/check.sh` (834 lines), `coax/web/wasm.sh` (740 lines), `apps/openagents-web/build.sh` (683 lines), `coax/web/room/vm-deploy.sh` (657 lines), `coax/web/api/test.sh` (632 lines), `coax/remote/remote.sh` (526 lines).

### 4.3 Misnamed `old/` and `legacy/` Directories That Are Still Load-Bearing

- **`apps/openagents-web/old/`** (106 files, 3.30 MB, including 4,047-line `old/experimental/fivelab.coax` and 3,485-line `old/v1/everglade-net.coax`) is still referenced by **91 external non-doc files**, including `apps/everglade-desktop/`, `apps/everglade-mobile/`, `apps/metaverse-engine-desktop/`, `apps/openagents-web/gpu/everglade-v1/`, and multiple realms.
- **`everglade/legacy/`** (17 files, 0.80 MB) is still referenced by **77 external non-doc files**, including `apps/openagents-web/build.sh`, `apps/openagents-web/everglade.coax`, and `apps/metaverse-engine-desktop/`.

Keeping active dependencies under `old/` and `legacy/` prevents safe deletion and causes agents to either import deprecated code or accidentally break builds when cleaning up. Extract what is still shared into proper modules, retire dead v1 experiments, and delete both directories.

### 4.4 Backend API Hybrid State (`coax/web/api/`)

`coax/web/api/` currently mixes three paradigms:
- **70 hand-written `.c` files** (including `auth.c` at 1,896 LOC and `player.c` at 1,586 LOC)
- **3 checked-in `.generated.c` files** (`nip98.generated.c` at 10,912 LOC)
- **123 `.coax` files** (37 `*-policy.coax` files)
- **27 C routes** still calling PL/pgSQL business logic in `schema.sql` (`sql-rules-routes = 27`).

Continue migrating policy logic out of PL/pgSQL and C handlers into `#authority` `*-policy.coax` modules, and generate `.generated.c` at build time rather than committing 10k-line diffs.

### 4.5 Component System Transition (`components/`)

The layered component architecture is now substantially built (**28,050 `.coax` LOC** across `core/`, `layout/`, `paint/`, `skin/`, `backend/`, `kit/`, `app/`), but:
- **`components/zeron/`** (74 files, 5,298 `.coax` LOC) still runs in parallel as the legacy chat set.
- **21 external files** (`CC_TO_MOVE` in `coax/web/components-check.coax`) still hand-roll line wrapping or Markdown parsing — including 3 files inside `apps/openagents-web/old/v1/` (retiring `old/v1/` removes 3 immediately).
- **`components/icons/`** tracks **756 individual icon files**; consider whether a packed vector atlas or single generated icon module would simplify distribution and reduce file count.

---

## 5. Compiler Duplication, Monoliths & App Surface Sprawl

### 5.1 Three Coax Compiler Implementations in `coax/`

| Path | Language | Files | LOC | Size | Role |
| --- | --- | ---: | ---: | ---: | --- |
| `coax/src/` | C++20 (`coax0`) | 24 | 22,627 | 1.30 MB | Original C++20 bootstrap compiler (`emit_c.cpp` 4,765 LOC, `main.cpp` 3,084 LOC, `verify.cpp` 2,856 LOC, `eval.cpp` 2,560 LOC) |
| `coax/bootstrap/` | C11 | 23 | 33,382 | 1.72 MB | C bootstrap (`checker.c` **21,916 LOC**, `parser.c` 3,098 LOC) |
| `coax/lib/compiler/` | Coax | 72 | 108,737 | 7.29 MB | Self-hosted compiler (`checker/matrix-{1..4}.coax` ~14.5k LOC each, `fixtures.coax` 12,013 LOC, `parser.coax` 5,554 LOC) |

Maintaining three full compiler frontends/checkers (**164,746 LOC** total) creates triple maintenance overhead for language changes. Establish and publish a single sunset milestone for retiring `coax/src/` (or `coax/bootstrap/`) once self-hosting parity is locked, and split `coax/bootstrap/checker.c` (21,916 LOC) if it must remain long-lived.

### 5.2 Largest Active Source Monoliths (Refactor Candidates)

| File | LOC | Refactor Opportunity |
| --- | ---: | --- |
| `apps/openagents-web/everglade.coax` | 5,298 | Split scene setup, input/HUD, networking, and combat bindings into `everglade/client/` modules |
| `coax/web/host.js` | 4,878 | Split WebGPU bridge, audio/voice, input/pointer, storage, and WASM loader into focused ES modules |
| `apps/openagents-web/myrealm.coax` | 4,445 | Extract reusable realm-shell and editor hooks into `metaverse-engine/` |
| `apps/openagents-web/gpu/everglade-v1/gpu.coax` | 3,860 | Retire with v1 Everglade once `everglade2` supersedes it |
| `everglade/zone-combat.coax` | 3,825 | Separate targeting/LOS, damage/status resolution, and encounter state transitions |
| `apps/openagents-web/belt.coax` / `stair.coax` | 3,766 / 3,495 | Share common realm loop, HUD, and wave/entity scaffolding |
| `apps/openagents-web/chat-web.coax` | 3,373 | Converge onto `components/kit/` + `components/app/` |
| `metaverse/splice-bots/splice-game.coax` | 3,352 | Split simulation, bot AI, and rendering |
| `apps/openagents-desktop/SceneView.swift` | 2,049 | Thin out Swift host logic toward Coax per "Coax first"; split Metal view lifecycle from input/bridge |
| `apps/openagents-desktop/GymWorkbenchView.swift` | 1,934 | Extract workbench state/policy into Coax; split SwiftUI subviews |
| `metaverse/voice-host/host.c` | 1,925 | Split audio device I/O, resampling/ringbuffer, and FFI glue |
| `coax/web/serve.c` | 1,791 | Split HTTP routing, WebSocket/live-reload, and static asset serving |

### 5.3 App & Realm Surface Sprawl

- **8 app directories under `apps/`:** `openagents-web`, `openagents-desktop`, `openagents-ios`, `openagents-cli`, `everglade-desktop`, `everglade-mobile`, `metaverse-engine-desktop`, and `shared`. Following the `openagents-desktop` consolidation, `evals/coder-desktop/` (228 files, 3.97 MB) still carries the legacy `coder-desktop` name, and `everglade-desktop` / `everglade-mobile` / `metaverse-engine-desktop` still couple into `apps/openagents-web/old/` and `everglade/legacy/`.
- **52 realm directories under `apps/openagents-web/realms/`:** Includes numerous one-off labs and superseded versions (`5e-lab`, `animation-lab`, `anime-lab`, `audio-lab`, `bitcoin-lab`, `coax-lab`, `dragon-lab`, `metahuman-lab`, `nostr-lab`, `portal-lab`, `splat-lab`, `state-lab`, `valley-lab`, `waterlab`, `everglade-classic`, `everglade-experimental`, `fixture`, `sandbox`). Separating production realms from archived/experimental labs would clarify what must be maintained and built.

---

## 6. Prioritized Action Plan

### P0 — Quick Wins (1–2 Days, High Impact, Low Risk)

1. **Externalize `docs/verification/` and `psionic/docs/verification/` (`-1.18 GB`, `-13,327 files`, `-54%` repo size).**  
   - Keep lightweight markdown summaries + SHA-256 manifests in git; move raw `.tar.gz`/`.tar.xz`, `.mp4`, `.f32`, `.log`, `.stderr`, `.stdout`, `.out`, screenshots, and generated `.c`/`.h`/`.coax` dumps to content-addressed object storage (or `~/.openagents/verification/`).
   - Add `.gitignore` rules under `docs/verification/` and `psionic/docs/verification/` blocking `*.tar.gz`, `*.tar.xz`, `*.log`, `*.stderr`, `*.stdout`, `*.out`, `*.f32`, `*.mp4`, and generated `.c`/`.h`.
   - Add a pre-push size/extension guard so future verification runs cannot re-introduce multi-MB archives or source snapshots.
2. **Remove out-of-scope binary files (`-61 MB`).**  
   - Remove `docs/anthropic/s1/anthropicpbc-sx1ax1.htm` (14.31 MB) and move `docs/research/ocean/*.mp4` (46.77 MB) out of git.
3. **Clean up immediate hygiene papercuts.**  
   - Remove the 5 deleted-file entries from `coax/tools-port.txt` (`1,227 → 1,222`).
   - Remove the duplicate, stale `components/` row in `AGENTS.md` (line 78).
   - Deduplicate `apps/openagents-web/demos/grid-crowd/assets/cast/Cast.bin` (reference `metaverse/stage-resources/Cast.bin`) and collapse the 10 identical `psionic/fixtures/verse-villain/candidates/{0..9}.bf16` files.

### P1 — High-Leverage Architectural Refactors (1–2 Weeks)

1. **Extract generic codecs out of `coax/lib/nostr/` (`layer-context`: `30 → 14`).**  
   - Move `json.coax`, `json-f64.coax`, `hex.coax`, `base64.coax`, and `fi-base64url.coax` into `coax/lib/encoding/` (or `coax/lib/json/` + `coax/lib/encoding/`), update callers, and remove 16 entries from `metaverse-engine/layer-allowlist.txt`.
2. **Externalize large binary packs & ML fixtures (`-593 MB`).**  
   - Move `apps/openagents-web/realms/**/*.pack{,.gz}` (starting with 93.14 MB `everglade-experimental/assets/glade.pack.gz`) and `psionic/fixtures/` (235.62 MB) to manifest-pinned fetches cached under `~/.openagents/` (aligned with `AGENTS.md`'s shared asset/model cache rule).
3. **Split `INVARIANTS.md` and triage `NEEDS_OWNER.md`.**  
   - Partition `INVARIANTS.md` (564 KB) into per-context `INVARIANTS.md` files with a concise root index.
   - Archive resolved/stale items from `NEEDS_OWNER.md` (119 KB) so active owner blockers fit on one screen.
4. **Eliminate load-bearing dependencies on `apps/openagents-web/old/` (91 refs) and `everglade/legacy/` (77 refs).**  
   - Promote still-needed modules out of `old/` and `legacy/`, update app build scripts, and delete the dead v1 trees (which also knocks 3 items off `components-to-move`).

### P2 — Structural Debt Burn-Down (2–6 Weeks)

1. **Finish `components/` migration (`components-to-move`: `21 → 0`) and retire `components/zeron/`.**
2. **Decompose the top source monoliths:** `coax/web/host.js` (4,878 LOC), `apps/openagents-web/everglade.coax` (5,298 LOC), `everglade/zone-combat.coax` (3,825 LOC), `apps/openagents-desktop/SceneView.swift` (2,049 LOC), and `GymWorkbenchView.swift` (1,934 LOC).
3. **Burn down `sql-rules-routes` (`27 → 0`) and stop checking in `*.generated.c` / generated `.wgsl` files** where deterministic build-time emission is fast.
4. **Burn down `coax/tools-port.txt` (`1,227 → <500`):** Audit for obsolete one-off scripts in `coax/` (549) and `psionic/` (487) that can be deleted outright before porting the remaining active scripts to Coax.
5. **Lock a retirement plan for redundant compiler frontends** (`coax/src/` vs `coax/bootstrap/` vs `coax/lib/compiler/`), and plan an eventual `git` history/LFS repack once large verification/pack binaries are no longer tracked on `main` (to shrink `.git` from **9.4 GB**).
