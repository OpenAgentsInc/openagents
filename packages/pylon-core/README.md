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
  ledger, codex-account-health-ledger) + **wave 3** (account-usage,
  account-status — unblocked once the executor agent leaves landed in step 5)
  done. **Still in `apps/pylon`:** account-connect (→ presence),
  codex-account-auth-health (→ account-connect).
- [ ] Step 3 — presence (blocked). Fresh trace of `presence.ts`'s closure
  narrowed the four out-of-package deps to their real status:
  - `active-assignment-runs` — **clean leaf, extracted** (see Step 5 below).
  - `provider-nip90` (P6 earning) — **NOT a blocker.** Presence uses only 4
    lightweight symbols (`PYLON_NIP90_PROVIDER_CAPABILITY_REF`,
    `OPENAGENTS_MARKET_RELAY_URL`, `providerNip90LaneRefs`, `relaysFromEnv`)
    that depend only on `KIND_JOB_*` from `@openagentsinc/nip90` — zero
    coupling to the retired `labor-market`/`labor`/`wallet` earning rail.
    Clean split: extract those into a small `presence/nip90-lane-refs.ts`; the
    heavy `provider-nip90.ts` earning body stays in the app.
  - `wallet` — **NOT a blocker.** Type-only (`Pick<WalletStatusProjection,
    "configured"|"daemonOnline"|"receiveReady"|"sendReady">`, all boolean);
    replace with a standalone structural interface — a full
    `WalletStatusProjection` stays assignable; wallet.ts is never touched.
  - `node/apple-fm-status` — **THE blocker.** presence calls
    `collectPylonAppleFmStatus` and its `PylonAppleFmStatusProjection` type
    structurally embeds `@openagentsinc/pylon-runtime`'s
    `ProbeBackendCapabilityReport`. pylon-runtime (`apps/pylon/packages/runtime`)
    is a NESTED workspace package imported only by relative path and not
    symlinked, so pylon-core can't import it by name without a monorepo
    install/lockfile change (unverifiable headlessly). Unblock next session by
    either making pylon-runtime resolvable by name + verifying runtime
    resolution, or injecting the apple-fm functions into presence.
- [ ] Step 4 — wallet (Spark): **attempted & deferred.** Clean static set
  EXCEPT `spark-backup-helper.ts` statically imports `spark-wasm-runtime.ts`,
  which resolves the Breez WASM via `./generated/spark-wasm-b64.js` produced
  by `apps/pylon/scripts/embed-spark-wasm.ts` for the Bun-compiled binary
  (#5166/#5404). Moving it needs the generator relocated + an **RC-binary
  build verification** (can't be done source-only). Do this in a session that
  can build+run the packaged binary.
- [~] Step 5 — executor (`executor/`): **leaf wave done** — the
  dependency-closed leaves `claude-agent`, `codex-agent`, `claude-turn-reporter`
  and (built on the first two) `workspace-materializer` are relocated with
  shims. That unblocked the custody `account-usage → account-status` wave
  above. Also relocated `active-assignment-runs` (+ its test) here: a clean
  leaf of presence's closure (only local dep was `state`) — but it is active
  coding-**run** persistence, execution-domain state that presence merely reads
  for capacity refs and that `assignment.ts` also consumes, so it homes in
  `executor/`, not a `presence/` folder built around a leaf while presence.ts
  itself can't yet move. Added `@openagentsinc/effect-boundary` as a pylon-core
  dep for it. **Still in `apps/pylon`:**
  - `claude-agent-executor` — extractable now (all real deps in-package), but
    it is MH-2's actively-contested file (#8583 / Claude worker-executor
    parity); left in place to avoid a whole-file move-conflict with their
    in-flight work. Extract in a window when MH-2 is idle, pushing fast.
  - `codex-agent-executor`, `assignment`, `khala-spawn` — TOP of the graph:
    they import `account-connect` (→ presence) and/or `presence`/`assignment`,
    so they need step 3 (presence) resolved first.
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
