import { makeReferenceAdapter } from "@openagentsinc/agent-harness-contract";
import { Effect, Stream } from "effect";

import {
  makeOpenAgentsCloudHarnessEnvironment,
  startHarnessInEnvironment,
} from "./contract.js";
import { makeOpenAgentsCloudHarnessEnvironmentRunner } from "./openagents-cloud-runner.js";

/**
 * Non-test production caller for HE-01.
 *
 * Constructs a typed `openagents_cloud` environment, installs the real
 * {@link makeOpenAgentsCloudHarnessEnvironmentRunner}, starts a harness
 * session, and drives one prompt turn through `POST /v1/cloud-coding-sessions`.
 *
 * Credentials and repo selection come from the call site / environment
 * variables — never from the published environment contract.
 */
export interface RunOpenAgentsCloudHarnessInput {
  readonly controlPlaneBaseUrl: string;
  readonly bearerToken: string;
  readonly repoRef: string;
  readonly objective: string;
  readonly sessionId?: string;
  readonly turnId?: string;
  readonly lane?: "cloud-gcp";
  readonly adapter?: "codex" | "claude_agent";
  readonly repoTrustTier?: "public" | "private" | "regulated";
  readonly verify?: ReadonlyArray<string>;
  readonly timeoutSeconds?: number;
  readonly fetch?: typeof globalThis.fetch;
  readonly pollIntervalMs?: number;
  readonly maxPollAttempts?: number;
}

export interface RunOpenAgentsCloudHarnessResult {
  readonly sessionId: string;
  readonly turnId: string;
  readonly finishReason: string;
  readonly lastCursor: number;
  readonly eventKinds: ReadonlyArray<string>;
}

export const runOpenAgentsCloudHarness = Effect.fn(
  "HarnessEnvironment.runOpenAgentsCloudHarness",
)(function* (input: RunOpenAgentsCloudHarnessInput) {
  const environment = makeOpenAgentsCloudHarnessEnvironment(input.controlPlaneBaseUrl);
  const runner = makeOpenAgentsCloudHarnessEnvironmentRunner({
    bearerToken: input.bearerToken,
    repoRef: input.repoRef,
    ...(input.lane === undefined ? {} : { lane: input.lane }),
    ...(input.adapter === undefined ? {} : { adapter: input.adapter }),
    ...(input.repoTrustTier === undefined
      ? {}
      : { repoTrustTier: input.repoTrustTier }),
    ...(input.verify === undefined ? {} : { verify: input.verify }),
    ...(input.timeoutSeconds === undefined
      ? {}
      : { timeoutSeconds: input.timeoutSeconds }),
    ...(input.fetch === undefined ? {} : { fetch: input.fetch }),
    ...(input.pollIntervalMs === undefined
      ? {}
      : { pollIntervalMs: input.pollIntervalMs }),
    ...(input.maxPollAttempts === undefined
      ? {}
      : { maxPollAttempts: input.maxPollAttempts }),
  });

  // Local reference adapter is only a session-start placeholder; cloud turns
  // do not execute it. Callers that already own an AgentHarness may replace
  // this later without changing the runner.
  const harness = makeReferenceAdapter({
    harnessId: "openagents-cloud-caller",
    scriptWords: ["unused"],
  });

  const sessionId = input.sessionId ?? `session.cloud.${Date.now().toString(36)}`;
  const turnId = input.turnId ?? `turn.cloud.${Date.now().toString(36)}`;

  const session = yield* startHarnessInEnvironment({
    environment,
    harness,
    options: {
      sessionId,
      source: {
        lane: "managed_cloud",
        adapterKind: "openagents_native",
        surface: "server",
      },
    },
    runners: { openagentsCloud: runner },
  });

  const control = yield* session.promptTurn({
    turnId,
    prompt: input.objective,
  });
  const events = yield* Stream.runCollect(control.events);
  const done = yield* control.done;

  return {
    sessionId: session.sessionId,
    turnId: control.turnId,
    finishReason: done.finishReason,
    lastCursor: done.lastCursor,
    eventKinds: events.map((event) => event.kind),
  } satisfies RunOpenAgentsCloudHarnessResult;
});

const readRequiredEnv = (name: string): string => {
  const value = process.env[name]?.trim();
  if (value === undefined || value === "") {
    throw new Error(`Missing required environment variable ${name}.`);
  }
  return value;
};

/**
 * CLI entry for the production caller.
 *
 * Required env:
 * - `OPENAGENTS_CLOUD_CONTROL_PLANE_BASE_URL`
 * - `OPENAGENTS_CLOUD_BEARER_TOKEN`
 * - `OPENAGENTS_CLOUD_REPO_REF`
 * - `OPENAGENTS_CLOUD_OBJECTIVE`
 */
export const main = async (): Promise<void> => {
  const result = await Effect.runPromise(
    runOpenAgentsCloudHarness({
      controlPlaneBaseUrl: readRequiredEnv("OPENAGENTS_CLOUD_CONTROL_PLANE_BASE_URL"),
      bearerToken: readRequiredEnv("OPENAGENTS_CLOUD_BEARER_TOKEN"),
      repoRef: readRequiredEnv("OPENAGENTS_CLOUD_REPO_REF"),
      objective: readRequiredEnv("OPENAGENTS_CLOUD_OBJECTIVE"),
      ...(process.env.OPENAGENTS_CLOUD_ADAPTER === "claude_agent" ||
      process.env.OPENAGENTS_CLOUD_ADAPTER === "codex"
        ? { adapter: process.env.OPENAGENTS_CLOUD_ADAPTER }
        : {}),
      ...(process.env.OPENAGENTS_CLOUD_REPO_TRUST_TIER === "public" ||
      process.env.OPENAGENTS_CLOUD_REPO_TRUST_TIER === "private" ||
      process.env.OPENAGENTS_CLOUD_REPO_TRUST_TIER === "regulated"
        ? { repoTrustTier: process.env.OPENAGENTS_CLOUD_REPO_TRUST_TIER }
        : {}),
    }),
  );
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
};

const invokedAsCli =
  typeof process.argv[1] === "string" &&
  /run-openagents-cloud-harness\.(ts|js|mjs|cjs)$/u.test(process.argv[1]);

if (invokedAsCli) {
  main().catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  });
}
