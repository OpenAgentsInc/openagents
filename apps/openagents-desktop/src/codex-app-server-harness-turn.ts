/**
 * HARN-09 (#9167) — the LIVE codex app-server dispatch route through the SDK
 * harness adapter (`makeCodexHarnessAdapter`, app-server mode, streaming
 * seam).
 *
 * Architecture (display authority vs lifecycle authority):
 *
 * - The hand-written `runCodexAppServerTurn` remains the single DISPLAY
 *   AUTHORITY: it runs the real JSON-RPC protocol over the supervised
 *   `codex app-server` process (supervisor leases, extension admission,
 *   product-spec skill, approvals via the reverse handler, steer/interrupt,
 *   receipts, identity fences) and emits every `ClaudeLocalEvent` exactly as
 *   the legacy path does — rich tool summaries, typed workbench items,
 *   `tool_progress`, `child_*`, `plan_updated`, `meter_updated`,
 *   `lane_notice`, `question_*`. Renderer parity with the legacy path is
 *   therefore BY CONSTRUCTION: the same code produces every renderer event.
 * - The SDK ADAPTER owns the neutral projection + turn lifecycle: the
 *   `onCodexEvent` tee inside `runCodexAppServerTurn` projects the own-thread
 *   wire onto the neutral `CodexEvent` vocabulary and feeds it through a
 *   live `CodexAppServerTransport.runTurnStreaming` bridge (rc.3 seam), so
 *   the adapter projects each event onto `HarnessStreamEvent` the instant it
 *   arrives and drives session start/promptTurn/done/stop.
 * - DEDUPE: every lowered core event from the adapter is SUPPRESSED here
 *   (the display authority already emitted the richer form), so the renderer
 *   never sees doubled text/tool/terminal events — the same pattern as the
 *   claude slice's rich-`turn_completed` dedupe, applied to the whole core
 *   stream because the host events are strictly richer (typed items, exact
 *   summaries) than the seven-core lowering.
 *
 * Custody notes (documented fall-throughs):
 *
 * - APPROVALS stay host-owned: `execCommandApproval` / `applyPatchApproval`
 *   server->client requests are answered by the desktop's reverse handler
 *   (`onServerRequest` in codex-local-runtime), which renders the exact
 *   3-option `question_pending` card and answers the JSON-RPC request
 *   directly. The transport's `respondToApproval` seam is intentionally a
 *   no-op here — routing approvals through the adapter's
 *   `RuntimeInteraction` model would require rebuilding the renderer
 *   question bridge for zero behavior gain, and the host flow already
 *   preserves Full Auto auto-decline exactly.
 * - INTERRUPT/STEER stay host-owned through `input.control` (set by
 *   `runCodexAppServerTurn`); the transport's `interruptTurn` delegates to
 *   the same control for completeness.
 * - The turn OUTCOME authority is the legacy `CodexAppServerTurnOutcome`
 *   (exact preContent/policyDenied/quota/rate classification the rotation
 *   loop depends on); the adapter's settled result is used as the fallback
 *   only if the legacy outcome is unavailable (defensive, not expected).
 */

import type { CodexAppServerTransport, CodexEvent } from "@openagentsinc/agent-harness-contract";
import { CodexTransportError } from "@openagentsinc/agent-harness-contract";
import { Effect, Stream } from "effect";
import { runCodexAppServerHarnessAttempt } from "./codex-app-server-harness-attempt";
import {
  runCodexAppServerTurn,
  type CodexAppServerTurnOutcome,
  type RunCodexAppServerTurnInput,
} from "./codex-app-server-turn";

/** Single-consumer async channel bridging Node callbacks into a Stream. */
interface AsyncChannel<T> extends AsyncIterable<T> {
  readonly push: (value: T) => void;
  readonly end: () => void;
}

const makeAsyncChannel = <T>(): AsyncChannel<T> => {
  const buffer: T[] = [];
  let done = false;
  let notify: (() => void) | null = null;
  return {
    push(value: T): void {
      if (done) return;
      buffer.push(value);
      notify?.();
    },
    end(): void {
      done = true;
      notify?.();
    },
    async *[Symbol.asyncIterator](): AsyncIterator<T> {
      while (true) {
        if (buffer.length > 0) {
          yield buffer.shift()!;
          continue;
        }
        if (done) return;
        await new Promise<void>((resolve) => {
          notify = resolve;
        });
        notify = null;
      }
    },
  };
};

/**
 * Run one live codex app-server turn THROUGH the SDK harness adapter while
 * `runCodexAppServerTurn` keeps display authority. Same input contract and
 * same returned outcome as calling `runCodexAppServerTurn` directly.
 */
export const runCodexAppServerHarnessTurn = async (
  input: RunCodexAppServerTurnInput,
): Promise<CodexAppServerTurnOutcome> => {
  const channel = makeAsyncChannel<CodexEvent>();
  const outcomeBox: { value: CodexAppServerTurnOutcome | null } = { value: null };
  let legacyPromise: Promise<CodexAppServerTurnOutcome> | null = null;
  let sawTerminal = false;

  const startLegacyTurn = (): void => {
    if (legacyPromise !== null) return;
    legacyPromise = runCodexAppServerTurn({
      ...input,
      onCodexEvent: (event) => {
        if (event.type === "turn.completed" || event.type === "turn.failed") sawTerminal = true;
        channel.push(event);
        input.onCodexEvent?.(event);
      },
    });
    void legacyPromise.then((outcome) => {
      outcomeBox.value = outcome;
      if (!sawTerminal) {
        // The turn settled without a wire-terminal notification (spawn/lease
        // failure, timeout, thrown protocol error): give the adapter an
        // honest terminal event so its lifecycle settles coherently.
        channel.push(
          outcome.outcome === "success"
            ? { type: "turn.completed", status: "completed" }
            : outcome.outcome === "interrupted"
              ? { type: "turn.completed", status: "interrupted" }
              : {
                  type: "turn.failed",
                  messageSafe: (outcome.detail === "" ? outcome.outcome : outcome.detail).slice(
                    0,
                    400,
                  ),
                },
        );
      }
      channel.end();
    });
  };

  const transport: CodexAppServerTransport = {
    // The real thread identity binds mid-turn (thread/start response inside
    // runCodexAppServerTurn) and reaches the adapter via the teed
    // `thread.started` event; the placeholder never escapes this bridge.
    startThread: (params) =>
      Effect.succeed({ threadId: params.resumeThreadId ?? `pending.${input.threadRef}` }),
    runTurn: () =>
      Effect.fail(
        new CodexTransportError({
          failureClass: "batch_unsupported",
          detail: "the live desktop bridge is streaming-only",
        }),
      ),
    runTurnStreaming: () => {
      startLegacyTurn();
      return Stream.fromAsyncIterable(
        channel,
        (cause) =>
          new CodexTransportError({
            failureClass: "stream_failed",
            detail: String(cause).slice(0, 400),
          }),
      );
    },
    // Approvals are host-owned (see module doc): the reverse handler answers
    // the JSON-RPC request directly, so the adapter never holds a pending
    // approval on this bridge.
    respondToApproval: () => Effect.void,
    interruptTurn: () => Effect.sync(() => input.control.interrupt?.()),
    shutdown: () => Effect.void,
  };

  const attempt = await runCodexAppServerHarnessAttempt({
    threadRef: input.threadRef,
    turnRef: input.turnRef,
    workspace: input.workspace,
    prompt: input.prompt,
    model: input.model,
    resumeThreadId: input.resumeThreadId,
    transport,
    // DEDUPE (see module doc): the display authority already emitted the
    // richer host events for the entire core stream; lowered duplicates are
    // suppressed so the renderer never sees doubled text/tool/terminal rows.
    emit: () => {},
  });

  // The legacy turn always settles once started (it catches everything);
  // await it so no in-flight protocol work outlives this call.
  if (legacyPromise !== null) {
    const outcome = await legacyPromise;
    return outcomeBox.value ?? outcome;
  }

  // Defensive: the adapter failed before ever subscribing the stream (the
  // legacy turn never ran). Map the attempt result onto the legacy outcome
  // shape with honest pre-content classification.
  return {
    outcome:
      attempt.outcome === "timeout"
        ? "timeout"
        : attempt.outcome === "reconnect_required"
          ? "reconnect_required"
          : "failed",
    text: attempt.text,
    usage: attempt.usage,
    threadId: attempt.threadId,
    detail: attempt.detail,
    preContent: attempt.text.trim() === "" && (attempt.usage?.totalTokens ?? 0) === 0,
    policyDenied: false,
    quotaExhausted: attempt.quotaExhausted,
    rateLimited: attempt.rateLimited,
  };
};
