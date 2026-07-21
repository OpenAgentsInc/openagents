import { Effect, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  CONTEXT_ENVELOPE_SCHEMA_LITERAL,
  InferenceProviderDescriptor,
  OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL,
  OwnerBoundCandidateSet,
  PROVIDER_SCHEMA_LITERAL,
  TurnRequestRef,
  WorkContextEnvelope,
} from "@openagentsinc/agent-runtime-schema";
import { TurnPolicy } from "@openagentsinc/agent-turn-runtime";

import { desktopTurnPolicyLayer } from "./desktop-turn-policy.ts";

const decodeDescriptor = S.decodeUnknownSync(InferenceProviderDescriptor);
const decodeCandidateSet = S.decodeUnknownSync(OwnerBoundCandidateSet);
const decodeContext = S.decodeUnknownSync(WorkContextEnvelope);
const decodeRequestRef = S.decodeUnknownSync(TurnRequestRef);

const appleDescriptor = (ready: boolean) =>
  decodeDescriptor({
    schema: PROVIDER_SCHEMA_LITERAL,
    providerRef: "provider.apple_fm.local",
    candidate: "apple_fm",
    model: "apple-fm",
    placement: "owner_local",
    supportedIntents: ["Ask"],
    supportedCandidateKinds: ["answer"],
    dataDestination: "on_device_local",
    usageTruth: "estimated",
    costClass: "local_resource_only",
    maxContextChars: 4000,
    maxOutputChars: 8192,
    supportsStreaming: false,
    supportsCancellation: true,
    supportsExternalTools: false,
    supportsExternalActions: false,
    readiness: ready
      ? { state: "ready" }
      : { state: "unavailable", reason: "unsupported_hardware" },
  });

const hostedKhalaDescriptor = decodeDescriptor({
  schema: PROVIDER_SCHEMA_LITERAL,
  providerRef: "provider.khala.hosted",
  candidate: "hosted_khala",
  model: "openagents/khala",
  placement: "openagents_managed",
  supportedIntents: ["Ask"],
  supportedCandidateKinds: ["answer"],
  dataDestination: "openagents_managed_remote",
  usageTruth: "exact",
  costClass: "managed_metered",
  maxContextChars: 24_000,
  maxOutputChars: 8192,
  supportsStreaming: true,
  supportsCancellation: true,
  supportsExternalTools: false,
  supportsExternalActions: false,
  readiness: { state: "ready" },
});

const context = decodeContext({
  schema: CONTEXT_ENVELOPE_SCHEMA_LITERAL,
  manifestRef: "context.thread-1",
  threadRef: "thread-1",
  generation: { state: "unknown", reason: "not_observed" },
  createdAt: "2026-07-21T08:00:00.000Z",
  items: [],
  totalByteLength: 0,
  byteLimit: 0,
  truncated: false,
  redacted: false,
});

const candidateSet = decodeCandidateSet({
  schema: OWNER_BOUND_CANDIDATE_SET_SCHEMA_LITERAL,
  ordered: ["provider.apple_fm.local", "provider.khala.hosted"],
  policyArtifactRef: "artifact.policy.desktop-local.v1",
});

const decide = (descriptors: ReadonlyArray<InferenceProviderDescriptor> | undefined) =>
  Effect.runPromise(
    Effect.gen(function* () {
      const policy = yield* TurnPolicy;
      return yield* policy.decide({
        requestRef: decodeRequestRef("request.policy.1"),
        intent: { _tag: "Ask", text: "hello" },
        context,
        candidateSet,
        recommendation: null,
        ...(descriptors === undefined ? {} : { descriptors }),
      });
    }).pipe(Effect.provide(desktopTurnPolicyLayer)),
  );

describe("Desktop readiness-aware turn policy (#9145)", () => {
  test("a READY Apple FM stays preferred over the hosted tail", async () => {
    const decision = await decide([appleDescriptor(true), hostedKhalaDescriptor]);
    if (decision.outcome !== "admitted") throw new Error("expected admitted");
    expect(decision.selected).toBe("provider.apple_fm.local");
    expect(decision.decisionReason).toBe("admitted_first_candidate");
    expect(decision.dispositions).toEqual([]);
    // Honest disclosure from the selected descriptor: local lane, local cost.
    expect(decision.disclosure.dataDestination).toBe("on_device_local");
    expect(decision.disclosure.costClass).toBe("local_resource_only");
    expect(decision.disclosure.localOnly).toBe(true);
  });

  test("an unready Apple FM is SKIPPED honestly and the hosted tail is decided", async () => {
    const decision = await decide([appleDescriptor(false), hostedKhalaDescriptor]);
    if (decision.outcome !== "admitted") throw new Error("expected admitted");
    expect(decision.selected).toBe("provider.khala.hosted");
    expect(decision.effective).toBe("provider.khala.hosted");
    expect(decision.decisionReason).toBe("admitted_first_candidate");
    expect(decision.dispositions).toEqual([
      {
        providerRef: "provider.apple_fm.local",
        candidate: "apple_fm",
        disposition: "skipped",
        reason: "resource_not_ready",
      },
    ]);
    expect(decision.disclosure.dataDestination).toBe("openagents_managed_remote");
    expect(decision.disclosure.costClass).toBe("managed_metered");
    expect(decision.disclosure.localOnly).toBe(false);
  });

  test("no ready candidate FAILS CLOSED with the skipped dispositions recorded", async () => {
    const unreadyHosted = decodeDescriptor({
      ...hostedKhalaDescriptor,
      readiness: { state: "unavailable", reason: "not_ready" },
    });
    const decision = await decide([appleDescriptor(false), unreadyHosted]);
    expect(decision.outcome).toBe("closed");
    if (decision.outcome !== "closed") throw new Error("expected closed");
    expect(decision.decisionReason).toBe("no_candidate_fail_closed");
    expect(decision.dispositions).toHaveLength(2);
  });

  test("without descriptors the previous first-candidate behavior is preserved", async () => {
    const decision = await decide(undefined);
    if (decision.outcome !== "admitted") throw new Error("expected admitted");
    expect(decision.selected).toBe("provider.apple_fm.local");
    expect(decision.decisionReason).toBe("admitted_first_candidate");
  });
});
