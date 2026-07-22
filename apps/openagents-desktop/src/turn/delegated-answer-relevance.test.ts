import { describe, expect, test } from "vite-plus/test"

import { Schema as S } from "effect"
import { SafeTurnProjection } from "@openagentsinc/agent-runtime-schema"

import {
  DELEGATED_RESULT_MISMATCH_REASON,
  admitDelegatedAnswerProjection,
  evaluateDelegatedAnswer,
} from "./delegated-answer-relevance.ts"

const decodeProjection = S.decodeUnknownSync(SafeTurnProjection)
const projectionWith = (answer: string) =>
  decodeProjection({
    schema: "openagents.agent_turn_projection.v1",
    threadRef: "thread.1",
    requestRef: "request.delegate.1",
    cardState: "done",
    candidate: "claude",
    dataDestination: "remote_provider",
    usageTruth: "exact",
    localOnly: true,
    updatedAt: "2026-07-22T02:00:00.000Z",
    messageChain: [{ entryRef: "answer.1", role: "assistant", text: answer }],
    evidenceRefs: [],
  })

describe("delegated answer relevance admission", () => {
  test("accepts a substantive answer connected to the current objective", () => {
    expect(evaluateDelegatedAnswer({
      objective: "Implement issue #9159 and verify ordinary chat routing.",
      answer: "Implemented the #9159 chat-routing fix and ran the focused verification.",
    })).toMatchObject({ kind: "accepted" })
  })

  test("rejects missing and receipt-only results", () => {
    expect(evaluateDelegatedAnswer({ objective: "Fix the login bug", answer: "" })).toEqual({
      kind: "rejected",
      reason: "missing_answer",
    })
    expect(evaluateDelegatedAnswer({ objective: "Fix the login bug", answer: "Done." })).toEqual({
      kind: "rejected",
      reason: "receipt_only",
    })
  })

  test("rejects the reported identity-to-unrelated-release exchange", () => {
    expect(evaluateDelegatedAnswer({
      objective: "hey who are you",
      answer: "Done — one concrete thing, completed: Packet A step 1 verification, Full Auto release gate.",
    })).toEqual({ kind: "rejected", reason: "objective_mismatch" })
  })

  test("accepts a substantive result when a short objective has no stable lexical anchor", () => {
    expect(evaluateDelegatedAnswer({
      objective: "fix bug",
      answer: "Resolved the failure and added a regression test.",
    })).toMatchObject({ kind: "accepted" })
  })

  test("marks a completed mismatched projection failed before promotion", () => {
    const projection = admitDelegatedAnswerProjection({
      objective: "Fix the login bug",
      projection: projectionWith("Finished an unrelated documentation inventory."),
    })
    expect(projection.cardState).toBe("failed")
    expect(projection.failureReason).toBe(DELEGATED_RESULT_MISMATCH_REASON)
  })
})
