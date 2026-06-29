#!/usr/bin/env bun
import { Effect } from "effect";
import {
  PSIONIC_QWEN_MODEL_REFS,
  defineProbeLlmTool,
  makeProbeLlmRequest,
  makePsionicQwenClient,
  probeLlmToolDefinitions,
  type PsionicQwenReadiness,
} from "../packages/runtime/src/index";

type SmokeState = "passed" | "blocked" | "failed";

type SmokeOutput = {
  schema: "openagents.pylon.psionic_qwen_live_smoke.v0.3";
  state: SmokeState;
  blockerRefs: string[];
  baseUrlSource?: string;
  model: string;
  doctor?: {
    status: string;
    ready: boolean;
    modelRefs: string[];
    observedModelRefs: string[];
    supportedEndpointRefs: string[];
    blockerRefs: string[];
    receipt: unknown;
  };
  plainInference?: {
    text: string;
    roundTrips: number;
    receipt: unknown;
  };
  toolRoundTrip?: {
    text: string;
    roundTrips: number;
    toolReceiptCount: number;
    receipt: unknown;
    toolReceipts: unknown[];
  };
  failureClass?: string;
  message?: string;
  redacted: true;
};

const args = parseArgs(Bun.argv.slice(2));
const model = stringArg(args, "model") ?? "qwen3.5-0.8b";

const output = await Effect.runPromise(runSmoke({ args, model }));
process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
process.exitCode = output.state === "passed" ? 0 : 1;

function runSmoke(input: {
  readonly args: Record<string, string | true>;
  readonly model: string;
}): Effect.Effect<SmokeOutput, never> {
  return Effect.gen(function* () {
    const client = yield* makePsionicQwenClient({
      explicitBaseUrl: stringArg(input.args, "base-url"),
      env: Bun.env,
    }).pipe(
      Effect.catch((error) =>
        Effect.succeed({
          _tag: "ClientError" as const,
          reason: error.reason,
        }),
      ),
    );

    if ("_tag" in client) {
      return baseOutput(input.model, "failed", {
        blockerRefs: ["blocker.psionic_qwen35.connector_unconfigured"],
        failureClass: "client_configuration",
        message: client.reason,
      });
    }

    const readiness = yield* client.doctor();
    const doctor = {
      status: readiness.status,
      ready: readiness.ready,
      modelRefs: [...readiness.modelRefs],
      observedModelRefs: [...readiness.observedModelRefs],
      supportedEndpointRefs: [...readiness.supportedEndpointRefs],
      blockerRefs: [...readiness.blockerRefs],
      receipt: readiness.receipt,
    };
    const doctorBlockers = doctorBlockerRefs(readiness);

    if (doctorBlockers.length > 0) {
      return baseOutput(input.model, "blocked", {
        blockerRefs: doctorBlockers,
        baseUrlSource: client.profile.baseUrlSource,
        doctor,
      });
    }

    const plain = yield* client.complete({
      request: makeProbeLlmRequest({
        model: { provider: "psionic", model: input.model },
        prompt: "Reply with exactly: psionic qwen smoke ok",
        generation: { maxTokens: 32, temperature: 0 },
      }),
      maxModelRoundTrips: 1,
    }).pipe(Effect.catch((error) => Effect.succeed(error)));

    if ("_tag" in plain) {
      return baseOutput(input.model, "failed", {
        blockerRefs: ["blocker.psionic_qwen35.chat_completion_failed"],
        baseUrlSource: client.profile.baseUrlSource,
        doctor,
        failureClass: plain.failureClass,
        message: plain.reason,
      });
    }

    const echo = defineProbeLlmTool({
      name: "echo_public_ref",
      description: "Echo a public reference for smoke testing.",
      inputSchema: {
        type: "object",
        properties: {
          ref: { type: "string" },
        },
        required: ["ref"],
      },
      execute: (toolInput) => Effect.succeed({ ref: toolInput.ref, ok: true }),
    });
    const tool = yield* client.complete({
      request: makeProbeLlmRequest({
        model: { provider: "psionic", model: input.model },
        prompt: "Call echo_public_ref once with ref set to receipt.psionic_qwen.live_smoke, then summarize the public ref.",
        generation: { maxTokens: 128, temperature: 0 },
        tools: probeLlmToolDefinitions({ echo_public_ref: echo }),
        toolChoice: { type: "required" },
      }),
      tools: { echo_public_ref: echo },
      maxModelRoundTrips: 3,
    }).pipe(Effect.catch((error) => Effect.succeed(error)));

    if ("_tag" in tool) {
      return baseOutput(input.model, "failed", {
        blockerRefs: ["blocker.psionic_qwen35.tool_call_failed"],
        baseUrlSource: client.profile.baseUrlSource,
        doctor,
        plainInference: {
          text: plain.text,
          roundTrips: plain.roundTrips,
          receipt: plain.receipt,
        },
        failureClass: tool.failureClass,
        message: tool.reason,
      });
    }

    return baseOutput(input.model, "passed", {
      blockerRefs: [],
      baseUrlSource: client.profile.baseUrlSource,
      doctor,
      plainInference: {
        text: plain.text,
        roundTrips: plain.roundTrips,
        receipt: plain.receipt,
      },
      toolRoundTrip: {
        text: tool.text,
        roundTrips: tool.roundTrips,
        toolReceiptCount: tool.toolReceipts.length,
        receipt: tool.receipt,
        toolReceipts: [...tool.toolReceipts],
      },
    });
  });
}

function doctorBlockerRefs(readiness: PsionicQwenReadiness): string[] {
  const blockers = new Set<string>(readiness.blockerRefs);
  const engine = readiness.health?.execution_engine ?? readiness.health?.executionEngine ?? readiness.health?.backend;

  if (readiness.status === "unreachable") {
    blockers.add("blocker.psionic_qwen35.health_unreachable");
  }

  if (readiness.status === "malformed") {
    blockers.add("blocker.psionic_qwen35.health_unreachable");
  }

  if (engine !== undefined && engine.toLowerCase() !== "psionic") {
    blockers.add("blocker.psionic_qwen35.execution_engine_not_psionic");
  }

  if (!readiness.supportedEndpointRefs.includes("endpoint.psionic.v1.chat_completions")) {
    blockers.add("blocker.psionic_qwen35.chat_completion_endpoint_missing");
  }

  if (!readiness.modelRefs.includes(PSIONIC_QWEN_MODEL_REFS.qwen35_0_8b)) {
    blockers.add("blocker.psionic_qwen35.model_0_8b_missing");
  }

  return [...blockers];
}

function baseOutput(
  model: string,
  state: SmokeState,
  input: Omit<SmokeOutput, "schema" | "state" | "model" | "redacted">,
): SmokeOutput {
  return {
    schema: "openagents.pylon.psionic_qwen_live_smoke.v0.3",
    state,
    model,
    redacted: true,
    ...input,
  };
}

function parseArgs(argv: string[]): Record<string, string | true> {
  const result: Record<string, string | true> = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) {
      result[key] = true;
      continue;
    }
    result[key] = value;
    index += 1;
  }
  return result;
}

function stringArg(args: Record<string, string | true>, key: string): string | undefined {
  const value = args[key];
  return typeof value === "string" && value.trim().length > 0 ? value : undefined;
}
