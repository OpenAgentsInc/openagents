import { describe, expect, it } from "vite-plus/test";

import type { RuntimeInteraction } from "@openagentsinc/agent-runtime-schema";

import { AcpVendorInteractionResolver } from "./interactions.ts";
import {
  registerAcpVendorInteractionHandlers,
  type AcpCursorTodoSnapshot,
  type AcpVendorHandlerContext,
  type AcpVendorInteractionDecision,
  type AcpVendorStdioHost,
} from "./vendor-handlers.ts";

const now = "2026-07-16T12:00:00.000Z";

const makeHost = () => {
  const requests = new Map<
    string,
    (params: unknown, context: AcpVendorHandlerContext) => unknown | Promise<unknown>
  >();
  const notifications = new Map<string, (params: unknown) => void>();
  const host: AcpVendorStdioHost = {
    registerReverseHandler(method, handler) {
      requests.set(method, handler);
      return () => {
        requests.delete(method);
      };
    },
    onNotification(method, handler) {
      notifications.set(method, handler);
      return () => {
        notifications.delete(method);
      };
    },
  };
  return { host, requests, notifications };
};

const decisionFor = (interaction: RuntimeInteraction): AcpVendorInteractionDecision => {
  if (interaction.payload.kind === "plan_review") {
    return {
      kind: "decision",
      envelope: {
        decisionRef: "decision.plan.1",
        idempotencyKey: "idem.plan.1",
        decidedAt: now,
        surface: "desktop",
        decision: { kind: "plan_review", outcome: "accept" },
      },
      evidenceRefs: ["evidence.plan.1"],
    };
  }
  if (interaction.payload.kind !== "provider_question") throw new Error("unexpected interaction");
  return {
    kind: "decision",
    envelope: {
      decisionRef: "decision.question.1",
      idempotencyKey: `idem.${interaction.interactionRef}`,
      decidedAt: now,
      surface: "desktop",
      decision: {
        kind: "provider_question",
        answers: interaction.payload.questions.map((question) => ({
          questionRef: question.questionRef,
          optionRefs: question.options.length === 0 ? [] : [question.options[0]!.optionRef],
          ...(question.options.length === 0 ? { text: "typed answer" } : {}),
        })),
      },
    },
    evidenceRefs: ["evidence.question.1"],
  };
};

const setup = (
  profile: "grok" | "cursor",
  decide = async (interaction: RuntimeInteraction) => decisionFor(interaction),
  resolver = new AcpVendorInteractionResolver(),
) => {
  const { host, requests, notifications } = makeHost();
  const opened: string[] = [];
  const settled: Array<readonly [string, string]> = [];
  const todos: AcpCursorTodoSnapshot[] = [];
  const audits: unknown[] = [];
  let request = 0;
  const unregister = registerAcpVendorInteractionHandlers({
    profile,
    connectionRef: "connection.vendor.1",
    generation: 7,
    source: { lane: "agent_client_protocol", surface: "server" },
    host,
    resolver,
    store: {
      put(binding) {
        opened.push(binding.interaction.interactionRef);
      },
      settle(interactionRef, outcome) {
        settled.push([interactionRef, outcome]);
      },
      updateTodos(snapshot) {
        todos.push(snapshot);
      },
    },
    broker: { decide },
    audit: {
      record(record) {
        audits.push(record);
      },
    },
    bindingFor({ context }) {
      request += 1;
      return {
        connectionRef: "connection.vendor.1",
        generation: context?.generation ?? 7,
        sessionId: "session.vendor.1",
        requestId: context?.requestId ?? `notification-${request}`,
        threadId: "thread.vendor.1",
        turnId: "turn.vendor.1",
        requestedSequence: request,
      };
    },
    now: () => now,
    deadlineMs: 60_000,
  });
  return { requests, notifications, opened, settled, todos, audits, unregister };
};

const context = (
  requestId: number,
  signal = new AbortController().signal,
): AcpVendorHandlerContext => ({
  generation: 7,
  requestId,
  signal,
});

describe("ACP vendor interaction handlers", () => {
  it("returns structured overload and audits without opening an interaction", async () => {
    const state = setup("grok", undefined, new AcpVendorInteractionResolver({ maxEntries: 0 }));
    await expect(
      state.requests.get("x.ai/ask_user_question")!(
        {
          sessionId: "session.vendor.1",
          toolCallId: "tool-1",
          questions: [{ question: "Q", options: [{ label: "A", description: "Answer A" }] }],
          mode: "default",
        },
        context(10),
      ),
    ).rejects.toMatchObject({
      code: -32005,
      data: { reason: "interaction_overloaded", retryable: true },
    });
    expect(state.opened).toEqual([]);
    expect(state.audits).toMatchObject([{ outcome: "overloaded" }]);
    state.unregister();
  });

  it.each(["x.ai/ask_user_question", "_x.ai/ask_user_question"] as const)(
    "normalizes Grok %s and returns exact accepted answers",
    async (method) => {
      const state = setup("grok");
      const handler = state.requests.get(method)!;
      const response = await handler(
        {
          sessionId: "session.vendor.1",
          toolCallId: "tool-1",
          questions: [
            {
              question: "Which scope?",
              options: [{ label: "Workspace", description: "Use workspace" }],
            },
          ],
          mode: "default",
        },
        context(11),
      );
      expect(response).toEqual({ outcome: "accepted", answers: { "Which scope?": ["Workspace"] } });
      expect(state.opened).toHaveLength(1);
      expect(state.settled[0]?.[1]).toBe("resolved");
      expect(JSON.stringify(state.audits)).not.toContain("Which scope");
      state.unregister();
    },
  );

  it("normalizes Cursor questions and returns Cursor answer maps", async () => {
    const state = setup("cursor");
    const response = await state.requests.get("cursor/ask_question")!(
      {
        toolCallId: "tool-1",
        title: "Choose",
        questions: [
          {
            id: "scope",
            prompt: "Which scope?",
            options: [{ id: "workspace", label: "Workspace" }],
          },
        ],
      },
      context(12),
    );
    expect(response).toEqual({ answers: { scope: ["Workspace"] } });
    expect(state.opened).toHaveLength(1);
  });

  it("routes Cursor create_plan through plan review and update_todos through the snapshot store", async () => {
    const state = setup("cursor");
    await expect(
      state.requests.get("cursor/create_plan")!(
        {
          toolCallId: "tool-plan",
          plan: "# Private plan",
          todos: [],
          overview: "Review this plan",
        },
        context(13),
      ),
    ).resolves.toEqual({ accepted: true });
    state.notifications.get("cursor/update_todos")!({
      toolCallId: "tool-plan",
      merge: false,
      todos: [{ id: "one", content: "private todo", status: "in_progress" }],
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(state.todos).toHaveLength(1);
    expect(state.todos[0]).toMatchObject({ merge: false, todos: [{ status: "in_progress" }] });
    expect(JSON.stringify(state.todos)).not.toContain("private todo");
  });

  it("binds a Cursor reverse request without native sessionId to its admitted session", async () => {
    const state = setup("cursor");
    let boundSessionId: string | undefined;
    await expect(
      state.requests.get("cursor/create_plan")!(
        { toolCallId: "tool-plan", plan: "Plan", todos: [] },
        {
          ...context(130),
          bindSession(sessionId) {
            boundSessionId = sessionId;
            return true;
          },
        },
      ),
    ).resolves.toEqual({ accepted: true });
    expect(boundSessionId).toBe("session.vendor.1");
  });

  it("rejects stale generation/session bindings before opening an interaction", async () => {
    const state = setup("grok");
    await expect(
      state.requests.get("x.ai/ask_user_question")!(
        {
          sessionId: "foreign-session",
          toolCallId: "tool-1",
          questions: [{ question: "Q", options: [] }],
          mode: "default",
        },
        context(14),
      ),
    ).rejects.toMatchObject({ code: -32002 });
    expect(state.opened).toEqual([]);
  });

  it("returns exact profile cancellation responses and settles cancellation", async () => {
    for (const [profile, method, params, expected] of [
      [
        "grok",
        "x.ai/ask_user_question",
        {
          sessionId: "session.vendor.1",
          toolCallId: "t",
          questions: [{ question: "Q", options: [] }],
          mode: "default",
        },
        { outcome: "cancelled" },
      ],
      [
        "cursor",
        "cursor/create_plan",
        { toolCallId: "t", plan: "Plan", todos: [] },
        { accepted: false },
      ],
    ] as const) {
      const state = setup(profile, async () => ({ kind: "cancelled" }));
      await expect(state.requests.get(method)!(params, context(15))).resolves.toEqual(expected);
      expect(state.settled[0]?.[1]).toBe("cancelled");
    }
  });
});
