/**
 * HARN-09 (#9167) Slice 1: run one codex-local exec turn THROUGH the SDK
 * harness adapter (`makeCodexHarnessAdapter`, exec mode) instead of the
 * hand-written spawn/parse path — behind the strangler flag
 * `OPENAGENTS_DESKTOP_CODEX_HARNESS_ADAPTER=1` (default off, zero behavior
 * change when unset).
 *
 * Division of labor: the DESKTOP keeps custody — the receipted exec arg
 * recipe, account environment selection, spawn seam, interrupt control,
 * journal `onDispatch` timing, and redaction stay here; the ADAPTER owns the
 * neutral projection and session/turn lifecycle; `harness-lowering` maps the
 * neutral stream back onto the frozen `ClaudeLocalEvent` renderer envelope.
 * Exact usage and the codex thread id are teed from the raw wire in the
 * spawner (never reconstructed from optional neutral fields).
 */

import type { CodexEvent, CodexExecSpawner } from "@openagentsinc/agent-harness-contract";
import {
  CodexTransportError,
  makeCodexHarnessAdapter,
  parseLiveCodexExecLine,
} from "@openagentsinc/agent-harness-contract";
import { Effect, Stream } from "effect";
import type { ClaudeLocalEvent } from "./claude-local-contract";
import type { CodexChildSpawn } from "./codex-child-runtime";
import type { CodexChildUsage } from "./codex-child-contract";
import { lowerHarnessEvent } from "./harness-lowering";

export interface CodexHarnessAttemptInput {
  readonly threadRef: string;
  readonly turnRef: string;
  readonly workspace: string;
  readonly prompt: string;
  readonly model: string;
  readonly reasoningEffort: string;
  readonly sandbox: string;
  readonly imagePaths: ReadonlyArray<string>;
  readonly resumeThreadId: string | null;
  readonly env: Record<string, string | undefined>;
  readonly spawnCodex: CodexChildSpawn;
  readonly emit: (event: ClaudeLocalEvent) => void;
  readonly registerChild: (child: { readonly kill: (signal?: NodeJS.Signals) => boolean }) => void;
  readonly timeoutMs?: number;
}

export interface CodexHarnessAttemptResult {
  readonly outcome: "success" | "reconnect_required" | "failed" | "timeout";
  readonly text: string;
  readonly usage: CodexChildUsage | null;
  readonly threadId: string | null;
  readonly detail: string;
  readonly quotaExhausted: boolean;
  readonly rateLimited: boolean;
}

/** The desktop's receipted exec arg recipe (mirrors the legacy path exactly). */
export const codexHarnessExecArgs = (
  input: Pick<
    CodexHarnessAttemptInput,
    "model" | "reasoningEffort" | "sandbox" | "imagePaths" | "workspace" | "prompt"
  >,
  resumeThreadId: string | null,
): ReadonlyArray<string> => {
  const imageFlags = input.imagePaths.flatMap((imagePath) => ["-i", imagePath]);
  return resumeThreadId === null
    ? [
        "exec",
        "--json",
        "-m",
        input.model,
        "-c",
        `model_reasoning_effort=${input.reasoningEffort}`,
        "-s",
        input.sandbox,
        "--skip-git-repo-check",
        ...imageFlags,
        "-C",
        input.workspace,
        input.prompt,
      ]
    : [
        "exec",
        "resume",
        resumeThreadId,
        "--json",
        "-m",
        input.model,
        "-c",
        `model_reasoning_effort=${input.reasoningEffort}`,
        "-c",
        `sandbox_mode="${input.sandbox}"`,
        ...imageFlags,
        "--skip-git-repo-check",
        input.prompt,
      ];
};

const classifyFailure = (
  detail: string,
): Pick<CodexHarnessAttemptResult, "outcome" | "quotaExhausted" | "rateLimited"> => {
  const lowered = detail.toLowerCase();
  if (
    lowered.includes("token could not be refreshed") ||
    lowered.includes("refresh token was revoked") ||
    lowered.includes("sign in again")
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
 * Run one exec turn through the adapter. The wire tee captures exact usage
 * and the codex-native thread id from the raw `CodexEvent` stream.
 */
export const runCodexHarnessExecAttempt = async (
  input: CodexHarnessAttemptInput,
): Promise<CodexHarnessAttemptResult> => {
  const wire: {
    usage: CodexChildUsage | null;
    threadId: string | null;
    failure: string | null;
  } = {
    usage: null,
    threadId: null,
    failure: null,
  };

  const spawner: CodexExecSpawner = {
    spawn: (params) =>
      Effect.callback<ReadonlyArray<CodexEvent>, CodexTransportError>((resume) => {
        const child = input.spawnCodex({
          args: [...codexHarnessExecArgs(input, params.resumeThreadId ?? null)],
          env: input.env,
          cwd: input.workspace,
        });
        if (child === null) {
          resume(
            Effect.fail(
              new CodexTransportError({
                failureClass: "spawn_failed",
                detail: "codex binary did not spawn",
              }),
            ),
          );
          return;
        }
        input.registerChild(child);
        let stdout = "";
        let stderr = "";
        let settled = false;
        const timer = setTimeout(() => {
          if (settled) return;
          settled = true;
          child.kill("SIGKILL");
          resume(
            Effect.fail(
              new CodexTransportError({
                failureClass: "turn_timeout",
                detail: `codex exec timed out after ${input.timeoutMs ?? 600_000}ms`,
              }),
            ),
          );
        }, input.timeoutMs ?? 600_000);
        child.stdout?.on("data", (chunk: Buffer) => {
          stdout += chunk.toString("utf8");
        });
        child.stderr?.on("data", (chunk: Buffer) => {
          stderr += chunk.toString("utf8");
        });
        child.on("close", () => {
          if (settled) return;
          settled = true;
          clearTimeout(timer);
          const events: CodexEvent[] = [];
          for (const line of stdout.split("\n")) {
            const event = parseLiveCodexExecLine(line);
            if (event !== null) events.push(event);
          }
          // Wire tee: exact usage + native thread id from the raw stream.
          for (const event of events) {
            if (event.type === "thread.started") wire.threadId = event.threadId;
            if (event.type === "turn.failed") wire.failure = event.messageSafe;
            if (event.type === "error" && wire.failure === null) wire.failure = event.messageSafe;
            if (event.type === "turn.completed" && event.usage !== undefined) {
              const usage = event.usage;
              wire.usage = {
                inputTokens: usage.inputTokens,
                cachedInputTokens: usage.cachedInputTokens,
                outputTokens: usage.outputTokens,
                reasoningOutputTokens: usage.reasoningOutputTokens,
                totalTokens: usage.inputTokens + usage.outputTokens + usage.reasoningOutputTokens,
              };
            }
          }
          if (events.length === 0) {
            resume(
              Effect.fail(
                new CodexTransportError({
                  failureClass: "execution_failed",
                  detail: stderr.trim().slice(0, 400) || "no parseable exec events",
                }),
              ),
            );
            return;
          }
          resume(Effect.succeed(events));
        });
      }),
  };

  const adapter = makeCodexHarnessAdapter({
    mode: "exec",
    codexBinaryPath: "codex",
    workingDirectory: input.workspace,
    model: input.model,
    spawner,
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
    const detail = String(
      (error as { detail?: string }).detail ?? (error as Error).message ?? error,
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
