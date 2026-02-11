# Autopilot (Simplified Spec)

Autopilot is a **single, persistent chat agent** hosted on a **single Cloudflare Worker**, with **Convex as the canonical product DB and realtime stream**.

Hard constraint: **no containers**. No sandboxes. No local executors.

MVP constraint: tool surface stays **tiny** (built-in tools only; no MCP; no “app store”).

---

## Homepage (`/`)

The homepage is the product entrypoint.

- Top left: **OpenAgents**
- Headline: **Introducing Autopilot**
- Tagline: **Your personal agent, no Mac Mini required**
- Actions:
  - **Log in**
  - **Start for free**

If the visitor is already authenticated, `/` may redirect them into chat.

---

## Single Flow (MVP)

1. Visitor opens `/`.
2. Visitor clicks **Start for free** (sign up) or **Log in**.
3. Visitor arrives at `/autopilot` (single-thread chat UI).
   - `/autopilot` MUST work for **unauthed** users.
4. `/autopilot` connects to Convex (WebSocket) and subscribes to:
   - the thread’s messages
   - the assistant message’s incremental `messageParts` (chunked deltas)
5. On user message:
   - Convex mutation writes the user message
   - Worker endpoint starts inference and writes assistant `messageParts` into Convex in batches (~250–500ms or N chars)
6. On auth (when it happens), the existing anon thread is migrated to an owned thread (see below).

There is no "Spawn" UI and no multi-thread UX in the MVP.

---

## Data Model (MVP)

- Convex is canonical for:
  - threads
  - messages
  - message parts (chunked streaming deltas)
  - receipts/budgets/tool calls (bounded; large payloads are `BlobRef`s)
- Anon threads exist as real Convex threads.
- **Anon -> owned migration is REQUIRED**:
  - on auth, the anon thread’s transcript MUST remain available to the user
  - preferred: claim/transfer ownership in-place (no copying)

---

## Runtime (No Containers)

- Cloudflare Worker (single host):
  - serves SSR + static assets
  - exposes `/api/*` endpoints for secret-bearing operations (model calls, tool execution)
  - enforces budgets, emits receipts
- Convex:
  - DB + auth + realtime subscriptions (browser connects directly via WS)
  - optional presence/participants for multiplayer

Explicitly out of scope for MVP:

- per-user Cloudflare DO / DO-SQLite execution plane
- multiple Autopilots per user
- MCP, approvals, extensions
- dashboards, billing, marketplace, community surfaces

Post-MVP (optional):

- reintroduce DO/DO-SQLite as an execution-plane optimization, while keeping Convex as the product DB + multiplayer surface.

---

## Success Metrics

- TTFT (p50/p95)
- message success rate (no silent failures)
- next-day return rate

---

## Repo Map (Current / Target)

- Worker host: `apps/web/src/effuse-host/worker.ts`
- Autopilot controller: `apps/web/src/effuse-app/controllers/autopilotController.ts`
- Autopilot page templates: `apps/web/src/effuse-pages/autopilot.ts`
- Master plan (Effuse stack): `packages/effuse/docs/MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md`
- Execution plane decision (Convex-first MVP): `docs/autopilot/reference/anon-chat-execution-plane.md`

---

## Notes

- Effuse architecture and migration plan: `packages/effuse/docs/MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md`
- Autopilot bootstrap state plan: `docs/autopilot/bootstrap-plan.md`
- Effect-centric telemetry/logging service spec: `docs/autopilot/reference/effect-telemetry-service.md`
- Context failures taxonomy (rot/poisoning/confusion): `docs/autopilot/reference/context-failures.md`
- RLM integration plan + trace mining workflow: `docs/autopilot/synergies/rlm-synergies.md`, `docs/autopilot/dse/rlm-trace-mining.md`
