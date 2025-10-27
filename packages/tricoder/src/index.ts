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

  const child = spawn("cargo", ["run", "-q", "-p", "oa-tunnel", "--", "--to", "bore.pub"], {
    cwd: repoRoot,
    stdio: ["ignore", "pipe", "inherit"],
  });

  let printedBridgeUrl = false;
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk: string) => {
    const lines = chunk.split(/\r?\n/).filter(Boolean);
    for (const line of lines) {
      const isUrl = line.startsWith("ws://") || line.startsWith("wss://");
      if (isUrl && !printedBridgeUrl) {
        printedBridgeUrl = true;
        console.log("\nPaste this into the mobile app Settings → Bridge URL:\n");
        console.log(chalk.greenBright(line));
        // After bridge URL, launch convex tunnel
        launchConvexTunnel(repoRoot);
      }
    }
  });
}

function launchConvexTunnel(repoRoot: string) {
  // Run a second tunnel for Convex (local 7788). We'll print an HTTP URL for the Settings override.
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
        console.log("\nOptional: set Convex URL (override) to:\n");
        console.log(chalk.cyan(httpUrl));
        console.log("\nTunnel is active. Leave this running to stay connected.\n");
      }
    }
  });
}

main();
