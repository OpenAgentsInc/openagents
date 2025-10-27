#!/usr/bin/env node
import chalk from "chalk"
import os from "node:os"
import { spawn, spawnSync } from "node:child_process"
import { existsSync } from "node:fs"
import http from "node:http"
import net from "node:net"
import { dirname, join } from "node:path"
import WebSocket from "ws"
import { buildTunnelArgs } from "./args.js"

// spawnSync already imported above
const VERBOSE = process.argv.includes("--verbose") || process.argv.includes("-v") || process.env.TRICODER_VERBOSE === "1";

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
  console.log("- " + (codex ? ok("codex binary present (assistant replies available)") : warn("codex binary not found (assistant replies won’t stream)")));
  console.log("");
}

function main() {
  console.info(chalk.bold("OpenAgents Tricoder - Desktop Bridge"));
  const repoRoot = findRepoRoot(process.cwd());
  // Always print a quick assessment so users see what's missing
  printEnvAssessment(repoRoot);

  if (!repoRoot) {
    console.log(chalk.yellow("No local OpenAgents repo detected."));
    console.log("- Short-term requirement: repo + Rust to build and run the bridge + tunnels.");
    console.log("- Planned improvement: single-command bootstrap to download needed binaries without cloning.");
    console.log("\nTo proceed now: clone the repo and ensure Rust is installed, then re-run:\n  git clone https://github.com/OpenAgentsInc/openagents\n  cd openagents\n  npx tricoder\n");
    return;
  }

  // Ensure the Rust bridge is running locally (best effort)
  ensureBridgeRunning(repoRoot);

  // Launch Bridge tunnel (local 8787)
  const child = spawn("cargo", buildTunnelArgs(8787, "bore.pub"), {
    cwd: repoRoot,
    stdio: ["ignore", "pipe", "pipe"],
  });

  let bridgeUrl: string | null = null;
  let convexUrl: string | null = null;
  let printedCombined = false;
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk: string) => {
    const lines = chunk.split(/\r?\n/).filter(Boolean);
    for (const line of lines) {
      const isUrl = line.startsWith("ws://") || line.startsWith("wss://");
      if (isUrl && !bridgeUrl) {
        bridgeUrl = line.trim();
        if (VERBOSE) console.log(chalk.dim(`[bridge-public] ${bridgeUrl}`));
        // After bridge URL, launch Convex tunnel
        launchConvexTunnel(repoRoot, (url) => {
          convexUrl = url;
          if (VERBOSE) console.log(chalk.dim(`[convex-public] ${convexUrl}`));
          maybePrintPairCode(bridgeUrl!, convexUrl!);
          // Public probes
          if (VERBOSE) { try { if (bridgeUrl) probePublicBridge(bridgeUrl); } catch { } }
          if (VERBOSE) { try { if (convexUrl) probePublicConvex(convexUrl); } catch { } }
          // Connectivity summary
          if (VERBOSE) console.log(chalk.dim(`[pair] bridge=${bridgeUrl} convex=${convexUrl}`));
          // Seed a demo thread via bridge controls to ensure history appears
          if (VERBOSE) { try { seedDemoViaBridgeControl(); } catch { } }
        });
        // Start local health probes (status changes only)
        if (VERBOSE) startLocalProbes(repoRoot);
      }
    }
  });
  child.stderr.setEncoding("utf8");
  // Aggregate noisy bore logs
  let bridgeConnNew = 0, bridgeConnExit = 0;
  setInterval(() => {
    if (VERBOSE && (bridgeConnNew || bridgeConnExit)) {
      console.log(chalk.dim(`[bridge-tunnel] ${bridgeConnNew} new, ${bridgeConnExit} exited (last 10s)`));
      bridgeConnNew = 0; bridgeConnExit = 0;
    }
  }, 10000);
  child.stderr.on("data", (chunk: string) => {
    if (!VERBOSE) return;
    for (const line of String(chunk).split(/\r?\n/).filter(Boolean)) {
      if (/bore_cli::client: new connection/.test(line)) { bridgeConnNew++; continue; }
      if (/bore_cli::client: connection exited/.test(line)) { bridgeConnExit++; continue; }
      console.error(chalk.dim(`[bridge-tunnel] ${line}`));
    }
  });

  function maybePrintPairCode(b: string, c: string) {
    if (printedCombined) return;
    if (!b || !c) return;
    printedCombined = true;
    const payload = {
      v: 1,
      type: "openagents-bridge",
      provider: "bore",
      bridge: b,
      convex: c,
      token: null as string | null,
    };
    const code = encodePairCode(payload);
    console.log("\nPaste this code into the mobile app Settings → Bridge Code:\n");
    console.log(chalk.greenBright(code));
    // Security notice
    console.log(chalk.yellowBright("\nWarning: This code is your private bridge token — never share it with anyone."));
    console.log("\nTunnel is active. Leave this running to stay connected.\n");
    // Helpful resources (light gray)
    const lite = (s: string) => chalk.hex('#9CA3AF')(s); // light gray
    console.log(lite("Resources:"));
    console.log(lite(" - Any questions? Please @ us on X: https://x.com/OpenAgentsInc"));
    console.log(lite(" - Download the iOS app on TestFlight: https://testflight.apple.com/join/dvQdns5B"));
    console.log(lite(" - Android coming soon"));
    console.log(lite(" - All code is open-source here: https://github.com/OpenAgentsInc/openagents"));
    console.log(lite(" - Or open an issue: https://github.com/OpenAgentsInc/openagents/issues"));
    // Check for codex binary presence — required for assistant responses
    try {
      const probe = spawnSync(process.platform === 'win32' ? 'where' : 'which', ['codex'], { stdio: 'pipe' });
      const ok = probe.status === 0;
      if (VERBOSE) console.log(ok ? chalk.dim('[codex] codex binary found in PATH') : chalk.yellow('[codex] codex binary NOT found — assistant responses will not stream'));
      // Bridge status via WS
      if (VERBOSE) { try { bridgeStatus(); } catch { } }
      // Start persistent WS tail to print bridge + codex events
      if (VERBOSE) { try { startBridgeEventTail(); } catch { } }
    } catch { }
    // In quiet mode, provide a concise Convex setup guide while backend initializes
    if (!VERBOSE) {
      try { monitorConvexSetupOnce(repoRoot as string) } catch { }
    }
  }
}

function launchConvexTunnel(repoRoot: string, onUrl: (url: string) => void) {
  // Run a second tunnel for Convex (local 7788). We'll emit an HTTP URL for override.
  const child = spawn("cargo", buildTunnelArgs(7788, "bore.pub"), {
    cwd: repoRoot,
    stdio: ["ignore", "pipe", "pipe"],
  });

  let printed = false;
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk: string) => {
    const lines = chunk.split(/\r?\n/).filter(Boolean);
    for (const line of lines) {
      const m = line.match(/^ws:\/\/([^:]+):(\d+)\/ws$/);
      if (m && !printed) {
        printed = true;
        const host = m[1];
        const port = m[2];
        const httpUrl = `http://${host}:${port}`;
        onUrl(httpUrl);
      }
    }
  });
  child.stderr.setEncoding("utf8");
  let convexConnNew = 0, convexConnExit = 0;
  setInterval(() => {
    if (VERBOSE && (convexConnNew || convexConnExit)) {
      console.log(chalk.dim(`[convex-tunnel] ${convexConnNew} new, ${convexConnExit} exited (last 10s)`));
      convexConnNew = 0; convexConnExit = 0;
    }
  }, 10000);
  child.stderr.on("data", (chunk: string) => {
    if (!VERBOSE) return;
    for (const line of String(chunk).split(/\r?\n/).filter(Boolean)) {
      if (/bore_cli::client: new connection/.test(line)) { convexConnNew++; continue; }
      if (/bore_cli::client: connection exited/.test(line)) { convexConnExit++; continue; }
      console.error(chalk.dim(`[convex-tunnel] ${line}`));
    }
  });
}

function encodePairCode(obj: any): string {
  const json = JSON.stringify(obj);
  const b64 = Buffer.from(json, "utf8").toString("base64");
  return b64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function ensureBridgeRunning(repoRoot: string) {
  const sock = net.createConnection({ host: "127.0.0.1", port: 8787 });
  let connected = false;
  sock.once("connect", async () => {
    connected = true; try { sock.end(); } catch { }
    const force = (process.env.TRICODER_FORCE_RESTART || '1') !== '0';
    // Probe whether current bridge supports echo. If not, or if forced, restart it.
    const supports = await probeBridgeEchoOnce(700).catch(() => false);
    if (force || !supports) {
      if (VERBOSE) console.log(chalk.dim(`Restarting local bridge with debug enabled (${force ? 'forced' : 'no echo support'})…`));
      try { restartBridgeProcess(repoRoot); } catch { }
    }
  });
  sock.once("error", () => {
    // Not listening; start it
    startBridgeProcess(repoRoot);
  });
  // timeout after brief period
  setTimeout(() => { try { if (!connected) sock.destroy(); } catch { } }, 500);
}

function startLocalProbes(repoRoot: string) {
  let lastBridgeOk: boolean | null = null;
  let lastConvexOk: boolean | null = null;
  const probeBridge = () => {
    const s = net.createConnection({ host: "127.0.0.1", port: 8787 });
    let ok = false;
    s.once("connect", () => { ok = true; try { s.end(); } catch { } });
    s.once("error", () => { ok = false; });
    s.once("close", () => {
      if (VERBOSE && lastBridgeOk !== ok) {
        lastBridgeOk = ok;
        console.log(ok ? chalk.dim("[bridge-local] 127.0.0.1:8787 reachable") : chalk.dim("[bridge-local] 127.0.0.1:8787 not reachable"));
      }
    });
    setTimeout(() => { try { s.destroy(); } catch { } }, 500);
  };
  const probeConvex = () => {
    const req = http.get({ host: "127.0.0.1", port: 7788, path: "/instance_version", timeout: 700 }, (res) => {
      const ok = res.statusCode !== undefined && res.statusCode >= 200 && res.statusCode < 300;
      if (VERBOSE && lastConvexOk !== ok) {
        lastConvexOk = ok;
        console.log(ok ? chalk.dim("[convex-local] http://127.0.0.1:7788 healthy") : chalk.dim("[convex-local] http://127.0.0.1:7788 unreachable"));
      }
      res.resume();
      // On first healthy, try function push once via root script (best effort)
      if (ok && (probeConvex as any)._firstDone !== true) {
        (probeConvex as any)._firstDone = true;
        tryPushConvexFunctions(repoRoot, !VERBOSE);
      }
    });
    req.on("error", () => {
      const ok = false;
      if (VERBOSE && lastConvexOk !== ok) {
        lastConvexOk = ok;
        console.log(chalk.dim("[convex-local] http://127.0.0.1:7788 unreachable"));
      }
    });
    req.setTimeout(800, () => { try { req.destroy(); } catch { } });
  };
  // Kick immediately and then poll
  probeBridge();
  probeConvex();
  setInterval(probeBridge, 5000);
  setInterval(probeConvex, 5000);
}

function monitorConvexSetupOnce(repoRoot: string) {
  const backendPath = join(os.homedir(), ".openagents", "bin", "local_backend")
  const exists = existsSync(backendPath)
  const startMsg = !exists
    ? "Setting up local Convex backend (first run downloads a small binary)…"
    : "Starting local Convex backend…"
  // Add a blank line before the status and show a spinner while waiting
  console.log("")
  const frames = ["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"]
  let fi = 0
  let spun = false
  const spin = setInterval(() => {
    spun = true
    try { process.stdout.write("\r" + chalk.cyanBright(`⏳ ${startMsg} ${frames[fi]}`)) } catch {}
    fi = (fi + 1) % frames.length
  }, 120)
  const start = Date.now()
  const finish = (okSeconds: number) => {
    try { clearInterval(spin) } catch {}
    // Clear spinner line
    if (spun) { try { process.stdout.write("\r\x1b[K") } catch {} }
    console.log(chalk.greenBright(`✔ Convex backend ready in ${okSeconds}s.`))
  }
  const check = () => {
    const req = http.get({ host: "127.0.0.1", port: 7788, path: "/instance_version", timeout: 1500 }, (res) => {
      if (res.statusCode && res.statusCode >= 200 && res.statusCode < 300) {
        finish(Math.round((Date.now() - start) / 1000))
      } else {
        setTimeout(check, 1000)
      }
      res.resume()
    })
    req.on("error", () => setTimeout(check, 1200))
  }
  check()
}

function tryPushConvexFunctions(repoRoot: string, announce?: boolean) {
  try {
    if (announce || VERBOSE) console.log(chalk.dim(`[convex] Deploying functions…`));
    const haveBun = hasCmd('bun');
    const haveNpx = hasCmd('npx');
    if (haveBun) {
      const child = spawn("bun", ["run", "convex:dev:once"], { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"] });
      child.stdout?.setEncoding("utf8");
      child.stdout?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
      child.stderr?.setEncoding("utf8");
      child.stderr?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
      child.on("exit", (code) => {
        if (code !== 0) {
          if (VERBOSE) console.log(chalk.dim(`[convex-bootstrap] script missing or failed; trying 'bunx convex dev' one-shot…`));
          const fallback = spawn("bunx", ["convex", "dev"], { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"] });
          fallback.stdout?.setEncoding("utf8");
          fallback.stdout?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
          fallback.stderr?.setEncoding("utf8");
          fallback.stderr?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
          fallback.on("exit", (c2) => { if (announce || VERBOSE) console.log(chalk.dim(`[convex] Functions deploy finished (code ${c2 ?? 0})`)) });
        } else {
          if (announce || VERBOSE) console.log(chalk.dim(`[convex] Functions deploy finished (code ${code ?? 0})`));
        }
      });
      return;
    }
    if (haveNpx) {
      if (VERBOSE) console.log(chalk.dim(`[convex-bootstrap] bun not found; trying 'npx convex dev' one-shot…`));
      const fallback = spawn("npx", ["-y", "convex", "dev"], { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"] });
      fallback.stdout?.setEncoding("utf8");
      fallback.stdout?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
      fallback.stderr?.setEncoding("utf8");
      fallback.stderr?.on("data", d => { if (!VERBOSE) return; String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))) });
      fallback.on("exit", (c2) => { if (announce || VERBOSE) console.log(chalk.dim(`[convex] Functions deploy finished (code ${c2 ?? 0})`)) });
      return;
    }
    if (announce || VERBOSE) console.log(chalk.dim(`[convex] Skipping functions deploy: bun/npx not available`));
  } catch (e: any) {
    if (announce || VERBOSE) console.log(chalk.dim(`[convex] Functions deploy skipped: ${e?.message || e}`));
  }
}

function probePublicConvex(base: string) {
  try {
    const u = new URL(base);
    const opts: http.RequestOptions = { host: u.hostname, port: Number(u.port || 80), path: "/instance_version", timeout: 2000 };
    const req = http.get(opts, (res) => {
      const code = res.statusCode || 0;
      let body = "";
      res.setEncoding("utf8");
      res.on("data", (c) => (body += c));
      res.on("end", () => {
        const snippet = body.trim().slice(0, 80).replace(/\s+/g, " ");
        if (VERBOSE) console.log(chalk.dim(`[convex-public-check] GET ${u.hostname}:${u.port}/instance_version -> ${code} ${snippet ? `body: ${snippet}` : ""}`));
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
    const port = Number(u.port || (u.protocol === "https:" ? 443 : 80));
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
    ws.on("open", () => {
      // no-op: we just tail broadcast feed
    });
    ws.on("message", (data: WebSocket.RawData) => {
      const s = String(data || "").trim();
      if (!s) return;
      if (s.startsWith("{")) {
        try {
          const obj = JSON.parse(s);
          const t = obj?.type || obj?.msg?.type || '';
          if (VERBOSE && String(t).startsWith('bridge.')) {
            // Print richer details for common debug events
            if (t === 'bridge.control' && typeof obj.raw === 'string') {
              console.log(chalk.dim(`[bridge-control] raw=${obj.raw}`));
              return;
            }
            if (t === 'bridge.ws_in' && typeof obj.preview === 'string') {
              console.log(chalk.dim(`[bridge-in] ${obj.preview}`));
              return;
            }
            if (t === 'bridge.run_submit') {
              console.log(chalk.dim(`[bridge] bridge.run_submit threadDocId=${obj.threadDocId || ''} len=${obj.len || 0}`));
              return;
            }
            if (t === 'bridge.client_connected' || t === 'bridge.client_disconnected') {
              console.log(chalk.dim(`[bridge] ${t}`));
              return;
            }
            if (t === 'bridge.echo') {
              console.log(chalk.dim(`[bridge-echo] tag=${obj.tag || ''} payload=${obj.payload || ''}`));
              return;
            }
            console.log(chalk.dim(`[bridge] ${t}`));
            return;
          }
          // surface codex JSONL events succinctly
          if (VERBOSE && t && /agent_message|assistant|message|reason|exec_begin|exec/.test(String(t))) {
            console.log(chalk.dim(`[codex] ${t}`));
            return;
          }
          if (VERBOSE && t === 'bridge.codex_raw' && typeof obj.line === 'string') {
            console.log(chalk.dim(`[codex-raw] ${obj.line}`));
            return;
          }
        } catch { /* ignore non-json */ }
      }
    });
    ws.on("close", () => { if (!closed) setTimeout(connect, 1200); });
    ws.on("error", () => { try { ws.close(); } catch { }; if (!closed) setTimeout(connect, 1500); });
  };
  connect();
  // Return a stopper in case we want to end later (not used for dev)
  return () => { closed = true; };
}

function startBridgeProcess(repoRoot: string) {
  if (VERBOSE) console.log(chalk.dim("Starting local bridge (cargo run -p codex-bridge)…"));
  const child = spawn("cargo", ["run", "-p", "codex-bridge", "--", "--bind", "0.0.0.0:8787"], {
    cwd: repoRoot,
    stdio: VERBOSE ? "inherit" : ["ignore", "ignore", "ignore"],
    env: {
      ...process.env,
      // Quiet verbose deps; keep our bridge at info
      RUST_LOG: process.env.RUST_LOG || (VERBOSE ? "info,convex=warn,convex::base_client=warn,tungstenite=warn,notify=warn" : "warn"),
      // Disable FS→Convex sync to avoid large startup imports/noise in dev
      OPENAGENTS_CONVEX_SYNC: process.env.OPENAGENTS_CONVEX_SYNC || "0",
      BRIDGE_DEBUG_WS: process.env.BRIDGE_DEBUG_WS || (VERBOSE ? "1" : "0"),
      BRIDGE_DEBUG_CODEX: process.env.BRIDGE_DEBUG_CODEX || (VERBOSE ? "1" : "0"),
    },
  });
  child.on("error", () => { });
}

function restartBridgeProcess(repoRoot: string) {
  // Try graceful: send a QUIT via lsof->pid
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
    ws.on("open", () => {
      try { ws.send(JSON.stringify({ control: 'echo', tag: 'probe', payload: 'ok' })) } catch { }
    });
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

main();
