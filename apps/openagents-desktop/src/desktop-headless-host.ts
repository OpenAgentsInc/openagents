/**
 * #9161 host-context bootstrap: an isolated, electron-free Desktop host that
 * runs turns through the PRODUCTION provider-lane services — the real
 * `makeProviderLaneDispatcher`, the real `ThreadStore`, the real
 * `LocalTurnJournal`, and the real Full Auto registry — without launching
 * the renderer or driving DOM selectors.
 *
 * This is the supported agent/evaluation control boundary for the Desktop
 * HOST layer (as opposed to the SDK-adapter layer proven by the harness
 * smokes). A caller creates a host over a disposable root, creates threads,
 * submits ordinary turns against a provider lane, and reads back the ordered
 * typed frames plus durable thread and Full Auto state. The lane is injected
 * (a live codex lane via the harness attempt, or a scripted lane for tests),
 * so the host assembly itself needs no live provider.
 *
 * Invariants proven here: an ordinary turn persists its user note and the
 * provider frames to the durable thread, and creates NO Full Auto run
 * record — Full Auto authority is never a side effect of an ordinary turn.
 */

import { mkdirSync } from "node:fs";
import { join } from "node:path";
import type { ClaudeLocalEvent, ClaudeLocalStartRequest } from "./claude-local-contract";
import type { DesktopThread } from "./chat-contract";
import { openFullAutoRegistry } from "./full-auto-registry";
import type { FullAutoRun } from "./full-auto-run-registry";
import { openFullAutoRunRegistry } from "./full-auto-run-registry";
import { openLocalTurnJournal } from "./local-turn-journal";
import type { ProviderLane, ProviderLaneDispatchResult } from "./provider-lane";
import { makeProviderLaneDispatcher } from "./provider-lane";
import { makeThreadStore } from "./thread-store";

export interface HeadlessHostOptions {
  /** Disposable root for threads.json / turns.json / full-auto state. */
  readonly root: string;
}

export interface HeadlessTurnFrame {
  readonly turnRef: string;
  readonly event: ClaudeLocalEvent;
}

export interface HeadlessTurnResult {
  readonly dispatch: ProviderLaneDispatchResult;
  /** Ordered typed frames the dispatcher forwarded for this turn. */
  readonly frames: ReadonlyArray<HeadlessTurnFrame>;
  /** The durable thread after the turn settled (or null if it aged out). */
  readonly thread: DesktopThread | null;
  /** Full Auto run records present after the turn (ordinary turns: empty). */
  readonly fullAutoRecordCount: number;
}

export interface HeadlessHost {
  createThread: (title?: string) => DesktopThread;
  listThreads: () => ReadonlyArray<DesktopThread>;
  /**
   * Submit ONE ordinary (non-Full-Auto) turn against the given lane and
   * capture its typed frames + durable state. `fullAuto` is forced false —
   * an ordinary turn can never create Full Auto authority.
   */
  submitOrdinaryTurn: <Context>(params: {
    readonly lane: ProviderLane<Context>;
    readonly threadRef: string;
    readonly turnRef: string;
    readonly message: string;
    readonly model?: string;
  }) => Promise<HeadlessTurnResult>;
  fullAutoRecordCount: () => number;
  /**
   * Explicitly start a Full Auto run through the production run registry.
   * Unlike an ordinary turn, this is the ONLY path that creates a run
   * record. Returns the started run (with its stable `runRef`).
   */
  startFullAutoRun: (params: {
    readonly title: string;
    readonly objective: string;
    readonly doneCondition: string;
    readonly threadRef?: string;
    readonly turnCap?: number;
  }) => FullAutoRun;
  /** The Full Auto run records present in the durable registry. */
  fullAutoRuns: () => ReadonlyArray<FullAutoRun>;
}

/** Construct an isolated headless Desktop host over a disposable root. */
export const createHeadlessHost = (options: HeadlessHostOptions): HeadlessHost => {
  const root = options.root;
  mkdirSync(join(root, "full-auto"), { recursive: true });
  const threads = makeThreadStore(join(root, "threads.json"));
  const journal = openLocalTurnJournal(join(root, "turns.json"));
  const registry = openFullAutoRegistry(join(root, "full-auto", "registry.json"));
  const runRegistry = openFullAutoRunRegistry(join(root, "full-auto", "runs.json"));

  const dispatcher = makeProviderLaneDispatcher({
    threads: () => threads,
    journal,
    liveAgentGraph: { beginTurn: () => {}, applyEvent: () => {} },
    usageLedger: { record: () => {} },
    captureTurnCheckpoint: async () => {},
    localTurnFlushers: new Set(),
    isQuitting: () => false,
  });

  return {
    createThread: (title) => threads.newThread(title),
    listThreads: () => threads.list(),
    fullAutoRecordCount: () => registry.list().length,
    fullAutoRuns: () => runRegistry.list(),
    startFullAutoRun: ({ title, objective, doneCondition, threadRef, turnCap }) => {
      const result = runRegistry.startNew({
        title,
        objective,
        doneCondition,
        objectiveSource: "control_caller",
        actor: "control_api",
        reason: "headless host explicit Full Auto start",
        ...(threadRef === undefined ? {} : { threadRef }),
        ...(turnCap === undefined ? {} : { turnCap }),
      });
      if (!result.ok) {
        throw new Error(`Full Auto start refused: ${result.reason}`);
      }
      return result.run;
    },
    submitOrdinaryTurn: async ({ lane, threadRef, turnRef, message, model }) => {
      const frames: HeadlessTurnFrame[] = [];
      // The real dispatcher forwards each projected event to a sender; the
      // headless host captures them in order instead of an IPC channel.
      const sender = {
        isDestroyed: () => false,
        send: (_channel: string, payload: unknown) => {
          const framed = payload as { turnRef: string; event: ClaudeLocalEvent };
          frames.push({ turnRef: framed.turnRef, event: framed.event });
        },
      };
      const request: ClaudeLocalStartRequest = {
        turnRef,
        threadRef,
        message,
        // Ordinary turn: NEVER Full Auto. The host forces this so an ordinary
        // submission cannot create Full Auto authority (the #9161 invariant).
        fullAuto: false,
        ...(model === undefined ? {} : { model }),
      };
      const dispatch = await dispatcher.dispatchTurn(lane, request, sender);
      const thread = threads.list().find((candidate) => candidate.id === threadRef) ?? null;
      return {
        dispatch,
        frames,
        thread,
        fullAutoRecordCount: registry.list().length,
      };
    },
  };
};
