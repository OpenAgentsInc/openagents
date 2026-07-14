# TC-5: bounded Vite Plus (vp) pilot on `apps/aiur` — evaluation

- Issue: #8776 (T3 Code Vite Plus adaptation plan; plan source:
  `docs/teardowns/2026-07-13-t3-code-teardown.md` §17)
- Date: 2026-07-14
- Verdict: **do not adopt — close.** Wall-clock parity, zero config deleted,
  +98 packages / ~169 MB, and vp silently swaps the pinned vite/vitest
  versions for its own bundled fork. Revisit triggers at the end.

## Scope and non-goals

Bounded pilot of Voidzero's Vite Plus (`vp`) on the ONLY existing Vite
surface, `apps/aiur` (`vite.config.ts`, `vite.config.cloudrun.ts`,
`vitest.config.ts`). Judged on: build/test wall-clock vs current, config
lines deleted, dependency delta, and whether the TC-1 (#8772) unified verb
set could front it without leaking vp-specific commands.

Explicit non-goals (restated from the issue):

- **No monorepo-wide vp.**
- **No `vite` → `@voidzero-dev/vite-plus-core` catalog aliasing.**
- **No `packageExtensions` test-framework rewiring** (the
  `@effect/vitest` → vp coupling T3 Code uses).

The pilot conversion was performed in a disposable worktree and deliberately
NOT landed; `main`'s aiur lane is untouched. This document is the only
artifact.

## Method and environment

- Worktree from `origin/main` at `981f38788e`; `bun install` (bun 1.3.11).
- Machine: darwin arm64 (Apple Silicon), Node v25.8.2 available on PATH.
- Install: `bun add -d vite-plus` inside `apps/aiur` — **the npm package
  works; the `curl -fsSL https://vite.plus | bash` installer from the T3
  Code README was not needed and nothing was installed globally.** Resolved
  `vite-plus@0.2.4` (bins `vp`, `vpr`, `oxlint`, `oxfmt` land in
  `apps/aiur/node_modules/.bin/`; the `vp` bin is a Node script — engines
  `^20.19.0 || ^22.18.0 || >=24.11.0` — so vp reintroduces a hard Node
  dependency into a Bun-first lane).
- Every operation: one warm-up run, then 3 timed warm runs (wall-clock via
  a monotonic-enough `time.time()` wrapper; dev = time from spawn to first
  HTTP 200 on `:3030`).
- Baseline is measured two ways: the real lane (`bun run <script>`) and the
  direct local bins (`./node_modules/.bin/vite`, `.../vitest`), so script-
  wrapper overhead is not misattributed to the toolchain.
- Test surface: 20 test files, 103 tests, all passing in every variant.

## Measurements

All values wall-clock seconds, 3 warm runs each, same machine, same session.

| Operation | Baseline (`bun run` lane) | Baseline (direct bin) | vp (`vp` bin) |
| --- | --- | --- | --- |
| `build` (vite.config.ts, client+ssr) | 1.74 / 1.88 / 1.89 | 2.33 / 1.34 / 1.40 | 1.27 / 1.28 / 1.32 |
| Cloud Run build, vite portion (`--config vite.config.cloudrun.ts`) | 1.43 / 1.46 / 1.47 (via `bun x vite`) | not measured separately | 0.97 / 0.98 / 1.00 |
| `test` (vitest run, 20 files / 103 tests) | 1.29 / 1.33 / 1.41 | 0.90 / 0.96 / 1.08 | 0.80 / 0.80 / 0.84 |
| `dev` startup → first HTTP 200 | 3.58 / 3.68 (warm re-check; first cold-cache session gave 6.19–8.59) | 3.66 / 3.67 / 3.89 | 3.50 / 3.53 / 3.85 |

Reading:

- **Parity, not a win.** vp is 0.1–0.5 s faster than the equivalent direct
  bin on a 1–2 s lane; most of the visible `bun run` delta is script-wrapper
  and bin-resolution overhead, not toolchain speed. On dev startup the three
  variants are indistinguishable once caches are warm (the first baseline
  dev session was measured colder and is reported honestly above, not used
  for the comparison).
- Part of vp's small edge is **not the same engine** (see friction #1): its
  bundled vite 8.1.3 / vitest 4.1.10 vs the workspace's pinned 8.0.16 /
  4.1.8.
- `vp build` output is functionally equivalent (same 27 files, ~same sizes,
  all 103 tests pass) but **not byte-identical** to `vite build` —
  identifier/chunk-ordering churn from the different bundled vite. Baseline
  `vite build` is byte-deterministic run-to-run; vp vs vite is not
  comparable at the hash level.
- No task-cache effect: vp 0.2.4 created no `.vite-plus` cache dir here;
  every timed vp run was a real rebuild.

## Config delta

**Lines of config deleted: 0.**

- The t3code consolidation win (one root `vite.config.ts` owning
  test/fmt/lint/staged, per-package configs deleted) does **not** transfer
  to aiur. Attempting it — `import 'vite-plus/test/config'`, `defineConfig`
  from `vite-plus`, the `test` block merged into `vite.config.ts`,
  `vitest.config.ts` deleted — fails hard: `@cloudflare/vite-plugin`'s
  `configResolved` validation rejects the vitest-created server environment
  (`resolve.external` may not be set in Cloudflare Worker environments),
  and `vp test` exits 1 before running a single test. T3 Code's root config
  carries **no build plugins**, which is precisely why unification works
  there. Aiur's split (`vitest.config.ts` with only `@vitejs/plugin-react`,
  `environment: 'node'`) exists for the same structural reason and must
  stay.
- What the conversion actually touches: `apps/aiur/package.json` gains one
  devDependency (`"vite-plus": "^0.2.4"`) and the scripts swap
  `vite dev|build` → `vp dev|build`, `vitest run` → `vp test run`. All
  three config files (84 + 90 + 29 lines) remain, unchanged. Net: **+1
  dependency line, −0 config lines.**

## Dependency delta

`bun add -d vite-plus` in the aiur workspace:

- **+98 packages** in `bun.lock`; `node_modules` grew from 5,571,952 KB to
  5,744,740 KB — **+172,788 KB (~169 MB)**.
- Notable additions: a **second, exact-pinned vitest** (`4.1.10` + the full
  `@vitest/*` 4.1.10 set, alongside aiur's existing `vitest@4.1.8`),
  `@vitest/browser` + `@vitest/browser-preview`,
  `@voidzero-dev/vite-plus-core@0.2.4` (which bundles its own **vite
  8.1.3** distribution and rolldown 1.1.4), `oxlint@1.72` /
  `oxfmt@0.57` / `oxlint-tsgolint` with their ~40 cross-platform binding
  stub packages, and `lightningcss`.
- Lockfile behavior was clean: bun handled the add without churn beyond the
  new subtree, and — genuinely good — **`vp install` at the repo root
  detected the Bun workspace and delegated to `bun install`**, preserving
  `bun.lock` (it did reproduce the known `packages/product-spec/src/cli.ts`
  file-mode flip that plain `bun install` also causes).

## Friction points

1. **Silent engine swap (the disqualifier).** Even with no catalog aliasing,
   `vp build` and `vp dev` do not use the workspace-pinned `vite@8.0.16`;
   they run vite-plus-core's bundled **vite 8.1.3** (banner: `vite v8.1.3
   building client environment...`). The app's plugins (pinned against
   `vite@^8.0.13` peers) executed inside a vite version the repo never
   chose, and the repo loses the ability to pin its build engine — vp 0.2.x
   pins it for you. It happened to work on aiur; that is luck, not a
   contract.
2. **Split-brain test framework.** `vp test` runs its bundled vitest 4.1.10
   while `bun run test` runs the repo's 4.1.8 — two framework versions for
   the same suite depending on entrypoint. (This is the standalone version
   of the `packageExtensions` rewiring the plan already refuses.)
3. **Config unification is structurally unavailable** on any surface whose
   app config carries environment-validating plugins (Cloudflare, and
   plausibly other Worker/SSR plugins). The headline "config lines deleted"
   metric for vp is 0 here.
4. **`vp check` conflicts with repo formatting.** `vp fmt --check` /
   `vp check` run oxfmt/oxlint on defaults ("No config found, using
   defaults"), flagging 66 of 68 aiur files. Adopting vp's verbs means
   adopting oxfmt formatting (a repo-wide reformat) or maintaining
   vp-specific `fmt`/`lint` config blocks — new config, not deleted config.
   `vp lint` (oxlint defaults) passed quietly.
5. **Node re-enters the lane.** The `vp` bin is a Node script with a strict
   engines range; it worked under system Node v25.8.2, but the lane's only
   toolchain requirement today is Bun.
6. **Pre-1.0 surface.** vp 0.2.4 exact-pins its internal toolchain (vite
   8.1.3, vitest 4.1.10, oxlint 1.72, oxfmt 0.57); every vp upgrade is a
   simultaneous forced upgrade of all of them.
7. **What worked well** (recorded for fairness): npm install path (no curl
   installer), `vp install` → bun delegation, `vp run --filter
   @openagentsinc/aiur test` correctly discovering the **Bun** workspace
   from the repo root, and out-of-the-box `vp build` / `vp test run` /
   `vp dev` all green with zero config changes.

## Can TC-1's verb set front vp without leaking vp commands?

**Yes — mechanically clean.** vp would live entirely inside
`apps/aiur/package.json` scripts (`dev`/`build`/`test` keep their names,
bodies become `vp ...`); the TC-1 root `bun run check` / `test` composition
invokes package scripts and never sees vp. Nothing above the package
boundary changes. Two caveats: vp ships its own `vp check` verb whose
meaning (oxfmt+oxlint on vp defaults) differs from TC-1's `check` — the
root verb must remain the only definition of green; and per friction #1/#2
the verbs would silently mean "a different vite and vitest than the
lockfile says," which is a semantic leak even when no command leaks.

## Recommendation

**Close #8776 without adopting.** On the pilot's own judging criteria:
build/test wall-clock is parity (≤0.5 s on a 1–2 s lane, partly explained
by different bundled engine versions), config lines deleted is zero (the
unification pattern is structurally blocked by the Cloudflare plugin),
and the dependency delta is +98 packages / ~169 MB including duplicate
pinned copies of vitest and vite. The decisive negative is control: vp
0.2.x substitutes its own vite and vitest for the repo's pinned versions
with no aliasing asked for — exactly the vendor coupling the teardown
refused, arriving through the front door. TC-1..TC-3 already capture the
toolchain-contract value tool-independently.

Revisit triggers (any of):

- vp reaches 1.0 with a contract to **resolve the workspace's own
  vite/vitest versions** (or pin them explicitly) instead of substituting
  bundled ones.
- The repo independently adopts oxlint/oxfmt (TC-2/TC-3 direction), making
  `vp check`'s defaults an alignment rather than a conflict.
- Vite surfaces multiply (≳3 apps) or the aiur test lane grows to where
  orchestration/caching (`vp run`, task cache) has something material to
  save — today's whole lane is under 2 seconds.
- First-class Bun support is documented by Voidzero (the `vp install` bun
  delegation observed here is promising but unversioned behavior).

## Reproduction notes

- Pilot worktree: detached from `origin/main` @ `981f38788e`; conversion
  reverted after measurement; nothing landed but this document.
- Nothing was installed globally on the machine: `vite-plus` was added only
  inside the disposable worktree's `node_modules` (removed with the
  worktree); the curl installer was never run.
