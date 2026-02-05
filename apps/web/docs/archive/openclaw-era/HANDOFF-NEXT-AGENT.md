# Handoff: Next Coding Agent

**For:** The next agent working on the OpenAgents web app (`apps/web`) and OpenClaw integration.
**Date:** 2026-02-04

**Update:** The OpenClaw tool-call fix has been implemented in code (explicit API base required; same-origin fallback removed). Ensure `PUBLIC_API_URL` (or `OPENCLAW_API_BASE`) is set and verify the `/chat` tool flow end-to-end.

---

## 1. Read this first

**Read the full report:**

- **`apps/web/docs/chat-only-html-error-report.md`**

That file explains what’s going on, what was tried, root cause, and what to try next. Do not skip it.

---

## 2. Investigate and fix the OpenClaw “Only HTML” issue

### Context

- The assistant at **POST `/chat`** works and correctly runs **multiple** OpenClaw tools in sequence (`openclaw_get_instance`, `openclaw_provision`, etc.).
- Every tool execution returns: `{"error": "Only HTML requests are supported here"}`.
- **Cause (from the report):** Tool execute functions run on the server and call same-origin `/api/openclaw/*`. Those paths have **no** TanStack server route, so the request hits the document (HTML) handler, which rejects `Accept: application/json`.

### Your tasks

1. **Investigate** using the report:
   - Confirm where `apiBase` is set (e.g. `apps/web/src/lib/openclawApi.ts` → `resolveApiBase`).
   - Confirm which paths tools call (e.g. `/openclaw/instance`, `/openclaw/runtime/status`, etc.).
   - Confirm that `PUBLIC_API_URL` (or equivalent) is unset in your dev setup, so `apiBase` becomes `${origin}/api`.

2. **Fix** using one of the options in the report (Section 4):
   - **Option A (recommended):** Set `PUBLIC_API_URL` to the real OpenClaw API base (Rust worker / gateway) in every environment so server-side tool calls never hit the TanStack app.
   - **Option B:** Add TanStack server routes for `/api/openclaw/*` that proxy to the real API (or Convex).
   - **Option D:** When building `apiConfig` on the server, require an env var (e.g. `OPENCLAW_API_BASE`) and do **not** fall back to `${origin}/api`, so misconfiguration fails fast with a clear error.

3. **Verify:** After your fix, run the assistant flow (“Help me set up OpenClaw…”) and confirm tool calls return real JSON (instance status, provision result, etc.) instead of “Only HTML requests are supported here”.

**Constraint:** Do **not** use `/api/chat`. Chat must remain at **POST `/chat`** (see report).

**Deploy and test yourself:** You are allowed (and expected) to run deploys. Use `npm run deploy` in `apps/web` for the main website; check `package.json` in `apps/api`, `apps/agent-worker`, `apps/indexer`, `apps/spark-api`, `apps/openclaw-runtime` for equivalent deploy scripts. Fix, deploy, then re-test. Do not wait for a human unless they ask you to slow down.

---

## 3. Continue with the roadmap where we left off

After the OpenClaw tool-flow fix is done, continue with:

### Repo-level roadmap

- **`ROADMAP.md`** (repo root) — MVP “Add Next” priorities, CODING_AGENT_LOOP, Verified Patch Bundle, tool signatures, etc.
- **`AGENTS.md`** — Authority rules, required reading, build/test commands.

### Web / OpenClaw roadmap (where we left off)

- **`apps/web/docs/openclaw-on-openagents-com.md`** — Canonical plan for OpenClaw on openagents.com (Cloudflare-first). Current gap: no first-class OpenClaw WebChat backed by the Gateway session model; we have a website chat that *manages* OpenClaw (provision, status) and tool executions were failing with the HTML error.
- **`apps/web/docs/openclaw-hatchery-architecture.md`** — Architecture, env vars, flows, and debugging for Hatchery ↔ OpenClaw.
- **`apps/web/docs/cloudflare-agents-sdk-openagents-com.md`** — Supporting context for Cloudflare + Agents SDK.

Next logical steps after the fix:

- Ensure assistant-driven provision + device pairing flow works end-to-end (user says “set up OpenClaw”, tools succeed, user can pair device).
- Then proceed with openclaw-on-openagents-com.md milestones (e.g. OpenClaw WebChat backed by Gateway sessions, streaming, device pairing UX).

---

## 4. Key files

| Purpose | Path |
|--------|------|
| Full “Only HTML” report | `apps/web/docs/chat-only-html-error-report.md` |
| Chat route (POST `/chat`, tools, apiConfig) | `apps/web/src/routes/chat.ts` |
| OpenClaw API client (resolveApiBase, openclawRequest) | `apps/web/src/lib/openclawApi.ts` |
| OpenClaw on openagents.com plan | `apps/web/docs/openclaw-on-openagents-com.md` |
| Repo roadmap | `ROADMAP.md` |
| Agent contract / authority | `AGENTS.md` |

---

## 5. Summary for the next agent

1. **Read** `apps/web/docs/chat-only-html-error-report.md`.
2. **Investigate** why server-side OpenClaw tool calls hit the HTML-only handler (same-origin `/api/openclaw/*`, no route).
3. **Fix** by pointing tool calls at the real API (Option A) or adding proxy routes (Option B) or failing fast when env is unset (Option D).
4. **Verify** the assistant flow so tool results are real JSON.
5. **Continue** with `ROADMAP.md` and `apps/web/docs/openclaw-on-openagents-com.md` (e.g. end-to-end provision + pairing, then WebChat/Gateway milestones).

## 6. Agents SDK alignment (new)

We reviewed `~/code/agents` (Cloudflare Agents SDK). The agent worker is still a **custom DO**, not `Agent`/`AIChatAgent`. That means:

- no built-in state sync or resumable streaming
- no `agents/react` client hooks
- approvals are stored manually (no SDK workflow approvals yet)

See `apps/web/docs/cloudflare-agents-sdk-openagents-com.md` for a full gap/opportunity list.
