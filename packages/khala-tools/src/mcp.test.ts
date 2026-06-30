import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createExternalMcpRegisteredTools,
  createKhalaPublicMcpToolRegistry,
  handleKhalaMcpRequest,
  khalaToolOk,
  makeKhalaMcpClient,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaMcpExternalTransport,
  type KhalaToolDefinition,
} from "./index.js"

const echoDefinition: KhalaToolDefinition = {
  authority: "read",
  availability: ["inspect"],
  description: "Echo input.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: { text: { type: "string" } },
    type: "object",
  },
  internalId: "khala.test.echo",
  label: "Echo",
  name: "echo",
  permissionMode: "allow",
  prompt: "Echo input.",
  promptGuidelines: [],
}

describe("Khala MCP client and server", () => {
  test("lists only policy-scoped public MCP built-ins", async () => {
    const response = await handleKhalaMcpRequest({
      id: "tools",
      jsonrpc: "2.0",
      method: "tools/list",
    })

    const tools = response.result?.tools as Array<{ name: string; annotations: { sourceLabel: string } }>
    expect(tools.map(tool => tool.name)).toEqual(["read", "ls", "glob", "grep"])
    expect(tools.every(tool => tool.annotations.sourceLabel === "khala-built-in")).toBe(true)
  })

  test("calls registered built-ins through MCP JSON-RPC", async () => {
    const registry = makeKhalaToolRegistry([
      {
        definition: echoDefinition,
        execute: input => Effect.succeed(khalaToolOk({ modelText: String(input.text ?? "") })),
      },
    ])

    const response = await handleKhalaMcpRequest(
      {
        id: "call",
        jsonrpc: "2.0",
        method: "tools/call",
        params: {
          arguments: { text: "hello" },
          name: "echo",
        },
      },
      { registry, services: makeKhalaToolServices() },
    )

    expect(response.result).toEqual({
      content: [{ text: "hello", type: "text" }],
    })
  })

  test("namespaces external MCP tools and prevents built-in shadowing", async () => {
    const transport: KhalaMcpExternalTransport = {
      callTool: async (_server, name) => ({
        content: [{ text: `called ${name}`, type: "text" }],
      }),
      listTools: async () => [
        {
          description: "External read.",
          inputSchema: { type: "object" },
          name: "read",
        },
      ],
    }
    const builtIns = createKhalaPublicMcpToolRegistry().list().map(tool => tool.name)
    const client = makeKhalaMcpClient({
      builtInToolNames: builtIns,
      servers: [
        {
          displayName: "Example MCP",
          lifecycleStatus: "configured",
          requestedAuthorities: ["workspace_read"],
          serverRef: "example",
        },
      ],
      transport,
    })

    const tools = await client.listTools()
    expect(tools).toEqual([
      {
        description: "External read. (source: Example MCP)",
        inputSchema: { type: "object" },
        name: "mcp.example.read",
        originalName: "read",
        serverRef: "example",
        sourceLabel: "Example MCP",
      },
    ])
    expect(builtIns).toContain("read")
    expect(tools[0]?.name).not.toBe("read")
    await expect(client.callTool("read", {})).resolves.toMatchObject({ isError: true })
    await expect(client.callTool("mcp.example.read", {})).resolves.toEqual({
      content: [{ text: "called read", type: "text" }],
    })
  })

  test("represents pending disabled failed and needs-auth servers without connecting", async () => {
    let listCalls = 0
    const client = makeKhalaMcpClient({
      builtInToolNames: [],
      servers: [
        {
          displayName: "Pending",
          lifecycleStatus: "pending",
          requestedAuthorities: ["workspace_read"],
          serverRef: "pending",
        },
        {
          displayName: "Disabled",
          lifecycleStatus: "disabled",
          requestedAuthorities: ["workspace_read"],
          serverRef: "disabled",
        },
        {
          displayName: "Failed",
          lifecycleStatus: "failed",
          requestedAuthorities: ["workspace_read"],
          serverRef: "failed",
        },
        {
          displayName: "Needs Auth",
          lifecycleStatus: "needs_auth",
          requestedAuthorities: ["workspace_read"],
          serverRef: "needs-auth",
        },
      ],
      transport: {
        callTool: async () => ({ content: [] }),
        listTools: async () => {
          listCalls += 1
          return []
        },
      },
    })

    expect(client.listServers().map(server => server.lifecycleStatus)).toEqual([
      "pending",
      "disabled",
      "failed",
      "needs_auth",
    ])
    expect(await client.listTools()).toEqual([])
    expect(listCalls).toBe(0)
  })

  test("blocks external servers outside the MCP authority policy", async () => {
    const client = makeKhalaMcpClient({
      builtInToolNames: [],
      servers: [
        {
          displayName: "Deploy MCP",
          lifecycleStatus: "configured",
          requestedAuthorities: ["deployment"],
          serverRef: "deploy",
        },
      ],
      transport: {
        callTool: async () => ({ content: [] }),
        listTools: async () => {
          throw new Error("should not connect")
        },
      },
    })

    expect(client.listServers()).toMatchObject([
      {
        lifecycleStatus: "blocked_by_policy",
        policyAllowed: false,
        serverRef: "deploy",
      },
    ])
    expect(await client.listTools()).toEqual([])
  })

  test("wraps external MCP tools as delegated Khala tools", async () => {
    const client = makeKhalaMcpClient({
      builtInToolNames: [],
      servers: [
        {
          displayName: "Example MCP",
          lifecycleStatus: "connected",
          requestedAuthorities: ["workspace_read"],
          serverRef: "example",
        },
      ],
      transport: {
        callTool: async (_server, name) => ({ content: [{ text: `external ${name}`, type: "text" }] }),
        listTools: async () => [{ description: "Search externally.", name: "search" }],
      },
    })
    const tools = await createExternalMcpRegisteredTools({ client })
    const registry = makeKhalaToolRegistry(tools)
    const result = await Effect.runPromise(registry.resolve("mcp.example.search")!.execute!(
      { query: "x" },
      {
        definition: registry.resolve("mcp.example.search")!.definition,
        emitProgress: () => Effect.void,
        invocation: {
          arguments: { query: "x" },
          id: "call",
          name: "mcp.example.search",
          sessionId: "session",
        },
        services: makeKhalaToolServices(),
      },
    ))

    expect(registry.resolve("mcp.example.search")?.definition.label).toBe("Example MCP: search")
    expect(result.modelOutput.text).toBe("external search")
    expect(result.publicSummary).toBe("External MCP tool mcp.example.search completed.")
  })
})
