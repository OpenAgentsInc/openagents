# @openagentsinc/pylon-core

Typed Effect engine services for **Pylon**, extracted out of `apps/pylon/src`.

This package is the headless engine behind the `pylon` / `khala` CLIs, the
standing engine daemon (today's `pylon node`), and the Khala Code desktop
cockpit. It exists so those surfaces share one typed service layer instead of
talking over a stringly stdout-subprocess seam.

See `docs/fable/2026-07-08-pylon-into-khala-code-proposal.md` (§3/§5) and
GitHub issue #8578 (PY-1) for the full plan.

## Service boundaries

| Area | Scope |
|---|---|
| `custody` (P1) | Per-account Codex/Claude homes, registry, quota, usage, status, health ledgers. Never touches `~/.codex`. |
| `executor` (P2) | Local coding-delegation runs: assignment, khala dispatch/requester/spawn, codex/claude executors, workspace materializer, closeouts. |
| `presence` (P3) | Go-online, heartbeat, counted capacity refs — what makes a machine a dispatch target. |
| `wallet` (P5) | The Spark Lightning rail. **A live, preserved payment rail** behind its own service boundary; never inside a GUI process. |

## Extraction status

Landing incrementally (bottom-up: leaf dependencies first). Original
`apps/pylon/src` modules become thin re-export shims as code moves here, so
existing consumers keep compiling.

- [x] Step 1 — package scaffold
- [x] Shared foundation (`shared/`) — version, wsl-host-detect, bootstrap,
  nostr-identity, inventory, state (linchpin the higher layers depend on)
- [~] Step 2 — custody: **wave 1** (account-registry, account-quota,
  codex-account-health, codex-custody-reprime) + **wave 2** (account-quota-
  ledger, codex-account-health-ledger) done. **Still in `apps/pylon`:**
  account-usage (→ claude/codex-agent = executor), account-status (→
  account-usage), account-connect (→ presence), codex-account-auth-health
  (→ account-connect).
- [ ] Step 3 — presence (blocked: `presence.ts` sits near the TOP of the
  graph — it transitively pulls in wallet, claude/codex-agent, active-
  assignment-runs, and the postponed P6 earning code; needs those resolved
  or stubbed first)
- [ ] Step 4 — wallet (Spark): **attempted & deferred.** Clean static set
  EXCEPT `spark-backup-helper.ts` statically imports `spark-wasm-runtime.ts`,
  which resolves the Breez WASM via `./generated/spark-wasm-b64.js` produced
  by `apps/pylon/scripts/embed-spark-wasm.ts` for the Bun-compiled binary
  (#5166/#5404). Moving it needs the generator relocated + an **RC-binary
  build verification** (can't be done source-only). Do this in a session that
  can build+run the packaged binary.
- [ ] Step 5 — executor (assignment, khala-dispatch/requester/spawn,
  codex/claude-agent-executor, workspace-materializer)
- [x] Step 6 — typed RPC contract (`rpc/`) — **unconsumed seed**; PY-2 (#8579)
  wires it and deletes the desktop stdout seam.
