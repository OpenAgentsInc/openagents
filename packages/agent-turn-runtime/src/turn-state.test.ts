import { describe, expect, test } from "vite-plus/test";

import {
  applyTurnEvent,
  bumpTurnGeneration,
  candidateRef,
  foldTurnEvents,
  initialTurnState,
  providerTurnRef,
  turnProviderRef,
  turnRequestRef,
  turnThreadRef,
  TURN_STATE_TRANSITION_CORPUS,
  TurnTransition,
} from "./turn-state.js";

const start = () => initialTurnState(turnRequestRef("request.state.1"), turnThreadRef("thread.state.1"));

describe("turn state machine", () => {
  test("every corpus scenario reaches its expected terminal record", () => {
    for (const scenario of TURN_STATE_TRANSITION_CORPUS) {
      const record = foldTurnEvents(start(), scenario.events);
      expect(record.state, scenario.name).toBe(scenario.expected.state);
      expect(record.generation, scenario.name).toBe(scenario.expected.generation);
      expect(record.progressCount, scenario.name).toBe(scenario.expected.progressCount);
      expect(record.candidateRef !== null, scenario.name).toBe(scenario.expected.hasCandidate);
      expect(record.refusalReason, scenario.name).toBe(scenario.expected.refusalReason);
      expect(record.failureReason, scenario.name).toBe(scenario.expected.failureReason ?? null);
    }
  });

  test("a terminal record accepts no further transition", () => {
    const completed = foldTurnEvents(start(), [
      TurnTransition.RouteStarted(),
      TurnTransition.RouteAdmitted({
        selected: turnProviderRef("provider.codex.1"),
        effective: turnProviderRef("provider.codex.1"),
      }),
      TurnTransition.ProviderStarted({ providerTurnRef: providerTurnRef("providerturn.1") }),
      TurnTransition.Completed({ candidateRef: candidateRef("candidate.1") }),
    ].map((transition) => ({ generation: 0, transition })));
    expect(completed.state).toBe("completed");
    const outcome = applyTurnEvent(completed, { generation: 0, transition: TurnTransition.Progress() });
    expect(outcome.ok).toBe(false);
    if (!outcome.ok) expect(outcome.reason).toBe("already_terminal");
  });

  test("an out-of-order transition is rejected without mutating state", () => {
    const routing = foldTurnEvents(start(), [{ generation: 0, transition: TurnTransition.RouteStarted() }]);
    const outcome = applyTurnEvent(routing, {
      generation: 0,
      transition: TurnTransition.ProviderStarted({ providerTurnRef: providerTurnRef("providerturn.1") }),
    });
    expect(outcome.ok).toBe(false);
    if (!outcome.ok) expect(outcome.reason).toBe("illegal_transition");
  });

  test("a late event from a superseded generation is fenced out", () => {
    const streaming = foldTurnEvents(start(), [
      TurnTransition.RouteStarted(),
      TurnTransition.RouteAdmitted({
        selected: turnProviderRef("provider.codex.1"),
        effective: turnProviderRef("provider.codex.1"),
      }),
      TurnTransition.ProviderStarted({ providerTurnRef: providerTurnRef("providerturn.1") }),
    ].map((transition) => ({ generation: 0, transition })));
    const superseded = bumpTurnGeneration(streaming);
    expect(superseded.generation).toBe(1);

    // A stale generation-0 completion must not change the active turn.
    const stale = applyTurnEvent(superseded, {
      generation: 0,
      transition: TurnTransition.Completed({ candidateRef: candidateRef("candidate.1") }),
    });
    expect(stale.ok).toBe(false);
    if (!stale.ok) expect(stale.reason).toBe("stale_generation");

    // The active generation-1 completion applies.
    const fresh = applyTurnEvent(superseded, {
      generation: 1,
      transition: TurnTransition.Completed({ candidateRef: candidateRef("candidate.1") }),
    });
    expect(fresh.ok).toBe(true);
    if (fresh.ok) expect(fresh.record.state).toBe("completed");
  });
});
