/**
 * HARN-09 (#9167) Slice 1 (app-server sub-path): run one codex-local
 * app-server turn THROUGH the SDK harness adapter
 * (`makeCodexHarnessAdapter`, app-server mode) using the LIVE streaming
 * transport seam (`CodexAppServerTransport.runTurnStreaming`, rc.3) — so the
 * adapter projects each event onto the neutral stream the INSTANT the
 * app-server produces it and the renderer sees text/tool rows live, exactly
 * like the hand-written path, instead of only after the turn settles.
 *
 * Division of labor (identical to the exec attempt, `codex-harness-attempt.ts`):
 * the DESKTOP keeps custody of the transport (its own supervised
 * `codex app-server` process, JSON-RPC framing, approvals, steer/interrupt,
 * supervisor leases, extension admission — injected here as a
 * `CodexAppServerTransport`), the account/env selection, and the
 * `onDispatch`/`onProviderSession` journal hooks; the ADAPTER owns the neutral
 * projection + turn lifecycle; `harness-lowering` maps the neutral CORE stream
 * back onto the frozen `ClaudeLocalEvent` renderer envelope. Exact usage and
 * the codex thread id tee from the raw wire (never reconstructed from optional
 * neutral fields).
 *
 * Display-only parity (RESOLVED — see `codex-app-server-harness-turn.ts`):
 * the neutral seven-core stream carries text/reasoning/tool/turn only, while
 * the live hand-written path (`codex-app-server-turn.ts`) also emits
 * display-only kinds with no neutral origin (`child_*`, `tool_progress`,
 * `plan_updated`, `meter_updated`, `lane_notice`, `question_*`, typed
 * workbench `item` payloads). The live dispatch route therefore wraps the
 * hand-written turn as the streaming transport: the legacy turn keeps DISPLAY
 * authority (every renderer event, by-construction parity) and this module's
 * lowered emissions are suppressed there, while the adapter owns the neutral
 * stream + lifecycle. Direct callers of this module (tests, headless-style
 * drives) still get the lowered seven-core renderer stream via `emit`.
 */

import type {
  CodexAppServerTransport,
  CodexEvent,
  HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract";
import { makeCodexHarnessAdapter } from "@openagentsinc/agent-harness-contract";
import { Effect, Stream } from "effect";
import type { ClaudeLocalEvent } from "./claude-local-contract";
import type { CodexChildUsage } from "./codex-child-contract";
import { lowerHarnessEvent } from "./harness-lowering";

export interface CodexAppServerHarnessAttemptInput {
  readonly threadRef: string;
  readonly turnRef: string;
  readonly workspace: string;
  readonly prompt: string;
  readonly model: string;
  readonly resumeThreadId: string | null;
  /**
   * The desktop-owned, streaming-capable app-server transport. In production a
   * live bridge over the supervised `codex app-server` process/supervisor
   * lease; in tests the SDK's scripted streaming transport. Must expose
   * `runTurnStreaming` for the live drive; a batch-only transport still works
   * (the adapter falls back to `runTurn`) but does not stream live.
   */
  readonly transport: CodexAppServerTransport;
  readonly emit: (event: ClaudeLocalEvent) => void;
}

export interface CodexAppServerHarnessAttemptResult {
  readonly outcome: "success" | "reconnect_required" | "failed" | "timeout";
  readonly text: string;
  readonly usage: CodexChildUsage | null;
  readonly threadId: string | null;
  readonly detail: string;
  readonly quotaExhausted: boolean;
  readonly rateLimited: boolean;
}

const classifyFailure = (
  detail: string,
): Pick<CodexAppServerHarnessAttemptResult, "outcome" | "quotaExhausted" | "rateLimited"> => {
  const lowered = detail.toLowerCase();
  if (
    lowered.includes("unauthorized") ||
    lowered.includes("authentication") ||
    lowered.includes("login") ||
    lowered.includes("credential") ||
    lowered.includes("token could not be refreshed") ||
    lowered.includes("sign in again") ||
    lowered.includes("401")
  ) {
    return { outcome: "reconnect_required", quotaExhausted: false, rateLimited: false };
  }
  if (lowered.includes("usage limit") || lowered.includes("quota")) {
    return { outcome: "failed", quotaExhausted: true, rateLimited: false };
  }
  if (lowered.includes("rate limit") || lowered.includes("429")) {
    return { outcome: "failed", quotaExhausted: false, rateLimited: true };
  }
  if (lowered.includes("timed out") || lowered.includes("timeout")) {
    return { outcome: "timeout", quotaExhausted: false, rateLimited: false };
  }
  return { outcome: "failed", quotaExhausted: false, rateLimited: false };
};

/**
 * Tee the exact usage, native thread id, and public-safe failure off the raw
 * {@link CodexEvent} wire as it streams — the same custody split as the exec
 * attempt's spawner tee. Never reconstructed from the (optional, possibly
 * absent) neutral usage fields.
 */
interface WireTee {
  usage: CodexChildUsage | null;
  threadId: string | null;
  failure: string | null;
}

const teeCodexEvent = (wire: WireTee, event: CodexEvent): void => {
  if (event.type === "thread.started") wire.threadId = event.threadId;
  else if (event.type === "turn.failed") wire.failure = event.messageSafe;
  else if (event.type === "error" && wire.failure === null) wire.failure = event.messageSafe;
  else if (event.type === "token_usage.updated") {
    const usage = event.usage;
    wire.usage = {
      inputTokens: usage.inputTokens,
      cachedInputTokens: usage.cachedInputTokens,
      outputTokens: usage.outputTokens,
      reasoningOutputTokens: usage.reasoningOutputTokens,
      totalTokens: usage.inputTokens + usage.outputTokens + usage.reasoningOutputTokens,
    };
  } else if (event.type === "turn.completed" && event.usage !== undefined) {
    const usage = event.usage;
    wire.usage = {
      inputTokens: usage.inputTokens,
      cachedInputTokens: usage.cachedInputTokens,
      outputTokens: usage.outputTokens,
      reasoningOutputTokens: usage.reasoningOutputTokens,
      totalTokens: usage.inputTokens + usage.outputTokens + usage.reasoningOutputTokens,
    };
  }
};

/**
 * Wrap the injected transport so both the streaming and batch drives tee the
 * raw wire, and `startThread` records the native thread id. The adapter is
 * given the wrapped transport unchanged in every other respect.
 */
const teeTransport = (
  transport: CodexAppServerTransport,
  wire: WireTee,
): CodexAppServerTransport => ({
  ...transport,
  startThread: (params) =>
    transport.startThread(params).pipe(
      Effect.tap((result) =>
        Effect.sync(() => {
          wire.threadId = result.threadId;
        }),
      ),
    ),
  runTurn: (params) =>
    transport
      .runTurn(params)
      .pipe(
        Effect.tap((events) =>
          Effect.sync(() => events.forEach((event) => teeCodexEvent(wire, event))),
        ),
      ),
  ...(transport.runTurnStreaming === undefined
    ? {}
    : {
        runTurnStreaming: (params) =>
          transport.runTurnStreaming!(params).pipe(
            Stream.tap((event) => Effect.sync(() => teeCodexEvent(wire, event))),
          ),
      }),
});

/**
 * Run one app-server turn through the adapter, lowering the neutral CORE
 * stream onto `ClaudeLocalEvent` live. Returns the desktop-shaped attempt
 * result with exact wire-teed usage/thread id/failure.
 */
export const runCodexAppServerHarnessAttempt = async (
  input: CodexAppServerHarnessAttemptInput,
): Promise<CodexAppServerHarnessAttemptResult> => {
  const wire: WireTee = { usage: null, threadId: input.resumeThreadId, failure: null };

  const adapter = makeCodexHarnessAdapter({
    mode: "app-server",
    codexBinaryPath: "codex",
    workingDirectory: input.workspace,
    model: input.model,
    transport: teeTransport(input.transport, wire),
  });

  const program = Effect.gen(function* () {
    const session = yield* adapter.start({
      sessionId: input.threadRef,
      source: { lane: "codex_app_server", adapterKind: "codex" },
      ...(input.resumeThreadId === null
        ? {}
        : {
            resumeFrom: {
              harnessId: "codex",
              sessionId: input.threadRef,
              data: { threadId: input.resumeThreadId },
            },
          }),
    });
    const control = yield* session.promptTurn({
      turnId: input.turnRef,
      prompt: input.prompt,
    });
    let text = "";
    // Lower and emit LIVE — each neutral event reaches the renderer as it is
    // pulled from the adapter's streaming projection, not after settle.
    yield* Stream.runForEach(control.events, (event: HarnessStreamEvent) =>
      Effect.sync(() => {
        for (const lowered of lowerHarnessEvent(event)) {
          if (lowered.kind === "text_delta") text += lowered.text;
          input.emit(lowered);
        }
      }),
    );
    const result = yield* control.done;
    yield* session.stop();
    return { text, finishReason: result.finishReason };
  });

  try {
    const outcome = await Effect.runPromise(program);
    if (wire.failure !== null) {
      const detail = wire.failure.slice(0, 400);
      return {
        text: outcome.text,
        usage: wire.usage,
        threadId: wire.threadId,
        detail,
        ...classifyFailure(detail),
      };
    }
    return {
      outcome: "success",
      text: outcome.text,
      usage: wire.usage,
      threadId: wire.threadId,
      detail: "",
      quotaExhausted: false,
      rateLimited: false,
    };
  } catch (error) {
    const failureClass =
      (error as { failureClass?: string }).failureClass ??
      (error as { error?: { failureClass?: string } }).error?.failureClass;
    const detail = String(
      (error as { detail?: string }).detail ?? failureClass ?? (error as Error).message ?? error,
    ).slice(0, 400);
    return {
      text: "",
      usage: wire.usage,
      threadId: wire.threadId,
      detail,
      ...classifyFailure(detail),
    };
  }
};
