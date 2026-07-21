import { Effect, Queue, Schema as S, Stream } from "effect";

import {
  CANDIDATE_SCHEMA_LITERAL,
  CandidateRef,
  MAX_TURN_CONTEXT_CHARS,
  MAX_TURN_OUTPUT_CHARS,
  PROVIDER_SCHEMA_LITERAL,
  ProviderTurnRef,
  TurnProviderRef,
  turnIntentTaskClass,
  type AnswerCandidate,
  type InferenceProviderDescriptor,
  type TurnIntent,
} from "@openagentsinc/agent-runtime-schema";
import {
  ProviderStartError,
  ProviderStreamEvent,
  type ProviderRegistryInterface,
  type ProviderRun,
  type ProviderStartInput,
} from "@openagentsinc/agent-turn-runtime";
import { projectSafeMessageChain, type ObservedAgentActivity } from "@openagentsinc/agent-surface";

import type { makeThreadStore } from "../thread-store.ts";

/**
 * #9145 hosted Khala kernel provider.
 *
 * Registers the hosted openagents.com Khala chat API (`POST /api/khala/chat` —
 * public, unauthenticated, streaming SSE) as ONE kernel inference provider, so
 * Desktop chat ALWAYS has a routable lane: Apple FM stays the preferred local
 * router when it is ready, and this lane is the always-available hosted tail
 * the readiness-aware policy falls through to. The old fixed "Stand by."
 * renderer fallback is removed with it.
 *
 * Honesty contract:
 * - `describe` is ALWAYS ready. A network failure is a typed in-stream `Failed`
 *   event on the started turn (or a bounded pre-stream HTTP failure), never a
 *   silent unreadiness that would re-open the no-lane hole.
 * - The descriptor discloses the hosted placement, the OpenAgents-managed
 *   remote data destination, and the managed-metered cost class.
 * - The answer candidate's provenance carries `hosted_khala` plus the SERVED
 *   model from the terminal `meta` frame, and its usage truth downgrades to
 *   `unknown` when no meta frame arrived.
 *
 * Wire mirror (server truth: `apps/openagents.com/workers/api/src/
 * khala-chat-routes.ts` + `khala-chat-program.ts`): request `{ messages }` with
 * user/assistant roles bounded to 40 messages / 8000 chars per message / 24000
 * total (this client truncates OLDEST first — it never fails on excess); SSE
 * frames `delta` / `reasoning` / `meta` / `done` / `error`; pre-stream JSON
 * errors 400/429/502. NO token is sent — the endpoint is public. This module
 * never touches the legacy paid `/api/v1` gateway path in `chat-service.ts`.
 */
export const HOSTED_KHALA_PROVIDER_REF = "provider.khala.hosted" as const;
export const HOSTED_KHALA_MODEL_ID = "openagents/khala" as const;
export const HOSTED_KHALA_CANDIDATE = "hosted_khala" as const;

/** Server bounds mirrored from `khala-chat-program.ts` (kept client-side so a long thread truncates instead of 400ing). */
export const HOSTED_KHALA_MAX_MESSAGES = 40 as const;
export const HOSTED_KHALA_MAX_MESSAGE_CHARS = 8_000 as const;
export const HOSTED_KHALA_MAX_TOTAL_CHARS = 24_000 as const;

type ThreadStore = ReturnType<typeof makeThreadStore>;

export interface HostedKhalaProviderConfig {
  readonly providerRef?: string;
  readonly model?: string;
  /**
   * The openagents.com origin (no trailing slash needed). Defaults to the same
   * origin-resolution pattern the legacy gateway client uses:
   * `OPENAGENTS_COM_BASE_URL` ?? `https://openagents.com`.
   */
  readonly baseUrl?: () => string;
  /** Injected for deterministic tests and the hermetic smoke — never a token carrier. */
  readonly fetchImpl?: typeof fetch;
  /**
   * The canonical thread store; by provider start the kernel has already
   * appended the current user message, so the store window IS the running
   * conversation this stateless API expects. Absent → single-message turns.
   */
  readonly getThreadStore?: () => ThreadStore | null;
  /** Deterministic turn/candidate id suffix source (tests inject a counter). */
  readonly nextId?: () => string;
  readonly now?: () => number;
}

const decodeProviderRef = S.decodeUnknownSync(TurnProviderRef);
const decodeProviderTurnRef = S.decodeUnknownSync(ProviderTurnRef);
const decodeCandidateRef = S.decodeUnknownSync(CandidateRef);

const defaultBaseUrl = (): string =>
  (process.env.OPENAGENTS_COM_BASE_URL ?? "https://openagents.com").replace(/\/+$/, "");

/** Text-bearing intents the hosted chat lane can answer. */
const intentPrompt = (intent: TurnIntent): string | null => {
  switch (intent._tag) {
    case "Ask":
      return intent.text;
    case "ProposeEdit":
      return intent.instruction;
    case "RecommendRoute":
      return intent.objective;
    default:
      return null;
  }
};

/** One wire message for the stateless hosted chat request. */
export interface HostedKhalaWireMessage {
  readonly role: "user" | "assistant";
  readonly content: string;
}

/**
 * Build the bounded `{ messages }` window from the canonical thread notes.
 * Server bounds are enforced CLIENT-side by truncating OLDEST first — a long
 * conversation degrades to a shorter window, it never fails the turn. The last
 * message is always a non-empty user message (the server validates exactly
 * that); when the store window does not end on one, the intent prompt is
 * appended as the closing user message.
 */
export const boundedHostedKhalaMessages = (
  window: ReadonlyArray<{ readonly role: string; readonly text: string }>,
  fallbackPrompt: string,
): ReadonlyArray<HostedKhalaWireMessage> => {
  const conversational: Array<HostedKhalaWireMessage> = window
    .filter(
      (note) => (note.role === "user" || note.role === "assistant") && note.text.trim() !== "",
    )
    .map((note) => ({
      role: note.role === "user" ? ("user" as const) : ("assistant" as const),
      content:
        note.text.length > HOSTED_KHALA_MAX_MESSAGE_CHARS
          ? note.text.slice(0, HOSTED_KHALA_MAX_MESSAGE_CHARS)
          : note.text,
    }));
  let messages =
    conversational.length > HOSTED_KHALA_MAX_MESSAGES
      ? conversational.slice(conversational.length - HOSTED_KHALA_MAX_MESSAGES)
      : conversational;
  const last = messages[messages.length - 1];
  if (last === undefined || last.role !== "user") {
    messages = [
      ...messages,
      { role: "user" as const, content: fallbackPrompt.slice(0, HOSTED_KHALA_MAX_MESSAGE_CHARS) },
    ];
    if (messages.length > HOSTED_KHALA_MAX_MESSAGES) {
      messages = messages.slice(messages.length - HOSTED_KHALA_MAX_MESSAGES);
    }
  }
  let total = messages.reduce((sum, message) => sum + message.content.length, 0);
  while (total > HOSTED_KHALA_MAX_TOTAL_CHARS && messages.length > 1) {
    total -= messages[0]!.content.length;
    messages = messages.slice(1);
  }
  if (total > HOSTED_KHALA_MAX_TOTAL_CHARS && messages.length === 1) {
    messages = [
      {
        role: messages[0]!.role,
        content: messages[0]!.content.slice(0, HOSTED_KHALA_MAX_TOTAL_CHARS),
      },
    ];
  }
  return messages;
};

/** Build the always-ready hosted Khala descriptor. */
export const makeHostedKhalaDescriptor = (input?: {
  readonly providerRef?: string;
  readonly model?: string;
}): InferenceProviderDescriptor => ({
  schema: PROVIDER_SCHEMA_LITERAL,
  providerRef: decodeProviderRef(input?.providerRef ?? HOSTED_KHALA_PROVIDER_REF),
  candidate: HOSTED_KHALA_CANDIDATE,
  model: input?.model ?? HOSTED_KHALA_MODEL_ID,
  placement: "openagents_managed",
  supportedIntents: ["Ask"],
  supportedCandidateKinds: ["answer"],
  // The hosted chat API sends turn input to the OpenAgents-managed remote mix.
  dataDestination: "openagents_managed_remote",
  // The terminal `meta` frame reports exact served usage; a turn without a
  // meta frame downgrades its own candidate provenance to `unknown`.
  usageTruth: "exact",
  costClass: "managed_metered",
  maxContextChars: Math.min(HOSTED_KHALA_MAX_TOTAL_CHARS, MAX_TURN_CONTEXT_CHARS),
  maxOutputChars: MAX_TURN_OUTPUT_CHARS,
  supportsStreaming: true,
  supportsCancellation: true,
  supportsExternalTools: false,
  supportsExternalActions: false,
  // ALWAYS ready: reachability failures are typed turn failures, never a
  // readiness hole that would leave chat with no admissible lane.
  readiness: { state: "ready" },
});

/** One parsed SSE frame. */
interface SseFrame {
  readonly event: string;
  readonly data: string;
}

/** Split complete `\n\n`-terminated SSE frames off the front of a buffer. */
export const splitHostedKhalaSseFrames = (
  buffer: string,
): { readonly frames: ReadonlyArray<SseFrame>; readonly rest: string } => {
  const frames: Array<SseFrame> = [];
  let rest = buffer;
  for (;;) {
    const boundary = rest.indexOf("\n\n");
    if (boundary === -1) break;
    const block = rest.slice(0, boundary);
    rest = rest.slice(boundary + 2);
    let event = "message";
    let data = "";
    for (const line of block.split("\n")) {
      if (line.startsWith("event:")) event = line.slice("event:".length).trim();
      else if (line.startsWith("data:"))
        data += (data === "" ? "" : "\n") + line.slice("data:".length).trim();
    }
    frames.push({ event, data });
  }
  return { frames, rest };
};

/** Bounded, public-safe failure detail (never a body dump, token, or URL). */
const boundedDetail = (value: string): string => value.replace(/\s+/g, " ").trim().slice(0, 240);

const jsonField = (raw: string, field: string): string | undefined => {
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed === "object" && parsed !== null) {
      const value = (parsed as Record<string, unknown>)[field];
      if (typeof value === "string") return value;
    }
  } catch {
    // Not JSON — the caller falls back to the raw bounded text.
  }
  return undefined;
};

/** The terminal result of one hosted chat pump. */
type HostedKhalaTurnResult =
  | {
      readonly ok: true;
      readonly text: string;
      readonly servedModel: string | undefined;
      readonly usageReported: boolean;
    }
  | { readonly ok: false; readonly detail: string };

/** Callbacks the pump streams through (delta text + provider-labeled reasoning). */
interface HostedKhalaPumpEvents {
  readonly onDelta: (text: string) => void;
  readonly onReasoning: (text: string) => void;
}

/**
 * Run one hosted chat turn: POST the bounded message window, then consume the
 * SSE stream incrementally. Terminal truth is the `done`/`error` frame; a
 * stream that ends without either is an honest `stream_interrupted` failure.
 */
const runHostedKhalaTurn = async (input: {
  readonly url: string;
  readonly fetchImpl: typeof fetch;
  readonly messages: ReadonlyArray<HostedKhalaWireMessage>;
  readonly signal: AbortSignal;
  readonly events: HostedKhalaPumpEvents;
}): Promise<HostedKhalaTurnResult> => {
  let response: Response;
  try {
    response = await input.fetchImpl(input.url, {
      method: "POST",
      headers: { accept: "text/event-stream", "content-type": "application/json" },
      body: JSON.stringify({ messages: input.messages }),
      signal: input.signal,
    });
  } catch (error) {
    return {
      ok: false,
      detail: boundedDetail(
        `hosted khala unreachable: ${error instanceof Error ? error.message : "network error"}`,
      ),
    };
  }
  if (!response.ok) {
    const body = await response.text().catch(() => "");
    const reason = jsonField(body, "reason") ?? jsonField(body, "error") ?? body;
    const label = response.status === 429 ? "rate_limited" : `http_${response.status}`;
    return {
      ok: false,
      detail: boundedDetail(`hosted khala ${label}${reason === "" ? "" : `: ${reason}`}`),
    };
  }

  let text = "";
  let servedModel: string | undefined;
  let usageReported = false;
  let terminal: HostedKhalaTurnResult | null = null;

  const applyFrame = (frame: SseFrame): void => {
    if (terminal !== null) return;
    switch (frame.event) {
      case "delta": {
        const delta = jsonField(frame.data, "text");
        if (delta !== undefined && delta !== "") {
          text += delta;
          input.events.onDelta(text);
        }
        return;
      }
      case "reasoning": {
        const reasoning = jsonField(frame.data, "text");
        if (reasoning !== undefined && reasoning !== "") input.events.onReasoning(reasoning);
        return;
      }
      case "meta": {
        const model = jsonField(frame.data, "servedModel");
        if (model !== undefined && model.trim() !== "") servedModel = model;
        try {
          const parsed: unknown = JSON.parse(frame.data);
          usageReported =
            typeof parsed === "object" &&
            parsed !== null &&
            typeof (parsed as { usage?: unknown }).usage === "object" &&
            (parsed as { usage?: unknown }).usage !== null;
        } catch {
          // A malformed meta frame only loses usage truth, never the answer.
        }
        return;
      }
      case "done": {
        terminal = { ok: true, text, servedModel, usageReported };
        return;
      }
      case "error": {
        const code = jsonField(frame.data, "code") ?? "stream_failed";
        const reason = jsonField(frame.data, "reason") ?? "";
        terminal = {
          ok: false,
          detail: boundedDetail(`hosted khala ${code}${reason === "" ? "" : `: ${reason}`}`),
        };
        return;
      }
      default:
        return;
    }
  };

  try {
    let buffer = "";
    const body = response.body;
    if (body === null) {
      buffer = await response.text();
      const { frames } = splitHostedKhalaSseFrames(`${buffer}\n\n`);
      for (const frame of frames) applyFrame(frame);
    } else {
      const reader = body.getReader();
      const decoder = new TextDecoder();
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        const { frames, rest } = splitHostedKhalaSseFrames(buffer);
        buffer = rest;
        for (const frame of frames) applyFrame(frame);
        if (terminal !== null) {
          await reader.cancel().catch(() => undefined);
          break;
        }
      }
      if (terminal === null && buffer !== "") {
        const { frames } = splitHostedKhalaSseFrames(`${buffer}\n\n`);
        for (const frame of frames) applyFrame(frame);
      }
    }
  } catch (error) {
    return {
      ok: false,
      detail: boundedDetail(
        `hosted khala stream failed: ${error instanceof Error ? error.message : "read error"}`,
      ),
    };
  }
  return terminal ?? { ok: false, detail: "hosted khala stream_interrupted: no terminal frame" };
};

/**
 * Create the hosted Khala `ProviderRegistry` interface: one always-ready
 * descriptor plus a streaming `start` that mirrors the delegate providers'
 * `Stream.callback` + queue pattern, emitting `Progress` + latest-snapshot
 * `Chain` events while the answer streams so the #9127 promotion can render the
 * text token-by-token.
 */
export const makeHostedKhalaProviderRegistry = (
  config: HostedKhalaProviderConfig = {},
): ProviderRegistryInterface => {
  const providerRef = config.providerRef ?? HOSTED_KHALA_PROVIDER_REF;
  const model = config.model ?? HOSTED_KHALA_MODEL_ID;
  const baseUrl = config.baseUrl ?? defaultBaseUrl;
  const fetchImpl = config.fetchImpl ?? fetch;
  const now = config.now ?? (() => Date.now());
  let counter = 0;
  const nextId = config.nextId ?? (() => `${(counter += 1)}`);

  const describe = Effect.sync(() => [makeHostedKhalaDescriptor({ providerRef, model })]);

  const answerCandidate = (input: {
    readonly seed: string;
    readonly intent: TurnIntent;
    readonly text: string;
    readonly servedModel: string | undefined;
    readonly usageReported: boolean;
    readonly latencyMs: number;
  }): AnswerCandidate => {
    const served = (input.servedModel ?? model).trim();
    return {
      schema: CANDIDATE_SCHEMA_LITERAL,
      kind: "answer",
      candidateRef: decodeCandidateRef(`candidate.khala.${input.seed}`),
      provenance: {
        providerRef: decodeProviderRef(providerRef),
        candidate: HOSTED_KHALA_CANDIDATE,
        model: (served === "" ? model : served).slice(0, 120),
        taskClass: turnIntentTaskClass[input.intent._tag],
        usageTruth: input.usageReported ? "exact" : "unknown",
        dataDestination: "openagents_managed_remote",
        latencyMs: Math.max(0, input.latencyMs),
        stale: false,
      },
      text: input.text.trim().slice(0, MAX_TURN_OUTPUT_CHARS),
    };
  };

  const start = (input: ProviderStartInput): Effect.Effect<ProviderRun, ProviderStartError> =>
    Effect.gen(function* () {
      const prompt = intentPrompt(input.intent);
      if (prompt === null || prompt.trim().length === 0) {
        return yield* Effect.fail(new ProviderStartError({ reason: "unavailable" }));
      }
      const seed = nextId();
      const providerTurnRef = decodeProviderTurnRef(`providerturn.khala.${seed}`);
      const startedAt = now();

      // The canonical thread window already contains the current user message
      // (the kernel appends it before provider start) — the same host-owned
      // history source the Apple FM prompt builder reads.
      const thread = config.getThreadStore?.()?.open(input.threadRef) ?? null;
      const window = thread === null ? [] : thread.notes;
      const messages = boundedHostedKhalaMessages(
        window.length > 0 ? window : [{ role: "user", text: prompt }],
        prompt,
      );

      const events = Stream.callback<ProviderStreamEvent>((queue) =>
        Effect.gen(function* () {
          // The redacted safe message chain: reasoning frames are ACTIVITY
          // (`system` role) entries, the streamed answer is ONE growing
          // assistant entry so the promotion streams it token-by-token.
          const activities: Array<ObservedAgentActivity> = [];
          let liveAssistantIndex = -1;
          const publishChain = (): void => {
            Queue.offerUnsafe(
              queue,
              ProviderStreamEvent.Chain({
                entries: projectSafeMessageChain(input.requestRef, activities),
              }),
            );
          };
          const onDelta = (fullText: string): void => {
            const entry: ObservedAgentActivity = { role: "assistant", text: fullText };
            if (liveAssistantIndex === -1) {
              liveAssistantIndex = activities.length;
              activities.push(entry);
            } else {
              activities[liveAssistantIndex] = entry;
            }
            Queue.offerUnsafe(queue, ProviderStreamEvent.Progress());
            publishChain();
          };
          const onReasoning = (text: string): void => {
            activities.push({ role: "system", text });
            Queue.offerUnsafe(queue, ProviderStreamEvent.Progress());
            publishChain();
          };

          // Cancellation propagates: closing the run scope interrupts this
          // fiber, and `tryPromise` aborts its AbortSignal on interruption, so
          // the in-flight fetch/stream read is torn down with the turn.
          const result = yield* Effect.tryPromise((signal) =>
            runHostedKhalaTurn({
              url: `${baseUrl().replace(/\/+$/, "")}/api/khala/chat`,
              fetchImpl,
              messages,
              signal,
              events: { onDelta, onReasoning },
            }),
          ).pipe(
            Effect.catch(() =>
              Effect.succeed<HostedKhalaTurnResult>({
                ok: false,
                detail: "hosted khala turn failed before a terminal frame",
              }),
            ),
          );

          if (result.ok) {
            const trimmed = result.text.trim();
            if (trimmed === "") {
              Queue.offerUnsafe(queue, ProviderStreamEvent.Refused({ reason: "empty_output" }));
            } else {
              const finalEntry: ObservedAgentActivity = { role: "assistant", text: trimmed };
              if (liveAssistantIndex === -1) activities.push(finalEntry);
              else activities[liveAssistantIndex] = finalEntry;
              publishChain();
              Queue.offerUnsafe(
                queue,
                ProviderStreamEvent.Completed({
                  candidate: answerCandidate({
                    seed,
                    intent: input.intent,
                    text: trimmed,
                    servedModel: result.servedModel,
                    usageReported: result.usageReported,
                    latencyMs: now() - startedAt,
                  }),
                }),
              );
            }
          } else {
            Queue.offerUnsafe(queue, ProviderStreamEvent.Failed({ detail: result.detail }));
          }
          yield* Queue.end(queue);
        }),
      );

      const run: ProviderRun = { providerTurnRef, events };
      return run;
    });

  return { describe, start };
};
