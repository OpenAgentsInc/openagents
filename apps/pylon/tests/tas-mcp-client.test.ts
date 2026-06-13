import { describe, expect, test } from "bun:test"

import {
  buildToolCallRequest,
  buildToolsListRequest,
  isResponseForRequest,
  parseToolResult,
  type McpResponseEnvelope,
  type McpToolCallResult,
} from "../src/tas/mcp-client"

describe("tas mcp client protocol core", () => {
  test("builds well-formed tools/list request envelope", () => {
    expect(buildToolsListRequest()).toEqual({
      jsonrpc: "2.0",
      id: "tas.mcp.tools/list:{}",
      method: "tools/list",
      params: {},
    })
  })

  test("builds well-formed tools/call request envelope", () => {
    expect(
      buildToolCallRequest("repo.search", {
        limit: 10,
        filters: { path: "apps/pylon", hidden: false },
        query: "mcp client",
      }),
    ).toEqual({
      jsonrpc: "2.0",
      id:
        'tas.mcp.tools/call:{"arguments":{"filters":{"hidden":false,"path":"apps/pylon"},"limit":10,"query":"mcp client"},"name":"repo.search"}',
      method: "tools/call",
      params: {
        name: "repo.search",
        arguments: {
          limit: 10,
          filters: { path: "apps/pylon", hidden: false },
          query: "mcp client",
        },
      },
    })
  })

  test("parses successful tool result content", () => {
    const response: McpResponseEnvelope<McpToolCallResult> = {
      jsonrpc: "2.0",
      id: "tas.mcp.tools/call:{}",
      result: {
        content: [{ type: "text", text: "done" }],
      },
    }

    expect(parseToolResult(response)).toEqual({
      ok: true,
      content: [{ type: "text", text: "done" }],
    })
  })

  test("parses protocol and tool-level errors", () => {
    expect(
      parseToolResult({
        jsonrpc: "2.0",
        id: "tas.mcp.tools/call:{}",
        error: {
          code: -32602,
          message: "Invalid tool arguments",
        },
      }),
    ).toEqual({
      ok: false,
      error: "Invalid tool arguments",
    })

    expect(
      parseToolResult({
        jsonrpc: "2.0",
        id: "tas.mcp.tools/call:{}",
        result: {
          isError: true,
          content: [{ type: "text", text: "Tool rejected request" }],
        },
      }),
    ).toEqual({
      ok: false,
      error: "Tool rejected request",
    })
  })

  test("request ids are deterministic and correlate responses", () => {
    const first = buildToolCallRequest("repo.search", {
      query: "mcp client",
      filters: { hidden: false, path: "apps/pylon" },
    })
    const second = buildToolCallRequest("repo.search", {
      filters: { path: "apps/pylon", hidden: false },
      query: "mcp client",
    })
    const response: McpResponseEnvelope<McpToolCallResult> = {
      jsonrpc: "2.0",
      id: first.id,
      result: {
        content: [{ type: "text", text: "same call" }],
      },
    }

    expect(first.id).toBe(second.id)
    expect(isResponseForRequest(first, response)).toBe(true)
    expect(
      isResponseForRequest(buildToolsListRequest(), response),
    ).toBe(false)
  })
})
