Cloudburrow: Broker‑Issued Cloudflare Tunnels for the Desktop Bridge

Overview
- Cloudburrow mints per‑device Cloudflare Tunnels on demand and returns `{ hostname, tunnelId, token }` to the desktop.
- The desktop runs `cloudflared tunnel run --token …` and exposes the Rust bridge at a stable `wss://<hostname>/ws` URL.
- Tricoder defaults to the Cloudburrow broker and manages the connector lifecycle for you.

Why
- Predictable, first‑level hostnames for each device (no random quick‑tunnel subpaths).
- Simpler pairing UX (QR / code), no local port forwarding configuration.
- Robust on locked‑down networks by forcing HTTP/2 to Cloudflare edge.

Prereqs
- Bun installed.
- Rust toolchain for the bridge.
- `cloudflared` is auto‑installed by Tricoder under `~/.openagents/bin` if missing.

How to use (dev)
1) Run the desktop helper from Tricoder:
   - `bunx tricoder`
   - Wait for the pairing code to print. It includes `provider:"cloudflare"`, the public `wss://…/ws` URL, and your local Convex URL.
2) In the mobile app → Settings → Bridge Code, paste the code or scan the QR. Tap Connect.
3) Expect connect attempts like: `wss://cloudburrow-<slug>.openagents.com/ws`.

Notes on stability
- Some networks block UDP/QUIC. Tricoder forces HTTP/2 transport to the edge.
- We also keep a single origin keepalive connection (`--proxy-keepalive-connections 1`) to reduce local port churn.
- If you ever see “can’t assign requested address” to `127.0.0.1:8787`, make sure only one `cloudflared` is running. Tricoder now enforces single‑instance using a PID file.

Cloudflare (what’s in the UI and what’s automated)
- Already done in this setup:
  - Custom Domain bound: `cloudburrow-broker.openagents.com` → the Worker.
  - Worker has vars: `TUNNEL_HOST_PREFIX=cloudburrow-`, `TUNNEL_HOST_SUFFIX=openagents.com`.
  - Broker endpoints:
    - `POST /tunnels` → create tunnel, mint token, add CNAME, and set remote ingress to `http://127.0.0.1:8787`.
    - `GET /tunnels/:id/status`
    - `DELETE /tunnels/:id`
- Required Worker secrets (set once):
  - `CF_API_TOKEN` — token with permissions: Cloudflare Tunnel (Edit) + DNS (Edit) on the account/zone.
  - `CF_ACCOUNT_ID` — account id
  - `CF_ZONE_ID` — zone id for `openagents.com`
  - Optional `BROKER_KEY` — if set, broker endpoints require `Authorization: Bearer <BROKER_KEY>`.
- You can set these from the `cloudburrow` repo root with:
  - `bun run cf:secret:api-token`
  - `bun run cf:secret:account-id`
  - `bun run cf:secret:zone-id`
  - `bun run cf:secret:broker-key`

Troubleshooting
- QUIC errors in logs are expected on some networks; HTTP/2 still registers.
- If DNS for `cloudburrow-<slug>.openagents.com` hasn’t propagated yet, wait a few seconds and retry.
- If origin 127.0.0.1:8787 is reported “not reachable”, confirm the Rust bridge is running (Tricoder keeps starting it until healthy).
