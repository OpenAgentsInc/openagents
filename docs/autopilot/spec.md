# Autopilot (Simplified Spec)

Autopilot is a **single, persistent chat agent** that runs on **Cloudflare Workers + Durable Objects** (Cloudflare Agents SDK).

Hard constraint: **no containers**. No sandboxes. No local executors. No tools. One Autopilot per user.

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
3. After auth, the app redirects to `/assistant`, which:
   - finds or creates the user's **one** Autopilot thread (`kind: "autopilot"`)
   - redirects to `/chat/:threadId`
4. `/chat/:threadId` opens the chat UI and connects to the Cloudflare Agent websocket:
   - `WS /agents/chat/:threadId`
   - transcript rehydration: `GET /agents/chat/:threadId/get-messages` (AIChatAgent built-in)

There is no `/autopilot` surface and no "Spawn" UI.

---

## Data Model (One Autopilot Per User)

- Each user has exactly **one** Autopilot thread.
- The thread id (`threadId`) is used as the **agent/DO name** so `/chat/:threadId` deterministically maps to one Durable Object.
- Convex can store lightweight UI metadata (thread title/id), but the canonical transcript lives in the Durable Object.

---

## Runtime (No Containers)

- Web app serves UI routes:
  - `GET /` (homepage)
  - `GET /assistant` (redirect helper)
  - `GET /chat/:threadId` (chat UI)
- Agent runtime is a Cloudflare Worker with a single Durable Object:
  - `Chat extends AIChatAgent` (Agents SDK + `@cloudflare/ai-chat`)
  - routed under `/agents/*` (websocket + REST endpoints)

Explicitly out of scope:

- Cloudflare Containers / Docker images
- multiple Autopilots per user
- tools, approvals, extensions
- dashboards, billing, marketplace, community surfaces

---

## Success Metrics

- TTFT (p50/p95)
- message success rate (no silent failures)
- next-day return rate

---

## Repo Map (Current)

- Homepage UI: `apps/web-old/src/routes/_app/index.tsx`
- Homepage component: `apps/web-old/src/components/hatchery/AutopilotPage.tsx`
- Chat redirect: `apps/web-old/src/routes/_app/assistant.tsx`
- Chat page: `apps/web-old/src/routes/_app/chat.$chatId.tsx`
- Autopilot worker (Agents SDK): `apps/liteclaw-worker/src/server.ts` (name is historical)
