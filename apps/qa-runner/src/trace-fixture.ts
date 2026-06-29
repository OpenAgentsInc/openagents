import { readFileSync, writeFileSync } from "node:fs";

import { redactValue, type RedactionReport } from "./redaction";
import {
  type AtifStep,
  type AtifToolCall,
  type AtifTrajectory,
} from "./atif";
import { assertValidAtif } from "./atif-validate";

export const TRACE_FIXTURE_SCHEMA_VERSION =
  "openagents.qa_runner.trace_fixture.v1" as const;

export interface TraceFixtureToolCall {
  readonly name: string;
  readonly arguments: Record<string, unknown>;
}

export interface TraceFixtureCase {
  readonly id: string;
  readonly stepId: number;
  readonly source: "user" | "agent" | "system";
  readonly input: {
    readonly message: string;
    readonly reasoning?: string;
    readonly toolCalls?: ReadonlyArray<TraceFixtureToolCall>;
  };
  readonly output: {
    readonly observations: ReadonlyArray<string>;
    readonly status: "ok" | "failed" | "unknown";
  };
}

export interface TraceFixture {
  readonly schemaVersion: typeof TRACE_FIXTURE_SCHEMA_VERSION;
  readonly evidenceRef: string;
  readonly trajectoryId: string;
  readonly sessionId?: string;
  readonly agent: {
    readonly name: string;
    readonly version: string;
    readonly model?: string;
  };
  readonly cases: ReadonlyArray<TraceFixtureCase>;
  readonly redaction: RedactionReport;
}

export interface GenerateTraceFixtureInput {
  readonly evidenceRef: string;
  readonly trajectory: AtifTrajectory;
}

const normalizeToolCall = (call: AtifToolCall): TraceFixtureToolCall => ({
  name: call.function_name,
  arguments: call.arguments,
});

const observationContents = (step: AtifStep): ReadonlyArray<string> =>
  (step.observation?.results ?? [])
    .map((result) => result.content)
    .filter((content): content is string => typeof content === "string");

const statusFromObservations = (
  observations: ReadonlyArray<string>,
): TraceFixtureCase["output"]["status"] => {
  if (observations.length === 0) return "unknown";
  return observations.some((content) => /^FAILED\b/i.test(content)) ? "failed" : "ok";
};

const caseForStep = (
  trajectoryId: string,
  step: AtifStep,
): TraceFixtureCase | undefined => {
  const observations = observationContents(step);
  const toolCalls = (step.tool_calls ?? []).map(normalizeToolCall);

  if (step.message === "" && toolCalls.length === 0 && observations.length === 0) {
    return undefined;
  }

  const input: TraceFixtureCase["input"] = {
    message: step.message,
    ...(step.reasoning_content ? { reasoning: step.reasoning_content } : {}),
    ...(toolCalls.length > 0 ? { toolCalls } : {}),
  };

  return {
    id: `${trajectoryId}:step-${step.step_id}`,
    stepId: step.step_id,
    source: step.source,
    input,
    output: {
      observations,
      status: statusFromObservations(observations),
    },
  };
};

/**
 * Extract a public-safe reproduction fixture from an ATIF trace evidence ref.
 *
 * The trace is first validated against the ATIF producer contract, then the
 * generated fixture is passed through the shared trace redactor. This keeps the
 * fixture usable for regression tests while preventing raw prompts, paths,
 * tokens, credentials, emails, and wallet material from becoming committed test
 * data.
 */
export const generateTraceFixture = (
  input: GenerateTraceFixtureInput,
): TraceFixture => {
  assertValidAtif(input.trajectory);

  const trajectoryId = input.trajectory.trajectory_id ?? input.trajectory.session_id ?? "trace";
  const rawFixture: Omit<TraceFixture, "redaction"> = {
    schemaVersion: TRACE_FIXTURE_SCHEMA_VERSION,
    evidenceRef: input.evidenceRef,
    trajectoryId,
    ...(input.trajectory.session_id ? { sessionId: input.trajectory.session_id } : {}),
    agent: {
      name: input.trajectory.agent.name,
      version: input.trajectory.agent.version,
      ...(input.trajectory.agent.model_name
        ? { model: input.trajectory.agent.model_name }
        : {}),
    },
    cases: input.trajectory.steps
      .map((step) => caseForStep(trajectoryId, step))
      .filter((fixtureCase): fixtureCase is TraceFixtureCase => fixtureCase !== undefined),
  };

  const redacted = redactValue(rawFixture);
  return {
    ...redacted.value,
    redaction: redacted.report,
  };
};

export const loadTraceFixtureFromFile = (
  path: string,
  options: { readonly evidenceRef?: string } = {},
): TraceFixture => {
  const trajectory = JSON.parse(readFileSync(path, "utf8")) as AtifTrajectory;
  return generateTraceFixture({
    evidenceRef: options.evidenceRef ?? path,
    trajectory,
  });
};

const usage = (): string =>
  [
    "Usage: bun run src/trace-fixture.ts <atif-trajectory.json> [--out fixture.json] [--ref evidence-ref]",
    "",
    "Reads a public-safe ATIF trajectory and writes a redacted reproduction fixture.",
  ].join("\n");

const runCli = (): void => {
  const args = process.argv.slice(2);
  const inputPath = args[0];
  if (!inputPath || inputPath === "--help" || inputPath === "-h") {
    console.log(usage());
    process.exit(inputPath ? 0 : 1);
  }

  const outIndex = args.indexOf("--out");
  const refIndex = args.indexOf("--ref");
  const outPath = outIndex >= 0 ? args[outIndex + 1] : undefined;
  const evidenceRef = refIndex >= 0 ? args[refIndex + 1] : undefined;
  if (outIndex >= 0 && !outPath) {
    throw new Error("--out requires a file path");
  }
  if (refIndex >= 0 && !evidenceRef) {
    throw new Error("--ref requires an evidence ref");
  }

  const fixture = loadTraceFixtureFromFile(
    inputPath,
    evidenceRef ? { evidenceRef } : {},
  );
  const body = `${JSON.stringify(fixture, null, 2)}\n`;
  if (outPath) {
    writeFileSync(outPath, body);
    return;
  }
  process.stdout.write(body);
};

if (import.meta.main) {
  runCli();
}
