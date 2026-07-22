import { Deferred, Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  makeDesktopSourceSafePoint,
  type DesktopSourceBinding,
  type DesktopSourceSubsystem,
} from "./desktop-source-safe-point.ts"

const binding: DesktopSourceBinding = {
  sessionRef: "session.alpha",
  attachmentRef: "attachment.alpha.7",
  grantRef: "grant.alpha",
  generation: 7,
}

const subsystem = (
  name: string,
  quiesce: DesktopSourceSubsystem["quiesce"],
): DesktopSourceSubsystem => ({ subsystem: name, quiesce })

describe("Desktop source safe point", () => {
  test("refuses stale bindings before a helper runs", async () => {
    let called = false
    const service = makeDesktopSourceSafePoint({
      currentBinding: () => binding,
      subsystems: [subsystem("workspace", async () => {
        called = true
        return { state: "quiesced" }
      })],
    })
    const result = await service.quiesce({ ...binding, generation: 6 })
    expect(result).toMatchObject({ state: "refused", reason: "stale_binding" })
    expect(called).toBe(false)
  })

  test("starts independent helpers in parallel and rejects work permanently after the safe point", async () => {
    const enteredA = await Effect.runPromise(Deferred.make<void>())
    const enteredB = await Effect.runPromise(Deferred.make<void>())
    const release = await Effect.runPromise(Deferred.make<void>())
    const service = makeDesktopSourceSafePoint({
      currentBinding: () => binding,
      subsystems: [
        subsystem("workspace", async () => {
          await Effect.runPromise(Deferred.succeed(enteredA, undefined))
          await Effect.runPromise(Deferred.await(release))
          return { state: "quiesced" }
        }),
        subsystem("provider", async () => {
          await Effect.runPromise(Deferred.succeed(enteredB, undefined))
          await Effect.runPromise(Deferred.await(release))
          return { state: "quiesced" }
        }),
      ],
    })
    const first = service.quiesce(binding)
    await Effect.runPromise(Deferred.await(enteredA))
    await Effect.runPromise(Deferred.await(enteredB))
    await Effect.runPromise(Deferred.succeed(release, undefined))
    const result = await first
    expect(result.state).toBe("quiescent")
    expect(result.outcomes.map(value => value.subsystem)).toEqual(["workspace", "provider"])
    expect(result.outcomes.every(value => value.executionClaim === "local_cleanup_only")).toBe(true)
    expect(result.remoteExecution).toBe("not_claimed")
    expect(await service.quiesce(binding)).toMatchObject({
      state: "refused",
      reason: "already_quiesced",
    })
  })

  for (const testCase of [
    {
      name: "blocked operation",
      make: async () => await new Promise<never>(() => undefined),
      state: "timed_out",
    },
    {
      name: "failed operation",
      make: async () => { throw new Error("private diagnostic") },
      state: "failed",
    },
    {
      name: "unsupported operation",
      make: async () => ({ state: "unsupported", detailRef: "desktop.helper.no-safe-point" } as const),
      state: "unsupported",
    },
    {
      name: "reported cleanup timeout",
      make: async () => ({ state: "timed_out", detailRef: "desktop.helper.cleanup-timeout" } as const),
      state: "timed_out",
    },
    {
      name: "reported cleanup failure",
      make: async () => ({ state: "failed", detailRef: "desktop.helper.cleanup-failed" } as const),
      state: "failed",
    },
  ] as const) {
    test(`records ${testCase.name} as failed safe-point evidence`, async () => {
      const service = makeDesktopSourceSafePoint({
        currentBinding: () => binding,
        timeoutMs: 10,
        subsystems: [subsystem("target", testCase.make)],
      })
      const result = await service.quiesce(binding)
      expect(result.state).toBe("not_quiescent")
      expect(result.outcomes[0]).toMatchObject({
        subsystem: "target",
        state: testCase.state,
        executionClaim: "local_cleanup_only",
      })
      expect(result.outcomes[0]?.evidenceRef).toMatch(/^ide\.source-safe-point\.[a-f0-9]{64}$/u)
      expect(JSON.stringify(result)).not.toContain("private diagnostic")
    })
  }

  test("does not report success when the attachment changes during quiescence", async () => {
    let current = binding
    const service = makeDesktopSourceSafePoint({
      currentBinding: () => current,
      subsystems: [subsystem("workspace", async () => {
        current = { ...binding, generation: 8 }
        return { state: "quiesced" }
      })],
    })
    const result = await service.quiesce(binding)
    expect(result.state).toBe("not_quiescent")
    expect(result.outcomes[0]?.state).toBe("quiesced")
  })
})
