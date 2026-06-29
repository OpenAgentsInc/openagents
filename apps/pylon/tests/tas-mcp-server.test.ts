import { describe, expect, test } from "bun:test"

import {
  createMcpToolRegistry,
  dispatchToolCall,
  handleToolsList,
  registerTool,
  type McpToolContract,
} from "../src/tas/mcp-server"

const repoStatusTool: McpToolContract = {
  name: "repo.status",
  handlerKind: "repository",
  readOnly: true,
}

describe("tas mcp server pure dispatch core", () => {
  test("tools/list lists registered tools", () => {
    const registry = createMcpToolRegistry()

    registerTool(registry, repoStatusTool)
    registerTool(registry, {
      name: "session.cancel",
      handlerKind: "session",
      readOnly: false,
    })

    expect(handleToolsList(registry)).toEqual({
      tools: [
        {
          name: "repo.status",
          handlerKind: "repository",
          readOnly: true,
        },
        {
          name: "session.cancel",
          handlerKind: "session",
          readOnly: false,
        },
      ],
    })
  })

  test("dispatch unknown tool returns an error", () => {
    const registry = createMcpToolRegistry([repoStatusTool])

    expect(
      dispatchToolCall(registry, {
        name: "browser.open",
        args: { url: "https://openagents.com" },
      }),
    ).toEqual({
      ok: false,
      error: {
        code: "unknown_tool",
        message: "Unknown MCP tool: browser.open",
      },
    })
  })

  test("dispatch known tool returns a routed descriptor", () => {
    const registry = createMcpToolRegistry([repoStatusTool])

    expect(
      dispatchToolCall(registry, {
        name: "repo.status",
        args: { workspaceRef: "workspace.fixture" },
      }),
    ).toEqual({
      ok: true,
      route: {
        name: "repo.status",
        handlerKind: "repository",
        readOnly: true,
        args: { workspaceRef: "workspace.fixture" },
      },
    })
  })

  test("duplicate registration rejected", () => {
    const registry = createMcpToolRegistry([repoStatusTool])

    expect(() => registerTool(registry, repoStatusTool)).toThrow(
      "MCP tool already registered: repo.status",
    )
  })
})
