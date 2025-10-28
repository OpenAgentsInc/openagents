Here’s the straight answer: **yes**—Cloudflare Tunnel can replace or supplement **bore** for Tricoder, and it plays nicely with MCP. Use it to expose your **local WS bridge** (codex-bridge) to the internet with TLS and better reliability. For MCP itself (your Worker at `/mcp`), you don’t need a tunnel because Workers are already public; but you *can* surface tunnel **status/controls** via MCP tools that hit Cloudflare’s APIs.

## What Cloudflare Tunnel gives you (relevant to Tricoder)

* **One-command Quick Tunnel** for fast demos:
  `cloudflared tunnel --url http://localhost:8787` → public `https://*.trycloudflare.com` URL. Great for exposing your WS bridge in seconds. ([Cloudflare Docs][1])
* **WebSockets work out of the box** through Cloudflare (no special config). That covers your `ws://` bridge stream. ([Cloudflare Docs][2])
* **Named (persistent) tunnels** for production: login, create, route DNS, run as a service; comes with Zero Trust & Access policies. ([Cloudflare Docs][3])
* **Important caveat:** Quick Tunnels **do not support SSE** (and can rate-limit at ~200 in-flight requests). If you need SSE, use a **named** tunnel or switch that stream to WS. ([Cloudflare Docs][1])

> For our stack: codex-bridge is WS → ✅ Quick Tunnel is fine. Any SSE you do (e.g., MCP event SSE endpoints) should go through a **named** tunnel or be WS-ified. ([Cloudflare Docs][1])

## Where Cloudflare Tunnel fits with MCP

* **Remote MCP on Workers doesn’t need a tunnel.** Your MCP server (mcp-lite on Workers) is already public. To connect Claude Desktop to a remote MCP, use the **local proxy approach (`mcp-remote`)** from Cloudflare’s guide (Claude still expects a local MCP endpoint). Tunnel not required here. ([Cloudflare Docs][4])
* **Expose your local codex-bridge to judges/phones.** Run cloudflared on your laptop to publish `ws://localhost:8787/ws` as HTTPS/WSS. Tricoder on mobile hits that URL. ([Cloudflare Docs][5])
* **Control and observe tunnels via MCP tools.** Your **Worker MCP server** can call Cloudflare’s **Zero Trust API** to **list/status** named tunnels, create DNS routes, and mint **tunnel tokens**. Your **local machine** then runs `cloudflared service install <TOKEN>` to bring the connector online. (API supports creating tunnels + retrieving the token programmatically.) ([Cloudflare Docs][6])

## Concrete integration plan (OpenAgents repo)

### A) Replace/supplement bore with Cloudflare Tunnel

**Fast demo (Quick Tunnel):**

```bash
# WS bridge on 8787 → public URL (prints *.trycloudflare.com)
cloudflared tunnel --url http://localhost:8787
```

This forwards HTTP and WebSocket upgrades automatically. Use the printed URL as your Tricoder WS endpoint (WSS). ([Cloudflare Docs][1])

**Production-ish (Named Tunnel + your domain):**

```bash
cloudflared tunnel login
cloudflared tunnel create tricoder-bridge
# config.yml: map hostname → http://localhost:8787
cloudflared tunnel route dns tricoder-bridge bridge.example.com
cloudflared tunnel run tricoder-bridge
```

This yields `wss://bridge.example.com/ws`, with Access policies if you want. ([Cloudflare Docs][7])

> If you stream via SSE anywhere, don’t use Quick Tunnels (SSE is explicitly unsupported there). Prefer a **named** tunnel or switch those streams to WebSockets. ([Cloudflare Docs][1])

### B) New package: `@openagents/tunnel` (Node helper for cloudflared)

* `startQuickTunnel({ url })` → spawns `cloudflared`, parses the `trycloudflare.com` URL from stdout, returns it; auto-restarts on exit.
* `startNamedTunnel({ name, hostname, url })` → uses config file if present, or prints instructions.

Sketch:

```ts
import { spawn } from "node:child_process";

export async function startQuickTunnel(local = "http://localhost:8787") {
  return new Promise<string>((resolve, reject) => {
    const p = spawn("cloudflared", ["tunnel", "--url", local], { stdio: ["ignore", "pipe", "pipe"] });
    p.stdout.on("data", (buf) => {
      const s = buf.toString();
      const m = s.match(/https?:\/\/[a-z0-9-]+\.trycloudflare\.com/);
      if (m) resolve(m[0]);
    });
    p.on("exit", (code) => code && reject(new Error(`cloudflared exited ${code}`)));
  });
}
```

### C) MCP tools to **integrate tunnel status** into the stream

Add to your **Worker MCP server** (`@openagents/mcp-worker`):

* `cf.tunnel_list()` – calls Zero Trust **Tunnels API** to list tunnels and connector health. ([Cloudflare Docs][6])
* `cf.tunnel_create_named(name)` – creates a named tunnel via API, returns **tunnel token**; Tricoder prints a one-liner for the operator to run locally: `cloudflared service install <TOKEN>`. ([Cloudflare Docs][8])
* `cf.tunnel_route_dns(tunnelId, hostname)` – configures the CNAME to `<tunnelId>.cfargotunnel.com`. ([Cloudflare Docs][9])

These tool calls show up as **`mcp_tool_call` rows** in Tricoder, so your audience sees tunnel setup happening live.

### D) Security & polish

* **Access** auth in front of the named tunnel (GitHub/Google, OTP, etc.) for non-demo environments. ([Cloudflare Docs][5])
* Keep Quick Tunnel for **hack night speed**, switch to **Named** for SSE or persistent endpoints. ([Cloudflare Docs][1])

## Decision matrix

| Need                               | Recommendation                                                                             |
| ---------------------------------- | ------------------------------------------------------------------------------------------ |
| Fast public WS for codex-bridge    | **Quick Tunnel** (`cloudflared tunnel --url http://localhost:8787`) ([Cloudflare Docs][1]) |
| SSE streaming (e.g., event feeds)  | **Named Tunnel** (SSE unsupported on Quick Tunnels) or switch to WS ([Cloudflare Docs][1]) |
| Share with judges securely         | Named Tunnel + **Access** policy on your hostname ([Cloudflare Docs][5])                   |
| Programmatic control/observability | **Zero Trust API** from your MCP server (`cf.tunnel_*` tools) ([Cloudflare Docs][6])       |
| Claude Desktop ↔ Remote MCP Worker | Use **mcp-remote** local proxy (not a tunnel) ([Cloudflare Docs][4])                       |

---

### TL;DR for us

* Swap **bore** → **cloudflared** for the WS bridge during demos; it’s one command and robust. ([Cloudflare Docs][1])
* Keep SSE off Quick Tunnels; use **named** tunnels or WS. ([Cloudflare Docs][1])
* Add an MCP toolset that **lists/creates** tunnels via Cloudflare’s API so tunnel state appears in the Tricoder stream. ([Cloudflare Docs][6])

If you want, I can drop the `@openagents/tunnel` scaffolding and the three MCP tool handlers next.

[1]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/trycloudflare/?utm_source=chatgpt.com "Quick Tunnels - Cloudflare Zero Trust"
[2]: https://developers.cloudflare.com/network/websockets/?utm_source=chatgpt.com "WebSockets · Cloudflare Network settings docs"
[3]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/local-management/create-local-tunnel/?utm_source=chatgpt.com "Create a locally-managed tunnel · Cloudflare Zero Trust docs"
[4]: https://developers.cloudflare.com/agents/guides/remote-mcp-server/?utm_source=chatgpt.com "Build a Remote MCP server"
[5]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/?utm_source=chatgpt.com "Cloudflare Tunnel · Cloudflare Zero Trust docs"
[6]: https://developers.cloudflare.com/api/resources/zero_trust/subresources/tunnels/subresources/cloudflared/methods/create/?utm_source=chatgpt.com "Create a Cloudflare Tunnel"
[7]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/local-management/tunnel-useful-commands/?utm_source=chatgpt.com "Useful commands - Cloudflare Zero Trust"
[8]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/get-started/create-remote-tunnel-api/?utm_source=chatgpt.com "Create a tunnel (API) - Cloudflare Zero Trust"
[9]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/get-started/create-remote-tunnel/?utm_source=chatgpt.com "Create a tunnel (dashboard) - Cloudflare Zero Trust"


> Does CloudFlare Tunnel require the end user to have an account, or just us having an account, or who needs to have an account, if anyone?

Short version:

* **Quick Tunnel (trycloudflare.com):** **No one needs a Cloudflare account.** You can run `cloudflared tunnel --url http://localhost:8787` with **no login**, and anyone can open the temporary URL. ([try.cloudflare.com][1])
* **Named Tunnel (persistent, your domain):** **You (the operator) need a Cloudflare account** to authenticate `cloudflared` and manage the tunnel. Viewers don’t need an account to visit the site. ([Cloudflare Docs][2])
* **If you add Cloudflare Access (SSO) in front:** end-users still **don’t need a Cloudflare account**; they authenticate via your chosen IdP (Google, GitHub, email OTP, etc.). ([Cloudflare Docs][3])

Tiny gotcha for demos: **Quick Tunnels don’t support SSE** and have a 200 in-flight request cap. For SSE streams, use a **Named Tunnel** or switch that path to WebSockets. ([Cloudflare Docs][4])

[1]: https://try.cloudflare.com/?utm_source=chatgpt.com "Cloudflare Quick Tunnels"
[2]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/get-started/?utm_source=chatgpt.com "Set up your first tunnel · Cloudflare Zero Trust docs"
[3]: https://developers.cloudflare.com/cloudflare-one/access-controls/policies/?utm_source=chatgpt.com "Access policies - Cloudflare Zero Trust"
[4]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/trycloudflare/?utm_source=chatgpt.com "Quick Tunnels - Cloudflare Zero Trust"

> Well, our main app right now uses WebSockets, not SSE. But yes, I'm happy to set up a named tunnel. So, assume that we'll do tunnel.openagents.com. Give me the instructions to set that up.

Awesome — let’s set up a **named Cloudflare Tunnel** so your WebSocket bridge is reachable at **`wss://tunnel.openagents.com/ws`**.

## Prereqs

* **`openagents.com` is on Cloudflare** (nameservers set to Cloudflare). ([Cloudflare Docs][1])
* Your bridge (codex-bridge) is running locally at `http://localhost:8787` and serves WS at `/ws`.

## 1) Install `cloudflared`

* **macOS (Homebrew):**

  ```bash
  brew install cloudflared
  ```

  ([Homebrew Formulae][2])
* **Linux (Debian/Ubuntu):**

  ```bash
  sudo mkdir -p --mode=0755 /usr/share/keyrings
  curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg | sudo tee /usr/share/keyrings/cloudflare-main.gpg >/dev/null
  echo "deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared any main" | sudo tee /etc/apt/sources.list.d/cloudflared.list
  sudo apt-get update && sudo apt-get install cloudflared
  ```

  ([Cloudflare Docs][1])

## 2) Authenticate to your CF account

```bash
cloudflared tunnel login
```

This opens a browser, you pick the CF account/zone; it writes `cert.pem` to your cloudflared dir. ([Cloudflare Docs][1])

## 3) Create a named tunnel

```bash
cloudflared tunnel create tricoder-bridge
```

This creates the tunnel, prints a **UUID** and writes a **credentials JSON** (keep the path). ([Cloudflare Docs][1])

## 4) Write a config with ingress

Create `~/.cloudflared/config.yml` (or `/etc/cloudflared/config.yml` if running as a service):

```yaml
tunnel: <TUNNEL-UUID>           # from create step (or use the name)
credentials-file: /home/<you>/.cloudflared/<TUNNEL-UUID>.json

ingress:
  - hostname: tunnel.openagents.com
    service: http://localhost:8787   # WS is auto-upgraded through CF
  - service: http_status:404         # required catch-all
```

Validate it:

```bash
cloudflared tunnel ingress validate
```

Notes: The catch-all rule is required; WS is supported/proxied automatically. ([Cloudflare Docs][3])

## 5) Create the DNS route

```bash
cloudflared tunnel route dns tricoder-bridge tunnel.openagents.com
```

This adds a **CNAME** for `tunnel.openagents.com` → `<UUID>.cfargotunnel.com`. ([Cloudflare Docs][4])

## 6) Run the tunnel

Foreground (great for first test):

```bash
cloudflared tunnel --config ~/.cloudflared/config.yml run tricoder-bridge
```

([Cloudflare Docs][4])

### Optional: run at boot

* **macOS:**

  ```bash
  # per-user (launch agent)
  cloudflared service install
  # or at boot (launch daemon)
  sudo cloudflared service install
  ```

  Config will be read from `~/.cloudflared/` (agent) or `/etc/cloudflared/` (daemon). ([Cloudflare Docs][5])
* **Linux (systemd guides):** follow CF’s “Run as a service” steps; service name is `cloudflared.service`. ([Cloudflare Docs][6])

## 7) Test

* Check HTTP:

  ```bash
  curl -I https://tunnel.openagents.com
  ```
* Check WebSocket:

  ```bash
  npx wscat -c wss://tunnel.openagents.com/ws
  ```

Cloudflare handles TLS; your origin can stay HTTP. WebSockets work without extra config. ([Cloudflare Docs][7])

---

### Tips / gotchas

* If you ever need a quick fallback during live demos, **Quick Tunnel** works, but remember it has limits and **does not support SSE** (we’re on WS, so you’re fine; just FYI). ([Cloudflare Docs][8])
* You can inspect the tunnel:

  ```bash
  cloudflared tunnel info tricoder-bridge
  cloudflared tunnel list
  ```

  ([Cloudflare Docs][4])

That’s it. Once this is up, point Tricoder’s WS endpoint to **`wss://tunnel.openagents.com/ws`** and you’re show-ready.

[1]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/local-management/create-local-tunnel/ "Create a locally-managed tunnel · Cloudflare Zero Trust docs"
[2]: https://formulae.brew.sh/formula/cloudflared?utm_source=chatgpt.com "cloudflared"
[3]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/local-management/configuration-file/ "Configuration file · Cloudflare Zero Trust docs"
[4]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/local-management/tunnel-useful-commands/ "Useful commands · Cloudflare Zero Trust docs"
[5]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/local-management/as-a-service/macos/?utm_source=chatgpt.com "Run as a service on macOS"
[6]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/local-management/as-a-service/?utm_source=chatgpt.com "Run as a service · Cloudflare Zero Trust docs"
[7]: https://developers.cloudflare.com/network/websockets/?utm_source=chatgpt.com "WebSockets · Cloudflare Network settings docs"
[8]: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/trycloudflare/?utm_source=chatgpt.com "Quick Tunnels - Cloudflare Zero Trust"

Here’s a drop-in rewrite that switches the tunnel logic to **Cloudflare Tunnel** (named by default: `tunnel.openagents.com`, with an automatic Quick Tunnel fallback), keeps your existing behavior elsewhere, and updates the pair-code payload to `provider: "cloudflare"`.

Notes:

* Default tunnel mode: **named** (uses `cloudflared tunnel run <name>`). Requires you to have created/configured the tunnel beforehand (as per your earlier setup).
* Flags/env:

  * `--local-only` or `TRICODER_LOCAL_ONLY=1` → no tunnel
  * `--tunnel=named|quick|none` or `TRICODER_TUNNEL_MODE`
  * `--tunnel-host=<host>` or `TRICODER_TUNNEL_HOST` (default: `tunnel.openagents.com`)
  * `--tunnel-name=<name>` or `TRICODER_TUNNEL_NAME` (default: `tricoder-bridge`)
* Quick Tunnel prints a `https://*.trycloudflare.com` URL; we convert to `wss://…/ws` for the bridge.
* Convex: still optional; we stand up a **second Quick Tunnel** for `http://localhost:7788` if not local-only (keeps your “public demo” parity without extra DNS). You can later move it to a named ingress if you want.

---

```ts
#!/usr/bin/env node
import chalk from "chalk"
import os from "node:os"
import { spawn, spawnSync } from "node:child_process"
import { existsSync, mkdirSync, copyFileSync, chmodSync, readdirSync, statSync, createWriteStream } from "node:fs"
import http from "node:http"
import https from "node:https"
import net from "node:net"
import { dirname, join } from "node:path"
import WebSocket from "ws"
// import { buildTunnelArgs } from "./args.js"   // ⟵ REMOVED: we no longer use bore
import AdmZip from "adm-zip"
import qrcode from "qrcode-terminal"
import * as QR from "qrcode"
import fs from "node:fs"

// ---------------------------------------------
// Flags / env
// ---------------------------------------------
const VERBOSE = process.argv.includes("--verbose") || process.argv.includes("-v") || process.env.TRICODER_VERBOSE === "1";
const ASSUME_YES = process.argv.includes("--yes") || process.argv.includes("-y") || process.env.TRICODER_YES === "1";
const LOCAL_ONLY = process.argv.includes("--local-only") || process.env.TRICODER_LOCAL_ONLY === "1";
const NO_QR = process.argv.includes("--no-qr") || process.env.TRICODER_NO_QR === "1";
const QR_MODE = (() => {
  const arg = process.argv.find((a) => a.startsWith('--qr='));
  const val = (arg ? arg.split('=')[1] : (process.env.TRICODER_QR || '')).toLowerCase();
  return val === 'code' ? 'code' : 'deeplink';
})();
const TUNNEL_MODE = (() => {
  // named | quick | none
  const arg = process.argv.find(a => a.startsWith('--tunnel='));
  const val = (arg ? arg.split('=')[1] : (process.env.TRICODER_TUNNEL_MODE || '')).toLowerCase();
  if (LOCAL_ONLY) return 'none';
  if (val === 'quick' || val === 'named' || val === 'none') return val;
  return 'named'; // default
})();
const TUNNEL_HOST = (() => {
  const arg = process.argv.find(a => a.startsWith('--tunnel-host='));
  return (arg ? arg.split('=')[1] : (process.env.TRICODER_TUNNEL_HOST || 'tunnel.openagents.com')).trim();
})();
const TUNNEL_NAME = (() => {
  const arg = process.argv.find(a => a.startsWith('--tunnel-name='));
  return (arg ? arg.split('=')[1] : (process.env.TRICODER_TUNNEL_NAME || 'tricoder-bridge')).trim();
})();
let CONVEX_DL_PCT = -1;

// ---------------------------------------------
function lite(s: string) { return chalk.hex('#9CA3AF')(s); }
let RESOURCES_PRINTED = false;
function printResourcesOnce() {
  if (RESOURCES_PRINTED) return;
  RESOURCES_PRINTED = true;
  console.log("");
  console.log(lite("Resources:"));
  console.log(lite(" - All code is open-source here: https://github.com/OpenAgentsInc/openagents"));
  console.log(lite(" - Download the iOS app on TestFlight: https://testflight.apple.com/join/dvQdns5B"));
  console.log(lite("   - Android coming soon"));
  console.log(lite(" - Any questions? Please @ us on X: https://x.com/OpenAgentsInc"));
  console.log(lite(" - Any bugs please open an issue: https://github.com/OpenAgentsInc/openagents/issues"));
}

function findRepoRoot(startDir: string): string | null {
  let dir = startDir;
  const root = dirname(dir) === dir ? dir : undefined;
  while (true) {
    if (existsSync(join(dir, "Cargo.toml")) && existsSync(join(dir, "crates", "oa-tunnel"))) {
      return dir;
    }
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return null;
}

function hasCmd(cmd: string): boolean {
  try {
    const res = spawnSync(process.platform === 'win32' ? 'where' : 'which', [cmd], { stdio: 'pipe' });
    return res.status === 0;
  } catch {
    return false;
  }
}

function getVersion(cmd: string, args: string[] = ["--version"]): string | null {
  try {
    const res = spawnSync(cmd, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
    if (res.status === 0) return String(res.stdout || res.stderr || '').trim().split(/\r?\n/)[0] || null;
    return null;
  } catch {
    return null;
  }
}

function parseSemver(s: string | null | undefined): [number, number, number] | null {
  if (!s) return null;
  const m = String(s).match(/(\d+)\.(\d+)\.(\d+)/);
  if (!m) return null;
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

function cmpSemver(a: [number, number, number], b: [number, number, number]): number {
  if (a[0] !== b[0]) return a[0] - b[0];
  if (a[1] !== b[1]) return a[1] - b[1];
  return a[2] - b[2];
}

function printEnvAssessment(repoRoot: string | null) {
  const header = chalk.bold("Environment assessment");
  const ok = (s: string) => chalk.green(`✔ ${s}`);
  const warn = (s: string) => chalk.yellow(`◔ ${s}`);
  const bad = (s: string) => chalk.red(`✘ ${s}`);

  const platform = `${process.platform} ${process.arch}`;
  const rust = hasCmd('rustc') && hasCmd('cargo');
  const rustcV = rust ? getVersion('rustc') : null;
  const cargoV = rust ? getVersion('cargo') : null;
  const git = hasCmd('git');
  const bun = hasCmd('bun');
  const bunx = hasCmd('bunx');
  const npx = hasCmd('npx');
  const codex = hasCmd('codex');
  const codexV = codex ? getVersion('codex', ['--version']) : null;
  const cloudflared = hasCmd('cloudflared');
  const cfVer = cloudflared ? getVersion('cloudflared', ['--version']) : null;

  console.log("");
  console.log(header);
  console.log("- " + ok(`Platform ${platform}`));
  console.log("- " + (repoRoot ? ok(`OpenAgents repo found at ${repoRoot}`) : warn("OpenAgents repo not found (will not be required in future)")));
  if (rust) {
    console.log("- " + ok(`Rust toolchain present (${rustcV || 'rustc'}; ${cargoV || 'cargo'})`));
  } else {
    console.log("- " + warn("Rust toolchain not found (required today to run the bridge + tunnels)"));
  }
  console.log("- " + (git ? ok("git present") : warn("git not found (needed to bootstrap the repo if not present)")));
  if (bun) {
    console.log("- " + ok("bun present"));
  } else if (npx) {
    console.log("- " + warn("bun not found; will fall back to npx for Convex CLI where possible"));
  } else {
    console.log("- " + warn("bun/npx missing; Convex CLI bootstrap may be skipped"));
  }
  if (codex) {
    const minStr = process.env.TRICODER_MIN_CODEX || '0.50.0';
    const have = codexV || '';
    const haveT = parseSemver(codexV);
    const minT = parseSemver(minStr);
    const cmp = haveT && minT ? cmpSemver(haveT, minT) : 0;
    if (!have) {
      console.log("- " + warn("codex present (version unknown)"));
    } else if (cmp < 0) {
      const sev = haveT && cmpSemver(haveT, [0,30,0]) < 0 ? chalk.red : chalk.yellow;
      console.log("- " + sev(`codex ${have} detected — recommended >= ${minStr}. Please upgrade: https://developers.openai.com/codex/cli`));
    } else {
      console.log("- " + ok(`codex ${have}`));
    }
  } else {
    console.log("- " + bad("codex binary NOT detected"));
    console.log(chalk.red("Install Codex CLI: https://developers.openai.com/codex/cli"));
    process.exit(1);
  }

  if (TUNNEL_MODE !== 'none') {
    if (cloudflared) {
      console.log("- " + ok(`cloudflared present ${cfVer ? `(${cfVer})` : ""}`));
    } else {
      console.log("- " + warn("cloudflared not found (required for Cloudflare Tunnel). Install: brew install cloudflared (mac) or see Cloudflare docs."));
    }
  }
  console.log("");
}

// ---------------------------------------------
// Main
// ---------------------------------------------
async function main() {
  console.info(chalk.bold("OpenAgents Tricoder - Desktop Bridge"));

  if (process.argv.includes("--delete")) {
    try { await destructiveReset(); } catch {}
    return;
  }
  let repoRoot = findRepoRoot(process.cwd());

  console.log(chalk.cyanBright("\nSetup overview"));
  console.log(lite(" - Checks your environment (Rust, git, Bun/NPM, codex)"));
  console.log(lite(" - Clones/updates the OpenAgents repo if missing (~/.openagents/openagents)"));
  console.log(lite(" - Builds the Rust bridge and starts it"));
  console.log(lite(" - Starts the local Convex backend and deploys functions (best effort)"));
  console.log(lite(" - Optionally opens Cloudflare Tunnels and prints a pairing code"));
  console.log(chalk.yellowBright("\nImportant: The mobile app and desktop features won’t fully work until the Convex backend is installed and healthy."));
  console.log(chalk.yellowBright("\nNote: First setup may take ~5 minutes on slower machines due to local Rust builds."));
  printResourcesOnce();
  printEnvAssessment(repoRoot);

  if (!repoRoot) {
    const home = os.homedir();
    const target = join(home, ".openagents", "openagents");
    console.log(chalk.yellow("No local OpenAgents repo detected — cloning to ~/.openagents/openagents…"));
    if (!hasCmd('git')) {
      console.log(chalk.red("git is required to clone the repository. Please install git and re-run."));
      process.exit(1);
    }
    try {
      spawnSync(process.platform === 'win32' ? 'cmd' : 'mkdir', process.platform === 'win32' ? ['/c', 'mkdir', target] : ['-p', target], { stdio: 'ignore' });
    } catch {}
    if (!existsSync(join(target, '.git'))) {
      const res = spawnSync('git', ['clone', '--depth', '1', 'https://github.com/OpenAgentsInc/openagents', target], { stdio: 'inherit' });
      if (res.status !== 0) {
        console.log(chalk.red("Failed to clone the OpenAgents repository."));
        process.exit(1);
      }
    } else {
      console.log(chalk.dim("Updating existing ~/.openagents/openagents…"));
      spawnSync('git', ['-C', target, 'pull', '--ff-only'], { stdio: VERBOSE ? 'inherit' : 'ignore' });
    }
    repoRoot = target;
  }

  try { ensureConvexBinaryWithProgress(); } catch { }
  ensureRustToolchain();
  prebuildCrates(repoRoot);
  ensureBridgeRunning(repoRoot);

  // ---------------------------------------------
  // Tunnel logic (Cloudflare)
  // ---------------------------------------------
  let bridgeUrl: string | null = null;
  let convexUrl: string | null = null;

  if (!LOCAL_ONLY) {
    if (!hasCmd('cloudflared')) {
      console.log(chalk.yellow("cloudflared not found; skipping tunnel. Install via `brew install cloudflared` or see Cloudflare docs."));
    } else {
      if (TUNNEL_MODE === 'named') {
        // Named tunnel: assume prior setup (credentials + DNS + ingress). We compute the URL directly.
        const wss = `wss://${TUNNEL_HOST}/ws`;
        bridgeUrl = wss;
        if (VERBOSE) console.log(chalk.dim(`[tunnel] Cloudflare named: ${wss} (using 'cloudflared tunnel run ${TUNNEL_NAME}')`));
        const child = spawn("cloudflared", ["tunnel", "run", TUNNEL_NAME], {
          stdio: VERBOSE ? "inherit" : ["ignore", "ignore", "ignore"],
        });
        child.on("error", (e) => { if (VERBOSE) console.log(chalk.dim(`[tunnel] error: ${e?.message || e}`)); });

        // Optional: also expose Convex via Quick Tunnel for demo (HTTP only)
        launchConvexQuickTunnel((url) => {
          convexUrl = url;
          if (VERBOSE) console.log(chalk.dim(`[convex-public] ${convexUrl}`));
          maybePrintPairCode(bridgeUrl!, convexUrl || undefined);
          if (VERBOSE) { try { if (bridgeUrl) probePublicBridge(bridgeUrl); } catch { } }
          if (VERBOSE) { try { if (convexUrl) probePublicConvex(convexUrl); } catch { } }
          if (VERBOSE) { try { seedDemoViaBridgeControl(); } catch { } }
          if (VERBOSE) startLocalProbes(repoRoot);
        });

        // If you don't want a convex tunnel, you can still print right away:
        setTimeout(() => {
          if (!convexUrl) {
            maybePrintPairCode(bridgeUrl!, undefined);
            if (VERBOSE) startLocalProbes(repoRoot);
          }
        }, 2000);

      } else if (TUNNEL_MODE === 'quick') {
        // Quick Tunnel for bridge (WS)
        startCloudflaredQuick("http://localhost:8787", (httpsPublic) => {
          const wss = httpsToWss(httpsPublic) + "/ws";
          bridgeUrl = wss;
          if (VERBOSE) console.log(chalk.dim(`[bridge-public] ${bridgeUrl}`));
          // Start convex quick tunnel
          launchConvexQuickTunnel((url) => {
            convexUrl = url;
            if (VERBOSE) console.log(chalk.dim(`[convex-public] ${convexUrl}`));
            maybePrintPairCode(bridgeUrl!, convexUrl || undefined);
            if (VERBOSE) { try { if (bridgeUrl) probePublicBridge(bridgeUrl); } catch { } }
            if (VERBOSE) { try { if (convexUrl) probePublicConvex(convexUrl); } catch { } }
            if (VERBOSE) console.log(chalk.dim(`[pair] bridge=${bridgeUrl} convex=${convexUrl}`));
            if (VERBOSE) { try { seedDemoViaBridgeControl(); } catch { } }
          });
          if (VERBOSE) startLocalProbes(repoRoot);
        });

      } else {
        if (VERBOSE) console.log(chalk.dim("[tunnel] mode=none; skipping Cloudflare Tunnel (local-only)"));
      }
    }
  } else {
    if (VERBOSE) console.log(chalk.dim("[tunnel] --local-only; skipping Cloudflare Tunnel"));
  }
}

// ---------------------------------------------
// Cloudflare helpers
// ---------------------------------------------
function httpsToWss(u: string): string {
  try {
    const url = new URL(u);
    url.protocol = (url.protocol === "http:") ? "ws:" : "wss:";
    return url.toString().replace(/\/+$/, ""); // trim trailing slash
  } catch {
    return u.replace(/^http:/, "ws:").replace(/^https:/, "wss:");
  }
}

function startCloudflaredQuick(localUrl: string, onHttpsUrl: (publicHttps: string) => void) {
  const child = spawn("cloudflared", ["tunnel", "--url", localUrl], {
    stdio: ["ignore", "pipe", "pipe"]
  });
  let printed = false;
  const scan = (s: string) => {
    // Look for a trycloudflare URL in stdout/stderr
    const m = s.match(/https?:\/\/[a-z0-9-]+\.trycloudflare\.com/);
    if (m && !printed) {
      printed = true;
      onHttpsUrl(m[0]);
    }
  };
  child.stdout?.setEncoding("utf8");
  child.stdout?.on("data", (d) => String(d).split(/\r?\n/).forEach(scan));
  child.stderr?.setEncoding("utf8");
  child.stderr?.on("data", (d) => String(d).split(/\r?\n/).forEach(scan));
  child.on("exit", (code) => {
    if (VERBOSE) console.log(chalk.dim(`[cloudflared] exited with code ${code ?? 0}`));
  });
  return child;
}

function launchConvexQuickTunnel(onUrl: (url: string) => void) {
  // Quick Tunnel for Convex (HTTP). We'll emit an HTTP base URL (not WS).
  startCloudflaredQuick("http://localhost:7788", (httpsUrl) => {
    // Convex API is HTTP; keep https URL as-is
    onUrl(httpsUrl);
  });
}

// ---------------------------------------------
// Existing functions (mostly unchanged)
// ---------------------------------------------
function ensureRustToolchain() {
  const rust = hasCmd('rustc') && hasCmd('cargo');
  if (rust) return;
  if (process.platform === 'win32') {
    console.log(chalk.yellow("Rust toolchain not found. Please install Rust from https://rustup.rs and re-run."));
    process.exit(1);
  }
  if (!ASSUME_YES) {
    const rl = require('node:readline').createInterface({ input: process.stdin, output: process.stdout });
    const ask = (q: string) => new Promise<string>(res => rl.question(q, (a: string) => res(a)));
    ask("Rust toolchain not found. Install rustup now? [y/N] ").then((ans: string) => {
      rl.close();
      if (!/^y(es)?$/i.test(ans.trim())) {
        console.log("Aborting; Rust is required.");
        process.exit(1);
      }
      installRustup();
    });
  } else {
    installRustup();
  }
}

function installRustup() {
  console.log(chalk.dim("Installing Rust via rustup (non-interactive)…"));
  const cmd = spawnSync('sh', ['-c', 'curl https://sh.rustup.rs -sSf | sh -s -- -y'], { stdio: 'inherit' });
  if (cmd.status !== 0) {
    console.log(chalk.red("Failed to install rustup. Please install from https://rustup.rs and re-run."));
    process.exit(1);
  }
  try {
    const cargoBin = join(os.homedir(), '.cargo', 'bin');
    process.env.PATH = `${cargoBin}:${process.env.PATH || ''}`;
  } catch {}
}

function prebuildCrates(repoRoot: string) {
  const env = { ...process.env } as Record<string, string>;
  if (process.platform === 'linux' && !env.CARGO_BUILD_JOBS) env.CARGO_BUILD_JOBS = '2';
  const run = (args: string[]) => spawnSync('cargo', args, { cwd: repoRoot, stdio: VERBOSE ? 'inherit' : 'ignore', env });
  console.log("");
  console.log(chalk.cyanBright("Checking/building required Rust crates (codex-bridge, oa-tunnel)…"));
  let r = run(['build', '-p', 'codex-bridge']);
  if (r.status !== 0) {
    console.log(chalk.yellow("cargo build -p codex-bridge failed; will rely on on-demand build during run."));
  }
  r = run(['build', '-p', 'oa-tunnel']);
  if (r.status !== 0) {
    console.log(chalk.yellow("cargo build -p oa-tunnel failed; will rely on on-demand build during run."));
  }
}

function ensureConvexBinaryWithProgress() {
  const home = os.homedir();
  const outDir = join(home, ".openagents", "bin");
  const outBin = join(outDir, process.platform === 'win32' ? 'local_backend.exe' : 'local_backend');
  if (existsSync(outBin)) return;
  const haveBunx = hasCmd('bunx');
  const haveNpx = hasCmd('npx');
  console.log("");
  console.log(chalk.cyanBright("Downloading Convex local backend (first-time only)…"));
  directDownloadConvexBackend(outBin).then((ok) => {
    if (ok) return;
    if (!haveBunx && !haveNpx) return;
    let lastPct = -1;
    const args = ["convex", "dev", "--configure", "--dev-deployment", "local", "--once", "--skip-push", "--local-force-upgrade"];
    const env = { ...process.env, CI: '1' } as Record<string,string>;
    const child = haveBunx
      ? spawn("bunx", args, { stdio: ["ignore", "pipe", "pipe"], env })
      : spawn("npx", ["-y", ...args], { stdio: ["ignore", "pipe", "pipe"], env });
    const show = (p: number) => {
      if (p < 0 || p > 100) return;
      if (p <= lastPct) return;
      lastPct = p; CONVEX_DL_PCT = p;
      try { process.stdout.write("\r" + chalk.cyanBright(`⬇️  Convex backend download: ${p}%`)); } catch {}
    };
    const maybeParse = (s: string) => {
      const m = s.match(/(\d{1,3})%/);
      if (m) { const pct = Math.max(0, Math.min(100, parseInt(m[1], 10))); show(pct); }
    };
    child.stdout?.setEncoding('utf8'); child.stdout?.on('data', (d) => { String(d).split(/\r?\n/).forEach(maybeParse); });
    child.stderr?.setEncoding('utf8'); child.stderr?.on('data', (d) => { String(d).split(/\r?\n/).forEach(maybeParse); });
    const done = () => {
      try { process.stdout.write("\r\x1b[K"); } catch {}
      const cacheRoot = join(home, '.cache', 'convex', 'binaries');
      const candidate = findNewestBackendBinary(cacheRoot);
      if (candidate) {
        try { mkdirSync(outDir, { recursive: true }); copyFileSync(candidate, outBin); try { chmodSync(outBin, 0o755); } catch {}; console.log(chalk.greenBright("✔ Convex backend installed.")); } catch (e: any) { console.log(chalk.yellow(`Convex backend cached but copy failed: ${e?.message || e}`)); }
      } else {
        console.log(chalk.yellow("Convex CLI finished but backend binary not found in cache (will let the bridge retry)."));
      }
    };
    child.on('exit', () => done());
    setTimeout(() => { try { child.kill(); } catch {} }, 180000);
  }).catch(() => { /* ignore; bridge will retry */ });
}

function findNewestBackendBinary(root: string): string | null {
  try {
    const entries = readdirSync(root, { encoding: 'utf8' }) as unknown as string[];
    let best: { path: string, mtime: number } | null = null;
    for (const dir of entries) {
      const d = join(root, dir);
      try {
        const files = readdirSync(d, { encoding: 'utf8' }) as unknown as string[];
        for (const f of files) {
          if (!/local_backend(\.exe)?$/.test(f) && !/convex-local-backend(\.exe)?$/.test(f)) continue;
          const p = join(d, f);
          const st = statSync(p);
          if (!best || st.mtimeMs > best.mtime) best = { path: p, mtime: st.mtimeMs };
        }
      } catch { }
    }
    return best?.path || null;
  } catch { return null; }
}

async function directDownloadConvexBackend(outBin: string): Promise<boolean> {
  try {
    const triple = (() => {
      switch (process.platform) {
        case 'darwin':
          return process.arch === 'arm64' ? 'aarch64-apple-darwin' : (process.arch === 'x64' ? 'x86_64-apple-darwin' : null);
        case 'linux':
          return process.arch === 'arm64' ? 'aarch64-unknown-linux-gnu' : (process.arch === 'x64' ? 'x86_64-unknown-linux-gnu' : null);
        case 'win32':
          return 'x86_64-pc-windows-msvc';
        default: return null;
      }
    })();
    if (!triple) return false;
    const filename = `convex-local-backend-${triple}.zip`;
    const version = await findLatestConvexVersionWithBinary(filename);
    if (!version) return false;
    const url = `https://github.com/get-convex/convex-backend/releases/download/${version}/${filename}`;
    const tmpZip = join(os.tmpdir(), `convex-${Date.now()}.zip`);
    await downloadWithProgress(url, tmpZip, (pct) => { CONVEX_DL_PCT = pct; try { process.stdout.write("\r" + chalk.cyanBright(`⬇️  Convex backend download: ${pct}%`)); } catch {} });
    const zip = new AdmZip(tmpZip);
    const entries = zip.getEntries();
    const entry = entries.find(e => /convex-local-backend(\.exe)?$/.test(e.entryName));
    if (!entry) return false;
    const outDir = dirname(outBin); mkdirSync(outDir, { recursive: true });
    zip.extractEntryTo(entry, outDir, false, true);
    const extractedPath = join(outDir, entry.entryName);
    if (extractedPath !== outBin) {
      try { copyFileSync(extractedPath, outBin); } catch {}
    }
    try { chmodSync(outBin, 0o755); } catch {}
    try { process.stdout.write("\r\x1b[K"); } catch {}
    console.log(chalk.greenBright("✔ Convex backend installed."));
    try { await tryStartConvexBackendIfNeeded(outBin); } catch {}
    return true;
  } catch {
    return false;
  }
}

async function findLatestConvexVersionWithBinary(filename: string): Promise<string | null> {
  let nextUrl: string | '' = 'https://api.github.com/repos/get-convex/convex-backend/releases?per_page=30';
  while (nextUrl) {
    const res = await fetch(nextUrl as any, { headers: { 'User-Agent': 'openagents-tricoder' } } as any);
    if (!res.ok) return null;
    const releases = await res.json() as any[];
    for (const r of releases) {
      if (r.prerelease || r.draft) continue;
      const assets = Array.isArray(r.assets) ? r.assets : [];
      if (assets.find((a: any) => a.name === filename)) {
        return r.tag_name as string;
      }
    }
    const link = res.headers.get('Link') || res.headers.get('link');
    if (!link) break;
    const m = /<([^>]+)>;\s*rel="next"/.exec(link);
    nextUrl = m ? (m[1] as string) : '';
  }
  return null;
}

async function downloadWithProgress(url: string, dest: string, onPct: (pct: number) => void): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const req = https.get(url, { headers: { 'User-Agent': 'openagents-tricoder' } }, (res) => {
      if (res.statusCode && res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        downloadWithProgress(res.headers.location, dest, onPct).then(resolve, reject); return;
      }
      if (!res.statusCode || res.statusCode < 200 || res.statusCode >= 300) { reject(new Error(`HTTP ${res.statusCode}`)); return; }
      const total = Number(res.headers['content-length'] || 0);
      let done = 0;
      const out = createWriteStream(dest);
      res.on('data', (chunk) => { done += chunk.length; if (total > 0) { const pct = Math.min(100, Math.max(0, Math.round((done / total) * 100))); onPct(pct); } });
      res.pipe(out);
      out.on('finish', () => { try { out.close(); } catch {}; onPct(100); resolve(); });
      res.on('error', (e) => { try { out.destroy(); } catch {}; reject(e); });
    });
    req.on('error', reject);
  });
}

async function tryStartConvexBackendIfNeeded(binPath: string): Promise<void> {
  const healthy = await new Promise<boolean>((resolve) => {
    const req = http.get({ host: '127.0.0.1', port: 7788, path: '/instance_version', timeout: 1200 }, (res) => {
      resolve(!!res.statusCode && res.statusCode >= 200 && res.statusCode < 300);
      res.resume();
    });
    req.on('error', () => resolve(false));
  });
  if (healthy) return;
  const db = join(os.homedir(), '.openagents', 'convex', 'data.sqlite3');
  const storage = join(os.homedir(), '.openagents', 'convex', 'storage');
  try { mkdirSync(join(os.homedir(), '.openagents', 'convex'), { recursive: true }); } catch {}
  const args = [
    db,
    '--db', 'sqlite',
    '--interface', '0.0.0.0',
    '--port', '7788',
    '--local-storage', storage,
    '--disable-beacon',
  ];
  try { spawn(binPath, args, { stdio: 'ignore' }); } catch {}
}

function maybePrintPairCode(bridgeWss: string, convexHttp?: string) {
  // Print once when we have the bridge URL; convex is optional
  if (!bridgeWss) return;
  if ((maybePrintPairCode as any)._printed) return;
  (maybePrintPairCode as any)._printed = true;

  const payload: any = {
    v: 1,
    type: "openagents-bridge",
    provider: "cloudflare",
    bridge: bridgeWss,
    token: readBridgeToken() as string | null,
  };
  if (convexHttp) payload.convex = convexHttp;

  const code = encodePairCode(payload);
  const deeplink = `openagents://connect?j=${code}`;

  console.log("\nPaste this code into the mobile app Settings → Bridge Code, or scan the QR below:\n");
  console.log(chalk.greenBright(code));
  console.log("");

  if (!NO_QR) {
    const qrPayload = (QR_MODE === 'deeplink') ? deeplink : code;
    const wantImg = shouldUseInlineImage();
    if (wantImg) {
      QR.toBuffer(qrPayload, { type: 'png', errorCorrectionLevel: 'L', margin: 1, scale: 6 })
        .then((buf: Buffer) => {
          try { printItermInlineImage(buf, 'qr.png'); }
          catch { printBrailleQR(qrPayload); }
          console.log('');
        })
        .catch(() => {
          try { printBrailleQR(qrPayload); }
          catch {
            try { (qrcode as any).setErrorLevel?.('L'); } catch {}
            qrcode.generate(qrPayload, { small: true });
          }
          console.log('');
        });
    } else {
      try { printBrailleQR(qrPayload, 2) } catch { try { (qrcode as any).setErrorLevel?.('L') } catch {}; qrcode.generate(qrPayload, { small: true }) }
      console.log('');
    }
  }

  console.log(chalk.yellowBright("\nWarning: This code is your private bridge token — never share it with anyone."));
  console.log("\nTunnel is active. Leave this running to stay connected.\n");
  console.log(chalk.yellowBright("Heads up: The app depends on the local Convex backend. If you don’t see threads/messages updating yet, wait until Convex reports healthy and functions finish deploying."));
  printResourcesOnce();

  try {
    const probe = spawnSync(process.platform === 'win32' ? 'where' : 'which', ['codex'], { stdio: 'pipe' });
    const ok = probe.status === 0;
    if (VERBOSE) console.log(ok ? chalk.dim('[codex] codex binary found in PATH') : chalk.yellow('[codex] codex binary NOT found — assistant responses will not stream'));
    if (VERBOSE) { try { bridgeStatus(); } catch { } }
    if (VERBOSE) { try { startBridgeEventTail(); } catch { } }
  } catch { }

  if (!VERBOSE) {
    try { monitorConvexSetupOnce(findRepoRoot(process.cwd()) as string) } catch { }
  }
}

function probePublicConvex(base: string) {
  try {
    const u = new URL(base);
    const opts: http.RequestOptions = { host: u.hostname, port: Number(u.port || 443), path: "/instance_version", timeout: 2000, };
    const req = https.get({ ...opts, protocol: 'https:' }, (res) => {
      const code = res.statusCode || 0;
      let body = "";
      res.setEncoding("utf8");
      res.on("data", (c) => (body += c));
      res.on("end", () => {
        const snippet = body.trim().slice(0, 80).replace(/\s+/g, " ");
        if (VERBOSE) console.log(chalk.dim(`[convex-public-check] GET ${u.hostname}${u.port ? ':' + u.port : ''}/instance_version -> ${code} ${snippet ? `body: ${snippet}` : ""}`));
      });
    });
    req.on("error", (e: any) => { if (VERBOSE) console.log(chalk.dim(`[convex-public-check] error: ${e?.message || e}`)); });
    req.setTimeout(2500, () => { try { req.destroy(); } catch { } });
  } catch (e: any) {
    if (VERBOSE) console.log(chalk.dim(`[convex-public-check] invalid URL: ${String(e?.message || e)}`));
  }
}

function probePublicBridge(wsUrl: string) {
  try {
    const u = new URL(wsUrl.replace(/^ws:/, "http:").replace(/^wss:/, "https:"));
    const host = u.hostname;
    const port = Number(u.port || 443);
    const path = u.pathname || "/ws";
    const key = Buffer.from(Math.random().toString()).toString("base64");
    const headers =
      `GET ${path} HTTP/1.1\r\n` +
      `Host: ${host}\r\n` +
      `Upgrade: websocket\r\n` +
      `Connection: Upgrade\r\n` +
      `Sec-WebSocket-Key: ${key}\r\n` +
      `Sec-WebSocket-Version: 13\r\n` +
      `\r\n`;
    const s = net.createConnection({ host, port }, () => {
      s.write(headers);
    });
    let buf = "";
    s.on("data", (chunk) => {
      buf += String(chunk);
      if (buf.includes("\r\n\r\n")) {
        const first = buf.split(/\r?\n/)[0] || "";
        if (VERBOSE) console.log(chalk.dim(`[bridge-public-check] ${first}`));
        try { s.destroy(); } catch { }
      }
    });
    s.on("error", (e: any) => { if (VERBOSE) console.log(chalk.dim(`[bridge-public-check] error: ${e?.message || e}`)); });
    setTimeout(() => { try { s.destroy(); } catch { } }, 2500);
  } catch (e: any) {
    if (VERBOSE) console.log(chalk.dim(`[bridge-public-check] invalid URL: ${String(e?.message || e)}`));
  }
}

function seedDemoViaBridgeControl() {
  const ws = new WebSocket("ws://127.0.0.1:8787/ws");
  let done = false;
  const timer = setTimeout(() => { try { ws.close(); } catch { } }, 4000);
  ws.on("open", () => {
    try {
      ws.send(JSON.stringify({ control: "convex.create_demo" }));
      ws.send(JSON.stringify({ control: "convex.create_demo_thread" }));
      ws.send(JSON.stringify({ control: "convex.create_threads" }));
      ws.send(JSON.stringify({ control: "convex.status" }));
    } catch { }
  });
  ws.on("message", (data: WebSocket.RawData) => {
    const s = String(data || "").trim();
    if (!s.startsWith("{")) return;
    try {
      const obj = JSON.parse(s);
      if (obj?.type === "bridge.convex_status") {
        const url = (obj.url || obj.convex_url || "") as string;
        if (VERBOSE) console.log(chalk.dim(`[bridge-control] convex.status -> ${obj.healthy ? "healthy" : "unhealthy"} url=${url}`));
      }
      if (obj?.type === "bridge.status") {
        if (VERBOSE) console.log(chalk.dim(`[bridge-status] bind=${obj.bind} convex_healthy=${obj.convex_healthy} codex_pid=${obj.codex_pid || 'none'}`));
      }
      if (obj?.type === "bridge.projects") {
        if (VERBOSE) console.log(chalk.dim(`[bridge-control] projects -> ${Array.isArray(obj.items) ? obj.items.length : 0} items`));
      }
    } catch { }
  });
  ws.on("close", () => { if (!done) clearTimeout(timer); done = true; });
  ws.on("error", () => { if (!done) clearTimeout(timer); done = true; });
}

function bridgeStatus() {
  const ws = new WebSocket("ws://127.0.0.1:8787/ws");
  const timer = setTimeout(() => { try { ws.close(); } catch { } }, 1800);
  ws.on("open", () => {
    try { ws.send(JSON.stringify({ control: 'bridge.status' })); } catch { }
  });
  ws.on("error", () => { try { clearTimeout(timer); ws.close(); } catch { } });
  ws.on("close", () => { try { clearTimeout(timer); } catch { } });
}

function startBridgeEventTail() {
  let closed = false;
  const connect = () => {
    if (closed) return;
    const ws = new WebSocket("ws://127.0.0.1:8787/ws");
    ws.on("open", () => { /* no-op */ });
    ws.on("message", (data: WebSocket.RawData) => {
      const s = String(data || "").trim();
      if (!s) return;
      if (s.startsWith("{")) {
        try {
          const obj = JSON.parse(s);
          const t = obj?.type || obj?.msg?.type || '';
          if (VERBOSE && String(t).startsWith('bridge.')) {
            if (t === 'bridge.control' && typeof obj.raw === 'string') {
              console.log(chalk.dim(`[bridge-control] ${obj.raw}`)); return;
            }
            if (t === 'bridge.convex_write') {
              const op = obj.op || '';
              const ok = obj.ok ? 'ok' : 'fail';
              const kind = obj.kind || '';
              const len = obj.len || 0;
              const itemId = obj.itemId || '';
              console.log(chalk.dim(`[convex-write] ${op} ${ok} kind=${kind} item=${itemId} len=${len}`)); return;
            }
            if (t === 'bridge.ws_in' && typeof obj.preview === 'string') {
              console.log(chalk.dim(`[bridge-in] ${obj.preview}`)); return;
            }
            if (t === 'bridge.run_submit') {
              console.log(chalk.dim(`[bridge] bridge.run_submit threadDocId=${obj.threadDocId || ''} len=${obj.len || 0}`)); return;
            }
            if (t === 'bridge.client_connected' || t === 'bridge.client_disconnected') {
              console.log(chalk.dim(`[bridge] ${t}`)); return;
            }
            if (t === 'bridge.echo') {
              console.log(chalk.dim(`[bridge-echo] tag=${obj.tag || ''} payload=${obj.payload || ''}`)); return;
            }
            console.log(chalk.dim(`[bridge] ${t}`)); return;
          }
          if (VERBOSE && t && /agent_message|assistant|message|reason|exec_begin|exec/.test(String(t))) {
            console.log(chalk.dim(`[codex] ${t}`)); return;
          }
          if (VERBOSE && t === 'bridge.codex_raw' && typeof obj.line === 'string') {
            console.log(chalk.dim(`[codex-raw] ${obj.line}`)); return;
          }
        } catch { /* ignore non-json */ }
      }
    });
    ws.on("close", () => { if (!closed) setTimeout(connect, 1200); });
    ws.on("error", () => { try { ws.close(); } catch { }; if (!closed) setTimeout(connect, 1500); });
  };
  connect();
  return () => { closed = true; };
}

function ensureBridgeRunning(repoRoot: string) {
  const sock = net.createConnection({ host: "127.0.0.1", port: 8787 });
  let connected = false;
  sock.once("connect", async () => {
    connected = true; try { sock.end(); } catch { }
    const force = (process.env.TRICODER_FORCE_RESTART || '1') !== '0';
    const supports = await probeBridgeEchoOnce(700).catch(() => false);
    if (force || !supports) {
      if (VERBOSE) console.log(chalk.dim(`Restarting local bridge with debug enabled (${force ? 'forced' : 'no echo support'})…`));
      try { restartBridgeProcess(repoRoot); } catch { }
    }
  });
  sock.once("error", () => {
    startBridgeProcess(repoRoot);
  });
  setTimeout(() => { try { if (!connected) sock.destroy(); } catch { } }, 500);
}

function startBridgeProcess(repoRoot: string) {
  if (VERBOSE) console.log(chalk.dim("Starting local bridge (cargo run -p codex-bridge)…"));
  const child = spawn("cargo", ["run", "-p", "codex-bridge", "--", "--bind", "0.0.0.0:8787"], {
    cwd: repoRoot,
    stdio: VERBOSE ? "inherit" : ["ignore", "ignore", "ignore"],
    env: {
      ...process.env,
      RUST_LOG: process.env.RUST_LOG || (VERBOSE ? "info,convex=warn,convex::base_client=warn,tungstenite=warn,notify=warn" : "warn"),
      OPENAGENTS_CONVEX_SYNC: process.env.OPENAGENTS_CONVEX_SYNC || "0",
      BRIDGE_DEBUG_WS: process.env.BRIDGE_DEBUG_WS || (VERBOSE ? "1" : "0"),
      BRIDGE_DEBUG_CODEX: process.env.BRIDGE_DEBUG_CODEX || (VERBOSE ? "1" : "0"),
    },
  });
  child.on("error", () => { });
}

function restartBridgeProcess(repoRoot: string) {
  try {
    const out = spawnSync(process.platform === 'darwin' || process.platform === 'linux' ? 'lsof' : 'netstat',
      process.platform === 'darwin' || process.platform === 'linux'
        ? ['-i', ':8787', '-sTCP:LISTEN', '-t']
        : [], { encoding: 'utf8' })
    const pids = String(out.stdout || '').split(/\s+/).filter(Boolean)
    for (const pid of pids) {
      try { process.kill(Number(pid), 'SIGTERM') } catch { }
    }
  } catch { }
  setTimeout(() => startBridgeProcess(repoRoot), 400);
}

async function probeBridgeEchoOnce(timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    const ws = new WebSocket("ws://127.0.0.1:8787/ws");
    let done = false;
    const timer = setTimeout(() => { if (!done) { done = true; try { ws.close(); } catch { }; resolve(false); } }, timeoutMs);
    ws.on("open", () => { try { ws.send(JSON.stringify({ control: 'echo', tag: 'probe', payload: 'ok' })) } catch { } });
    ws.on("message", (data) => {
      const s = String(data || '').trim();
      if (!s.startsWith('{')) return;
      try {
        const obj = JSON.parse(s);
        if (obj?.type === 'bridge.echo' && (obj?.tag === 'probe')) { if (!done) { done = true; clearTimeout(timer); try { ws.close(); } catch { }; resolve(true); } }
      } catch { }
    });
    ws.on("error", () => { if (!done) { done = true; clearTimeout(timer); resolve(false); } });
    ws.on("close", () => { if (!done) { done = true; clearTimeout(timer); resolve(false); } });
  });
}

main().catch((e) => {
  console.error(chalk.red(String(e?.stack || e)));
  process.exit(1);
});

// ---------------------------------------------
// Destructive reset + misc helpers (unchanged)
// ---------------------------------------------
function destructiveReset(): Promise<void> { /* ... unchanged from your original ... */
  const home = os.homedir();
  const paths: Array<{ path: string; desc: string }> = [
    { path: join(home, ".openagents", "openagents"), desc: "OpenAgents repo clone" },
    { path: join(home, ".openagents", "bin", process.platform === 'win32' ? 'local_backend.exe' : 'local_backend'), desc: "Convex local backend binary" },
    { path: join(home, ".openagents", "convex"), desc: "Convex local data + storage" },
  ];
  console.log(chalk.yellow("\nDanger: This will delete local OpenAgents clones, the Convex local backend binary, and local Convex data."));
  try { killListeners(8787) } catch {}
  try { killListeners(7788) } catch {}
  if (!ASSUME_YES) {
    return promptYesNoTTY("Proceed with full reset? [y/N] ").then((ans) => {
      if (!ans) { console.log("Aborted."); try { process.stdin.pause(); } catch {}; process.exit(0); return; }
      runDelete(paths);
    });
  } else {
    runDelete(paths);
    return Promise.resolve();
  }
}

function promptYesNoTTY(question: string): Promise<boolean> { /* unchanged */
  return new Promise<boolean>((resolve) => {
    const rl = tryOpenTTYReadline();
    if (rl) {
      const anyRl = rl as any;
      rl.question(question, (ans: string) => {
        try { rl.close(); } catch {}
        try { anyRl.__ttyIn?.destroy?.(); anyRl.__ttyOut?.end?.(); anyRl.__ttyOut?.destroy?.(); } catch {}
        resolve(/^y(es)?$/i.test(String(ans || '').trim()));
      });
      return;
    }
    try { process.stdout.write(question); } catch {}
    try { process.stdin.setEncoding('utf8'); } catch {}
    try { (process.stdin as any).resume?.(); } catch {}
    const onData = (buf: Buffer | string) => {
      const s = String(buf || '').trim();
      try { process.stdin.pause(); } catch {}
      resolve(/^y(es)?$/i.test(s));
    };
    process.stdin.once('data', onData);
  });
}

function tryOpenTTYReadline(): any | null { /* unchanged */
  try {
    const fs = require('node:fs');
    const readline = require('node:readline');
    if (process.platform !== 'win32') {
      try {
        const ttyIn = fs.createReadStream('/dev/tty');
        const ttyOut = fs.createWriteStream('/dev/tty');
        const rl = readline.createInterface({ input: ttyIn, output: ttyOut });
        (rl as any).__ttyIn = ttyIn; (rl as any).__ttyOut = ttyOut;
        return rl;
      } catch { /* ignore */ }
    }
    if (process.stdin && process.stdout) {
      return readline.createInterface({ input: process.stdin, output: process.stdout });
    }
    return null;
  } catch {
    return null;
  }
}

function runDelete(paths: Array<{ path: string; desc: string }>) { /* unchanged */
  for (const p of paths) {
    try {
      if (!existsSync(p.path)) { if (VERBOSE) console.log(chalk.dim(`[delete] skip (missing): ${p.path}`)); continue; }
      const st = statSync(p.path);
      if (st.isDirectory()) {
        rmrf(p.path);
      } else {
        try { require('node:fs').unlinkSync(p.path); } catch {}
      }
      console.log(chalk.green(`✔ Deleted ${p.desc}: ${p.path}`));
    } catch (e: any) {
      console.log(chalk.yellow(`⚠ Failed to delete ${p.path}: ${e?.message || e}`));
    }
  }
  console.log(chalk.greenBright("Done. You can re-run `npx tricoder` for a fresh setup."));
  try { process.stdin.pause(); } catch {}
  process.exit(0);
}

function killListeners(port: number) { /* unchanged */
  try {
    if (process.platform === 'darwin' || process.platform === 'linux') {
      const out = spawnSync('lsof', ['-i', `:${port}`, '-sTCP:LISTEN', '-t'], { encoding: 'utf8' });
      const pids = String(out.stdout || '').split(/\s+/).filter(Boolean);
      for (const pid of pids) {
        try { process.kill(Number(pid), 'SIGTERM'); if (VERBOSE) console.log(chalk.dim(`[delete] killed pid ${pid} on :${port}`)) } catch {}
      }
    }
  } catch {}
}

function rmrf(target: string) { /* unchanged */
  try {
    const fs = require('node:fs');
    const path = require('node:path');
    const entries = fs.readdirSync(target, { withFileTypes: true });
    for (const entry of entries) {
      const full = path.join(target, entry.name);
      if (entry.isDirectory()) rmrf(full); else { try { fs.unlinkSync(full) } catch {} }
    }
    try { fs.rmdirSync(target) } catch {}
  } catch {}
}

function readBridgeToken(): string | null {
  try {
    const envTok = String(process.env.OPENAGENTS_BRIDGE_TOKEN || '').trim();
    if (envTok) return envTok;
  } catch {}
  try {
    const home = os.homedir();
    const p = join(home, '.openagents', 'bridge.json');
    if (!fs.existsSync(p)) return null;
    const raw = fs.readFileSync(p, 'utf8');
    const obj = JSON.parse(raw);
    const t = obj?.token;
    if (typeof t === 'string' && t.length > 0) return t;
  } catch {}
  return null;
}

// QR helpers (unchanged)
function printBrailleQR(text: string, scaleCells: number = 1) { /* unchanged */
  const qr: any = (QR as any).create(String(text || ''), { errorCorrectionLevel: 'L' });
  const mods = qr.modules || {};
  const size: number = mods.size || (Array.isArray(mods.data) ? (Array.isArray(mods.data[0]) ? mods.data.length : Math.sqrt(mods.data.length)) : 0);
  if (!size) throw new Error('qr size unknown');
  const at = (x: number, y: number): boolean => {
    if (x < 0 || y < 0 || x >= size || y >= size) return false;
    if (Array.isArray(mods.data?.[0])) return !!mods.data[y][x];
    return !!mods.data[y * size + x];
  };
  const margin = 1;
  let out = '';
  for (let y = -margin; y < size + margin; y += 4) {
    let line = '';
    for (let x = -margin; x < size + margin; x += 2) {
      let bits = 0;
      if (at(x + 0, y + 0)) bits |= 1 << 0;
      if (at(x + 0, y + 1)) bits |= 1 << 1;
      if (at(x + 0, y + 2)) bits |= 1 << 2;
      if (at(x + 0, y + 3)) bits |= 1 << 6;
      if (at(x + 1, y + 0)) bits |= 1 << 3;
      if (at(x + 1, y + 1)) bits |= 1 << 4;
      if (at(x + 1, y + 2)) bits |= 1 << 5;
      if (at(x + 1, y + 3)) bits |= 1 << 7;
      const ch = bits ? String.fromCodePoint(0x2800 + bits) : ' ';
      const hrep = Math.max(1, scaleCells);
      for (let k = 0; k < hrep; k++) line += ch;
    }
    const vrep = Math.max(1, scaleCells);
    for (let k = 0; k < vrep; k++) out += line + '\n';
  }
  console.log(out.trimEnd());
}

function shouldUseInlineImage(): boolean {
  const force = String(process.env.TRICODER_QR_IMAGE || '').trim() === '1';
  if (force) return true;
  const prog = String(process.env.TERM_PROGRAM || '').toLowerCase();
  if (prog.includes('iterm')) return true;
  if (prog.includes('wezterm')) return true;
  return false;
}

function printItermInlineImage(buf: Buffer, name: string = 'image.png') {
  const b64Name = Buffer.from(name).toString('base64');
  const b64 = buf.toString('base64');
  const esc = `\u001b]1337;File=name=${b64Name};inline=1;width=auto;height=auto;preserveAspectRatio=1:${b64}\u0007`;
  console.log(esc);
}
```

**What changed (high-level):**

* Replaced Bore with **Cloudflare Tunnel**:

  * Default `--tunnel=named` → assumes `tunnel.openagents.com` bound to your named tunnel (`TRICODER_TUNNEL_HOST`/`--tunnel-host` to override; `TRICODER_TUNNEL_NAME`/`--tunnel-name` for the `cloudflared tunnel run <name>`).
  * `--tunnel=quick` → automatic **Quick Tunnel** for the bridge and Convex (parses `trycloudflare.com`).
  * `--tunnel=none` or `--local-only` → skip tunnels.
* **Pair code** now uses `provider: "cloudflare"` and prints as soon as the **bridge** URL is known; Convex URL is optional and included when available.
* Added presence checks & env report for `cloudflared`.
* Kept your Convex bootstrap, bridge start, probes, and QR logic intact.

> Convex is not optional. User must have it installed so simplify to remove the optionality. And clarify if our same QR code flow works with the Cloudflare Tunnel setup


Absolutely—two changes:

1. **Convex is now mandatory.** The script will install/start it or exit with a clear error. No silent fallback/skip paths.
2. **QR flow works unchanged with Cloudflare Tunnel.** We keep the exact same deep-link + base64 payload. The only change is the `provider` field is set to `"cloudflared"` (vs `"bore"`). The app reads the `bridge` and `convex` URLs from the QR payload—so as long as your named tunnel maps those hostnames, pairing works the same.

Below are **surgical patches** you can apply to your `npx tricoder` script.

---

## PATCH 1 — Imports & Cloudflare Tunnel host config

**Remove** the bore args import and **add** tunnel host envs and provider:

```diff
- import { buildTunnelArgs } from "./args.js"
+ // Cloudflare Tunnel (named) hosts – override via env if needed
+ const TUNNEL_HOST = process.env.TRICODER_TUNNEL_HOST || "tunnel.openagents.com";
+ const defaultConvexHost = TUNNEL_HOST.startsWith("tunnel.")
+   ? TUNNEL_HOST.replace(/^tunnel\./, "convex.")
+   : "convex.openagents.com";
+ const CONVEX_HOST = process.env.TRICODER_CONVEX_HOST || defaultConvexHost;
+ const TUNNEL_PROVIDER = "cloudflared"; // used in QR payload
```

Optional (recommended): ensure `cloudflared` exists when not `--local-only`:

```diff
 const LOCAL_ONLY = process.argv.includes("--local-only") || process.env.TRICODER_LOCAL_ONLY === "1";
+if (!LOCAL_ONLY && !hasCmd("cloudflared")) {
+  console.error(chalk.red("cloudflared is required to run the named Cloudflare Tunnel."));
+  console.error(lite("Install: https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/installation"));
+  process.exit(1);
+}
```

---

## PATCH 2 — Environment assessment: Convex is REQUIRED

Make Bun or NPX a **hard requirement** (so we can bootstrap Convex CLI if direct download fails):

```diff
-  if (bun) {
-    console.log("- " + ok("bun present"));
-  } else if (npx) {
-    console.log("- " + warn("bun not found; will fall back to npx for Convex CLI where possible"));
-  } else {
-    console.log("- " + warn("bun/npx missing; Convex CLI bootstrap may be skipped"));
-  }
+  if (bun || npx) {
+    console.log("- " + ok(`${bun ? "bun" : "npx"} present (Convex install ready)`));
+  } else {
+    console.log("- " + bad("Neither bun nor npx found — required to install Convex CLI if direct download fails."));
+    process.exit(1);
+  }
```

Also tweak the overview text to remove optional language:

```diff
-  console.log(lite(" - Starts the local Convex backend and deploys functions (best effort)"));
+  console.log(lite(" - Installs & starts the local Convex backend (REQUIRED) and deploys functions"));
```

---

## PATCH 3 — Replace bore with **Cloudflare Tunnel (named)**

**In `main()`**, replace the bore tunnel launch + convex tunnel chain with a single named Cloudflare Tunnel and **predetermined URLs** (no stdout parsing):

```diff
-  // Launch Bridge tunnel (local 8787)
-  const child = LOCAL_ONLY ? null : spawn("cargo", buildTunnelArgs(8787, "bore.pub"), {
-    cwd: repoRoot,
-    stdio: ["ignore", "pipe", "pipe"],
-  });
-
-  let bridgeUrl: string | null = null;
-  let convexUrl: string | null = null;
-  let printedCombined = false;
-  child?.stdout?.setEncoding("utf8");
-  child?.stdout?.on("data", (chunk: string) => {
-    const lines = chunk.split(/\r?\n/).filter(Boolean);
-    for (const line of lines) {
-      const isUrl = line.startsWith("ws://") || line.startsWith("wss://");
-      if (isUrl && !bridgeUrl) {
-        bridgeUrl = line.trim();
-        if (VERBOSE) console.log(chalk.dim(`[bridge-public] ${bridgeUrl}`));
-        // After bridge URL, launch Convex tunnel
-        if (!LOCAL_ONLY) launchConvexTunnel(repoRoot, (url) => {
-          convexUrl = url;
-          if (VERBOSE) console.log(chalk.dim(`[convex-public] ${convexUrl}`));
-          maybePrintPairCode(bridgeUrl!, convexUrl!);
-          // Public probes
-          if (VERBOSE) { try { if (bridgeUrl) probePublicBridge(bridgeUrl); } catch { } }
-          if (VERBOSE) { try { if (convexUrl) probePublicConvex(convexUrl); } catch { } }
-          // Connectivity summary
-          if (VERBOSE) console.log(chalk.dim(`[pair] bridge=${bridgeUrl} convex=${convexUrl}`));
-          // Seed a demo thread via bridge controls to ensure history appears
-          if (VERBOSE) { try { seedDemoViaBridgeControl(); } catch { } }
-        });
-        // Start local health probes (status changes only)
-        if (VERBOSE) startLocalProbes(repoRoot);
-      }
-    }
-  });
+  // Public endpoints (named Cloudflare Tunnel) or local-only
+  let bridgeUrl: string | null = null;
+  let convexUrl: string | null = null;
+  let printedCombined = false;
+
+  if (LOCAL_ONLY) {
+    bridgeUrl = "ws://127.0.0.1:8787/ws";
+    convexUrl = "http://127.0.0.1:7788";
+    maybePrintPairCode(bridgeUrl, convexUrl);
+    if (VERBOSE) startLocalProbes(repoRoot);
+  } else {
+    // Run the named tunnel (assumes `cloudflared tunnel create tricoder-bridge` + config done)
+    const tunnelName = process.env.CF_TUNNEL_NAME || "tricoder-bridge";
+    const tunnelProc = spawn("cloudflared", ["tunnel", "run", tunnelName], {
+      stdio: VERBOSE ? "inherit" : ["ignore", "ignore", "inherit"],
+    });
+    tunnelProc.on("error", (e) => console.error(chalk.red(`[cloudflared] ${e?.message || e}`)));
+
+    // Predetermined public URLs
+    bridgeUrl = `wss://${TUNNEL_HOST}/ws`;
+    convexUrl = `https://${CONVEX_HOST}`;
+    if (VERBOSE) {
+      console.log(chalk.dim(`[bridge-public] ${bridgeUrl}`));
+      console.log(chalk.dim(`[convex-public] ${convexUrl}`));
+      try { probePublicBridge(bridgeUrl); } catch {}
+      try { probePublicConvex(convexUrl); } catch {}
+      startLocalProbes(repoRoot);
+    }
+    maybePrintPairCode(bridgeUrl, convexUrl);
+  }
```

**Delete** the entire `launchConvexTunnel(...)` function (no longer used).

---

## PATCH 4 — QR payload: set provider = `"cloudflared"` and always require Convex

Change the payload in `maybePrintPairCode`:

```diff
-    const payload = {
+    const payload = {
       v: 1,
       type: "openagents-bridge",
-      provider: "bore",
+      provider: LOCAL_ONLY ? "local" : TUNNEL_PROVIDER, // "cloudflared" when tunneled
       bridge: b,
       convex: c,
       token: readBridgeToken() as string | null,
     };
```

Add a clear log line right below the QR about Cloudflare Tunnel being compatible:

```diff
-    console.log("\nTunnel is active. Leave this running to stay connected.\n");
+    console.log("\nTunnel is active. Leave this running to stay connected.\n");
+    if (!LOCAL_ONLY) {
+      console.log(chalk.cyanBright("Cloudflare Tunnel in use — the same QR/deeplink flow works. Your app will use the tunneled URLs in the payload."));
+    }
```

---

## PATCH 5 — Make Convex setup **fatal** if it can’t be installed/started

1. Ensure the installer path **exits** the process on failure (instead of “best effort”):

```diff
-  try { ensureConvexBinaryWithProgress(); } catch { }
+  try {
+    await ensureConvexBinaryWithProgressStrict();
+  } catch (e: any) {
+    console.error(chalk.red(`Failed to install/start Convex: ${e?.message || e}`));
+    process.exit(1);
+  }
```

Add a **strict** wrapper (new function):

```ts
async function ensureConvexBinaryWithProgressStrict(): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let done = false;
    const finishOk = () => { if (!done) { done = true; resolve(); } };
    const finishBad = (e?: any) => { if (!done) { done = true; reject(e || new Error("convex install failed")); } };

    // reuse the existing ensureConvexBinaryWithProgress but detect if backend becomes healthy
    try {
      ensureConvexBinaryWithProgress();
    } catch (e) { return finishBad(e); }

    // wait until healthy (same probe as monitor), with a hard timeout
    const timeoutMs = 120000;
    const started = Date.now();
    const tick = () => {
      const req = http.get({ host: "127.0.0.1", port: 7788, path: "/instance_version", timeout: 1500 }, (res) => {
        const ok = !!res.statusCode && res.statusCode >= 200 && res.statusCode < 300;
        res.resume();
        if (ok) return finishOk();
        if (Date.now() - started > timeoutMs) return finishBad(new Error("Convex backend not healthy within timeout"));
        setTimeout(tick, 1000);
      });
      req.on("error", () => {
        if (Date.now() - started > timeoutMs) return finishBad(new Error("Convex backend not reachable"));
        setTimeout(tick, 1200);
      });
    };
    tick();
  });
}
```

2. In `monitorConvexSetupOnce` make the timeout **fatal** (instead of “print help and continue”):

```diff
-    console.log(chalk.redBright("✘ Convex backend did not become healthy within 2 minutes."));
+    console.log(chalk.redBright("✘ Convex backend did not become healthy within 2 minutes (REQUIRED)."));
...
-      } else {
-        printConvexHelp()
-      }
+      } else {
+        printConvexHelp();
+        process.exit(1);
+      }
...
-  function printConvexHelp() {
+  function printConvexHelp() {
     console.log(chalk.yellow("If this persists, try one of the following:"))
     console.log(lite(" - Ensure Bun or Node/NPM are installed so tricoder can fetch the backend"))
     console.log(lite(" - Run: bunx convex dev  (or: npx -y convex dev) once to install the local backend"))
     console.log(lite(" - Then re-run: npx tricoder"))
+    console.log(chalk.red("Convex is required. Exiting."));
   }
```

(You already call `tryStartConvexBackendIfNeeded`; this change simply makes failure exit.)

---

## What changes for the user?

* **Same command:** `npx tricoder`
* **Convex is required**: if the local backend can’t be installed & started, the script exits with instructions.
* **Cloudflare Tunnel**: if you’ve created a named tunnel and mapped:

  * `tunnel.openagents.com` → `http://localhost:8787` (bridge + `/ws`)
  * `convex.openagents.com` → `http://localhost:7788`

  the script will run `cloudflared tunnel run tricoder-bridge`, compute:

  * **bridge** = `wss://tunnel.openagents.com/ws`
  * **convex**  = `https://convex.openagents.com`

  then emit the **same QR/deeplink** payload (with `provider: "cloudflared"`). The Tricoder app reads those URLs and connects—no UI changes needed.

If you want me to ship this as a PR-ready commit (or convert into an `apply_patch` for your repo), say the word and I’ll output it as a diff against your current file.
