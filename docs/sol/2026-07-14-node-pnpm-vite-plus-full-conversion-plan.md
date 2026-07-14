# Node, pnpm, and Vite Plus full-conversion plan

- Class: contract
- Date: 2026-07-14
- Status: owner-authorized live issue program; implementation not yet claimed
- Dispatch: only through epic
  [#8777](https://github.com/OpenAgentsInc/openagents/issues/8777), its bounded
  leaves below, and [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md)
- Owner: Sol runtime and toolchain conversion
- Source snapshot: OpenAgents `180e073e281b95b5ce3b7409f1d62c6865be2a70`;
  T3 Code `c1ec1915fc16f3dc1ec5d47d9a97f6210a574526`; Vite Plus
  `5d61de0b4b0b75bf3fa1b2f4da407fd244c3c6dc`
- Decision evidence:
  [`Bun vs Vite Plus analysis`](../fable/2026-07-13-bun-vs-vite-plus-analysis.md),
  [`T3 Code teardown`](../teardowns/2026-07-13-t3-code-teardown.md), and
  [`TC-5 aiur pilot`](../research/2026-07-14-vite-plus-pilot.md)

## Live issue program

[#8777](https://github.com/OpenAgentsInc/openagents/issues/8777) is the only
cutover epic. The phase issues own dependency gates; the retained TC issues
are subordinate implementation leaves, not a second program.

| Role | Live issue | Disposition |
| --- | --- | --- |
| Epic | [#8777](https://github.com/OpenAgentsInc/openagents/issues/8777) | Full Node/pnpm/Vite Plus cutover; T3 topology; payments removed |
| VP-0 | [#8794](https://github.com/OpenAgentsInc/openagents/issues/8794) | Freeze, inventory, baselines, pins, and claim ledger |
| VP-1 | [#8795](https://github.com/OpenAgentsInc/openagents/issues/8795) | Reconcile, decommission, and delete non-MVP money paths |
| VP-2 | [#8796](https://github.com/OpenAgentsInc/openagents/issues/8796) | Node-native retained runtime and public CLIs |
| VP-3 | [#8797](https://github.com/OpenAgentsInc/openagents/issues/8797) | Effect TSGo and Vite Plus test/typecheck parity |
| VP-4 | [#8798](https://github.com/OpenAgentsInc/openagents/issues/8798) | Atomic pnpm/Vite Plus workspace-authority replacement |
| VP-5 | [#8799](https://github.com/OpenAgentsInc/openagents/issues/8799) | Host/release stabilization and Node production images |
| VP-6 | [#8800](https://github.com/OpenAgentsInc/openagents/issues/8800) | Bun excision, final matrix, and closure |
| VP-4a / TC-1 | [#8772](https://github.com/OpenAgentsInc/openagents/issues/8772) | Retained and folded: canonical root verbs |
| VP-3/4 / TC-2 | [#8773](https://github.com/OpenAgentsInc/openagents/issues/8773) | Retained and folded: invariant-enforcing Oxlint rules |
| VP-4b / TC-3 | [#8774](https://github.com/OpenAgentsInc/openagents/issues/8774) | Retained and folded: formatter-only staged hook and gate gradient |

TC-4 [#8775](https://github.com/OpenAgentsInc/openagents/issues/8775) and
TC-5 [#8776](https://github.com/OpenAgentsInc/openagents/issues/8776) remain
closed evidence. They are not reopened or counted as unperformed phases.

## Decision

OpenAgents will convert the supported TypeScript server and tooling estate to
**Node + pnpm + Vite Plus**, using T3 Code's integrated operating model rather
than adding `vp` beside the existing Bun stack.

The destination is:

- an exact Node 24 LTS project pin and Node/V8 as the reference server, CLI,
  test, repository-automation, and build runtime;
- pnpm 11 as the sole workspace/package-manager authority, with one root
  lockfile, catalogs, `workspace:*` edges, explicit build-script policy, and
  exact package-manager provenance;
- an exact-pinned Vite Plus release as the toolchain authority for Vite,
  Rolldown, Vitest, Oxlint, Oxfmt, tsdown packaging, staged-file checks, and the
  workspace task graph;
- shared root Vite Plus config plus honest package-local configs for Electron,
  Expo, Cloudflare, server packaging, and other host-specific constraints;
- the same completion verbs for humans, agents, pre-push checks, owned CI, and
  releases; and
- zero Bun runtime, package-manager, test, lockfile, shebang, container, or API
  dependency in the supported destination.

This is a full conversion, not an indefinite dual-runtime policy. Temporary
Node/Bun adapters are migration scaffolding and must have deletion gates.
Browser JavaScript, Electron renderers, React Native/Hermes, and native
Rust/Swift/Kotlin remain their actual host runtimes; "one runtime" here means
the server and JavaScript tooling baseline.

This document is the explicit model-boundary exception for the invariant
change in this commit: it changes repository direction and freezes new Bun/
payment growth, but changes no production runtime, route, balance, secret,
container, or promise state by itself. Existing runtime and money invariants
remain binding until a claimed implementation change supplies the matching
tests, model/smoke evidence, transition receipts, and operational proof.

## Why the T3 precedent changes the decision

T3 did not prove Vite Plus with an additive one-app pilot. Its sequence was:

1. make server and Desktop execution Node-native in `8dba2d64` (#2098);
2. move applicable typechecks to Effect TSGo in `6b3050ee` (#2851);
3. replace Bun/Turbo/Vitest/tsdown/Oxlint/Oxfmt workspace machinery atomically
   with pnpm/Vite Plus in `b440dd18` (#2899), touching 299 files; and
4. land narrow install, task-graph, test-import, publishing, Electron, hook,
   and CI stabilization commits immediately afterward.

At the audited snapshot, T3 pins Node `^24.13.1`, pnpm `11.10.0`, Vite Plus
`0.2.2`, aliases `vite` to the matching Vite Plus core, imports tests from
`vite-plus/test` or its patched `@effect/vitest`, packages Node executables
through `vp pack`, and gates agents on `vp check` plus a separate Effect-aware
typecheck. Its public server artifact starts with Node even though a few
optional Bun adapters remain. OpenAgents copies the integrated topology and
the migration order; it does not copy those retained Bun branches.

The completed TC-5 aiur pilot remains correct for what it measured: adding
Vite Plus to one Bun workspace app produced duplicate engines, +98 packages,
about 169 MB, no config deletion, and no material speed win. Those are the
expected properties of the transitional topology T3 deliberately removed.
TC-5 explicitly prohibited the root Vite Plus core alias, test-framework
rewiring, monorepo task graph, pnpm lockfile, and deletion of the displaced
stack. It therefore did not test this destination. Its bundled-engine finding
becomes an adoption rule: Vite Plus's exact bundled engines are declared build
provenance, not silent substitutes for fictional independent workspace pins.

## What the Vite Plus source adds to the case

The audited Vite Plus source supports the following concrete benefits and
limits:

- The Rust global CLI can provision the project-resolved Node runtime and
  package manager before invoking JS-backed commands. Node is the only
  supported managed runtime today; an exact `.node-version` is therefore part
  of the product, not local setup trivia.
- Managed Node downloads verify official signed SHASUMS and archive hashes and
  install atomically. Owned CI and release builders keep managed mode enabled;
  they do not use signature-skip flags or `vp env off`.
- `vp run` reads the normal workspace dependency graph and supplies recursive,
  transitive, filtered, dependency-ordered execution. Configured `run.tasks`
  can cache and automatically track inputs and outputs.
- Direct `vp build`, `vp test`, `vp lint`, `vp check`, and `vp pack` are not
  task-cache entries. Cache claims apply only when those commands are invoked
  through correctly modeled `vp run` tasks. The built-in store is local at
  this snapshot under `node_modules/.vite/task-cache`.
- Package scripts are uncached by default; configured tasks are cached by
  default. Environment variables that change output must be named and
  fingerprinted. Secrets and volatile CI variables must not enter cached
  outputs accidentally. There is no built-in remote-cache service; external
  cross-run cache restore/save is experimental and is not part of this
  conversion's acceptance claim.
- Vite Plus intentionally bundles or projects a coherent Vite/Rolldown/
  Vitest/tsdown/Oxc stack. The repo pins both Vite Plus releases and upstream
  source revisions. OpenAgents records the selected Vite Plus package,
  lockfile integrity, reported component versions, and—when published by the
  release—the upstream source revisions in build receipts.
- `vp check` is not automatically an Effect-aware typecheck. At this snapshot
  typechecking is coupled to Oxlint's type-aware/type-check settings, while T3
  disables that route for TSGo compatibility. The separate `vp run typecheck`
  gate remains until a parity issue proves it redundant.
- `vp migrate --full` supplies useful rewrites and setup but is explicitly not
  a push-button conversion. It cannot remove OpenAgents-specific Bun APIs,
  payment paths, custom build scripts, or every arbitrary `vite` import.

These facts make the toolchain legible. They also set the rollback boundary:
production artifacts execute on Node and do not need Vite Plus installed, so a
future toolchain replacement does not reopen the runtime migration.

## Destination contract: copy T3's system, not its incidental values

### Runtime and package manager

- Commit an exact `.node-version` on the Node 24 LTS line. Use a compatible
  `engines.node` range for source work and separately declare a broader range
  only for compiled public packages after their artifact matrix passes.
- Pin pnpm 11 exactly in `packageManager`, including an integrity hash where
  supported. Do not carry conflicting `devEngines.packageManager` authority.
- Replace root and nested `bun.lock` files with one root `pnpm-lock.yaml`.
  Nested independently released projects need an explicit exception rather
  than an accidental second workspace.
- Generate `pnpm-workspace.yaml` from the actual workspace list. Put shared
  versions in one catalog, internal packages on `workspace:*`, install scripts
  in an explicit allow/deny policy, and native architecture support in a
  reviewed list.
- Production containers use a pinned official Node image or Vite Plus's
  verified exported Node binary in a clean runtime stage. They contain only
  production dependencies and built artifacts—not `vp`, pnpm caches, test
  code, or source-only tooling.

### Vite Plus and config hierarchy

- Select one reviewed Vite Plus release at the implementation snapshot and pin
  it exactly. T3's `0.2.2` proves the pattern; it is not a command to downgrade
  OpenAgents from a later reviewed release.
- Use the release's prescribed exact `vite` alias to
  `@voidzero-dev/vite-plus-core`, Vitest override, and pnpm
  `packageExtensions`. Accept bundled engine substitution explicitly and
  delete split-brain direct engines where the host permits it.
- If the installed `@effect/vitest` still imports upstream Vitest directly,
  carry a small reviewed patch like T3's 124-line patch, with a focused test
  proving Effect tests use the same Vite Plus test identity. Delete the patch
  as soon as upstream compatibility makes it unnecessary.
- Root `vite.config.ts` owns shared test defaults, formatter policy, lint
  policy, staged commands, and root task defaults. Package configs merge the
  root and own host plugins, pack entries, test projects, timeouts, and task
  dependencies. Aiur's Cloudflare build/test separation remains explicit.
  Keep `run.tasks` statically analyzable where possible and lazy-load heavy
  host plugins so lint/format/task config reads do not eagerly execute every
  build plugin.
- Model cache-worthy builds/checks in `run.tasks`; use `cache: false` for dev
  servers, deployment, stateful integration tests, release mutation, and other
  side-effecting tasks. Use `--fail-if-no-match` in gates where a missing
  package selection is failure.
- Do not claim cached `vp pack` outputs until its current `dist` exclusion/TODO
  is resolved and a local proof demonstrates correct restoration.

### One definition of green

The target root vocabulary is small and stable:

```text
vp install
vp check
vp run typecheck
vp run test
vp run build
```

Package-specific smokes remain `vp run --filter <package> <task>`. The exact
root tasks may split by host or risk, but contributors and agents do not need
to know an underlying package-manager command.

OpenAgents does **not** copy T3's GitHub Actions workflow because repository
law forbids GitHub-hosted CI. The owned GCE/local runner setup performs the
same role as `setup-vp`: install the exact CLI, resolve the exact Node and pnpm
pins, run `vp install --frozen-lockfile`, restore only safe caches, run the same
verbs, and publish a version receipt. `.vite-hooks/pre-commit` is only
`vp staged`; staged config formats only. Full check, typecheck, tests, and host
smokes remain pre-push, agent-completion, owned-CI, and release gates.

Use `effect-tsgo patch && vp config --no-agent` in the prepare path if still
required. `--no-agent` preserves the hand-owned repository contract instead
of letting generated generic guidance overwrite it.

### Architecture enforcement

- Port `oxlint-plugin-openagents` into root Vite Plus config and keep its rules
  tied to named invariants.
- Freeze legacy violations behind checked-in baselines, reject every new
  occurrence, and burn the baseline down by phase. This copies T3's pragmatic
  `no-manual-effect-runtime-in-tests` technique.
- Keep TSGo for Effect-heavy Node/browser packages and explicit `tsc`, Expo,
  Astro, Cloudflare, generated-code, and native exceptions until each is
  proven compatible. A common default is not a false universal config.
- End with a zero-Bun scan across source, tests, manifests, Dockerfiles,
  scripts, hooks, docs, examples, fixtures that represent supported commands,
  and published tarballs.

## Payment decision: decommission and delete, do not port

Payments, markets, and settlement are already explicit non-goals of the
accepted MVP ProductSpec. They will not consume Node/Vite Plus migration work.
The conversion removes active money paths before the atomic workspace cutover.

"Remove" means remove executable product and authority, including:

- the Bun-only `mdk-sidecar`, `mdk-treasury`, and `mdk-tips-buffer` services,
  their Dockerfiles, deploy/smoke scripts, dependencies, workspace entries,
  service accounts/secrets after shutdown, and live routing;
- new-money Worker/API mutations for billing, checkout, paid plans, Stripe/IAP,
  Lightning/MDK/L402, Forum tips, treasury payout, settlement jobs, and related
  cron/queue producers;
- Pylon wallet, receive/send, payout-target, earning, paid-assignment,
  settlement, Spark backup/sweep, tip-readiness, and paid-market CLI/UI/runtime
  paths;
- Forum/web/Desktop/mobile payment, tip, wallet, balance, payout, checkout,
  plan-purchase, and earnings affordances; and
- payment-only tests, dependencies, secrets, deploy manifests, product copy,
  and product promises after their historical status is preserved.

Deletion is not permission to abandon live value or rewrite history. The
operational sequence is mandatory:

1. disable new deposits, purchases, tips, paid assignments, automated payouts,
   and every other new-money ingress;
2. inventory and reconcile pending intents, unsettled rows, owned balances,
   refund/return/sweep obligations, and public promises without printing or
   moving secrets outside their current authority;
3. complete the legally and operationally required return/settle/export path,
   with explicit blockers for anything that cannot close;
4. freeze immutable ledgers, applied database migrations, and redacted receipt
   history read-only; mutation endpoints return a typed retired/`410` outcome;
5. scale money services to zero, remove routes and schedules, revoke runtime
   access, and delete their secrets through the owning operational authority;
6. delete executable code, tests, dependencies, configs, UI, and deployment
   surfaces; and
7. prove that no supported MVP process can receive, hold, quote, charge, tip,
   pay, settle, or claim earnings.

Do not drop already-applied financial tables, edit old migrations, erase
historical settlement facts, or break stable redacted receipt URLs merely to
make a source scan empty. One Node-compatible, read-only archive/projection may
remain if it is needed to reconcile owner balances, satisfy record retention,
or keep old receipts dereferenceable. It has no wallet keys, mutation, cron,
new records, payout, pricing, or product-promise authority.

No-spend execution remains explicit: retained MVP work must say
`paymentMode: no-spend` (or a narrowly retained unpaid-smoke state),
`settlementState: not_applicable`, and `payoutClaimAllowed: false`. Removing
payment rails must not accidentally make provider usage, managed compute, or
other external cost free or unmetered.

Active payment product promises transition to a stable-ID withdrawn/retired
state with historical evidence intact; they are not deleted or left green.
Payment restoration later is a new product program requiring a new ProductSpec,
threat model, custody and recovery design, ledger/migration plan, provider and
jurisdiction review, invariants, behavior contracts, tests, and live receipts.
It does not start by reverting this deletion commit.

## Implementation sequence

Each phase is a bounded live issue or small dependency-ordered issue group.
Serial ownership is required for `package.json`, lockfiles,
`pnpm-workspace.yaml`, root `vite.config.ts`, root TypeScript configs, hooks,
Docker bases, `AGENTS.md`, and invariant ledgers.

### VP-0 — freeze, inventory, and choose exact versions ([#8794](https://github.com/OpenAgentsInc/openagents/issues/8794))

Implementation evidence: the
[`VP-0 baseline receipt`](./2026-07-14-node-pnpm-vite-plus-vp0-baseline.md)
records the deterministic 37,675-match inventory, exact destination pins,
clean-source measurements, red parity prerequisites, claim ledger, and
temporary no-growth gate. It changes no runtime authority.

- Freeze new Bun APIs, Bun-only packages, payment work, and new direct
  Vite/Vitest/Oxc/tsdown configuration.
- Refresh the Bun perimeter by category: runtime, test, script, package
  manager, Docker, SQLite, subprocess, HTTP, file, bundler, binary, and docs.
- Record baseline wall-clock, failure, and artifact hashes for root checks,
  representative tests, Desktop, Pylon, API, aiur, realtime services, and
  public package tarballs.
- Select exact Node, pnpm, Vite Plus, TSGo, TypeScript, Effect, and engine
  aliases from a reviewed lockfile. Record Vite Plus component versions and
  available upstream source revisions.
- File the remaining phases with exact file claims and rollback points.

Exit: reproducible inventory, pins, gates, issue DAG, and no new perimeter
growth. No runtime claim changes.

### VP-1 — remove the non-MVP payment graph ([#8795](https://github.com/OpenAgentsInc/openagents/issues/8795))

- Execute the inflow stop, reconciliation, read-only freeze, service shutdown,
  secret revocation, promise withdrawal, route tombstone, and code deletion
  sequence above.
- Update root, `openagents.com`, Cloud, Pylon, Forum, product-promise, and
  behavior-contract invariants in the same changes that remove their code.
- Remove payment-only Bun perimeter entries instead of porting them.

Exit: no active money mutation or custody process, no green payment promise,
no payment-only service in the workspace/deploy graph, pending balances either
closed or explicitly blocked under a read-only owner, and historical receipts
still truthful.

### VP-2 — make retained production code Node-ready ([#8796](https://github.com/OpenAgentsInc/openagents/issues/8796))

- Put filesystem, process, subprocess/PTY, HTTP/WebSocket, SQLite, hashing,
  executable packaging, and runtime detection behind owned Effect services or
  narrow platform adapters.
- Promote the existing `@openagentsinc/sqlite-runtime` Node implementation and
  conformance suite; convert remaining `bun:sqlite` stores to `node:sqlite` or
  another selected Node driver.
- Compile Pylon and other public CLIs to ESM/declarations with a Node shebang;
  smoke the packed tarball under stock Node with no source-TypeScript runtime.
- Run representative retained services and tests under the exact Node pin
  while Bun remains only as a temporary comparison oracle.

Exit: retained production entrypoints have Node implementations and parity
tests; any remaining Bun adapter has a named deletion phase.

### VP-3 — establish Effect TSGo and Vitest parity ([#8797](https://github.com/OpenAgentsInc/openagents/issues/8797))

- Move applicable typechecks to TSGo with bounded concurrency, retaining named
  host exceptions.
- Convert `bun:test` imports, mocks, fake timers, snapshots, process lifecycle,
  and test preload behavior to `vite-plus/test`/`@effect/vitest` semantics in
  mechanical batches.
- Prove interruption/finalizer, fake-time, SQLite, subprocess, WebSocket,
  renderer, and resource-heavy suites specifically; green test counts alone
  are insufficient.
- Prepare the `@effect/vitest` patch/package-extension only if the selected
  versions still require it.

Exit: the future Vite Plus test and typecheck lanes pass against the retained
Node code without weakening assertions or excluding failing suites.

### VP-4 — atomic pnpm/Vite Plus workspace cutover ([#8798](https://github.com/OpenAgentsInc/openagents/issues/8798))

Land one coordinated integration change, like T3's #2899:

- add `.node-version`, `pnpm-workspace.yaml`, exact package-manager and Vite
  Plus pins, catalog, overrides, package extensions, patches, and allow-builds;
- generate the sole `pnpm-lock.yaml`; delete root/nested Bun lockfiles;
- add root and package Vite Plus configs and modeled task dependencies;
- replace root/package scripts, hooks, agent gates, owned-CI setup, build,
  test, packaging, release, and deploy commands together;
- delete displaced Turbo/direct Vitest/tsdown/Oxlint/Oxfmt configs and
  dependencies where Vite Plus owns the equivalent;
- keep host-specific configs whose plugins require isolation; and
- run install, check, typecheck, test, build, Desktop packaging, Pylon tarball,
  API/aiur/realtime image, and owned-runner smoke gates before merge.

Do not land a mixed root with both lockfiles or two authoritative test/build
engines. A failed cutover is reverted as one integration unit while the
already-landed Node adapters remain useful.

Exit: fresh clone/bootstrap and every normal root gate use Node/pnpm/`vp`; the
displaced toolchain is absent rather than duplicated.

### VP-5 — stabilization and production image cutover ([#8799](https://github.com/OpenAgentsInc/openagents/issues/8799))

- Expect short follow-ups for native installs, Electron packaging, Expo/Metro,
  Cloudflare plugins, release manifests, publishing overrides, Windows paths,
  cache inputs, and flaky resource-heavy tests. T3 required these; schedule
  them instead of calling them surprises.
- Convert retained Cloud Run/runtime images one at a time, with readiness,
  graceful shutdown, filesystem, SQLite, subprocess, latency, memory, and
  rollback evidence.
- Keep Vite Plus and dev dependencies out of final images. Record exact Node,
  pnpm, Vite Plus, Vite/Rolldown/Vitest/Oxc/tsdown, lockfile, source, and
  artifact hashes in release receipts.
- Verify local task-cache correctness before enabling caching for a task. Do
  not advertise remote caching.

Exit: retained deployed services execute Node artifacts with service-specific
rollback proof; owned runners and releases use the same root verbs.

### VP-6 — excise Bun and close ([#8800](https://github.com/OpenAgentsInc/openagents/issues/8800))

- Delete temporary Bun adapters, `@types/bun`, Bun platform/storage packages,
  Bun shebangs, Docker bases, scripts, install docs, command examples, and
  perimeter allowlists.
- Scan tracked files and packed/deployed artifacts for live `Bun.*`, `bun:*`,
  `bun:test`, Bun commands, Bun lockfiles, and Bun runtime dependencies.
  Historical analysis and literal user-command fixtures may remain only when
  clearly non-operational and excluded by a reviewed rule.
- Run clean-machine macOS/Linux and supported Windows bootstrap, full gates,
  public package install, signed Desktop, mobile, Cloudflare, API, Pylon, and
  retained service smokes.
- Update docs and examples; T3's stale Bun references demonstrate why this is
  a separate acceptance gate.

Exit: zero supported Bun path, no dual authority, no payment runtime, exact
release receipts, and owner acceptance of the installed/deployed Node result.

## Gates and falsifiers

Full adoption fails closed if any of the following remains at closure:

- a supported production or CLI entrypoint requires Bun or raw TypeScript from
  `node_modules`;
- both Bun and pnpm lockfiles or both direct and bundled test/build engines are
  authoritative;
- a payment mutation, key-bearing wallet, payout/tip/settlement service, green
  payment promise, or new-money route remains reachable;
- money was stranded, silently swept, erased, or made historically
  unverifiable during deletion;
- `vp check` is represented as Effect-aware typecheck without diagnostic
  parity, or tests were skipped/relaxed to make the cutover green;
- a cacheable task omits behavior-affecting inputs/environment or replays
  incorrect artifacts;
- owned CI, agent completion, pre-push, and release use different definitions
  of green;
- Vite Plus's bundled engine versions are absent from provenance;
- a production image contains Vite Plus/tooling when only built Node output is
  required; or
- the repository claims full completion while Bun compatibility remains an
  unnamed permanent fallback.

## Success

The conversion succeeds when a fresh contributor or owned runner installs one
exact Node/pnpm/Vite Plus stack, runs one root verb set, and produces the same
checked/tested/built artifacts that are packaged and deployed under Node;
public CLIs install normally through npm-compatible workflows; architecture
law runs in the same static gate agents use; payment code is absent from the
MVP runtime rather than expensively ported; historical money facts remain
truthful; and removing Vite Plus later would be a toolchain replacement, not a
second production-runtime migration.
