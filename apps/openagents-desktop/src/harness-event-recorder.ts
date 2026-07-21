// HARN-03 (live wiring) + HARN-06 (cursor liveness): record every dispatched
// turn's `ClaudeLocalEvent` stream into a durable, cursor-exact neutral harness
// event log (`@openagentsinc/agent-harness-contract`). This is the additive
// integration that makes the harness stack USED in the live dispatch path: the
// dispatcher already calls `onTurnEventProjected` for every emitted event, so we
// attach here, project each event onto the neutral `KhalaRuntimeEvent` stream via
// the HARN-03 projector, and append it to the HARN-02 event-log store keyed by
// turn. Existing behavior is unchanged — this only observes.
//
// The recorded log gives each turn a monotonic cursor and a last-event kind,
// which HARN-06 reads as cursor-exact Full Auto liveness (`{cursor, lastEventKind}`),
// and which is replayable for inspection.

import { Effect } from "effect";
import {
  makeInMemoryEventLogStore,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract";
import type { KhalaRuntimeLane, KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import type { ClaudeLocalEvent } from "./claude-local-contract.ts";
import { makeClaudeLocalHarnessProjector } from "./harness-projection.ts";

/** Cursor-exact liveness for one turn, read by Full Auto (HARN-06). */
export interface HarnessTurnLiveness {
  readonly turnRef: string;
  /** Last neutral event sequence recorded for the turn, or -1 when none. */
  readonly cursor: number;
  /** Kind of the last neutral event (`text.delta`, `tool.call`, `turn.finished`, ...). */
  readonly lastEventKind?: string;
  /** Number of neutral events recorded for the turn. */
  readonly eventCount: number;
}

export interface HarnessEventRecorder {
  /** Observe one dispatched `ClaudeLocalEvent`. Never throws into the dispatcher. */
  readonly observe: (params: {
    readonly threadRef: string;
    readonly turnRef: string;
    /** Optional lane hint for the event source label. Absence only affects the label. */
    readonly graphLaneRef?: string;
    readonly event: ClaudeLocalEvent;
  }) => void;
  /** Current cursor-exact liveness for a turn, or `undefined` if unseen. */
  readonly liveness: (turnRef: string) => HarnessTurnLiveness | undefined;
  /** Replay the recorded neutral events for a turn (inspection / demo). */
  readonly replay: (turnRef: string) => ReadonlyArray<HarnessStreamEvent>;
}

const LANE_SOURCE: Readonly<
  Record<
    string,
    { readonly lane: KhalaRuntimeLane; readonly adapterKind: KhalaRuntimeSource["adapterKind"] }
  >
> = {
  claude_local: { lane: "claude_pylon", adapterKind: "claude_code" },
  codex_local: { lane: "codex_app_server", adapterKind: "codex" },
  grok_acp: { lane: "agent_client_protocol", adapterKind: "grok_cli" },
  cursor_acp: { lane: "agent_client_protocol", adapterKind: "cursor_cli" },
};

const sourceFor = (graphLaneRef: string | undefined): KhalaRuntimeSource => {
  const mapped = graphLaneRef === undefined ? undefined : LANE_SOURCE[graphLaneRef];
  if (mapped === undefined) return { lane: "test_fixture" };
  return { lane: mapped.lane, adapterKind: mapped.adapterKind };
};

interface TurnState {
  readonly project: (event: ClaudeLocalEvent) => ReadonlyArray<HarnessStreamEvent>;
  cursor: number;
  lastEventKind: string | undefined;
  eventCount: number;
}

/**
 * Build the recorder. Uses the in-memory event-log STORE directly (not the
 * PubSub-backed runtime) so appends run synchronously inside the dispatcher's
 * void callback. The store keys events by turn id, so one store serves every
 * turn; per-turn projectors keep sequence numbers contiguous.
 */
export const makeHarnessEventRecorder = (): HarnessEventRecorder => {
  const store = makeInMemoryEventLogStore();
  const turns = new Map<string, TurnState>();

  const stateFor = (turnRef: string, graphLaneRef: string | undefined): TurnState => {
    const existing = turns.get(turnRef);
    if (existing !== undefined) return existing;
    const created: TurnState = {
      project: makeClaudeLocalHarnessProjector({
        turnId: turnRef,
        threadId: turnRef,
        source: sourceFor(graphLaneRef),
      }),
      cursor: -1,
      lastEventKind: undefined,
      eventCount: 0,
    };
    turns.set(turnRef, created);
    return created;
  };

  const observe: HarnessEventRecorder["observe"] = ({ turnRef, graphLaneRef, event }) => {
    try {
      const state = stateFor(turnRef, graphLaneRef);
      const neutral = state.project(event);
      for (const neutralEvent of neutral) {
        // Best-effort durable append; a rejected duplicate never breaks the turn.
        Effect.runSync(store.append(neutralEvent).pipe(Effect.catch(() => Effect.void)));
        state.cursor = neutralEvent.sequence;
        state.lastEventKind = neutralEvent.kind;
        state.eventCount += 1;
      }
    } catch {
      // The recorder is a pure observer. It must never disturb dispatch.
    }
  };

  const liveness: HarnessEventRecorder["liveness"] = (turnRef) => {
    const state = turns.get(turnRef);
    if (state === undefined) return undefined;
    return {
      turnRef,
      cursor: state.cursor,
      ...(state.lastEventKind === undefined ? {} : { lastEventKind: state.lastEventKind }),
      eventCount: state.eventCount,
    };
  };

  const replay: HarnessEventRecorder["replay"] = (turnRef) =>
    Effect.runSync(store.read({ turnId: turnRef, fromCursor: -1 }));

  return { observe, liveness, replay };
};
