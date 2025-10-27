#!/usr/bin/env node
/**
 * OpenAgents NPX placeholder (no Node WebSockets, no bridge spawn).
 *
 * For now, `npx tricoder` only prints guidance. The Rust bridge
 * remains the single source of truth and is started separately.
 */
import chalk from "chalk";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { buildTunnelArgs } from "./args.js";
import net from "node:net";
import http from "node:http";
import WebSocket from "ws";

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

function main() {
  console.info(chalk.bold("OpenAgents Tricoder - Desktop Bridge"));
  const repoRoot = findRepoRoot(process.cwd());
  if (!repoRoot) {
    console.log("\nThis internal tunnel flow requires the OpenAgents repo and Rust.");
    console.log("Clone: https://github.com/OpenAgentsInc/openagents");
    console.log("Then run from repo root: cargo run -p oa-tunnel -- --to bore.pub\n");
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
        // Log public bridge URL
        console.log(chalk.dim(`[bridge-public] ${bridgeUrl}`));
        // After bridge URL, launch Convex tunnel
        launchConvexTunnel(repoRoot, (url) => {
          convexUrl = url;
          console.log(chalk.dim(`[convex-public] ${convexUrl}`));
          maybePrintPairCode(bridgeUrl!, convexUrl!);
          // Public probes
          try { if (bridgeUrl) probePublicBridge(bridgeUrl); } catch {}
          try { if (convexUrl) probePublicConvex(convexUrl); } catch {}
          // Connectivity summary
          console.log(chalk.dim(`[pair] bridge=${bridgeUrl} convex=${convexUrl}`));
          // Seed a demo thread via bridge controls to ensure history appears
          try { seedDemoViaBridgeControl(); } catch {}
        });
        // Start local health probes (status changes only)
        startLocalProbes(repoRoot);
      }
    }
  });
  child.stderr.setEncoding("utf8");
  // Aggregate noisy bore logs
  let bridgeConnNew = 0, bridgeConnExit = 0;
  setInterval(() => {
    if (bridgeConnNew || bridgeConnExit) {
      console.log(chalk.dim(`[bridge-tunnel] ${bridgeConnNew} new, ${bridgeConnExit} exited (last 10s)`));
      bridgeConnNew = 0; bridgeConnExit = 0;
    }
  }, 10000);
  child.stderr.on("data", (chunk: string) => {
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
    console.log("\nPaste this single code into the mobile app Settings → Bridge Code:\n");
    console.log(chalk.greenBright(code));
    console.log("\nTunnel is active. Leave this running to stay connected.\n");
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
    if (convexConnNew || convexConnExit) {
      console.log(chalk.dim(`[convex-tunnel] ${convexConnNew} new, ${convexConnExit} exited (last 10s)`));
      convexConnNew = 0; convexConnExit = 0;
    }
  }, 10000);
  child.stderr.on("data", (chunk: string) => {
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
  sock.once("connect", () => { connected = true; try { sock.end(); } catch {} });
  sock.once("error", () => {
    // Not listening; try to start it
    console.log(chalk.dim("Starting local bridge (cargo bridge)…"));
    const child = spawn("cargo", ["bridge"], { cwd: repoRoot, stdio: "inherit" });
    child.on("error", () => {});
  });
  // timeout after brief period
  setTimeout(() => { try { if (!connected) sock.destroy(); } catch {} }, 400);
}

function startLocalProbes(repoRoot: string) {
  let lastBridgeOk: boolean | null = null;
  let lastConvexOk: boolean | null = null;
  const probeBridge = () => {
    const s = net.createConnection({ host: "127.0.0.1", port: 8787 });
    let ok = false;
    s.once("connect", () => { ok = true; try { s.end(); } catch {} });
    s.once("error", () => { ok = false; });
    s.once("close", () => {
      if (lastBridgeOk !== ok) {
        lastBridgeOk = ok;
        console.log(ok ? chalk.dim("[bridge-local] 127.0.0.1:8787 reachable") : chalk.dim("[bridge-local] 127.0.0.1:8787 not reachable"));
      }
    });
    setTimeout(() => { try { s.destroy(); } catch {} }, 500);
  };
  const probeConvex = () => {
    const req = http.get({ host: "127.0.0.1", port: 7788, path: "/instance_version", timeout: 700 }, (res) => {
      const ok = res.statusCode !== undefined && res.statusCode >= 200 && res.statusCode < 300;
      if (lastConvexOk !== ok) {
        lastConvexOk = ok;
        console.log(ok ? chalk.dim("[convex-local] http://127.0.0.1:7788 healthy") : chalk.dim("[convex-local] http://127.0.0.1:7788 unreachable"));
      }
      res.resume();
      // On first healthy, try function push once via root script (best effort)
      if (ok && (probeConvex as any)._firstDone !== true) {
        (probeConvex as any)._firstDone = true;
        tryPushConvexFunctions(repoRoot);
      }
    });
    req.on("error", () => {
      const ok = false;
      if (lastConvexOk !== ok) {
        lastConvexOk = ok;
        console.log(chalk.dim("[convex-local] http://127.0.0.1:7788 unreachable"));
      }
    });
    req.setTimeout(800, () => { try { req.destroy(); } catch {} });
  };
  // Kick immediately and then poll
  probeBridge();
  probeConvex();
  setInterval(probeBridge, 5000);
  setInterval(probeConvex, 5000);
}

function tryPushConvexFunctions(repoRoot: string) {
  try {
    // Try root package script first; if missing, fall back to bunx convex dev
    console.log(chalk.dim(`[convex-bootstrap] pushing functions…`));
    const child = spawn("bun", ["run", "convex:dev:once"], { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"] });
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", d => String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))));
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", d => String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))));
    child.on("exit", (code) => {
      if (code !== 0) {
        console.log(chalk.dim(`[convex-bootstrap] script missing or failed; trying 'bunx convex dev' one-shot…`));
        const fallback = spawn("bunx", ["convex", "dev"], { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"] });
        fallback.stdout.setEncoding("utf8");
        fallback.stdout.on("data", d => String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))));
        fallback.stderr.setEncoding("utf8");
        fallback.stderr.on("data", d => String(d).split(/\r?\n/).forEach(l => l && console.log(chalk.dim(`[convex-bootstrap] ${l}`))));
        fallback.on("exit", (c2) => console.log(chalk.dim(`[convex-bootstrap] done (fallback code ${c2 ?? 0})`)));
      } else {
        console.log(chalk.dim(`[convex-bootstrap] done (code ${code ?? 0})`));
      }
    });
  } catch (e: any) {
    console.log(chalk.dim(`[convex-bootstrap] skipped: ${e?.message || e}`));
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
        console.log(chalk.dim(`[convex-public-check] GET ${u.hostname}:${u.port}/instance_version -> ${code} ${snippet ? `body: ${snippet}` : ""}`));
      });
    });
    req.on("error", (e: any) => {
      console.log(chalk.dim(`[convex-public-check] error: ${e?.message || e}`));
    });
    req.setTimeout(2500, () => { try { req.destroy(); } catch {} });
  } catch (e: any) {
    console.log(chalk.dim(`[convex-public-check] invalid URL: ${String(e?.message || e)}`));
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
        console.log(chalk.dim(`[bridge-public-check] ${first}`));
        try { s.destroy(); } catch {}
      }
    });
    s.on("error", (e: any) => {
      console.log(chalk.dim(`[bridge-public-check] error: ${e?.message || e}`));
    });
    setTimeout(() => { try { s.destroy(); } catch {} }, 2500);
  } catch (e: any) {
    console.log(chalk.dim(`[bridge-public-check] invalid URL: ${String(e?.message || e)}`));
  }
}

function seedDemoViaBridgeControl() {
  const ws = new WebSocket("ws://127.0.0.1:8787/ws");
  let done = false;
  const timer = setTimeout(() => { try { ws.close(); } catch {} }, 4000);
  ws.on("open", () => {
    try {
      ws.send(JSON.stringify({ control: "convex.create_demo_thread" }));
      ws.send(JSON.stringify({ control: "convex.status" }));
    } catch {}
  });
  ws.on("message", (data: WebSocket.RawData) => {
    const s = String(data || "").trim();
    if (!s.startsWith("{")) return;
    try {
      const obj = JSON.parse(s);
      if (obj?.type === "bridge.convex_status") {
        console.log(chalk.dim(`[bridge-control] convex.status -> ${obj.healthy ? "healthy" : "unhealthy"} url=${obj.convex_url || ""}`));
      }
      if (obj?.type === "bridge.projects") {
        console.log(chalk.dim(`[bridge-control] projects -> ${Array.isArray(obj.items) ? obj.items.length : 0} items`));
      }
    } catch {}
  });
  ws.on("close", () => { if (!done) clearTimeout(timer); done = true; });
  ws.on("error", () => { if (!done) clearTimeout(timer); done = true; });
}

main();
