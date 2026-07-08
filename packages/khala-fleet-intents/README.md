# @openagentsinc/khala-fleet-intents

The single typed intent/mutator vocabulary for steering parallel coding agents
(`codex | claude | grok | auto`) across Khala Code mobile, desktop, and Khala
Sync.

One vocabulary, two uses:

- **Effect Native UI intents** — a phone card or a desktop button dispatches a
  typed `KhalaFleetIntent`.
- **Khala Sync mutators** — the exact same serializable value is the mutator /
  audit spine. Never a second parallel vocabulary.

This package is intentionally **narrow**: it depends only on `effect`, so mobile
and Sync code can consume it without pulling in any desktop code (MH-0,
analysis §11.5.1).

## Contents

- `MarginalCostClass` — `free | subscription | api_metered | not_measured`
  (encodes the free-Grok window as data so `auto` re-ranks without a code
  change).
- `FleetHarnessKind` (`codex | claude | grok`) and `FleetWorkerKind`
  (adds `auto`).
- `HarnessSessionRef` — per-harness session as **opaque** string data plus
  `{ resume, fork }` capability flags (no harness-specific session shape).
- `FleetAutoPolicy` — the deliberately dumb, fully typed `auto` v1 policy.
- `KhalaFleetIntent` (`khala.fleet_intent.v1`) — the intent/mutator union:
  `fleet_run_control` (pause/resume/drain/stop), `approval_decision`
  (allow/deny), `steer_message`, and `worker_selection`.

See `docs/fable/2026-07-08-multi-harness-parallelization-effect-native-analysis.md`
(§8, §11.5, §12) and MASTER_ROADMAP §MH.
