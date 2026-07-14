# Node, pnpm, and Vite Plus VP-3 parity receipt

- Class: receipt
- Date: 2026-07-14
- Status: complete; canonical package-manager and command authority remain
  assigned to VP-4
- Dispatch: no; use [#8797](https://github.com/OpenAgentsInc/openagents/issues/8797)
- Parent: [#8777](https://github.com/OpenAgentsInc/openagents/issues/8777)
- Owner: Sol runtime and toolchain conversion
- Proof command layer: Vite Plus `0.2.4`, Vitest `4.1.10`
- Destination runtime: Node `24.13.1`; proof host: Node `25.8.2`

## Result

The retained workspace test graph now executes through Vite Plus/Vitest rather
than `bun:test` or `node:test`. The conversion migrated approximately 1,200
test imports to the single `vite-plus/test` identity, provided explicit
projects for Node, Foldkit, AIUR, TanStack Start, web, API, and React Native,
and removed the obsolete Bun test adapter from AssuranceSpec. The proof config
is deliberately named `vite.config.vp3.ts`: VP-3 proves the destination while
leaving root package-manager, lockfile, hooks, and canonical command authority
unchanged until the atomic VP-4 cutover.

The complete sharded run passed:

- 2,392 test files;
- 19,898 passed tests;
- 60 explicit skips; and
- zero failures.

The skips are existing environment or product-boundary declarations: live
Khala sync, GCS HMAC, PostgreSQL backend, Gemini live/managed, and already
retired desktop-shell/settings cases. No unexplained skip was added to make
the migration green.

The proof also deletes obsolete payment, order, market, onboarding, and Sites
tests instead of porting them. Exact-route and OpenAPI projections reject the
retired capability graph, and agent-home no longer advertises wallet, payout,
payment, customer-order, paid rate-limit recovery, or Sites scopes/resources.
Historical mobile balances are preserved as records rather than falsely
zeroed during account deletion. Treasury itself is retired; the isolated
wallet-recovery procedure lives in
[`docs/ops/2026-07-14-vp1-treasury-wallet-recovery-runbook.md`](../ops/2026-07-14-vp1-treasury-wallet-recovery-runbook.md).

## Behavioral findings fixed

The migration exposed runtime differences that a mechanical import rewrite
would have missed:

- Pylon assignment progress used an interval that could queue overlapping
  async work under Node; it now uses a serialized recursive timeout and keeps
  lifecycle phase in operator status.
- child-process exit could win a race with stdout/stderr delivery; session
  completion now waits for both stream pumps to drain.
- retained HTTP and MCP test harnesses now await actual server readiness.
- hosted dispatch no longer double-counts a claimed unit of work.
- checkout verification uses the same Vite Plus globals and isolated-worktree
  contract as the root proof.
- the Khala CLI smoke uses Node and an absolute `tsx` loader instead of an
  ambient Bun execution assumption.
- exact route registration now filters every remaining retired-capability
  route before exposure.

## Host boundaries

The Vite Plus topology records these boundaries rather than concealing them:

- Cloudflare virtual modules use test aliases, and `effect-cf` is explicitly
  retained in the server-side dependency graph.
- React Native/Hermes has its own project and narrow native-module stubs.
- OpenTUI currently has a test-only stub because its package cannot be loaded
  by the proof Node host; VP-4 must remove or lazily isolate the production
  boundary before making Vite Plus canonical.
- SQLite tests use the promoted Node harness; the Bun comparison oracle is
  still temporary migration debt.
- one Vitest JSON-output shard completed all tests but did not terminate. The
  ordinary reporter rerun exited successfully with 148 passed files, one
  skipped file, 1,108 passed tests, and two skipped tests in 8.88 seconds.
  This is bounded reporter debt, not a hidden test failure.

The other 15 machine-readable shard reports contain 2,243 unique test files,
18,790 passes, 58 skips, and zero failures. Shards ran four at a time; observed
batch durations were generally 20–30 seconds. This is a parity receipt, not a
benchmark claim.

## Effect typecheck disposition

The Effect TSGo pilot remains a conditional go for a package-local parallel
lane, not canonical authority. On `@openagentsinc/khala-tools` it found zero
false positives, zero missed diagnostics, four cosmetic Effect language
suggestions, and 6/6 synthetic parity cases; warm median was 0.26 seconds for
TSGo versus 2.34 seconds for `tsc`. Canonical typechecking remains the
Effect-aware TypeScript path until patch-free TypeScript 7 compatibility and
the parity triggers in #8775 are met.

## OpenAgents architecture lint proof

Following T3 Code's local-plugin pattern, VP-3 adds
`packages/oxlint-plugin-openagents` with six executable laws:

1. no user-facing keyword routing;
2. no manually managed Effect runtime in tests;
3. no runtime dependencies in schema-contract packages;
4. package subpath imports only where the package contract requires them;
5. no renderer-side runtime credentials; and
6. no inline schema compilation.

Each rule reports the owning `AGENTS.md` or `INVARIANTS.md` authority. The
plugin's twelve valid/invalid fixture behaviors pass under `vp lint`, and its
TypeScript project passes with no emit. VP-4 owns canonical `jsPlugins` wiring
into root `vp check`; this phase proves the plugin and rule semantics without
creating a second lint authority.

## Exit and remaining debt

Static scans now find no real `bun:test` or `node:test` imports outside scanner
fixtures. Direct Bun references remain only in the explicitly named migration
perimeter: a handful of maintenance scripts, the SQLite comparison oracle,
test aliases/types, and the macOS process-sandbox compatibility branch. VP-4
ports the scripts, prunes the allowlist, installs the plugin, replaces the root
manifest/lock/hooks atomically, and folds #8772–#8774. VP-5 converts retained
release images; VP-6 removes the final Bun compatibility perimeter and proves
zero supported references.
