#!/usr/bin/env node
/**
 * OpenAgents NPX placeholder launcher (no Node WebSockets).
 *
 * Intent: `npx openagents` should start the Rust bridge (codex-bridge)
 * and point users to the mobile app to connect. This is a thin wrapper
 * around `cargo bridge` when run inside the OpenAgents repo. For now,
 * we avoid adding Node WS or pairing logic — the Rust bridge remains
 * the single source of truth.
 */

import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

function findRepoRoot(start: string): string | null {
  // Walk up to find a directory that contains crates/codex-bridge/Cargo.toml
  let dir = path.resolve(start);
  const root = path.parse(dir).root;
  while (true) {
    const marker = path.join(dir, "crates", "codex-bridge", "Cargo.toml");
    if (fs.existsSync(marker)) return dir;
    if (dir === root) break;
    dir = path.dirname(dir);
  }
  return null;
}

function printHeader() {
  console.log("OpenAgents — Desktop Bridge Launcher (placeholder)\n");
}

function printInstructions(wsUrl = "ws://localhost:8787/ws") {
  console.log("This will launch the Rust bridge (codex-bridge).\n");
  console.log("How to connect:");
  console.log("  1) Open the OpenAgents mobile app");
  console.log("  2) In Settings, set Bridge URL to: " + wsUrl);
  console.log("  3) Return to Session and start chatting\n");
}

async function main() {
  printHeader();
  printInstructions();

  const repoRoot = findRepoRoot(process.cwd());
  if (!repoRoot) {
    console.log(
      "No OpenAgents repo detected in parent directories.\n" +
        "To run the bridge manually:\n" +
        "  • Clone the repo: https://github.com/OpenAgentsInc/openagents\n" +
        "  • From the repo root run: cargo bridge\n"
    );
    process.exit(0);
  }

  // Spawn `cargo bridge` from the repo root. This is an alias defined in .cargo/config.toml
  console.log("Starting Rust bridge (cargo bridge)…\n");
  const child = spawn("cargo", ["bridge"], {
    cwd: repoRoot,
    stdio: "inherit",
    env: process.env,
  });

  child.on("exit", (code, signal) => {
    if (signal) {
      console.log(`Bridge exited via signal: ${signal}`);
    } else {
      console.log(`Bridge exited with code: ${code ?? 0}`);
    }
  });
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

