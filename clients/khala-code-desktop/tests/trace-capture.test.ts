import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import type {
  KhalaPrivacyRedactionResult,
  KhalaPrivacyRedactionServiceShape,
} from "@openagentsinc/khala-tools"

import {
  KHALA_CODE_DESKTOP_TRACE_CAPTURE_INGEST_AUDIENCE,
  captureKhalaCodeDesktopTraceEvent,
} from "../src/shared/trace-capture"

const event = {
  eventId: "event.trace.1",
  observedAt: "2026-07-04T00:00:00.000Z",
  role: "user" as const,
  sessionId: "session.trace.1",
  text: "Ask Alex Rivera to review the diff.",
}

const redactionResult = (text: string): KhalaPrivacyRedactionResult => ({
  engine: "@nationaldesignstudio/rampart",
  mode: "rampart_model",
  placeholders: ["[GIVEN_NAME_1]", "[SURNAME_1]"],
  redacted: true,
  redactionRefs: ["redaction.khala.rampart.pii"],
  text: text.replace("Alex Rivera", "[GIVEN_NAME_1] [SURNAME_1]"),
})

const redaction = (input: {
  readonly fail?: boolean
  readonly calls?: string[]
} = {}): KhalaPrivacyRedactionServiceShape => ({
  protectModelText: text => {
    input.calls?.push(`model:${text}`)
    return input.fail === true ? Effect.die("redaction failed") : Effect.succeed(redactionResult(text))
  },
  protectUserText: text => {
    input.calls?.push(`user:${text}`)
    return input.fail === true ? Effect.die("redaction failed") : Effect.succeed(redactionResult(text))
  },
  revealForLocalUser: text => Effect.succeed(text),
  revealTransform: () => Effect.succeed(new TransformStream<string, string>()),
})

// Oracle for khala_code.plans.free_trace_capture_explicit_consent.v1
describe("khala code desktop trace capture planner", () => {
  test("defaults to not captured when consent is off", async () => {
    const calls: string[] = []
    let ingestCalls = 0
    const result = await captureKhalaCodeDesktopTraceEvent({
      consentEnabled: false,
      event,
      ownerArmed: true,
      ownerOnlyIngest: async () => {
        ingestCalls += 1
        return { receiptRef: "receipt.trace.should_not_exist" }
      },
      planCaptureExcluded: false,
      planKind: "free",
      redaction: redaction({ calls }),
    })

    expect(result).toMatchObject({
      captured: false,
      reason: "consent_disabled",
      state: "not_captured",
    })
    expect(result.marker).toEqual({
      payoutEligible: false,
      revenueShareEligible: false,
      settlementEligible: false,
    })
    expect(calls).toEqual([])
    expect(ingestCalls).toBe(0)
  })

  test("paid-plan capture opt-out blocks capture even with consent and owner arming", async () => {
    const calls: string[] = []
    const result = await captureKhalaCodeDesktopTraceEvent({
      consentEnabled: true,
      event,
      ownerArmed: true,
      ownerOnlyIngest: async () => ({ receiptRef: "receipt.trace.should_not_exist" }),
      planCaptureExcluded: true,
      planKind: "paid",
      redaction: redaction({ calls }),
    })

    expect(result).toMatchObject({
      captured: false,
      reason: "paid_plan_capture_excluded",
      state: "not_captured",
    })
    expect(calls).toEqual([])
  })

  test("redaction failure fails closed before owner-only ingest", async () => {
    let ingestCalls = 0
    const result = await captureKhalaCodeDesktopTraceEvent({
      consentEnabled: true,
      event,
      ownerArmed: true,
      ownerOnlyIngest: async () => {
        ingestCalls += 1
        return { receiptRef: "receipt.trace.should_not_exist" }
      },
      planCaptureExcluded: false,
      planKind: "free",
      redaction: redaction({ fail: true }),
    })

    expect(result).toMatchObject({
      captured: false,
      reason: "redaction_failed",
      state: "not_captured",
    })
    expect(ingestCalls).toBe(0)
  })

  test("owner-armed free-plan capture redacts before owner-only ingest and stays payout-inert", async () => {
    const ingested: unknown[] = []
    const result = await captureKhalaCodeDesktopTraceEvent({
      consentEnabled: true,
      event,
      ownerArmed: true,
      ownerOnlyIngest: async record => {
        ingested.push(record)
        return { receiptRef: "receipt.owner_only_trace.redacted.1" }
      },
      planCaptureExcluded: false,
      planKind: "free",
      redaction: redaction(),
    })

    expect(result).toMatchObject({
      captured: true,
      ingestAudience: KHALA_CODE_DESKTOP_TRACE_CAPTURE_INGEST_AUDIENCE,
      ownerOnlyReceiptRef: "receipt.owner_only_trace.redacted.1",
      state: "captured",
    })
    expect(result.marker).toEqual({
      payoutEligible: false,
      revenueShareEligible: false,
      settlementEligible: false,
    })
    expect(ingested).toHaveLength(1)
    expect(JSON.stringify(ingested[0])).not.toContain("Alex Rivera")
    expect(JSON.stringify(ingested[0])).toContain("[GIVEN_NAME_1] [SURNAME_1]")
  })
})
