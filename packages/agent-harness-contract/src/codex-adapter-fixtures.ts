import { Effect, Stream } from "effect";
import type { CodexAppServerTransport, CodexEvent, CodexExecSpawner } from "./codex-adapter.ts";
import { CodexTransportError } from "./codex-adapter.ts";

/**
 * Hermetic fixtures for the Codex harness adapter tests: scripted
 * implementations of the injected {@link CodexAppServerTransport} and
 * {@link CodexExecSpawner} seams plus representative turn scripts, mirroring
 * the wire shapes the monorepo's Codex lanes consume (app-server v2
 * notifications; `codex exec --json` JSONL). No live codex binary or process
 * is involved — every call is recorded so tests can assert exactly what the
 * adapter asked the seam to do.
 */

/**
 * A representative app-server turn: streamed text + reasoning deltas, a
 * command execution with a native approval request, a file change, exact
 * token usage, and a completed turn.
 */
export const codexAppServerTurnScript: ReadonlyArray<CodexEvent> = [
  { type: "turn.started" },
  { type: "reasoning.delta", itemId: "item_r1", delta: "Plan the fix." },
  { type: "agent_message.delta", itemId: "item_m1", delta: "Running the check. " },
  {
    type: "item.started",
    item: {
      itemType: "command_execution",
      id: "call_1",
      commandDisplay: "pnpm test",
      status: "in_progress",
    },
  },
  {
    type: "approval.requested",
    requestId: "rpc_41",
    callId: "call_1",
    toolKind: "exec_command",
    displayText: "Allow Codex to run pnpm test?",
  },
  {
    type: "item.completed",
    item: {
      itemType: "command_execution",
      id: "call_1",
      commandDisplay: "pnpm test",
      status: "completed",
      exitCode: 0,
    },
  },
  {
    type: "item.completed",
    item: {
      itemType: "file_change",
      id: "patch_1",
      status: "completed",
      changes: [{ path: "src/index.ts", kind: "update" }],
    },
  },
  { type: "agent_message.delta", itemId: "item_m1", delta: "Done." },
  {
    type: "item.completed",
    item: { itemType: "agent_message", id: "item_m1", text: "Running the check. Done." },
  },
  {
    type: "token_usage.updated",
    usage: { inputTokens: 40, cachedInputTokens: 10, outputTokens: 12, reasoningOutputTokens: 5 },
  },
  { type: "turn.completed", status: "completed" },
];

/**
 * A representative `codex exec --json` turn: completions only (the exec
 * stream surfaces no deltas), thread identity for resume, and exact usage
 * carried inline on `turn.completed`.
 */
export const codexExecTurnScript: ReadonlyArray<CodexEvent> = [
  { type: "thread.started", threadId: "thread_exec_1" },
  { type: "turn.started" },
  {
    type: "item.completed",
    item: { itemType: "reasoning", id: "item_r1", text: "planned the reply" },
  },
  {
    type: "item.completed",
    item: {
      itemType: "command_execution",
      id: "call_1",
      commandDisplay: "ls",
      status: "completed",
      exitCode: 0,
    },
  },
  { type: "item.completed", item: { itemType: "agent_message", id: "item_m1", text: "All done." } },
  {
    type: "turn.completed",
    status: "completed",
    usage: { inputTokens: 20, cachedInputTokens: 4, outputTokens: 8, reasoningOutputTokens: 3 },
  },
];

/** One recorded `startThread` call. */
export interface RecordedStartThread {
  /** Absent in owner-local mode (currently-authenticated default home). */
  readonly codexHome?: string;
  readonly workingDirectory?: string;
  readonly model?: string;
  readonly resumeThreadId?: string;
}

/** One recorded `runTurn` call. */
export interface RecordedRunTurn {
  readonly threadId: string;
  readonly prompt: string;
}

/** One recorded approval response. */
export interface RecordedApprovalResponse {
  readonly requestId: string;
  readonly decision: string;
}

/** One recorded exec spawn. */
export interface RecordedSpawn {
  readonly codexBinaryPath: string;
  /** Absent in owner-local mode (currently-authenticated default home). */
  readonly codexHome?: string;
  readonly workingDirectory?: string;
  readonly model?: string;
  readonly prompt: string;
  readonly resumeThreadId?: string;
}

/** Call ledger + scripted app-server transport. */
export interface ScriptedCodexAppServerTransport {
  readonly transport: CodexAppServerTransport;
  readonly startThreadCalls: Array<RecordedStartThread>;
  readonly runTurnCalls: Array<RecordedRunTurn>;
  readonly approvalResponses: Array<RecordedApprovalResponse>;
  readonly interrupts: Array<{ readonly threadId: string; readonly turnId?: string }>;
  readonly steered: Array<{ readonly threadId: string; readonly text: string }>;
  readonly shutdowns: { count: number };
}

/**
 * Build a scripted app-server transport. Each `runTurn` call replays the next
 * script from `turnScripts` (the last script repeats when exhausted). A
 * configured `runTurnFailure` makes every `runTurn` fail with that typed
 * transport error instead.
 */
/**
 * A strictly single-element-chunk {@link Stream} over `script`, so a lazy
 * pull-based consumer observes each event ARRIVE and be projected before the
 * next event is produced (openagents#9167). `onProduce` records production
 * order for the incremental-emission proof. `Stream.fromIterable(script)` would
 * emit the whole script as one chunk and defeat that observation, so each
 * element is concatenated as its own one-element stream.
 */
const perElementCodexStream = (
  script: ReadonlyArray<CodexEvent>,
  onProduce?: (event: CodexEvent) => void,
): Stream.Stream<CodexEvent, CodexTransportError> =>
  script.reduce(
    (acc, event) =>
      acc.pipe(
        Stream.concat(
          Stream.fromIterable([event]).pipe(
            Stream.tap((produced) => Effect.sync(() => onProduce?.(produced))),
          ),
        ),
      ),
    Stream.empty as Stream.Stream<CodexEvent, CodexTransportError>,
  );

export const makeScriptedCodexAppServerTransport = (options?: {
  readonly threadId?: string;
  readonly turnScripts?: ReadonlyArray<ReadonlyArray<CodexEvent>>;
  readonly runTurnFailure?: CodexTransportError;
  readonly withSteer?: boolean;
  /**
   * When true, the transport also exposes the live streaming seam
   * {@link CodexAppServerTransport.runTurnStreaming} (openagents#9167),
   * emitting the same scripted events one at a time as a pull-based stream.
   * `runTurn` (batch) stays available for back-compat coverage.
   */
  readonly streaming?: boolean;
  /** Records each event AS the streaming transport produces it (incremental proof). */
  readonly onProduce?: (event: CodexEvent) => void;
  /** Fails the streaming turn stream with this typed error instead of completing. */
  readonly streamingFailure?: CodexTransportError;
}): ScriptedCodexAppServerTransport => {
  const threadId = options?.threadId ?? "thread_app_1";
  const turnScripts = options?.turnScripts ?? [codexAppServerTurnScript];
  const startThreadCalls: Array<RecordedStartThread> = [];
  const runTurnCalls: Array<RecordedRunTurn> = [];
  const approvalResponses: Array<RecordedApprovalResponse> = [];
  const interrupts: Array<{ readonly threadId: string; readonly turnId?: string }> = [];
  const steered: Array<{ readonly threadId: string; readonly text: string }> = [];
  const shutdowns = { count: 0 };

  const transport: CodexAppServerTransport = {
    startThread: (params) =>
      Effect.sync(() => {
        startThreadCalls.push(params);
        return { threadId: params.resumeThreadId ?? threadId };
      }),
    runTurn: (params) =>
      options?.runTurnFailure !== undefined
        ? Effect.fail(options.runTurnFailure)
        : Effect.sync(() => {
            runTurnCalls.push(params);
            const index = Math.min(runTurnCalls.length - 1, turnScripts.length - 1);
            return turnScripts[index] ?? [];
          }),
    ...(options?.streaming === true
      ? {
          runTurnStreaming: (params: { readonly threadId: string; readonly prompt: string }) => {
            runTurnCalls.push(params);
            const index = Math.min(runTurnCalls.length - 1, turnScripts.length - 1);
            const script = turnScripts[index] ?? [];
            const base = perElementCodexStream(script, options?.onProduce);
            return options?.streamingFailure !== undefined
              ? base.pipe(Stream.concat(Stream.fail(options.streamingFailure)))
              : base;
          },
        }
      : {}),
    respondToApproval: (params) =>
      Effect.sync(() => {
        approvalResponses.push(params);
      }),
    interruptTurn: (params) =>
      Effect.sync(() => {
        interrupts.push(params);
      }),
    ...(options?.withSteer === true
      ? {
          steerTurn: (params: { readonly threadId: string; readonly text: string }) =>
            Effect.sync(() => {
              steered.push(params);
            }),
        }
      : {}),
    shutdown: () =>
      Effect.sync(() => {
        shutdowns.count += 1;
      }),
  };

  return {
    transport,
    startThreadCalls,
    runTurnCalls,
    approvalResponses,
    interrupts,
    steered,
    shutdowns,
  };
};

/** Call ledger + scripted exec spawner. */
export interface ScriptedCodexExecSpawner {
  readonly spawner: CodexExecSpawner;
  readonly spawns: Array<RecordedSpawn>;
}

/**
 * Build a scripted exec spawner. Each `spawn` call replays the next script
 * from `spawnScripts` (the last script repeats when exhausted). A configured
 * `failure` makes every spawn fail with that typed transport error instead.
 */
export const makeScriptedCodexExecSpawner = (options?: {
  readonly spawnScripts?: ReadonlyArray<ReadonlyArray<CodexEvent>>;
  readonly failure?: CodexTransportError;
}): ScriptedCodexExecSpawner => {
  const spawnScripts = options?.spawnScripts ?? [codexExecTurnScript];
  const spawns: Array<RecordedSpawn> = [];

  const spawner: CodexExecSpawner = {
    spawn: (params) =>
      options?.failure !== undefined
        ? Effect.fail(options.failure)
        : Effect.sync(() => {
            spawns.push(params);
            const index = Math.min(spawns.length - 1, spawnScripts.length - 1);
            return spawnScripts[index] ?? [];
          }),
  };

  return { spawner, spawns };
};
