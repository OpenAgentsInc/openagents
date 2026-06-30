import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  khalaToolOk,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  redactKhalaPublicText,
  resolveKhalaBackend,
  toOpenAiCompatibleTools,
  type KhalaToolDefinition,
} from "./index.js"

const echoDefinition: KhalaToolDefinition = {
  authority: "read",
  availability: ["inspect", "coding"],
  description: "Echo input text.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: { text: { type: "string" } },
    required: ["text"],
    type: "object",
  },
  internalId: "khala.test.echo",
  label: "Echo",
  name: "echo",
  permissionMode: "allow",
  prompt: "Echo text.",
  promptGuidelines: ["Use for tests only."],
}

describe("@openagentsinc/khala-tools foundation", () => {
  test("materializes tool definitions and provider-compatible schemas", () => {
    const registry = makeKhalaToolRegistry([{ definition: echoDefinition }])

    expect(registry.materialize("inspect").map((tool: KhalaToolDefinition) => tool.name)).toEqual(["echo"])
    expect(toOpenAiCompatibleTools(registry.list())).toEqual([
      {
        function: {
          description: "Echo input text.",
          name: "echo",
          parameters: echoDefinition.inputSchema,
        },
        type: "function",
      },
    ])
  })

  test("returns a typed unknown-tool failure", async () => {
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry(),
        { arguments: {}, id: "call_1", name: "missing", sessionId: "session_1" },
        makeKhalaToolServices(),
      ),
    )

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("unknown_tool")
  })

  test("denies approval-required tools through the permission service", async () => {
    let executed = false
    const registry = makeKhalaToolRegistry([
      {
        definition: { ...echoDefinition, permissionMode: "approval_required" },
        execute: () => {
          executed = true
          return Effect.succeed(khalaToolOk({ modelText: "should not run" }))
        },
      },
    ])

    const result = await Effect.runPromise(
      executeKhalaTool(
        registry,
        { arguments: { path: "README.md" }, id: "call_1", name: "echo", sessionId: "session_1" },
        makeKhalaToolServices({ permission: denyAllKhalaPermissionService }),
      ),
    )

    expect(executed).toBe(false)
    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("permission_denied")
  })

  test("executes tools after permission approval and redacts public result text", async () => {
    const registry = makeKhalaToolRegistry([
      {
        definition: { ...echoDefinition, permissionMode: "approval_required" },
        execute: (input: Readonly<Record<string, unknown>>) =>
          Effect.succeed(
            khalaToolOk({
              modelText: `value ${String(input.text)}`,
              publicSummary: `OPENROUTER_API_KEY=${String(input.text)}`,
            }),
          ),
      },
    ])

    const result = await Effect.runPromise(
      executeKhalaTool(
        registry,
        { arguments: { text: "sk-or-testsecret123456789" }, id: "call_1", name: "echo", sessionId: "session_1" },
        makeKhalaToolServices({ permission: allowAllKhalaPermissionService }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.publicSummary).toBe("OPENROUTER_API_KEY=[REDACTED]")
    expect(result.modelOutput.text).toBe("value [REDACTED_OPENROUTER_KEY]")
    expect(result.publicSafety).toBe("redacted")
  })

  test("resolves hosted, request-specific BYOK metadata, and mock backends without exposing raw keys", () => {
    expect(resolveKhalaBackend({ env: {} })).toEqual({
      baseUrl: "https://openagents.com",
      kind: "hosted_openagents",
      model: "openagents/khala",
    })

    expect(resolveKhalaBackend({
      env: {
        OPENROUTER_API_KEY: "sk-or-secret",
        OPENROUTER_MODEL: "anthropic/claude-haiku",
      },
    })).toEqual({
      baseUrl: "https://openagents.com",
      credentialSource: "env:OPENROUTER_API_KEY",
      kind: "hosted_openagents",
      model: "openagents/khala",
      provider: "openrouter",
    })

    const defaultOpenRouter = resolveKhalaBackend({ env: { OPENROUTER_API_KEY: "sk-or-secret" } })
    expect(defaultOpenRouter.kind).toBe("hosted_openagents")
    expect(defaultOpenRouter.model).toBe("openagents/khala")
    expect(JSON.stringify(defaultOpenRouter)).not.toContain("sk-or-secret")
    expect(resolveKhalaBackend({ preferred: "mock" }).kind).toBe("mock")
  })

  test("redacts token-shaped public text", () => {
    expect(redactKhalaPublicText("Bearer abcdefghijklmnopqrstuvwxyz")).toBe("Bearer [REDACTED_TOKEN]")
  })
})
