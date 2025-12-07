#!/usr/bin/env bun
/**
 * HuggingFace Dataset CLI
 *
 * Commands:
 *   download openthoughts-sft   Download OpenThoughts SFT dataset
 *   download --repo <repo>      Download any HF dataset
 *   list                        List downloaded datasets
 *   info <repo>                 Show info about a downloaded dataset
 *   delete <repo>               Delete a downloaded dataset
 *   count                       Count trajectories in OpenThoughts
 *   sample [n]                  Show sample trajectories from OpenThoughts
 */

import { Effect, Layer } from "effect";
import { BunContext } from "@effect/platform-bun";
import {
  HFDatasetService,
  HFDatasetServiceLive,
} from "./service.js";
import {
  OpenThoughtsService,
  OpenThoughtsServiceLive,
} from "./openthoughts.js";
import {
  OPENTHOUGHTS_SFT_CONFIG,
  HFDatasetError,
} from "./schema.js";

// ============================================================================
// Argument Parsing
// ============================================================================

const args = process.argv.slice(2);
const command = args[0];

function printUsage(): void {
  console.log(`
HuggingFace Dataset CLI

Usage:
  bun run hf:download openthoughts-sft     Download OpenThoughts SFT dataset
  bun run hf:download --repo <owner/name>  Download any HF dataset
  bun run hf:list                          List downloaded datasets
  bun run hf:info <owner/name>             Show info about a downloaded dataset
  bun run hf:delete <owner/name>           Delete a downloaded dataset
  bun run hf:count                         Count trajectories in OpenThoughts
  bun run hf:sample [n]                    Show n sample trajectories (default: 3)

Environment:
  HF_TOKEN    Optional HuggingFace access token for private datasets

Examples:
  bun run hf:download openthoughts-sft
  bun run hf:download --repo open-thoughts/OpenThoughts-TB-dev
  bun run hf:sample 5
`);
}

// ============================================================================
// Commands
// ============================================================================

const downloadCommand = () =>
  Effect.gen(function* () {
    const hfService = yield* HFDatasetService;

    const target = args[1];

    if (target === "openthoughts-sft") {
      console.log(`Downloading: ${OPENTHOUGHTS_SFT_CONFIG.repo}`);
      const result = yield* hfService.download(OPENTHOUGHTS_SFT_CONFIG);
      console.log(`\nDownload complete!`);
      console.log(`  Location: ${result.localPath}`);
      console.log(`  Files: ${result.files.length}`);
      console.log(`  Size: ${formatBytes(result.totalBytes)}`);
    } else if (target === "--repo") {
      const repo = args[2];
      if (!repo) {
        console.error("Error: --repo requires a repository name");
        process.exit(1);
      }

      console.log(`Downloading: ${repo}`);
      const result = yield* hfService.download({
        repo,
        filePattern: args[3] ?? "**/*.parquet",
      });
      console.log(`\nDownload complete!`);
      console.log(`  Location: ${result.localPath}`);
      console.log(`  Files: ${result.files.length}`);
      console.log(`  Size: ${formatBytes(result.totalBytes)}`);
    } else {
      printUsage();
      process.exit(1);
    }
  });

const listCommand = () =>
  Effect.gen(function* () {
    const hfService = yield* HFDatasetService;
    const datasets = yield* hfService.listDownloaded();

    if (datasets.length === 0) {
      console.log("No datasets downloaded yet.");
      console.log("\nRun: bun run hf:download openthoughts-sft");
      return;
    }

    console.log("Downloaded Datasets:\n");
    for (const ds of datasets) {
      console.log(`  ${ds.repo}`);
      console.log(`    Path: ${ds.localPath}`);
      console.log(`    Files: ${ds.files.length}`);
      console.log(`    Size: ${formatBytes(ds.totalBytes)}`);
      console.log(`    Downloaded: ${ds.downloadedAt}`);
      console.log();
    }
  });

const infoCommand = () =>
  Effect.gen(function* () {
    const hfService = yield* HFDatasetService;
    const repo = args[1];

    if (!repo) {
      console.error("Error: info command requires a repository name");
      process.exit(1);
    }

    const info = yield* hfService.getDownloadInfo(repo);

    if (!info) {
      console.log(`Dataset not downloaded: ${repo}`);
      return;
    }

    console.log(`Dataset: ${info.repo}`);
    console.log(`Path: ${info.localPath}`);
    console.log(`Revision: ${info.revision}`);
    console.log(`Downloaded: ${info.downloadedAt}`);
    console.log(`Size: ${formatBytes(info.totalBytes)}`);
    console.log(`Files:`);
    for (const file of info.files) {
      console.log(`  - ${file}`);
    }
  });

const deleteCommand = () =>
  Effect.gen(function* () {
    const hfService = yield* HFDatasetService;
    const repo = args[1];

    if (!repo) {
      console.error("Error: delete command requires a repository name");
      process.exit(1);
    }

    yield* hfService.delete(repo);
    console.log(`Deleted: ${repo}`);
  });

const countCommand = () =>
  Effect.gen(function* () {
    const otService = yield* OpenThoughtsService;
    const count = yield* otService.count();
    console.log(`OpenThoughts SFT trajectories: ${count.toLocaleString()}`);
  });

const sampleCommand = () =>
  Effect.gen(function* () {
    const otService = yield* OpenThoughtsService;
    const n = parseInt(args[1] ?? "3", 10);

    console.log(`Fetching ${n} sample trajectories from OpenThoughts SFT...\n`);

    const trajectories = yield* otService.getTrajectories(0, n);

    for (const traj of trajectories) {
      console.log(`─────────────────────────────────────────`);
      console.log(`Session: ${traj.session_id}`);
      console.log(`Agent: ${traj.agent.name} (${traj.agent.model_name})`);
      console.log(`Task: ${(traj.extra as Record<string, unknown>)?.task ?? "unknown"}`);
      console.log(`Steps: ${traj.steps.length}`);
      console.log();

      // Show first few steps
      const previewSteps = traj.steps.slice(0, 3);
      for (const step of previewSteps) {
        const content = typeof step.message === "string"
          ? step.message.slice(0, 200)
          : JSON.stringify(step.message).slice(0, 200);
        console.log(`  [${step.source}] ${content}${content.length >= 200 ? "..." : ""}`);
      }

      if (traj.steps.length > 3) {
        console.log(`  ... and ${traj.steps.length - 3} more steps`);
      }
      console.log();
    }

    console.log(`─────────────────────────────────────────`);
    console.log(`Showing ${trajectories.length} of ${(yield* otService.count()).toLocaleString()} total trajectories`);
  });

// ============================================================================
// Main
// ============================================================================

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

const program = Effect.gen(function* () {
  switch (command) {
    case "download":
      yield* downloadCommand();
      break;
    case "list":
      yield* listCommand();
      break;
    case "info":
      yield* infoCommand();
      break;
    case "delete":
      yield* deleteCommand();
      break;
    case "count":
      yield* countCommand();
      break;
    case "sample":
      yield* sampleCommand();
      break;
    default:
      printUsage();
  }
}).pipe(
  Effect.catchAll((error) => {
    if (error instanceof HFDatasetError) {
      console.error(`Error [${error.reason}]: ${error.message}`);
    } else {
      console.error(`Error: ${error}`);
    }
    process.exit(1);
  }),
);

// Build the layer stack
const mainLayer = Layer.mergeAll(
  HFDatasetServiceLive(),
).pipe(
  Layer.provideMerge(BunContext.layer),
);

// OpenThoughts layer depends on HFDatasetService
const fullLayer = OpenThoughtsServiceLive.pipe(
  Layer.provideMerge(mainLayer),
);

// Run
Effect.runPromise(program.pipe(Effect.provide(fullLayer)));
