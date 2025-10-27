Ngrok (ngrok-rust) vs. Bore For Remote Mobile ↔ Desktop Bridge

Summary
- Goal: connect the mobile app to the desktop bridge and Convex backend across networks with minimal setup and good security/ATS compliance.
- Options we’ve trialed or considered:
  - Bore: simple reverse TCP tunnel (public bore.pub or self‑host), plaintext by default.
  - Ngrok (via ngrok-rust): embedded tunneling library that provides HTTPS/WSS endpoints with TLS, auth, and an edge feature set.

Quick Verdict
- For internal validation and “it just works” TLS, ngrok-rust is the better fit. It yields stable HTTPS/WSS URLs without us running an edge, satisfies iOS ATS, and has built‑in security controls. Bore is great for quick demos and when we want no external account, but it’s plaintext and needs extra work to be safe and ATS‑friendly.

What Each Provides
- Bore
  - Creates a public TCP listener mapped to a local port.
  - Works with any TCP protocol (HTTP/WebSocket), no TLS or auth by default.
  - Use public bore.pub or self‑host the server; ephemeral ports.
  - Pros: zero account, tiny, simple, easy to vendor or fork.
  - Cons: plaintext, iOS ATS friction (ws:// blocked), no built‑in TLS/auth; stability depends on bore.pub unless we self‑host.

- Ngrok-rust
  - Native Rust crate (MIT/Apache) that embeds the ngrok agent into our process.
  - Programmatically creates HTTP/TCP tunnels; for HTTP it returns an HTTPS URL (WSS compatible for WS upgrades).
  - Features we can turn on from Rust: TLS termination, basic auth, OAuth/OIDC, IP policies, headers, policy engine, request inspection, reserved domains, labels/metadata, health checks.
  - Pros: one‑line HTTPS/WSS public URL, ATS‑friendly, rich security and routing features, no separate binary or shelling out.
  - Cons: requires ngrok account + authtoken; free plan is limited (ephemeral URLs, rate limits); traffic traverses ngrok’s edge.

Fit For Our Bridge
- Bridge is Axum WS at `/ws` on local `:8787` and Convex on local `:7788`.
- Bore works if we open two tunnels and paste two ports; still plaintext unless we add TLS elsewhere.
- Ngrok lets us open two HTTP tunnels (to 8787 and 7788) and returns TLS endpoints:
  - Bridge: produce `https://<subdomain>.ngrok.*` → app can use `wss://…/ws`.
  - Convex: produce `https://<subdomain2>.ngrok.*` → app pastes as Convex base URL.
- ATS: satisfied because ngrok produces TLS URLs.

Security Considerations
- Bore path:
  - Add WS token‑gating on our bridge before exposing publicly.
  - Add TLS either by putting a TLS proxy in front or by forking Bore to support TLS.
- Ngrok path:
  - WS token gating still recommended, but transport is TLS out of the box.
  - Optional auth controls (basic auth, OAuth/OIDC) at the edge; IP allow‑listing if needed.

Observability & Reliability
- Bore: no built‑in inspector; rely on logs. Public bore.pub is best‑effort.
- Ngrok: request inspector + logs via dashboard; ngrok edge reliability and routing; reserved, stable domains with paid plans.

Code Sketch: Using ngrok-rust From Our Workspace
- Crate dependency: `cargo add ngrok` in a small helper crate (e.g., extend `crates/oa-tunnel` with a `--provider ngrok` flag).
- Bridge tunnel (WS at /ws):
```rust
use url::Url;

let session = ngrok::Session::builder()
    .authtoken_from_env() // set NGROK_AUTHTOKEN
    .connect()
    .await?;

// Forward public HTTPS → local http://127.0.0.1:8787
let mut ws_tunnel = session
    .http_endpoint()
    .listen_and_forward(Url::parse("http://127.0.0.1:8787").unwrap())
    .await?;

let bridge_https = ws_tunnel.url().to_string();
let bridge_wss = bridge_https.replacen("https://", "wss://", 1) + "/ws";
println!("{}", bridge_wss);

// Keep running: ws_tunnel.join().await?;
```

- Convex tunnel (HTTP API):
```rust
let mut convex_tunnel = session
    .http_endpoint()
    .listen_and_forward(Url::parse("http://127.0.0.1:7788").unwrap())
    .await?;

let convex_https = convex_tunnel.url().to_string();
println!("{}", convex_https);
```

Integration Plan
- Phase 1 (internal validation):
  - Extend `crates/oa-tunnel` with `--provider=ngrok|bore`.
  - For `ngrok`, open two HTTP tunnels via `listen_and_forward` and print:
    - Bridge URL as `wss://…/ws`
    - Convex URL as `https://…`
  - Update `tricoder` to use `--provider ngrok` when `NGROK_AUTHTOKEN` is present; otherwise fall back to Bore.

- Phase 2 (security polish):
  - Add WS token gating to `/ws` in the bridge.
  - Optionally enable edge auth on the ngrok side.

- Phase 3 (better UX):
  - Collapse to a single domain using ngrok edge routing (path‑based forwarders), or proxy Convex through the bridge (e.g., `/convex`) so we output one URL.
  - Add a QR payload that carries both bridge and Convex URLs (or a single consolidated URL) for one‑scan setup.

Cost & Ops
- Bore: no account, can self‑host; plaintext unless we add TLS.
- Ngrok: free plan OK for validation, reserved domains + higher limits require paid. Zero ops to get TLS + WSS + ATS compliance.

Recommendation
- Use Bore for quick, no‑account demos (we already support it).
- Prefer ngrok‑rust for the default “just works” flow: secure (TLS), ATS‑friendly, and easy to integrate with our Rust workspace. Keep WS token gating on our side either way.

