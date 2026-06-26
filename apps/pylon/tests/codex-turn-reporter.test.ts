import { describe, expect, test } from "bun:test"

import {
  PYLON_CODEX_TURN_INGEST_PATH,
  PYLON_CODEX_TURN_SCHEMA_VERSION,
  createPylonCodexTurnReporter,
} from "../src/codex-turn-reporter"

describe("Pylon Codex turn reporter", () => {
  test("posts the exact turn envelope with bearer auth and stable idempotency", async () => {
    const calls: Array<{ init?: RequestInit; url: string }> = []
    const fetchImpl = async (
      input: RequestInfo | URL,
      init?: RequestInit,
    ): Promise<Response> => {
      calls.push({ init, url: String(input) })
      return new Response(JSON.stringify({ ok: true }), { status: 200 })
    }
    const reporter = createPylonCodexTurnReporter({
      agentToken: "oa_agent_test_token",
      baseUrl: "https://openagents.com/",
      fetch: fetchImpl as typeof fetch,
    })

    await reporter?.({
      assignmentRef: "assignment.public.codex-report",
      leaseRef: "lease.public.codex-report",
      pylonRef: "pylon.public.codex-report",
      runRef: "run.public.codex-report",
      sessionRef: "session.public.codex-report",
      workspaceRef: "workspace.public.codex-report",
      turnIndex: 0,
      observedAt: "2026-06-26T12:00:00.000Z",
      usage: {
        cachedInputTokens: 3.8,
        inputTokens: 100.9,
        outputTokens: -4,
        reasoningOutputTokens: 7.2,
      },
      items: [
        {
          itemType: "agent_message",
          message: "Completed safely.",
          ordinal: 1,
          status: "completed",
        },
        {
          commandLabel: "shell_command",
          exitCode: 0,
          itemType: "command_execution",
          ordinal: 2,
          outputBytes: 123.9,
          status: "completed",
        },
      ],
    })

    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe(
      `https://openagents.com${PYLON_CODEX_TURN_INGEST_PATH}`,
    )
    expect(calls[0]?.init?.method).toBe("POST")
    expect(calls[0]?.init?.headers).toMatchObject({
      "Idempotency-Key":
        "pylon.codex.turn.pylon.public.codex-report.assignment.public.codex-report.session.public.codex-report.1",
      authorization: "Bearer oa_agent_test_token",
      "content-type": "application/json",
    })
    const body = JSON.parse(String(calls[0]?.init?.body)) as Record<
      string,
      unknown
    >

    expect(body).toMatchObject({
      assignmentRef: "assignment.public.codex-report",
      leaseRef: "lease.public.codex-report",
      pylonRef: "pylon.public.codex-report",
      runRef: "run.public.codex-report",
      schemaVersion: PYLON_CODEX_TURN_SCHEMA_VERSION,
      sessionRef: "session.public.codex-report",
      turnIndex: 1,
      workspaceRef: "workspace.public.codex-report",
    })
    expect(body.usage).toEqual({
      cachedInputTokens: 3,
      inputTokens: 100,
      outputTokens: 0,
      reasoningOutputTokens: 7,
    })
    expect(body.items).toMatchObject([
      {
        itemType: "agent_message",
        message: "Completed safely.",
        ordinal: 1,
      },
      {
        commandLabel: "shell_command",
        exitCode: 0,
        itemType: "command_execution",
        ordinal: 2,
        outputBytes: 123,
      },
    ])
    expect(JSON.stringify(body)).not.toContain("raw shell output")
  })

  test("is disabled until both the base URL and agent token are present", () => {
    expect(
      createPylonCodexTurnReporter({
        agentToken: "oa_agent_test_token",
      }),
    ).toBeUndefined()
    expect(
      createPylonCodexTurnReporter({
        baseUrl: "https://openagents.com",
      }),
    ).toBeUndefined()
  })

  test("surfaces non-2xx ingest responses to the SDK fail-soft caller", async () => {
    const reporter = createPylonCodexTurnReporter({
      agentToken: "oa_agent_test_token",
      baseUrl: "https://openagents.com",
      fetch: (async () =>
        new Response(JSON.stringify({ error: "temporarily_unavailable" }), {
          status: 503,
        })) as typeof fetch,
    })

    await expect(
      reporter?.({
        assignmentRef: "assignment.public.codex-report",
        leaseRef: "lease.public.codex-report",
        pylonRef: "pylon.public.codex-report",
        turnIndex: 1,
        usage: {
          inputTokens: 1,
          outputTokens: 1,
        },
        items: [],
      }),
    ).rejects.toThrow("Pylon Codex turn ingest failed (503)")
  })
})
