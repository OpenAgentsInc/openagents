# Tunneling Overview (Updated)

We are standardizing on a single provider: **Cloudflare Tunnel (named only)** for the desktop bridge.

- Goal: expose the local Rust WebSocket bridge (`http://localhost:8787/ws`) on a stable, TLS endpoint for the mobile app.
- Result: a single hostname, e.g., `wss://tunnel.openagents.com/ws`, protected by our WS token and optionally Cloudflare Access.
- Status: tracked in issue #1324 — “Switch Tricoder tunnels from bore to Cloudflare Tunnel (named only)”.

## Why Cloudflare Tunnel (named-only)

- TLS and ATS-compliant by default; WebSockets work out of the box.
- Stable DNS hostname; easy to add Cloudflare Access (SSO/OTP) in front.
- Removes ambiguity and flaky ports from bore-style tunnels and avoids ngrok account/limits.

> We explicitly removed “Quick Tunnel” fallback to reduce complexity. All docs and tooling should assume a named tunnel.

## One-time setup

Follow the step-by-step guide: `docs/tunnel/cloudflare-named-setup.md`

That guide covers:
- Installing `cloudflared`
- `cloudflared tunnel login`
- Creating the named tunnel `tricoder-bridge`
- Writing `config.yml` with an ingress for `tunnel.openagents.com → http://localhost:8787`
- Creating the DNS route
- Running the tunnel (foreground or as a service)

When running, you should be able to:
- Open `https://tunnel.openagents.com` (HTTP response)
- Connect WS: `wss://tunnel.openagents.com/ws` (e.g., `npx wscat -c wss://tunnel.openagents.com/ws`)

## How Tricoder uses it

- Tricoder reads the configured hostname and emits a pairing payload containing:
  - `provider: "cloudflare"`
  - `bridge: wss://tunnel.openagents.com/ws`
  - `convex: http://127.0.0.1:7788` (local) or a named ingress if configured later
  - `token: <secret>` (required by the bridge)
- The mobile app scans the QR or deep link and connects over WSS.

## Security

- The Rust bridge at `/ws` requires a token (query `?token=` or `Authorization: Bearer`). See `docs/tunnel/ws-token-auth.md`
- Consider enabling Cloudflare Access on `tunnel.openagents.com` for an additional SSO/OTP gate.
- Logs should not print the full token; Tricoder shows it only in the QR/deep link payload.

## Historical notes (bore / ngrok)

- We previously evaluated/used bore and ngrok; those notes are kept for reference:
  - `docs/tunnel/bore-learnings-from-ngrok.md`
  - `docs/tunnel/ngrok-vs-bore.md`
- Decision: move to a named Cloudflare Tunnel permanently. No Quick Tunnel, no bore, no ngrok in the default flow.

## Troubleshooting

- If the mobile app reports “unreachable”, verify the tunnel is running and the local bridge is up:
  - `curl -I https://tunnel.openagents.com`
  - `npx wscat -c wss://tunnel.openagents.com/ws`
- Ensure the token is included. Missing/invalid tokens return HTTP 401 on upgrade.
- Some networks block high outbound ports; named Cloudflare Tunnel uses standard HTTPS (443) and avoids this.

## Tracking issue

- #1324 — Switch Tricoder tunnels from bore to Cloudflare Tunnel (named only)

