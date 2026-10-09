# Repository size: why a clone is ~9 GB, and the fix

Measured 2026-10-09 on a fresh `git clone --bare` of
`OpenAgentsInc/openagents` (heads + tags, which is what a normal clone
fetches), in a throwaway copy. Nothing on the real repo or remote was
changed.

## Cloning

Fast clone (history blobs download only when a command needs them):

    git clone --filter=blob:none https://github.com/OpenAgentsInc/openagents

Bench captures, record archives and large reports are no longer in the
tree (#11110). They live in the private bucket
`gs://openagents-bench-artifacts` (one object per sha256, never
overwritten or deleted), and each run directory keeps a
`bench-artifacts.json` manifest plus a `.gitignore` for those files.
Bring them back (gcloud must be authenticated):

    scripts/bench-artifacts.py restore                 # everything under bench/
    scripts/bench-artifacts.py restore bench/verse/2026-10-04/battle-scale-sustained

New run output goes the same way: `scripts/bench-artifacts.py push RUN_DIR`
uploads captures and files over 1 MB and writes the manifest. Enable the
large-file check once per clone with `git config core.hooksPath .githooks`
(it runs `scripts/dev/check-large-files.sh`; allowlist in
`scripts/dev/large-files-allowlist.txt`).

## Where the 9 GB comes from

| Part of a normal clone | Size |
| --- | --- |
| Packed history (`.git`, `size-pack`) | 3.95 GiB (482k objects, 191 s to clone) |
| Checked-out working tree at `main` | 4.83 GB (39,296 files) |
| **Total on disk after `git clone`** | **~8.8 GB** |

The owner's long-lived checkout is bigger still (8.18 GiB pack) because it
also holds local agent branches, worktree refs and unpruned objects.

## It is new, and it is one directory

Blobs reachable from `main`, by date:

| Date | Pack weight |
| --- | --- |
| 2026-03-01 | 0.20 GB |
| 2026-09-01 | 0.57 GB |
| 2026-09-25 | 0.88 GB |
| 2026-10-01 | 1.33 GB |
| 2026-10-04 | 1.83 GB |
| 2026-10-07 | 3.52 GB |
| 2026-10-09 | ~4.2 GB (all refs) |

Three years of history fit in 0.57 GB. About 3.5 GB arrived in the last
three weeks, almost all of it benchmark and capture output under `bench/`.

### By top-level path (compressed bytes across all history)

| Path | Size |
| --- | --- |
| `bench/verse` | 1,934 MB |
| `assets/verse` | 617 MB |
| `bench/terminal-bench` | 525 MB |
| `bench/wow` | 248 MB (deleted from HEAD; history only) |
| `apps/openagents.com` | 103 MB |
| `docs/mobile` | 44 MB |
| `crates/psionic` | 39 MB |
| `bins/openagents-ios` | 35 MB |
| everything else | < 35 MB each |

`bench/` is 2.7 GB of the 4.2 GB pack (64%). In the working tree it is
4.16 GB of the 4.83 GB checkout (`bench/verse/2026-10-04..06` alone is
2.5 GB, `bench/terminal-bench/traces` 0.86 GB).

### By file type (compressed, all history)

| Ext | Compressed | Raw | Files |
| --- | --- | --- | --- |
| png | 1,146 MB | 1,164 MB | 2,975 |
| mp4 | 1,108 MB | 1,114 MB | 118 |
| gz / tar.gz | 458 MB | 458 MB | 361 |
| json | 326 MB | 4,510 MB | 25,293 |
| vtp (Verse terrain) | 315 MB | 540 MB | 42 |
| jsonl | 161 MB | 570 MB | 2,447 |
| rs | 108 MB | 3,278 MB | 52,765 |
| ts | 74 MB | 1,817 MB | 45,813 |
| md | 69 MB | 915 MB | 39,249 |
| glb | 53 MB | 89 MB | 496 |
| wasm | 33 MB | 179 MB | 44 |

Already-compressed media (png, mp4, gz, vtp) do not delta or compress, so
they cost full size forever. Source code is cheap: all `.rs` across history
is 108 MB. JSON reports are cheap in the pack but heavy in the checkout
(single `report.json` files of 25-45 MB).

2,313 blobs over 1 MB make up 3.04 GB of the pack; 170 blobs over 10 MB
make up 1.67 GB.

### Largest blobs

| Size | Path | First / last commit | In HEAD |
| --- | --- | --- | --- |
| 57.0 MB | `bench/wow/2026-10-03/verse-smooth-combat.mp4` | 0e676ecea 10-03 / 93fe80568 10-04 | no |
| 56.4 MB | `bench/wow/2026-10-03/verse-agent-combat.mp4` | 07527c5d1 10-03 / 93fe80568 10-04 | no |
| 51.4 MB | `bench/terminal-bench/experiments/2026-09-25-candidate-review/records/prospective-astra-traces.tar.gz` | 5da7665b3 09-25 | yes |
| 50.2 MB | `.../records/prospective-traces.tar.gz` | a2730fa5e 09-24 | yes |
| 45.2 MB | `bench/verse/2026-10-06/battle-writer-trace-600/report.json` (+4 siblings at 43-45 MB) | 4b90b1989 10-06 | yes |
| 37.7 MB x5 | `bench/verse/2026-10-04/{audio,animation-graphs}/*/audio.wav.gz` | fc5a704cf etc. 10-04 | yes |
| 32.9 MB | `bench/wow/2026-10-03/verse-dark-combat.mp4` | 3905133a2 10-03 / 93fe80568 10-04 | no |
| 31.8 MB | `bench/verse/2026-10-04/battle-scale-sustained/primary.mp4` (+ ~10 more 18-28 MB `primary.mp4`) | fa13a404e 10-04 | yes |
| 22.4 MB | `assets/verse/everglade/2c6d0e58...vtp` | | yes |
| 18.9 MB | `crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v2/checkpoint_state.json` | | yes |

### HEAD vs history

| | Blobs | Compressed |
| --- | --- | --- |
| Reachable from current HEAD | 32,725 | 2.38 GB |
| History only (deleted or overwritten) | 192,281 | 1.81 GB |

So even a history rewrite that kept every current file would leave a
2.4 GB pack plus a 4.8 GB checkout. The checkout is the bigger problem.

### Branches, tags and PR refs are not the cause

44 heads and tags besides `main`. Objects not reachable from `main`
total 358 MB, and 357 MB of that is one branch,
`codex/10936-10937-temporal-debris` (temporal-AA audit PNGs under
`bench/verse/2026-10-08/`). Every other branch or tag pins under 18 MB
(`codex/coast-kit` 17 MB; the rest under 2 MB).

The remote also carries 76 `refs/remotes/origin/*` refs (old April-era
branches pushed as remote-tracking refs) and ~3,060 GitHub
`refs/pull/*` refs. Neither is fetched by a normal clone; `--mirror` does
fetch them.

Stale branch candidates (list only; none deleted): the four
`coder/stranded-*`, the 13 `devin/*`, `wip/everglade-b2-doc`,
`wip/everglade-medieval-p9`, `issue-9996-desktop-chat`,
`meteor-swarm-visuals`, `spell-physics-10451`, merged `codex/*`
everglade/water branches, and all 76 `refs/remotes/origin/*` refs.
Deleting them saves at most ~0.4 GB (mostly the temporal-debris branch).

## Plan, safest first

### A. No history change (do now)

1. **Stop adding captures to git.** New rule: `bench/**` run output
   (mp4, png, wav.gz, tar.gz, report JSON over 1 MB, traces) goes to a
   bucket, and git keeps a small manifest per run (path, bytes, sha256,
   URL), following the pinned-artifact pattern already used by
   `scripts/fetch-kev-artifacts.py`. Writers to change first:
   `crates/verse/examples/battle_scale.rs`,
   `crates/verse-pbr/examples/water_capture.rs`, `scripts/grid-soak.sh`,
   the terminal-bench recorders and `crates/gym` tests that write under
   `bench/`.
2. **Large-file gate.** A pre-commit hook plus a check script
   (`scripts/check-large-files.sh`) that rejects any added blob over
   1 MB unless its path is on an allowlist (`assets/verse/**`, `bins/**`
   app icons, fonts). Agents commit through hooks, so this catches them.
3. **Git LFS for assets that must live with the code.** Add
   `.gitattributes` routing `*.vtp *.glb *.gltf *.ktx2 *.mp4 *.wav *.npy
   *.tar.gz` under `assets/**` to LFS. Note that `git-lfs` is not
   installed on this Mac or CoderOS today, and LFS bandwidth on GitHub is
   metered; the bucket plus manifest route is preferred for `bench/`.
4. **Move current `bench/` captures out of the tree** in one ordinary
   commit (upload to the bucket, leave manifests). That cuts the checkout
   from 4.8 GB to ~0.7 GB immediately for everyone, with no hash changes.
   History stays 4 GB.
5. **Cheaper clones today:**
   `git clone --filter=blob:none https://github.com/OpenAgentsInc/openagents`
   (fetches history blobs lazily; pack drops to roughly the HEAD set) or
   `--depth 1` for CI and CoderOS throwaway checkouts. Combine with
   sparse checkout (`git sparse-checkout set --no-cone '/*' '!/bench/'`)
   to skip the captures.
6. **Prune stale refs** listed above after owners confirm.

### B. History rewrite (only with a freeze window)

Dry run with `git filter-repo` 2.47 on the throwaway bare clone
(`git gc --aggressive` after each step):

| Scenario | Pack | Notes |
| --- | --- | --- |
| Today | 3.95 GiB | |
| Drop `bench/` from all history (`--path bench/ --invert-paths`) | **1.53 GiB** | 177 s; captures must already be in the bucket |
| Also strip every blob > 1 MB (`--strip-blobs-bigger-than 1M`) | **0.80 GiB** | stand-in for `git lfs migrate import --everything` on media; HEAD tree becomes 455 MB |

Recommended target: drop `bench/` history, and migrate `assets/verse`
media plus `bins/**` to LFS (`git lfs migrate import --everything
--include='assets/**/*.vtp,assets/**/*.glb,assets/**/*.png,bins/**'`).
Expected clone: ~0.8 GB pack plus ~0.5 GB checkout, versus ~8.8 GB today.

What a rewrite breaks:

- Every commit hash on `main` after 2023-11 changes. All existing clones,
  the owner's checkout and every `.claude/worktrees/agent-*` worktree
  diverge and must be re-cloned (rebasing onto rewritten history is
  error-prone with this many files).
- All 44 remote branches/tags must be rewritten or deleted; open PR #11088
  (`proposal/non-custodial-agent-commerce`) must be rebased and
  force-pushed by its author or recreated.
- Commit hashes cited in docs (71 docs cite `commit <sha>`), issue and
  PR comments, `NEEDS_OWNER.md`, release notes, Coder tags
  (`coder-one-*`), and receipts/fixtures that pin a source SHA stop
  resolving on `main` (GitHub keeps old objects reachable by SHA URL for a
  while, but not via clones).
- CoderOS host checkouts and any deployed build that reports a git SHA.
- Closed PRs keep pointing at old commits; GitHub `refs/pull/*` cannot
  be rewritten by us, so the old objects stay on GitHub's side until
  GitHub support runs a gc.

Coordination steps:

1. Do section A first (bucket, manifests, hooks); the rewrite is useless
   if captures keep landing.
2. Announce a freeze window; stop Coder/Codex/Devin batches and agent
   worktrees; merge or close PR #11088.
3. Publish an old-to-new SHA map (filter-repo writes
   `filter-repo/commit-map`) to the repo as `docs/repo/commit-map.txt`
   so cited hashes stay traceable.
4. Run filter-repo on a fresh `--mirror` clone, verify `cargo check`
   and the web build on the rewritten `main`, then force-push all
   heads and tags in one step.
5. Ask GitHub support to gc the repo so the old objects actually leave.
6. Everyone re-clones (`git clone --filter=blob:none ...`); delete old
   worktrees; re-clone on CoderOS.

Recommendation: do A now (it fixes the checkout and stops growth with no
breakage), and schedule B only when there is a natural quiet window.
