# GameBuilder → OpenAgents Monorepo Consolidation & Unified Asset Pipeline Audit

**Date:** 2026-11-20  
**Snapshots Inspected:**
- `~/work/gamebuilder` at `ea3efb0` (`main`, pinned via `openagents.lock` to `openagents` `7c8823286b8`)
- `openagents` at `a274aaf77e` (`origin/main`)

> **Related docs:** Complements [docs/desktop/2026-11-20-gamebuilder-3d-worlds-audit.md](../desktop/2026-11-20-gamebuilder-3d-worlds-audit.md) (which covers embedding `gamebuilder` CLI/MCP and 3D world panes inside OpenAgents Desktop) and [2026-11-20-codebase-bloat-and-refactor-audit.md](2026-11-20-codebase-bloat-and-refactor-audit.md) (especially `#13844`, asset/fixture externalization).

---

## 1. Executive Summary

`~/work/gamebuilder` is compact (**347 tracked files**, **12.94 MB**, **166 `.coax` files** totaling 4.95 MB) and is already deeply coupled to `openagents`: it makes **336 references to 129 targets** inside `vendor/openagents/` (`coax/bin/coax`, `coax/web/tool.sh`, `metaverse-engine/*`, `components/*`, `coax/lib/*`, `apps/shared/host/linux`, `metaverse/splice-bots/*`), and `openagents` already hosts GameBuilder's server-side wire/API pieces (`coax/web/api/gamebuilder-agents.c`, `gamebuilder-agent-wire.coax`, `proto/coder/gamebuilder-agent-v1.json`).

With the owner's 2026-11-20 decision to open-source GameBuilder, keeping `~/work/gamebuilder` as a separate repository now imposes pure friction: a lockfile (`openagents.lock`), a pinned symlink/worktree (`vendor/openagents`, `scripts/pin.sh`), duplicated host/web/deploy scripts, and split asset management.

Folding `~/work/gamebuilder` into `openagents` should be done **together with a unified content-addressed asset pipeline** that solves both GameBuilder's binary assets (`village.pack`, `scrappers.gbm`, images/icons) and `openagents`'s existing `#13844` asset bloat (`358 MB` of realm `.pack`/`.pack.gz` bundles + `235 MB` of `psionic/fixtures/`) in **one coherent migration**:
- **Adds:** ~5 MB of active GameBuilder Coax/Swift/C/Web/CLI source under `apps/gamebuilder/` + `docs/gamebuilder/`.
- **Removes:** `~597 MB` of tracked binary blobs from git (realm packs, `psionic/fixtures/`, `village.pack`, `scrappers.gbm`), plus retires `openagents.lock`, `vendor/openagents`, `scripts/pin.sh`, and legacy `apps/openagents-web/worldbuilder*`.
- **Net result:** One monorepo, zero cross-repo pinning, one durable content-addressed asset pipeline, and a ~60% smaller working tree.

---

## 2. Inventory of `~/work/gamebuilder` (at `ea3efb0`)

| Directory | Files | Size | Contents |
| --- | ---: | ---: | --- |
| `web/` | 62 | 4.57 MB | `gamebuilder.org` Coax Native Web app (`page.coax`, `hud.coax`, `ghost-data.coax`, `assets/village.pack` 1.84 MB, `img/` 0.54 MB, `fonts/` 0.19 MB, `gpu/`, `host/`) |
| `host/` | 41 | 3.48 MB | Apple (`Sources/` 16 Swift files + `Shaders.metal`, `Assets.xcassets` 1.30 MB, `Models/scrappers.gbm` 1.87 MB, `Bridge.h`, entitlements) + Linux (`host/linux/`, `host/panel/`, `host/page/`) |
| `coax/` | 49 | 1.77 MB | Core GameBuilder logic (`app.coax`, `commands.coax`, `editor.coax`, `level.coax`, `play.coax`, `decide.coax`, `mcp.coax`, `lobby.coax`, `sync.coax`, `publish.coax`, `world.coax`, etc.) |
| `docs/` | 36 | 1.53 MB | 26 engineering docs + `docs/images/` (0.48 MB) + **`docs/business/` (9 files, 0.28 MB)** |
| `tools/` | 47 | 0.66 MB | Checks, recordings, `player-test.coax`, `splicebots-*` porting tools, `models.py` |
| `tests/` | 60 | 0.38 MB | `commands/` (26 `.case` files), `player/` (31 files), `user/` (3 files) |
| `cli/` | 11 | 0.25 MB | `gamebuilder` CLI (`main.coax`, `account.coax`, `cloud.coax`, `publish.coax`, `resident.coax`, etc.) |
| `shared/` | 5 | 0.11 MB | `buildings.coax`, `game-types.coax`, `kinds.coax`, `loop.coax`, `plays.coax` |
| `scripts/` | 20 | 0.07 MB | `build.sh`, `check.sh`, `pin.sh`, `web-deploy.sh`, `release.sh`, `install-cli.sh`, etc. |
| `deploy/api/` | 3 | 0.01 MB | `Dockerfile`, `deploy.sh`, `run.sh` (Cloud Run / GCS for `https://gamebuilder.org` API) |
| `<root>` & misc | 13 | 0.11 MB | `AGENTS.md` (72.6 KB), `project.yml`, `openagents.lock`, `LICENSE`, `.claude/`, `.github/` |
| **Total tracked** | **347** | **12.94 MB** | **166 `.coax`, 15 `.swift`, 27 `.sh`, 8 `.c`, 5 `.js`, 1 `.py`, 39 `.md`** |

> **Pre-flight warning (uncommitted local state in `~/work/gamebuilder`):**  
> `git status` currently shows `M scripts/web-deploy.sh` (+9/-1 lines) and untracked `docs/claude-ui-spec/tokens.css` (7,038 bytes). Commit or explicitly discard both before starting the migration so no work is lost.

### Live External Dependencies & Production Surfaces in `~/work/gamebuilder`
- **Apple bundle IDs (`project.yml`):** `com.openagents.gamebuilder`, `com.openagents.gamebuilder.dev`
- **Web hosting (`scripts/web-deploy.sh`):** `gs://openagentsgemini-gamebuilder-web` → `https://gamebuilder.org/`
- **API hosting (`deploy/api/deploy.sh`):** GCP Cloud Run + GCS bucket + `GB_MODEL_KEY_FILE=/secrets/model/key`, serving `https://gamebuilder.org`
- **Top `vendor/openagents/` imports:** `coax/web/tool.sh` (48×), `coax/bin/coax` (27×), `coax/lib/sys/{tool,fs,proc,clock}.coax` (67×), `coax/lib/json/json.coax` (14×), `metaverse-engine/{arch,render,editor,physics,cam,state,host,core}/*` (35+×), `coax/web/player-browser.coax` (6×), `components/{gallery/linux/main.c,kit/hotbar.coax}` (6×), `apps/shared/host/linux` (3×), `metaverse/splice-bots/rules/walk.coax` (2×).

---

## 3. Core Architectural Decisions

Six decisions should be locked before moving files so the migration lands cleanly on the first pass:

### D1. License & Public-vs-Private Split (`docs/business/`)
- **Context:** `~/work/gamebuilder/LICENSE` is still proprietary, though the owner decided on 2026-11-20 to open-source GameBuilder. Meanwhile, `~/work/gamebuilder/docs/business/` contains 9 internal strategy/financial files (including `economics.md`, `phase-one.md`, `roblox-scale.md`, `comparables.md`, and `cursor-for-game-development-2026-11-18.pdf`).
- **Decision needed:** Confirm that all GameBuilder code/assets/engineering docs adopt `openagents`'s root `LICENSE`, and decide whether `docs/business/` stays private (moved to a private ops repo / local archive) or is intentionally published in `openagents`.
- **Recommendation:** Adopt root `LICENSE` for code/assets/engineering docs; **exclude `docs/business/` from public `openagents`** unless explicitly approved for public release.

### D2. Canonical Directory Layout ("What Goes Where")
Keep GameBuilder cohesive under **`apps/gamebuilder/`** (matching `apps/openagents-desktop/`, `apps/openagents-ios/`, `apps/openagents-web/`, `apps/openagents-cli/`, `apps/everglade-desktop/`), while promoting genuinely generic engine/editor/tool pieces upward:

| Source in `~/work/gamebuilder` | Destination in `openagents` | Rationale |
| --- | --- | --- |
| `coax/` (49 files) | `apps/gamebuilder/core/` | Product/editor/play/lobby/MCP Coax logic |
| `shared/` (5 files) | `apps/gamebuilder/shared/` | Shared game types/buildings/kinds/plays between native + web + CLI |
| `cli/` (11 files) | `apps/gamebuilder/cli/` | Installs `~/.openagents/bin/gamebuilder` (matches `apps/openagents-cli/`) |
| `web/` (62 files, excl. heavy assets) | `apps/gamebuilder/web/` | Dedicated `gamebuilder.org` web surface reusing `coax/web/` host & `components/` |
| `host/Sources/`, `project.yml`, `Bridge.h`, `Assets.xcassets`, entitlements | `apps/gamebuilder/apple/` | Converge shared auth/GPU host glue onto `apps/shared/host/apple/` |
| `host/linux/`, `host/panel/`, `host/page/` | `apps/gamebuilder/linux/` | Converge onto `apps/shared/host/linux/` & `components/` |
| `deploy/api/` | `apps/gamebuilder/deploy/api/` (Phase 2) → converge into `coax/web/api/` (Phase 3) | Keep `gamebuilder.org` API deploy unbroken during move, then unify |
| `tools/` (47 files) | `apps/gamebuilder/tools/` (product checks) + promote generic SpliceBots/player-test helpers to `metaverse-engine/tools/` | Avoid fragmenting engine tools |
| `tests/` (60 files) | `apps/gamebuilder/tests/` | Wired directly into `coax/web/suites.coax` |
| `scripts/` (minus `pin.sh`, `env.sh`) | `apps/gamebuilder/scripts/` | Rewrite `vendor/openagents/` paths to repo-relative; delete pinning scripts |
| `docs/` (excl. `docs/business/`) | `docs/gamebuilder/` | Consistent with `docs/desktop/`, `docs/cli/`, `docs/metaverse/` |
| `AGENTS.md` (72.6 KB) | Merge domain rules into `apps/gamebuilder/AGENTS.md` (short) + `apps/gamebuilder/INVARIANTS.md` (indexed in root `INVARIANTS.md`) | Follows the `#13845` per-context invariants pattern |
| `web/assets/village.pack`, `host/Models/scrappers.gbm`, large PNG/JPGs | **Externalized via Unified Asset Pipeline (Section 4)** | Do not add new multi-MB binaries to git |
| `openagents.lock`, `vendor/openagents`, `scripts/pin.sh`, `GameBuilder.xcodeproj`, `Generated/`, `build/`, `.DS_Store` | **Delete / Do not migrate** | Eliminated by monorepo + `xcodegen` |

### D3. Bounded Context (`contexts.txt`) & JSON Parser Deduplication
- Register a `gamebuilder` bounded context in `contexts.txt`:
  ```text
  gamebuilder folders: apps/gamebuilder/ may load: engine protocol coder chat nostr toolchain zeron-kit splice
  ```
  so `coax/web/tool.sh layer-check` enforces boundaries from Day 1 with **0 new entries in `metaverse-engine/layer-allowlist.txt`**.
- **Deduplication opportunity spotted:** `openagents` currently has **two** JSON libraries:
  - `coax/lib/encoding/json.coax` (RFC 8259 pull parser/writer, just extracted in `#13843`)
  - `coax/lib/json/json.coax` (50.5 KB, loaded 14× by GameBuilder)
  Verify whether `coax/lib/json/` is a higher-level shape/DOM layer over `encoding/json.coax` or a second parser; if a second parser, plan consolidation onto one canonical JSON module.

### D4. Product Surface Consolidation (Retiring `WorldBuilder`)
`~/work/gamebuilder/docs/openagents-integration.md` already recorded the owner's direction: *WorldBuilder is deprecated in favor of GameBuilder*. Moving GameBuilder into `openagents` makes the product map crisp:
1. **`apps/gamebuilder/`** — Canonical 3D game/world creator (iOS, iPadOS, macOS, Linux, Web at `gamebuilder.org`, plus `gamebuilder` CLI & MCP server).
2. **`apps/openagents-desktop/`** — Canonical desktop workspace, invoking `~/.openagents/bin/gamebuilder` (CLI/MCP) and rendering 3D worlds inline/side-pane via shared `components/app/` (per `docs/desktop/2026-11-20-gamebuilder-3d-worlds-audit.md`).
3. **Retire `apps/openagents-web/worldbuilder*` & `apps/openagents-web/realms/create-world/`** (12.09 MB) once `gamebuilder.org` / `apps/gamebuilder/web/` covers the `/create-world` route.

### D5. `tools-in-coax` Compliance Before Landing
`coax/remote/counts.sh` gates `tools-port` so no commit may raise it. `~/work/gamebuilder` contains **27 `.sh`**, **5 `.js`**, and **1 `.py`** (`tools/models.py`) scripts. Before merging into `openagents`:
- Delete obsolete scripts (`scripts/pin.sh`, `scripts/env.sh`, etc.).
- Add valid `Firm reason: N (...)` headers (per `coax/tools-in-coax.md`) to thin wrappers and browser glue (`web/app.js`, `web/hud-glue.js`, `web/keep.js`, `tools/browser-run-record.js`, `tools/entrance-probe.js`).
- Port any remaining logic-heavy scripts (e.g., `tools/models.py` if still used) to Coax so `tools-check` passes with **0 new `tools-port.txt` entries**.

### D6. Git History Strategy
- **Do not** run a raw `git subtree add` / merge of `~/work/gamebuilder` if `docs/business/` or proprietary history must remain private in public `openagents`.
- **Recommended:** Import clean, logical commits (Phase 2) onto `openagents` `main` (or run `git filter-repo` stripping `docs/business/`, `.pack`, `.gbm`, `.pdf`, `.xcodeproj` if commit history is desired), then archive `OpenAgentsInc/gamebuilder` read-only with a deprecation banner pointing to `openagents/apps/gamebuilder/`.

---

## 4. Unified Content-Addressed Asset & Fixture Pipeline (One Migration for GameBuilder + `#13844`)

Instead of treating GameBuilder's binary assets (`village.pack` 1.84 MB, `scrappers.gbm` 1.87 MB, icons/images) separately from `openagents`'s `#13844` bloat (`apps/openagents-web/realms/` 358 MB + `psionic/fixtures/` 235 MB), solve **all** large binary assets with one standard pipeline aligned with `AGENTS.md`'s `~/.openagents/` shared-files rule:

### Architecture

1. **One Durable Content-Addressed Remote Bucket (No TTL)**
   - Provision **`gs://openagents-assets`** (separate from the 14-day-TTL `gs://openagentsgemini-coax-build-cache` build cache, in the same GCP project as `gs://openagentsgemini-gamebuilder-web`).
   - Object key layout: `gs://openagents-assets/sha256/<first2>/<sha256>` (immutable, deduplicated globally across all realms, GameBuilder, and Psionic fixtures).
   - Serve public read-only HTTPS via Cloud CDN (or Cloudflare R2) at `https://assets.openagents.com/sha256/<first2>/<sha256>` so **fresh checkouts, contributors, and remote build hosts require zero credentials to download**. Uploads use the existing automation service account.

2. **One Shared Local Cache (`~/.openagents/assets/sha256/`)**
   - Extends the existing `~/.openagents/assets/` directory mandated by `AGENTS.md` ("Shared asset library").
   - Every worktree on a machine shares the same cached blobs — `git worktree add` never duplicates gigabytes of `.pack` or `.f32` files.

3. **Small Checked-In SHA-256 Lock Manifests in Git**
   - `assets/manifests/gamebuilder.lock.json` (`village.pack`, `scrappers.gbm`, large raster assets)
   - `assets/manifests/realms.lock.json` (`everglade2.pack`, `everglade-zone.pack`, `everglade-avatar.pack`, `glade.pack`, `glade.pack.gz`, `fantasy.pack`, `arkai_c.ttf`, etc.)
   - `assets/manifests/psionic-fixtures.lock.json` (large `psionic/fixtures/` binaries >256 KB: `original-edit/`, `deepseek/`, `robotics/`, `harmony/`, `text/`)
   - Each row pins `{ "path": "...", "sha256": "...", "bytes": N, "group": "..." }`.

4. **One Coax Hydrator Tool (`coax/web/assets-sync.coax` → `coax/web/tool.sh assets-sync`)**
   - `coax/web/tool.sh assets-sync [--group gamebuilder|realms|psionic|all] [--check]`
   - For each manifest entry:
     1. Checks if workspace target exists with matching size/SHA-256 (fast no-op).
     2. Checks `~/.openagents/assets/sha256/<aa>/<sha256>`; if missing, downloads from `https://assets.openagents.com/sha256/<aa>/<sha256>` into a temp file, verifies SHA-256 (fails closed on mismatch), and atomically moves into cache.
     3. Materializes into the workspace path via **copy / clonefile (`cp -c` on APFS)** — **not symlink** — because `coax/web/serve.c:1444` explicitly rejects symlinks (`realpath(full) != full`)! On macOS APFS, `cp -c` is copy-on-write (instant, zero extra disk space, regular file!).
   - Invoked automatically by `apps/gamebuilder/scripts/build.sh`, `apps/openagents-web/build.sh`, `psionic/check.sh`, and `coax/remote/host-job.sh`.
   - Externalized workspace paths are `.gitignore`d.

5. **Pre-Push Binary Bloat Guard (`coax/web/prepush.coax`)**
   - Extend the `#13840` verification guard in `coax/web/prepush.coax` to refuse newly added/modified binary assets (`*.pack`, `*.pack.gz`, `*.gbm`, `*.glb`, `*.gltf`, `*.f32`, `*.f64`, `*.mp4`, `*.ttf`, `*.otf`) above a threshold (e.g., 512 KB) with an actionable message pointing to `coax/web/tool.sh assets-sync` and `assets/manifests/`.

### Net Size Impact of the Combined Migration
| Bucket | Before | After (in git) | Delta |
| --- | ---: | ---: | ---: |
| `apps/openagents-web/realms/` packs | 358.0 MB | ~2 MB (manifests + small HTML/MD) | **-356 MB** |
| `psionic/fixtures/` large binaries | 235.6 MB | ~15 MB (small unit fixtures + manifest) | **-220 MB** |
| GameBuilder source + docs (`apps/gamebuilder/`, `docs/gamebuilder/`) | 0 MB | ~5.5 MB (`.coax`, `.swift`, `.c`, `.md`, tests) | **+5.5 MB** |
| GameBuilder binary assets (`village.pack`, `scrappers.gbm`) | 0 MB | 0 MB (in `assets/manifests/gamebuilder.lock.json`) | **0 MB** |
| **Total net working-tree delta** | — | — | **~-570 MB** |

---

## 5. Phased Execution Roadmap

### Phase 0 — Pre-Flight & Owner Sign-Offs (Day 1)
- [ ] In `~/work/gamebuilder`: inspect and commit (or discard) uncommitted `scripts/web-deploy.sh` and untracked `docs/claude-ui-spec/tokens.css`.
- [ ] Owner sign-off on **D1** (confirm `docs/business/` stays private outside public `openagents`).
- [ ] Owner provisions durable no-TTL bucket `gs://openagents-assets` + public read CDN `https://assets.openagents.com/sha256/` (unblocking `#13844`, tracked in `NEEDS_OWNER.md`).

### Phase 1 — Build Unified Asset Pipeline & Close `#13844` (Days 2–3)
- [ ] Implement `coax/web/assets-sync.coax` (with `#test` checks for manifest parsing, SHA-256 verification, fail-closed mismatch refusal, and APFS clone/copy materialization).
- [ ] Upload `apps/openagents-web/realms/**/*.pack{,.gz}`, large `psionic/fixtures/`, and GameBuilder's `village.pack` + `scrappers.gbm` to `gs://openagents-assets/sha256/`.
- [ ] Commit `assets/manifests/{realms,psionic-fixtures,gamebuilder}.lock.json`, wire `assets-sync` into `apps/openagents-web/build.sh`, `psionic/check.sh`, and `coax/remote/host-job.sh`, `git rm` the externalized blobs, and verify fresh-worktree builds pass.
- [ ] Add binary-size guard to `coax/web/prepush.coax`. Close `#13844`.

### Phase 2 — Fold GameBuilder into `apps/gamebuilder/` (Days 4–7)
- [ ] Copy GameBuilder code/tests/docs/deploy into `apps/gamebuilder/` and `docs/gamebuilder/` per **D2** (omitting `vendor/`, `openagents.lock`, `scripts/pin.sh`, `GameBuilder.xcodeproj`, `Generated/`, `build/`, `docs/business/`, and externalized binaries).
- [ ] Rewrite all 336 `vendor/openagents/` paths to repo-relative paths (`../../coax/...`, `../../metaverse-engine/...`, etc.).
- [ ] Add `gamebuilder` bounded context to `contexts.txt` and ensure `coax/web/tool.sh layer-check` passes with 0 new allowlist entries.
- [ ] Tag/port all 33 non-Coax scripts in `apps/gamebuilder/` so `coax/web/tool.sh tools-check` passes with 0 new `tools-port.txt` entries.
- [ ] Register GameBuilder Coax suites in `coax/web/suites.coax`.
- [ ] End-to-end verification of every target from `openagents` `main`:
  - `apps/gamebuilder/scripts/check.sh` (all Coax checks + command goldens)
  - `apps/gamebuilder/cli/build.sh` → `~/.openagents/bin/gamebuilder`
  - `apps/gamebuilder/web/build.sh` + `scripts/web-deploy.sh` dry-run / staging against `gamebuilder.org`
  - `xcodegen generate` + `xcodebuild` for `GameBuilder-iOS` and `GameBuilder-macOS`
  - `apps/gamebuilder/linux/build.sh` on a Linux build host

### Phase 3 — Converge Shared Surfaces & Retire Legacy Duplicates (Weeks 2–3)
- [ ] Converge `apps/gamebuilder/deploy/api/` with `coax/web/api/` (which already owns `gamebuilder-agents.c`, `worldbuilder.c`, `realms.c`, `worlds.c`).
- [ ] Converge `apps/gamebuilder/apple/` & `linux/` host glue onto `apps/shared/host/{apple,linux}/`.
- [ ] Deduplicate `coax/lib/json/json.coax` vs `coax/lib/encoding/json.coax`.
- [ ] Retire `apps/openagents-web/worldbuilder*` and `apps/openagents-web/realms/create-world/` once `apps/gamebuilder/web/` serves that flow.
- [ ] Wire `gamebuilder` CLI/MCP + inline 3D world pane into `apps/openagents-desktop/` per [docs/desktop/2026-11-20-gamebuilder-3d-worlds-audit.md](../desktop/2026-11-20-gamebuilder-3d-worlds-audit.md).

### Phase 4 — Deprecate `~/work/gamebuilder` (After First Production Deploy from `openagents`)
- [ ] Verify production `gamebuilder.org` web, API, and TestFlight iOS/macOS builds deploy cleanly from `openagents/apps/gamebuilder/`.
- [ ] Replace `~/work/gamebuilder/README.md` with a deprecation banner linking to `OpenAgentsInc/openagents` (`apps/gamebuilder/`).
- [ ] Archive `OpenAgentsInc/gamebuilder` on GitHub as read-only and remove local `~/work/gamebuilder` checkout.
