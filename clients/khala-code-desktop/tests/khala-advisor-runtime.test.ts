import { describe, expect, test } from "bun:test"

import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopUsage,
} from "../src/shared/rpc.js"
import {
  createKhalaAdvisorRuntime,
  KHALA_ADVISOR_ADVISORY_SCHEMA,
  khalaAdvisorSteeringText,
  type KhalaAdvisorAdvisory,
  type KhalaAdvisorReviewRequest,
  type KhalaAdvisorTokenUsageReport,
  type KhalaAdvisorTranscriptCard,
} from "../src/bun/khala-advisor-runtime.js"

const usage: KhalaCodeDesktopUsage = {
  cachedInput: 1,
  input: 10,
  output: 4,
  reasoningOutput: 2,
}

const advisory = (
  advisoryRef: string,
  severity: KhalaAdvisorAdvisory["severity"],
  summary: string,
  guidance = "Patch the affected path before continuing.",
): KhalaAdvisorAdvisory => ({
  schema: KHALA_ADVISOR_ADVISORY_SCHEMA,
  advisoryRef,
  generatedAt: "2026-07-02T12:00:00.000Z",
  severity,
  summary,
  guidance,
  evidenceRefs: ["turn.fixture"],
})

const eventStart = (
  turnId: string,
  id: string,
  body: string,
): KhalaCodeDesktopChatTurnEvent => ({
  message: { body, id, role: "assistant" },
  turnId,
  type: "message_start",
})

describe("KhalaAdvisorRuntime", () => {
  test("consumes turn deltas only and batches nit advisories into transcript cards", async () => {
    const reviewRequests: KhalaAdvisorReviewRequest[] = []
    const nitCards: KhalaAdvisorTranscriptCard[] = []

    const runtime = createKhalaAdvisorRuntime({
      modelSession: {
        reviewTurnDelta: async request => {
          reviewRequests.push(request)
          return {
            advisories: [
              advisory("advisor.nit.1", "nit", "Mention the failing assertion in the summary."),
            ],
            model: "claude-fixture-advisor",
            usage,
          }
        },
      },
      onNitBatch: card => nitCards.push(card),
      steerTurn: async request => ({
        desktopSessionId: request.sessionId,
        desktopTurnId: request.turnId,
        ok: true,
      }),
      tokenUsageReporter: async () => undefined,
    })

    runtime.acceptEvent(eventStart("turn-1", "assistant-1", "First chunk"))
    runtime.acceptEvent({
      delta: " plus streamed delta",
      messageId: "assistant-1",
      turnId: "turn-1",
      type: "message_delta",
    })

    const closeout = await runtime.flushTurn({
      desktopSessionId: "session-1",
      turnId: "turn-1",
    })

    expect(reviewRequests).toHaveLength(1)
    expect(reviewRequests[0]?.delta).toEqual({
      body: "assistant: First chunk plus streamed delta",
      desktopSessionId: "session-1",
      turnId: "turn-1",
    })
    expect(closeout.steered).toEqual([])
    expect(closeout.nitBatch?.advisories.map(item => item.advisoryRef)).toEqual(["advisor.nit.1"])
    expect(nitCards).toHaveLength(1)
    expect(nitCards[0]?.role).toBe("advisor")
  })

  test("routes concern and blocker advisories through codex steering with an immune-turn budget", async () => {
    const steeredTexts: string[] = []
    const runtime = createKhalaAdvisorRuntime({
      immuneTurns: 1,
      modelSession: {
        reviewTurnDelta: async () => ({
          advisories: [
            advisory("advisor.concern.1", "concern", "The migration drops existing rows."),
            advisory("advisor.blocker.1", "blocker", "The verifier command was weakened."),
          ],
          model: "claude-fixture-advisor",
          usage,
        }),
      },
      steerTurn: async request => {
        steeredTexts.push(request.text)
        return {
          desktopSessionId: request.sessionId,
          desktopTurnId: request.turnId,
          ok: true,
        }
      },
      tokenUsageReporter: async () => undefined,
    })

    runtime.acceptEvent(eventStart("turn-2", "assistant-2", "I changed the verifier."))
    const closeout = await runtime.flushTurn({
      desktopSessionId: "session-2",
      turnId: "turn-2",
    })

    expect(closeout.steered.map(item => item.clientUserMessageId)).toEqual(["advisor.concern.1"])
    expect(steeredTexts).toHaveLength(1)
    expect(steeredTexts[0]).toBe(khalaAdvisorSteeringText(
      advisory("advisor.concern.1", "concern", "The migration drops existing rows."),
    ))

    runtime.acceptEvent(eventStart("turn-3", "assistant-3", "Continuing after the first interrupt."))
    const secondCloseout = await runtime.flushTurn({
      desktopSessionId: "session-2",
      turnId: "turn-3",
    })
    expect(secondCloseout.steered).toEqual([])
  })

  test("enforces duplicate and content-free emission guards in code", async () => {
    const runtime = createKhalaAdvisorRuntime({
      modelSession: {
        reviewTurnDelta: async () => ({
          advisories: [
            advisory("advisor.concern.1", "concern", "The migration drops existing rows."),
            advisory("advisor.concern.dup", "concern", "The migration drops existing rows."),
            advisory("advisor.nit.empty", "nit", "Looks good", "ok"),
          ],
          model: "claude-fixture-advisor",
          usage,
        }),
      },
      steerTurn: async request => ({
        desktopSessionId: request.sessionId,
        desktopTurnId: request.turnId,
        ok: true,
      }),
      tokenUsageReporter: async () => undefined,
    })

    runtime.acceptEvent(eventStart("turn-4", "assistant-4", "Dropped rows."))
    const closeout = await runtime.flushTurn({
      desktopSessionId: "session-4",
      turnId: "turn-4",
    })

    expect(closeout.droppedAdvisories).toEqual([
      { advisoryRef: "advisor.concern.dup", reason: "duplicate" },
      { advisoryRef: "advisor.nit.empty", reason: "content_free" },
    ])
    expect(closeout.steered.map(item => item.clientUserMessageId)).toEqual(["advisor.concern.1"])
  })

  test("reset clears dedupe and restores interrupt budget after compaction or thread switch", async () => {
    const steered: string[] = []
    const runtime = createKhalaAdvisorRuntime({
      immuneTurns: 1,
      modelSession: {
        reviewTurnDelta: async () => ({
          advisories: [
            advisory("advisor.blocker.1", "blocker", "The verifier command was weakened."),
          ],
          model: "claude-fixture-advisor",
          usage,
        }),
      },
      steerTurn: async request => {
        steered.push(request.clientUserMessageId ?? "")
        return {
          desktopSessionId: request.sessionId,
          desktopTurnId: request.turnId,
          ok: true,
        }
      },
      tokenUsageReporter: async () => undefined,
    })

    runtime.acceptEvent(eventStart("turn-5", "assistant-5", "First blocker."))
    await runtime.flushTurn({ desktopSessionId: "session-5", turnId: "turn-5" })
    runtime.acceptEvent(eventStart("turn-6", "assistant-6", "Same blocker before reset."))
    const beforeReset = await runtime.flushTurn({ desktopSessionId: "session-5", turnId: "turn-6" })

    runtime.reset("compaction")
    runtime.acceptEvent(eventStart("turn-7", "assistant-7", "Same blocker after reset."))
    const afterReset = await runtime.flushTurn({ desktopSessionId: "session-5", turnId: "turn-7" })

    expect(beforeReset.droppedAdvisories).toEqual([
      { advisoryRef: "advisor.blocker.1", reason: "duplicate" },
    ])
    expect(afterReset.steered.map(item => item.clientUserMessageId)).toEqual(["advisor.blocker.1"])
    expect(steered).toEqual(["advisor.blocker.1", "advisor.blocker.1"])
  })

  test("reports advisor usage as separate exact role usage", async () => {
    const reports: KhalaAdvisorTokenUsageReport[] = []
    const runtime = createKhalaAdvisorRuntime({
      clock: { now: () => new Date("2026-07-02T12:34:56.000Z") },
      modelSession: {
        reviewTurnDelta: async () => ({
          advisories: [
            advisory("advisor.concern.usage", "concern", "The patch lacks a regression test."),
          ],
          model: "claude-fixture-advisor",
          usage,
        }),
      },
      steerTurn: async request => ({
        desktopSessionId: request.sessionId,
        desktopTurnId: request.turnId,
        ok: true,
      }),
      tokenUsageReporter: async report => {
        reports.push(report)
      },
    })

    runtime.acceptEvent(eventStart("turn-8", "assistant-8", "No test was added."))
    const closeout = await runtime.flushTurn({
      desktopSessionId: "session-8",
      turnId: "turn-8",
    })

    expect(closeout.advisorUsageReports).toBe(1)
    expect(reports).toEqual([{
      advisoryRef: "advisor.concern.usage",
      desktopSessionId: "session-8",
      model: "claude-fixture-advisor",
      observedAt: "2026-07-02T12:00:00.000Z",
      role: "advisor",
      turnId: "turn-8",
      usage,
    }])
  })
})
