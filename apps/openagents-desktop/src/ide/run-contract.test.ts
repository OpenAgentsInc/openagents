import { describe, expect, test } from "vite-plus/test"
import { Schema } from "effect"

import {
  IdeRunCommandSchema,
  IdeRunSnapshotSchema,
  decodeIdeRunCommand,
  decodeIdeRunSnapshot,
} from "./run-contract.ts"
import {
  ideRunFixtureController,
  ideRunFixtureSnapshot,
  ideRunFixtureTask,
} from "./run-fixture.ts"

describe("IDE-10 run contract", () => {
  test("round-trips one identified terminal/task/test/Output graph", () => {
    const fixture = ideRunFixtureSnapshot()
    const snapshot = IdeRunSnapshotSchema.make({
      ...fixture,
      taskDefinitions: [ideRunFixtureTask()],
      testControllers: [ideRunFixtureController()],
    })
    const encoded = Schema.encodeSync(IdeRunSnapshotSchema)(snapshot)
    expect(decodeIdeRunSnapshot(encoded)).toEqual(snapshot)
    expect(JSON.stringify(encoded)).not.toContain(process.env.PATH ?? "unmatchable-path")
    expect(snapshot.profiles[0]?.executable.shellInterpolation).toBe(false)
  })

  test("decodes tagged commands and rejects executable/argv injection fields", () => {
    const command = IdeRunCommandSchema.cases.StartTask.make({
      definitionRef: ideRunFixtureTask().definitionRef,
      actor: { _tag: "Human", actorRef: "owner.desktop" },
    })
    expect(decodeIdeRunCommand(command)).toEqual(command)
    expect(decodeIdeRunCommand({ ...command, executable: "/bin/sh", argv: ["-c", "rm"] })).toBeNull()
    expect(decodeIdeRunCommand({ _tag: "RunTests", controllerRef: "../../escape" })).toBeNull()
  })

  test("refuses malformed output sequences and raw environment values", () => {
    const snapshot = ideRunFixtureSnapshot()
    expect(decodeIdeRunSnapshot({ ...snapshot, outputChannels: [{ channelRef: "bad" }] })).toBeNull()
    expect(decodeIdeRunSnapshot({
      ...snapshot,
      profiles: [{ ...snapshot.profiles[0], environment: { API_TOKEN: "secret" } }],
    })).toBeNull()
  })
})
