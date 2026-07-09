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
- [x] Step 2 — custody: **wave 1** (account-registry, account-quota,
  codex-account-health, codex-custody-reprime) + **wave 2** (account-quota-
  ledger, codex-account-health-ledger) + **wave 3** (account-usage,
  account-status) + **wave 4** (account-connect, codex-account-auth-health —
  unblocked once presence landed) done. `defaultCodexAuthValidityProbe`
  (account-connect's real Codex-CLI probe implementation, coupled to
  `codex-composer.ts`) stays app-side by design; both moved modules take it
  as an injected `probe` and the `apps/pylon` shims default-wire the real one
  so production behavior is unchanged — see those files' header comments.
- [x] Step 3 — presence (`presence/`) — **landed.** The apple-fm blocker
  (presence directly called `collectPylonAppleFmStatus`, whose
  `PylonAppleFmStatusProjection` type structurally embeds
  `@openagentsinc/pylon-runtime`'s `ProbeBackendCapabilityReport` — a nested
  workspace package never resolvable by name from a sibling package) was
  resolved via **option B, dependency injection** (the lower-risk path):
  `presence/apple-fm-status.ts` defines a structural mirror of
  `PylonAppleFmStatusProjection` (every `ProbeBackendCapabilityReport["field"]`
  reference inlined as its literal type) plus the two pure capacity-ref
  helpers; presence's existing (previously test-only) `appleFmStatusProbe`
  injection seam is now load-bearing, defaulting to
  `NOT_PROBED_APPLE_FM_STATUS` (no contribution, no blocker) when omitted.
  `apps/pylon/src/presence.ts` is now a wrapper (not a pure re-export): its
  `sendHeartbeat` always injects the real `collectPylonAppleFmStatus` probe
  by default when a caller doesn't supply one — every current production
  call site — so production heartbeat behavior is unchanged. The other two
  couplings resolved the same session: `presence/nip90-lane-refs.ts` (the 4
  lane-ref/relay/capability symbols presence needs, depending only on
  `@openagentsinc/nip90`) and a standalone `HeartbeatWalletProbe` structural
  type (4 booleans) replacing `Pick<WalletStatusProjection, ...>` — wallet.ts
  itself is untouched. `active-assignment-runs` (a clean leaf of presence's
  closure) was already extracted into `executor/` in a prior session.
- [ ] Step 4 — wallet (Spark): **attempted & deferred.** Clean static set
  EXCEPT `spark-backup-helper.ts` statically imports `spark-wasm-runtime.ts`,
  which resolves the Breez WASM via `./generated/spark-wasm-b64.js` produced
  by `apps/pylon/scripts/embed-spark-wasm.ts` for the Bun-compiled binary
  (#5166/#5404). Moving it needs the generator relocated + an **RC-binary
  build verification** (can't be done source-only). Do this in a session that
  can build+run the packaged binary.
- [~] Step 5 — executor (`executor/`): **leaf wave + Claude executor done** — the
  dependency-closed leaves `claude-agent`, `codex-agent`, `claude-turn-reporter`
  and (built on the first two) `workspace-materializer` are relocated with
  shims. That unblocked the custody `account-usage → account-status` wave
  above. Also relocated `active-assignment-runs` (+ its test) here: a clean
  leaf of presence's closure (only local dep was `state`) — but it is active
  coding-**run** persistence, execution-domain state that presence merely reads
  for capacity refs and that `assignment.ts` also consumes, so it homes in
  `executor/`, not a `presence/` folder built around a leaf while presence.ts
  itself can't yet move. Added `@openagentsinc/effect-boundary` as a pylon-core
  dep for it. `claude-agent-executor` is now relocated too (after the MH-2
  collision cleared), with the app path reduced to a thin re-export shim.
  **Still in `apps/pylon`:**
  - `codex-agent-executor`, `assignment`, `khala-spawn` — now unblocked
    dependency-wise on `presence`/`account-connect` (both landed), but not
    yet traced/attempted this session; next session should re-verify their
    full closures (they're large, top-of-graph files) before moving.
  - `khala-requester`, `khala-dispatch` — extractable dependency-wise, but
    their leaf closure includes `tips` (Spark-tipping / payment-adjacent,
    semantically an earning/wallet-boundary module, NOT executor) and
    `work-requester`. Deferred pending a boundary decision on where `tips`
    lives; do not shove `tips` into `executor/` just to satisfy the move.
- [x] Step 6 — typed RPC contract (`rpc/`) — **unconsumed seed**; PY-2 (#8579)
  wires it and deletes the desktop stdout seam.
- [ ] MCP consolidation — **design decision, not a mechanical move; plan
  recorded in issue #8578.** The two MCP surfaces (`apps/pylon/src/khala-mcp.ts`
  `khala.*` request-delegation tools on the homegrown `tas/mcp-server.ts`, vs
  `clients/khala-code-desktop/src/bun/khala-fleet-*` `fleet_run_*`/`codex_*`
  orchestration tools on the shared `@openagentsinc/khala-tools` +
  `@openagentsinc/mcp-contract` framework) are complementary tool sets on two
  different frameworks — duplication is at the plumbing layer. Plan: the
  `khala-tools`/`mcp-contract` framework wins, `tas/mcp-server.ts` retires,
  `khala.*` tools re-register as `RegisteredKhalaTool`s on one registry, and the
  engines (khala-requester, orchestration store/work-planner/supervisor) move
  into pylon-core so the desktop/autopilot clients stop reaching into
  `apps/pylon/src/**` by `../../../` relative paths. Gated on the executor
  engine landing in pylon-core and its own security-adversarial-harness
  verification pass.
