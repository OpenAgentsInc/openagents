import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Effect } from "effect"
import { afterEach, beforeEach, describe, expect, test } from "vite-plus/test"

import {
  cardStateForLifecycle,
  foldTurnEvents,
  initialTurnState,
  TurnJournal,
  turnRequestRef,
  turnThreadRef,
  TURN_STATE_TRANSITION_CORPUS,
} from "@openagentsinc/agent-turn-runtime"

import { desktopTurnJournalLayer } from "./desktop-turn-journal.ts"

const requestRef = turnRequestRef("request.desktop.1")
const threadRef = turnThreadRef("thread.desktop.1")

let dir: string
const journalFile = (): string => path.join(dir, "agent-turns", "journal.json")

beforeEach(() => {
  dir = mkdtempSync(path.join(tmpdir(), "afs-desktop-journal-"))
})
afterEach(() => {
  rmSync(dir, { recursive: true, force: true })
})

const runJournal = <A>(filePath: string, body: Effect.Effect<A, unknown, TurnJournal>): Promise<A> =>
  Effect.runPromise(body.pipe(Effect.provide(desktopTurnJournalLayer({ filePath }))))

describe("Desktop turn journal transition adapter", () => {
  test("reproduces the shared state-transition corpus terminal state", async () => {
    for (const scenario of TURN_STATE_TRANSITION_CORPUS) {
      const record = foldTurnEvents(initialTurnState(requestRef, threadRef), scenario.events)
      const loaded = await runJournal(
        journalFile(),
        Effect.gen(function* () {
          const journal = yield* TurnJournal
          yield* journal.record(record)
          return yield* journal.load(requestRef)
        }),
      )
      expect(loaded, scenario.name).toEqual(record)
    }
  })

  test("a fresh journal over the same file reconstructs the terminal card (renderer reload)", async () => {
    const file = journalFile()
    const terminal = foldTurnEvents(
      initialTurnState(requestRef, threadRef),
      TURN_STATE_TRANSITION_CORPUS.find((scenario) => scenario.name === "completes deterministically")!.events,
    )
    // Write with one journal instance.
    await runJournal(
      file,
      Effect.gen(function* () {
        const journal = yield* TurnJournal
        yield* journal.record(terminal)
      }),
    )
    // Reload from a brand-new journal instance over the same durable file.
    const reloaded = await runJournal(
      file,
      Effect.gen(function* () {
        const journal = yield* TurnJournal
        return yield* journal.load(requestRef)
      }),
    )
    expect(reloaded).not.toBeNull()
    expect(reloaded && cardStateForLifecycle(reloaded.state)).toBe("done")
  })
})
