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

  // Launch Bridge tunnel (local 8787)
  const child = spawn("cargo", buildTunnelArgs(8787, "bore.pub"), {
    cwd: repoRoot,
    stdio: ["ignore", "pipe", "inherit"],
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
        // After bridge URL, launch Convex tunnel
        launchConvexTunnel(repoRoot, (url) => {
          convexUrl = url;
          maybePrintPairCode(bridgeUrl!, convexUrl!);
        });
      }
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
    stdio: ["ignore", "pipe", "inherit"],
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
}

function encodePairCode(obj: any): string {
  const json = JSON.stringify(obj);
  const b64 = Buffer.from(json, "utf8").toString("base64");
  return b64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

main();
