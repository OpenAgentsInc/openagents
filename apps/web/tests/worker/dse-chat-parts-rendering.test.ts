import { describe, expect, it } from "vitest"

import { renderToString } from "@openagentsinc/effuse"

import { applyChatWirePart } from "../../src/effect/chatWire"
import type { ActiveStream } from "../../src/effect/chatWire"
import { extractL402PaymentMetadata, toAutopilotRenderParts } from "../../src/effuse-app/controllers/autopilotChatParts"
import { autopilotChatTemplate } from "../../src/effuse-pages/autopilot"
import type { AutopilotChatData } from "../../src/effuse-pages/autopilot"
import { autopilotGmailReviewStreamV1, dseKitchenSinkStreamV1 } from "../../src/fixtures/wireTranscripts"

describe("apps/web autopilot chat: DSE wire parts render (Stage 2.5)", () => {
  it("consumes a realistic tool-loop transcript and surfaces DSE signature/tool parts in the UI", () => {
    const active: ActiveStream = { id: "run-1", messageId: "m-assistant", parts: [] }
    for (const ev of autopilotGmailReviewStreamV1) applyChatWirePart(active, ev.part)

    const renderParts = toAutopilotRenderParts({ parts: active.parts })
    expect(renderParts.length).toBeGreaterThan(0)

    const sigs = renderParts.filter((p) => p.kind === "dse-signature")
    expect(sigs).toHaveLength(2)
    expect(sigs[0]?.model.id).toBe("dsepart_sig_1")
    expect(sigs[0]?.model.state).toBe("ok")
    expect(sigs[0]?.model.signatureId).toBe("@openagents/autopilot/blueprint/SelectTool.v1")
    expect(sigs[0]?.model.receiptId).toBe("rcpt_select_tool_1")
    expect(sigs[0]?.model.outputPreview?.preview ?? "").toContain("gmail.connect")

    const tools = renderParts.filter((p) => p.kind === "tool")
    expect(tools.length).toBeGreaterThan(0)

    const data: AutopilotChatData = {
      messages: [
        { id: "m-user", role: "user", renderParts: [{ kind: "text", text: "review my recent gmail things" }] },
        { id: "m-assistant", role: "assistant", renderParts },
      ],
      isBusy: false,
      isAtBottom: true,
      inputValue: "",
      errorText: null,
      auth: {
        isAuthed: true,
        authedEmail: "you@example.com",
        step: "closed",
        email: "",
        code: "",
        isBusy: false,
        errorText: null,
      },
    }

    const html = renderToString(autopilotChatTemplate(data))
    expect(html).toContain('data-dse-card-title="DSE Signature"')
    expect(html).toContain('data-effuse-tool-name="gmail.connect"')
  })

  it("renders the full DSE kitchen sink card set (signature/tool/compile/promote/rollback/budget)", () => {
    const active: ActiveStream = { id: "run-ks", messageId: "m-assistant", parts: [] }
    for (const ev of dseKitchenSinkStreamV1) applyChatWirePart(active, ev.part)

    const renderParts = toAutopilotRenderParts({ parts: active.parts })
    expect(renderParts.some((p) => p.kind === "dse-signature")).toBe(true)
    expect(renderParts.some((p) => p.kind === "dse-compile")).toBe(true)
    expect(renderParts.some((p) => p.kind === "dse-promote")).toBe(true)
    expect(renderParts.some((p) => p.kind === "dse-rollback")).toBe(true)
    expect(renderParts.some((p) => p.kind === "dse-budget-exceeded")).toBe(true)

    const data: AutopilotChatData = {
      messages: [{ id: "m-assistant", role: "assistant", renderParts }],
      isBusy: false,
      isAtBottom: true,
      inputValue: "",
      errorText: null,
      auth: {
        isAuthed: true,
        authedEmail: "you@example.com",
        step: "closed",
        email: "",
        code: "",
        isBusy: false,
        errorText: null,
      },
    }

    const html = renderToString(autopilotChatTemplate(data))
    expect(html).toContain('data-dse-card-title="DSE Signature"')
    expect(html).toContain('data-dse-card-title="DSE Compile"')
    expect(html).toContain('data-dse-card-title="DSE Promote"')
    expect(html).toContain('data-dse-card-title="DSE Rollback"')
    expect(html).toContain('data-dse-card-title="DSE Budget Stop"')
  })

  it("extracts deterministic L402 payment metadata from lightning tool outputs", () => {
    const metadata = extractL402PaymentMetadata([
      {
        type: "dse.tool",
        v: 1,
        id: "tool-1",
        state: "ok",
        toolName: "lightning_l402_fetch",
        toolCallId: "call-1",
        input: {
          url: "https://api.example.com/paid",
          method: "GET",
          scope: "episode-212",
          maxSpendMsats: 1_200,
        },
        output: {
          taskId: "task-1",
          status: "completed",
          proofReference: "preimage:abc123",
          paymentId: "payment-1",
          amountMsats: 900,
          responseStatusCode: 200,
        },
      },
      {
        type: "dynamic-tool",
        toolName: "lightning_l402_fetch",
        toolCallId: "call-2",
        state: "output-available",
        input: {
          url: "https://api.example.com/blocked",
          maxSpendMsats: 500,
        },
        output: {
          taskId: "task-2",
          status: "blocked",
          denyReason: "policy_denied",
        },
      },
      {
        type: "dse.tool",
        v: 1,
        id: "tool-ignored",
        state: "ok",
        toolName: "gmail.connect",
        toolCallId: "call-ignored",
        input: {},
        output: {},
      },
    ])

    expect(metadata).toEqual([
      {
        toolName: "lightning_l402_fetch",
        toolCallId: "call-1",
        status: "completed",
        taskId: "task-1",
        paymentId: "payment-1",
        amountMsats: 900,
        responseStatusCode: 200,
        proofReference: "preimage:abc123",
        denyReason: undefined,
        url: "https://api.example.com/paid",
        method: "GET",
        scope: "episode-212",
        maxSpendMsats: 1_200,
      },
      {
        toolName: "lightning_l402_fetch",
        toolCallId: "call-2",
        status: "blocked",
        taskId: "task-2",
        paymentId: undefined,
        amountMsats: 500,
        responseStatusCode: undefined,
        proofReference: undefined,
        denyReason: "policy_denied",
        url: "https://api.example.com/blocked",
        method: undefined,
        scope: undefined,
        maxSpendMsats: 500,
      },
    ])
  })
})
