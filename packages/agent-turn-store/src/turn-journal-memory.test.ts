import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  cardStateForLifecycle,
  foldTurnEvents,
  initialTurnState,
  TurnJournal,
  turnRequestRef,
  turnThreadRef,
  TURN_STATE_TRANSITION_CORPUS,
} from "@openagentsinc/agent-turn-runtime";

import {
  decodeTurnRecord,
  encodeTurnRecord,
  inMemoryTurnJournalLayer,
  migratePersistedTurnRecord,
} from "./turn-journal-memory.js";

const requestRef = turnRequestRef("request.store.1");
const threadRef = turnThreadRef("thread.store.1");

const runJournal = <A>(body: Effect.Effect<A, unknown, TurnJournal>): Promise<A> =>
  Effect.runPromise(body.pipe(Effect.provide(inMemoryTurnJournalLayer)));

describe("driver-neutral turn journal", () => {
  test("round-trips every kernel record through the persisted schema", () => {
    for (const scenario of TURN_STATE_TRANSITION_CORPUS) {
      const record = foldTurnEvents(initialTurnState(requestRef, threadRef), scenario.events);
      const restored = decodeTurnRecord(encodeTurnRecord(record));
      expect(restored, scenario.name).toEqual(record);
    }
  });

  test("the in-memory adapter reproduces the shared corpus terminal state", async () => {
    for (const scenario of TURN_STATE_TRANSITION_CORPUS) {
      const record = foldTurnEvents(initialTurnState(requestRef, threadRef), scenario.events);
      const loaded = await runJournal(
        Effect.gen(function* () {
          const journal = yield* TurnJournal;
          yield* journal.record(record);
          return yield* journal.load(requestRef);
        }),
      );
      expect(loaded, scenario.name).toEqual(record);
    }
  });

  test("a renderer reload reconstructs the terminal card without replaying an action", async () => {
    // Persist a completed terminal record, then load it from a fresh journal and
    // derive the card. No provider run, no action broker, no replay.
    const terminal = foldTurnEvents(
      initialTurnState(requestRef, threadRef),
      TURN_STATE_TRANSITION_CORPUS.find((scenario) => scenario.name === "completes deterministically")!.events,
    );
    const reloaded = await runJournal(
      Effect.gen(function* () {
        const journal = yield* TurnJournal;
        yield* journal.record(terminal);
        return yield* journal.load(requestRef);
      }),
    );
    expect(reloaded).not.toBeNull();
    expect(reloaded && cardStateForLifecycle(reloaded.state)).toBe("done");
  });

  test("an unversioned persisted value migrates to the current schema", () => {
    const record = foldTurnEvents(initialTurnState(requestRef, threadRef), []);
    const persisted = encodeTurnRecord(record);
    const { schema: _schema, ...unversioned } = persisted;
    const migrated = migratePersistedTurnRecord(unversioned);
    expect(migrated).toHaveProperty("schema");
    expect(decodeTurnRecord(unversioned)).toEqual(record);
  });
});
