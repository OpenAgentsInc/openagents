/**
 * HARN-09 (#9167) Slice 2: run one claude-local turn THROUGH the SDK harness
 * adapter (`makeClaudeCodeHarnessAdapter`) instead of the hand-written
 * `query()` drive — the DEFAULT-ON dispatch route
 * (`OPENAGENTS_DESKTOP_CLAUDE_HARNESS_ADAPTER=0` is the rollback to the
 * legacy drive).
 *
 * Division of labor mirrors slice 1: the DESKTOP keeps custody — the exact
 * `query()` options it owns (bundled executable, MCP servers, plugins,
 * skills, maxTurns, permission mode, and especially its allow-all
 * `canUseTool` that parks AskUserQuestion) are handed to the adapter through
 * the `queryOverrides` seam so approval routing stays host-side; the ADAPTER
 * owns neutral projection and turn lifecycle; `harness-lowering` maps the
 * neutral stream back onto the frozen `ClaudeLocalEvent` renderer envelope.
 * Exact usage and the effective model are teed from the raw SDK messages.
 */

import type { ClaudeCodeMessage, ClaudeCodeQuery } from "@openagentsinc/agent-harness-contract";
import { makeClaudeCodeHarnessAdapter } from "@openagentsinc/agent-harness-contract";
import { Effect, Stream } from "effect";
import type { ClaudeChildUsage, ClaudeLocalEvent } from "./claude-local-contract";
import { lowerHarnessEvent } from "./harness-lowering";

export interface ClaudeHarnessAttemptInput {
  readonly threadRef: string;
  readonly turnRef: string;
  readonly workspace: string;
  readonly prompt: string;
  readonly model: string;
  readonly resumeSessionId: string | null;
  /** The host's structural `query()` seam (the real Claude Agent SDK query). */
  readonly query: ClaudeCodeQuery;
  /**
   * The exact query options the desktop owns (env, canUseTool, mcpServers,
   * plugins, skills, maxTurns, permissionMode, pathToClaudeCodeExecutable,
   * allowedTools, disallowedTools). Merged LAST by the adapter.
   */
  readonly queryOverrides: Readonly<Record<string, unknown>>;
  readonly emit: (event: ClaudeLocalEvent) => void;
  /**
   * Host hook fired once when the SDK `system/init` message is observed, with
   * the raw session id, effective model, and per-server MCP status. The host
   * uses it to run the behaviors the neutral stream has NO origin for — the
   * "must be Claude" model-substitution guard, `mcp_server_unavailable`, and
   * `onProviderSession` continuity — and may abort through the
   * `queryOverrides.abortController` it owns. Called before any content lowers.
   */
  readonly onInit?: (info: {
    readonly sessionId: string | null;
    readonly effectiveModel: string | null;
    readonly mcpServers: ReadonlyArray<{ readonly name: string; readonly status: string }>;
  }) => void;
  /**
   * HARN-09 display reconstruction: when provided, EVERY raw SDK message is
   * handed to this observer (via the adapter's `onRawMessage`) and the
   * attempt's own internal `model_effective` reconstruction is disabled —
   * the host's shared display projector becomes the single display
   * authority, so renderer parity with the legacy `query()` drive is by
   * construction. Callers that use this typically also pass a no-op `emit`
   * to suppress the lowered core stream (the projector emits the richer
   * legacy form of every core event).
   */
  readonly observeMessage?: (message: ClaudeCodeMessage) => void;
}

export interface ClaudeHarnessAttemptResult {
  readonly outcome: "success" | "reconnect_required" | "failed";
  readonly text: string;
  readonly totalTokens: number | null;
  readonly usage: ClaudeChildUsage | null;
  readonly sessionId: string | null;
  readonly effectiveModel: string | null;
  readonly detail: string;
  /** Raw `result` message fields teed from the wire (HARN-09 parity): the
   * host replicates the legacy subtype-exact failure classification and the
   * legacy final-text authority order (result text, then assistant blocks,
   * then stream deltas) from these. */
  readonly resultText: string | null;
  readonly resultSubtype: string | null;
  readonly resultIsError: boolean;
}

const usageFromResult = (message: ClaudeCodeMessage): ClaudeChildUsage | null => {
  if (message.type !== "result") return null;
  const usage = message.usage as typeof message.usage | null;
  // `usage: null` is a real wire shape (fixture-pinned); treat like absent.
  if (usage === undefined || usage === null) return null;
  const input = usage.input_tokens ?? 0;
  const output = usage.output_tokens ?? 0;
  const cacheRead = usage.cache_read_input_tokens ?? 0;
  const cacheWrite = usage.cache_creation_input_tokens ?? 0;
  const cachedInputTokens = cacheRead + cacheWrite;
  return {
    inputTokens: input,
    cachedInputTokens,
    outputTokens: output,
    reasoningTokens: 0,
    // Usage-ledger exactness: match the legacy claude-local usage math
    // (`usageSplitFromResult`) exactly — total counts cached input tokens,
    // not just input + output — so the flag-on adapter path and the legacy
    // path report identical totals.
    totalTokens: input + cachedInputTokens + output,
  };
};

const classifyFailure = (detail: string): ClaudeHarnessAttemptResult["outcome"] => {
  const lowered = detail.toLowerCase();
  if (
    lowered.includes("not logged in") ||
    lowered.includes("please run /login") ||
    lowered.includes("sign in again")
  ) {
    return "reconnect_required";
  }
  return "failed";
};

/** Run one claude turn through the adapter, teeing usage/session/model. */
export const runClaudeHarnessAttempt = async (
  input: ClaudeHarnessAttemptInput,
): Promise<ClaudeHarnessAttemptResult> => {
  const wire: {
    usage: ClaudeChildUsage | null;
    sessionId: string | null;
    effectiveModel: string | null;
    failure: string | null;
    resultText: string | null;
    resultSubtype: string | null;
    resultIsError: boolean;
  } = {
    usage: null,
    sessionId: null,
    effectiveModel: null,
    failure: null,
    resultText: null,
    resultSubtype: null,
    resultIsError: false,
  };

  // Tee the raw SDK messages the adapter consumes (usage, session id,
  // effective model, error) without disturbing the adapter's projection.
  const teedQuery: ClaudeCodeQuery = (params) => {
    const iterable = input.query(params);
    return (async function* () {
      // Manual iterator drive (not for-await): the terminal-result settle
      // below must NOT await the source's return() — a stuck iterator close
      // must never block the settled turn (parity with the legacy drive's
      // fire-and-forget closeIterator).
      const iterator = iterable[Symbol.asyncIterator]();
      while (true) {
        const step = await iterator.next();
        if (step.done === true) return;
        let message = step.value;
        if (message.type === "system" && message.subtype === "init") {
          if (message.session_id.length > 0) wire.sessionId = message.session_id;
          if (typeof (message as { model?: string }).model === "string") {
            wire.effectiveModel = (message as { model?: string }).model ?? null;
          }
          const rawServers = (message as { mcp_servers?: unknown }).mcp_servers;
          const mcpServers = Array.isArray(rawServers)
            ? rawServers.flatMap((entry) =>
                entry !== null && typeof entry === "object"
                  ? [
                      {
                        name:
                          typeof (entry as { name?: unknown }).name === "string"
                            ? (entry as { name: string }).name
                            : "",
                        status:
                          typeof (entry as { status?: unknown }).status === "string"
                            ? (entry as { status: string }).status
                            : "",
                      },
                    ]
                  : [],
              )
            : [];
          input.onInit?.({
            sessionId: wire.sessionId,
            effectiveModel: wire.effectiveModel,
            mcpServers,
          });
        }
        if (message.type === "result") {
          wire.usage = usageFromResult(message);
          const resultValue = (message as { result?: unknown }).result;
          if (typeof resultValue === "string" && resultValue.length > 0) {
            wire.resultText = resultValue;
          }
          const subtypeValue = (message as { subtype?: unknown }).subtype;
          wire.resultSubtype = typeof subtypeValue === "string" ? subtypeValue : null;
          wire.resultIsError = message.is_error === true;
          if (message.is_error === true) {
            wire.failure = (message as { result?: string }).result ?? "claude turn failed";
          }
          // `usage: null` is a real wire shape (fixture-pinned); the adapter's
          // projection expects the field absent instead. Normalize the copy
          // handed to the adapter — the tee above already read the original.
          if ((message as { usage?: unknown }).usage === null) {
            const { usage: _nullUsage, ...rest } = message as unknown as Record<string, unknown>;
            message = rest as unknown as typeof message;
          }
        }
        yield message;
        // Terminal-result settle (parity with the legacy drive's
        // break-on-result + fire-and-forget closeIterator): the SDK `result`
        // message is the turn's terminal frame, so the drive settles even
        // when the underlying iterator never resolves its close.
        // Query.close() is the SDK's documented subprocess/resource authority
        // — invoke it first, then best-effort iterator return, never awaited.
        if (message.type === "result") {
          try {
            (iterable as { close?: () => void }).close?.();
          } catch {
            // Terminal provider truth still wins cleanup exceptions.
          }
          try {
            const closing = iterator.return?.();
            if (closing !== undefined) void Promise.resolve(closing).catch(() => undefined);
          } catch {
            // An iterator cleanup exception cannot un-settle the turn.
          }
          return;
        }
      }
    })();
  };

  // Display reconstruction from the raw wire (openagents#9167 slice 3): when
  // the host supplies `observeMessage`, its shared display projector is the
  // single display authority and observes EVERY raw message; otherwise this
  // module reconstructs the display-only `model_effective` renderer event
  // itself (the neutral stream has no effective-model event).
  let announcedModel: string | null = null;
  const observeMessage = input.observeMessage;
  const adapter = makeClaudeCodeHarnessAdapter({
    query: teedQuery,
    cwd: input.workspace,
    model: input.model,
    queryOverrides: input.queryOverrides,
    onRawMessage:
      observeMessage !== undefined
        ? observeMessage
        : (message) => {
            if (message.type !== "system" || message.subtype !== "init") return;
            const model = (message as { model?: string }).model;
            if (typeof model === "string" && model.length > 0 && model !== announcedModel) {
              announcedModel = model;
              input.emit({ kind: "model_effective", model: model.slice(0, 120) });
            }
          },
  });

  const program = Effect.gen(function* () {
    const session = yield* adapter.start({
      sessionId: input.threadRef,
      source: { lane: "claude_pylon", adapterKind: "claude_code" },
      ...(input.resumeSessionId === null
        ? {}
        : {
            resumeFrom: {
              harnessId: "claude-code",
              sessionId: input.threadRef,
              data: { claudeSessionId: input.resumeSessionId },
            },
          }),
    });
    const control = yield* session.promptTurn({ turnId: input.turnRef, prompt: input.prompt });
    let text = "";
    yield* Stream.runForEach(control.events, (event) =>
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

  // `base` reads `wire` AFTER the turn runs — the tee fills it during
  // iteration, so it must be built from the settled wire, never before.
  const base = () => ({
    totalTokens: wire.usage?.totalTokens ?? null,
    usage: wire.usage,
    sessionId: wire.sessionId,
    effectiveModel: wire.effectiveModel,
    resultText: wire.resultText,
    resultSubtype: wire.resultSubtype,
    resultIsError: wire.resultIsError,
  });

  try {
    const outcome = await Effect.runPromise(program);
    if (wire.failure !== null) {
      const detail = wire.failure.slice(0, 400);
      return { ...base(), outcome: classifyFailure(detail), text: outcome.text, detail };
    }
    return { ...base(), outcome: "success", text: outcome.text, detail: "" };
  } catch (error) {
    const detail = String(
      (error as { detail?: string }).detail ?? (error as Error).message ?? error,
    ).slice(0, 400);
    return { ...base(), outcome: classifyFailure(detail), text: "", detail };
  }
};
