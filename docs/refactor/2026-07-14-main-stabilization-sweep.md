# Main Stabilization Sweep — 2026-07-14

- Class: repo-health record
- Date: 2026-07-14
- Scope: burn down the pre-existing verification reds recorded by the MVP
  prune ledger and the additional drift exposed by the Effect beta.94 and
  superseded-desktop merges.
- Integration base: `393510cee5` (`refactor(desktop): remove superseded Khala
  Code client`), followed by the concurrent UX-5 landing before final push.

## Red -> green ledger

| Gate | Root cause | Resolution |
| --- | --- | --- |
| `test:bun-api-perimeter` | FEED-1 added a loopback `Bun.serve` boundary after the freeze; the retained QA PR-comment runner also used `Bun.spawn`. | Registered the Pylon loopback server as one reviewed named perimeter. Ported the QA runner process adapter to `node:child_process` behind its existing Effect/injected-runner boundary; no broad allowlist was added. |
| `typecheck:api` | Updated Workers types require `ExecutionContext.tracing`; the Worker graph also reached a Bun-typed Pylon module for portable artifact types and its SQLite seam lacked the `readonly` option. | Added one shared inert tracing implementation for synthetic contexts, mirrored the SQLite option in the ambient/test adapters, and moved portable bundle/artifact contracts into the runtime-neutral `@openagentsinc/portable-session-contract` package. Pylon re-exports the moved types for compatibility. |
| `check:architecture` | July route lanes grew zero-budget generic throws/raw runtime calls and Response-surface counts without maintaining the ratchets. | Replaced the debt with typed errors, typed JSON/time/process boundaries, and composition-root Promise adapters; then recorded the audited Response counts with dated attribution. Omni handoff operator auth now follows the same Effect dependency boundary. |
| Pylon test typecheck | Diagnostic messages embedded checkout-specific absolute paths, making the committed baseline non-portable. | Normalized diagnostic import paths, requested untruncated diagnostics, bumped the baseline schema, and regenerated the reviewed baseline. |
| Effect topology check | The guard still modeled three Effect lines after the repository unified on `effect@4.0.0-beta.94`. | Replaced the retired beta.70/Effect-3 isolation policy with a single-line installed-resolution check, retained explicit upstream peer exceptions, and pinned the reviewed `nostr-effect` Git dependency. |
| Pylon-core Claude controller tests | The controller context gained a required `requestId`. | Supplied stable bounded request refs in the three affected authority/cancellation tests. |
| Post-removal harness resolution | Removing the superseded desktop changed the workspace install graph; a pre-removal `node_modules` no longer materialized the harness's already-declared workspace dependencies. | Performed a fresh lock-consistent install. No dependency or lockfile widening was required. |

## Verification

Final verification is run from the rebased worktree with a fresh
`bun install`; the commit/push handoff records the exact gate results. Focused
receipts during the sweep included:

- portable-session contract, Pylon artifact/ledger, and managed provisioner:
  20 pass / 0 fail;
- Pylon-core runtime-interaction bridge: 6 pass / 0 fail;
- QA PR-comment composition: 16 pass / 0 fail;
- API and Khala QA harness typechecks: exit 0;
- Bun API perimeter scan: 2 pass / 0 fail, zero unreviewed findings;
- Effect topology check: pass on the unified beta.94 install.

The full root typecheck, architecture check, and deploy gate are the final
integration receipts; they are rerun after the last upstream rebase rather
than inferred from any pre-rebase result.
