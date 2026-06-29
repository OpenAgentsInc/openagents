// Run ONE scenario across N targets (#6190, Rhys req #3).
//
// "The same (or near-same) test runs against multiple targets — 'test the dev
// server,' 'test production' — without rewriting it." This is the multi-target
// run driver: given a scenario DEFINED ONCE (a fresh-brain factory + optional
// commitments) and a SELECTION of registry targets, it runs the SAME scenario
// against each target and assembles a per-target result + video.
//
// HONEST + ADDITIVE:
//   - It reuses `runQaSession` unchanged (the target-selection is a READ on the
//     runner; the restriction enforcement already lives in the runner's drive
//     loop). A read-only target that the scenario tries to mutate fails per
//     target — visibly, never silently.
//   - The brain is created FRESH per target (a brain is stateful — it must not be
//     shared across runs), exactly like the eval harness does per variant.
//   - The persisted matrix is tripwire-checked (`assertPublicSafeResult`) before
//     write — no prompts/tokens/secrets.
//   - PUBLIC-SAFE: only per-target status/duration/target-name/baseUrl/video +
//     the verify verdict, mirroring the run read model the /pro page renders.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Effect } from "effect";
import type { Backend } from "./backend";
import type { Brain } from "./brain";
import { assertPublicSafeResult, type QaVerifyVerdict } from "./result";
import { runQaSession } from "./runner";
import { isReadOnly, type Target } from "./target";
import type { Commitment } from "./verify";

export const TARGET_MATRIX_SCHEMA_VERSION =
  "openagents.qa_runner.target_matrix.v1" as const;

// A scenario defined ONCE, run against every selected target with no rewrite.
export interface MultiTargetScenario {
  /** Stable, URL-safe id, e.g. "login-regression". */
  readonly id: string;
  readonly title: string;
  /** Fresh Brain per target (the decision-maker — stateful, never shared). */
  readonly brain: () => Brain;
  /** Fresh Backend per target (isolation; usually shared-fixture in CI). */
  readonly backend: () => Backend;
  /** Optional commitments checked per target (the verify stage, #6192). */
  readonly commitments?: ReadonlyArray<Commitment>;
}

export interface MultiTargetRunInput {
  readonly scenario: MultiTargetScenario;
  /** The targets to run the scenario against (resolved from the registry). */
  readonly targets: ReadonlyArray<Target>;
  /** Root directory; each target gets a subdir for its artifacts. */
  readonly artifactDir: string;
  readonly maxSteps?: number;
  readonly headed?: boolean;
  /** Injectable clock (deterministic timestamps in tests). */
  readonly now?: () => Date;
}

// One target's public-safe footprint in the matrix: status + duration + target
// identity + the artifact dir (relative) so a viewer can dereference video/result.
export interface TargetRunRef {
  readonly targetName: string;
  readonly targetBaseUrl: string;
  /** True when the target declared the `read-only` restriction (#6190). */
  readonly readOnly: boolean;
  readonly status: "pass" | "fail";
  readonly durationMs: number;
  /** Relative path (from the matrix root) to this target's artifact dir. */
  readonly artifactDir: string;
  readonly video?: string;
  readonly videoFormat?: "mp4" | "webm";
  /** Honest one-line failure summary when status is "fail" (incl. a restriction
   *  refusal on a read-only target — never a silent skip). */
  readonly failure?: string;
  /** The verify investigator verdict, when commitments were declared (#6192). */
  readonly verdict?: QaVerifyVerdict;
}

export interface TargetMatrixResult {
  readonly schemaVersion: typeof TARGET_MATRIX_SCHEMA_VERSION;
  readonly scenarioId: string;
  readonly title: string;
  readonly startedAt: string;
  readonly endedAt: string;
  readonly targets: ReadonlyArray<TargetRunRef>;
  /** passes / total across the selected targets, in [0,1]. */
  readonly passRate: number;
  readonly passCount: number;
  readonly targetCount: number;
}

export interface MultiTargetOutcome {
  readonly result: TargetMatrixResult;
  readonly resultPath: string;
}

/**
 * Run the scenario against every selected target, one per subdir, and assemble
 * the per-target matrix. Honest: each target's pass/fail is its real run
 * outcome; a read-only restriction refusal surfaces as that target's failure.
 */
export function runScenarioAcrossTargets(
  input: MultiTargetRunInput,
): Effect.Effect<MultiTargetOutcome, Error> {
  const now = input.now ?? (() => new Date());
  return Effect.gen(function* () {
    if (input.targets.length === 0) {
      return yield* Effect.fail(
        new Error("a multi-target run needs >= 1 target; got 0"),
      );
    }
    mkdirSync(input.artifactDir, { recursive: true });
    const startedAt = now();

    const refs: TargetRunRef[] = [];
    for (const target of input.targets) {
      const subdir = target.name;
      const runDir = join(input.artifactDir, subdir);
      const outcome = yield* runQaSession({
        target,
        brain: input.scenario.brain(),
        backend: input.scenario.backend(),
        artifactDir: runDir,
        ...(input.maxSteps !== undefined ? { maxSteps: input.maxSteps } : {}),
        ...(input.headed !== undefined ? { headed: input.headed } : {}),
        ...(input.scenario.commitments !== undefined
          ? { commitments: input.scenario.commitments }
          : {}),
        now,
      });
      const r = outcome.result;
      refs.push({
        targetName: target.name,
        targetBaseUrl: target.baseUrl,
        readOnly: isReadOnly(target),
        status: r.status,
        durationMs: r.durationMs,
        artifactDir: subdir,
        ...(r.artifacts.video !== undefined
          ? { video: join(subdir, r.artifacts.video) }
          : {}),
        ...(r.artifacts.videoFormat !== undefined
          ? { videoFormat: r.artifacts.videoFormat }
          : {}),
        ...(r.failure !== undefined ? { failure: r.failure } : {}),
        ...(r.verify !== undefined ? { verdict: r.verify.verdict } : {}),
      });
    }

    const endedAt = now();
    const passCount = refs.filter((t) => t.status === "pass").length;
    const result: TargetMatrixResult = {
      schemaVersion: TARGET_MATRIX_SCHEMA_VERSION,
      scenarioId: input.scenario.id,
      title: input.scenario.title,
      startedAt: startedAt.toISOString(),
      endedAt: endedAt.toISOString(),
      targets: refs,
      passRate: refs.length === 0 ? 0 : passCount / refs.length,
      passCount,
      targetCount: refs.length,
    };

    // Tripwire: never persist a matrix that leaks a forbidden field.
    assertPublicSafeResult(result);

    const resultPath = join(input.artifactDir, "target-matrix.json");
    writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);
    return { result, resultPath };
  });
}
