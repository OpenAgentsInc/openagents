import { Effect, Schema as S } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  AmbientTaskRunner,
  CommitMessageDraftInput,
  commitMessageDraftSignature,
} from "@openagentsinc/apple-fm-runtime"

import type { AppleFmHost } from "../apple-fm-host.ts"
import type { AppleFmStatus, AppleFmTurnResult } from "../apple-fm-contract.ts"
import {
  desktopAmbientTaskRunnerLayer,
  forkDesktopAmbientTask,
  runDesktopAmbientTask,
} from "./ambient-tasks.ts"

const readyStatus: AppleFmStatus = {
  schema: "openagents.desktop.apple_fm.status.v1",
  supported: true,
  state: "ready",
  readiness: "ready",
  ready: true,
  mode: "local_launched",
  model: "apple-foundation-model",
  profileId: "apple-fm-local",
  usageTruth: "estimated",
  unavailableReason: null,
  blockerRefs: [],
}

const completedTurn = (text: string): AppleFmTurnResult => ({
  schema: "openagents.desktop.apple_fm.turn.v1",
  ok: true,
  outcome: "completed",
  text,
  usageTruth: "estimated",
  promptTokens: null,
  completionTokens: null,
  totalTokens: null,
  failureClass: null,
})

const unavailableStatus: AppleFmStatus = {
  ...readyStatus,
  ready: false,
  state: "unavailable",
  readiness: "unavailable",
  mode: "none",
  unavailableReason: "not_ready",
}

const makeFakeHost = (options: {
  readonly ready?: boolean
  readonly runTurn?: (prompt: string) => Promise<AppleFmTurnResult>
}): AppleFmHost => {
  const ready = options.ready ?? true
  const status = (): AppleFmStatus => (ready ? readyStatus : unavailableStatus)
  const runTurn =
    options.runTurn ?? ((_prompt: string) => Promise.resolve(completedTurn("Refactor parser and cover tokenizer path")))
  return {
    status,
    ensureStarted: () => Promise.resolve(status()),
    refresh: () => Promise.resolve(status()),
    runTurn,
    stop: () => status(),
    dispose: () => {},
  }
}

const commitFacts = S.decodeUnknownSync(CommitMessageDraftInput)({
  sourceControlRef: "sc.main.1",
  branch: "main",
  stagedFileCount: 2,
  diffSummary: "Refactor the parser and add tokenizer tests.",
})

describe("AFS-07 Desktop ambient-task wiring", () => {
  test("drafts a commit message advisory over a ready host with zero-token provenance", async () => {
    const outcome = await runDesktopAmbientTask(
      { signature: commitMessageDraftSignature, facts: commitFacts },
      () => makeFakeHost({}),
    )
    expect(outcome._tag).toBe("Completed")
    if (outcome._tag !== "Completed") return
    expect(outcome.result.subject.length).toBeGreaterThan(0)
    expect(outcome.provenance.advisory).toBe(true)
    expect(outcome.provenance.dataDestination).toBe("on_device_local")
    expect(outcome.provenance.usageTruth).toBe("estimated")
    // Zero-token: no accounting fields leak into the advisory provenance.
    for (const forbidden of ["promptTokens", "completionTokens", "totalTokens"]) {
      expect(Object.keys(outcome.provenance)).not.toContain(forbidden)
    }
  })

  test("degrades to not_ready when no host is present (no failure surface)", async () => {
    const outcome = await runDesktopAmbientTask(
      { signature: commitMessageDraftSignature, facts: commitFacts },
      () => null,
    )
    expect(outcome._tag).toBe("Degraded")
    if (outcome._tag === "Degraded") expect(outcome.reason).toBe("not_ready")
  })

  test("degrades to not_ready when the host is unavailable", async () => {
    const outcome = await runDesktopAmbientTask(
      { signature: commitMessageDraftSignature, facts: commitFacts },
      () => makeFakeHost({ ready: false }),
    )
    expect(outcome._tag).toBe("Degraded")
    if (outcome._tag === "Degraded") expect(outcome.reason).toBe("not_ready")
  })

  test("passes the host runTurn a bounded prompt within the 4000-char contract", async () => {
    let seenPrompt = ""
    await runDesktopAmbientTask({ signature: commitMessageDraftSignature, facts: commitFacts }, () =>
      makeFakeHost({
        runTurn: (prompt) => {
          seenPrompt = prompt
          return Promise.resolve(completedTurn("Refactor parser"))
        },
      }),
    )
    expect(seenPrompt.length).toBeGreaterThan(0)
    expect(seenPrompt.length).toBeLessThanOrEqual(4000)
    expect(seenPrompt).toContain("Refactor the parser and add tokenizer tests.")
  })

  test("forkDesktopAmbientTask is non-blocking and cancellable", async () => {
    let resolveTurn!: (result: AppleFmTurnResult) => void
    const pending = new Promise<AppleFmTurnResult>((resolve) => {
      resolveTurn = resolve
    })
    const dispatch = forkDesktopAmbientTask(
      { signature: commitMessageDraftSignature, facts: commitFacts },
      () => makeFakeHost({ runTurn: () => pending }),
    )
    // The dispatch returned immediately without awaiting the pending turn.
    expect(typeof dispatch.cancel).toBe("function")
    dispatch.cancel()
    const outcome = await dispatch.outcome
    expect(outcome._tag).toBe("Cancelled")
    // Release the pending turn so no handle leaks.
    resolveTurn(completedTurn("late"))
  })

  test("the runner layer resolves the AmbientTaskRunner service", async () => {
    const outcome = await Effect.runPromise(
      AmbientTaskRunner.pipe(
        Effect.flatMap((runner) => runner.run({ signature: commitMessageDraftSignature, facts: commitFacts })),
        Effect.provide(desktopAmbientTaskRunnerLayer(() => makeFakeHost({}))),
      ),
    )
    expect(outcome._tag).toBe("Completed")
  })
})
