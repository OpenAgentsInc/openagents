# Plan: Sandbox + Codex Auth for the Webapp

This document outlines how to integrate **Cloudflare Sandbox SDK** and **Codex (ChatGPT) auth** with the OpenAgents webapp (`apps/web/`). Sandboxes are not yet integrated; Codex auth is documented in [opencode-codex-auth.md](./opencode-codex-auth.md). The plan is split into two parts so sandbox work can proceed first and Codex auth can follow once a sandbox-backed runtime exists.

**Audience:** Engineers wiring the webapp to a sandbox-backed agent and/or Codex OAuth.

**Related:** `docs/liteclaw/README.md`, `docs/liteclaw/spec.md`, `apps/cloudflare-agent-sdk-demo/`, [Sandbox SDK docs](https://developers.cloudflare.com/sandbox/).

---

## Current state

- **Webapp (`apps/web/`):** TanStack Start + Convex + WorkOS Auth. Chat UI uses `@assistant-ui/react`, `@cloudflare/ai-chat`, and `agents` (Agents SDK client). Threads are Convex-backed; the runtime connects via **WebSocket** to the LiteClaw worker (`WS /agents/chat/{id}`).
- **LiteClaw worker (`apps/liteclaw-worker/`):** Cloudflare Workers + Durable Objects + Agents SDK. Handles chat, transcript, tool policy, extensions. **No sandbox today** (no containers, no `@cloudflare/sandbox`).
- **Cloudflare agent demo (`apps/cloudflare-agent-sdk-demo/`):** Effect-based request handling, MCP, tools; useful reference for worker structure. No sandbox.
- **Codex:** OpenCode’s built-in Codex plugin does OAuth (browser PKCE + localhost callback, or device code). See [opencode-codex-auth.md](./opencode-codex-auth.md) for flows and web viability.

---

## Part A: Sandbox integration plan

Goal: run agent workloads (and eventually OpenCode/Codex) inside Cloudflare Sandbox containers, with the webapp still talking to a worker that orchestrates them.

### A.1 Where sandboxes live

**Decision:** Add sandbox usage **inside the existing LiteClaw worker**. The worker gets a Sandbox Durable Object binding; for each thread, it obtains a sandbox, runs commands/processes, and (when needed) runs an OpenCode server in the container. This keeps chat + tools + Codex under a single worker endpoint.

**Deferred:** A dedicated “sandbox worker” is a future split if we need isolation or independent scaling.

### A.2 Worker and wrangler

- Add `@cloudflare/sandbox` (and peer deps) to the app that will host the Sandbox DO (e.g. `apps/liteclaw-worker` or a new `apps/sandbox-agent`).
- In `wrangler.toml` (or equivalent):
  - Define a Sandbox **class** (Durable Object) and bind it (e.g. `Sandbox` or `SandboxOpencode`).
  - Use the **opencode** (or custom) image for Codex/OpenCode; default image for non-Codex use.
  - Follow [Sandbox SDK Getting started](https://developers.cloudflare.com/sandbox/get-started/) and [Wrangler configuration](https://developers.cloudflare.com/sandbox/configuration/wrangler/).
- Ensure the worker has the right **CPU/memory** and that **subrequest** usage is acceptable for sandbox RPC (see [Limits](https://developers.cloudflare.com/sandbox/platform/limits/)).

### A.3 Session and lifecycle

- **Decision:** `scope_id = thread_id` always. The webapp must create a thread **before** “Connect Codex” or any sandbox-backed tool. This guarantees Codex auth, OpenCode state, and tool runs share the same container.
- Use **sessions** ([Session management](https://developers.cloudflare.com/sandbox/concepts/sessions/)) for isolation (working directory, env). Create a session when a thread first needs sandbox (e.g. first tool that requires execution or first “use Codex” action).
- Respect **lifecycle** ([Sandbox lifecycle](https://developers.cloudflare.com/sandbox/concepts/sandboxes/)): cold start, sleep, eviction. Persist only what you store outside the container (e.g. in the DO or Convex); treat container filesystem as ephemeral unless you use [R2 mount](https://developers.cloudflare.com/sandbox/guides/mount-buckets/) or similar.

### A.4 Exposing sandbox to the chat agent

- Today LiteClaw uses **tools** (e.g. `workspace.read`/`write`/`edit`, `http.fetch`). To add sandbox:
  - **Path 1:** Implement tools that the agent can call; the worker receives the tool call, runs the operation via Sandbox SDK (e.g. `sandbox.exec`, file APIs, or `createOpencodeServer`), and returns the result. No change to webapp other than new tool behaviors.
  - **Path 2:** Run OpenCode inside the sandbox and proxy the **OpenCode UI** to the user (like [Sandbox OpenCode example](https://developers.cloudflare.com/sandbox/tutorials/claude-code/)). That gives a full IDE-like experience; the webapp would open it in an iframe or new window (see Part B for auth).
- **Decision (for now):** Use **OpenCode** inside the sandbox for code execution and agent tool runs. Sky tools are deferred until after OpenCode is working end-to-end.
- **OpenCode-first path (SDK):**
  - Use `@cloudflare/sandbox/opencode` and `opencode-ai/sdk` to drive a session inside the container.
  - Typical flow inside the worker:
    - `const sandbox = getSandbox(env.Sandbox, "opencode");`
    - `await sandbox.gitCheckout("https://github.com/cloudflare/agents.git", { targetDir: "/home/user/project" });`
    - `const { client } = await createOpencode<OpencodeClient>(sandbox, { directory: "/home/user/project", config: { provider: { anthropic: { options: { apiKey: env.ANTHROPIC_API_KEY } } } } });`
    - `const session = await client.session.create({ body: { title: "My Session" }, query: { directory: "/home/user/project" } });`
    - `const result = await client.session.prompt({ path: { id: session.data!.id }, query: { directory: "/home/user/project" }, body: { model: { providerID: "anthropic", modelID: "claude-haiku-4-5" }, parts: [{ type: "text", text: "Summarize README.md in 2-3 sentences." }] } });`
  - Map this into LiteClaw tool calls so the agent can request OpenCode actions and receive structured results.
- **Webapp changes for Part A (minimal):**
  - No change to `useAgent` / `useAgentChat` if the worker URL and route stay the same.
  - Optional: feature flag or “sandbox tools” toggle so only certain threads/users get sandbox-backed tools until stable.
  - If you add a **separate** “OpenCode in sandbox” route (e.g. `GET /sandbox/opencode?thread=...`), add a link or tab in the chat UI that opens that URL (or embed via iframe with appropriate CSP).

### A.5 Effect and type discipline

- All sandbox/OpenCode wiring must follow the **Effect** pattern used in `apps/cloudflare-agent-sdk-demo` and `apps/liteclaw-worker`.
- Wrap Sandbox SDK calls in `Effect.tryPromise`, use **tagged errors**, and keep the **Effect boundary in the worker** (no ad-hoc Promise chains).
- Preserve **exactOptionalPropertyTypes** and strict TS; add types for sandbox options (session id, image type, env) rather than loose objects.

### A.6 Testing and rollout

- **Unit:** Mock the Sandbox DO or SDK in tests (e.g. stub `getSandbox`, `exec`, `createOpencodeServer`).
- **E2E:** Use a test worker with a sandbox class and a real (or test) image; run a minimal “exec one command” or “start OpenCode and GET /” flow. See [sandbox-sdk E2E](https://github.com/cloudflare/sandbox-sdk/tree/main/tests/e2e) for patterns.
- Roll out behind a flag; monitor CPU time, subrequests, and error rates from the Sandbox API.

### A.7 Docs and references

- [Sandbox SDK – Getting started](https://developers.cloudflare.com/sandbox/get-started/)
- [Architecture](https://developers.cloudflare.com/sandbox/concepts/architecture/) and [Containers](https://developers.cloudflare.com/sandbox/concepts/containers/)
- [Commands](https://developers.cloudflare.com/sandbox/api/commands/), [Files](https://developers.cloudflare.com/sandbox/api/files/), [Background processes](https://developers.cloudflare.com/sandbox/guides/background-processes/)
- [Run Claude Code on a Sandbox](https://developers.cloudflare.com/sandbox/tutorials/claude-code/) (analogous to “run OpenCode on a Sandbox”)

---

## Part B: Codex auth plan

Goal: let users sign in with **ChatGPT (Codex)** in the webapp when the agent (or an OpenCode instance) runs in a sandbox, so the model can use their ChatGPT Plus/Pro subscription instead of (or in addition to) API keys.

Background: [opencode-codex-auth.md](./opencode-codex-auth.md) explains how OpenCode’s Codex plugin does OAuth (browser vs device code) and why the **browser redirect flow** fails when the UI is served via a Worker (redirect goes to localhost). The **device code flow** works today with OpenCode in a sandbox.

### B.1 Where Codex auth runs

- Codex auth is performed **inside the OpenCode server** running in the **sandbox container**. The OpenCode server exposes:
  - `POST /provider/openai/oauth/authorize` (body: `{ method: number }`)
  - `POST /provider/openai/oauth/callback` (body: `{ method, code? }`)
- So: the **webapp** must send these requests to the **OpenCode server**. That implies the worker (or the webapp’s backend) must **proxy** these calls to the sandbox. With Part A in place, the flow is: **Browser → Worker → Sandbox (container:4096) → OpenCode server**. The worker already has a way to talk to the container (e.g. `sandbox.containerFetch` to port 4096).

### B.2 Webapp UX for “Connect Codex” (device code MVP)

- Add a **settings or provider-connect** surface in the webapp (e.g. in chat settings, or a “Connect ChatGPT (Codex)” entry next to existing provider options). For the **device code** flow (works without OpenCode changes):
  1. User clicks “Connect ChatGPT (Codex)”.
  2. If there is no active thread yet, the webapp **creates one first** and uses that `thread_id` for the sandbox scope.
  3. Frontend calls an **app backend** route that proxies to the sandbox’s OpenCode:  
     `POST /provider/openai/oauth/authorize` with `{ method: 1 }` (headless).
  4. Backend returns `{ url, method: "auto", instructions }` (e.g. `url: "https://auth.openai.com/codex/device"`, `instructions: "Enter code: XXXXX"`).
  5. Webapp shows the **URL** and **code**, and tells the user to open the URL and enter the code.
  6. Frontend then calls a backend route that proxies **blocking** `POST /provider/openai/oauth/callback` with `{ method: 1 }`. The request stays open until the user completes the device flow; OpenCode (in the container) polls until tokens are received, then returns.
  7. On success, show “Codex connected” and optionally refresh provider list.
- **Important:** The backend that proxies to OpenCode must be **session-scoped** and **thread-scoped**. We always use `scope_id = thread_id`.

### B.3 Backend routes (worker → sandbox)

- Add HTTP routes on the worker that the webapp can call, for example:
  - `POST /api/sandbox/:threadId/opencode/provider/openai/oauth/authorize`  
    Body: `{ method: number }`. Worker: get sandbox for `threadId`, ensure OpenCode is running (e.g. `createOpencodeServer`), then `fetch` to `http://container:4096/provider/openai/oauth/authorize` (via `sandbox.containerFetch`). Return JSON.
  - `POST /api/sandbox/:threadId/opencode/provider/openai/oauth/callback`  
    Body: `{ method, code? }`. Same: resolve sandbox, proxy to container `.../oauth/callback`. This request will block until the plugin’s callback resolves (device flow polls until user completes).
  - Scope id is always `thread_id` so the same OpenCode process (and its `auth.json`) is used for chat and Codex.
  - **Effect:** implement these routes using Effect handlers and `effect/Schema` for request/response validation. Convert sandbox/OpenCode failures into tagged errors and map them to consistent HTTP responses.
  - **Timeouts:** enforce an upper bound for the blocking callback and abort the container fetch if the client disconnects, to avoid leaking long-poll requests.

### B.4 Webapp → backend

**Decision:** The webapp calls the **same worker** that owns the sandbox (LiteClaw worker). Frontend uses:
`fetch(workerUrl + '/api/sandbox/' + threadId + '/opencode/provider/openai/oauth/...')`.

**Auth:** use a **short-lived, signed session token** that includes `thread_id`, `user_id`, and `exp`. The worker verifies this token (HMAC or JWT) before proxying to the sandbox. This keeps Convex out of the long-polling callback path while still enforcing thread ownership.

### B.5 Browser redirect flow (optional, requires OpenCode changes)

- To support the **browser** Codex flow (user clicks “Sign in with ChatGPT”, redirects back to our domain), OpenCode’s Codex plugin must support a **configurable redirect_uri** and a **callback route on the main OpenCode server** (see [opencode-codex-auth.md](./opencode-codex-auth.md) “Making the browser flow work”). That work is in the **OpenCode repo**, not in openagents.
- The **PKCE state/verifier** must be stored server-side in the OpenCode process so the Worker callback can complete without the localhost listener.
- Once OpenCode supports it:
  - Worker would proxy a **GET** callback path (e.g. `GET /api/sandbox/opencode-oauth/redirect?code=...&state=...`) to the container’s OpenCode server, which would complete the exchange and return success HTML.
  - Webapp would use method index **0** (browser) and open the auth URL; redirect would go to the worker’s public URL, then to the container.
- Until then, **device code only** in the webapp.

### B.6 Persistence and multi-tab (decision)

- OpenCode stores tokens in `auth.json` inside the container. Containers are ephemeral; when the sandbox sleeps or is recreated, that file is lost unless we persist it.
- **Decision:** Persist tokens **outside the container** and re-inject on sandbox start.
  - Store encrypted token payloads in the LiteClaw DO storage keyed by `thread_id`.
  - Encrypt at rest with a worker secret (AES-GCM), rotate when needed, and fail closed if decryption fails.
  - On sandbox start: hydrate `auth.json` before launching OpenCode.
  - On refresh or re-auth: write-through to storage.

### B.7 Summary for Codex

| Item | Action |
|------|--------|
| Auth flow to use first | Device code (method 1); no OpenCode changes. |
| Webapp | Add “Connect Codex” UI; call backend for authorize + callback (with thread). |
| Backend | Worker routes that proxy authorize/callback to OpenCode in the sandbox; scope = thread. |
| Browser redirect flow | Later; depends on OpenCode adding configurable redirect_uri + server callback route. |
| Token persistence | Persist encrypted tokens in DO storage and re-inject `auth.json` on sandbox start. |

---

## Dependency order

1. **Part A (Sandbox)** first: worker with Sandbox DO, session per thread, and at least one of: (a) sandbox-backed tools, or (b) OpenCode server in container + proxy to webapp (e.g. iframe or new window).
2. **Part B (Codex auth)** next: add worker routes that proxy OpenCode OAuth to the sandbox; add webapp “Connect Codex” using device code; persist tokens for cold start.

If you only want “OpenCode in sandbox” without Codex, Part A is enough (use API keys for models). If you want Codex in the web without sandbox (e.g. a different backend that runs OpenCode on a long-lived server), that would be a different design and not covered here.

---

## Files and places to touch (checklist)

**Sandbox (Part A)**

- [ ] Worker app (`apps/liteclaw-worker`): add `@cloudflare/sandbox`, wrangler Sandbox class + binding, getSandbox + session id from thread.
- [ ] Worker: implement tool handlers or OpenCode proxy that use Sandbox SDK (exec, files, or `createOpencodeServer` + `proxyToOpencode`).
- [ ] Optional: `apps/web` – link or route to “OpenCode in sandbox” (e.g. `/sandbox/opencode?thread=...`) if you expose the full UI.

**Codex auth (Part B)**

- [ ] Worker: `POST .../opencode/provider/openai/oauth/authorize` and `.../callback` that proxy to container:4096.
- [ ] Webapp: “Connect ChatGPT (Codex)” UI (device code: show URL + code, then long-poll callback).
- [ ] Webapp: pass thread to backend; worker validates and maps to sandbox.
- [ ] Token persistence implementation and re-inject on sandbox start.

---

## References

- [opencode-codex-auth.md](./opencode-codex-auth.md) – OpenCode Codex flows and web viability.
- [Sandbox SDK](https://developers.cloudflare.com/sandbox/) – Getting started, API, concepts, tutorials.
- [LiteClaw README](../liteclaw/README.md) and [spec](../liteclaw/spec.md) – Current worker and webapp flow.
- [cloudflare-agent-sdk-demo](../cloudflare/) – Effect + Agents SDK structure (no sandbox).
