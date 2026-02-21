# ADR-0029: Khala Is a Sync Layer, Runtime Is Authority; Codex Uses Constrained Agent Mode

## Status

Accepted

## Date

2026-02-19

## Context

OpenAgents needs cross-client reactive sync for web/mobile/desktop while
preserving runtime correctness guarantees for long-running execution, policy,
spend, and replay.

Historical guidance in `docs/local/convo.md` already converged on:

- runtime/Postgres as kernel truth,
- Khala as optional reactive projection layer,
- single-writer projection flow.

Recent Khala self-hosting and CLI/MCP capabilities (reviewed from local clones
under `~/code/khala`) make this practical:

- self-hosted backend + dashboard are supported,
- production posture supports Postgres/MySQL and same-region guidance,
- MCP + agent mode (`KHALA_AGENT_MODE=anonymous`) supports remote coding-agent
  workflows,
- production MCP access is intentionally gated.

What was missing was an explicit OpenAgents architecture decision tying these to
Codex/runtime contracts.

## Decision

OpenAgents will use Khala (self-hosted or cloud) only as a reactive sync/read
layer. Runtime/Postgres remains authoritative for execution and economic
correctness.

Normative rules:

1. Runtime/Postgres is the source of truth for run events, Codex worker
   lifecycle, policy decisions, spend state, and replay artifacts.
2. Khala stores only derived projection/read-model state for subscriptions and
   low-latency UI sync.
3. Runtime is the single writer into Khala projection documents.
4. Laravel remains auth/session authority and mints short-lived Khala auth JWTs.
5. Codex/cloud agent workflows interacting with Khala default to constrained
   mode:
   - use `KHALA_AGENT_MODE=anonymous` for remote dev environments,
   - keep MCP production access disabled unless explicitly enabled for a task,
   - require explicit change-control fields and TTL guard via
     `apps/runtime/deploy/khala/mcp-production-access-gate.sh`,
   - for self-hosted deployments, use explicit env-file deployment selection
     (`KHALA_SELF_HOSTED_URL`, `KHALA_SELF_HOSTED_ADMIN_KEY`).

### Schema / Spec Authority

- `proto/` remains Layer-0 schema authority (ADR-0028).
- Runtime API authority: `apps/runtime/docs/RUNTIME_CONTRACT.md`.
- Codex architecture authority: `docs/codex/unified-runtime-desktop-plan.md`.

## Scope

What this ADR covers:

- Authority boundary between runtime and Khala.
- Projection write ownership model.
- Codex operational posture for Khala CLI/MCP in agent workflows.

What this ADR does NOT cover:

- Detailed Khala table/document schema design.
- Runtime endpoint-level projector implementation details.
- Deployment choice between runtime-owned Khala and any optional external compatibility lane per environment.

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Execution authority | Stable: runtime/Postgres is canonical |
| Khala role | Stable: projection/read-model layer only |
| Projection writer | Stable: runtime single-writer model |
| Auth authority | Stable: Laravel issues Khala client auth tokens |
| Codex cloud-agent default | Stable: constrained mode, no implicit production mutation |

Backward compatibility expectations:

- Khala projection schemas evolve additively where possible.
- Projection rebuild remains possible from runtime durable history.
- Runtime API contracts remain authoritative even if Khala schema changes.

## Consequences

**Positive:**

- Preserves kernel correctness while adding reactive multi-client sync.
- Reduces multi-writer race risks in projection state.
- Gives explicit Codex/MCP safety posture for production data.

**Negative:**

- Introduces another runtime-adjacent subsystem to operate (Khala).
- Requires maintaining projection/rebuild tooling discipline.

**Neutral:**

- Laravel and runtime control-plane boundaries do not change.
- Desktop-first Codex execution model remains intact.

## Alternatives Considered

1. **Khala as kernel source-of-truth** — rejected (would weaken runtime
   correctness boundaries and increase migration risk).
2. **No Khala at all** — rejected for now (misses strong cross-client reactive
   sync capabilities).
3. **Multi-writer projection model (Laravel + runtime)** — rejected (high drift
   and race risk).
4. **Unconstrained Codex MCP access to production by default** — rejected
   (unacceptable mutation risk).

## References

- `docs/local/convo.md`
- `docs/plans/active/khala-self-hosting-runtime-sync-plan.md`
- `apps/runtime/docs/KHALA_SYNC.md`
- `docs/codex/unified-runtime-desktop-plan.md`
- `docs/codex/webapp-sandbox-and-codex-auth-plan.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
- `docs/adr/ADR-0028-layer0-proto-canonical-schema.md`
