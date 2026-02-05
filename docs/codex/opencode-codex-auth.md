## How OpenCode does Codex auth

Codex auth lives in the **built-in plugin** at `packages/opencode/src/plugin/codex.ts`. It’s loaded as an internal plugin (with Copilot) in `packages/opencode/src/plugin/index.ts` and registers **OpenAI** with two OAuth methods and one API-key method.

### 1. **ChatGPT Pro/Plus (browser)** – method index 0

- **PKCE** with `https://auth.openai.com` (client_id, scopes, `code_challenge`, etc.).
- A **local OAuth callback server** is started with `Bun.serve()` on **port 1455**.
- `redirect_uri` is fixed: **`http://localhost:1455/auth/callback`**.
- Flow:
  1. `authorize()` starts the server, builds the auth URL with that redirect_uri, and returns `{ url, method: "auto", instructions }`.
  2. Frontend opens the URL; user signs in at OpenAI; OpenAI redirects the **browser** to `http://localhost:1455/auth/callback?code=...&state=...`.
  3. The **local** server on 1455 receives that GET, exchanges code for tokens, resolves the pending promise, and returns HTML (“Authorization Successful”).
  4. The frontend has already sent `POST /provider/openai/oauth/callback` (no body). That request is **blocking** on the server until the plugin’s `callback()` resolves (i.e. until the local server on 1455 gets the redirect and completes the exchange).
  5. `ProviderAuth.callback()` then calls `Auth.set("openai", { type: "oauth", ... })` and the flow is done.

So the **browser** flow assumes the redirect hits a server on the **user’s machine** at `localhost:1455`. The OpenCode server (4096) never receives the redirect; it only waits for the in-process promise that the local server on 1455 resolves.

### 2. **ChatGPT Pro/Plus (headless)** – method index 1

- **Device code** flow: no local server, no redirect to the app.
- `authorize()` calls `https://auth.openai.com/api/accounts/deviceauth/usercode`, gets `user_code` and `device_auth_id`, and returns e.g. `url: "https://auth.openai.com/codex/device"` and `instructions: "Enter code: XXXXX"`.
- User opens that URL and enters the code.
- `callback()` (no code in body) **polls** `https://auth.openai.com/api/accounts/deviceauth/token` until the user completes the step, then exchanges the returned authorization code for tokens and calls `Auth.set(...)`.

All of this runs inside the OpenCode process; no callback URL on the app is needed.

### Server API (used by web + CLI)

- **`POST /provider/:providerID/oauth/authorize`** – body: `{ method: number }`. Dispatches to the plugin’s method; returns `{ url, method: "auto"|"code", instructions }`.
- **`POST /provider/:providerID/oauth/callback`** – body: `{ method, code? }`. For “auto” it runs the plugin’s `callback()` (which either waits for the local server or polls device auth); for “code” it uses the pasted code. Then calls `Auth.set(...)`.

Auth is stored in `Auth` (e.g. `auth.json` on disk) and the Codex plugin’s **loader** uses it to add a custom `fetch` that sends the access token and rewrites requests to `https://chatgpt.com/backend-api/codex/responses`.

---

## Can we do this in the web via Cloudflare Sandbox?

When the UI is served through the Worker, the **OpenCode server runs inside the container**; the browser only talks to the Worker, which proxies to the container (e.g. port 4096).

### Browser flow (method 0) – **does not work as-is**

- Redirect URI is **always** `http://localhost:1455/auth/callback`.
- After login, the **user’s browser** is redirected to **their** `localhost:1455`. Nothing is listening there; the OAuth server on 1455 runs **inside the container**, so the redirect never reaches it and the promise in the plugin never resolves.
- So the **current** browser (PKCE + local server) flow **cannot** work when the only entry point is the web UI through the Worker.

### Device code flow (method 1) – **works**

- No redirect to the app. The plugin only needs to:
  - Call OpenAI’s device-auth APIs from inside the container.
  - Poll until the user has completed the step on `https://auth.openai.com/codex/device`.
- The frontend already supports it: it calls `authorize` with the **headless** method index, shows the URL and code, then calls `callback` with that method. Both requests go Worker → container; the container runs the plugin and polls until success.
- So **Codex auth via the device (headless) flow is possible today** when OpenCode runs in the Sandbox and the web UI is proxied through the Worker, as long as the UI uses method index **1** (headless) for OpenAI.

### Making the browser flow work in the web (would require OpenCode changes)

To support the **browser** redirect flow when the UI is behind the Worker, you’d need something like:

1. **Configurable redirect_uri**
   In the Codex plugin, allow a redirect URI that is **not** localhost (e.g. from env or config), e.g. the Worker’s public URL:
   `https://<your-worker>.<subdomain>.workers.dev/provider/openai/oauth/redirect` (or a dedicated path).

2. **No local server when using a web redirect**
   If redirect_uri is that public URL, **don’t** start `Bun.serve` on 1455. Instead, store the pending PKCE/state in the same way the plugin already does, but resolve it when the **main** OpenCode server receives the callback.

3. **Callback route on the main server**
   Add a route on the OpenCode Hono app (e.g. `GET /provider/openai/oauth/redirect`) that:
   - Reads `code` and `state` from the query.
   - Finds the pending OAuth for that state (you’d need to expose or replicate the plugin’s pending state).
   - Exchanges the code for tokens (same logic as in the plugin’s local server).
   - Calls `Auth.set("openai", { type: "oauth", ... })` and returns the same “Authorization Successful” HTML (or redirect back to the app).

4. **Worker and proxy**
   Ensure the Worker proxies **all** requests (including that path) to the container’s OpenCode server (port 4096). Then the redirect from OpenAI would go:
   Browser → Worker → Container (OpenCode) → handle callback and complete auth.

So: **today you can use Codex auth in the web via Sandbox with the device (headless) flow**. The browser (redirect) flow would require the changes above in the OpenCode repo (configurable redirect_uri, optional local server, and a server-side callback route).
