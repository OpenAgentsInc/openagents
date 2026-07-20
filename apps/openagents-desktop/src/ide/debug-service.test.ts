import { describe, expect, test } from "vite-plus/test"
import { Context, Effect, Exit, Layer, Scope } from "effect"

import {
  IdeDebugBreakpointRefSchema,
  IdeDebugBreakpointSchema,
  IdeDebugFrameRefSchema,
  IdeDebugOperationRefSchema,
  IdeDebugScopeRefSchema,
  IdeDebugSourceRefSchema,
  IdeDebugStaleEvent,
  IdeDebugThreadRefSchema,
  IdeDebugVariableRefSchema,
} from "./debug-contract.ts"
import {
  ideDebugFixtureConfiguration,
  ideDebugFixtureSnapshot,
} from "./debug-fixture.ts"
import {
  IdeDebugService,
  makeIdeDebugServiceLayer,
  type IdeDebugServiceShape,
} from "./debug-service.ts"
import {
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeFileRefSchema,
} from "./project-contract.ts"

const owner = { _tag: "Human" as const, actorRef: "owner.desktop" }
const secret = "fixture-private-token"
let operationSequence = 0
const operation = () => ({
  operationRef: IdeDebugOperationRefSchema.make(`ide.debug-operation.fixture-${++operationSequence}`),
})

const withService = async <A>(run: (service: IdeDebugServiceShape) => Promise<A>): Promise<A> => {
  const scope = await Effect.runPromise(Scope.make())
  const context = await Effect.runPromise(Layer.buildWithScope(makeIdeDebugServiceLayer(
    ideDebugFixtureSnapshot(),
    { now: () => "2026-07-19T20:00:00.000Z", consoleByteLimit: 32, secretValues: [secret] },
  ), scope))
  try {
    return await run(Context.get(context, IdeDebugService))
  } finally {
    await Effect.runPromise(Scope.close(scope, Exit.void))
  }
}

const fence = (session: Awaited<ReturnType<typeof startFixture>>) => ({
  sessionRef: session.sessionRef,
  sessionGeneration: session.sessionGeneration,
  adapterGeneration: session.adapterGeneration,
  targetGeneration: session.targetGeneration,
})

const startFixture = async (service: IdeDebugServiceShape, intent: "launch" | "attach" = "launch") =>
  Effect.runPromise(service.start({ ...operation(), configuration: ideDebugFixtureConfiguration(intent), actor: owner }))

const source = () => ({
  sourceRef: IdeDebugSourceRefSchema.make("ide.debug-source.fixture"),
  fileRef: IdeFileRefSchema.make("ide.file.fixture"),
  documentRef: IdeDocumentRefSchema.make("ide.document.fixture"),
  documentGeneration: IdeDocumentGenerationSchema.make(1),
  pathRef: "ide.path.fixture",
  label: "fixture.ts",
  origin: "project" as const,
  availability: "available" as const,
  sourceMapRef: null,
})

describe("IDE-11 Effect debug service", () => {
  test("validates and starts launch and attach through distinct disclosed receipts", async () => {
    await withService(async (service) => {
      const launchConfig = ideDebugFixtureConfiguration("launch")
      const validated = await Effect.runPromise(service.validate({
        ...launchConfig,
        intent: launchConfig.intent._tag === "Launch"
          ? { ...launchConfig.intent, argumentLabels: [`--token=${secret}`] }
          : launchConfig.intent,
      }, owner, operation().operationRef))
      expect(JSON.stringify(validated)).not.toContain(secret)
      const launch = await startFixture(service, "launch")
      const attach = await startFixture(service, "attach")
      expect(launch.configuration.intent._tag).toBe("Launch")
      expect(attach.configuration.intent._tag).toBe("Attach")
      const snapshot = await Effect.runPromise(service.snapshot)
      expect(snapshot.sessions).toHaveLength(2)
      expect(snapshot.receipts.map((receipt) => receipt.operation)).toEqual(["validate", "launch", "attach"])
      expect(snapshot.receipts.every((receipt) => receipt.environmentDigest === "environment-digest-fixture")).toBe(true)
      expect(JSON.stringify(snapshot)).not.toContain(secret)
    })
  })

  test("refuses remote attach without an admitted authentication reference", async () => {
    await withService(async (service) => {
      const configuration = ideDebugFixtureConfiguration("attach")
      if (configuration.intent._tag !== "Attach") throw new Error("The attach fixture returned a launch intent.")
      const result = await Effect.runPromise(Effect.result(service.start({
        ...operation(),
        configuration: { ...configuration, intent: { ...configuration.intent, authenticationRef: null } },
        actor: owner,
      })))
      expect(result._tag).toBe("Failure")
      const snapshot = await Effect.runPromise(service.snapshot)
      expect(snapshot.sessions).toHaveLength(0)
      expect(snapshot.receipts).toHaveLength(0)
    })
  })

  test("preserves stable breakpoints and gates unsupported breakpoint and control capabilities", async () => {
    await withService(async (service) => {
      const supported = await startFixture(service)
      const sourceBreakpoint = IdeDebugBreakpointSchema.cases.Source.make({
        breakpointRef: IdeDebugBreakpointRefSchema.make("ide.debug-breakpoint.fixture"),
        enabled: true,
        condition: `token=${secret}`,
        hitCondition: null,
        logMessage: null,
        verified: false,
        message: null,
        location: { source: source(), line: 4, column: 2, endLine: null, endColumn: null },
        requestedLine: 4,
        sourceVersion: IdeDocumentGenerationSchema.make(1),
      })
      const changed = await Effect.runPromise(service.replaceBreakpoints({
        ...fence(supported), ...operation(), actor: owner, breakpoints: [sourceBreakpoint],
      }))
      expect(changed.breakpoints[0]?.breakpointRef).toBe(sourceBreakpoint.breakpointRef)
      expect(changed.breakpoints[0]?.condition).toBe("token=[REDACTED]")
      const restarted = await Effect.runPromise(service.control({
        ...fence(changed), ...operation(), actor: owner, operation: "restart_session",
      }))
      expect(restarted.breakpoints[0]?.breakpointRef).toBe(sourceBreakpoint.breakpointRef)
      const persisted = restarted.breakpoints[0]
      expect(persisted?._tag).toBe("Source")
      if (persisted?._tag === "Source") expect(persisted.sourceVersion).toBe(IdeDocumentGenerationSchema.make(1))

      const unsupportedServiceScope = await Effect.runPromise(Scope.make())
      const unsupportedContext = await Effect.runPromise(Layer.buildWithScope(makeIdeDebugServiceLayer(
        ideDebugFixtureSnapshot(), { now: () => "2026-07-19T20:00:00.000Z" },
      ), unsupportedServiceScope))
      try {
        const unsupportedService = Context.get(unsupportedContext, IdeDebugService)
        const session = await Effect.runPromise(unsupportedService.start({
          ...operation(), configuration: ideDebugFixtureConfiguration("launch", ["function_breakpoints", "step_back"]), actor: owner,
        }))
        const functionResult = await Effect.runPromise(Effect.result(unsupportedService.replaceBreakpoints({
          ...fence(session), ...operation(), actor: owner,
          breakpoints: [IdeDebugBreakpointSchema.cases.Function.make({
            breakpointRef: IdeDebugBreakpointRefSchema.make("ide.debug-breakpoint.function"), enabled: true,
            condition: null, hitCondition: null, logMessage: null, verified: false, message: null, functionName: "main",
          })],
        })))
        expect(functionResult._tag).toBe("Failure")
        const stepResult = await Effect.runPromise(Effect.result(unsupportedService.control({
          ...fence(session), ...operation(), actor: owner, operation: "step_back",
        })))
        expect(stepResult._tag).toBe("Failure")
      } finally {
        await Effect.runPromise(Scope.close(unsupportedServiceScope, Exit.void))
      }
    })
  })

  test("fences old generations after restart and does not mutate the new session", async () => {
    await withService(async (service) => {
      const session = await startFixture(service)
      const oldFence = fence(session)
      const restarted = await Effect.runPromise(service.control({ ...oldFence, ...operation(), actor: owner, operation: "restart_session" }))
      expect(restarted.sessionGeneration).toBe(2)
      expect(restarted.adapterGeneration).toBe(2)
      expect(restarted.targetGeneration).toBe(2)
      const late = await Effect.runPromise(Effect.result(service.applyAdapterEvent({
        ...oldFence,
        event: { _tag: "Output", category: "stdout", text: `late ${secret}` },
      })))
      expect(late._tag).toBe("Failure")
      if (late._tag === "Failure") expect(late.failure).toBeInstanceOf(IdeDebugStaleEvent)
      const snapshot = await Effect.runPromise(service.snapshot)
      expect(snapshot.sessions[0]?.console).toHaveLength(0)
      expect(JSON.stringify(snapshot)).not.toContain(secret)
    })
  })

  test("bounds and redacts adapter projections, evaluations, console, and teardown", async () => {
    await withService(async (service) => {
      const session = await startFixture(service)
      const currentFence = fence(session)
      const threadRef = IdeDebugThreadRefSchema.make("ide.debug-thread.fixture")
      const frameRef = IdeDebugFrameRefSchema.make("ide.debug-frame.fixture")
      const scopeRef = IdeDebugScopeRefSchema.make("ide.debug-scope.fixture")
      await Effect.runPromise(service.applyAdapterEvent({
        ...currentFence,
        event: {
          _tag: "Projection",
          threads: [{ threadRef, name: "main", state: "stopped", stopReason: "breakpoint" }],
          frames: [{ frameRef, threadRef, name: "main", location: { source: source(), line: 4, column: 2, endLine: null, endColumn: null }, moduleRef: null, canRestart: true }],
          scopes: [{ scopeRef, frameRef, name: "Locals", expensive: false, variableCount: 1, state: "ready" }],
          variables: [{
            variableRef: IdeDebugVariableRefSchema.make("ide.debug-variable.fixture"), parentRef: null, scopeRef,
            name: "token", value: secret, type: "string", evaluateName: "token", childCount: 0, redacted: false, truncated: false,
          }],
          modules: [], loadedSources: [source()],
        },
      }))
      const updatedVariable = await Effect.runPromise(service.recordSetVariable({
        ...currentFence, ...operation(),
        actor: owner,
        variableRef: IdeDebugVariableRefSchema.make("ide.debug-variable.fixture"),
        value: `updated ${secret}`,
        type: "string",
      }))
      expect(updatedVariable.value).toBe("updated [REDACTED]")
      expect(updatedVariable.redacted).toBe(true)
      const navigated = await Effect.runPromise(service.navigateSource({ ...currentFence, ...operation(), actor: owner, source: source() }))
      expect(navigated.documentGeneration).toBe(IdeDocumentGenerationSchema.make(1))
      const watch = await Effect.runPromise(service.recordEvaluation({
        ...currentFence, ...operation(), actor: owner, expression: `token=${secret}`, value: `${secret}${"x".repeat(32_000)}`, type: "string", failedMessage: null,
      }))
      expect(watch.redacted).toBe(true)
      expect(watch.truncated).toBe(true)
      await Effect.runPromise(service.applyAdapterEvent({
        ...currentFence, event: { _tag: "Output", category: "stdout", text: `${secret}${"y".repeat(128)}` },
      }))
      const beforeCleanup = await Effect.runPromise(service.snapshot)
      expect(beforeCleanup.sessions[0]?.variables[0]?.value).toBe("updated [REDACTED]")
      expect(beforeCleanup.sessions[0]?.retainedConsoleBytes).toBeLessThanOrEqual(32)
      expect(beforeCleanup.sessions[0]?.droppedConsoleBytes).toBeGreaterThan(0)
      expect(JSON.stringify(beforeCleanup)).not.toContain(secret)
      const terminated = await Effect.runPromise(service.control({
        ...currentFence, ...operation(), actor: owner, operation: "terminate",
      }))
      expect(terminated.lifecycle._tag).toBe("Terminated")
      expect(terminated.variables).toHaveLength(0)
      const lateAfterTerminate = await Effect.runPromise(Effect.result(service.applyAdapterEvent({
        ...currentFence, event: { _tag: "Output", category: "stdout", text: `after terminate ${secret}` },
      })))
      expect(lateAfterTerminate._tag).toBe("Failure")
      const cleaned = await Effect.runPromise(service.cleanup(operation().operationRef, `cleanup ${secret}`, owner))
      expect(cleaned.stopped).toBe(true)
      expect(cleaned.sessions[0]?.lifecycle._tag).toBe("Terminated")
      expect(cleaned.sessions[0]?.variables).toHaveLength(0)
      expect(cleaned.sessions[0]?.console).toHaveLength(0)
      expect(cleaned.receipts.at(-1)?.operation).toBe("cleanup")
      expect(JSON.stringify(cleaned)).not.toContain(secret)
    })
  })
})
