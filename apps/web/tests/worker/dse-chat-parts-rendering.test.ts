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
        denyReasonCode: undefined,
        host: undefined,
        quotedAmountMsats: undefined,
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
        denyReasonCode: undefined,
        host: undefined,
        quotedAmountMsats: undefined,
        url: "https://api.example.com/blocked",
        method: undefined,
        scope: undefined,
        maxSpendMsats: 500,
      },
    ])
  })

  it("renders chat-visible L402 payment lifecycle states with proof and deny reason", () => {
    const renderParts = toAutopilotRenderParts({
      parts: [
        {
          type: "dse.tool",
          v: 1,
          id: "tool-intent",
          state: "start",
          toolName: "lightning_l402_fetch",
          toolCallId: "call-intent",
          input: {
            url: "https://api.example.com/intent",
            method: "GET",
            maxSpendMsats: 1_000,
          },
        },
        {
          type: "dse.tool",
          v: 1,
          id: "tool-sent",
          state: "ok",
          toolName: "lightning_l402_fetch",
          toolCallId: "call-sent",
          input: {
            url: "https://api.example.com/sent",
            method: "GET",
            maxSpendMsats: 1_200,
          },
          output: {
            taskId: "task-sent",
            status: "completed",
            proofReference: "preimage:sent123",
            paymentId: "payment-sent",
            amountMsats: 850,
            responseStatusCode: 200,
          },
        },
        {
          type: "dse.tool",
          v: 1,
          id: "tool-cached",
          state: "ok",
          toolName: "lightning_l402_fetch",
          toolCallId: "call-cached",
          input: {
            url: "https://api.example.com/cached",
            method: "GET",
            maxSpendMsats: 1_000,
          },
          output: {
            taskId: "task-cached",
            status: "cached",
            proofReference: "preimage:cache123",
            responseStatusCode: 200,
          },
        },
        {
          type: "dse.tool",
          v: 1,
          id: "tool-blocked",
          state: "error",
          toolName: "lightning_l402_fetch",
          toolCallId: "call-blocked",
          input: {
            url: "https://api.example.com/blocked",
            method: "GET",
            maxSpendMsats: 10_000,
          },
          output: {
            taskId: "task-blocked",
            status: "blocked",
            denyReason: "policy_denied",
          },
          errorText: "policy_denied",
        },
        {
          type: "dse.tool",
          v: 1,
          id: "tool-failed",
          state: "error",
          toolName: "lightning_l402_fetch",
          toolCallId: "call-failed",
          input: {
            url: "https://api.example.com/failed",
            method: "GET",
            maxSpendMsats: 2_000,
          },
          output: {
            taskId: "task-failed",
            status: "failed",
            denyReason: "desktop_executor_timeout",
          },
          errorText: "desktop_executor_timeout",
        },
      ],
    })

    const paymentStates = renderParts
      .filter((part): part is Extract<(typeof renderParts)[number], { kind: "payment-state" }> => part.kind === "payment-state")
      .map((part) => part.model.state)
    expect(paymentStates).toEqual([
      "payment.intent",
      "payment.sent",
      "payment.cached",
      "payment.blocked",
      "payment.failed",
    ])

    const data: AutopilotChatData = {
      messages: [{ id: "m-payment", role: "assistant", renderParts }],
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
    expect(html).toContain('data-payment-state="payment.intent"')
    expect(html).toContain('data-payment-state="payment.sent"')
    expect(html).toContain('data-payment-state="payment.cached"')
    expect(html).toContain('data-payment-state="payment.blocked"')
    expect(html).toContain('data-payment-state="payment.failed"')
    expect(html).toContain("preimage:sent123")
    expect(html).toContain("policy_denied")
  })

  it("formats amount_over_cap L402 policy denials into a stable blocked sentence", () => {
    const renderParts = toAutopilotRenderParts({
      parts: [
        {
          type: "dse.tool",
          v: 1,
          id: "tool-overcap",
          state: "error",
          toolName: "lightning_l402_fetch",
          toolCallId: "call-overcap",
          input: {
            url: "https://api.example.com/premium",
            method: "GET",
            maxSpendMsats: 100_000,
          },
          output: {
            taskId: "task-overcap",
            status: "blocked",
            denyReason: "Quoted invoice amount exceeds configured spend cap",
            denyReasonCode: "amount_over_cap",
            host: "api.example.com",
            maxSpendMsats: 100_000,
            quotedAmountMsats: 250_000,
          },
          errorText: "policy_denied",
        },
      ],
    })

    const html = renderToString(
      autopilotChatTemplate({
        messages: [{ id: "m-overcap", role: "assistant", renderParts }],
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
      }),
    )

    expect(html).toContain('data-payment-state="payment.blocked"')
    expect(html).toContain("Blocked: quoted 250 sats &gt; cap 100 sats")
  })
})
