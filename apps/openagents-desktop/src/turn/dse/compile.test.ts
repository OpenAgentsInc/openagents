import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import { resolveReleasedArtifact } from "@openagentsinc/dse"

import { compileHonestChatArtifact, compileTurnRouteArtifact, type DseCompileBundle } from "./compile.ts"
import {
  HONESTY_INSTRUCTION_MARKER,
  ROUTE_INSTRUCTION_MARKER,
} from "./fixtures.ts"
import {
  HONEST_CHAT_POINTER,
  HONEST_CHAT_WINNER,
  TURN_ROUTE_POINTER,
  TURN_ROUTE_WINNER,
} from "./artifacts.generated.ts"

/**
 * AFS-09 offline compile exit checks: each compiled artifact beats its frozen
 * baseline on validation and holdout, carries an uncertainty record, reproduces
 * the checked-in bytes deterministically, resolves offline, and is promoted
 * under an independent review.
 */

const holdoutDelta = (bundle: DseCompileBundle): number =>
  bundle.holdoutReport.aggregateScore - bundle.baselineHoldoutReport.aggregateScore

describe("AFS-09 HonestChatReply.v1 compile", () => {
  test("the compiled artifact beats the baseline on validation and holdout with an uncertainty record", async () => {
    const bundle = await compileHonestChatArtifact()
    // The compiler selected the compiled honesty instruction on validation.
    expect(bundle.winner.program.promptIr.instruction).toContain(HONESTY_INSTRUCTION_MARKER)
    // It beats the hand-written baseline on holdout.
    expect(holdoutDelta(bundle)).toBeGreaterThan(0)
    expect(bundle.holdoutReport.split).toBe("holdout")
    // A small dataset carries an explicit uncertainty record.
    expect(bundle.uncertainty.method).toBe("small_sample_note")
    expect(bundle.uncertainty.holdoutDelta).toBeCloseTo(holdoutDelta(bundle), 5)
  })

  test("the compile is deterministic and reproduces the checked-in released bytes", async () => {
    const bundle = await compileHonestChatArtifact()
    expect(bundle.winner.digest).toBe(HONEST_CHAT_WINNER.digest)
    expect(bundle.winner.candidateId).toBe(HONEST_CHAT_WINNER.candidateId)
    expect(bundle.pointer.released.digest).toBe(HONEST_CHAT_POINTER.released.digest)
  })

  test("the released artifact resolves offline from the checked-in bytes", async () => {
    const resolved = await Effect.runPromise(
      resolveReleasedArtifact({
        pointer: HONEST_CHAT_POINTER,
        candidateBytes: HONEST_CHAT_WINNER,
        expectedSignatureId: "AppleFm/HonestChatReply.v1",
      }),
    )
    expect(resolved.program.promptIr.instruction).toContain(HONESTY_INSTRUCTION_MARKER)
  })

  test("promotion uses an independent reviewer distinct from the producer", async () => {
    const bundle = await compileHonestChatArtifact()
    // The pointer only exists because a reviewer distinct from the producer
    // admitted it (the DSE gate refuses a self-admit); it binds the winner bytes.
    expect(bundle.pointer.released.kind).toBe("prompt_program")
    expect(bundle.pointer.released.digest).toBe(bundle.winner.digest)
  })
})

describe("AFS-09 TurnRoute.v1 compile", () => {
  test("the compiled route artifact beats the baseline on holdout (fixes the refusal spiral)", async () => {
    const bundle = await compileTurnRouteArtifact()
    expect(bundle.winner.program.promptIr.instruction).toContain(ROUTE_INSTRUCTION_MARKER)
    expect(holdoutDelta(bundle)).toBeGreaterThan(0)
    expect(bundle.uncertainty.candidateHoldoutScore).toBeGreaterThan(bundle.uncertainty.baselineHoldoutScore)
  })

  test("the compile reproduces the checked-in released bytes and resolves offline", async () => {
    const bundle = await compileTurnRouteArtifact()
    expect(bundle.winner.digest).toBe(TURN_ROUTE_WINNER.digest)
    expect(bundle.pointer.released.digest).toBe(TURN_ROUTE_POINTER.released.digest)
    const resolved = await Effect.runPromise(
      resolveReleasedArtifact({
        pointer: TURN_ROUTE_POINTER,
        candidateBytes: TURN_ROUTE_WINNER,
        expectedSignatureId: "AppleFm/TurnRoute.v1",
      }),
    )
    expect(resolved.program.promptIr.instruction).toContain(ROUTE_INSTRUCTION_MARKER)
  })
})
