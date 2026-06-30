import { describe, expect, test } from "bun:test"
import { fileURLToPath } from "node:url"
import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  khalaToolOk,
  makeKhalaPrivacyRedactionService,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  planKhalaTools,
  redactKhalaPublicText,
  resolveKhalaBackend,
  toOpenAiCompatibleTools,
  type KhalaPrivacyRedactionResult,
  type KhalaRampartGuard,
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

  test("plans visible tools by mode, model metadata, env, and feature flags", () => {
    const networkTool: KhalaToolDefinition = {
      ...echoDefinition,
      authority: "network",
      availability: ["coding", "network"],
      internalId: "khala.test.hosted_lookup",
      name: "hosted_lookup",
    }
    const visionTool: KhalaToolDefinition = {
      ...echoDefinition,
      availability: ["coding"],
      internalId: "khala.test.image_reason",
      name: "image_reason",
    }
    const registry = makeKhalaToolRegistry([
      { definition: echoDefinition },
      {
        definition: networkTool,
        planner: {
          featureFlags: ["network_tools"],
          requiredEnv: ["KHALA_SEARCH_PROVIDER"],
        },
      },
      {
        definition: visionTool,
        planner: {
          modelCapabilities: ["vision"],
          modes: ["coding"],
        },
      },
    ])

    expect(registry.plan({ mode: "inspect" }).visible.map(tool => tool.name)).toEqual(["echo"])
    expect(registry.plan({ mode: "coding" }).visible.map(tool => tool.name)).toEqual(["echo"])
    expect(
      registry.plan({
        env: { KHALA_SEARCH_PROVIDER: "test" },
        featureFlags: { network_tools: true },
        mode: "coding",
        model: { capabilities: ["vision"], id: "test/vision" },
      }).visible.map(tool => tool.name),
    ).toEqual(["echo", "hosted_lookup", "image_reason"])
  })

  test("keeps first-party tools visible while deferring searchable external tools", () => {
    const pluginDeployTool: KhalaToolDefinition = {
      ...echoDefinition,
      authority: "network",
      availability: ["coding", "extension"],
      description: "Deploy an app through an external plugin.",
      internalId: "plugin.deploy",
      label: "Plugin Deploy",
      name: "plugin_deploy",
      prompt: "Deploy with the configured plugin.",
      promptGuidelines: ["Use for deployment requests after repository verification."],
    }
    const mcpDocsTool: KhalaToolDefinition = {
      ...echoDefinition,
      availability: ["coding", "extension"],
      description: "Search the external MCP documentation index.",
      internalId: "mcp.docs.search",
      label: "MCP Docs",
      name: "mcp_docs",
      prompt: "Search external documentation.",
    }
    const registry = makeKhalaToolRegistry([
      { definition: echoDefinition },
      {
        definition: pluginDeployTool,
        planner: { defer: true, searchable: true, source: "plugin" },
      },
      {
        definition: mcpDocsTool,
        planner: { defer: true, searchable: true, source: "mcp" },
      },
    ])

    const plan = registry.plan({ mode: "coding" })

    expect(plan.visible.map(tool => tool.name)).toEqual(["echo", "tool_search"])
    expect(plan.deferred.map(tool => [tool.definition.name, tool.source])).toEqual([
      ["plugin_deploy", "plugin"],
      ["mcp_docs", "mcp"],
    ])
    expect(plan.searchTool?.name).toBe("tool_search")
    expect(plan.searchDeferredTools("deploy", 1).map(tool => tool.definition.name)).toEqual(["plugin_deploy"])
  })

  test("can explicitly include deferred external tools for hosts that do not use progressive disclosure", () => {
    const registry = makeKhalaToolRegistry([
      { definition: echoDefinition },
      {
        definition: {
          ...echoDefinition,
          availability: ["coding", "extension"],
          internalId: "plugin.visible",
          name: "plugin_visible",
        },
        planner: { defer: true, source: "plugin" },
      },
    ])

    expect(registry.plan({ includeDeferred: true, mode: "coding" }).visible.map(tool => tool.name)).toEqual([
      "echo",
      "plugin_visible",
    ])
  })

  test("plans standalone registered tools without a registry instance", () => {
    expect(planKhalaTools([{ definition: echoDefinition }], { mode: "coding" }).visible.map(tool => tool.name)).toEqual([
      "echo",
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

  test("protects text through the Rampart heuristics guard", async () => {
    const redaction = makeKhalaPrivacyRedactionService({
      rampartOptions: { device: "cpu", heuristicsOnly: true },
    })

    const result = await Effect.runPromise(
      redaction.protectUserText("Email alex@example.com and SSN 472-81-0094 before sending."),
    )

    expect(result.engine).toBe("@nationaldesignstudio/rampart")
    expect(result.mode).toBe("rampart_heuristics")
    expect(result.text).toContain("[EMAIL_1]")
    expect(result.text).toContain("[SSN_1]")
    expect(result.placeholders).toContain("[EMAIL_1]")
    expect(result.placeholders).toContain("[SSN_1]")
    expect(result.redacted).toBe(true)
  })

  test("loads the Rampart contextual model under Bun and redacts names by default", async () => {
    const { result, revealed } = await runBunJson<{
      readonly result: KhalaPrivacyRedactionResult
      readonly revealed: string
    }>(
      `
        import { Effect } from "effect";
        import { makeKhalaPrivacyRedactionService } from "./src/index.ts";

        const redaction = makeKhalaPrivacyRedactionService();
        const result = await Effect.runPromise(redaction.protectUserText(
          "My name is Alice Johnson. Email alice@example.com. I live at 100 Main Street in Chicago, IL 60601.",
        ));
        const revealed = await Effect.runPromise(
          redaction.revealForLocalUser("Hello [GIVEN_NAME_1] [SURNAME_1], email [EMAIL_1]."),
        );
        console.log(JSON.stringify({ result, revealed }));
      `,
      khalaToolsRoot,
    )

    expect(result.engine).toBe("@nationaldesignstudio/rampart")
    expect(result.mode).toBe("rampart_model")
    expect(result.text).toContain("[GIVEN_NAME_1] [SURNAME_1]")
    expect(result.text).toContain("[EMAIL_1]")
    expect(result.text).toContain("[BUILDING_NUMBER_1] [STREET_NAME_1]")
    expect(result.text).toContain("Chicago, IL 60601")
    expect(result.text).not.toContain("Alice Johnson")
    expect(result.text).not.toContain("alice@example.com")
    expect(result.text).not.toContain("100 Main Street")
    expect(result.redactionRefs).toEqual(["redaction.khala.rampart.pii"])
    expect(revealed).toBe("Hello Alice Johnson, email alice@example.com.")
  })

  test("keeps one reversible Rampart session table per redaction service", async () => {
    const guard = fakeRampartGuard({
      protectText: text => text.replace("Alex Rivera", "[GIVEN_NAME_1] [SURNAME_1]"),
      revealText: text => text
        .replaceAll("[GIVEN_NAME_1]", "Alex")
        .replaceAll("[SURNAME_1]", "Rivera"),
    })
    const redaction = makeKhalaPrivacyRedactionService({
      guardFactory: async () => guard,
    })

    const protectedText = await Effect.runPromise(
      redaction.protectUserText("My name is Alex Rivera."),
    )
    const revealed = await Effect.runPromise(
      redaction.revealForLocalUser("Hello [GIVEN_NAME_1] [SURNAME_1]."),
    )

    expect(protectedText.mode).toBe("rampart_model")
    expect(protectedText.text).toBe("My name is [GIVEN_NAME_1] [SURNAME_1].")
    expect(protectedText.placeholders).toEqual(["[GIVEN_NAME_1]", "[SURNAME_1]"])
    expect(protectedText.redactionRefs).toContain("redaction.khala.rampart.pii")
    expect(revealed).toBe("Hello Alex Rivera.")
  })

  test("falls back to Rampart heuristics when model initialization fails", async () => {
    const redaction = makeKhalaPrivacyRedactionService({
      guardFactory: async options => {
        if (options?.heuristicsOnly === true) {
          return fakeRampartGuard({
            protectText: text => text.replace("472-81-0094", "[SSN_1]"),
            revealText: text => text.replaceAll("[SSN_1]", "472-81-0094"),
          })
        }
        throw new Error("model unavailable")
      },
      rampartOptions: { device: "cpu" },
    })

    const result = await Effect.runPromise(
      redaction.protectUserText("SSN 472-81-0094"),
    )

    expect(result.mode).toBe("rampart_heuristics")
    expect(result.text).toBe("SSN [SSN_1]")
    expect(result.redactionRefs).toContain("redaction.khala.rampart.full_model_unavailable")
    expect(result.redactionRefs).toContain("redaction.khala.rampart.pii")
  })
})

const khalaToolsRoot = fileURLToPath(new URL("..", import.meta.url))

async function runBunJson<T>(script: string, cwd: string): Promise<T> {
  const proc = Bun.spawn([process.execPath, "--eval", script], {
    cwd,
    env: process.env,
    stderr: "pipe",
    stdout: "pipe",
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) {
    throw new Error(`bun child process failed with exit ${exitCode}\nstdout:\n${stdout}\nstderr:\n${stderr}`)
  }
  const line = stdout.trim().split(/\n/u).filter(Boolean).at(-1)
  if (line === undefined) {
    throw new Error(`bun child process produced no JSON\nstderr:\n${stderr}`)
  }
  return JSON.parse(line) as T
}

function fakeRampartGuard(input: {
  readonly protectText: (text: string) => string
  readonly revealText: (text: string) => string
}): KhalaRampartGuard {
  const protect = async (text: string) => {
    const safe = input.protectText(text)
    return {
      placeholders: safe.match(/\[[A-Z_]+_\d+\]/gu) ?? [],
      text: safe,
    }
  }
  return {
    protect,
    protectReply: protect,
    reveal: input.revealText,
    revealTransform: () => new TransformStream<string, string>({
      transform(chunk, controller) {
        controller.enqueue(input.revealText(chunk))
      },
    }),
  }
}
