# Node, pnpm, and Vite Plus VP-0 baseline receipt

- Class: receipt
- Date: 2026-07-14
- Snapshot: 2026-07-14
- Status: completed VP-0 evidence
- Dispatch: no; use the live phase issues and claim protocol
- Owner: Sol runtime and toolchain conversion
- Issue: [#8794](https://github.com/OpenAgentsInc/openagents/issues/8794)
- Parent: [#8777](https://github.com/OpenAgentsInc/openagents/issues/8777)
- Source revision: `0ccbb57de29bbe2934c3291bf9570a9f8348e919`
- Proof rung: clean-checkout baseline plus committed deterministic policy gate
- Final disposition: retain until VP-6 closes the conversion and archives its
  superseding clean-machine receipt

## Result

VP-0 freezes growth without changing a production runtime, deployment, money
state, secret, root lockfile, or product behavior. The committed Node-native
scanner inventories every Git-tracked text surface in scope, assigns every hit
to one owner phase and disposition, collapses the transcript archive to a
single historical exception, and fails on any new or duplicated authority.
Applied migrations are stronger: all 407 are hash-locked, so removal or byte
change fails even when the aggregate inventory shrinks.

The baseline contains 8,915 classified entries and 37,675 exact matches. There
are zero unclassified entries. The generated evidence is
[`node-vp-cutover-inventory.v1.json`](./evidence/node-vp-cutover-inventory.v1.json).
It stores paths, categories, phases, dispositions, counts, and normalized-line
hashes rather than source contents or credentials.

This receipt deliberately preserves red baseline gates. VP-0 establishes the
ledger and prevents growth; it does not claim that the Bun source baseline was
green or that Node/pnpm/Vite Plus is installed.

## Reproduction

From a checkout with dependencies installed:

```sh
node scripts/node-vp-cutover-inventory.mjs --check
node --test scripts/node-vp-cutover-inventory.test.mjs
bun scripts/bun-api-perimeter-scan.ts
```

To reproduce the committed snapshot at its exact source revision:

```sh
node scripts/node-vp-cutover-inventory.mjs \
  --write-baseline \
  --source-commit 0ccbb57de29bbe2934c3291bf9570a9f8348e919
```

The first command is wired into the transitional fast policy gate. The second
proves classification determinism, burn-down acceptance, rejection of new or
duplicated authority, migration immutability, and exclusion of untracked local
files. The existing Bun perimeter now reports 627 findings: two named seams,
196 of 196 grandfathered files, and zero new violations. VP-0 made a one-time
snapshot correction for `apps/pylon/src/harness-mcp-server.ts`; VP-2 owns its
removal and the correction is not permission for further growth.

## Inventory

| Owner phase | Exact matches | Disposition |
| --- | ---: | --- |
| VP-1 | 22,437 | withdraw, reconcile, tombstone, and delete non-MVP money authority while preserving history |
| VP-2 | 5,970 | port retained runtime, services, and public CLIs to stock Node |
| VP-3 | 5,513 | convert tests/fixtures and prove Effect TSGo plus Vite Plus parity |
| VP-4 | 1,706 | atomically replace package, config, command, and hook authority |
| VP-5 | 336 | stabilize retained release paths and production images |
| VP-6 | 733 | remove remaining deprecated/docs references or record bounded historical exceptions |
| historical | 978 | retain read-only receipts, migrations, and collapsed transcript history |

The largest category counts are 19,668 money-authority matches, 7,702 Bun
commands, 2,720 direct-tool references, 2,017 Bun module references, 1,413
`bun:test` references, 1,254 Bun API references, 501 money-named path surfaces,
407 applied migrations, 373 secret-binding names, 370 money-rail dependency
references, 154 Bun shebangs, 73 release surfaces, and 18 runtime images.
Overlap is intentional: one line may establish several independently enforced
obligations. The committed VP-0 gate and receipt account for the small increase
from the pre-tooling measurement; they are part of the frozen landing snapshot.

The burn-down rule permits a signature count to decrease or disappear. It
rejects a new path/category/signature or increased multiplicity. Historical
and generated classifications are explicit entries, not scanner omissions;
only the scanner, its generated baseline, and generated Sol projections are
self-excluded. Binary files are counted and skipped.

## Destination pins and provenance

These are selected destination pins, not dependencies installed by VP-0.
OpenAgents keeps its current Effect beta and does not copy the Vite Plus
repository's older development Node/pnpm pins.

| Component | Selected pin | Provenance |
| --- | --- | --- |
| Node | `24.13.1`; engine `^24.13.1` | exact Node 24 LTS destination |
| pnpm | `11.10.0` | exact package-manager destination; Corepack/package-manager integrity is verified again at VP-4 |
| Vite Plus | `vite-plus/core@0.2.4` | Vite Plus source `5d61de0b4b0b75bf3fa1b2f4da407fd244c3c6dc` |
| Vite compatibility alias | `vite@npm:@voidzero-dev/vite-plus-core@0.2.4` | same reviewed Vite Plus source |
| Effect TSGo | `@effect/tsgo@0.13.2` | T3 adoption proof plus current registry artifact |
| native TypeScript preview | `@typescript/native-preview@7.0.0-dev.20260604.1` | T3 exact adoption pin |
| TypeScript | `6.0.3` | exact compiler compatibility pin |
| Vitest override | `4.1.10` | exact test-runner compatibility pin |
| Oxlint / Oxfmt | `1.73.0` / `0.58.0` | Vite Plus CLI component pins |
| oxlint-tsgolint | `0.24.0` | Vite Plus CLI component pin |
| bundled Vite / Rolldown / tsdown | `8.1.3` / `1.1.5` / `0.22.4` | component versions in reviewed Vite Plus `0.2.4` source |
| Effect | existing `4.0.0-beta.94` | preserve the repository's newer landed Effect line |

Relevant source anchors are upstream Vite
`578ffb80d46940f3b99cd96ed609f8b3a0ac5ede`, Rolldown
`f09947ab017d6df74299f691853dcfc4f4f0f86e`, and T3 Code
`c1ec1915fc16f3dc1ec5d47d9a97f6210a574526`. The T3 sequence used as the
operational model is Node runtime commit
`8dba2d6484e283a4211b53fe6d3273e6e4c962d0`, TSGo commit
`6b3050ee740f510ad06c12a91ecd7e338792dce0`, and atomic Vite Plus cutover
`b440dd1812a7fdc2cf569941328d2368ac0169b7`.

## Clean-source baseline

The measurement checkout was detached at the exact source revision on Darwin
25.4.0 arm64, Apple M5 Max, 128 GiB RAM. The host had Node `v25.8.2` and Bun
`1.3.11`; this records the source environment and does not replace the selected
Node 24 destination. Root `bun.lock` SHA-256 was
`db9c52c88ad43be4b13e33719e685439b589a372a036b2d2c9eb8ba5dbaef144`.

| Measurement | Result | Wall time | Peak RSS |
| --- | --- | ---: | ---: |
| cold `bun install --frozen-lockfile` | pass; 3,553 packages | 13.53 s | 2.46 GiB |
| warm install | pass; four packages refreshed | 5.43 s | 2.43 GiB |
| installed tree | 5,347,820 KiB; 121,945 files | n/a | n/a |
| `bun run check:fast` | pass; 19 policy tests | 5.13 s | 155 MiB |
| `bun run lint` | pass against committed baselines | 13.40 s | 1.25 GiB |
| `bun run typecheck` | fail in API worker after 76-target dispatch | 136.15 s | 4.22 GiB |
| `bun run test` | fail at Desktop verify: 1,282 pass, 39 skip, 3 fail | 241.94 s | 1.16 GiB |
| AIUR build | pass | 7.33 s | 766 MiB |
| `openagents.com` web + Worker dry-run build | pass | 12.92 s | 977 MiB |
| public ProductSpec/AssuranceSpec pack | pass | 0.81 s | 105 MiB |
| Pylon release gate | fail: 2,453 pass, 3 skip, 15 fail, 9 errors | 265.79 s | 787 MiB |

The typecheck blockers are Cloudflare `ExecutionContext<unknown>` fixtures
missing the required `tracing` member, one nullable Artanis forum-delivery test
argument, and the unsupported `readonly` SQLite option. VP-2 owns retained
runtime/SQLite fixes and VP-3 owns the test/compiler conversion.

Desktop failed one 60-second 20k-entry workspace-scale case and two five-second
Git-review cases. The oversized-diff case also observed `stale_status` instead
of `diff_too_large` after the timeout. Pylon exposed a 20-second harness-adapter
timeout and several five-second Git-worktree materialization timeouts with
follow-on typed checkout errors. These are destination parity blockers, not
VP-0 regressions.

The web/API build was a Wrangler dry run only; it made no deployment. It also
proved that payment containers and bindings remain in the build graph, which
VP-1 must retire before VP-4. The public pack produced:

- `openagentsinc-product-spec-0.1.0.tgz`: 19,985 bytes,
  SHA-256 `42e2ecaec2c81d1b07209f6b3ddc7fea7c01017fa59766876a2c08252817d37f`;
- `openagentsinc-assurance-spec-0.1.1.tgz`: 63,354 bytes,
  SHA-256 `8c1bd8a9399e692510c56a07c1356b580387c85498131ea74ebfbf5f88362046`.

## Dependency and claim ledger

| Phase | Live issue | Depends on | Hot claims | Exit and rollback |
| --- | --- | --- | --- | --- |
| VP-1 | [#8795](https://github.com/OpenAgentsInc/openagents/issues/8795) | VP-0 | money routes/services/secrets/migrations/promises | Stop ingress and reconcile first; preserve migrations/receipts. Roll back before destructive deletion by restoring the read-only freeze, never by reopening spend. |
| VP-2 | [#8796](https://github.com/OpenAgentsInc/openagents/issues/8796) | VP-0; disjoint from VP-1 | retained runtime adapters, SQLite, Pylon/public CLIs | Stock Node parity and packed-CLI smoke. Roll back adapter-by-adapter while Bun remains a comparison oracle. |
| VP-3 | [#8797](https://github.com/OpenAgentsInc/openagents/issues/8797) plus [#8773](https://github.com/OpenAgentsInc/openagents/issues/8773) | VP-1 and VP-2 | Effect TSGo, tests, Vite Plus/Oxlint parity | Both old/new oracle comparison and invariant lint pass. Revert config/rule pilots without changing runtime authority. |
| VP-4 | [#8798](https://github.com/OpenAgentsInc/openagents/issues/8798) plus [#8772](https://github.com/OpenAgentsInc/openagents/issues/8772) and [#8774](https://github.com/OpenAgentsInc/openagents/issues/8774) | VP-1 through VP-3 | root manifests/lock/config/scripts/hooks/AGENTS | One atomic pnpm/Vite Plus authority and clean-machine gate. Roll back the single cutover commit to the frozen Bun baseline; do not run mixed lockfile authority. |
| VP-5 | [#8799](https://github.com/OpenAgentsInc/openagents/issues/8799) | VP-4 | retained hosts, release paths, Docker images | Node production/release matrix passes. Roll back each image/release target to its last signed retained artifact. |
| VP-6 | [#8800](https://github.com/OpenAgentsInc/openagents/issues/8800) | VP-5 | final Bun deletion, docs, historical exceptions | Zero supported Bun path and final clean-machine receipt. Roll back only bounded deletions that violate retained history; never restore runtime authority. |

The dependency graph remains
`VP-0 -> {VP-1, VP-2} -> VP-3 -> VP-4 -> VP-5 -> VP-6`.
#8772's handoff is explicitly deferred to VP-4 because its final acceptance
requires the Node/pnpm/Vite Plus topology; its partial Bun runner is evidence,
not completion. VP-1 and VP-2 may overlap only through valid, disjoint claims.

## Rollback and non-claims

Reverting the VP-0 tool-and-evidence commits removes the temporary gate and
restores the prior perimeter snapshot. No runtime or deploy rollback is needed
because VP-0 changes neither. Later phases should update the generated
inventory by burn-down; they must not weaken a rule or rewrite the baseline to
hide growth.

This receipt does not claim a pnpm install, a Vite Plus build, a Node 24
production runtime, payment decommission, a clean full suite, a release, or an
owner-accepted final cutover. Those claims belong to their ordered phase
issues.
