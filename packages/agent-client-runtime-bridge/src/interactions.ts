import { createHash } from "node:crypto";
import {
  applyRuntimeInteractionDecision,
  decodeRuntimeInteraction,
  decodeRuntimeInteractionDecisionEnvelope,
  type KhalaRuntimeSource,
  type RuntimeInteraction,
} from "@openagentsinc/agent-runtime-schema";

const safeRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`;
const safeDisplay = (value: string, limit: number): string =>
  value
    .replace(
      /(?:bearer\s+[A-Za-z0-9._-]+|(?:sk|xai|ghp|github_pat)[-_][A-Za-z0-9_-]{8,})/gi,
      "[redacted]",
    )
    .slice(0, limit);

export type AcpInteractionBinding = Readonly<{
  profile: "grok" | "cursor";
  kind: "question" | "plan";
  connectionRef: string;
  generation: number;
  sessionId: string;
  requestId: string | number;
  interaction: RuntimeInteraction;
  nativeQuestionRefs: Readonly<Record<string, string>>;
  nativeOptionValues: Readonly<Record<string, string>>;
}>;

type Common = Readonly<{
  profile: "grok" | "cursor";
  connectionRef: string;
  generation: number;
  sessionId: string;
  requestId: string | number;
  threadId: string;
  turnId: string;
  requestedSequence: number;
  requestedAt: string;
  expiresAt: string;
  source: KhalaRuntimeSource;
}>;

export const createAcpQuestionInteraction = (
  input: Common &
    Readonly<{
      title: string;
      questions: ReadonlyArray<
        Readonly<{
          nativeQuestionId: string;
          prompt: string;
          multiSelect?: boolean;
          options: ReadonlyArray<Readonly<{ value: string; label: string; description?: string }>>;
        }>
      >;
    }>,
): AcpInteractionBinding => {
  const scope = `${input.profile}:${input.connectionRef}:${input.generation}:${input.sessionId}:${input.requestId}`;
  const nativeQuestionRefs: Record<string, string> = {};
  const nativeOptionValues: Record<string, string> = {};
  const questions = input.questions.map((question, questionIndex) => {
    const questionRef = safeRef(
      "question",
      `${scope}:${questionIndex}:${question.nativeQuestionId}`,
    );
    nativeQuestionRefs[questionRef] = question.nativeQuestionId;
    return {
      questionRef,
      displayText: safeDisplay(question.prompt, 2_000),
      multiSelect: question.multiSelect ?? false,
      options: question.options.map((option, optionIndex) => {
        const optionRef = safeRef(
          "option",
          `${scope}:${questionIndex}:${optionIndex}:${option.value}`,
        );
        nativeOptionValues[optionRef] = option.value;
        return {
          optionRef,
          label: safeDisplay(option.label, 160),
          ...(option.description === undefined
            ? {}
            : { description: safeDisplay(option.description, 500) }),
        };
      }),
    };
  });
  const interaction = decodeRuntimeInteraction({
    schema: "openagents.runtime_interaction.v1",
    interactionRef: safeRef("interaction", scope),
    threadId: input.threadId,
    turnId: input.turnId,
    requestedSequence: input.requestedSequence,
    requestedAt: input.requestedAt,
    expiresAt: input.expiresAt,
    source: input.source,
    visibility: "private",
    redactionClass: "private_ref",
    causalityRefs: [safeRef("request", scope)],
    payload: { kind: "provider_question", displayTitle: safeDisplay(input.title, 160), questions },
    lifecycle: { status: "pending" },
  });
  return {
    profile: input.profile,
    kind: "question",
    connectionRef: input.connectionRef,
    generation: input.generation,
    sessionId: input.sessionId,
    requestId: input.requestId,
    interaction,
    nativeQuestionRefs,
    nativeOptionValues,
  };
};

export const createCursorPlanInteraction = (
  input: Omit<Common, "profile"> & Readonly<{ planRef: string; displayText: string }>,
): AcpInteractionBinding => {
  const scope = `cursor:${input.connectionRef}:${input.generation}:${input.sessionId}:${input.requestId}`;
  const interaction = decodeRuntimeInteraction({
    schema: "openagents.runtime_interaction.v1",
    interactionRef: safeRef("interaction", scope),
    threadId: input.threadId,
    turnId: input.turnId,
    requestedSequence: input.requestedSequence,
    requestedAt: input.requestedAt,
    expiresAt: input.expiresAt,
    source: input.source,
    visibility: "private",
    redactionClass: "private_ref",
    causalityRefs: [safeRef("request", scope)],
    payload: {
      kind: "plan_review",
      displayText: safeDisplay(input.displayText, 2_000),
      planRef: safeRef("plan", input.planRef),
    },
    lifecycle: { status: "pending" },
  });
  return {
    profile: "cursor",
    kind: "plan",
    connectionRef: input.connectionRef,
    generation: input.generation,
    sessionId: input.sessionId,
    requestId: input.requestId,
    interaction,
    nativeQuestionRefs: {},
    nativeOptionValues: {},
  };
};

export type AcpVendorInteractionResponse =
  | Readonly<{
      profile: "grok";
      value: Readonly<{
        outcome: "accepted";
        answers: Readonly<Record<string, ReadonlyArray<string>>>;
      }>;
    }>
  | Readonly<{ profile: "grok"; value: Readonly<{ outcome: "cancelled" }> }>
  | Readonly<{
      profile: "cursor";
      kind: "question";
      value: Readonly<{ answers: Readonly<Record<string, ReadonlyArray<string>>> }>;
    }>
  | Readonly<{ profile: "cursor"; kind: "plan"; value: Readonly<{ accepted: boolean }> }>;

const cancelAcpVendorInteraction = (binding: AcpInteractionBinding): AcpVendorInteractionResponse =>
  binding.profile === "grok"
    ? { profile: "grok", value: { outcome: "cancelled" } }
    : binding.kind === "plan"
      ? { profile: "cursor", kind: "plan", value: { accepted: false } }
      : { profile: "cursor", kind: "question", value: { answers: {} } };

const resolveAcpVendorInteraction = (
  input: Readonly<{
    binding: AcpInteractionBinding;
    connectionRef: string;
    generation: number;
    sessionId: string;
    decisionEnvelope: unknown;
    serverNow: string;
  }>,
): AcpVendorInteractionResponse => {
  if (
    input.connectionRef !== input.binding.connectionRef ||
    input.generation !== input.binding.generation ||
    input.sessionId !== input.binding.sessionId
  )
    throw new Error("stale ACP interaction binding");
  const envelope = decodeRuntimeInteractionDecisionEnvelope(input.decisionEnvelope);
  const applied = applyRuntimeInteractionDecision(
    input.binding.interaction,
    envelope,
    input.serverNow,
  );
  if (applied.state !== "applied" && applied.state !== "duplicate")
    throw new Error(`ACP interaction decision ${applied.state}`);
  if (input.binding.kind === "plan") {
    if (envelope.decision.kind !== "plan_review") throw new Error("ACP interaction kind mismatch");
    return {
      profile: "cursor",
      kind: "plan",
      value: { accepted: envelope.decision.outcome === "accept" },
    };
  }
  if (envelope.decision.kind !== "provider_question")
    throw new Error("ACP interaction kind mismatch");
  const answers: Record<string, ReadonlyArray<string>> = {};
  for (const answer of envelope.decision.answers) {
    const nativeQuestion = input.binding.nativeQuestionRefs[answer.questionRef];
    if (nativeQuestion === undefined) throw new Error("ACP interaction question mismatch");
    answers[nativeQuestion] = answer.optionRefs
      .map((option) => input.binding.nativeOptionValues[option] ?? option)
      .concat(answer.text === undefined ? [] : [answer.text]);
  }
  return input.binding.profile === "grok"
    ? { profile: "grok", value: { outcome: "accepted", answers } }
    : { profile: "cursor", kind: "question", value: { answers } };
};

/** Generation-scoped resolver that persists lifecycle and caches exact native responses. */
export class AcpInteractionOverloadError extends Error {
  override readonly name = "AcpInteractionOverloadError";
}

export class AcpVendorInteractionResolver {
  readonly #records = new Map<
    string,
    {
      binding: AcpInteractionBinding;
      interaction: RuntimeInteraction;
      decision?: string;
      response?: AcpVendorInteractionResponse;
    }
  >();
  readonly #maxEntries: number;
  readonly #now: () => string;

  constructor(options: Readonly<{ maxEntries?: number; now?: () => string }> = {}) {
    this.#maxEntries = options.maxEntries ?? 1_024;
    this.#now = options.now ?? (() => new Date().toISOString());
  }

  register(binding: AcpInteractionBinding): void {
    const now = Date.parse(this.#now());
    if (Number.isFinite(now)) {
      for (const [interactionRef, record] of this.#records) {
        if (Date.parse(record.interaction.expiresAt) <= now) this.#records.delete(interactionRef);
      }
    }
    if (this.#records.has(binding.interaction.interactionRef))
      throw new Error("ACP interaction already registered");
    if (this.#records.size >= this.#maxEntries)
      throw new AcpInteractionOverloadError("ACP interaction resolver overloaded");
    this.#records.set(binding.interaction.interactionRef, {
      binding,
      interaction: binding.interaction,
    });
  }

  resolve(
    input: Readonly<{
      interactionRef: string;
      connectionRef: string;
      generation: number;
      sessionId: string;
      decisionEnvelope: unknown;
      serverNow: string;
    }>,
  ): AcpVendorInteractionResponse {
    const record = this.#records.get(input.interactionRef);
    if (record === undefined) throw new Error("unknown ACP interaction");
    const decision = createHash("sha256")
      .update(JSON.stringify(input.decisionEnvelope))
      .digest("hex");
    if (record.response !== undefined) {
      if (record.decision !== decision) throw new Error("ACP interaction decision conflict");
      return record.response;
    }
    const binding = { ...record.binding, interaction: record.interaction };
    const response = resolveAcpVendorInteraction({ ...input, binding });
    const envelope = decodeRuntimeInteractionDecisionEnvelope(input.decisionEnvelope);
    const applied = applyRuntimeInteractionDecision(record.interaction, envelope, input.serverNow);
    if (applied.state !== "applied" && applied.state !== "duplicate")
      throw new Error(`ACP interaction decision ${applied.state}`);
    record.interaction = applied.interaction;
    record.decision = decision;
    record.response = response;
    return response;
  }

  cancel(
    input: Readonly<{
      interactionRef: string;
      connectionRef: string;
      generation: number;
      sessionId: string;
    }>,
  ): AcpVendorInteractionResponse {
    const record = this.#records.get(input.interactionRef);
    if (record === undefined) throw new Error("unknown ACP interaction");
    if (
      input.connectionRef !== record.binding.connectionRef ||
      input.generation !== record.binding.generation ||
      input.sessionId !== record.binding.sessionId
    )
      throw new Error("stale ACP interaction binding");
    if (record.response !== undefined) return record.response;
    const response = cancelAcpVendorInteraction(record.binding);
    record.interaction = {
      ...record.interaction,
      lifecycle: {
        status: "revoked",
        terminalAt: new Date().toISOString(),
        reasonRef: "reason.acp_interaction_cancelled",
      },
    };
    record.response = response;
    record.decision = "cancelled";
    return response;
  }

  close(): void {
    this.#records.clear();
  }
}
