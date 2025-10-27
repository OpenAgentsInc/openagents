Bore++: What We Can Borrow From ngrok (without an account)

Purpose
- Capture pragmatic features and patterns from the ngrok-rust codebase that we can implement in our Bore-based stack to deliver a secure, ATS‑friendly, “one URL” experience with no third‑party account.

Scope
- We keep Bore’s fundamental “client ↔ server ↔ public TCP” architecture, but add TLS, routing, and guardrails inspired by ngrok.
- Short‑term: minimal code deltas to ship a safe, usable MVP.
- Medium‑term: a cohesive “Bore++” design that scales and feels polished.

Key Lessons From ngrok-rust
- TLS first, everywhere
  - ngrok HTTP endpoints are HTTPS (and support WS upgrades to WSS). iOS ATS is happy; users don’t think about certs.
  - Implementation cues: futures-rustls/rustls acceptors, ALPN for h2/http1, optional upstream TLS verification.
- “listen_and_forward” abstraction
  - Clear API that maps a public listener to a local URL. For us: public TLS listener → local 127.0.0.1:8787 (/ws) and 127.0.0.1:7788 (Convex).
  - Small convenience with big UX payoff. Our client/server can agree on a single “forward spec”.
- Edge controls before forwarding
  - Basic auth, OAuth/OIDC, IP allow/deny, header add/remove. We don’t need all of these now, but a simple allowlist/token gate at the edge is valuable.
- Metadata + observability
  - ngrok exposes tunnel URL, forwards_to, and offers an inspector. We can log per-conn summaries and print stable URLs.
- Domain and routing
  - ngrok routes many tunnels through a single TLS edge via host/path rules. We can move off “random remote port per tunnel” to SNI/Host routing for a single :443 endpoint.

What To Build Into Our Bore Fork (Incrementally)
1) TLS termination on the public side (server)
   - Accept incoming TLS on the “remote” listener (today’s random port). Wrap accepted TcpStream in a rustls acceptor. Pipe decrypted bytes through the existing server↔client tunnel.
   - Certs: start with static PEM keypair; add rustls-acme for auto‑provision under our domain later.
   - ALPN: advertise ["h2", "http/1.1"]. WS upgrades keep working.

2) WS token gating at the bridge edge (app layer)
   - Not strictly part of Bore, but essential once public. Require `?token=…` on `/ws` in our Axum bridge.
   - tricoder prints `wss://…/ws?token=…` so users paste one URL.

3) One-port, host‑based routing (server)
   - Replace “port-per-tunnel” with a single TLS listener on :443.
   - Use SNI (TLS ServerName) or HTTP Host to route to the correct tunnel. Assign `*.bridge.openagents.dev` and `*.convex.openagents.dev` subdomains.
   - On Accept, stash the SNI/Host and select the tunnel; fall back to 404 if unknown.

4) Minimal edge auth & filtering (server)
   - Basic auth (401) toggle per tunnel.
   - IP allow/deny lists (CIDR match) to block the world by default in some modes.
   - Header add/remove (e.g., set `X-Forwarded-For`, `X-Forwarded-Proto=wss/https`).

5) Upstream TLS (optional)
   - If a local service is TLS (rare for us), support TLS on the client side when dialing `local_host:local_port` with `verify_upstream_tls` flag.

6) Developer ergonomics
   - “listen_and_forward” style config in the client: forward spec = { proto: http, to: http://127.0.0.1:8787, pathRules: ["/ws"] }.
   - Print exactly two URLs (Bridge WSS + Convex HTTPS) or consolidate to one URL once the bridge proxies `/convex`.
   - Structured logs: tunnel URL, SNI/Host, conn counts, errors.

Concrete Implementation Notes
- Server TLS acceptor
  - Use rustls with aws-lc provider (like ngrok) for performance, or pure rustls default.
  - Derive ALPN to support h2 for future HTTP/2 (not required for WS).
  - For SNI routing, inspect `server_name` from the rustls accept state.

- Tunnel registry keyed by SNI/Host
  - Today Bore keys by remote port. Add an optional “host label” on tunnel creation.
  - Server maps `example.bridge.openagents.dev` → tunnel X, `example.convex.openagents.dev` → tunnel Y.
  - Return the fully qualified public URLs to the client so tricoder can print them.

- Health and inspector (MVP)
  - Add `/healthz` and `/version` on the server’s control plane.
  - Optional simple inspector: per-connection counters and last errors logged to a control socket or a small HTTP page.

- ACME (Phase 2)
  - Use rustls-acme for automatic cert issuance/renewal. Start with a staging CA for development.
  - Store certs in `~/.openagents/certs/` on the server.

Security Timeline
- Step 1 (now): TLS termination + WS token at bridge. Satisfies ATS and basic abuse protection.
- Step 2: Host-based routing on :443 for a single URL per tunnel. Adds polish and avoids confusing ports.
- Step 3: Basic auth and IP allow/deny if we need stricter external access.

What We Won’t Rebuild (Yet)
- Full ngrok policy engine, OAuth/OIDC integrations, dashboards, or a global edge. We’ll keep it minimal and self‑hosted.

Open Questions
- Do we want per-user subdomains or a shared domain with opaque path tokens?
- Central server location and ops (we can start with a small VM and systemd).
- Should the bridge proxy Convex under `/convex` so users need only one tunnel?

Next Actions
- Prototype TLS acceptor in a Bore server fork and gate by a `--tls` flag with a PEM.
- Add SNI label in the client handshake (optional field) and route by SNI if set.
- Ship bridge WS token gating (small Axum change) and print `wss://…/ws?token=…`.
- Add tricoder flags to switch providers once TLS is working end‑to‑end.

