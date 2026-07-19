import { describe, expect, test } from "vite-plus/test"
import { Context, Effect, Exit, Layer, Scope } from "effect"

import {
  IdeOutputChannelRefSchema,
  IdeTaskDefinitionRefSchema,
  IdeTaskDefinitionSchema,
  IdeTerminalReconnectGenerationSchema,
  IdeTerminalSessionRefSchema,
  IdeTerminalSessionSchema,
  IdeTerminalSplitRefSchema,
} from "./run-contract.ts"
import { IdeAttachmentGenerationSchema } from "./project-contract.ts"
import {
  ideRunFixtureBinding,
  ideRunFixtureController,
  ideRunFixtureEnvironment,
  ideRunFixtureExecutable,
  ideRunFixtureProfile,
  ideRunFixtureSnapshot,
  ideRunFixtureTask,
} from "./run-fixture.ts"
import {
  IdeRunService,
  IdeRunStale,
  makeIdeRunServiceLayer,
  type IdeRunServiceShape,
} from "./run-service.ts"

const owner = { _tag: "Human" as const, actorRef: "owner.desktop" }

const withService = async <A>(run: (service: IdeRunServiceShape) => Promise<A>): Promise<A> => {
  const scope = await Effect.runPromise(Scope.make())
  const context = await Effect.runPromise(Layer.buildWithScope(makeIdeRunServiceLayer(
    ideRunFixtureSnapshot(),
    { now: () => "2026-07-19T12:00:00.000Z", outputByteLimit: 32 },
  ), scope))
  try {
    return await run(Context.get(context, IdeRunService))
  } finally {
    await Effect.runPromise(Scope.close(scope, Exit.void))
  }
}

describe("IDE-10 Effect run service", () => {
  test("owns a terminal lifecycle, exact output sequence, bounded gap, and teardown", async () => {
    await withService(async (service) => {
      const sessionRef = IdeTerminalSessionRefSchema.make("ide.terminal.fixture")
      const outputChannelRef = IdeOutputChannelRefSchema.make("ide.output-channel.terminal.fixture")
      await Effect.runPromise(service.terminalStarted({
        actor: owner,
        session: IdeTerminalSessionSchema.make({
          sessionRef,
          title: "Fixture",
          profileRef: ideRunFixtureProfile().profileRef,
          splitRef: IdeTerminalSplitRefSchema.make("ide.terminal-split.fixture"),
          binding: ideRunFixtureBinding(),
          environment: ideRunFixtureEnvironment(),
          executable: { ...ideRunFixtureExecutable(), source: "profile" },
          outputChannelRef,
          cols: 80,
          rows: 24,
          reconnectGeneration: IdeTerminalReconnectGenerationSchema.make(1),
          shellIntegration: ["links"],
          lifecycle: { _tag: "Running", startedAt: "2026-07-19T12:00:00.000Z", pidPresent: true },
        }),
      }))
      for (const text of ["12345678901234567890", "abcdefghijklmnopqrst", "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"]) {
        await Effect.runPromise(service.appendOutput({
          channelRef: outputChannelRef,
          producer: { _tag: "Terminal", sessionRef },
          stream: "pty",
          text,
          byteLength: text.length,
          redacted: false,
          truncated: false,
          gapBefore: false,
          invalidEncoding: false,
          locations: [],
        }))
      }
      const beforeStop = await Effect.runPromise(service.snapshot)
      const channel = beforeStop.outputChannels[0]!
      expect(channel.lastSequence).toBe(3)
      expect(channel.gap).toBe(true)
      expect(channel.retainedBytes).toBeLessThanOrEqual(32)
      expect(channel.chunks[0]?.text).toBe("efghijklmnopqrstuvwxyz0123456789")
      expect(channel.droppedBytes).toBe(70)
      expect(beforeStop.receipts[0]?.operation).toBe("terminal_create")
      const stopped = await Effect.runPromise(service.stop("test complete"))
      expect(stopped.stopped).toBe(true)
      expect(stopped.outputChannels[0]?.disposed).toBe(true)
    })
  })

  test("does not turn process exit zero into task or test success without semantic evidence", async () => {
    await withService(async (service) => {
      await Effect.runPromise(service.replaceDiscovery([ideRunFixtureTask()], [ideRunFixtureController()]))
      const task = await Effect.runPromise(service.startTask({ definitionRef: ideRunFixtureTask().definitionRef, actor: owner }))
      const ready = await Effect.runPromise(service.taskReady(task.runRef))
      expect(ready.outcome._tag).toBe("Ready")
      const failedTask = await Effect.runPromise(service.settleTask({
        runRef: task.runRef,
        exitCode: 0,
        cancelled: false,
        timedOut: false,
        semanticChecksPassed: false,
        problems: [],
        artifacts: [],
      }))
      expect(failedTask.outcome._tag).toBe("Failed")

      const controller = ideRunFixtureController()
      const testRun = await Effect.runPromise(service.startTests({
        controllerRef: controller.controllerRef,
        itemRefs: [controller.items[1]!.itemRef],
        profile: "run",
        actor: owner,
        retryOf: null,
      }))
      const failedTests = await Effect.runPromise(service.settleTests({
        runRef: testRun.runRef,
        exitCode: 0,
        cancelled: false,
        assertionsObserved: false,
        results: [{ itemRef: controller.items[1]!.itemRef, status: "passed", durationMs: 1, message: null, location: null }],
        artifacts: [],
        coveragePercent: null,
      }))
      expect(failedTests.outcome._tag).toBe("Failed")
      const snapshot = await Effect.runPromise(service.snapshot)
      expect(snapshot.receipts.filter((receipt) => receipt.operation === "task_run")).toHaveLength(2)
      expect(snapshot.receipts.filter((receipt) => receipt.operation === "test_run")).toHaveLength(2)
    })
  })

  test("refuses stale discovery bindings and cyclic dependencies", async () => {
    await withService(async (service) => {
      const stale = IdeTaskDefinitionSchema.make({
        ...ideRunFixtureTask(),
        binding: { ...ideRunFixtureBinding(), attachmentGeneration: IdeAttachmentGenerationSchema.make(2) },
      })
      const result = await Effect.runPromise(Effect.result(service.replaceDiscovery([stale], [])))
      expect(result._tag).toBe("Failure")
      if (result._tag === "Failure") expect(result.failure).toBeInstanceOf(IdeRunStale)

      const twoRef = IdeTaskDefinitionRefSchema.make("ide.task-definition.two")
      const one = IdeTaskDefinitionSchema.make({ ...ideRunFixtureTask(), dependencies: [twoRef] })
      const two = IdeTaskDefinitionSchema.make({ ...ideRunFixtureTask(), definitionRef: twoRef, dependencies: [one.definitionRef] })
      const cycle = await Effect.runPromise(Effect.result(service.replaceDiscovery([one, two], [])))
      expect(cycle._tag).toBe("Failure")
    })
  })
})
