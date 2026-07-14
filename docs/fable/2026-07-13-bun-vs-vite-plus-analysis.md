# Bun vs Vite Plus — runtime/toolchain strategy analysis

- Date: 2026-07-13
- Author: Fable (workspace strategy lane)
- Status: decision input, not an execution claim. This document does not
  commit the repo to any migration; it exists so the runtime/toolchain
  decision is made on evidence rather than category confusion.
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

**Recommendation (§6): the hybrid.** Keep Bun as runtime where it is
load-bearing today; adopt the toolchain *contract* tool-independently
exactly as TC-1..TC-3 already specify; let TC-4/TC-5 pilots plus explicit
revisit triggers (§7) decide any future runtime move. Do not start a
wholesale Bun→Node migration now.

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

- Pros: maximal toolchain consolidation; standard runtime everywhere,
  matching the Electron/Expo surfaces that are Node-shaped anyway; one
  root config and one `vp check` verb; widest npm-package audience for
  Pylon.
- Cons: by far the most expensive option — rewriting ~170 files of `Bun.*`
  usage, replacing `bun:sqlite` in the orchestration store, sync client,
  and wallet storage, rebasing ~10 production Docker images, converting
  127 root scripts and the entire pre-push gate, and re-verifying the
  payments path, all for a 0.2.x toolchain; trades Anthropic stewardship
  for Cloudflare stewardship rather than reducing exposure; forfeits
  `bun test` speed on a 2,660-file suite with no measured replacement;
  directly competes with the live TC sequence instead of feeding it.

### Option C — Hybrid: keep the runtime, adopt the contract (recommended)

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

**Option C.** Concretely:

1. Execute TC-1..TC-3 now, on the existing Bun toolchain, exactly as filed
   (#8772, #8773, #8774). Nothing in the contract requires vp or Node.
2. Add the Bun-API containment rule to the TC-2 plugin scope and start the
   runtime-seam refactor with `bun:sqlite` (one adapter module, T3's
   `Sqlite.ts` as the reference pattern).
3. Run TC-4 (#8775) and TC-5 (#8776) as bounded pilots and judge them on
   measured speed, config deletion, and drift — not on vendor sentiment.
4. Defer the runtime decision, explicitly, to these **revisit triggers**:
   - Bun licensing, release cadence, or Node-compat posture materially
     changes under Anthropic `[public signals]`;
   - Vite Plus reaches 1.0 with a stable config surface and the TC-5 pilot
     shows decisive wins on our workloads;
   - the Bun-API adapter work gets the load-bearing surface small enough
     that a runtime swap becomes a bounded task instead of a program;
   - a business requirement (e.g. Pylon distribution, a partner's runtime
     constraint) forces runtime neutrality on a deadline;
   - our Anthropic concentration changes materially in either direction
     (e.g. model/harness diversification, or deeper platform integration
     that makes runtime alignment an asset instead of a risk).

Per docs/fable convention: this is decision input. No migration is approved
or scheduled by this document; TC-5 and the revisit triggers own the next
decision point.

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

### B.9 What the completed TC-5 pilot changes

The bounded aiur result landed after the original analysis:
[`docs/research/2026-07-14-vite-plus-pilot.md`](../research/2026-07-14-vite-plus-pilot.md),
with the closeout summarized on
[#8776](https://github.com/OpenAgentsInc/openagents/issues/8776#issuecomment-4966050487).
It is direct evidence against adopting Vite Plus 0.2.4 today:

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

Because the bounded pilot preserved the baseline toolchain, its +98 packages
is an incremental pilot delta, not a forecast of the net full-conversion
footprint; a wholesale cutover could delete direct Vite/Vitest and Bun-only
dependencies. The silent engine substitution and loss of workspace pin
authority remain regardless.

This changes the honest steelman in two ways.

First, the benefits in B.1-B.8 are **end-state benefits of Node, pnpm, explicit
artifacts, architectural seams, and a workspace control plane**. TC-5 supplies
no OpenAgents-specific performance or config-deletion evidence for Vite Plus
itself. A full conversion cannot cite that pilot as a win.

Second, if the owner still chooses Vite Plus 0.2.4 now, the coherent posture is
to make its bundled Vite/Vitest versions the explicit source of build
provenance, not pretend the workspace pins remain authoritative. Exact-pin
`vite-plus`, remove split-brain entrypoints where possible, record the embedded
engine versions in release receipts, preserve separate host configs where
plugins require them, and accept the dependency delta as the price of the
unified command layer. The alternative is to complete Node/pnpm/runtime
portability first and hold the final `vp` binding to TC-5's revisit trigger:
Vite Plus 1.0 with workspace-resolved or explicitly selectable engine versions.

That distinction keeps "full Node conversion" from being held hostage to one
pre-1.0 tool release while still treating Node + pnpm + Vite Plus as the desired
destination if the owner selects it.

### B.10 The strongest credible end state

If choosing Option B, the clean version would look like this:

- one pinned, active Node LTS line in `engines`, local setup, CI, Docker, and
  release smokes;
- pnpm 11 with one root workspace/lockfile, catalogs, and strict
  `workspace:` dependencies;
- an exact-pinned Vite Plus release as the command layer, with its Node-manager
  behavior optional rather than the only record of the runtime version and,
  while 0.2.x bundles its own engines, those Vite/Vitest versions recorded as
  explicit build provenance;
- a root `vite.config.ts` that owns shared check/fmt/lint/test/staged/task
  defaults and composes package overrides where hosts genuinely differ, while
  retaining separate configs for environment-validating plugins such as
  aiur's Cloudflare plugin;
- compiled ESM artifacts for public CLIs and packages, tested from packed
  tarballs under stock Node;
- Node-based production images with no `vite-plus` production dependency;
- no direct `Bun.*`, `bun:*`, `bun:test`, `bun.lock`, or Bun shebangs left in
  the supported path; and
- deployment parity receipts for the API, realtime services, SQLite stores,
  Pylon orchestration, and payments before each Bun image is retired.

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
differences, make Vite Plus 1.0-stable, or eliminate all vendor influence. The
task cache must be measured against OpenAgents' real filesystem-heavy tests;
Vitest parity must be proven; pnpm strictness will initially break undeclared
assumptions; Node's built-in TypeScript stripping ignores `tsconfig.json` and
does not make source-TypeScript npm packages a good distribution format
([Node TypeScript docs](https://nodejs.org/api/typescript.html)); and payment/
wallet behavior must be requalified on V8.

TC-5 also rules out claiming immediate Vite Plus speed, dependency reduction,
config deletion, or workspace-version control: the measured result was parity,
+98 packages / ~169 MB, zero deleted config lines, and bundled-engine
substitution. Those facts are the current adoption price until a revisit
trigger fires.

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

If the only desired outcome is one check command and better lint/format gates,
Option C remains the rational cheaper route. If the owner values the four
structural outcomes above enough to fund the cutover, Option B is a defensible
destination—provided it is executed service-by-service with dual-runtime
conformance, production parity receipts, and the payment path last, rather than
as a lockfile-and-shebang big bang.

On the current evidence, the case for **Node + pnpm** is materially stronger
than the case for **Vite Plus 0.2.4**. Choosing the full stack now can still be
rational, but it is a strategic bet on the future control plane and a conscious
acceptance of TC-5's engine-coupling cost—not a conclusion supported by the
pilot's speed or config results.

This addendum steelmans the destination; it does not itself approve or schedule
the migration.
