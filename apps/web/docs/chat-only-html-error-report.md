# OpenClaw Tool Flow: "Only HTML requests are supported here" – Full Report

**Date:** 2026-02-04
**Status:** Fix implemented (explicit API base required); verification pending
**Constraint:** Do not use `/api/chat`; chat endpoint must remain at **POST `/chat`**.

---

## Executive summary

The assistant correctly uses multiple OpenClaw tools in sequence (`openclaw_get_instance`, `openclaw_provision`, etc.), but every tool execution returns `{"error": "Only HTML requests are supported here"}`. The cause is not the chat endpoint (POST `/chat` works) but the fact that **tool code runs on the server and calls same-origin `/api/openclaw/*`**. Those paths have no server route in the TanStack app, so requests fall through to the document (HTML) handler, which rejects non-HTML `Accept` headers. **Fix:** Point server-side OpenClaw calls at the real API via `PUBLIC_API_URL`, or add TanStack server routes that proxy `/api/openclaw/*` to the real API.

**Update:** `resolveApiBase` now requires an explicit API base (`OPENCLAW_API_BASE`, `OPENAGENTS_API_URL`, or `PUBLIC_API_URL`) and no longer falls back to `${origin}/api`, so misconfiguration fails fast instead of hitting the HTML handler. Set the env var in the web worker and local dev before verifying the flow.

---

## 1. What’s going on

### Observed behavior

- User sends a message (e.g. “Help me set up OpenClaw…”).
- **POST `/chat`** is handled correctly by the TanStack Start server route in `src/routes/chat.ts` (streaming, tools, multi-step).
- The model calls tools in sequence (e.g. `openclaw_get_instance`, then `openclaw_provision`, then `openclaw_get_instance` again).
- Each tool’s **execute** function runs on the server and performs an HTTP request.
- Those requests return:
  **`{"error": "Only HTML requests are supported here"}`**

So: the **chat** request is fine; the **outbound requests made inside tool execution** are what hit the “Only HTML” response.

### Where the error comes from

- TanStack Start’s request pipeline in `@tanstack/start-server-core`:
  - For a request that is **not** handled by a server route handler, it falls through to **executeRouter** (document/HTML render path).
  - That path only allows `Accept: */*` or `Accept: text/html`.
    See `createStartHandler.ts`:

  ```ts
  const supportedMimeTypes = ['*/*', 'text/html'];
  const isSupported = supportedMimeTypes.some((mimeType) =>
    acceptParts.some((part) => part.trim().startsWith(mimeType)),
  );
  if (!isSupported) {
    return Response.json(
      { error: 'Only HTML requests are supported here' },
      { status: 500 },
    );
  }
  ```

- So any request that:
  - Reaches the document handler (no matching server route), and
  - Has an `Accept` header that is neither `*/*` nor `text/html`

  gets that JSON error.

### Why tool executions hit it

- In `src/routes/chat.ts`, tool config uses `apiConfig` built from:

  ```ts
  const origin = new URL(request.url).origin;  // e.g. http://localhost:3000
  const apiBase = resolveApiBase(origin);
  ```

- In `src/lib/openclawApi.ts`, **resolveApiBase**:

  - If `process.env.PUBLIC_API_URL` is set → uses that.
  - Else if `origin` is passed → returns **`${origin}/api`** (e.g. `http://localhost:3000/api`).

- So when `PUBLIC_API_URL` is **not** set (e.g. in dev), **apiBase** is **same-origin `/api`**:
  `http://localhost:3000/api`.

- Tool execute functions (e.g. `getOpenclawInstance`, `createOpenclawInstance`) call:

  ```ts
  openclawRequest(config, '/openclaw/instance', ...)
  → fetch(`${config.apiBase}/openclaw/instance`, { headers: { accept: 'application/json' }, ... })
  → fetch('http://localhost:3000/api/openclaw/instance', { accept: 'application/json' })
  ```

- That request goes to the **same** TanStack Start app. There is **no** server route for **`/api/openclaw/instance`** (or other `/api/openclaw/*` paths) in this app, so:

  - The request is **not** handled by a route handler.
  - It falls through to **executeRouter** (document path).
  - Document path sees `Accept: application/json` → not in `['*/*', 'text/html']` → returns **“Only HTML requests are supported here”**.

So the failure is **not** the chat endpoint; it’s that **server-side tool code is calling same-origin `/api/openclaw/*`**, and those URLs are not defined as server routes, so they hit the HTML-only document handler.

---

## 2. What was tried

### 2.1 Chat endpoint path

- **Tried:** Add a second chat handler at **POST `/api/chat`** (TanStack Start quickstart style) and point the frontend at `/api/chat`.
- **Result:** Would likely fix the *chat* request hitting the document handler if that were the issue; in this app the chat request at **POST `/chat`** is already handled by the server route.
- **Reverted:** Per requirement: **do not use `/api/chat`**; chat must stay on **POST `/chat`**.

### 2.2 Frontend and route cleanup

- **Done:** Frontend uses **`api: '/chat'`** only. Removed `/api/chat` route and api layout; route tree has no `/api` chat route.

### 2.3 Tool names and schema (for completeness)

- Tool names: normalized to `^[a-zA-Z0-9_-]+$` (e.g. `openclaw_get_instance`).
- Tool parameters: use explicit **`inputSchema`** via `jsonSchema({ type: 'object', properties: {} })` so OpenAI gets `type: "object"` and not `"None"`.
- Multi-step: **`stopWhen: stepCountIs(10)`** so the model can chain multiple tool calls.

These are correct and unrelated to the “Only HTML” response.

---

## 3. Root cause (summary)

- **Chat:** POST `/chat` is correctly handled by the server route; no change needed there.
- **Tools:** Execute on the server and call **`openclawRequest(config, path)`**.
- **apiBase:** When `PUBLIC_API_URL` is unset, `resolveApiBase(origin)` is **`${origin}/api`** → same host, path prefix `/api`.
- **Effect:** Tool code does **same-origin** `fetch('.../api/openclaw/instance', { accept: 'application/json' })`. Those requests hit the same TanStack app; no server route exists for `/api/openclaw/*`, so they hit the document handler and get “Only HTML requests are supported here”.

---

## 4. What to try next

### Option A: Point server-side OpenClaw calls at the real API (recommended)

- **Idea:** Ensure tool executions **never** call same-origin `/api`; they should call the actual OpenClaw/Rust API.
- **How:**
  - Set **`PUBLIC_API_URL`** (or whatever `resolveApiBase` reads in your env) to the **real** OpenClaw API base URL (e.g. Rust worker or gateway), e.g.
    `https://api.openagents.com/api` or your deployed worker URL.
  - In **local** dev, either:
    - Run the Rust/OpenClaw API locally and set `PUBLIC_API_URL=http://localhost:<port>/api` (or similar), or
    - Use a shared dev API URL.
- **Result:** Tool execute functions will `fetch(realApiUrl + '/openclaw/instance', ...)`. Those requests go to the OpenClaw API, not the TanStack app, so they never hit the document handler.
- **Files:** No code change required in `openclawApi.ts` or chat route; only env (e.g. `.env.local`, Convex env, or deployment env) for `PUBLIC_API_URL`.

### Option B: Add TanStack server routes for `/api/openclaw/*`

- **Idea:** Keep using same-origin `/api` in dev, but define **server route handlers** for the paths the tools call, so those requests are handled before the document router.
- **How:**
  - Add file routes (or a small router) that register **server-only** handlers for paths such as:
    - `/api/openclaw/instance` (GET/POST)
    - `/api/openclaw/runtime/status`
    - `/api/openclaw/runtime/devices`
    - etc.
  - Each handler would forward the request to the real OpenClaw API (or Convex) and return the response.
- **Result:** Same-origin `fetch('.../api/openclaw/instance')` would match a server route and return JSON, never reaching the HTML-only document path.
- **Caveat:** You must implement and maintain these proxy routes and keep them in sync with what `openclawApi.ts` calls.

### Option C: Relax Accept check in TanStack Start (not recommended)

- **Idea:** Change the document handler so it does not return “Only HTML” for `Accept: application/json`.
- **How:** Fork or patch `@tanstack/start-server-core` (e.g. in `createStartHandler.ts`) to allow more `Accept` values or to treat API-like paths differently.
- **Downside:** Framework change; easy to break SSR or security expectations; not addressing the real issue (wrong target for tool calls).

### Option D: Different apiBase for server-originated calls

- **Idea:** When building `apiConfig` **on the server** (in the POST `/chat` handler), never use same-origin `/api`; always use a configured base URL.
- **How:** In `resolveApiBase(origin)` (or a server-only variant), when `origin` is the same as the current server, require an env var (e.g. `OPENCLAW_API_BASE` or `PUBLIC_API_URL`) and do not fall back to `${origin}/api`.
- **Result:** In dev, if the env var is unset, tool calls would fail fast with a clear “configure OPENCLAW_API_BASE” error instead of hitting the document handler.
- **Combined with Option A:** Enforce “server-side tool calls always use explicit API URL” and document that env var.

---

## 5. Recommended next step

- **First:** Implement **Option A**:
  - Set **`PUBLIC_API_URL`** (or the env var your `resolveApiBase` uses) to the real OpenClaw API base in every environment (local dev, staging, production).
  - For local dev, run the OpenClaw/Rust API locally or point to a shared dev API; do not rely on same-origin `/api` for tool execution.
- **If** you want to keep same-origin `/api` in dev without running the full API: add **Option B** (server routes for `/api/openclaw/*` that proxy to the real API or Convex), and ensure `apiBase` in that dev setup still uses the origin that hits these proxy routes (e.g. `http://localhost:3000/api`).

---

## 6. References

- TanStack Start handler: `node_modules/@tanstack/start-server-core/src/createStartHandler.ts` (executeRouter, “Only HTML” response).
- Chat route: `apps/web/src/routes/chat.ts` (POST handler, apiConfig, streamText, tools).
- OpenClaw API client: `apps/web/src/lib/openclawApi.ts` (`resolveApiBase`, `openclawRequest`, tool helpers).
- Constraint: Do not use `/api/chat`; chat stays at **POST `/chat`**.
