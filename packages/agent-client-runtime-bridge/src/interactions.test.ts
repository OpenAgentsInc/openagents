import { describe, expect, it } from "vite-plus/test";
import {
  AcpVendorInteractionResolver,
  createAcpQuestionInteraction,
  createCursorPlanInteraction,
} from "./interactions.ts";

const common = {
  connectionRef: "connection.1",
  generation: 3,
  sessionId: "session.1",
  requestId: 9,
  threadId: "thread.1",
  turnId: "turn.1",
  requestedSequence: 2,
  requestedAt: "2026-07-16T12:00:00.000Z",
  expiresAt: "2026-07-16T12:05:00.000Z",
  source: { lane: "agent_client_protocol" as const, surface: "server" as const },
};

describe("ACP vendor runtime interactions", () => {
  it.each(["grok", "cursor"] as const)(
    "maps %s questions and returns profile-native answers",
    (profile) => {
      const binding = createAcpQuestionInteraction({
        ...common,
        profile,
        title: "Choose",
        questions: [
          {
            nativeQuestionId: "q-native",
            prompt: "Which?",
            options: [{ value: "yes-native", label: "Yes" }],
          },
        ],
      });
      expect(binding.interaction.payload.kind).toBe("provider_question");
      const payload = binding.interaction.payload;
      if (payload.kind !== "provider_question") throw new Error("wrong payload");
      const resolver = new AcpVendorInteractionResolver();
      resolver.register(binding);
      const result = resolver.resolve({
        interactionRef: binding.interaction.interactionRef,
        connectionRef: common.connectionRef,
        generation: 3,
        sessionId: common.sessionId,
        serverNow: common.requestedAt,
        decisionEnvelope: {
          decisionRef: "decision.1",
          idempotencyKey: "idem.1",
          decidedAt: common.requestedAt,
          surface: "desktop",
          decision: {
            kind: "provider_question",
            answers: [
              {
                questionRef: payload.questions[0]!.questionRef,
                optionRefs: [payload.questions[0]!.options[0]!.optionRef],
              },
            ],
          },
        },
      });
      expect(JSON.stringify(result)).toContain("yes-native");
      expect(JSON.stringify(result)).toContain("q-native");
    },
  );

  it("maps Cursor create_plan to plan_review and validates generation", () => {
    const binding = createCursorPlanInteraction({
      ...common,
      planRef: "native-plan",
      displayText: "Review plan",
    });
    expect(binding.interaction.payload.kind).toBe("plan_review");
    const resolver = new AcpVendorInteractionResolver();
    resolver.register(binding);
    expect(() =>
      resolver.resolve({
        interactionRef: binding.interaction.interactionRef,
        connectionRef: common.connectionRef,
        generation: 2,
        sessionId: common.sessionId,
        serverNow: common.requestedAt,
        decisionEnvelope: {},
      }),
    ).toThrow("stale");
    expect(
      resolver.resolve({
        interactionRef: binding.interaction.interactionRef,
        connectionRef: common.connectionRef,
        generation: 3,
        sessionId: common.sessionId,
        serverNow: common.requestedAt,
        decisionEnvelope: {
          decisionRef: "decision.plan",
          idempotencyKey: "idem.plan",
          decidedAt: common.requestedAt,
          surface: "desktop",
          decision: { kind: "plan_review", outcome: "accept" },
        },
      }),
    ).toEqual({ profile: "cursor", kind: "plan", value: { accepted: true } });
  });

  it("returns profile-exact cancellation without resolving through a prompt", () => {
    const grok = createAcpQuestionInteraction({
      ...common,
      profile: "grok",
      title: "Choose",
      questions: [{ nativeQuestionId: "q", prompt: "Q", options: [] }],
    });
    const cursor = createCursorPlanInteraction({ ...common, planRef: "p", displayText: "P" });
    const grokResolver = new AcpVendorInteractionResolver();
    grokResolver.register(grok);
    const cursorResolver = new AcpVendorInteractionResolver();
    cursorResolver.register(cursor);
    expect(
      grokResolver.cancel({
        interactionRef: grok.interaction.interactionRef,
        connectionRef: common.connectionRef,
        generation: 3,
        sessionId: common.sessionId,
      }),
    ).toEqual({ profile: "grok", value: { outcome: "cancelled" } });
    expect(
      cursorResolver.cancel({
        interactionRef: cursor.interaction.interactionRef,
        connectionRef: common.connectionRef,
        generation: 3,
        sessionId: common.sessionId,
      }),
    ).toEqual({ profile: "cursor", kind: "plan", value: { accepted: false } });
  });

  it("persists lifecycle, caches exact replay, and rejects conflicts", () => {
    const binding = createAcpQuestionInteraction({
      ...common,
      profile: "grok",
      title: "Choose",
      questions: [
        { nativeQuestionId: "q", prompt: "Q", options: [{ value: "yes", label: "Yes" }] },
      ],
    });
    if (binding.interaction.payload.kind !== "provider_question") throw new Error("wrong kind");
    const decisionEnvelope = {
      decisionRef: "decision.persisted",
      idempotencyKey: "idem.persisted",
      decidedAt: common.requestedAt,
      surface: "desktop",
      decision: {
        kind: "provider_question",
        answers: [
          {
            questionRef: binding.interaction.payload.questions[0]!.questionRef,
            optionRefs: [binding.interaction.payload.questions[0]!.options[0]!.optionRef],
          },
        ],
      },
    };
    const resolver = new AcpVendorInteractionResolver();
    resolver.register(binding);
    const input = {
      interactionRef: binding.interaction.interactionRef,
      connectionRef: common.connectionRef,
      generation: 3,
      sessionId: common.sessionId,
      serverNow: common.requestedAt,
      decisionEnvelope,
    };
    const first = resolver.resolve(input);
    expect(resolver.resolve(input)).toEqual(first);
    expect(() =>
      resolver.resolve({
        ...input,
        decisionEnvelope: { ...decisionEnvelope, decisionRef: "decision.conflict" },
      }),
    ).toThrow("conflict");
  });

  it("bounds pending interaction state", () => {
    const resolver = new AcpVendorInteractionResolver({
      maxEntries: 1,
      now: () => common.requestedAt,
    });
    const first = createAcpQuestionInteraction({
      ...common,
      profile: "grok",
      title: "One",
      questions: [{ nativeQuestionId: "q1", prompt: "Q", options: [] }],
    });
    const second = createAcpQuestionInteraction({
      ...common,
      profile: "grok",
      requestId: 10,
      title: "Two",
      questions: [{ nativeQuestionId: "q2", prompt: "Q", options: [] }],
    });
    resolver.register(first);
    expect(() => resolver.register(second)).toThrow("overloaded");
    resolver.close();
    expect(() => resolver.register(second)).not.toThrow();
  });
});
