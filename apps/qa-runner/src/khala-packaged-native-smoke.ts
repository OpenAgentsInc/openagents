#!/usr/bin/env bun
// Headed packaged-app AX smoke for Khala Code.
//
// This launches the built Electrobun `.app`, arms the native macOS AX backend
// through QA_NATIVE_DESKTOP, drives the actual owner-facing window against the
// fixture backend, and writes screenshots + result artifacts.

import { resolve } from "node:path";

import { runKhalaDesktopHeadedNativeSmoke } from "./khala-desktop-backend";
import { makeTarget } from "./target";

interface Args {
  readonly appProcessName?: string;
  readonly appPath?: string;
  readonly baselineDir?: string;
  readonly blessBaselines?: boolean;
  readonly composerSelector?: string;
  readonly help?: boolean;
  readonly hotbarSelector?: string;
  readonly out?: string;
  readonly prompt?: string;
  readonly requireBaselines?: boolean;
  readonly sendSelector?: string;
}

function parseArgs(argv: ReadonlyArray<string>): Args {
  const args: Record<string, string | boolean> = {};
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]!;
    if (arg === "--") continue;
    if (arg === "--help" || arg === "-h") {
      args.help = true;
      continue;
    }
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[i + 1];
    if (next !== undefined && !next.startsWith("--")) {
      args[key] = next;
      i++;
    } else {
      args[key] = true;
    }
  }
  return {
    ...(typeof args["app-path"] === "string" ? { appPath: args["app-path"] } : {}),
    ...(typeof args["app-process-name"] === "string" ? { appProcessName: args["app-process-name"] } : {}),
    ...(typeof args["baseline-dir"] === "string" ? { baselineDir: args["baseline-dir"] } : {}),
    ...(args["bless-baselines"] === true ? { blessBaselines: true } : {}),
    ...(typeof args["composer-selector"] === "string" ? { composerSelector: args["composer-selector"] } : {}),
    ...(args.help === true ? { help: true } : {}),
    ...(typeof args["hotbar-selector"] === "string" ? { hotbarSelector: args["hotbar-selector"] } : {}),
    ...(typeof args.out === "string" ? { out: args.out } : {}),
    ...(typeof args.prompt === "string" ? { prompt: args.prompt } : {}),
    ...(args["require-baselines"] === true ? { requireBaselines: true } : {}),
    ...(typeof args["send-selector"] === "string" ? { sendSelector: args["send-selector"] } : {}),
  };
}

const defaultVisualBaselineDir = (): string =>
  resolve(import.meta.dir, "../../../clients/khala-code-desktop/visual-baselines");

function usage(): void {
  console.log(`Usage:
  QA_NATIVE_DESKTOP=1 bun run src/khala-packaged-native-smoke.ts -- --out ./runs/khala-packaged-native

Options:
  --out <dir>                  Artifact directory (default: ./runs/khala-packaged-native)
  --app-path <path>            Explicit Khala Code.app path; otherwise auto-discovered under clients/khala-code-desktop
  --app-process-name <name>    macOS AX process name (default: bundle name without .app)
  --baseline-dir <dir>         Visual baseline store (default: clients/khala-code-desktop/visual-baselines)
  --bless-baselines            Copy screenshots into the baseline store and update manifest.json
  --require-baselines          Fail when a screenshot has no blessed baseline entry
  --hotbar-selector <selector> AX selector or point:x,y fallback for the hotbar target
  --composer-selector <sel>    AX selector or point:x,y fallback for the composer
  --send-selector <selector>   AX selector or point:x,y fallback for send
  --prompt <text>              Fixture prompt text (raw text is never written to result.json)
`);
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  if (args.help === true) {
    usage();
    return;
  }

  const artifactDir = resolve(args.out ?? "./runs/khala-packaged-native");
  const baselineDir = resolve(args.baselineDir ?? process.env.KHALA_CODE_VISUAL_BASELINE_DIR ?? defaultVisualBaselineDir());
  const selectors = {
    ...(args.hotbarSelector === undefined ? {} : { hotbar: args.hotbarSelector }),
    ...(args.composerSelector === undefined ? {} : { composer: args.composerSelector }),
    ...(args.sendSelector === undefined ? {} : { send: args.sendSelector }),
  };
  const outcome = await runKhalaDesktopHeadedNativeSmoke({
    artifactDir,
    ...(args.appProcessName === undefined ? {} : { appProcessName: args.appProcessName }),
    ...(args.appPath === undefined ? {} : { appPath: args.appPath }),
    ...(args.prompt === undefined ? {} : { promptText: args.prompt }),
    ...(Object.keys(selectors).length === 0 ? {} : { selectors }),
    target: makeTarget({ name: "khala-code-packaged", baseUrl: "khala-desktop://packaged" }),
    visualBaseline: {
      baselineDir,
      bless: args.blessBaselines === true || process.env.KHALA_CODE_VISUAL_BASELINE_BLESS === "1",
      requireBaseline:
        args.requireBaselines === true ||
        process.env.KHALA_CODE_VISUAL_BASELINE_REQUIRE === "1",
    },
  });

  console.log("=== Khala Code packaged native smoke ===");
  console.log("status:", outcome.result.status);
  console.log("result:", outcome.resultPath);
  console.log("smoke report:", outcome.smokeReportPath);
  console.log("screenshots:", JSON.stringify(outcome.result.artifacts.screenshots));
  console.log("visual baselines:", JSON.stringify(outcome.visualBaselines.map((result) => ({
    id: result.id,
    status: result.status,
    delta: result.delta,
  }))));
  if (outcome.result.failure) console.log("failure:", outcome.result.failure);
  process.exit(outcome.result.status === "pass" ? 0 : 1);
}

if (import.meta.main) {
  await main();
}
