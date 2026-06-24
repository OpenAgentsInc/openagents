// Loader: read a completed run directory into the public-safe ComposeRunMeta
// the planner consumes. This is the ONLY part of the compose layer that touches
// the filesystem for inputs; it stays thin and side-effect-isolated so
// `buildComposePlan` can remain pure.
//
// A run directory is expected to contain:
//   - result.json        (openagents.qa_runner.result.v1)
//   - session-trace.json (openagents.khala.session_trace.v1)  [optional]
//   - session.mp4        (the raw clip)
//   - 00-*.png           (per-step screenshots)

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { assertPublicSafeResult } from "../result.ts";
import type { ComposeRunMeta, ComposeStep, ComposeVerdict } from "./plan.ts";

interface RawResultStep {
  index: number;
  kind: string;
  label: string;
  status: "ok" | "failed";
}

interface RawResult {
  status: "pass" | "fail";
  target: { name: string; baseUrl: string };
  brain: string;
  durationMs: number;
  steps: ReadonlyArray<RawResultStep>;
  artifacts: {
    video?: string;
    screenshots: ReadonlyArray<string>;
  };
}

interface RawTrace {
  goal?: string;
}

function readJson<T>(path: string): T {
  return JSON.parse(readFileSync(path, "utf8")) as T;
}

/**
 * Read a run directory into ComposeRunMeta. `variantLabel` tags the run for a
 * before/after layout (e.g. "Before" / "After"). The result is run through the
 * public-safety tripwire before it is returned.
 */
export function loadRunMeta(
  runDir: string,
  variantLabel?: string,
): ComposeRunMeta {
  const result = readJson<RawResult>(join(runDir, "result.json"));

  let goal: string | undefined;
  try {
    const trace = readJson<RawTrace>(join(runDir, "session-trace.json"));
    goal = trace.goal;
  } catch {
    // session-trace.json is optional; absence just drops the subtitle.
    goal = undefined;
  }

  const verdict: ComposeVerdict = result.status === "pass" ? "pass" : "fail";
  const steps: ComposeStep[] = result.steps.map((s) => ({
    index: s.index,
    kind: s.kind,
    label: s.label,
    status: s.status,
  }));

  const meta: ComposeRunMeta = {
    scenarioTitle: result.target.name,
    ...(goal !== undefined ? { goal } : {}),
    verdict,
    targetName: result.target.name,
    targetBaseUrl: result.target.baseUrl,
    brain: result.brain,
    durationMs: result.durationMs,
    steps,
    video: result.artifacts.video ?? "session.mp4",
    screenshots: [...result.artifacts.screenshots],
    ...(variantLabel !== undefined ? { variantLabel } : {}),
  };

  // Tripwire: the metadata we are about to overlay must be public-safe.
  assertPublicSafeResult(meta);
  return meta;
}
