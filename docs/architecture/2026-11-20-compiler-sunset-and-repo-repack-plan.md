# Compiler Sunset Plan, Realm Tiering & `.git` Repack Strategy

**Date:** 2026-11-20  
**Issue:** `#13851`  
**Context:** Follow-on to [docs/audits/2026-11-20-codebase-bloat-and-refactor-audit.md](../audits/2026-11-20-codebase-bloat-and-refactor-audit.md).

---

## 1. Coax Compiler Sunset Plan (`coax/src/` vs `coax/bootstrap/` vs `coax/lib/compiler/`)

Three compiler implementations currently coexist in `coax/` (**164,746 LOC** combined):

| Path | Language | Size | Role Today | Target Fate |
| --- | --- | ---: | --- | --- |
| `coax/src/` | C++20 (`coax0`) | 24 files, 22,627 LOC | Production compiler (`coax0 0.52.0`) | **Retire at Stage-3 Self-Host Milestone** |
| `coax/bootstrap/` | C11 | 23 files, 33,382 LOC (`checker.c` = 21,916 LOC) | C11 bootstrap compiler | **Split `checker.c` if kept as seed, else retire once Stage-2 C seed is reproducible** |
| `coax/lib/compiler/` | Coax | 72 files, 108,737 LOC | Self-hosted compiler | **Canonical compiler** |

### Verifiable Exit Criteria to Retire `coax/src/` (`coax0`)
`coax/src/` may be deleted only when **all five gates hold simultaneously** on `main` (Linux x86_64 + macOS arm64):
1. **3-Stage Fixpoint:** `stage1` (built by `coax/bootstrap/` or pinned release binary) compiles `coax/lib/compiler/` → `stage2`; `stage2` compiles `coax/lib/compiler/` → `stage3`; the emitted C of `stage2` and `stage3` is **byte-for-byte identical** (`shasum -a 256` equal).
2. **Full Coax Suite Parity:** `coax/tests/run.sh --part 1/8` through `8/8` passes 100% under `stage2` with zero skipped suites.
3. **Product & Host Build Parity:** `apps/openagents-web/build.sh`, `apps/openagents-cli/build.sh`, `gamebuilder/cli/build.sh`, `apps/openagents-desktop/build.sh`, `apps/openagents-ios/`, and `psionic/check.sh` all build and pass under `stage2`.
4. **WGSL & Determinism Parity:** `coax/web/tool.sh wgsl-module --site --check` passes with zero diff against `coax0`-emitted shaders.
5. **Mutation Gate Parity:** `coax mutate --changed origin/main` matches `coax0` mutant counts on `#authority` suites (`pay/`, `coax/lib/encoding/`, `coax/web/api/*-policy.coax`).

### `coax/bootstrap/checker.c` Decomposition Rule
If `coax/bootstrap/` is retained as the minimal C11 bootstrap seed after `coax/src/` retires, split `coax/bootstrap/checker.c` (21,916 LOC) into focused compilation units (`checker_types.c`, `checker_expr.c`, `checker_borrow.c`, `checker_effects.c`, `checker_emit.c`) so no bootstrap C file exceeds 4,000 LOC.

---

## 2. Production vs Lab Realm Tiering (`apps/openagents-web/realms/tiers.json`)

All 49 directories under `apps/openagents-web/realms/` are explicitly classified in [`apps/openagents-web/realms/tiers.json`](../../apps/openagents-web/realms/tiers.json) into four tiers:
- **`production`:** Shipped realms built and checked by default (`everglade2`, `character-creator`, `the-grid`, `belt-2200`, `orbit-league`, `dead-gears`, `hold-the-stair`, `haunted-house`, `hecates-cemetery`, `ice-cream-man`, `fairy-forest`, `splice-bots`, `one-shot`, `garden`, `characters`, `watchtower`, `crew`, `drop`, `new`, `built`, `druid`, `archery-range`, `painted-village`, `penultimate-frontier`, `slab`, `valley-world`, `valley-canyon`).
- **`lab`:** Experimental R&D sandboxes built only under `COAXIAL_PAGES=all` (`5e-lab`, `animation-lab`, `anime-lab`, `audio-lab`, `bitcoin-lab`, `coax-lab`, `dragon-lab`, `metahuman-lab`, `nostr-lab`, `portal-lab`, `sandbox`, `splat-lab`, `state-lab`, `valley-lab`, `waterlab`).
- **`legacy`:** Superseded v1/deprecated realms scheduled for removal (`everglade`, `everglade-classic`, `everglade-experimental`, `5e-mmo`, `create-world`, `splice-bots-jam`).
- **`fixture`:** Automated test harness realm (`fixture`).

Build scripts (`apps/openagents-web/build.sh`) and release checks (`coax/remote/check.sh`) should treat `production` as the default release set, `lab` as opt-in (`COAXIAL_PAGES=all`), and `legacy` as frozen for deletion (`#13846`, `#13856`).

---

## 3. Post-Externalization `.git` Repack & History Cleanup Runbook

P0 (`#13840`, `#13841`) and Phase 1 (`#13853`, `#13844`) removed **~1.75 GB** of tracked verification dumps, videos, `.pack`/`.pack.gz` bundles, and `psionic/fixtures/` blobs from `main`, and `coax/web/prepush.coax` now blocks re-introduction of raw verification dumps and binary assets `>512 KiB`. However, `.git` remains **~9.4 GB** because historical commits still reference those blobs.

### Scheduled Cleanup Window (Requires Owner Sign-Off & Brief Push Freeze)
1. **Archive Tag:** Push an immutable backup tag `archive/pre-repack-2026-11` to a private backup mirror (`openagents-archive`) so full pre-repack history remains inspectable.
2. **Strip Historical Blobs on a Fresh Bare Mirror:**
   ```sh
   git clone --mirror git@github.com:OpenAgentsInc/openagents.git openagents-repack.git
   cd openagents-repack.git
   git filter-repo --strip-blobs-bigger-than 2M \
     --path-glob 'docs/verification/**/*.tar.gz' \
     --path-glob 'docs/verification/**/*.tar.xz' \
     --path-glob 'docs/verification/**/*.mp4' \
     --path-glob 'docs/verification/**/*.f32' \
     --path-glob 'psionic/docs/verification/**' \
     --invert-paths
   git reflog expire --expire=now --all
   git gc --prune=now --aggressive
   ```
3. **Verify Integrity & Size:** Confirm `HEAD` tree hash matches pre-repack `main` identically (`git rev-parse HEAD^{tree}`), verify `du -sh .` drops from **~9.4 GB** to **<500 MB**, and run `scripts/check.sh` on a fresh clone.
4. **Coordinate Cutover:** Announce 15-minute push freeze, force-push rewritten `main` + `main-green`, and re-clone build pool hosts (`coax/remote/remote.sh`) and local worktrees.
