#!/usr/bin/env bun
/**
 * CLI for managing container sandbox.
 *
 * Usage:
 *   bun src/sandbox/cli.ts status    - Check if container CLI is installed
 *   bun src/sandbox/cli.ts bootstrap - Download and install container CLI
 *   bun src/sandbox/cli.ts start     - Start container system service
 *   bun src/sandbox/cli.ts stop      - Stop container system service
 *   bun src/sandbox/cli.ts test      - Run a test container
 */

import { Effect } from "effect";
import * as BunContext from "@effect/platform-bun/BunContext";
import {
  checkStatus,
  bootstrap,
  startSystem,
  stopSystem,
  downloadInstaller,
  installFromPkg,
} from "./bootstrap.js";
import { ContainerBackendTag } from "./backend.js";
import { macOSContainerLive } from "./macos-container.js";

const command = process.argv[2];

const run = <A, E>(effect: Effect.Effect<A, E, any>) =>
  Effect.runPromise(effect.pipe(Effect.provide(BunContext.layer)));

const runWithContainer = <A, E>(
  effect: Effect.Effect<A, E, ContainerBackendTag>,
) => Effect.runPromise(effect.pipe(Effect.provide(macOSContainerLive)));

async function main() {
  switch (command) {
    case "status": {
      const status = await run(checkStatus);
      console.log("\n📊 Container Status\n");
      console.log(`Platform:       ${status.platform}`);
      console.log(`macOS Version:  ${status.macOSVersion ?? "N/A"}`);
      console.log(`CLI Installed:  ${status.cliInstalled ? "✅" : "❌"}`);
      if (status.cliVersion) {
        console.log(`CLI Version:    ${status.cliVersion}`);
      }
      console.log(`System Running: ${status.systemRunning ? "✅" : "❌"}`);
      console.log();
      break;
    }

    case "bootstrap": {
      console.log("\n🚀 Bootstrapping container support...\n");
      try {
        const result = await run(bootstrap);
        console.log(result.success ? "✅" : "❌", result.message);
        if (result.installerPath) {
          console.log(`   Installer: ${result.installerPath}`);
        }
      } catch (e: any) {
        console.error("❌ Bootstrap failed:", e.message ?? e);
        process.exit(1);
      }
      console.log();
      break;
    }

    case "download": {
      console.log("\n📥 Downloading container installer...\n");
      try {
        const result = await run(downloadInstaller);
        console.log("✅", result.message);
        console.log(`   Path: ${result.installerPath}`);
      } catch (e: any) {
        console.error("❌ Download failed:", e.message ?? e);
        process.exit(1);
      }
      console.log();
      break;
    }

    case "install": {
      const pkgPath = process.argv[3];
      if (!pkgPath) {
        console.error("Usage: bun src/sandbox/cli.ts install <path-to-pkg>");
        process.exit(1);
      }
      console.log("\n📦 Installing container CLI...\n");
      try {
        const result = await run(installFromPkg(pkgPath));
        console.log(result.success ? "✅" : "❌", result.message);
      } catch (e: any) {
        console.error("❌ Install failed:", e.message ?? e);
        process.exit(1);
      }
      console.log();
      break;
    }

    case "start": {
      console.log("\n▶️  Starting container system...\n");
      try {
        const result = await run(startSystem);
        console.log("✅", result.message);
      } catch (e: any) {
        console.error("❌ Start failed:", e.message ?? e);
        process.exit(1);
      }
      console.log();
      break;
    }

    case "stop": {
      console.log("\n⏹️  Stopping container system...\n");
      try {
        const result = await run(stopSystem);
        console.log(result.success ? "✅" : "❌", result.message);
      } catch (e: any) {
        console.error("❌ Stop failed:", e.message ?? e);
        process.exit(1);
      }
      console.log();
      break;
    }

    case "test": {
      console.log("\n🧪 Testing container...\n");
      try {
        const result = await runWithContainer(
          Effect.gen(function* () {
            const backend = yield* ContainerBackendTag;

            // Check availability
            const available = yield* backend.isAvailable();
            if (!available) {
              return {
                success: false,
                message: "Container backend not available. Run 'bootstrap' first.",
              };
            }

            // Run test command
            console.log("Running: container run --rm alpine echo 'Hello from container'");
            const runResult = yield* backend.run(["echo", "Hello from container"], {
              image: "alpine:latest",
              workspaceDir: process.cwd(),
            });

            return {
              success: runResult.exitCode === 0,
              message: runResult.stdout.trim() || runResult.stderr.trim(),
              exitCode: runResult.exitCode,
            };
          }),
        );
        console.log(result.success ? "✅" : "❌", result.message);
        if ("exitCode" in result) {
          console.log(`   Exit code: ${result.exitCode}`);
        }
      } catch (e: any) {
        console.error("❌ Test failed:", e.message ?? e);
        process.exit(1);
      }
      console.log();
      break;
    }

    default: {
      console.log(`
Container Sandbox CLI

Usage:
  bun src/sandbox/cli.ts <command>

Commands:
  status     Check if container CLI is installed and running
  bootstrap  Download and install container CLI (requires macOS 26+)
  download   Download the installer without installing
  install    Install from a downloaded .pkg file
  start      Start container system service
  stop       Stop container system service
  test       Run a test container to verify everything works

Examples:
  bun src/sandbox/cli.ts status
  bun src/sandbox/cli.ts bootstrap
  bun src/sandbox/cli.ts test
`);
      break;
    }
  }
}

main().catch(console.error);
