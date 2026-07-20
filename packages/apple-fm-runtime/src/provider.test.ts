import { describe, expect, test } from "vite-plus/test";
import { Effect, Schema as S, Stream } from "effect";

import { makeAppleFmDescriptor, makeAppleFmProviderRegistry } from "./provider.js";
import type { AppleFmCompletionTurn } from "./client.js";

import {
  CONTEXT_ENVELOPE_SCHEMA_LITERAL,
  TurnIntent,
  TurnProviderRef,
  TurnRequestRef,
  TurnThreadRef,
  WorkContextEnvelope,
} from "@openagentsinc/agent-runtime-schema";
import type { ProviderStartInput } from "@openagentsinc/agent-turn-runtime";

const AT = "2026-07-20T08:00:00.000Z" as const;
const PROVIDER_REF = "provider.apple_fm.local";

const decodeContext = S.decodeUnknownSync(WorkContextEnvelope);
const decodeIntent = S.decodeUnknownSync(TurnIntent);
const decodeProviderRef = S.decodeUnknownSync(TurnProviderRef);
const decodeRequestRef = S.decodeUnknownSync(TurnRequestRef);
const decodeThreadRef = S.decodeUnknownSync(TurnThreadRef);

const askInput = (text: string): ProviderStartInput => ({
  providerRef: decodeProviderRef(PROVIDER_REF),
  requestRef: decodeRequestRef("request.apple_fm.1"),
  threadRef: decodeThreadRef("thread.apple_fm.1"),
  intent: decodeIntent({ _tag: "Ask", text }),
  context: decodeContext({
    schema: CONTEXT_ENVELOPE_SCHEMA_LITERAL,
    manifestRef: "context.apple_fm.1",
    threadRef: "thread.apple_fm.1",
    generation: { state: "known", value: 0 },
    createdAt: AT,
    items: [],
    totalByteLength: 0,
    byteLimit: 0,
    truncated: false,
    redacted: false,
  }),
});

const runStart = (
  config: Parameters<typeof makeAppleFmProviderRegistry>[0],
  input: ProviderStartInput,
): Promise<{ tags: ReadonlyArray<string>; events: ReadonlyArray<unknown> }> =>
  Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const registry = makeAppleFmProviderRegistry(config);
        const run = yield* registry.start(input);
        const chunk = yield* Stream.runCollect(run.events);
        const events: ReadonlyArray<unknown> = [...chunk];
        return { tags: events.map((event) => (event as { _tag: string })._tag), events };
      }),
    ),
  );

const readyComplete = (turn: AppleFmCompletionTurn) => ({
  providerRef: PROVIDER_REF,
  readiness: () => ({ ready: true }),
  complete: async () => turn,
});

describe("Apple FM inference provider adapter", () => {
  test("the descriptor declares a local, action-free answer lane", () => {
    const descriptor = makeAppleFmDescriptor({ providerRef: PROVIDER_REF, readiness: { ready: true } });
    expect(descriptor.candidate).toBe("apple_fm");
    expect(descriptor.dataDestination).toBe("on_device_local");
    expect(descriptor.costClass).toBe("local_resource_only");
    expect(descriptor.supportsExternalActions).toBe(false);
    expect(descriptor.supportsExternalTools).toBe(false);
    expect(descriptor.supportedCandidateKinds).toEqual(["answer"]);
    expect(descriptor.readiness).toEqual({ state: "ready" });
  });

  test("helper readiness maps into provider readiness with no renderer input", () => {
    const descriptor = makeAppleFmDescriptor({
      providerRef: PROVIDER_REF,
      readiness: { ready: false, unavailableReason: "apple_intelligence_disabled" },
    });
    expect(descriptor.readiness).toEqual({ state: "unavailable", reason: "apple_intelligence_disabled" });
  });

  test("a ready lane converts a completion into an AnswerCandidate", async () => {
    const { tags, events } = await runStart(
      readyComplete({ outcome: "completed", text: "Here is the answer.", usageTruth: "estimated", totalTokens: 7 }),
      askInput("explain this"),
    );
    expect(tags).toEqual(["Progress", "Completed"]);
    const completed = events[1] as { candidate: { kind: string; text: string; provenance: Record<string, unknown> } };
    expect(completed.candidate.kind).toBe("answer");
    expect(completed.candidate.text).toBe("Here is the answer.");
    expect(completed.candidate.provenance).toMatchObject({
      candidate: "apple_fm",
      dataDestination: "on_device_local",
      usageTruth: "estimated",
      stale: false,
    });
  });

  test("an empty completion refuses (empty_output), never dispatches a candidate", async () => {
    const { tags, events } = await runStart(
      readyComplete({ outcome: "completed", text: "   ", usageTruth: "estimated" }),
      askInput("hi"),
    );
    expect(tags).toEqual(["Progress", "Refused"]);
    expect((events[1] as { reason: string }).reason).toBe("empty_output");
  });

  test("an oversized completion refuses (oversized_output)", async () => {
    const { events } = await runStart(
      { ...readyComplete({ outcome: "completed", text: "x".repeat(50), usageTruth: "estimated" }), maxOutputChars: 10 },
      askInput("hi"),
    );
    expect((events[1] as { reason: string }).reason).toBe("oversized_output");
  });

  test("an action-claim completion refuses (action_claim_rejected)", async () => {
    const claim = JSON.stringify({ candidate: "codex", taskClass: "delegate", reasonCode: "needs_delegation", confidence: 0.9, action: { tool: "shell" } });
    const { events } = await runStart(readyComplete({ outcome: "completed", text: claim, usageTruth: "estimated" }), askInput("hi"));
    expect((events[1] as { reason: string }).reason).toBe("action_claim_rejected");
  });

  test("a failed completion surfaces a Failed event", async () => {
    const { tags, events } = await runStart(
      readyComplete({ outcome: "failed", usageTruth: "unknown", failureClass: "bridge_unreachable" }),
      askInput("hi"),
    );
    expect(tags).toEqual(["Progress", "Failed"]);
    expect((events[1] as { detail: string }).detail).toBe("bridge_unreachable");
  });

  test("an unavailable lane fails start with a typed ProviderStartError", async () => {
    const exit = await Effect.runPromiseExit(
      Effect.scoped(
        makeAppleFmProviderRegistry({
          providerRef: PROVIDER_REF,
          readiness: () => ({ ready: false, unavailableReason: "not_ready" }),
          complete: async () => ({ outcome: "failed", usageTruth: "unknown" }),
        }).start(askInput("hi")),
      ),
    );
    expect(exit._tag).toBe("Failure");
  });

  test("describe reports one Apple FM descriptor", async () => {
    const descriptors = await Effect.runPromise(
      makeAppleFmProviderRegistry(readyComplete({ outcome: "failed", usageTruth: "unknown" })).describe,
    );
    expect(descriptors).toHaveLength(1);
    expect(descriptors[0]?.candidate).toBe("apple_fm");
  });
});
