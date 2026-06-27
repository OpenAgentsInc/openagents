import { afterEach, describe, expect, test } from "bun:test"

import {
  handlePylonKhalaMcpRequest,
  pylonKhalaMcpConfig,
} from "../src/khala-mcp"
import {
  buildToolCallRequest,
  buildToolsListRequest,
  parseToolResult,
  type McpToolCallResult,
} from "../src/tas/mcp-client"

const sse = (id: string, content: string) =>
  `data: ${JSON.stringify({
    choices: [{ delta: { content }, index: 0 }],
    id,
    object: "chat.completion.chunk",
  })}\n\ndata: [DONE]\n\n`

const servers: ReturnType<typeof Bun.serve>[] = []

afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true)
})

const callMcp = async (
  request: Parameters<typeof handlePylonKhalaMcpRequest>[0],
  baseUrl = "https://openagents.test",
) =>
  handlePylonKhalaMcpRequest(request, {
    network: {
      agentToken: "oa_agent_mcp_test",
      baseUrl,
    },
  })

const parseJsonContent = (result: McpToolCallResult): Record<string, unknown> => {
  const text = result.content[0]?.text
  expect(text).toBeDefined()
  return JSON.parse(text ?? "{}") as Record<string, unknown>
}

describe("pylon khala MCP stdio handler", () => {
  test("tools/list exposes Khala tools with authority annotations", async () => {
    const response = await callMcp(buildToolsListRequest())

    expect(response.error).toBeUndefined()
    const tools = response.result?.tools as Array<{
      annotations?: Record<string, unknown>
      name: string
    }>
    expect(tools.map((tool) => tool.name).sort()).toEqual([
      "khala.capacity",
      "khala.request",
      "khala.resume",
      "khala.spawn",
      "khala.spawnStatus",
      "khala.status",
    ])
    expect(
      tools.find((tool) => tool.name === "khala.request")?.annotations,
    ).toMatchObject({
      handlerKind: "coding_session_control",
      readOnlyHint: false,
    })
    expect(
      tools.find((tool) => tool.name === "khala.resume")?.annotations,
    ).toMatchObject({
      handlerKind: "private_account_read",
      readOnlyHint: true,
    })
    expect(
      tools.find((tool) => tool.name === "khala.spawn")?.annotations,
    ).toMatchObject({
      handlerKind: "coding_session_control",
      readOnlyHint: false,
    })
  })

  test("khala.request drives the OpenAI-compatible request path and returns a durable handle", async () => {
    const requests: Array<{ body: Record<string, unknown>; headers: Headers; path: string }> =
      []
    const server = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url)
        requests.push({
          body: JSON.parse(await request.text()) as Record<string, unknown>,
          headers: request.headers,
          path: url.pathname,
        })
        return new Response(sse("chatcmpl_mcp", "mcp delegated"), {
          headers: {
            "openagents-coding-assignment-ref": "assignment.public.mcp",
            "openagents-durable-stream-url":
              "/v1/chat/completions/durable/chatcmpl_mcp",
          },
        })
      },
    })
    servers.push(server)

    const response = await callMcp(
      buildToolCallRequest("khala.request", {
        prompt: "Run the MCP fixture task",
        targetPylonRef: "pylon.owner.codex",
        workflow: "codex_agent_task",
      }),
      `http://127.0.0.1:${server.port}`,
    )

    const parsed = parseToolResult(response as { jsonrpc: "2.0"; id: string; result?: McpToolCallResult })
    expect(parsed.ok).toBe(true)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.path).toBe("/v1/chat/completions")
    expect(requests[0]?.headers.get("authorization")).toBe(
      "Bearer oa_agent_mcp_test",
    )
    expect(requests[0]?.body).toMatchObject({
      model: "openagents/khala",
      openagents: {
        coding: { targetPylonRef: "pylon.owner.codex" },
        workflowClass: "codex_agent_task",
      },
      stream: true,
    })
    const body = parseJsonContent(response.result as McpToolCallResult)
    expect(body).toMatchObject({
      assignmentRef: "assignment.public.mcp",
      durableRequestId: "chatcmpl_mcp",
      durableStreamUrl: "/v1/chat/completions/durable/chatcmpl_mcp",
      ok: true,
      schema: "openagents.pylon.khala_request.v1",
      text: "mcp delegated",
      workflow: "codex_agent_task",
    })
  })

  test("khala.resume proxies through the remote MCP surface so ownership is checked server-side", async () => {
    const requests: Array<{ body: Record<string, unknown>; headers: Headers; path: string }> =
      []
    const server = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url)
        requests.push({
          body: JSON.parse(await request.text()) as Record<string, unknown>,
          headers: request.headers,
          path: url.pathname,
        })
        return Response.json({
          id: "pylon.local.khala.resume",
          jsonrpc: "2.0",
          result: {
            content: [
              {
                text: JSON.stringify({
                  durableRequestId: "chatcmpl_mcp",
                  ok: true,
                  schema: "openagents.khala_mcp.durable_read.v1",
                  streamClosed: true,
                }),
                type: "text",
              },
            ],
          },
        })
      },
    })
    servers.push(server)

    const response = await callMcp(
      buildToolCallRequest("khala.resume", {
        durableRequestId: "chatcmpl_mcp",
        offset: 42,
      }),
      `http://127.0.0.1:${server.port}`,
    )

    const body = parseJsonContent(response.result as McpToolCallResult)
    expect(body).toMatchObject({
      durableRequestId: "chatcmpl_mcp",
      ok: true,
      streamClosed: true,
    })
    expect(requests).toHaveLength(1)
    expect(requests[0]?.path).toBe("/api/mcp")
    expect(requests[0]?.headers.get("authorization")).toBe(
      "Bearer oa_agent_mcp_test",
    )
    expect(requests[0]?.body).toMatchObject({
      method: "tools/call",
      params: {
        arguments: {
          durableRequestId: "chatcmpl_mcp",
          offset: 42,
        },
        name: "khala.resume",
      },
    })
  })

  test("khala.resume preserves remote MCP ownership denial as an isError result", async () => {
    const server = Bun.serve({
      port: 0,
      fetch() {
        return Response.json({
          id: "pylon.local.khala.resume",
          jsonrpc: "2.0",
          result: {
            content: [
              {
                text: "durable_request_not_authorized",
                type: "text",
              },
            ],
            isError: true,
          },
        })
      },
    })
    servers.push(server)

    const response = await callMcp(
      buildToolCallRequest("khala.resume", {
        durableRequestId: "chatcmpl_mcp",
      }),
      `http://127.0.0.1:${server.port}`,
    )

    const parsed = parseToolResult(
      response as { jsonrpc: "2.0"; id: string; result?: McpToolCallResult },
    )
    expect(parsed).toEqual({
      error: "durable_request_not_authorized",
      ok: false,
    })
  })

  test("khala.spawn proxies through the remote MCP surface", async () => {
    const requests: Array<{ body: Record<string, unknown>; headers: Headers; path: string }> =
      []
    const server = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url)
        requests.push({
          body: JSON.parse(await request.text()) as Record<string, unknown>,
          headers: request.headers,
          path: url.pathname,
        })
        return Response.json({
          id: "pylon.local.khala.spawn",
          jsonrpc: "2.0",
          result: {
            content: [
              {
                text: JSON.stringify({
                  assignedCount: 2,
                  children: [
                    {
                      assignmentRef: "assignment.public.khala_coding.one",
                      durableRequestId: "chatcmpl_one",
                      ok: true,
                    },
                    {
                      assignmentRef: "assignment.public.khala_coding.two",
                      durableRequestId: "chatcmpl_two",
                      ok: true,
                    },
                  ],
                  ok: true,
                  schema: "openagents.khala_mcp.spawn.v1",
                  spawnRef: "spawn.public.khala_coding.mcp",
                }),
                type: "text",
              },
            ],
          },
        })
      },
    })
    servers.push(server)

    const response = await callMcp(
      buildToolCallRequest("khala.spawn", {
        count: 2,
        objective: "Run two MCP child workers",
        targetPylonRef: "pylon.owner.codex",
      }),
      `http://127.0.0.1:${server.port}`,
    )

    const body = parseJsonContent(response.result as McpToolCallResult)
    expect(body).toMatchObject({
      assignedCount: 2,
      ok: true,
      spawnRef: "spawn.public.khala_coding.mcp",
    })
    expect(requests).toHaveLength(1)
    expect(requests[0]?.path).toBe("/api/mcp")
    expect(requests[0]?.headers.get("authorization")).toBe(
      "Bearer oa_agent_mcp_test",
    )
    expect(requests[0]?.body).toMatchObject({
      method: "tools/call",
      params: {
        arguments: {
          count: 2,
          objective: "Run two MCP child workers",
          targetPylonRef: "pylon.owner.codex",
        },
        name: "khala.spawn",
      },
    })
  })

  test("khala.spawnStatus proxies parent status reads through the remote MCP surface", async () => {
    const requests: Array<{ body: Record<string, unknown>; path: string }> = []
    const server = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url)
        requests.push({
          body: JSON.parse(await request.text()) as Record<string, unknown>,
          path: url.pathname,
        })
        return Response.json({
          id: "pylon.local.khala.spawnStatus",
          jsonrpc: "2.0",
          result: {
            content: [
              {
                text: JSON.stringify({
                  childCount: 2,
                  children: [
                    { durableRequestId: "chatcmpl_one", state: "offered" },
                    { durableRequestId: "chatcmpl_two", state: "offered" },
                  ],
                  ok: true,
                  schema: "openagents.khala_mcp.spawn_status.v1",
                  spawnRef: "spawn.public.khala_coding.mcp",
                }),
                type: "text",
              },
            ],
          },
        })
      },
    })
    servers.push(server)

    const response = await callMcp(
      buildToolCallRequest("khala.spawnStatus", {
        spawnRef: "spawn.public.khala_coding.mcp",
      }),
      `http://127.0.0.1:${server.port}`,
    )

    const body = parseJsonContent(response.result as McpToolCallResult)
    expect(body).toMatchObject({
      childCount: 2,
      ok: true,
      spawnRef: "spawn.public.khala_coding.mcp",
    })
    expect(requests[0]?.body).toMatchObject({
      method: "tools/call",
      params: {
        arguments: {
          spawnRef: "spawn.public.khala_coding.mcp",
        },
        name: "khala.spawnStatus",
      },
    })
  })

  test("tool failures are returned as MCP isError results", async () => {
    const response = await callMcp(
      buildToolCallRequest("khala.request", {
        workflow: "codex_agent_task",
      }),
    )

    const parsed = parseToolResult(
      response as { jsonrpc: "2.0"; id: string; result?: McpToolCallResult },
    )
    expect(parsed).toEqual({
      error: "khala.request requires prompt or objective",
      ok: false,
    })
  })

  test("capacity reports the local auth/config projection without printing the token", async () => {
    const response = await callMcp(buildToolCallRequest("khala.capacity", {}))
    const body = parseJsonContent(response.result as McpToolCallResult)

    expect(body).toMatchObject({
      baseUrl: "https://openagents.test",
      schema: "openagents.pylon.khala_mcp_capacity.v1",
      tokenConfigured: true,
    })
    expect(JSON.stringify(body)).not.toContain("oa_agent_mcp_test")
  })

  test("config emits local stdio and remote Streamable-HTTP MCP entries", () => {
    const config = pylonKhalaMcpConfig({
      baseUrl: "https://openagents.test/",
      command: "/usr/local/bin/pylon",
    })

    expect(config).toMatchObject({
      mcpServers: {
        "openagents-khala-local": {
          args: ["mcp"],
          command: "/usr/local/bin/pylon",
          env: {
            OPENAGENTS_AGENT_TOKEN: "${OPENAGENTS_AGENT_TOKEN}",
            PYLON_OPENAGENTS_BASE_URL: "https://openagents.test/",
          },
        },
        "openagents-khala-remote": {
          headers: {
            Authorization: "Bearer ${OPENAGENTS_AGENT_TOKEN}",
          },
          type: "http",
          url: "https://openagents.test/api/mcp",
        },
      },
      schema: "openagents.pylon.mcp_config.v1",
    })
  })
})
