/**
 * #9161: a live Codex `ProviderLane` for the headless host. It wraps the
 * HARN-09 codex harness attempt (`runCodexHarnessExecAttempt`) behind the
 * production `ProviderLane` SPI, so a real owner-local Codex turn runs
 * through the same `makeProviderLaneDispatcher` path the Desktop uses —
 * with no renderer.
 *
 * Owner-local: the child env leaves `CODEX_HOME` unset (the developer's
 * currently-authenticated Codex home). The lane is the real-provider seam a
 * `createHeadlessHost` acceptance script plugs in; a scripted lane covers
 * the unit tests.
 */

import { defaultSpawnCodex } from "./codex-child-runtime";
import type { ClaudeLocalEvent } from "./claude-local-contract";
import { runCodexHarnessExecAttempt } from "./codex-harness-attempt";
import type { ProviderLane } from "./provider-lane";

export interface CodexHeadlessLaneOptions {
  /** Working directory for the turn (framework-created, disposable). */
  readonly workspace: string;
  /** Model id (default `gpt-5.6-terra`). */
  readonly model?: string;
  /** Reasoning effort (default `medium`). */
  readonly reasoningEffort?: string;
  /** Sandbox policy (default `read-only`). */
  readonly sandbox?: "read-only" | "workspace-write";
  /** Per-turn timeout (default 300000ms). */
  readonly timeoutMs?: number;
}

/** Build an owner-local Codex provider lane for the headless host. */
export const makeCodexHeadlessLane = (options: CodexHeadlessLaneOptions): ProviderLane<null> => {
  const model = options.model ?? "gpt-5.6-terra";
  // Track the codex thread id across the turn for provider-session continuity.
  const state: { threadId: string | null } = { threadId: null };
  return {
    laneRef: "codex-local",
    graphLaneRef: "codex_local",
    eventChannel: "openagents:codex-local:event",
    usageProvider: "codex",
    capabilities: () => ({
      laneRef: "codex-local",
      provider: "codex",
      models: [model],
      features: {
        skills: false,
        planOnly: false,
        reasoningEffort: true,
        images: false,
        fullAuto: true,
        interrupt: true,
        queueFollowup: false,
        steerTurn: false,
        steerChild: false,
        answerQuestion: false,
      },
      composer: {
        displayName: "Codex (headless)",
        reasoningEfforts: ["low", "medium", "high"],
        permissionModes: ["owner_full"],
        approvals: "none",
        extensions: [],
      },
      policy: {
        source: "native-static-declaration",
        profileRef: "native:codex-local-headless:v1",
        evidence: "conformant",
        allowedModels: [model],
        allowedFeatures: ["fullAuto", "reasoningEffort", "interrupt"],
        allowedExtensions: [],
      },
      recovery: "provider_session_replay",
    }),
    admit: () => ({ ok: true, model, context: null }),
    streamMeta: (ctx) => ({ lane: "codex-local", turnRef: ctx.request.turnRef }),
    modelNoteText: (m) => `Codex · ${m}`,
    runTurn: async ({ request, emit, message }) => {
      const env = { ...process.env };
      // Owner-local: never override CODEX_HOME.
      delete env.CODEX_HOME;
      const attempt = await runCodexHarnessExecAttempt({
        threadRef: request.threadRef,
        turnRef: request.turnRef,
        workspace: options.workspace,
        prompt: message,
        model,
        reasoningEffort: options.reasoningEffort ?? "medium",
        sandbox: options.sandbox ?? "read-only",
        imagePaths: [],
        resumeThreadId: state.threadId,
        env,
        spawnCodex: defaultSpawnCodex,
        emit: (event: ClaudeLocalEvent) => emit(event),
        registerChild: () => {},
        ...(options.timeoutMs === undefined ? {} : { timeoutMs: options.timeoutMs }),
      });
      if (attempt.threadId !== null) state.threadId = attempt.threadId;
      if (attempt.outcome === "success") {
        const usage =
          attempt.usage === null
            ? undefined
            : {
                inputTokens: attempt.usage.inputTokens,
                cachedInputTokens: attempt.usage.cachedInputTokens,
                outputTokens: attempt.usage.outputTokens,
                reasoningTokens: attempt.usage.reasoningOutputTokens,
                totalTokens: attempt.usage.totalTokens,
              };
        return {
          ok: true,
          text: attempt.text,
          totalTokens: attempt.usage?.totalTokens ?? null,
          ...(usage === undefined ? {} : { usage }),
          providerSessionRef: attempt.threadId,
        };
      }
      const reason =
        attempt.outcome === "reconnect_required"
          ? ("session_failed" as const)
          : attempt.outcome === "timeout"
            ? ("timeout" as const)
            : ("session_failed" as const);
      return { ok: false, reason, detail: attempt.detail };
    },
    interrupt: () => false,
    finalMeta: (ctx) => ({ lane: "codex-local", turnRef: ctx.request.turnRef }),
    failureMessage: (reason, detail) => `Codex turn failed (${reason} · ${detail}).`,
  };
};
