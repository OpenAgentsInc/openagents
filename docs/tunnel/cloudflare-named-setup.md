# Cloudflare Tunnel — One‑Time Named Setup (tunnel.openagents.com)

Use these steps once to create a stable, TLS‑terminating Cloudflare Tunnel for the desktop bridge (codex-bridge at `http://localhost:8787`). After this, Tricoder can publish a WSS endpoint like `wss://tunnel.openagents.com/ws` without any bore/Quick Tunnel fallbacks.

Audience: operator of the desktop bridge who manages DNS for `openagents.com`.

## Prerequisites

- Cloudflare account with the `openagents.com` zone using Cloudflare nameservers.
- Local desktop runs the bridge at `http://localhost:8787` (Axum WS on `/ws`).
- Install `cloudflared` on the machine that runs the bridge.

### Install cloudflared

- macOS (Homebrew):

```bash
brew install cloudflared
```

- Debian/Ubuntu:

```bash
sudo mkdir -p --mode=0755 /usr/share/keyrings
curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg \
  | sudo tee /usr/share/keyrings/cloudflare-main.gpg >/dev/null
echo "deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared any main" \
  | sudo tee /etc/apt/sources.list.d/cloudflared.list
sudo apt-get update && sudo apt-get install -y cloudflared
```

## 1) Authenticate the connector

```bash
cloudflared tunnel login
```

This opens a browser to select your account/zone and writes `cert.pem` to your cloudflared config directory.

## 2) Create a named tunnel

```bash
cloudflared tunnel create tricoder-bridge
```

Note the printed Tunnel UUID and the path to the credentials JSON (e.g., `~/.cloudflared/<UUID>.json`).

## 3) Configure ingress for the desktop bridge

Create `~/.cloudflared/config.yml` (or `/etc/cloudflared/config.yml` if running as a system service):

```yaml
tunnel: <TUNNEL-UUID>                # from the create step; you can also use the name
credentials-file: /Users/<you>/.cloudflared/<TUNNEL-UUID>.json

ingress:
  - hostname: tunnel.openagents.com
    service: http://localhost:8787   # Cloudflare auto-upgrades WS to WSS; /ws remains the WS route
  - service: http_status:404         # required catch-all
```

Validate syntax:

```bash
cloudflared tunnel ingress validate
```

## 4) Route DNS for the hostname

```bash
cloudflared tunnel route dns tricoder-bridge tunnel.openagents.com
```

This creates a CNAME: `tunnel.openagents.com → <UUID>.cfargotunnel.com` in your Cloudflare zone.

## 5) Run the tunnel

First run (foreground):

```bash
cloudflared tunnel --config ~/.cloudflared/config.yml run tricoder-bridge
```

You should be able to:

- Open `https://tunnel.openagents.com` (HTTP 200/404 depending on bridge root)
- Connect WebSocket: `wss://tunnel.openagents.com/ws` (e.g., `npx wscat -c wss://tunnel.openagents.com/ws`)

### Optional: run at boot

- macOS (LaunchAgent):

```bash
cloudflared service install
```

- macOS (LaunchDaemon, system-wide):

```bash
sudo cloudflared service install
```

- Linux (systemd): follow Cloudflare’s "Run as a service" docs; service name is `cloudflared.service`.

## 6) (Optional) Add Access in front of the hostname

In Cloudflare Zero Trust → Access, protect `tunnel.openagents.com` with an app policy (e.g., GitHub/Google SSO or OTP). This adds an extra gate in addition to the WS token Tricoder already enforces.

## Notes

- WebSockets are supported natively by Cloudflare Tunnel; no extra configuration is needed beyond the HTTP origin.
- If you also want to expose the local Convex backend publicly, add another ingress rule mapping `http://localhost:7788` to a separate hostname (e.g., `convex.openagents.com`) and create a corresponding DNS route.
- Tricoder pairing payload will show `provider: "cloudflare"` and use the `wss://tunnel.openagents.com/ws` URL. The token remains required — unauthenticated WS receives HTTP 401.

