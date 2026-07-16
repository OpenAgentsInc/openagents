import { createHash } from "node:crypto";

import type { KhalaRuntimeSource, RuntimeInteraction } from "@openagentsinc/agent-runtime-schema";
import { AgentStdioHandlerError } from "@openagentsinc/agent-stdio-transport";

import {
  AcpInteractionOverloadError,
  AcpVendorInteractionResolver,
  createAcpQuestionInteraction,
  createCursorPlanInteraction,
  type AcpInteractionBinding,
} from "./interactions.ts";

export type AcpVendorProfile = "grok" | "cursor";

export type AcpVendorHandlerContext = Readonly<{
  generation: number;
  signal: AbortSignal;
  requestId?: string | number;
  bindSession?(sessionId: string): boolean;
}>;

export type AcpVendorStdioHost = Readonly<{
  registerReverseHandler(
    method: string,
    handler: (params: unknown, context: AcpVendorHandlerContext) => unknown | Promise<unknown>,
  ): () => void;
  onNotification(method: string, handler: (params: unknown) => void): () => void;
}>;

export type AcpVendorRequestBinding = Readonly<{
  connectionRef: string;
  generation: number;
  sessionId: string;
  requestId: string | number;
  threadId: string;
  turnId: string;
  requestedSequence: number;
}>;

export type AcpVendorInteractionDecision =
  | Readonly<{ kind: "decision"; envelope: unknown; evidenceRefs?: ReadonlyArray<string> }>
  | Readonly<{ kind: "cancelled"; evidenceRefs?: ReadonlyArray<string> }>;

export type AcpVendorInteractionStore = Readonly<{
  put(binding: AcpInteractionBinding): void | Promise<void>;
  settle(
    interactionRef: string,
    outcome: "resolved" | "cancelled" | "failed",
    evidenceRefs: ReadonlyArray<string>,
  ): void | Promise<void>;
  updateTodos?(snapshot: AcpCursorTodoSnapshot): void | Promise<void>;
}>;

export type AcpVendorInteractionBroker = Readonly<{
  decide(
    interaction: RuntimeInteraction,
    input: Readonly<{ signal: AbortSignal; expiresAt: string }>,
  ): Promise<AcpVendorInteractionDecision>;
}>;

export type AcpVendorAuditRecord = Readonly<{
  auditRef: string;
  profile: AcpVendorProfile;
  method: string;
  connectionRef: string;
  generation: number;
  sessionRef: string;
  requestRef?: string;
  interactionRef?: string;
  outcome: "opened" | "resolved" | "cancelled" | "rejected" | "observed" | "overloaded";
  evidenceRefs: ReadonlyArray<string>;
}>;

export type AcpVendorAuditPort = Readonly<{
  record(record: AcpVendorAuditRecord): void | Promise<void>;
}>;

export type AcpCursorTodoSnapshot = Readonly<{
  profile: "cursor";
  method: "cursor/update_todos";
  connectionRef: string;
  generation: number;
  sessionRef: string;
  requestRef: string;
  toolCallRef: string;
  merge: boolean;
  todos: ReadonlyArray<
    Readonly<{
      todoRef: string;
      contentRef?: string;
      status: string;
    }>
  >;
}>;

export type AcpVendorHandlerOptions = Readonly<{
  profile: AcpVendorProfile;
  connectionRef: string;
  generation: number;
  source: KhalaRuntimeSource;
  host: AcpVendorStdioHost;
  resolver: AcpVendorInteractionResolver;
  store: AcpVendorInteractionStore;
  broker: AcpVendorInteractionBroker;
  audit?: AcpVendorAuditPort;
  bindingFor(
    input: Readonly<{
      method: string;
      params: unknown;
      context?: AcpVendorHandlerContext;
    }>,
  ): AcpVendorRequestBinding;
  now(): string;
  deadlineMs?: number;
}>;

type NormalizedQuestion = Readonly<{
  nativeQuestionId: string;
  prompt: string;
  multiSelect?: boolean;
  options: ReadonlyArray<Readonly<{ value: string; label: string; description?: string }>>;
}>;

const MAX_QUESTIONS = 8;
const MAX_OPTIONS = 12;
const MAX_TODOS = 256;
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$/;

const object = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

const requiredString = (value: unknown, limit: number, label: string): string => {
  if (typeof value !== "string" || value.trim() === "" || value.length > limit) {
    throw new AgentStdioHandlerError(-32602, `Invalid ${label}.`, {
      reason: "invalid_vendor_payload",
      retryable: false,
    });
  }
  return value;
};

const optionalString = (value: unknown, limit: number, label: string): string | undefined =>
  value === undefined || value === null ? undefined : requiredString(value, limit, label);

const safeRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`;

const evidenceRefs = (values: ReadonlyArray<string> | undefined): ReadonlyArray<string> => [
  ...new Set(
    (values ?? []).map((value) => (safeRefPattern.test(value) ? value : "evidence.redacted")),
  ),
];

const normalizeQuestions = (
  value: unknown,
  profile: AcpVendorProfile,
): ReadonlyArray<NormalizedQuestion> => {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_QUESTIONS) {
    throw new AgentStdioHandlerError(-32602, "Invalid vendor question list.", {
      reason: "invalid_vendor_payload",
      retryable: false,
    });
  }
  return value.map((entry, questionIndex) => {
    const question = object(entry);
    const prompt = requiredString(
      profile === "grok" ? question.question : question.prompt,
      2_000,
      "vendor question",
    );
    const nativeQuestionId =
      profile === "grok" ? prompt : requiredString(question.id, 256, "Cursor question id");
    if (!Array.isArray(question.options) || question.options.length > MAX_OPTIONS) {
      throw new AgentStdioHandlerError(-32602, "Invalid vendor question options.", {
        reason: "invalid_vendor_payload",
        retryable: false,
      });
    }
    const options = question.options
      .map((entry, optionIndex) => {
        const option = object(entry);
        const label = requiredString(option.label, 160, "vendor option label");
        if (profile === "cursor") requiredString(option.id, 256, "Cursor option id");
        // Both T3 provider adapters answer with the labels exposed to the operator.
        const value = label;
        const description = optionalString(option.description, 500, "vendor option description");
        return {
          value,
          label,
          ...(description === undefined ? {} : { description }),
          // Preserve deterministic order even when native IDs repeat; interaction refs include index.
          _index: optionIndex,
        };
      })
      .map(({ _index: _ignored, ...option }) => option);
    const multiSelect =
      profile === "grok" ? question.multiSelect === true : question.allowMultiple === true;
    return {
      nativeQuestionId: nativeQuestionId || `question-${questionIndex}`,
      prompt,
      multiSelect,
      options,
    };
  });
};

const normalizeGrokQuestion = (
  method: string,
  raw: unknown,
): Readonly<{
  sessionId: string;
  title: string;
  questions: ReadonlyArray<NormalizedQuestion>;
}> => {
  const outer = object(raw);
  const wrapped = object(outer.params);
  const value = Object.keys(wrapped).length > 0 ? wrapped : outer;
  if (Object.keys(wrapped).length > 0) {
    const wrappedMethod = outer.method;
    if (wrappedMethod !== "x.ai/ask_user_question" && wrappedMethod !== "_x.ai/ask_user_question") {
      throw new AgentStdioHandlerError(-32602, "Invalid wrapped Grok extension method.");
    }
  }
  const sessionId = requiredString(value.sessionId, 256, "Grok session id");
  requiredString(value.toolCallId, 256, "Grok tool call id");
  if (value.mode !== "default" && value.mode !== "plan") {
    throw new AgentStdioHandlerError(-32602, "Invalid Grok question mode.");
  }
  return {
    sessionId,
    title: value.mode === "plan" ? "Grok plan question" : "Grok question",
    questions: normalizeQuestions(value.questions, "grok"),
  };
};

const normalizeCursorQuestion = (
  raw: unknown,
): Readonly<{
  title: string;
  questions: ReadonlyArray<NormalizedQuestion>;
}> => {
  const value = object(raw);
  requiredString(value.toolCallId, 256, "Cursor tool call id");
  return {
    title: optionalString(value.title, 160, "Cursor question title") ?? "Cursor question",
    questions: normalizeQuestions(value.questions, "cursor"),
  };
};

const assertBinding = (
  options: AcpVendorHandlerOptions,
  method: string,
  params: unknown,
  context: AcpVendorHandlerContext | undefined,
  nativeSessionId?: string,
): AcpVendorRequestBinding => {
  const binding = options.bindingFor({
    method,
    params,
    ...(context === undefined ? {} : { context }),
  });
  if (
    binding.connectionRef !== options.connectionRef ||
    binding.generation !== options.generation ||
    (context?.generation !== undefined && context.generation !== binding.generation) ||
    (context?.requestId !== undefined && context.requestId !== binding.requestId) ||
    (nativeSessionId !== undefined && nativeSessionId !== binding.sessionId) ||
    !safeRefPattern.test(binding.connectionRef) ||
    !safeRefPattern.test(binding.sessionId) ||
    !safeRefPattern.test(binding.threadId) ||
    !safeRefPattern.test(binding.turnId) ||
    !Number.isSafeInteger(binding.requestedSequence)
  ) {
    throw new AgentStdioHandlerError(-32002, "Stale ACP vendor interaction binding.", {
      reason: "stale_vendor_binding",
      retryable: false,
    });
  }
  if (context?.bindSession !== undefined && !context.bindSession(binding.sessionId)) {
    throw new AgentStdioHandlerError(-32002, "Stale ACP vendor interaction binding.", {
      reason: "stale_vendor_binding",
      retryable: false,
    });
  }
  return binding;
};

const audit = async (
  options: AcpVendorHandlerOptions,
  input: Omit<AcpVendorAuditRecord, "auditRef" | "profile" | "connectionRef" | "generation">,
): Promise<void> => {
  if (options.audit === undefined) return;
  await options.audit.record({
    auditRef: safeRef(
      "audit",
      `${options.profile}:${input.method}:${input.requestRef ?? input.interactionRef ?? input.sessionRef}:${input.outcome}`,
    ),
    profile: options.profile,
    connectionRef: options.connectionRef,
    generation: options.generation,
    ...input,
    evidenceRefs: evidenceRefs(input.evidenceRefs),
  });
};

const cancellation = (
  signal: AbortSignal,
  expiresAt: string,
  serverNow: string,
): Promise<"cancelled"> =>
  new Promise((resolve) => {
    const remainingMs = Date.parse(expiresAt) - Date.parse(serverNow);
    if (signal.aborted || remainingMs <= 0) {
      resolve("cancelled");
      return;
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const done = () => {
      if (timer !== undefined) clearTimeout(timer);
      signal.removeEventListener("abort", done);
      resolve("cancelled");
    };
    signal.addEventListener("abort", done, { once: true });
    timer = setTimeout(done, remainingMs);
  });

const decide = async (
  options: AcpVendorHandlerOptions,
  binding: AcpInteractionBinding,
  signal: AbortSignal,
  method: string,
): Promise<unknown> => {
  try {
    options.resolver.register(binding);
  } catch (error) {
    if (!(error instanceof AcpInteractionOverloadError)) throw error;
    await audit(options, {
      method,
      sessionRef: safeRef("session", binding.sessionId),
      interactionRef: binding.interaction.interactionRef,
      outcome: "overloaded",
      evidenceRefs: [],
    });
    throw new AgentStdioHandlerError(-32005, "ACP vendor interaction overloaded.", {
      reason: "interaction_overloaded",
      retryable: true,
    });
  }
  await options.store.put(binding);
  const requestRef = safeRef(
    "request",
    `${binding.connectionRef}:${binding.generation}:${binding.requestId}`,
  );
  await audit(options, {
    method,
    sessionRef: safeRef("session", binding.sessionId),
    requestRef,
    interactionRef: binding.interaction.interactionRef,
    outcome: "opened",
    evidenceRefs: [],
  });
  const broker = options.broker.decide(binding.interaction, {
    signal,
    expiresAt: binding.interaction.expiresAt,
  });
  const result = await Promise.race([
    broker,
    cancellation(signal, binding.interaction.expiresAt, options.now()),
  ]);
  if (result === "cancelled" || result.kind === "cancelled") {
    const refs = result === "cancelled" ? [] : evidenceRefs(result.evidenceRefs);
    const response = options.resolver.cancel({
      interactionRef: binding.interaction.interactionRef,
      connectionRef: binding.connectionRef,
      generation: binding.generation,
      sessionId: binding.sessionId,
    });
    await options.store.settle(binding.interaction.interactionRef, "cancelled", refs);
    await audit(options, {
      method,
      sessionRef: safeRef("session", binding.sessionId),
      requestRef,
      interactionRef: binding.interaction.interactionRef,
      outcome: "cancelled",
      evidenceRefs: refs,
    });
    return response.value;
  }
  try {
    const response = options.resolver.resolve({
      interactionRef: binding.interaction.interactionRef,
      connectionRef: binding.connectionRef,
      generation: binding.generation,
      sessionId: binding.sessionId,
      decisionEnvelope: result.envelope,
      serverNow: options.now(),
    });
    const refs = evidenceRefs(result.evidenceRefs);
    await options.store.settle(binding.interaction.interactionRef, "resolved", refs);
    await audit(options, {
      method,
      sessionRef: safeRef("session", binding.sessionId),
      requestRef,
      interactionRef: binding.interaction.interactionRef,
      outcome: "resolved",
      evidenceRefs: refs,
    });
    return response.value;
  } catch {
    await options.store.settle(binding.interaction.interactionRef, "failed", []);
    throw new AgentStdioHandlerError(-32602, "Invalid ACP vendor interaction decision.", {
      reason: "invalid_vendor_decision",
      retryable: false,
    });
  }
};

const registerQuestion = (
  options: AcpVendorHandlerOptions,
  method: "x.ai/ask_user_question" | "_x.ai/ask_user_question" | "cursor/ask_question",
): (() => void) =>
  options.host.registerReverseHandler(method, async (params, context) => {
    if (context.signal.aborted) {
      throw new AgentStdioHandlerError(-32800, "ACP vendor interaction cancelled.");
    }
    let nativeSessionId: string | undefined;
    let normalized: Readonly<{ title: string; questions: ReadonlyArray<NormalizedQuestion> }>;
    if (options.profile === "grok") {
      const grok = normalizeGrokQuestion(method, params);
      nativeSessionId = grok.sessionId;
      normalized = grok;
    } else {
      normalized = normalizeCursorQuestion(params);
    }
    const binding = assertBinding(options, method, params, context, nativeSessionId);
    const requestedAt = options.now();
    const expiresAt = new Date(
      Date.parse(requestedAt) + (options.deadlineMs ?? 300_000),
    ).toISOString();
    const interaction = createAcpQuestionInteraction({
      profile: options.profile,
      ...binding,
      requestedAt,
      expiresAt,
      source: options.source,
      title: normalized.title,
      questions: normalized.questions,
    });
    return decide(options, interaction, context.signal, method);
  });

const registerCursorPlan = (options: AcpVendorHandlerOptions): (() => void) =>
  options.host.registerReverseHandler("cursor/create_plan", async (params, context) => {
    if (context.signal.aborted)
      throw new AgentStdioHandlerError(-32800, "Cursor plan interaction cancelled.");
    const value = object(params);
    requiredString(value.toolCallId, 256, "Cursor plan tool call id");
    const plan = requiredString(value.plan, 200_000, "Cursor plan");
    if (!Array.isArray(value.todos) || value.todos.length > MAX_TODOS) {
      throw new AgentStdioHandlerError(-32602, "Invalid Cursor plan todos.");
    }
    const binding = assertBinding(options, "cursor/create_plan", params, context);
    const requestedAt = options.now();
    const expiresAt = new Date(
      Date.parse(requestedAt) + (options.deadlineMs ?? 300_000),
    ).toISOString();
    const interaction = createCursorPlanInteraction({
      ...binding,
      requestedAt,
      expiresAt,
      source: options.source,
      planRef: safeRef("native_plan", plan),
      displayText:
        optionalString(value.overview, 2_000, "Cursor plan overview") ?? "Review the Cursor plan.",
    });
    return decide(options, interaction, context.signal, "cursor/create_plan");
  });

const registerCursorTodos = (options: AcpVendorHandlerOptions): (() => void) =>
  options.host.onNotification("cursor/update_todos", (params) => {
    void (async () => {
      try {
        const value = object(params);
        const toolCallId = requiredString(value.toolCallId, 256, "Cursor todo tool call id");
        if (
          !Array.isArray(value.todos) ||
          value.todos.length > MAX_TODOS ||
          typeof value.merge !== "boolean"
        ) {
          throw new AgentStdioHandlerError(-32602, "Invalid Cursor todo update.");
        }
        const binding = assertBinding(options, "cursor/update_todos", params, undefined);
        const scope = `${binding.connectionRef}:${binding.generation}:${binding.sessionId}:${binding.requestId}`;
        const snapshot: AcpCursorTodoSnapshot = {
          profile: "cursor",
          method: "cursor/update_todos",
          connectionRef: binding.connectionRef,
          generation: binding.generation,
          sessionRef: safeRef("session", binding.sessionId),
          requestRef: safeRef("request", String(binding.requestId)),
          toolCallRef: safeRef("tool", `${scope}:${toolCallId}`),
          merge: value.merge,
          todos: value.todos.map((entry, index) => {
            const todo = object(entry);
            const id = optionalString(todo.id, 256, "Cursor todo id") ?? String(index);
            const content = optionalString(
              todo.content ?? todo.title,
              2_000,
              "Cursor todo content",
            );
            const status = optionalString(todo.status, 64, "Cursor todo status") ?? "pending";
            return {
              todoRef: safeRef("todo", `${scope}:${id}`),
              ...(content === undefined ? {} : { contentRef: safeRef("content", content) }),
              status,
            };
          }),
        };
        await options.store.updateTodos?.(snapshot);
        await audit(options, {
          method: "cursor/update_todos",
          sessionRef: snapshot.sessionRef,
          requestRef: snapshot.requestRef,
          outcome: "observed",
          evidenceRefs: [],
        });
      } catch {
        // JSON-RPC notifications have no response. Invalid payloads are isolated and safely audited.
        const fallback = safeRef("session", options.connectionRef);
        await audit(options, {
          method: "cursor/update_todos",
          sessionRef: fallback,
          outcome: "rejected",
          evidenceRefs: [],
        });
      }
    })();
  });

export const registerAcpVendorInteractionHandlers = (
  options: AcpVendorHandlerOptions,
): (() => void) => {
  if (options.profile !== "grok" && options.profile !== "cursor") {
    throw new TypeError("explicit Grok or Cursor ACP peer profile is required");
  }
  if (
    !safeRefPattern.test(options.connectionRef) ||
    !Number.isSafeInteger(options.generation) ||
    options.generation < 0
  ) {
    throw new TypeError("invalid ACP vendor connection generation");
  }
  const unregister =
    options.profile === "grok"
      ? [
          registerQuestion(options, "x.ai/ask_user_question"),
          registerQuestion(options, "_x.ai/ask_user_question"),
        ]
      : [
          registerQuestion(options, "cursor/ask_question"),
          registerCursorPlan(options),
          registerCursorTodos(options),
        ];
  return () => {
    for (const dispose of unregister.reverse()) dispose();
    options.resolver.close();
  };
};
