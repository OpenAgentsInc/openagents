import { describe, expect, test } from "vite-plus/test"
import {
  decodeAppleFmStartTurnRequest,
  decodeAppleFmStatus,
  decodeAppleFmStopResult,
  decodeAppleFmTurnResult,
  invalidAppleFmTurn,
  notSupportedAppleFmStatus,
  refusedNotReadyAppleFmTurn,
  refusedUnsupportedAppleFmTurn,
  unavailableAppleFmStatus,
  unavailableAppleFmStopResult,
} from "./apple-fm-contract.ts"

describe("Apple FM IPC contract decode round-trips", () => {
  test("status projection round-trips and rejects out-of-vocabulary state", () => {
    const status = {
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
    expect(decodeAppleFmStatus(status)).toEqual(status)
    expect(decodeAppleFmStatus({ ...status, state: "definitely_not_a_state" })).toBeNull()
    expect(decodeAppleFmStatus({ ...status, readiness: "green" })).toBeNull()
    // A base URL or path can never survive the boundary: unknown fields are
    // stripped, so the projection returned to the renderer holds only the
    // declared public-safe keys.
    expect(decodeAppleFmStatus({ ...status, baseUrl: "http://127.0.0.1:11435" })).toEqual(status)
  })

  test("every public-safe constructor decodes as a valid projection", () => {
    for (const status of [notSupportedAppleFmStatus(), unavailableAppleFmStatus()]) {
      expect(decodeAppleFmStatus(status)).toEqual(status)
    }
    for (const turn of [refusedNotReadyAppleFmTurn(), refusedUnsupportedAppleFmTurn(), invalidAppleFmTurn()]) {
      expect(decodeAppleFmTurnResult(turn)).toEqual(turn)
    }
    expect(decodeAppleFmStopResult(unavailableAppleFmStopResult())).toEqual(unavailableAppleFmStopResult())
  })

  test("start-turn request bounds the single prompt field", () => {
    expect(decodeAppleFmStartTurnRequest({ prompt: "read the readme" })).toEqual({ prompt: "read the readme" })
    expect(decodeAppleFmStartTurnRequest({ prompt: "" })).toBeNull()
    expect(decodeAppleFmStartTurnRequest({ prompt: "x".repeat(4001) })).toBeNull()
    expect(decodeAppleFmStartTurnRequest({})).toBeNull()
    expect(decodeAppleFmStartTurnRequest({ prompt: 12 })).toBeNull()
    // A file-contents smuggle path is neutralized: unknown fields are stripped,
    // so only the bounded `prompt` ever reaches main.
    expect(decodeAppleFmStartTurnRequest({ prompt: "hi", fileContents: "secret" })).toEqual({ prompt: "hi" })
  })

  test("turn result round-trips a bounded completion", () => {
    const turn = {
      schema: "openagents.desktop.apple_fm.turn.v1",
      ok: true,
      outcome: "completed",
      text: "hello",
      usageTruth: "estimated",
      promptTokens: 3,
      completionTokens: 2,
      totalTokens: 5,
      failureClass: null,
    }
    expect(decodeAppleFmTurnResult(turn)).toEqual(turn)
    expect(decodeAppleFmTurnResult({ ...turn, text: "x".repeat(8193) })).toBeNull()
  })
})
