Date: 2025-10-27

Summary
- Mobile ↔ Desktop via Bore tunnels is working for both the bridge and Convex.
- Pairing switched to a single base64url code; the app decodes and auto-connects.
- Convex functions are bootstrapped automatically; history populates.

Evidence (tricoder)
- Bridge WS (public): HTTP/1.1 101 Switching Protocols
- Convex HTTP (public): GET /instance_version -> 200 body: unknown
- Local health: 127.0.0.1:8787 reachable; http://127.0.0.1:7788 healthy
- Pair summary example:
  - [pair] bridge=ws://bore.pub:20080/ws convex=http://bore.pub:16599

App updates
- Settings shows Convex base URL and HTTP status inline.
- Drawer now lists all threads (no zero-message filter) and uses threads:listWithCounts.
- Bridge Code field has a clear (trash) icon; centered alignment.

Tricoder improvements
- Public/local probes with clear prefixes; WS handshake check.
- Tunnel event aggregation every 10s to reduce spam.
- Auto-start cargo bridge if needed.
- Auto-bootstrap Convex functions and seed demo data via bridge controls.

Next
- Add WS token gating on bridge `/ws`.
- Add TLS termination for public endpoints (wss/https) and collapse to a single URL.
