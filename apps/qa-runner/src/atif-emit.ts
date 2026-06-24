#!/usr/bin/env bun
// ATIF emit CLI (epic #6174): turn a completed Khala run directory into the hero
// deliverable — ONE beautiful ATIF-v1.7 `trajectory.json` + a self-contained,
// beautiful `trace.html` (dark, command-surface timeline; plays the video).
//
// Reads result.json + session-trace.json from a run dir, maps them to ATIF,
// VALIDATES the trajectory (a trajectory that does not validate is never
// written), renders the HTML, and writes both into an output dir (defaults to
// the run dir). The video + screenshots are referenced by RELATIVE path, so copy
// session.mp4 + *.png alongside the emitted files for an in-place viewer.
//
// Usage:
//   bun run src/atif-emit.ts --run ./runs/login --out ./samples/login-trace \
//     [--session-id login-trace] [--title "..."]

import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { mapKhalaRunToAtif, serializeTrajectory } from "./atif";
import { renderTraceHtml } from "./atif-html";
import { assertValidAtif } from "./atif-validate";
import { decodeSessionTrace } from "./session-trace";
import { decodeQaRunResult } from "./result";

function parseArgs(argv: ReadonlyArray<string>): Record<string, string> {
  const args: Record<string, string> = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a.startsWith("--")) {
      const next = argv[i + 1];
      if (next && !next.startsWith("--")) {
        args[a.slice(2)] = next;
        i++;
      } else args[a.slice(2)] = "true";
    }
  }
  return args;
}

export interface EmitAtifInput {
  readonly runDir: string;
  readonly outDir: string;
  readonly sessionId?: string;
  readonly title?: string;
  /** Copy session.mp4 + *.png from runDir into outDir (default true). */
  readonly copyArtifacts?: boolean;
}

export interface EmitAtifOutput {
  readonly trajectoryPath: string;
  readonly htmlPath: string;
  readonly verdict: string;
  readonly stepCount: number;
}

/** Emit trajectory.json + trace.html from a run directory. Pure-ish (FS I/O). */
export function emitAtif(input: EmitAtifInput): EmitAtifOutput {
  const resultPath = join(input.runDir, "result.json");
  const tracePath = join(input.runDir, "session-trace.json");
  if (!existsSync(resultPath)) throw new Error(`missing result.json at ${resultPath}`);
  if (!existsSync(tracePath)) throw new Error(`missing session-trace.json at ${tracePath}`);

  const result = decodeQaRunResult(JSON.parse(readFileSync(resultPath, "utf8")));
  const trace = decodeSessionTrace(JSON.parse(readFileSync(tracePath, "utf8")));

  const sessionId = input.sessionId ?? basename(input.runDir);
  const trajectory = mapKhalaRunToAtif({ result, trace, sessionId });

  // Tripwire: never write an invalid trajectory.
  assertValidAtif(trajectory);

  mkdirSync(input.outDir, { recursive: true });

  const trajectoryPath = join(input.outDir, "trajectory.json");
  writeFileSync(trajectoryPath, serializeTrajectory(trajectory));

  // Copy artifacts so the relative-path HTML viewer renders + plays in place.
  if (input.copyArtifacts !== false) {
    const video = result.artifacts.video;
    if (video && existsSync(join(input.runDir, video))) {
      copyFileSync(join(input.runDir, video), join(input.outDir, video));
    }
    for (const shot of result.artifacts.screenshots) {
      if (existsSync(join(input.runDir, shot))) {
        copyFileSync(join(input.runDir, shot), join(input.outDir, shot));
      }
    }
  }

  const html = renderTraceHtml(trajectory, input.title ? { title: input.title } : {});
  const htmlPath = join(input.outDir, "trace.html");
  writeFileSync(htmlPath, html);

  const verdict = String(trajectory.final_metrics?.extra?.verdict ?? "INCONCLUSIVE");
  return { trajectoryPath, htmlPath, verdict, stepCount: trajectory.steps.length };
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const runDir = args.run;
  if (!runDir) {
    console.error("usage: bun run src/atif-emit.ts --run <runDir> [--out <outDir>] [--session-id <id>] [--title <t>]");
    process.exit(2);
  }
  const outDir = args.out ?? runDir;
  const out = emitAtif({
    runDir,
    outDir,
    ...(args["session-id"] ? { sessionId: args["session-id"] } : {}),
    ...(args.title ? { title: args.title } : {}),
  });
  console.log("=== ATIF emit (epic #6174) ===");
  console.log("verdict:    ", out.verdict);
  console.log("steps:      ", out.stepCount);
  console.log("trajectory: ", out.trajectoryPath);
  console.log("html:       ", out.htmlPath);
}

if (import.meta.main) {
  await main();
}
