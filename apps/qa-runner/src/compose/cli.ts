#!/usr/bin/env bun
// `compose` CLI — render a polished, shareable mp4 from a completed run dir.
//
//   bun run --cwd apps/qa-runner compose -- --run <dir> --out <mp4>
//   bun run --cwd apps/qa-runner compose -- --before <dir> --after <dir> --out <mp4>
//
// Flags:
//   --run <dir>      single-run layout source directory
//   --before <dir>   before-variant directory (with --after)
//   --after <dir>    after-variant directory  (with --before)
//   --out <mp4>      output path (default: <run>/compose.mp4 or ./compose.mp4)
//   --brand <text>   brand wordmark (default "OpenAgents")
//   --plan-only      print the resolved ComposePlan JSON and exit (no render)
//
// The CLI is a thin shell over the pure planner (`buildComposePlan`) and the
// ffmpeg executor (`renderComposePlan`). It does no per-video bespoke logic.

import { buildComposePlan } from "./build-plan.ts";
import { renderComposePlan, type RunDirs } from "./ffmpeg.ts";
import { loadRunMeta } from "./load.ts";
import type { ComposeInput } from "./plan.ts";

interface Args {
  run?: string;
  before?: string;
  after?: string;
  out?: string;
  brand?: string;
  planOnly: boolean;
}

function parseArgs(argv: ReadonlyArray<string>): Args {
  const args: Args = { planOnly: false };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    switch (a) {
      case "--run":
        args.run = argv[++i];
        break;
      case "--before":
        args.before = argv[++i];
        break;
      case "--after":
        args.after = argv[++i];
        break;
      case "--out":
        args.out = argv[++i];
        break;
      case "--brand":
        args.brand = argv[++i];
        break;
      case "--plan-only":
        args.planOnly = true;
        break;
      default:
        throw new Error(`compose: unknown flag "${a}"`);
    }
  }
  return args;
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));

  const isPair = args.before !== undefined || args.after !== undefined;
  if (isPair && (args.before === undefined || args.after === undefined)) {
    throw new Error("compose: --before and --after must be provided together");
  }
  if (!isPair && args.run === undefined) {
    throw new Error("compose: provide --run <dir>, or --before <dir> --after <dir>");
  }

  const style = args.brand !== undefined ? { brand: args.brand } : undefined;
  let input: ComposeInput;
  const dirs: RunDirs = {};

  if (isPair) {
    const beforeDir = args.before as string;
    const afterDir = args.after as string;
    const before = loadRunMeta(beforeDir, "Before");
    const after = loadRunMeta(afterDir, "After");
    dirs.before = beforeDir;
    dirs.after = afterDir;
    input = { before, after, ...(style !== undefined ? { style } : {}) };
  } else {
    const runDir = args.run as string;
    const single = loadRunMeta(runDir);
    dirs.run = runDir;
    input = { single, ...(style !== undefined ? { style } : {}) };
  }

  const plan = buildComposePlan(input);

  if (args.planOnly) {
    process.stdout.write(`${JSON.stringify(plan, null, 2)}\n`);
    return;
  }

  const out =
    args.out ??
    (args.run !== undefined ? `${args.run.replace(/\/$/, "")}/compose.mp4` : "compose.mp4");

  process.stderr.write(`compose: rendering ${plan.layout} plan → ${out}\n`);
  const result = await renderComposePlan(plan, dirs, out);
  if (!result.drawText) {
    process.stderr.write(
      "compose: NOTE this ffmpeg build lacks the drawtext filter (no libfreetype); " +
        "text overlays rendered as colored pills/badges only. " +
        "Install a libfreetype-enabled ffmpeg for full text overlays.\n",
    );
  }
  process.stdout.write(`${result.outPath}\n`);
}

main().catch((err: unknown) => {
  const message = err instanceof Error ? err.message : String(err);
  process.stderr.write(`compose: ${message}\n`);
  process.exit(1);
});
