#!/usr/bin/env bun
/*
 * Dev wrapper for Tauri + Vite used by `beforeDevCommand`.
 * - If port 1420 is free: start Vite normally.
 * - If 1420 is already a Vite dev server: reuse it and keep this process alive.
 * - Otherwise: print a clear, actionable error and exit.
 */

import http from "http";
import net from "net";
import { spawn } from "child_process";

const BASE_PORT = Number(process.env.VITE_PORT ?? 1420);
const HOST = process.env.TAURI_DEV_HOST ?? "localhost";

async function isPortInUse(port: number): Promise<boolean> {
  const tryConnect = (host: string) =>
    new Promise<boolean>((resolve) => {
      const socket = net.createConnection({ host, port });
      const done = (result: boolean) => {
        try {
          socket.destroy();
        } catch {}
        resolve(result);
      };
      socket.once("connect", () => done(true));
      socket.once("error", (err: any) => {
        if (err && (err.code === "ECONNREFUSED" || err.code === "EHOSTUNREACH")) {
          return done(false);
        }
        // For timeouts/other errors, assume in use to be safe.
        return done(true);
      });
      socket.setTimeout(1000, () => done(true));
    });
  // Try IPv6 and IPv4 loopbacks.
  const [v6, v4] = await Promise.allSettled([
    tryConnect("::1"),
    tryConnect("127.0.0.1"),
  ]);
  const val = (s: PromiseSettledResult<boolean>) =>
    s.status === "fulfilled" ? s.value : true;
  return val(v6) || val(v4);
}

async function isViteServer(url: string): Promise<boolean> {
  return new Promise((resolve) => {
    const req = http.get(url, (res) => {
      let data = "";
      res.setEncoding("utf8");
      res.on("data", (chunk) => (data += chunk));
      res.on("end", () => {
        // Heuristic: Vite injects /@vite/client in index during dev
        resolve(data.includes("/@vite/client"));
      });
    });
    req.on("error", () => resolve(false));
    req.setTimeout(1500, () => {
      req.destroy();
      resolve(false);
    });
  });
}

async function printPortInfo(port: number) {
  try {
    const { execSync } = await import("node:child_process");
    const out = execSync(`lsof -nP -iTCP:${port} -sTCP:LISTEN || true`, {
      stdio: ["ignore", "pipe", "ignore"],
      encoding: "utf8",
    });
    if (out.trim()) {
      console.log("\nWho is using the port:\n" + out);
    }
  } catch {
    // ignore
  }
}

function keepAlive() {
  console.log(
    `tauri: reusing existing dev server on :${BASE_PORT} — keeping this process alive...`
  );
  // Keep the process running until killed by Tauri or user.
  const interval = setInterval(() => {}, 1 << 30);
  const cleanup = () => {
    clearInterval(interval);
    process.exit(0);
  };
  process.on("SIGINT", cleanup);
  process.on("SIGTERM", cleanup);
}

async function main() {
  const busy = await isPortInUse(BASE_PORT);
  if (!busy) {
    // Start Vite as usual; honor any env overrides for host/ports.
    const child = spawn("vite", [], {
      stdio: "inherit",
      env: process.env,
    });
    child.on("exit", (code, signal) => {
      if (signal) process.kill(process.pid, signal);
      process.exit(code ?? 0);
    });
    return;
  }

  // Port is busy. If it's a Vite dev server, reuse it and keep this process alive.
  const ok = await isViteServer(`http://localhost:${BASE_PORT}`);
  if (ok) {
    console.log(
      `Detected an existing Vite dev server at http://localhost:${BASE_PORT} (likely already running).`
    );
    return keepAlive();
  }

  // Not Vite — fail gracefully with help.
  console.error(
    `\nError: Port ${BASE_PORT} is in use by another process.\n` +
      `Tauri dev expects the frontend at http://localhost:${BASE_PORT}.\n` +
      `Please stop the process using the port, then re-run your command.`
  );
  await printPortInfo(BASE_PORT);
  console.error(
    `\nOptions:\n` +
      `  - Kill the process using the port (e.g., via Activity Monitor or 'kill <PID>').\n` +
      `  - Or start your own Vite on ${BASE_PORT} in another terminal, then re-run 'cargo tauri ios dev'.\n`
  );
  process.exit(1);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
