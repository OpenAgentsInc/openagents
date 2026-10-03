# Paseo worktree handling: gap analysis against Coder

Date: 2026-10-03. Paseo read at `getpaseo/paseo` `main` `5293ddac3`, cloned read-only at
`~/work/projects/repos/paseo`. OpenAgents read at `main` `5a7b318a53`/`e0d1e6bbf6`.
Paseo paths below are relative to the Paseo repo; ours are relative to this repo. Claims
marked *inferred* were reasoned from code rather than read directly.

## Summary

Paseo treats a worktree as a **first-class, long-lived object with a lifecycle**:
create (with a named branch) → background setup from a committed `paseo.json` → agents
attach → review/merge/PR → **archive** (teardown script, `worktree remove --force`,
`prune`) → **restore** (branch kept, worktree recreated). Coder treats a worktree as a
**disposable side effect of a task**: detached, unconfigured, never removed by the task
lifecycle, and only reclaimed if an off-by-default background rule happens to run.

**Emulate:**

1. **An explicit end-of-life step tied to the task lifecycle** (Paseo's "archive"), run
   on land/stop/abandon, with a dirty/unpushed check in front of it and a restore path
   behind it. This is the direct fix for our disk-filling pain.
2. **A committed per-repo worktree config** (`worktree.setup` / `teardown`, env vars
   like `PASEO_SOURCE_CHECKOUT_PATH`, `PASEO_WORKTREE_PORT`) so a worktree can be made
   buildable (env files, deps, ports) by the repo rather than by Coder code.
3. **Fetch the exact base ref right before creating** with a short timeout and a cached
   fallback, *without* adding a fetch refspec (Paseo #5788).
4. **A list/archive/restore surface** for worktrees in the terminal and app.

**Do not emulate:**

- Paseo's lack of disk/GC awareness (no quota, no scheduled GC) — our slot leasing,
  64 GiB target budget, and `git::removable` safety check are ahead of it.
- Archive that never checks dirtiness on the server/CLI (only the app warns).
- Per-worktree cold `npm ci` with no cache sharing; our shared warm Cargo slots and
  spares are the right model for a 26k-file Rust repo.
- Branch-per-worktree as a requirement. Our detached worktrees plus `HEAD:refs/heads/…`
  pushes are compatible with the #10247 source guard (which forbids ref writes from inside
  the worktree); Paseo-style local branches would need the branch created outside the
  guard. Keep detached, but *record* the intended branch/base in metadata like Paseo does.
- Leaving refspecs in `remote.origin.fetch` (Paseo's open diagnose-5556 bug).

## Paseo in brief (cited)

- **Location/naming.** `<worktrees.root | $PASEO_HOME/worktrees>/<8-char base36 hash of
  repo root>/<slug>`, slug from `mnemonic-id` (e.g. `tidy-fox`), `-1`, `-2` suffix on
  collision (`packages/server/src/utils/worktree.ts:838-873`, `:1231-1237`;
  `packages/server/src/server/worktree-core.ts:84`). Ownership is decided by path shape
  (`worktree.ts:964-991`); per-worktree metadata lives at `<gitdir>/paseo/worktree.json`
  (`packages/server/src/utils/worktree-metadata.ts:162`).
- **Branch policy.** Always a real branch, never detached: branch-off uses
  `-b <branch> --no-track <base>` (`worktree.ts:1383`), so a first push can't land on the
  base (#5249). Checkout-branch and checkout-PR modes exist (`worktree.ts:1402-1467`,
  `:1535-1568`). Placeholder branch names may be renamed from AI-generated metadata after
  the first prompt (`server/paseo-worktree-service.ts:165-237`). "Always create a fresh
  worktree" (b2ff359b6) — never reused.
- **Base freshness.** App defaults to the upstream ref (`packages/app/src/screens/new-workspace-picker-item.ts:160-178`).
  `refreshRemoteTrackingBaseRef` fetches only that one branch, 15 s timeout, falls back to
  the cached ref, and deliberately adds no refspec (`worktree.ts:1706-1739`). A local
  `main` base is never fetched; docs say use `origin/main` (`public-docs/worktrees.md:58`).
  Background `git fetch origin --prune` every 180 s while a workspace is observed
  (`server/workspace-git-service.ts:82`, `:2186`).
- **Setup.** `paseo.json` `worktree.{setup,teardown,terminals,servicePorts}`
  (`packages/protocol/src/paseo-config-schema.ts:46-53`). The *worktree's committed* copy
  is authoritative; the source's is copied in only if the ref has none
  (`worktree.ts:810-820`, c190fed27). Setup runs in the background after create, streamed
  and abortable, worktree kept on failure (`server/worktree-session.ts:658-693`,
  `:811-929`; `worktree.ts:642-703`). Env: `PASEO_SOURCE_CHECKOUT_PATH`,
  `PASEO_WORKTREE_PATH`, `PASEO_BRANCH_NAME`, `PASEO_WORKTREE_PORT` (persisted per
  worktree) (`worktree.ts:722-754`). Untrusted sources (fork PRs) don't run setup until a
  user approves (`server/workspace-automation-gate.ts:22-45`). No automatic copying of
  ignored files — the "worktree includes" feature (#2419) was reverted (c6df5ffd4); docs
  tell you to `cp "$PASEO_SOURCE_CHECKOUT_PATH/.env" .env` in setup
  (`public-docs/worktrees.md:100-113`).
- **Services/ports.** `scripts.<n>.type: "service"` gets an allocated port (explicit →
  `portScript` → range → ephemeral) and a proxy route
  `<script>--<branch>--<project>.localhost:<daemonPort>`
  (`server/workspace-service-port-allocator.ts:25-45`; `docs/development.md:415`).
- **Parallelism.** Many workspaces per repo, several may share a worktree; no per-repo
  lock around `worktree add` (collisions handled by suffixing). All git processes go
  through a global scheduler, concurrency 8 / 64 per s, configurable
  (`utils/git-process-scheduler.ts:9-45`).
- **Caches.** None shared; each worktree installs its own deps (`paseo.json`
  `worktree.setup` runs `npm ci`).
- **Results.** Diff vs. working tree or vs. stored base (`server/checkout-diff-manager.ts:154-163`);
  commit, pull, push, PR (`gh`), merge-PR, merge-to-base (conflict → error, original
  branch restored), merge-from-base (clean tree required, conflict → `merge --abort`)
  (`packages/app/src/git/policy.ts:9-24`; `utils/checkout-git.ts:3646-3827`,
  `:4048-4089`). No local rebase.
- **Cleanup.** Archive stops setup, archives agents, kills terminals, runs teardown
  (skipped for untrusted), and if Paseo-owned and unreferenced runs `worktree remove
  --force` + rm with retries + `worktree prune`; teardown failure keeps the directory
  (`server/workspace-archive-service.ts:123-190`, `:367-400`; `worktree.ts:1078-1214`).
  Branches are never deleted, which enables **restore** (prune + recreate with the old
  slug, setup not re-run) (`session/workspace-recovery/workspace-recovery-service.ts:150-187`).
  Dirty/unpushed check exists only in the app UI (`app/src/git/worktree-archive-warning.ts:4-58`);
  opt-in auto-archive on PR merge refuses dirty or ahead worktrees and is keyed to the PR
  identity (`auto-archive-on-merge/archive-if-safe.ts:63-68`; 8625f0543). No disk
  handling or scheduled GC.
- **UI.** New-workspace local/worktree picker with base picker, sidebar rows with worktree
  names, setup progress dialog, Project Settings editor for setup/teardown with an
  "uncommitted" warning, archive shortcut Cmd/Ctrl+Shift+Backspace, CLI
  `paseo workspace create|ls|rename|archive|setup`
  (`app/src/screens/new-workspace-screen.tsx:651-735`; `keyboard/keyboard-shortcuts.ts:336-362`;
  `packages/cli/src/cli.ts:158-160`).
- **Remote.** One daemon per machine owns its worktrees; clients reach it over WebSocket,
  relay, SSH or Tailscale (`docs/architecture.md:1-40`). No cross-device worktree sync.
- **Known issues.** CLI help says archive "removes worktree and associated branch"
  (`packages/cli/src/commands/worktree/index.ts:33`) but no branch delete exists; glossary
  path omits the hash dir (`docs/glossary.md:20`). Open bug on branch
  `diagnose-5556-stale-fetch-refspec`: PR-checkout adds an exact `remote.origin.fetch`
  refspec (`worktree.ts:1594-1614`); after the branch is deleted on origin, background
  `fetch --prune` exits 128 and `origin/main` stops advancing.

## Coder in brief (cited)

- **Location/naming.** `<store parent>/worktrees/<checkout-name>-<task12>`, default
  `~/.openagents/worktrees` (`crates/coder/src/task/local.rs:414-418`, `:1429-1433`).
  Always `worktree add --detach` (`local.rs:1435-1454`). Older terminal issue flow makes
  branches `coder/issue-<N>-<stamp>` under `~/.openagents/coder-one/runs/`
  (`crates/coder-delegate/src/issue.rs:50-54`, `:556-605`).
- **Base.** Chat/terminal runs use checkout `HEAD` without fetching (`local.rs:15`);
  issue flow fetches `origin/<branch>` under the shared fetch lock first
  (`crates/coder/src/task/issue_run.rs:1127-1139`; `landing.rs:224-252`, #10233).
- **Spares.** One pre-made, sealed spare per project, claimed by rename, ready only when
  no preparation lock is held (#10286), swept when stale (`crates/coder/src/task/spare.rs:36-141`,
  `:239-431`). Any ignored file disqualifies a spare (`spare.rs:16-21`).
- **Build caches.** 4 leased slots per project `~/.openagents/targets/<repo>-<hash>-slot-N`,
  64 GiB budget, LRU trim of idle slots (`crates/coder/src/task/targets.rs:9-118`,
  `:176-225`) — but only full-access runs lease one (`adapter.rs:765-769`), and issue-flow
  checks build in `~/.openagents/coder-one/target` instead
  (`crates/coder-delegate/src/issue/confined.rs:153-170`).
- **Setup.** None: no deps install, no env/ignored-file copy, no ports.
- **Landing.** Commit, then multi-machine fetch → rebase → selective re-check → plain push
  with jittered backoff, 12 attempts, conflict aborts the rebase
  (`crates/coder/src/task/landing.rs:1-89`, `:260-417`); failure soft-resets and leaves the
  change in the worktree with an issue comment naming it (`issue_run.rs:1785`, `:2050-2087`).
  PR mode pushes `coder/issue-<N>-<task8>` and runs `gh pr create` (`issue_run.rs:1858-1902`).
- **Source guard.** #10247 blocks writes to the main working tree and shared git dir
  except `.git/worktrees/<name>` and objects, via `sandbox-exec`/`bwrap`
  (`crates/coder-boundary/src/source.rs:1-212`); ref writes from inside the worktree fail
  by design (`INVARIANTS.md:46`).
- **Cleanup.** Coder removes only failed-submit worktrees and discarded spares
  (`local.rs:1171-1176`; `spare.rs:224-230`) — nothing on success, stop or failure.
  The background `disk` rule (worktrees, targets, gate pools; strong safety check
  `git::removable`: linked, clean, no non-cache ignored files, nothing unpushed, no stash)
  and the daily `worktrees` rule are both **off by default**
  (`crates/background/src/rule.rs:457-530`; `plan.rs:514-563`; `git.rs:61-194`;
  `builtins.rs:41-81`). The cleaner hardcodes `~/.openagents/{worktrees,targets}`
  (`crates/background/src/paths.rs:121-130`) while Coder derives them from
  `$OPENAGENTS_TASKS` — they diverge when that is set (*inferred*).
- **UI.** Terminal shows a run's worktree path (`crates/openagents-terminal/src/rows.rs:202`)
  and resolves paths against it; the picker has no worktree actions
  (`crates/openagents-terminal/src/picker.rs:14-21`); the app's "What changed" card diffs
  at exact revisions and offers Publish (`crates/openagents-chat-app/src/changes.rs:1-19`).
  Nothing lists, archives, or restores worktrees.
- **Remote.** Boat: a sandbox per issue, deleted after landing (`docs/cloud/boat-chat-work.md:21-87`).
  GCE pool: 2 runs per 200 GB host, host self-deletes after 10 idle min
  (`docs/cloud/gce-pool.md:61-88`). Host auto-start shares one worktree
  (`docs/coder/runtime/host-autostart.md:45-57`).
- **Disk evidence.** 1.8 TB Mac filled twice: targets 131 GB/day, worktrees 35 GB / 29,
  `coder-one/target` 85 GB, gate 55 GB (`docs/background/2026-10-02-background-processes.md:19-35`);
  coderos-4080 at 0 bytes (`docs/terminal-bench/2026-09-26-tb21-sweep.md:150`). The owner
  reports 98 GB of stale worktrees on coderos (not found in the repo).

## Side by side

| Concern | Paseo | Coder |
|---|---|---|
| Unit | Workspace (worktree may be shared, outlives agents) | Task (one worktree per task) |
| Path | `$PASEO_HOME/worktrees/<repohash>/<slug>`, root configurable | `~/.openagents/worktrees/<name>-<task12>`, follows `$OPENAGENTS_TASKS` |
| Branch | Always a named branch, `--no-track` | Detached; branch only at push time |
| Base freshness | One-branch fetch before create, 15 s fallback; background fetch 180 s | Issue flow fetches under lock; chat runs use local `HEAD`, no fetch |
| Create latency | Plain `worktree add` | Pre-sealed spare claimed by rename (faster) |
| Setup | Committed `worktree.setup`, background, abortable, env vars, trust gate | None |
| Ignored files / env | Via setup script (`$PASEO_SOURCE_CHECKOUT_PATH`); auto-includes reverted | None; spares with ignored files rejected |
| Ports / services | Allocated per worktree, persisted, proxy route | None |
| Build cache | Per worktree, cold | Shared leased slots (full access only), 64 GiB budget |
| Concurrency control | Global git process scheduler; no add lock | Fetch flock, landing flock, spare lock, task-store locks |
| Results | Diff, commit, PR, merge-PR, merge-to-base, merge-from-base; no rebase | Commit, fetch/rebase/re-check/push retry to main, or PR |
| Source protection | None found | #10247 sandbox guard |
| End of life | Archive: teardown → remove --force → prune; branch kept | No lifecycle removal; off-by-default background rules |
| Dirty-safety | App UI only; auto-archive checks | Strong `git::removable` in cleaner |
| Restore | Yes (branch kept, recreate) | Background-rule undo only |
| Disk/GC | None | Slot budget, disk rule (off by default) |
| UI | Picker, sidebar list, setup progress, archive, settings | Path in result row; diff card; no list/archive |
| Config | `paseo.json` + `config.json` | `.openagents/coder-issues.json`, env vars, code constants |

## Gaps ranked by impact

1. **No lifecycle end for task worktrees (high).** Coder never removes a worktree when a
   task lands, stops, or is abandoned; reclamation depends on background rules that ship
   off. This is the root of the stale-worktree disk fill. Paseo's archive is the model,
   but gated by our existing `git::removable` check rather than Paseo's `--force`.
2. **Cleaner and Coder disagree on paths (high, small).** `crates/background/src/paths.rs:121-130`
   hardcodes `~/.openagents/{worktrees,targets}`; Coder follows `$OPENAGENTS_TASKS`. Any
   host with a non-default store silently never gets cleaned (*inferred*).
3. **Build output outside the slot system (high).** Non-full-access runs don't lease a
   slot, and issue-flow checks use `~/.openagents/coder-one/target` (85 GB observed). Two
   cache roots per host, one unbudgeted.
4. **Failed landings strand worktrees forever (medium).** A soft-reset change is dirty, so
   `git::removable` correctly refuses it, but nothing surfaces or expires it. Paseo keeps
   the branch so archive is reversible; we keep nothing but the path in an issue comment.
5. **No per-repo setup hook (medium).** Agents can't get `.env`, local config, or ports in
   a worktree. Paseo's committed `worktree.setup` / `teardown` with source-path env vars
   is the simplest proven contract.
6. **No worktree list/archive/restore surface (medium).** Owners can't see what exists,
   how big it is, or reclaim it, from terminal or app.
7. **Chat runs branch from local `HEAD` without fetch (low-medium).** Paseo's one-branch,
   timeout-bounded, refspec-free fetch before create is cheap and avoids stale bases.
8. **Two worktree systems (low).** `coder-delegate` branched runs under `coder-one/runs`
   vs. task worktrees; separate paths, separate cleanup.
9. **Host auto-start shares one worktree (low).** Serializes tasks per workspace; fine
   for now.

## Phased recommendation (issue-sized)

**Phase 1 — stop the bleeding (disk)**

- P1.1 Make `background::Layout` derive `worktrees` and `targets` from the task store
  exactly as Coder does (share one function); test with `OPENAGENTS_TASKS` set.
- P1.2 Lifecycle removal: when a task reaches landed / PR-opened / stopped-clean, call
  `git::removable` and remove the worktree + prune immediately; record base, commit and
  intended branch in the task record so the existing undo can restore it.
- P1.3 Enable the `worktrees` rule (daily, `worktree_days` 7) by default on CoderOS and
  pool hosts; keep the `disk` rule opt-in on owner Macs.
- P1.4 Route issue-flow checks (`confined.rs` `Setup::for_run`) through the leased slot
  instead of `coder-one/target`, and lease a slot for every access mode that builds.

**Phase 2 — explicit lifecycle and surface**

- P2.1 `openagents worktree ls|archive|restore`: lists task worktrees with task state,
  size, dirty/unpushed, age; archive uses `git::removable` (or `--force` with typed
  confirmation); restore recreates detached at the recorded commit.
- P2.2 Failed-landing handling: on landing failure, commit the change to a
  `coder/stranded-<task8>` ref pushed to origin (outside the guard, in the issue-flow
  process) so the worktree becomes removable and the work is recoverable by branch.
- P2.3 Terminal and app: show worktree count/size per project and an Archive action on
  ended tasks (UI-first, per owner preference).

**Phase 3 — setup contract**

- P3.1 Add `worktree.setup` / `worktree.teardown` to `.openagents/` repo config, run in
  the background after create and before the engine starts, with
  `OPENAGENTS_SOURCE_CHECKOUT`, `OPENAGENTS_WORKTREE`, `OPENAGENTS_WORKTREE_PORT` env;
  run outside the source guard but never writing the source checkout; skip for
  untrusted sources. Spares stay ignored-file-free; setup runs after claim.
- P3.2 Before creating a chat-run worktree from a tracked branch, fetch that one branch
  with a short timeout and cached fallback, never adding a refspec (Paseo #5788 /
  diagnose-5556 lesson).

**Phase 4 — consolidation (optional)**

- P4.1 Fold `coder-delegate` branched runs onto task worktrees.
- P4.2 Consider a global git process scheduler like Paseo's
  (`utils/git-process-scheduler.ts`) if lock contention persists after the above.
