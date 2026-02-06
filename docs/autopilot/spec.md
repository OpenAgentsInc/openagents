# Autopilot (Simplified Spec)

Autopilot is a **single, persistent chat agent** that runs on **Cloudflare Workers + Durable Objects** (Cloudflare Agents SDK).

Hard constraint: **no containers**. No sandboxes. No local executors. One Autopilot per user.

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

If the visitor is already authenticated, `/` immediately redirects them into chat.

---

## Single Flow

1. Visitor opens `/`.
2. Visitor clicks **Start for free** (sign up) or **Log in**.
3. After auth, the app redirects to `/autopilot` (single-thread chat UI).
   - `/assistant` exists as a legacy redirect helper but should not be user-visible.
4. `/autopilot` connects to the Cloudflare Agent websocket:
   - `WS /agents/chat/:threadId` (where `threadId` is the WorkOS `user.id`, used as the Durable Object name)
   - transcript rehydration: `GET /agents/chat/:threadId/get-messages` (AIChatAgent built-in)

There is no "Spawn" UI and no multi-thread UX in the MVP.

---

## Data Model (One Autopilot Per User)

- Each user has exactly **one** Autopilot thread.
- The thread id (`threadId`) is used as the **agent/DO name** so `/autopilot` deterministically maps to one Durable Object (without exposing the id in the browser URL).
- Convex can store lightweight UI metadata (thread title/id), but the canonical transcript lives in the Durable Object.

---

## Runtime (No Containers)

- Web app serves UI routes:
  - `GET /` (homepage)
  - `GET /assistant` (legacy redirect helper)
  - `GET /autopilot` (chat UI)
  - `GET /chat/:threadId` (legacy redirect, should forward to `/autopilot`)
- Agent runtime is a Cloudflare Worker with a single Durable Object:
  - `Chat extends AIChatAgent` (Agents SDK + `@cloudflare/ai-chat`)
  - routed under `/agents/*` (websocket + REST endpoints)

Explicitly out of scope:

- Cloudflare Containers / Docker images
- multiple Autopilots per user
- MCP, approvals, extensions
- dashboards, billing, marketplace, community surfaces

---

## Success Metrics

- TTFT (p50/p95)
- message success rate (no silent failures)
- next-day return rate

---

## Repo Map (Current)

- Homepage UI: `apps/web/src/routes/index.tsx`
- Chat redirect: `apps/web/src/routes/assistant.tsx`
- Chat page: `apps/web/src/routes/autopilot.tsx`
- Legacy redirect: `apps/web/src/routes/chat.$chatId.tsx` (redirects to `/autopilot`)
- Dev proxy for worker endpoints: `apps/web/vite.config.ts` (`/agents/*` → `127.0.0.1:8787`)
- Autopilot worker (Agents SDK): `apps/autopilot-worker/src/server.ts`

---

## Notes

- Effect migration starting points for the current `apps/web` app: `docs/autopilot/effect-migration-web.md`
- Effect-centric telemetry/logging service spec: `docs/autopilot/effect-telemetry-service.md`
- Effect + Convex patterns from `~/code/crest` to adopt: `docs/autopilot/effect-patterns-from-crest.md`

## Status (2026-02-06)

- Web flow: `/` → `/autopilot` implemented in `apps/web/src/routes/*` (with `/assistant` and `/chat/:threadId` as legacy redirects).
- One Autopilot per user: `threadId` is the WorkOS `user.id` (Durable Object name), but it is not exposed in the browser URL.
- Worker rename: `apps/liteclaw-worker` → `apps/autopilot-worker`.
- Routes run via Effect runtime: loaders execute Effect programs using `context.effectRuntime` (Telemetry events on load/redirect).
- Tools (MVP): `get_time`, `echo` (to validate the tool loop + unblock basic capability testing).
