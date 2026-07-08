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
- [ ] Step 2 — custody
- [ ] Step 3 — presence
- [ ] Step 4 — wallet (Spark)
- [ ] Step 5 — executor
- [ ] Step 6 — typed RPC contract
