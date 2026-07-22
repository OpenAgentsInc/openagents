/**
 * HARN-09 (#9167) Slice 2: run one claude-local turn THROUGH the SDK harness
 * adapter (`makeClaudeCodeHarnessAdapter`) instead of the hand-written
 * `query()` drive — behind the strangler flag
 * `OPENAGENTS_DESKTOP_CLAUDE_HARNESS_ADAPTER=1` (default off, zero behavior
 * change when unset).
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
}

export interface ClaudeHarnessAttemptResult {
  readonly outcome: "success" | "reconnect_required" | "failed";
  readonly text: string;
  readonly totalTokens: number | null;
  readonly usage: ClaudeChildUsage | null;
  readonly sessionId: string | null;
  readonly effectiveModel: string | null;
  readonly detail: string;
}

const usageFromResult = (message: ClaudeCodeMessage): ClaudeChildUsage | null => {
  if (message.type !== "result") return null;
  const usage = message.usage;
  if (usage === undefined) return null;
  const input = usage.input_tokens ?? 0;
  const output = usage.output_tokens ?? 0;
  const cacheRead = usage.cache_read_input_tokens ?? 0;
  const cacheWrite = usage.cache_creation_input_tokens ?? 0;
  return {
    inputTokens: input,
    cachedInputTokens: cacheRead + cacheWrite,
    outputTokens: output,
    reasoningTokens: 0,
    totalTokens: input + output,
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
  } = { usage: null, sessionId: null, effectiveModel: null, failure: null };

  // Tee the raw SDK messages the adapter consumes (usage, session id,
  // effective model, error) without disturbing the adapter's projection.
  const teedQuery: ClaudeCodeQuery = (params) => {
    const iterable = input.query(params);
    return (async function* () {
      for await (const message of iterable) {
        if (message.type === "system" && message.subtype === "init") {
          if (message.session_id.length > 0) wire.sessionId = message.session_id;
          if (typeof (message as { model?: string }).model === "string") {
            wire.effectiveModel = (message as { model?: string }).model ?? null;
          }
        }
        if (message.type === "result") {
          wire.usage = usageFromResult(message);
          if (message.is_error === true) {
            wire.failure = (message as { result?: string }).result ?? "claude turn failed";
          }
        }
        yield message;
      }
    })();
  };

  const adapter = makeClaudeCodeHarnessAdapter({
    query: teedQuery,
    cwd: input.workspace,
    model: input.model,
    queryOverrides: input.queryOverrides,
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
