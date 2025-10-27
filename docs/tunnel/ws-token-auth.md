Title: WebSocket token auth for the bridge (/ws)

Overview
- The Rust WebSocket bridge at /ws requires a shared secret token for every connection. This prevents unauthorized access when exposed via a public tunnel.
- Clients must present the token via one of:
  - Authorization: Bearer <token>
  - Query parameter: ?token=<token>
Token is always required; there is no unauthenticated mode.

Configure the token
- Env var: set OPENAGENTS_BRIDGE_TOKEN before launching the bridge.
- Config file: create ~/.openagents/bridge.json with:
  {
    "token": "<your-secret>"
  }
  The env var takes precedence over the file.
- Auto-generate: if neither env nor file is provided, the bridge generates a new random token at startup and writes it to ~/.openagents/bridge.json.

Client URL examples
- Query form: wss://host:port/ws?token=<your-secret>
- Header form (recommended): set Authorization: Bearer <your-secret> on the WebSocket upgrade request.

Behavior
- On missing/wrong token, the server returns 401 Unauthorized and refuses the upgrade.
- Tokens are not logged. Avoid enabling BRIDGE_DEBUG_WS in production because it may echo inbound payload previews.

Tricoder and tunnels
- The tricoder launcher should surface the final URL including the token for copy/paste. For example:
  wss://<public-host>:<port>/ws?token=<your-secret>
- For iOS ATS, prefer a TLS-terminating tunnel or proxy so the app can connect with wss://.

Notes
- Only the /ws route is gated; no additional REST endpoints were added.
- OPENAGENTS_HOME can be overridden to point at a custom ~/.openagents root.
