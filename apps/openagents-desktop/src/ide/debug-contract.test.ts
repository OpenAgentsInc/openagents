import { describe, expect, test } from "vite-plus/test"
import { Exit, Schema } from "effect"

import {
  IdeDebugAdapterEventSchema,
  IdeDebugCommandSchema,
  IdeDebugConfigurationSchema,
  IdeDebugOperationRefSchema,
  IdeDebugSnapshotSchema,
} from "./debug-contract.ts"
import {
  ideDebugFixtureConfiguration,
  ideDebugFixtureSnapshot,
} from "./debug-fixture.ts"

const encodeSnapshot = Schema.encodeSync(IdeDebugSnapshotSchema)
const decodeSnapshot = Schema.decodeUnknownSync(IdeDebugSnapshotSchema)
const decodeConfiguration = Schema.decodeUnknownSync(IdeDebugConfigurationSchema)
const decodeConfigurationExit = Schema.decodeUnknownExit(IdeDebugConfigurationSchema)
const decodeAdapterEventExit = Schema.decodeUnknownExit(IdeDebugAdapterEventSchema)
const decodeCommandExit = Schema.decodeUnknownExit(IdeDebugCommandSchema)

describe("IDE-11 debug contract", () => {
  test("round-trips separate launch and attach configurations without secret values", () => {
    const launch = ideDebugFixtureConfiguration("launch")
    const attach = ideDebugFixtureConfiguration("attach")
    const snapshot = IdeDebugSnapshotSchema.make({
      ...ideDebugFixtureSnapshot(),
      configurations: [launch, attach],
    })
    const encoded = encodeSnapshot(snapshot)
    expect(decodeSnapshot(encoded)).toEqual(snapshot)
    expect(launch.intent._tag).toBe("Launch")
    expect(attach.intent._tag).toBe("Attach")
    expect(JSON.stringify(encoded)).not.toContain("API_TOKEN=")
    expect(launch.environment.valuesExposedToRenderer).toBe(false)
    expect(launch.sourceMaps.guessPositions).toBe(false)
  })

  test("rejects malformed generations, raw environment maps, and dead attach reuse", () => {
    const launch = ideDebugFixtureConfiguration("launch")
    const attach = ideDebugFixtureConfiguration("attach")
    expect(Exit.isFailure(decodeConfigurationExit({
      ...launch,
      binding: { ...launch.binding, attachmentGeneration: 0 },
    }))).toBe(true)
    const decoded = decodeConfiguration({
      ...launch,
      environment: { ...launch.environment, values: { API_TOKEN: "private" } },
    })
    expect(JSON.stringify(decoded)).not.toContain("private")
    expect(JSON.stringify(decoded)).not.toContain('"values":')
    expect(Exit.isFailure(decodeConfigurationExit({
      ...attach,
      intent: { ...attach.intent, reusedDeadAttachment: true },
    }))).toBe(true)
  })

  test("bounds protocol projections at the schema boundary", () => {
    const oversizedOutput = {
      _tag: "Output",
      category: "stdout",
      text: "x".repeat(262_145),
    }
    expect(Exit.isFailure(decodeAdapterEventExit(oversizedOutput))).toBe(true)
    expect(Exit.isFailure(decodeAdapterEventExit({
      _tag: "Projection",
      threads: [], frames: [], scopes: [], variables: [], modules: [],
      loadedSources: Array.from({ length: 20_001 }, () => ({})),
    }))).toBe(true)
  })

  test("derives generation-fenced commands from the same debug graph", () => {
    const command = IdeDebugCommandSchema.cases.Start.make({
      operationRef: IdeDebugOperationRefSchema.make("ide.debug-operation.contract-start"),
      configurationRef: ideDebugFixtureConfiguration("launch").configurationRef,
      actor: { _tag: "Human", actorRef: "owner.desktop" },
    })
    expect(Exit.isSuccess(decodeCommandExit(command))).toBe(true)
    expect(Exit.isFailure(decodeCommandExit({
      _tag: "Control",
      operationRef: "ide.debug-operation.contract-control",
      sessionRef: "../../escape",
      sessionGeneration: 1,
      adapterGeneration: 1,
      targetGeneration: 1,
      operation: "continue",
      actor: { _tag: "Human", actorRef: "owner.desktop" },
    }))).toBe(true)
  })
})
