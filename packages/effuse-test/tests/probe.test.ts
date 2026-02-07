import * as Fs from "node:fs/promises"
import * as Os from "node:os"
import * as Path from "node:path"

import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"

import { ProbeServiceLive, ProbeService } from "../src/effect/ProbeService.ts"
import type { TestEvent } from "../src/spec.ts"

describe("ProbeService", () => {
  it.live("writes events.jsonl and flushes", () =>
    Effect.scoped(
      Effect.gen(function* () {
        const dir = yield* Effect.tryPromise({
          try: () => Fs.mkdtemp(Path.join(Os.tmpdir(), "effuse-test-probe-")),
          catch: (e) => (e instanceof Error ? e : new Error(String(e))),
        })

        yield* Effect.addFinalizer(() =>
          Effect.tryPromise({
            try: () => Fs.rm(dir, { recursive: true, force: true }),
            catch: (e) => (e instanceof Error ? e : new Error(String(e))),
          }).pipe(Effect.catchAll(() => Effect.void)),
        )

        const eventsPath = Path.join(dir, "events.jsonl")
        const layer = ProbeServiceLive({ eventsPath, capacity: 8 })

        const events: Array<TestEvent> = [
          { type: "log", runId: "r1", ts: 1, level: "info", message: "one" },
          { type: "log", runId: "r1", ts: 2, level: "info", message: "two" },
        ]

        yield* Effect.gen(function* () {
          const probe = yield* ProbeService
          for (const e of events) yield* probe.emit(e)
          yield* probe.flush
        }).pipe(Effect.provide(layer))

        const content = yield* Effect.tryPromise({
          try: () => Fs.readFile(eventsPath, "utf8"),
          catch: (e) => (e instanceof Error ? e : new Error(String(e))),
        })

        const lines = content.trim().split("\n")
        expect(lines.length).toBe(2)
        expect(JSON.parse(lines[0]!).message).toBe("one")
        expect(JSON.parse(lines[1]!).message).toBe("two")
      }),
    ),
  )
})
