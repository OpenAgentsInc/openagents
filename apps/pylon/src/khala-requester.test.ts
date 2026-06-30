import { describe, expect, test } from "bun:test"

import {
  buildPylonKhalaChatRequestBody,
  issuePylonKhalaRequest,
  resumePylonKhalaRequest,
} from "./khala-requester.js"

describe("Pylon Khala requester", () => {
  test("carries target account hash in both root and nested coding fields", () => {
    const body = buildPylonKhalaChatRequestBody({
      prompt: "Run the public-safe fixture.",
      targetAccountRefHash: "account.pylon.codex.651c03fed68925d7acb2c02f",
      targetPylonRef: "pylon.33afd48282a649047e3a",
      workflow: "codex_agent_task",
    }) as {
      openagents?: { coding?: Record<string, unknown> }
      targetAccountRefHash?: string
      targetPylonRef?: string
    }

    expect(body.targetPylonRef).toBe("pylon.33afd48282a649047e3a")
    expect(body.targetAccountRefHash).toBe("account.pylon.codex.651c03fed68925d7acb2c02f")
    expect(body.openagents?.coding?.targetPylonRef).toBe("pylon.33afd48282a649047e3a")
    expect(body.openagents?.coding?.targetAccountRefHash).toBe("account.pylon.codex.651c03fed68925d7acb2c02f")
  })

  test("surfaces public-safe dispatch gate evidence on failed coding requests", async () => {
    const fetchMock = (async () =>
      new Response(
        JSON.stringify({
          dispatchGate: {
            blockerRefs: [
              "blocker.public.pylon_dispatch.duplicate_active_assignment",
            ],
          },
          error: "target_pylon_unavailable",
          evidenceRefs: [
            "evidence.khala_coding.target_pylon_ref.dispatch_gate_blocked",
          ],
          reason:
            "The requested linked Pylon is available but the controlled assignment dispatch gate refused the coding lease.",
          requestedPylonRef: "pylon.33afd48282a649047e3a",
        }),
        { status: 409 },
      )) as unknown as typeof fetch

    const result = issuePylonKhalaRequest(
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.example",
        fetch: fetchMock,
      },
      {
        prompt: "Run the public-safe fixture.",
        targetPylonRef: "pylon.33afd48282a649047e3a",
        workflow: "codex_agent_task",
      },
    )

    await expect(result).rejects.toThrow(
      "requestedPylonRef=pylon.33afd48282a649047e3a",
    )
    await expect(result).rejects.toThrow(
      "evidence.khala_coding.target_pylon_ref.dispatch_gate_blocked",
    )
    await expect(result).rejects.toThrow(
      "blocker.public.pylon_dispatch.duplicate_active_assignment",
    )
  })

  test("parses CRLF SSE frames, public event text, and durable resume metadata", async () => {
    const fetchMock = (async () =>
      new Response(
        [
          ": keepalive\r\n",
          "event: delta\r\n",
          'data: {"text":"Hel"}\r\n',
          "\r\n",
          'data: {"choices":[{"delta":{"content":"lo"}}]}\r\n',
          "\r\n",
          "data: [DONE]\r\n",
          "\r\n",
        ].join(""),
        {
          headers: {
            "openagents-durable-stream-url": "/v1/chat/completions/durable/request.public.123",
            "stream-closed": "true",
            "stream-next-offset": "128",
            "stream-up-to-date": "true",
          },
        },
      )) as unknown as typeof fetch

    const result = await issuePylonKhalaRequest(
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.example",
        fetch: fetchMock,
      },
      {
        prompt: "Run the public-safe fixture.",
        workflow: "codex_agent_task",
      },
    )

    expect(result.diagnostics).toEqual([])
    expect(result.durableRequestId).toBe("request.public.123")
    expect(result.frames.map((frame) => frame.event ?? null)).toEqual(["delta", null, null])
    expect(result.nextOffset).toBe("128")
    expect(result.streamClosed).toBe(true)
    expect(result.streamUpToDate).toBe(true)
    expect(result.text).toBe("Hello")
  })

  test("records malformed SSE JSON as a typed diagnostic instead of silently dropping stream evidence", async () => {
    const fetchMock = (async () =>
      new Response(
        [
          "event: delta\n",
          'data: {"text":"ok"}\n',
          "\n",
          "event: delta\n",
          "data: {not-json}\n",
          "\n",
        ].join(""),
        {
          headers: {
            "stream-up-to-date": "true",
          },
        },
      )) as unknown as typeof fetch

    const result = await resumePylonKhalaRequest(
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.example",
        fetch: fetchMock,
      },
      {
        durableRequestId: "request.public.123",
        offset: 0,
      },
    )

    expect(result.text).toBe("ok")
    expect(result.diagnostics).toHaveLength(1)
    expect(result.diagnostics[0]).toMatchObject({
      code: "malformed_sse_json",
      event: "delta",
      frameIndex: 1,
    })
    expect(result.frames[1]).toMatchObject({
      data: "{not-json}",
      event: "delta",
      parsed: null,
    })
  })
})
