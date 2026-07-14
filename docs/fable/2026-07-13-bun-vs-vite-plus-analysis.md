# Bun vs Vite Plus — runtime/toolchain strategy analysis

- Date: 2026-07-13
- Author: Fable (workspace strategy lane)
- Status: decision evidence. The original hybrid recommendation is superseded
  by the 2026-07-14 owner selection of full Node + pnpm + Vite Plus adoption.
  Execution is governed by the separate
  [`Sol conversion plan`](../sol/2026-07-14-node-pnpm-vite-plus-full-conversion-plan.md),
  not by this historical analysis.
- Prompted by: owner question — "analyzing bun vs vite-plus, pros and cons
  etc — like what one does vs the other. anything u can tell from t3code
  repo why they went the vite plus route, why not bun, could we / should we
  make the switch from bun to vite plus (bonus: bun is now owned by
  Anthropic and we don't want to have dependencies on them)"
- Companion to: the T3 Code teardown's "Adapt as a program: the Vite Plus
  toolchain contract" section
  (`docs/teardowns/2026-07-13-t3-code-teardown.md`) and the toolchain
  program it spawned — epic
  [#8777](https://github.com/OpenAgentsInc/openagents/issues/8777) with
  leaves [#8772](https://github.com/OpenAgentsInc/openagents/issues/8772)
  (TC-1 one root verb set),
  [#8773](https://github.com/OpenAgentsInc/openagents/issues/8773) (TC-2
  oxlint plugin), [#8774](https://github.com/OpenAgentsInc/openagents/issues/8774)
  (TC-3 fmt-on-commit),
  [#8775](https://github.com/OpenAgentsInc/openagents/issues/8775) (TC-4
  `@effect/tsgo` pilot), and
  [#8776](https://github.com/OpenAgentsInc/openagents/issues/8776) (TC-5
  bounded `vp` pilot on `apps/aiur`).
- Evidence sources: this repo (paths and counts below), a read-only clone of
  the public T3 Code repo (commit hashes cited), and public announcements
  (URLs cited, marked `[public]`).

## 0. Executive summary

"Bun vs Vite Plus" is a category error dressed as a choice, and untangling
it is most of the answer. **Bun is a JavaScript runtime** (plus a package
manager, test runner, and bundler that ride along with it). **Vite Plus is a
toolchain** (build, test, lint, format, task-run) **that executes on
Node.js** — it does not replace the runtime at all. Any "switch from Bun to
Vite Plus" is therefore two independent decisions:

1. **Runtime**: keep executing production code on Bun, or move to Node.
2. **Toolchain**: keep Bun's built-in `install`/`test`/task-runner, or adopt
   Vite Plus (`vp`), or adopt its pieces (oxlint/oxfmt/vitest) à la carte.

T3 Code made both decisions, separately: Node 24 + pnpm 11 as
runtime/package manager, `vp` as toolchain — while keeping their server
*Bun-capable* at user runtime. OpenAgents' position is different: Bun is the
**production runtime** for roughly ten Cloud Run services including the
`openagents.com` API itself, and `Bun.*`/`bun:sqlite` APIs appear in ~170
non-test source files. For T3, dropping Bun was cheap; for us it is a
genuine migration.

The Anthropic-ownership premise **is verified**: Anthropic acquired Oven
(Bun) on 2025-12-02 `[public]`. But the symmetrical fact matters just as
much: **Vite Plus's maker VoidZero was acquired by Cloudflare on
2026-06-04** `[public]` — and this repo deliberately completed a full
migration off Cloudflare in July 2026 (epic #8515). There is no
vendor-neutral option on this menu. Both toolchains are MIT-licensed and
forkable; the question is which platform vendor's roadmap gravity we prefer,
and how much concentration we accept.

**Updated recommendation (§7 and addendum): full Option B.** Make retained
production code Node-native, replace the workspace atomically with pnpm and
Vite Plus in T3's operating pattern, and remove—not port—the payment graph
that the accepted MVP explicitly excludes. TC-5 remains valid evidence that
*additive* Vite Plus adoption is the wrong topology; it is not a test of the
integrated replacement T3 actually performed.

## 1. The category asymmetry

**Bun** (by Oven, now Anthropic) is a JavaScript runtime built on Apple's
JavaScriptCore engine, positioned as a drop-in Node.js replacement. It
bundles four roles into one binary:

- runtime (`bun run file.ts` — executes TypeScript directly, owns
  `bun:sqlite`, `Bun.serve`, `Bun.spawn`, `Bun.file`, single-file
  executables)
- package manager (`bun install`, `bun.lock`)
- test runner (`bun test`, `bun:test`)
- bundler (`Bun.build`)

**Vite Plus** (by VoidZero, now Cloudflare) is a unified development
toolchain: Vite (on the Rolldown bundler), Vitest, Oxlint, Oxfmt, tsdown,
and a task runner behind one `vp` binary and one root config. It is
MIT-licensed as of the beta `[public]`
([announcement](https://voidzero.dev/posts/announcing-vite-plus),
[alpha 2026-03-13](https://voidzero.dev/posts/announcing-vite-plus-alpha),
[beta](https://voidzero.dev/posts/announcing-vite-plus-beta)). Crucially,
`vp` **runs on Node** (it can even manage the Node version and package
manager for you) — it has no opinion about, and no ability to replace, the
runtime your production services execute on.

So the two products overlap only in the middle of the stack:

| Role                    | Bun            | Vite Plus              |
| ----------------------- | -------------- | ---------------------- |
| Production runtime      | yes (JSC)      | no (assumes Node)      |
| Package manager         | `bun install`  | manages pnpm/npm       |
| Test runner             | `bun test`     | Vitest (`vp test`)     |
| Bundler/build           | `Bun.build`    | Vite/Rolldown/tsdown   |
| Linter                  | no             | Oxlint (`vp lint`)     |
| Formatter               | no             | Oxfmt (`vp fmt`)       |
| Monorepo task runner    | `bun run`      | `vp run` / `vpr`       |
| Staged/commit hooks     | no             | staged config          |

Bun's unique value is the top row; Vite Plus's unique value is the bottom
three. Everything in between is contested. That is why "switch from Bun to
Vite Plus" decomposes into a runtime decision and a toolchain decision — and
why the two can be made independently, on different timelines, with
different risk profiles.

## 2. What OpenAgents actually uses Bun for (inventory)

The decisive question is where Bun is the **runtime executing production
code** versus where it is merely the **dev-loop task/test runner**. The swap
cost differs by an order of magnitude.

### 2.1 Runtime — load-bearing (expensive to swap)

- **Production Cloud Run images.** The core API and most services build
  `FROM oven/bun:1` (or pinned `oven/bun:1.3.1`):
  `apps/openagents.com/workers/api/Dockerfile` (the `openagents.com` API
  monolith), `apps/openagents.com/apps/start/Dockerfile`,
  `apps/openagents.com/services/mdk-sidecar/`, `services/mdk-treasury/`,
  `services/mdk-tips-buffer/`, `apps/aiur/`, `apps/khala-capture/`,
  `apps/khala-live-hub/`, `apps/oa-queue-worker/`, `apps/oa-updates/`.
  Roughly **ten deployed services execute on Bun**, including the payments
  sidecars and the realtime sync capture path.
- **Bun-only APIs in shipped source.** ~**170 non-test source files** use
  `Bun.*` APIs (`Bun.spawn`, `Bun.file`, `Bun.serve`, …), concentrated in
  `apps/pylon/src/**` (wallet, orchestration, executors, MCP, composer) and
  its runtime package. `bun:sqlite` appears in **10 non-test files**,
  including `apps/pylon/src/orchestration/store.ts`,
  `packages/khala-sync-client/src/sqlite-store.ts`,
  `apps/khala-live-hub/src/scope-hub.ts`, and both `spark-bun-storage.ts`
  copies (Pylon and the treasury service) — i.e. the fleet-orchestration
  state store, the sync client's SQLite store, and Lightning wallet storage
  are written against Bun's SQLite driver.
- **The published Pylon package requires Bun at user runtime.**
  `apps/pylon/package.json` ships `bin.pylon = "src/index.ts"` whose first
  line is `#!/usr/bin/env bun`; the README instructs `bun install` /
  `bun apps/pylon/src/index.ts`. A user without Bun cannot run Pylon.
- **Notably absent:** there are **zero** imports of
  `@effect/platform-bun` *or* `@effect/platform-node` in the repo — our
  Effect surfaces sit above hand-rolled adapters and raw `Bun.*` calls, so
  there is no ready-made platform-layer seam to flip (T3 has exactly that
  seam; see §3).

### 2.2 Toolchain — dev loop (cheap to swap in principle, wide in practice)

- **Task runner:** **127 of 130 root `package.json` scripts** invoke `bun`
  (`bun run`, `bun scripts/*.ts`, `bunx`), and per-app scripts follow suit
  (e.g. `apps/openagents-desktop` builds with `bun scripts/build.ts` and
  verifies with `bun run typecheck && bun test && bun run build`).
- **Test runner:** ~**2,660 `*.test.ts` files** run under `bun test` /
  `bun:test`, including the desktop and Pylon gates.
- **Lockfiles:** `bun.lock` at the repo root and a second one inside the
  nested `apps/openagents.com` workspace.
- **Gates:** there is no GitHub Actions CI; the enforcement surface is the
  local `.githooks/pre-push` chain, which is written entirely in `bun run`
  invocations (conflict-marker checks, QA visual smoke gate, mobile gate,
  desktop typecheck+test+smoke, deploy check).
- **Neutral already:** `clients/khala-cli` ships compiled `dist/index.js`
  and documents "plain Node or Bun" — the runtime-neutral pattern the rest
  of the repo does not yet follow.
- **Node-shaped by necessity:** the Electron main process
  (`apps/openagents-desktop`, `apps/autopilot-desktop`) and Expo/Metro
  (`apps/openagents-mobile`, `clients/khala-mobile`) execute on Node no
  matter what we choose — Bun cannot own those surfaces, only their build
  scripts and tests.

Net: Bun is not a convenience here. It is the production runtime of the API
and payments path, the storage driver of the orchestration and sync layers,
and the contractual runtime of a published npm binary — *plus* the entire
dev loop.

## 3. What T3 Code actually chose, and why

All claims verified against a read-only clone of the public T3 Code repo.

### 3.1 The facts

- **Runtime/PM:** root `package.json` pins `engines.node: "^24.13.1"` and
  `packageManager: "pnpm@11.10.0"`. The server package publishes to npm
  with `engines.node: "^22.16 || ^23.11 || >=24.10"` — Node is the
  contractual runtime.
- **Toolchain:** every root verb is `vp` (`vp run`, `vp lint`, `vp fmt`,
  `vp test` via `vp run -r test`); a single root `vite.config.ts` owns
  test/lint/fmt/staged config for the whole monorepo; the pnpm catalog
  aliases `vite` to `npm:@voidzero-dev/vite-plus-core@0.2.2`; `AGENTS.md`
  line one gates every coding agent on "`vp check` and `vp run typecheck`
  must pass".
- **They migrated *off* Bun.** T3 Code used Bun until June 2026:
  `.mise.toml` pinned `node = "24.13.1"` **and** `bun = "1.3.9"`, a
  `bun.lock` was tracked, and history is full of `bun run` fixes ("Fix
  `bun run dev:desktop` …", "Require bun fmt for completion", "…during bun
  install"). Commit `b440dd18` (2026-06-03, PR #2899, "Migrate workspace to
  Vite+ and pnpm") removed `bun.lock`, `.mise.toml`'s bun pin,
  `.oxlintrc.json`, `.oxfmtrc.json`, `turbo.jsonc` files, and
  `tsdown.config.ts` in one stroke.
- **What Bun actually was for them, pre-migration:** package manager and
  task runner only. The pre-migration `package.json` already ran builds
  through **turbo**, lint through **standalone oxlint**, fmt through
  **standalone oxfmt**, tests through **vitest**, TypeScript through
  **@effect/tsgo**, bundling through **tsdown** — with Node pinned
  alongside Bun in mise. Bun the *runtime* was never their production
  story.
- **They kept Bun as an optional runtime, not a toolchain.**
  `apps/server/src/persistence/Layers/Sqlite.ts` selects
  `@effect/sql-sqlite-bun` when `process.versions.bun` is defined and a
  Node SQLite client otherwise; `apps/server/src/terminal/` ships both
  `BunPtyAdapter.ts` and `NodePtyAdapter.ts`; `@effect/platform-bun` stays
  in the catalog. Post-migration commits keep tending this ("Structure
  unavailable Bun PTY operations", #3394). The npm package is Node-first,
  Bun-capable.
- **Residual crumb, resolved:** `AGENTS.md` still says "Manage vendored
  subtrees with `bun run sync:repos`", but the referenced script
  (`scripts/sync-reference-repos.ts`) begins `#!/usr/bin/env node` and
  imports `@effect/platform-node` — the doc line is stale text from the
  pre-migration era, not live Bun usage.

### 3.2 The best-supported explanation

`[inferred from the evidence above; motive was not stated in the commit]`

1. **Bun was shallow for them.** Because build/lint/fmt/test were *already*
   VoidZero tools (oxlint, oxfmt, vitest) plus turbo and tsdown, `vp`
   consolidated six tools and five configs into one binary and one root
   config. The migration deleted config files; it did not rewrite runtime
   code. Their only runtime-coupled code (SQLite, PTY) was already behind a
   platform seam.
2. **Their product shape caps Bun's value.** T3 Code is an Electron desktop
   app + Expo mobile app + an npm-published Node server. Electron's main
   process and Metro are Node by definition, and an npm CLI that demands
   Bun shrinks its audience. When the runtime must be Node on every
   customer-facing surface, Bun's remaining value — fast install, fast
   tests — competes directly with pnpm+vitest, and `vp` erased the
   config-sprawl argument.
3. **Agent-gate coherence.** Their monorepo is maintained largely by coding
   agents; `vp check` gives one machine-checkable definition of done that
   is identical for humans, CI, and agents (see the teardown §"Vite Plus
   toolchain contract"). That is a toolchain argument, not a runtime one —
   and it is the part worth copying regardless of runtime.
4. **Public alignment, for what it is worth.** `[public]` Theo has covered
   VoidZero enthusiastically since its 2024 funding announcement and made
   dedicated videos on it and on the Cloudflare acquisition
   ([his announcement thread](https://x.com/t3dotgg/status/1842392504341291132)).
   We found no public evidence of a formal investment relationship, so
   treat this as visible enthusiasm and early-adopter positioning, not a
   verified financial tie. Anti-Anthropic motive is likewise unevidenced:
   the migration (2026-06-03) came six months after the Bun acquisition and
   the same week as the Cloudflare/VoidZero announcement; T3 Code itself
   wraps Codex first, not Claude, so their vendor gravity differs from
   ours. The mundane explanation — consolidation of an already-VoidZero
   stack — requires no conspiracy and fits every artifact.

### 3.3 What does *not* transfer to us

T3's migration was cheap because Bun was only their PM/task runner. Ours is
not: §2.1 shows Bun as our production runtime, storage driver, and
published-binary contract. Copying their endpoint without their starting
point means paying a migration they never paid.

## 4. The ownership facts and the dependency-risk analysis

### 4.1 Verified facts `[public]`

- **Anthropic acquired Oven (Bun) on 2025-12-02** — Anthropic's first
  acquisition. Bun "stays open-source & MIT-licensed," the same team
  continues, and the stated rationale is that Claude Code ships as a Bun
  single-file executable, making Bun infrastructure for Claude Code and the
  Agent SDK. Bun's stated roadmap keeps "Node.js compatibility & replacing
  Node.js as the default server-side runtime" as a priority.
  Sources: [Bun blog](https://bun.com/blog/bun-joins-anthropic),
  [Anthropic announcement](https://www.anthropic.com/news/anthropic-acquires-bun-as-claude-code-reaches-usd1b-milestone).
- **Cloudflare acquired VoidZero on 2026-06-04.** Vite, Vitest, Rolldown,
  Oxc, and Vite Plus "remain open-source and MIT-licensed"; Evan You and
  the team joined Cloudflare with a stated vendor-neutrality commitment.
  Source: [VoidZero announcement](https://voidzero.dev/posts/voidzero-cloudflare).

So the owner's premise is confirmed — and symmetrical. Neither option is an
independent vendor: Bun is Anthropic infrastructure; Vite Plus is Cloudflare
infrastructure.

### 4.2 The risk, steelmanned

The concern is **concentration**, not ownership per se. OpenAgents already
depends on Anthropic for frontier models and for the Claude Code / Agent SDK
harness family. Adding "steward of our production runtime" to the same
vendor means one company's priorities shape our model quality, our agent
harness, *and* our runtime's roadmap. If Anthropic optimizes Bun for
Claude-Code-shaped workloads at the expense of general server workloads, or
deprioritizes the Node-compat work that keeps our exit cheap, we feel it.

### 4.3 The counterweights

1. **The dependency is local, not a service.** Bun is a binary we pin
   (`oven/bun:1.3.1` in the payment sidecars). Nothing about the
   acquisition can revoke access, change pricing, or alter behavior of a
   version we already run. Stewardship risk is about *future direction*,
   not access.
2. **MIT and forkable.** Both Bun and Vite Plus are MIT. A hostile turn
   produces a fork (the JS ecosystem has done this repeatedly), not a
   hostage situation.
3. **Bun's own roadmap is our exit ramp.** Node compatibility is Bun's
   stated priority; every compat improvement *reduces* our switching cost
   to Node. The lock-in surface is the `Bun.*`/`bun:sqlite` API set (§2.1),
   which is bounded and enumerable — and which we can shrink opportunistically
   (see §6) without a migration program.
4. **Switching to Vite Plus does not reduce vendor exposure — it trades
   it.** We would exchange Anthropic-stewarded (runtime) for
   Cloudflare-stewarded (toolchain) — the same Cloudflare this repo just
   completed a deliberate infrastructure exit from (epic #8515, July 2026).
   That exit was about *hosting our workloads on their platform*, which a
   dev toolchain is not; but "we don't want dependencies on them" cuts at
   least as hard against Cloudflare here as against Anthropic. And `vp` is
   0.2.x pre-1.0 software with its config surface still moving — a
   different, nearer-term risk than Bun's stewardship question.
5. **Alignment, honestly assessed.** There is also a non-cynical reading of
   the Anthropic acquisition for a shop like ours: our production workloads
   are agent harnesses. The new steward's flagship workload *is* our
   workload. That cuts both ways (roadmap capture vs roadmap fit); it is
   not purely a liability.

## 5. Performance and ergonomics, briefly

- **Install:** `bun install` remains the fastest installer; pnpm 11 is
  close enough that this is not decision-grade for a repo our size.
- **Tests:** `bun test` starts fast and runs our 2,660-file suite today;
  Vitest 4 (under `vp`) is heavier per-process but brings the browser mode,
  workspace projects, and the `@effect/vitest` integration T3 leans on. We
  have no measured evidence either way on *our* suite — which is exactly
  what TC-5 exists to produce.
- **TypeScript execution:** Bun runs `.ts` directly (our scripts and the
  Pylon bin depend on this); Node 24 now also runs TypeScript natively via
  type-stripping `[public]`, which removes what used to be the strongest
  ergonomic argument for Bun-as-script-runner — worth knowing for any
  future migration costing, since our 127 `bun`-invoking scripts would not
  all need a build step.
- **Lint/fmt:** Bun has no linter or formatter; oxlint/oxfmt are the
  relevant tools *in both futures* and run standalone without `vp` (TC-2,
  TC-3 depend on nothing Bun- or vp-specific).
- **Agent ergonomics:** the real T3 lesson — one verb set, one config, laws
  as lint, identical human/CI/agent gate — is toolchain-agnostic and
  already programmed as TC-1..TC-3.

## 6. Decision analysis

### Option A — Stay Bun everywhere (status quo)

- Pros: zero migration cost; fastest dev loop today; single tool for
  run/install/test; production images already pinned and working; steward's
  flagship workload matches ours.
- Cons: vendor concentration on Anthropic deepens by default; no linter or
  formatter comes with it (we still need oxlint/oxfmt regardless); the
  `Bun.*`/`bun:sqlite` API surface keeps growing unchecked, silently
  raising future exit cost; Pylon's Bun-only bin narrows its install base.

### Option B — Node + pnpm + vp wholesale (mirror T3)

- Pros: removes Anthropic-owned Bun from the production failure path; puts the
  server/tooling baseline on OpenJS-governed Node; makes Pylon a conventional
  compiled npm artifact; replaces root-script entropy with pnpm's explicit
  graph and `vp run`; gives humans, agents, owned CI, and releases the same
  gate; and pushes the corporate dependency outward into a replaceable build
  layer rather than the deployed runtime.
- Cons: by far the largest conversion—214 grandfathered Bun-API production
  files at the post-BUN-1 snapshot, thousands of Bun tests, root scripts,
  containers, native dependencies, and release paths. Vite Plus is beta and
  deliberately substitutes a bundled toolchain, pnpm exposes undeclared
  dependency assumptions, and test/runtime parity must be re-proved. Removing
  the non-MVP payment graph avoids spending migration effort on its Bun-native
  wallet/services, but requires an orderly balance/receipt shutdown before
  deletion.

### Option C — Hybrid: keep the runtime, adopt the contract (superseded)

Keep Bun as runtime where it is load-bearing (§2.1). Adopt the toolchain
*contract* tool-independently, exactly per the filed sequence: TC-1 one
root verb set on the existing Bun task runner, TC-2 `oxlint-plugin-openagents`
(standalone oxlint, no vp), TC-3 fmt-on-commit gradient, TC-4 `@effect/tsgo`
pilot, TC-5 bounded `vp` pilot on `apps/aiur` only. In parallel, adopt two
cheap de-risking disciplines with no migration program:

- **Stop widening the lock-in surface.** New code reaches for portable APIs
  (`node:` builtins Bun implements, or a thin owned adapter) instead of new
  `Bun.*`/`bun:sqlite` call sites; the T3 pattern to copy is their runtime
  seam (`process.versions.bun` selection behind one module), applied first
  to SQLite access. This can be a TC-2 lint rule
  (`no-new-bun-api-outside-adapter`) — enforcement at agent speed, cost
  near zero.
- **Make the published binaries runtime-neutral over time.** `khala-cli`
  already ships compiled JS for "Node or Bun"; Pylon should trend the same
  way when it is next touched, decoupling our users' runtime from ours.

Pros: zero disruption to production or the payments path; captures the
entire agent-governance payoff (the actual reason T3's setup is enviable);
produces the measured evidence (TC-4/TC-5) a real runtime decision needs;
caps and then shrinks exit cost instead of paying it speculatively; takes
no new position on either vendor. Cons: vendor concentration persists in
the interim; two toolstacks coexist during the pilots; requires discipline
(the lint rule) to keep the Bun surface from re-widening.

### Option D — Node migration with à la carte oxlint/oxfmt/vitest, no vp

- Pros: standard runtime without coupling to `vp`'s pre-1.0 release train;
  each tool chosen on merit; this was T3's *own* pre-vp stack, so it is
  proven viable.
- Cons: pays all of Option B's migration cost while forgoing its
  consolidation payoff; more configs to keep coherent (the exact sprawl vp
  exists to kill); still needs a task runner (turbo or similar) to replace
  `bun run`.

## 7. Recommendation

**Option B is selected.** The source audit changes the relevant comparison
from “working Bun versus an additive beta dependency” to “one integrated
system versus two permanent systems.” T3's actual order is the template:

1. make retained server/CLI code Node-native behind owned platform seams;
2. establish Effect TSGo and Vite Plus test parity with explicit host
   exceptions;
3. replace Bun's package-manager/test/task/build authority atomically with
   pnpm and exact-pinned Vite Plus, deleting the displaced stack; and
4. stabilize native installs, packaging, host configs, publishing, owned CI,
   and production images, then delete every temporary Bun branch.

OpenAgents should go further than T3 at the runtime boundary: T3's public
artifact is Node-first but retains optional Bun HTTP, PTY, platform, and
SQLite adapters. The selected OpenAgents destination keeps the adapter seam
and deletes the Bun implementation after conformance.

Payments are not a late conversion risk. They are outside the accepted MVP
and should be decommissioned and deleted before the atomic workspace cutover:
stop new money, reconcile outstanding balances/intents, preserve applied
migrations and historical receipts read-only, withdraw active payment
promises, shut down/revoke the money services, then remove wallet/tip/payout/
settlement code and dependencies. Restoration later is a new ProductSpec and
custody program, not a revert.

The binding implementation sequence, gates, and payment retirement order are
in the
[`Sol full-conversion plan`](../sol/2026-07-14-node-pnpm-vite-plus-full-conversion-plan.md).
This Fable document remains evidence and does not dispatch work by itself.

---

**Status 2026-07-14 (BUN-1 landed — #8779).** The Bun-API containment seam
from Option C is now in place. `@openagentsinc/sqlite-runtime`
(`packages/sqlite-runtime/`) is the dual-runtime SQLite seam modeled on T3
Code's `Sqlite.ts`/`NodeSqliteClient.ts`: runtime detection via
`process.versions.bun`, a `bun:sqlite` client under Bun and a `node:sqlite`
client under Node behind one synchronous `SqliteDatabase` contract
(signature-compatible with khala-sync-client's `SqlDriver`), with Effect
wrappers for open/scoped ownership. One shared conformance suite runs under
`bun test` (Bun client) and under real `node --test` (Node client — no
mocked runtime). The pilot store is khala-sync-client's
`sqlite-store.ts` (KS-5.1), which no longer imports `bun:sqlite` at all;
its full suite stays green with zero wire/behavior change. The perimeter is
now enforced by `bun run scan:bun-api-perimeter`
(`scripts/bun-api-perimeter-scan.ts`): 214 production source files with
`bun:*` imports or `Bun.*` usage are grandfathered in
`scripts/bun-api-perimeter-allowlist.ts` as a checked-in burn-down list, the
seam's Bun client is the first named-perimeter entry, and any NEW
un-allowlisted usage fails the scan (wired into the root test sweep as
`test:bun-api-perimeter`; merges into `oxlint-plugin-openagents` when #8773
lands). The third revisit trigger above is now measurable: the burn-down
count is the "load-bearing surface small enough" metric.

## Addendum — 2026-07-14: Option B, fully steelmanned

The short Option B treatment above prices the migration cost correctly, but
compresses its upside too aggressively. If the owner *does* want a full
conversion, there is a coherent affirmative case for it. The strongest version
is not "copy T3 because `vp` is fashionable" and not "Vite Plus replaces Bun."
It is:

1. make a pinned Node LTS release the one server, CLI, and tooling runtime;
2. make pnpm the workspace/package-resolution contract; and
3. make Vite Plus the replaceable developer control plane over build, test,
   lint, format, packaging, and task execution.

Only the first decision changes the production runtime. `vp` should not be in
the deployed runtime dependency graph at all.

### B.1 The biggest benefit: it really would reduce Anthropic concentration

The earlier statement that Option B "trades Anthropic stewardship for
Cloudflare stewardship" is true by product count but too symmetrical about
blast radius.

- Bun is currently in the **production trust and failure path**: the API,
  realtime services, payment sidecars, Pylon, SQLite stores, scripts, and tests
  execute on an Anthropic-owned runtime.
- Node is an Impact Project hosted by the vendor-neutral OpenJS Foundation
  ([OpenJS projects](https://openjsf.org/projects),
  [OpenJS mission and governance](https://openjsf.org/about)). A Node cutover
  would therefore remove Anthropic stewardship from the production runtime
  rather than merely rename it.
- Vite Plus would introduce Cloudflare stewardship in the **development and
  build path**, but that is a smaller and more reversible dependency. Its
  outputs are ordinary JavaScript, assets, declarations, and package metadata;
  the underlying Vite, Vitest, Oxc, Rolldown, and tsdown pieces remain
  separately usable; and Vite Plus documents both an opt-out from its Node
  manager (`vp env off`) and a removal command (`vp implode`)
  ([Vite Plus getting started](https://viteplus.dev/guide/)).

That distinction is the best vendor-risk argument for Option B: **production
runtime governance moves to a neutral foundation; the corporate dependency is
pushed outward to a replaceable toolchain layer.** This does not make
Cloudflare irrelevant, but it is a genuine reduction in critical-path vendor
concentration.

### B.2 One reference engine for server code, CLIs, tests, and host tooling

OpenAgents already has a split world: server code runs on Bun, while Electron
main/build tooling and Expo/Metro tooling are Node-shaped, npm packages are
consumed primarily in Node, and much portable source is written against
`node:` builtins that Bun happens to implement. At the 2026-07-14 snapshot,
roughly **639 non-test production files mention `node:` builtins**, versus the
**214 grandfathered production files** in the enforced Bun-API burn-down list.

A successful conversion would make Node/V8 the reference behavior for:

- Cloud Run services and their local development processes;
- Pylon and other published CLIs;
- unit/integration tests for Node-targeted code;
- Electron main/preload code and desktop build tooling; and
- Expo/Metro and repository automation.

That removes a whole compatibility dimension: fewer "works under Bun's Node
compatibility layer but not under real Node" failures, fewer engine-specific
mocking differences, one ESM/streams/child-process behavior to support, and
tests that execute on the same engine as the server artifact. Browser,
Electron-renderer, React Native/Hermes, and native Rust/Swift/Kotlin code would
of course remain distinct runtimes; Option B standardizes the **server and
tooling baseline**, not literally every processor in the product.

BUN-1 makes this payoff less hypothetical. `@openagentsinc/sqlite-runtime`
already proves that real `node:sqlite` can satisfy the shared store contract
under `node --test`. A full conversion would turn that dual-runtime seam from
an exit option into the first completed migration slice.

### B.3 Pylon becomes a normal npm/Node product instead of a Bun-required one

Today `@openagentsinc/pylon` advertises itself as "built on Bun," pins
`packageManager: bun`, publishes TypeScript source, and points its `pylon` bin
at `src/index.ts`. That is elegant for a Bun-native shop, but it makes Bun a
prerequisite for every contributor and customer machine.

A Node-first Pylon could ship compiled ESM plus declarations and run through
ordinary `npm`, `npx`, pnpm, Corepack, containers, enterprise Node images, and
the existing Node estate of prospective operators. Vite Plus's `vp pack`
wraps tsdown for declarations, multiple output formats, source maps, and
minification
([Vite Plus pack](https://viteplus.dev/guide/pack)). The important outcome is
not the packer brand; it is that the public package stops executing raw
TypeScript out of `node_modules` and gains an explicit build artifact and
runtime contract.

That broadens Pylon's addressable install base, reduces onboarding steps, and
makes "install the OpenAgents contributor node" fit the normal JavaScript
deployment vocabulary. It also lets OpenAgents test the exact tarball under a
stock Node LTS image before publishing instead of assuming a globally installed
Bun.

### B.4 `vp run` could provide a real workspace execution model

The present root has **150 scripts, 147 of which invoke Bun**. The root `test`
and `typecheck` verbs are long hand-maintained `&&` chains. They do not express
the package graph, do not select only affected work, and do not cache successful
subtasks. Their simplicity is valuable, but their cost grows linearly with the
repo.

Vite Task now gives `vp run`:

- dependency-ordered recursive workspace execution;
- package, directory, dependency, and dependent filters;
- bounded parallelism;
- explicit task dependencies;
- content-based caching with automatic file-read/write tracking; and
- cached output restoration plus explanations for cache misses.

Those are current first-party capabilities, not inferred roadmap claims
([Vite Plus run](https://viteplus.dev/guide/run),
[task caching](https://viteplus.dev/guide/cache)). For OpenAgents, the payoff
could be substantial: a coding agent changing one schema package could run the
package, dependents, and shared laws rather than blindly traverse every app;
repeated checks in the same worktree could become cache hits; and the task DAG
could replace duplicated ordering knowledge embedded in root scripts and hooks.

This is a stronger benefit than "one command." It is a chance to make the
monorepo's dependency graph executable and inspectable.

The completed TC-5 pilot did **not** validate this benefit on OpenAgents. It
measured direct `vp build` / `vp test` / `vp dev` on one sub-two-second app
lane, not a root task DAG, and observed no task-cache effect. Workspace-aware
execution and caching remain a credible full-conversion benefit, but they need
a separate root-scale benchmark rather than being backfilled from the aiur
numbers.

### B.5 pnpm would make dependency boundaries stricter and more legible

pnpm is not only a slower/faster-installer trade against Bun. Its default
non-flat `node_modules` exposes only declared direct dependencies, which catches
phantom imports that a hoisted install can hide. Its `workspace:` protocol can
refuse a registry fallback when a required local package/version is missing,
and one workspace lockfile keeps resolution reviewable. The content-addressed
store also shares package files across worktrees and repositories
([pnpm motivation](https://pnpm.io/motivation),
[pnpm workspaces](https://pnpm.io/workspaces)).

That fits a 90-package OpenAgents workspace unusually well. The migration would
surface undeclared dependencies and peer-resolution assumptions up front, but
afterward those failures become install-time graph errors rather than
machine-dependent runtime surprises. The stricter package graph also gives
`vp run` a better dependency DAG to schedule.

### B.6 The test and build paths would converge on the wider ecosystem

The migration cost of roughly 2,800 test files (about 1,450 importing
`bun:test`) is real. The end-state benefit is also real:

- `vp test` is Vitest using the same Vite resolution/transformation plugins as
  the applications, with standard ESM/TypeScript/JSX handling, snapshots, and
  coverage ([Vite Plus test](https://viteplus.dev/guide/test));
- renderer packages can share their Vite plugin graph with their tests rather
  than maintain Bun-only test transforms;
- Node-targeted tests run under Node semantics rather than Bun compatibility;
  and
- Effect-aware Vitest helpers and the broader Jest/Vitest tooling ecosystem
  become first-class rather than adapter work.

Likewise, `vp build` and `vp pack` provide one front door for Vite/Rolldown app
builds and tsdown library builds. OpenAgents could replace packages that publish
source `.ts`, one-off Bun build scripts, and separate lint/test/build configs
with explicit, reproducible artifacts. This matters most for public packages
and desktop/web renderers; a plain Node service can still run compiled JS
without pretending it is a Vite app.

This is convergence of the **verb and tool family**, not necessarily one
physical config file. TC-5 proved that aiur's Cloudflare environment-validating
plugin cannot share the same Vite config as its Node-environment tests; that
surface must retain a separate test config even in a full conversion.

### B.7 One static gate can become the repo's agent operating system

Vite Plus's current `vp check` combines Oxfmt, Oxlint, and an optional
tsgolint-backed typecheck in one config
([Vite Plus check](https://viteplus.dev/guide/check)). `vp staged` reads staged
rules from that same config
([commit hooks](https://viteplus.dev/guide/commit-hooks)). Together with
workspace-aware `vp run`, a full conversion could give every human, Codex,
Claude, Grok, and future harness the same small completion vocabulary:

```text
vp check
vp test
vp run <targeted proof>
```

The gain is governance throughput: architectural laws in
`oxlint-plugin-openagents`, formatting, package overrides, test defaults,
task dependencies, and the staged gradient live behind the same commands. An
agent cannot accidentally choose the wrong package-local dialect of "green."

The TSGo pilot's caveat still applies. OpenAgents should keep canonical `tsc`
or another separately verified typecheck task until the type-aware Oxlint/
Effect diagnostic path has repo-wide parity. Full Option B does not require
turning every Vite Plus default on the first day.

### B.8 The migration itself would force a healthier platform architecture

Today raw `Bun.*` calls mix runtime choice with domain code. Paying the
conversion cost would force each concern behind an owned boundary:

- SQLite behind `@openagentsinc/sqlite-runtime`;
- HTTP serving behind a server adapter;
- filesystem/blob access behind owned file services;
- child processes and PTYs behind executor adapters;
- packaging behind compiled artifact contracts; and
- runtime choice behind Effect layers or equally narrow typed modules.

That work is not throwaway migration glue. It leaves the codebase more
portable, more testable, and less coupled to *any* next runtime, including
Node. The full conversion can therefore be understood as a platform-boundary
program whose chosen destination is Node, not as 214 isolated search-and-
replace jobs.

### B.9 What the completed TC-5 pilot does—and does not—change

The bounded aiur result landed after the original analysis:
[`docs/research/2026-07-14-vite-plus-pilot.md`](../research/2026-07-14-vite-plus-pilot.md),
with the closeout summarized on
[#8776](https://github.com/OpenAgentsInc/openagents/issues/8776#issuecomment-4966050487).
It is direct evidence against **additive** Vite Plus 0.2.4 adoption inside the
existing Bun workspace:

- build/test deltas were wall-clock parity (at most about 0.5 seconds on a
  one-to-two-second lane), partly because the compared engines differed;
- the Cloudflare Vite plugin structurally blocked the unified-config pattern,
  so the number of config lines deletable was **zero**;
- adding Vite Plus pulled **98 packages / about 169 MB**, including duplicate
  Vite/Vitest toolchains and many cross-platform native binding packages;
- `vp build` and `vp test` silently used Vite 8.1.3 and Vitest 4.1.10 from
  `vite-plus-core`, not the workspace-pinned Vite 8.0.16 and Vitest 4.1.8;
- `vp check`'s formatter defaults conflicted with 66 of aiur's 68 files; and
- the positive integration result was that TC-1's root verb set could front
  `vp` without leaking `vp` commands, while `vp install` correctly delegated
  to the existing Bun package manager.

The issue and its closeout explicitly ruled out monorepo-wide Vite Plus, pnpm,
the `vite` → Vite Plus core alias, `@effect/vitest` rewiring, and changes to
`main`. It invoked direct `vp` built-ins on one Cloudflare-plugin app, not a
root `vp run` workspace graph. Direct built-ins do not participate in Vite
Task's task cache. The pilot therefore selected exactly the topology most
likely to duplicate dependencies while deleting nothing.

T3's `b440dd18` migration selected the inverse topology: remove `bun.lock`,
Turbo, standalone Vitest/tsdown/Oxlint/Oxfmt configs, migrate the test imports,
make pnpm the workspace authority, alias `vite` explicitly to exact-pinned
Vite Plus core, and change CI/releases/agents/hooks together. Its conversion
touched 299 files and was followed immediately by install, task-graph,
Effect-test, publishing, Electron, and hook repairs. The +98 packages and
169 MB are consequently an incremental-pilot measurement, not a forecast of
the net replacement footprint.

This changes the honest steelman in two ways.

First, the benefits in B.1-B.8 are **end-state benefits of Node, pnpm, explicit
artifacts, architectural seams, and a workspace control plane**. TC-5 supplies
no OpenAgents-specific performance or config-deletion evidence for Vite Plus
itself. A full conversion cannot cite that pilot as a speed win, but neither can
the pilot falsify a topology it prohibited.

Second, bundled substitution is not accidental in the full system. Vite Plus
builds and projects pinned Vite/Rolldown/Vitest/tsdown/Oxc sources as one
known-compatible stack; its test resolution is bundle-first specifically to
avoid split type identity. The adoption posture is therefore to exact-pin the
reviewed Vite Plus release and lockfile, accept the matching core/test aliases,
delete split-brain direct entrypoints, and record both reported component
versions and available upstream source revisions in release receipts.

The pilot's Cloudflare-plugin result still matters. T3 itself does not really
have one physical config: root policy is merged into server, web, Desktop, and
client-runtime configs with host-specific task, pack, plugin, and test rules.
OpenAgents should copy that hierarchy and keep aiur's environment-validating
build/test boundary rather than force a false one-file abstraction.

### B.10 The strongest credible end state

If choosing Option B, the clean version would look like this:

- one exact `.node-version` on the active Node LTS line, with compatible
  `engines`, local setup, owned CI, Docker, and release smokes;
- pnpm 11 with one root workspace/lockfile, catalogs, and strict
  `workspace:` dependencies plus an exact integrity-bearing package-manager
  pin and explicit native build-script policy;
- an exact-pinned Vite Plus release as the command layer, with its Node-manager
  behavior reinforcing the exact project pin and, while it bundles its own
  engines, those Vite/Rolldown/Vitest/Oxc/tsdown versions and source provenance
  recorded explicitly;
- a root `vite.config.ts` that owns shared check/fmt/lint/test/staged/task
  defaults and composes package overrides where hosts genuinely differ, while
  retaining separate configs for environment-validating plugins such as
  aiur's Cloudflare plugin;
- compiled ESM artifacts for public CLIs and packages, tested from packed
  tarballs under stock Node;
- Node-based production images with no `vite-plus` production dependency;
- no direct `Bun.*`, `bun:*`, `bun:test`, `bun.lock`, or Bun shebangs left in
  the supported path; and
- deployment parity receipts for the retained API, realtime services, SQLite
  stores, and Pylon orchestration; payment paths are decommissioned and deleted
  rather than ported because the accepted MVP excludes them.

This keeps the strategic center on Node and owned contracts. `vp` is the chosen
front door in this option, but it remains replaceable by its constituent tools
if its 0.2.x release train becomes a liability.

### B.11 What success would buy, in one table

| Dimension | Current Bun-first state | Full Option B end state |
| --- | --- | --- |
| Production runtime stewardship | Anthropic-owned Bun in API/service/payment paths | OpenJS-governed Node LTS |
| Server/tooling engine matrix | Bun servers plus Node-shaped Electron/Expo/npm tooling | Node as the server/tooling reference engine |
| Pylon install contract | Bun prerequisite; source TypeScript bin | Standard Node/npm install; compiled artifact |
| Root task model | 150 scripts, mostly hand-chained Bun verbs | Workspace DAG, filters, concurrency, and cache behind `vp run` |
| Dependency enforcement | Bun workspace resolution | pnpm strict direct dependencies and explicit workspace protocol |
| Test/build configuration | Bun tests plus per-host build/config islands | Shared Vite/Vitest/Oxc/tsdown defaults with explicit host overrides |
| Runtime coupling | 214-file Bun-API burn-down perimeter | Owned platform adapters; zero Bun API in the supported path |
| Toolchain exit | Runtime and toolchain concerns fused in Bun | Runtime stays Node if `vp` is removed or replaced |

### B.12 What it would *not* buy

The steelman should not turn into sales copy. Full Option B would not
automatically make tests faster, erase browser/Hermes/native runtime
differences, make Vite Plus GA-stable, or eliminate all vendor influence. Vite
Plus describes the audited release line as beta: stable but incomplete. The
task cache must be measured against OpenAgents' real filesystem-heavy tests;
only modeled `vp run` tasks are cached, and there is no built-in remote-cache
service (external cross-run cache reuse is experimental). Vitest parity must
be proven; pnpm strictness will initially break undeclared assumptions;
Node's built-in TypeScript stripping ignores `tsconfig.json` and
does not make source-TypeScript npm packages a good distribution format
([Node TypeScript docs](https://nodejs.org/api/typescript.html)).

TC-5 also rules out claiming immediate Vite Plus speed. Its dependency and
config numbers describe additive adoption, while its bundled-engine finding is
the explicit coherence contract of full adoption—not a reason to maintain two
authorities.

Those are migration costs and proof obligations, not evidence that the end
state has no value.

### B.13 Bottom line if the owner wants the conversion

If the strategic goals are to remove Anthropic from the production runtime,
standardize the server/tooling estate on a neutrally governed LTS, make Pylon a
normal Node-distributed product, and replace root-script entropy with a cached
workspace control plane, **Option B has durable benefits that Option C only
approaches asymptotically**. In that framing, the migration is not a speculative
tool swap; it is a deliberate portability, distribution, and monorepo-
governance program.

The owner has selected those structural outcomes and the full stack. Option C
is no longer the recommendation. The execution should mirror T3's ordering—
Node readiness, Effect TSGo/test parity, atomic pnpm/Vite Plus replacement,
focused stabilization—while going further to delete the temporary Bun runtime
branches at completion.

The payment path moves first, not last. Disable new money, reconcile pending
balances/intents, freeze applied migrations and historical receipts read-only,
withdraw active payment promises, shut down and revoke the money services, and
delete wallet/tip/payout/settlement code instead of porting it. That both honors
the MVP boundary and removes some of the hardest Bun-native storage/process
work from the conversion graph.

The separate
[`Sol conversion plan`](../sol/2026-07-14-node-pnpm-vite-plus-full-conversion-plan.md)
owns the phased contract and gates. This addendum records why the selected
destination is rational; it does not claim that implementation has started.

### B.14 Source-audit details worth copying literally

The strongest adoption pattern is more precise than “use `vp` everywhere”:

- T3's domain/dev scripts remain ordinary Node, Expo, Astro, and host commands;
  `vp run` owns package selection, ordering, filtering, and the shared verb.
- Root config owns defaults, while server, web, Desktop, and client packages
  own honest overrides. Server and Desktop model build dependencies and
  multiple `vp pack` entries locally.
- The current staged hook is formatter-only (`vp staged` → `vp fmt`); full
  correctness belongs at agent completion, pre-push, owned CI, and release.
- `vp check` is paired with a separate bounded-concurrency TSGo task because
  type-aware Oxlint/TSGo parity is not assumed.
- pnpm policy includes catalogs, overrides, `workspace:*`, package extensions,
  patches, supported architectures, and an allow/deny list for install scripts;
  the graph is a supply-chain contract, not just a lockfile format.
- T3's `@effect/vitest` integration requires a 124-line patch at the audited
  versions. OpenAgents should copy it only if its selected Effect/Vite Plus
  versions still need it, prove the single test identity, and delete it when
  upstream support lands.
- Public CLIs are compiled, packed, start with `#!/usr/bin/env node`, and are
  tested as artifacts. Node's source-work pin can be narrow while the compiled
  public artifact supports a broader separately tested engine range.
- Vite Plus managed Node verifies signed official SHASUMS and archive hashes;
  owned CI uses managed mode plus `vp install --frozen-lockfile`. Dynamic heavy
  host plugins should be lazy so lint/fmt/task config reads do not eagerly load
  every build plugin.

Finally, T3 still has stale Bun prose and a few optional Bun adapters after the
migration. Its precedent proves serious operational adoption, not automatic
perfection. OpenAgents needs an explicit final scan of commands, docs, images,
tarballs, adapters, and fixtures before it may say “full conversion.”

## Addendum — 2026-07-14: the decision is made; what tonight already built

Author note: this addendum is written by **Opus**, after a mid-session model
handoff from the Fable lane that authored the body and from the Sol lane that
authored the §B steelman. Per this repository's own no-silent-substitution
posture — the same law the Cursor/Composer teardown made load-bearing — the
byline is named rather than ghost-written under an earlier voice. The analysis
above stands; this is a shorter, execution-facing coda.

**The owner has decided: Bun comes out. Option C is dead; the destination is
full Option B (Node LTS runtime, pnpm workspace, Vite Plus command layer), on
the phased contract in
[`Sol conversion plan`](../sol/2026-07-14-node-pnpm-vite-plus-full-conversion-plan.md).**
This addendum does not re-argue that; it records what the same night's work
already contributed to the migration, because three of the plan's named proof
obligations moved from "asserted" to "evidenced" while this document was being
written.

### C.1 The extraction is already a countable, gated burn-down — not a vibe

BUN-1 ([#8779](https://github.com/OpenAgentsInc/openagents/issues/8779),
`23ffe398e7`) landed two things that convert "rip out Bun" from a slogan into a
tracked program:

1. **A dual-runtime SQLite seam** (`@openagentsinc/sqlite-runtime`): one
   synchronous contract with a `bun:sqlite` implementation and a `node:sqlite`
   implementation selected at runtime, with the Node path tested under real
   `node --test` rather than a mocked flag. The deepest Bun-native coupling in
   the codebase — `bun:sqlite` in the orchestration, sync, and wallet stores —
   now has a proven, conformance-tested escape hatch. One store
   (`khala-sync-client`) is already migrated onto it with zero wire change.
2. **A perimeter scan with a checked-in burn-down allowlist**
   (`scan:bun-api-perimeter`): it measured the exact extraction surface —
   **710 `Bun.*`/`bun:*` findings across 214 files** — grandfathered them, and
   fails the sweep on any *new* Bun usage.

That second artifact is the single most useful thing to come out of this whole
question, and it should be the migration's scoreboard. The honest cost of the
runtime extraction is not "+98 packages / 169 MB" (an additive-pilot artifact,
correctly discounted in §B.9) — it is **214 files, counting down to zero**.
Every store ported onto the seam, every `Bun.*` call replaced by an owned
platform adapter, shrinks the allowlist by a nameable amount, and the guard
guarantees the number only moves the right direction. The runtime flip is
safe to schedule when the perimeter reaches zero in the supported path, not
before — and now that is a measurement, not a judgment call.

### C.2 The typecheck-parity leg is de-risked; the toolchain half-measure is ruled out

Two pilots the same night bear directly on §B.9–B.12's proof obligations:

- **TC-4** ([#8775](https://github.com/OpenAgentsInc/openagents/issues/8775),
  `8b84af0043`): `@effect/tsgo` typechecked a real Effect-heavy package (~17.7k
  lines) at **~9× the speed and ~2.5× less memory than `tsc`, with zero
  semantic drift** (0 false positives, 0 missed errors). Sol's plan names
  "Effect TSGo/test parity" as a gate; for the typecheck half, that gate now
  has affirmative evidence. The remaining obligation is precise and known: the
  proven backend is a preview compiler installed by patching a binary inside
  `node_modules`, so it earns an opt-in package-local lane today and the
  canonical path only once a patch-free `@effect/tsgo` on `typescript>=7`
  ships. That is a real dependency, cleanly stated.
- **TC-5** ([#8776](https://github.com/OpenAgentsInc/openagents/issues/8776),
  `8158e6948f`): the additive Vite Plus pilot on aiur returned a *disqualifier*
  — `vp` silently ran its bundled Vite 8.1.3 instead of the workspace-pinned
  8.0.16, with no aliasing requested. Read narrowly that kills additive
  adoption. Read correctly, as §B.9 does, it **confirms the atomic-swap
  ordering**: there is no safe toe-in-the-water topology. It is T3's
  inverse-topology conversion (`b440dd18`: remove `bun.lock`/Turbo, migrate
  test imports, make pnpm authoritative, alias `vite` to exact-pinned core, and
  change CI/release/hooks together) or nothing. The half-measure that would
  have quietly split engine identity is off the table, which is a gift to the
  program, not a setback.

### C.3 The one caution worth carrying into execution

The payment-path-first ordering (§B.13) is right — it deletes rather than ports
the hardest Bun-native storage/process work and honors the MVP boundary. My
only addition, as the lane that inventoried the runtime coupling: **the SQLite
stores are the critical path, and BUN-1 piloted exactly one of them.** The seam
is proven; the remaining stores are mechanical but each needs dual-runtime
wiring plus its own Node-suite conformance before the runtime can flip. That
work is fully parallelizable now that the contract exists — it is the part of
the graph most amenable to fanning out, and the perimeter allowlist is the
shared ledger that keeps concurrent extraction honest.

### C.4 Bottom line

The strategic case (§B.1) was always the real case: **move the production
runtime off an Anthropic-owned engine onto a neutrally-governed foundation, and
push the corporate dependency outward to a replaceable, removable toolchain
layer.** Tonight did not decide that — the owner did — but it built the seam,
measured the surface, de-risked the typecheck gate, and eliminated the unsafe
shortcut. The migration starts from a stronger footing than the body of this
document assumed when it recommended Option C. Execute it on the Sol plan;
watch the perimeter count to zero; keep the runtime on Node the moment it can
be, and keep `vp` where it can always be pulled back out.

— Opus, 2026-07-14
