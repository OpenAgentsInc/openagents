# GameBuilder → OpenAgents Monorepo Consolidation & Unified Asset Pipeline Audit

**Date:** 2026-11-20 (revised same day per owner direction: no standalone GameBuilder / Everglade / Metaverse Engine apps)  
**Snapshots Inspected:**
- `~/work/gamebuilder` at `ea3efb0` (`main`, pinned via `openagents.lock` to `openagents` `7c8823286b8`)
- `openagents` at `a274aaf77e` (`origin/main`)

> **Related docs:** Complements [docs/desktop/2026-11-20-gamebuilder-3d-worlds-audit.md](../desktop/2026-11-20-gamebuilder-3d-worlds-audit.md) (which covers embedding `gamebuilder` CLI/MCP and 3D world panes inside OpenAgents Desktop) and [2026-11-20-codebase-bloat-and-refactor-audit.md](2026-11-20-codebase-bloat-and-refactor-audit.md) (especially `#13844` and `#13846`).

> [!IMPORTANT]
> **Owner Direction (2026-11-20) — One Desktop App, One Mobile App:** For the short term, there will be **no standalone apps** for GameBuilder, Everglade, or Metaverse Engine / AethyrForge. Everything folds into **`apps/openagents-desktop/`** (desktop) and **`apps/openagents-ios/`** (mobile), plus supporting surfaces (**`apps/openagents-web/`** and **`apps/openagents-cli/`**). This dramatically simplifies the migration: we drop `~/work/gamebuilder/host/` completely (no Xcode project, no standalone Swift/Metal/Linux shells, no separate bundle IDs) and retire `apps/everglade-desktop/`, `apps/everglade-mobile/`, and `apps/metaverse-engine-desktop/`.

---

## 1. Executive Summary

`~/work/gamebuilder` is compact (**347 tracked files**, **12.94 MB**, **166 `.coax` files** totaling 4.95 MB) and is already deeply coupled to `openagents`: it makes **336 references to 129 targets** inside `vendor/openagents/` (`coax/bin/coax`, `coax/web/tool.sh`, `metaverse-engine/*`, `components/*`, `coax/lib/*`, `apps/shared/host/linux`, `metaverse/splice-bots/*`), and `openagents` already hosts GameBuilder's server-side wire/API pieces (`coax/web/api/gamebuilder-agents.c`, `gamebuilder-agent-wire.coax`, `proto/coder/gamebuilder-agent-v1.json`).

With two owner decisions on 2026-11-20 — **(1) open-source GameBuilder** and **(2) no standalone GameBuilder / Everglade / Metaverse Engine apps** — folding `~/work/gamebuilder` into `openagents` becomes dramatically cleaner:

- **Do NOT migrate standalone native shells:** `~/work/gamebuilder/host/` (41 files, 3.48 MB: 16 Swift files, `Shaders.metal`, `Assets.xcassets`, `project.yml`, `GameBuilder.xcodeproj`, `host/linux/`, `host/panel/`, `host/page/`, `com.openagents.gamebuilder` bundle IDs) is **dropped outright**. Only GameBuilder's Coax domain logic (`core/`, `shared/`), CLI/MCP (`cli/`), tests, tools, and web/API surfaces move into a domain package **`gamebuilder/`** (just like `everglade/`, `coder/`, `metaverse-engine/`, `pay/`, `rules/`).
- **Collapse `apps/` from 8 app directories to 4 + `shared/`:** Retire `apps/everglade-desktop/` (47 files, 1.37 MB), `apps/everglade-mobile/` (35 files, 0.46 MB), and `apps/metaverse-engine-desktop/` (117 files, 0.92 MB). Everything lives in `apps/openagents-desktop/`, `apps/openagents-ios/`, `apps/openagents-web/`, and `apps/openagents-cli/` (plus `apps/shared/`).
- **Direct unblock for `#13846` & `components/zeron/`:** Those three standalone desktop/mobile shells were major callers of `apps/openagents-web/old/`, `everglade/legacy/`, and legacy `components/zeron/` chat bridges (`Workshop.swift`). Retiring them eliminates a large chunk of cross-repo coupling in one stroke.
- **Solve asset bloat in one migration (`#13844` + GameBuilder assets):** Instead of committing `village.pack` (1.84 MB) and `scrappers.gbm` (1.87 MB) into git, introduce one unified content-addressed asset pipeline (`gs://openagents-assets` + `~/.openagents/assets/sha256/` + `coax/web/tool.sh assets-sync`) covering GameBuilder assets, `openagents` realm `.pack`/`.pack.gz` bundles (358 MB), and large `psionic/fixtures/` (235 MB) — **net ~-575 MB** working-tree reduction.

---

## 2. Inventory of `~/work/gamebuilder` (at `ea3efb0`)

| Directory | Files | Size | Disposition Under No-Standalone-App Direction |
| --- | ---: | ---: | --- |
| `coax/` | 49 | 1.77 MB | **Migrate → `gamebuilder/core/`** (editor, commands, play, lobby, decide, mcp, sync, publish, world) |
| `shared/` | 5 | 0.11 MB | **Migrate → `gamebuilder/shared/`** (`buildings.coax`, `game-types.coax`, `kinds.coax`, `loop.coax`, `plays.coax`) |
| `cli/` | 11 | 0.25 MB | **Migrate → `gamebuilder/cli/`** (builds `~/.openagents/bin/gamebuilder` or `openagents gamebuilder` subcommand + MCP server) |
| `tests/` | 60 | 0.38 MB | **Migrate → `gamebuilder/tests/`** (`commands/`, `player/`, `user/`), wired into `coax/web/suites.coax` |
| `tools/` | 47 | 0.66 MB | **Migrate → `gamebuilder/tools/`** (product checks/goldens; promote generic SpliceBots/player-test helpers to `metaverse-engine/tools/`) |
| `web/` | 62 | 4.57 MB | **Migrate → `gamebuilder/web/`** (if `gamebuilder.org` web surface stays live; externalize `assets/village.pack` 1.84 MB via asset pipeline) |
| `deploy/api/` | 3 | 0.01 MB | **Migrate → `gamebuilder/deploy/api/`** short-term, then converge into `coax/web/api/` |
| `docs/` (excl. `business/`) | 27 | 1.25 MB | **Migrate → `docs/gamebuilder/`** (externalize large PNGs if needed) |
| `docs/business/` | 9 | 0.28 MB | **Exclude from public repo** (keep private unless owner approves publishing) |
| `host/` (`Sources/`, `linux/`, `Assets.xcassets`, `Models/`, `panel/`, `page/`) | 41 | 3.48 MB | **Drop standalone shells** — salvage only `Models/scrappers.gbm` (1.87 MB → asset manifest) and any reusable Coax/Metal bits needed by `openagents-desktop` / `openagents-ios` |
| `scripts/` | 20 | 0.07 MB | **Keep only `check.sh`, `web.sh`, `web-deploy.sh`, `install-cli.sh` (rewritten repo-relative)**; **drop** `pin.sh`, `env.sh`, standalone `build.sh`, `release.sh` |
| `<root>` (`openagents.lock`, `vendor/`, `project.yml`, `GameBuilder.xcodeproj`, `Generated/`, `build/`, `LICENSE`, `AGENTS.md`) | 13 | 0.11 MB | **Drop** lock/vendor/Xcode/build files; distill domain rules from `AGENTS.md` into `gamebuilder/INVARIANTS.md` |
| **Total tracked** | **347** | **12.94 MB** | **Only ~3.5–4.5 MB of pure Coax/web/docs actually enters git** |

> **Pre-flight warning (uncommitted local state in `~/work/gamebuilder`):**  
> `git status` currently shows `M scripts/web-deploy.sh` (+9/-1 lines) and untracked `docs/claude-ui-spec/tokens.css` (7,038 bytes). Commit or explicitly discard both before archiving `~/work/gamebuilder`.

---

## 3. Core Architectural Decisions

### D1. License & Public-vs-Private Split (`docs/business/`)
- **Context:** `~/work/gamebuilder/LICENSE` is proprietary, though the owner decided on 2026-11-20 to open-source GameBuilder. `~/work/gamebuilder/docs/business/` holds 9 internal strategy/economics files (`economics.md`, `phase-one.md`, `roblox-scale.md`, `comparables.md`, `cursor-for-game-development-2026-11-18.pdf`).
- **Recommendation:** Adopt `openagents`'s root `LICENSE` for all migrated code, engineering docs, and assets; **exclude `docs/business/` from public `openagents`** unless explicitly approved for public release.

### D2. Place GameBuilder as a Top-Level Domain Package (`gamebuilder/`), Not a Standalone App
Because there is **no standalone GameBuilder desktop/mobile app**, placing GameBuilder at **`gamebuilder/`** (parallel to `everglade/`, `coder/`, `metaverse-engine/`, `components/`, `pay/`, `rules/`) matches the rest of the monorepo cleanly:

- `gamebuilder/core/` — Editor state, command table (`commands.coax`), play loop, lobby, Decide, MCP server (`mcp.coax`), sync, publish, world logic.
- `gamebuilder/shared/` — `buildings.coax`, `game-types.coax`, `kinds.coax`, `loop.coax`, `plays.coax`.
- `gamebuilder/cli/` — `gamebuilder` CLI entrypoint (installed to `~/.openagents/bin/gamebuilder` or exposed via `apps/openagents-cli/` as `openagents gamebuilder ...`).
- `gamebuilder/web/` & `gamebuilder/deploy/api/` — `gamebuilder.org` web surface and Cloud Run deploy wrapper (until converged into `apps/openagents-web/` and `coax/web/api/`).
- `gamebuilder/tools/` & `gamebuilder/tests/` — Coax checks, command goldens, player tests.
- `gamebuilder/INVARIANTS.md` — Distilled from `~/work/gamebuilder/AGENTS.md` and indexed in root `INVARIANTS.md` (following `#13845`).
- `docs/gamebuilder/` — Engineering docs.

**What `apps/openagents-desktop/` and `apps/openagents-ios/` consume from `gamebuilder/`:**
- Desktop (`apps/openagents-desktop/`) runs `~/.openagents/bin/gamebuilder` (CLI + MCP) in agent chats and renders live 3D worlds/levels in the shared `components/app/pane.coax` side pane or full screen using `metaverse-engine/host/apple/PageHost.m` and `gamebuilder/core/` (per [docs/desktop/2026-11-20-gamebuilder-3d-worlds-audit.md](../desktop/2026-11-20-gamebuilder-3d-worlds-audit.md)).
- Mobile (`apps/openagents-ios/`) links `gamebuilder/core/` + `metaverse-engine/` directly into the single `com.openagents.app` iOS app — no separate `com.openagents.gamebuilder` Xcode target or TestFlight binary to ship.

### D3. Collapse `apps/` from 8 App Directories to 4 + `shared/`

| Directory | Status | Role |
| --- | --- | --- |
| `apps/openagents-desktop/` | **Keep (Canonical Desktop)** | Single macOS/Linux desktop app: chat/agents, GameBuilder 3D creation/editing, Everglade / Metaverse realms, Metaverse Engine tools |
| `apps/openagents-ios/` | **Keep (Canonical Mobile)** | Single iOS/iPadOS app (`com.openagents.app`): chat, GameBuilder 3D creation/editing, Everglade / Metaverse realms |
| `apps/openagents-web/` | **Keep (Canonical Web)** | `openagents.com` (+ `/create-world` superseded by `gamebuilder/web/`, retiring `worldbuilder*` and `realms/create-world/`) |
| `apps/openagents-cli/` | **Keep (Canonical CLI)** | `openagents` CLI (`~/.openagents/bin/openagents`) + `gamebuilder` CLI/MCP (`~/.openagents/bin/gamebuilder` or subcommand) |
| `apps/shared/` | **Keep (Shared Host Glue)** | Shared Apple/Linux host bindings (`AppAuth.swift`, `ui-host.m`, `NativeChrome.swift`, Linux host) |
| `apps/everglade-desktop/` | **Retire** (47 files, 1.37 MB) | Superseded by `apps/openagents-desktop/`; removes `Workshop.swift` coupling to legacy `zeron/` and `apps/openagents-web/old/` |
| `apps/everglade-mobile/` | **Retire** (35 files, 0.46 MB) | Superseded by `apps/openagents-ios/`; removes `apps/openagents-web/old/` coupling |
| `apps/metaverse-engine-desktop/` | **Retire** (117 files, 0.92 MB) | Superseded by `apps/openagents-desktop/`; removes `everglade/legacy/v1/` and `old/v1/` coupling + 2 `layer-context` allowlist entries |

**Bonus impact on existing issues:** Retiring `everglade-desktop`, `everglade-mobile`, and `metaverse-engine-desktop` removes a large portion of the external references blocking `#13846` (`apps/openagents-web/old/` and `everglade/legacy/`), removes 2 entries from `metaverse-engine/layer-allowlist.txt` (`apps/metaverse-engine-desktop/scene-policy.coax` and `chat.coax`), and removes the last desktop blocker on retiring `components/zeron/` (`#13847`).

### D4. Bounded Context (`contexts.txt`) & JSON Library Roles
- Register `gamebuilder` in `contexts.txt`:
  ```text
  gamebuilder folders: gamebuilder/ may load: engine protocol coder chat nostr toolchain zeron-kit splice
  ```
  and allow `apps/openagents-desktop/` (`coder` context) and `apps/openagents-ios/` to load `gamebuilder`, keeping `layer-check` at **0 new allowlist entries**.
- **JSON libraries clarified (no unnecessary rewrite):** `openagents` has two intentionally distinct JSON libraries:
  - `coax/lib/encoding/json.coax` (extracted in `#13843`) — zero-alloc RFC 8259 streaming pull parser/writer for `#authority`, wire admission, and Nostr.
  - `coax/lib/json/json.coax` (`#module "coax.json"`, 740 LOC, loaded 14× by GameBuilder) — caller-array tree/DOM reader/writer for tools.
  Document this two-tier split in `coax/lib/` rather than rewriting GameBuilder's 14 callers.

### D5. `tools-in-coax` Compliance Before Landing
`coax/remote/counts.sh` gates `tools-port` so no commit may raise it. Dropping `host/` and standalone build scripts immediately eliminates most of GameBuilder's `.sh` files. For the remaining non-Coax scripts (browser glue `web/{app,hud-glue,keep}.js`, `tools/{browser-run-record,entrance-probe}.js`, `tools/models.py`, and thin shell wrappers):
- Tag genuine browser glue / thin wrappers with `Firm reason: N (...)` per `coax/tools-in-coax.md`.
- Port or delete any remaining logic-heavy scripts (e.g., `tools/models.py`) so `tools-check` passes with **0 new `tools-port.txt` entries**.

### D6. Git History Strategy
- Do **not** raw-merge `~/work/gamebuilder` history if `docs/business/` or proprietary commits must remain private in public `openagents`.
- Land clean, logical commits directly onto `openagents` `main` (or `git filter-repo` stripped of `docs/business/`, `host/`, `.pack`, `.gbm`, `.pdf`, `.xcodeproj`), then archive `OpenAgentsInc/gamebuilder` read-only with a deprecation banner pointing to `openagents/gamebuilder/`.

---

## 4. Unified Content-Addressed Asset & Fixture Pipeline (One Migration for GameBuilder + `#13844`)

Solve **all** large binary assets across `gamebuilder/`, `apps/openagents-web/realms/`, and `psionic/fixtures/` with one standard pipeline aligned with `AGENTS.md`'s `~/.openagents/` shared-files rule:

### Architecture

1. **One Durable Content-Addressed Remote Bucket (No TTL)**
   - Provision **`gs://openagents-assets`** (separate from the 14-day-TTL `gs://openagentsgemini-coax-build-cache` build cache, in the same GCP project as `gs://openagentsgemini-gamebuilder-web`).
   - Object key layout: `gs://openagents-assets/sha256/<first2>/<sha256>` (immutable, globally deduplicated across all realms, GameBuilder, and Psionic fixtures).
   - Serve public read-only HTTPS via Cloud CDN (or Cloudflare R2) at `https://assets.openagents.com/sha256/<first2>/<sha256>` so **fresh checkouts, contributors, and remote build hosts need zero credentials to download**. Uploads use the existing automation service account.

2. **One Shared Local Cache (`~/.openagents/assets/sha256/`)**
   - Extends the `~/.openagents/assets/` directory already mandated by `AGENTS.md` ("Shared asset library").
   - Every worktree on a machine shares the same cached blobs — `git worktree add` never duplicates gigabytes of `.pack` or `.f32` files.

3. **Extend Existing `assets/registry/` Lockfiles (No Second Manifest System)**
   - `openagents` already has `assets/registry/` (`oa-collection-lock-v1.schema.json`, `oa-package-lock-v1.schema.json`, `assets/registry/collections/*.lock.json`) and `~/.openagents/bin/asset-registry`. **Do not invent a second `assets/manifests/` convention** — extend `assets/registry/collections/` with:
     - `assets/registry/collections/gamebuilder.lock.json` (`village.pack`, `scrappers.gbm`, `ghost-data` binary replacing 330 KB base64 in `web/ghost-data.coax`)
     - `assets/registry/collections/realms.lock.json` (`everglade2.pack`, `everglade-zone.pack`, `everglade-avatar.pack`, `glade.pack`, `glade.pack.gz`, `fantasy.pack`, `arkai_c.ttf`, etc.)
     - `assets/registry/collections/psionic-fixtures.lock.json` (large `psionic/fixtures/` binaries >256 KB: `original-edit/`, `deepseek/`, `robotics/`, `harmony/`, `text/`)
   - Each row pins `{ "path": "...", "sha256": "...", "bytes": N, "group": "..." }`.

4. **One Coax Hydrator Tool (`coax/web/assets-sync.coax` → `coax/web/tool.sh assets-sync`)**
   - `coax/web/tool.sh assets-sync [--group gamebuilder|realms|psionic|all] [--check]`
   - Reads `assets/registry/collections/*.lock.json`. For each entry:
     1. Checks if workspace target exists with matching size/SHA-256 (fast no-op).
     2. Checks `~/.openagents/assets/sha256/<aa>/<sha256>`; if missing, downloads from `https://assets.openagents.com/sha256/<aa>/<sha256>` into a temp file, verifies SHA-256 (fails closed on mismatch), and atomically moves into cache.
     3. Materializes into the workspace path via **APFS clonefile (`cp -c`) / regular copy** — **not symlink** — because `coax/web/serve.c:1444` explicitly rejects symlinks (`realpath(full) != full`). On macOS APFS, `cp -c` is copy-on-write (instant, zero extra disk space, regular file).
   - Invoked automatically by `gamebuilder/scripts/web.sh`, `apps/openagents-web/build.sh`, `psionic/check.sh`, and `coax/remote/host-job.sh`.
   - Externalized workspace paths are `.gitignore`d.

5. **Pre-Push Binary Bloat Guard (`coax/web/prepush.coax`)**
   - Extend the `#13840` verification guard in `coax/web/prepush.coax` to refuse newly added/modified binary assets (`*.pack`, `*.pack.gz`, `*.gbm`, `*.glb`, `*.gltf`, `*.f32`, `*.f64`, `*.mp4`, `*.ttf`, `*.otf`) above 512 KB, pointing authors to `coax/web/tool.sh assets-sync` and `assets/registry/collections/`.

### Net Size Impact of the Combined Migration
| Bucket | Before | After (in git) | Delta |
| --- | ---: | ---: | ---: |
| `apps/openagents-web/realms/` packs | 358.0 MB | ~2 MB (manifests + small HTML/MD) | **-356 MB** |
| `psionic/fixtures/` large binaries | 235.6 MB | ~15 MB (small unit fixtures + manifest) | **-220 MB** |
| Retiring `everglade-desktop`, `everglade-mobile`, `metaverse-engine-desktop` | 2.75 MB | 0 MB | **-2.75 MB** |
| `gamebuilder/` core, CLI, web, tests, tools + `docs/gamebuilder/` (no `host/`) | 0 MB | ~4.0 MB | **+4.0 MB** |
| GameBuilder binary assets (`village.pack`, `scrappers.gbm`, `ghost-data`) | 0 MB | 0 MB (in `assets/registry/collections/gamebuilder.lock.json`) | **0 MB** |
| **Total net working-tree delta** | — | — | **~-575 MB** |

---

## 5. Phased Execution Roadmap

### Phase 0 — Pre-Flight & Owner Sign-Offs (Day 1)
- [ ] In `~/work/gamebuilder`: commit or discard uncommitted `scripts/web-deploy.sh` and untracked `docs/claude-ui-spec/tokens.css`.
- [ ] Owner confirms **D1** (`docs/business/` stays private outside public `openagents`) and whether `gamebuilder.org` web/API deploys stay live during transition (see Section 6.2).
- [ ] Owner provisions durable no-TTL bucket `gs://openagents-assets` + public read CDN `https://assets.openagents.com/sha256/` (unblocking `#13844`, tracked in `NEEDS_OWNER.md`).

### Phase 1 — Build Unified Asset Pipeline & Close `#13844` (Days 2–3)
- [ ] Implement `coax/web/assets-sync.coax` (with `#test` checks for `assets/registry/collections/*.lock.json` parsing, SHA-256 verification, fail-closed mismatch refusal, and clone/copy materialization).
- [ ] Upload `apps/openagents-web/realms/**/*.pack{,.gz}`, large `psionic/fixtures/`, and GameBuilder's `village.pack` + `scrappers.gbm` to `gs://openagents-assets/sha256/`.
- [ ] Commit `assets/registry/collections/{realms,psionic-fixtures,gamebuilder}.lock.json`, wire `assets-sync` into `apps/openagents-web/build.sh`, `psionic/check.sh`, and `coax/remote/host-job.sh`, `git rm` the externalized blobs, and verify fresh-worktree builds pass.
- [ ] Add binary-size guard to `coax/web/prepush.coax`. Close `#13844`.

### Phase 2 — Fold GameBuilder into `gamebuilder/` (Days 4–6)
- [ ] Copy GameBuilder `core/`, `shared/`, `cli/`, `web/` (if kept), `deploy/api/`, `tools/`, `tests/`, and `docs/gamebuilder/` into `openagents` (omitting `host/`, `web/host/`, `vendor/`, `openagents.lock`, `scripts/pin.sh`, `project.yml`, `GameBuilder.xcodeproj`, `Generated/`, `build/`, `docs/business/`, and externalized binaries).
- [ ] Rewrite all `vendor/openagents/` paths to repo-relative paths (`../coax/...`, `../metaverse-engine/...`, etc.).
- [ ] Add `gamebuilder` bounded context to `contexts.txt`; verify `coax/web/tool.sh layer-check` passes with 0 new allowlist entries.
- [ ] Tag/port remaining non-Coax scripts so `coax/web/tool.sh tools-check` passes with 0 new `tools-port.txt` entries.
- [ ] Register `gamebuilder/` Coax suites in `coax/web/suites.coax` and build `~/.openagents/bin/gamebuilder` from `apps/openagents-cli/build.sh` (or expose `openagents gamebuilder` + symlink).
- [ ] Verify `gamebuilder/scripts/check.sh` (all Coax checks + command goldens), CLI/MCP (`gamebuilder mcp`), and `gamebuilder/web/` build (if kept).

### Phase 3 — Embed in OpenAgents Desktop/iOS & Retire Standalone Shells (Weeks 2–3)
- [ ] Wire `gamebuilder` CLI/MCP + live 3D world side pane into `apps/openagents-desktop/` (per [docs/desktop/2026-11-20-gamebuilder-3d-worlds-audit.md](../desktop/2026-11-20-gamebuilder-3d-worlds-audit.md)) and 3D world creation/play into `apps/openagents-ios/`.
- [ ] Retire `apps/everglade-desktop/`, `apps/everglade-mobile/`, and `apps/metaverse-engine-desktop/` (unblocking `#13846` and `#13847`).
- [ ] Move the 10 `tools/splicebots-*` campaign files into `metaverse/splice-bots/` (or archive), converge `gamebuilder/deploy/api/` into `coax/web/api/`, and retire `apps/openagents-web/worldbuilder*` + `realms/create-world/`.

### Phase 4 — Deprecate `~/work/gamebuilder` (After First Production Deploy from `openagents`)
- [ ] Verify `gamebuilder` CLI/MCP, desktop/iOS integration, and `gamebuilder.org` (if kept) ship cleanly from `openagents` `main`.
- [ ] Replace `~/work/gamebuilder/README.md` with a deprecation banner linking to `OpenAgentsInc/openagents` (`gamebuilder/`).
- [ ] Archive `OpenAgentsInc/gamebuilder` on GitHub as read-only and remove local `~/work/gamebuilder` checkout.

---

## 6. Additional Simplifications Found on Second Pass

1. **One Asset Registry, Not Two (`assets/registry/`):** `openagents` already has `assets/registry/` with `oa-collection-lock-v1.schema.json` and 21 `assets/registry/collections/*.lock.json` files. Extending `assets/registry/collections/` directly avoids inventing a parallel `assets/manifests/` directory and keeps one schema + one `~/.openagents/assets/` cache.
2. **Eliminate 12,789 LOC of Duplicate Web UI If `gamebuilder.org` Is Not Kept as a Separate Portal:** `~/work/gamebuilder/web/` contains `hud.coax` (5,963 LOC), `page.coax` (3,197 LOC), `chat.coax` (1,405 LOC), `agents.coax` (924 LOC), `account.coax` (910 LOC), and `login.coax` (390 LOC) — essentially a second copy of OpenAgents login/account/chat/agents on the web, plus an untracked 58-file copy of `coax/web/*.js` in `web/host/` and a 330 KB base64-baked Quaternius mesh in `web/ghost-data.coax` (generated by `tools/ghost-bake.coax`).
   - **If `gamebuilder.org` redirects to `openagents.com` (or becomes a thin landing/download page):** We skip migrating `web/` entirely (**-4.57 MB, -62 files, -12.8k LOC** of duplicate web shell).
   - **If `gamebuilder.org` stays live:** Serve `coax/web/` directly without copying `web/host/` and move `ghost-data` out of base64 `.coax` into `assets/registry/collections/gamebuilder.lock.json`.
3. **`deploy/api/` Is Just a 10-Line Wrapper Around `gamebuilder serve`:** `deploy/api/run.sh` runs `gamebuilder serve --accounts "$GB_ACCOUNTS" --origin "$GB_ORIGIN" --port 8811` behind `socat` — there is no separate Postgres schema or C backend. In the monorepo, `deploy/api/deploy.sh` no longer needs its two-repo staging dance (`gamebuilder/` + `openagents-gamebuilder/` at `openagents.lock`); it just builds the `gamebuilder` CLI from `openagents` `main` (and can be retired completely once Desktop/iOS talk directly to `coax/web/api/`).
4. **One CLI Build/Install Path (`apps/openagents-cli/build.sh`):** Instead of maintaining a separate CLI build/install pipeline for GameBuilder, have `apps/openagents-cli/build.sh` build both `~/.openagents/bin/openagents` and `~/.openagents/bin/gamebuilder` (or expose `openagents gamebuilder` with a `gamebuilder` symlink) so developers and `apps/openagents-desktop/install.sh` keep one binary install step.
5. **Fold `tools/splicebots-*` (10 files) into `metaverse/splice-bots/`:** `openagents` already owns `metaverse/splice-bots/` (201 files, 5.84 MB) and `apps/openagents-web/realms/splice-bots/`. Keeping GameBuilder's 10 `splicebots-*` porting-campaign files inside `metaverse/splice-bots/` (or archiving them if the port is finished) avoids splitting SpliceBots across two top-level directories.
6. **Keep Both JSON Libraries With Documented Roles:** `coax/lib/encoding/json.coax` (pull parser for `#authority` / wire) and `coax/lib/json/json.coax` (`#module "coax.json"`, tree DOM for tools) serve complementary roles — no need to churn GameBuilder's 14 callers.
