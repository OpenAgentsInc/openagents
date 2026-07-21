/**
 * Monorepo-only bridge: Khala tool registry → Vercel AI SDK tool map.
 * Kept out of the extracted @openagentsinc/ai-model so the public SDK never
 * depends on @openagentsinc/khala-tools (owner extraction boundary).
 */
import { createHash } from "node:crypto"
import { jsonSchema, tool } from "ai"
import { Effect } from "effect"

import {
  makeKhalaToolDispatcher,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaToolDispatcherOptions,
  type KhalaToolRegistry,
  type KhalaToolResult,
  type KhalaToolServices,
  type RegisteredKhalaTool,
} from "./index.ts"

export type KhalaAiSdkCoreToolBridgeOptions = Readonly<{
  dispatcherOptions?: KhalaToolDispatcherOptions
  registry?: KhalaToolRegistry
  services?: KhalaToolServices
  sessionId?: string
  telemetryTags?: Readonly<Record<string, string | number | boolean>>
  tools?: ReadonlyArray<RegisteredKhalaTool>
}>

export function khalaToolsToAiSdkTools(
  input: KhalaAiSdkCoreToolBridgeOptions,
): Record<string, unknown> {
  const registry = input.registry ?? makeKhalaToolRegistry(input.tools ?? [])
  const dispatcher = makeKhalaToolDispatcher(input.dispatcherOptions)
  const services = input.services ?? makeKhalaToolServices()
  const sessionId = input.sessionId ?? "session.ai_sdk_core"
  const aiTools: Record<string, unknown> = {}

  for (const definition of registry.list()) {
    aiTools[definition.name] = tool({
      description: definition.description,
      inputSchema: jsonSchema(definition.inputSchema),
      metadata: {
        khalaToolInternalId: definition.internalId,
        khalaToolAuthority: definition.authority,
      },
      title: definition.label,
      execute: async (args: unknown, options: unknown) => {
        const toolCallId = toolCallIdFromOptions(options, definition.name, args)
        const dispatched = await Effect.runPromise(
          dispatcher.dispatch({
            invocation: {
              arguments: isRecord(args) ? args : {},
              id: toolCallId,
              name: definition.name,
              sessionId,
            },
            registry,
            services,
            telemetryTags: {
              lane: "ai_sdk_core",
              ...(input.telemetryTags ?? {}),
            },
          }),
        )
        return khalaToolResultForModel(dispatched.result)
      },
    })
  }

  return aiTools
}

function khalaToolResultForModel(result: KhalaToolResult): Record<string, unknown> {
  return {
    artifactRefs: result.artifacts.map((artifact) => artifact.artifactRef),
    privateDataRefs: [...result.privateDataRefs],
    publicSafety: result.publicSafety,
    publicSummary: result.publicSummary,
    redactionRefs: [...result.redactionRefs],
    status: result.status,
    text: result.modelOutput.text,
    ui: result.ui,
  }
}

function toolCallIdFromOptions(
  options: unknown,
  toolName: string,
  args: unknown,
): string {
  if (isRecord(options) && typeof options.toolCallId === "string") {
    return options.toolCallId
  }
  return `tool_call.${stableRef([toolName, JSON.stringify(args)])}`
}

function stableRef(parts: ReadonlyArray<string>): string {
  return createHash("sha256").update(parts.join("\0")).digest("hex").slice(0, 24)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
