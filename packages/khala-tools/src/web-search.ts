import { Effect } from "effect"
import {
  khalaToolError,
  khalaToolOk,
  KhalaToolRuntimeError,
  type KhalaToolDefinition,
  type KhalaToolExecuteContext,
  type KhalaToolResult,
  type KhalaWebSearchItem,
  type KhalaWebSearchResult,
  type RegisteredKhalaTool,
} from "./index.js"

export const webSearchToolDefinition: KhalaToolDefinition = {
  authority: "network",
  availability: ["network", "owner_local_full"],
  description: "Search the web through a configured provider after explicit network approval.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      domains: {
        description: "Optional domain filters such as example.com.",
        items: { type: "string" },
        type: "array",
      },
      limit: {
        description: "Maximum number of search results.",
        minimum: 1,
        type: "integer",
      },
      query: {
        description: "Web search query.",
        type: "string",
      },
      recency_days: {
        description: "Optional recency filter in days.",
        minimum: 1,
        type: "integer",
      },
    },
    required: ["query"],
    type: "object",
  },
  internalId: "khala.network.web_search",
  label: "Web Search",
  name: "web_search",
  outputSchema: {
    additionalProperties: false,
    properties: {
      provider: { type: "string" },
      resultCount: { type: "integer" },
      searchedAt: { type: "string" },
    },
    required: ["provider", "resultCount", "searchedAt"],
    type: "object",
  },
  permissionMode: "approval_required",
  prompt: "Search the web with an approved network/search provider.",
  promptGuidelines: [
    "Use repository-local read, glob, and grep for code navigation before reaching for web_search.",
    "Do not pass provider credentials, headers, cookies, or authority flags.",
    "Cite result URLs and timestamps when using search output.",
  ],
  renderer: { kind: "web_search", rendererRef: "khala.renderer.web_search.v1" },
}

export function createWebSearchTool(): RegisteredKhalaTool {
  return {
    definition: webSearchToolDefinition,
    execute: executeWebSearchTool,
  }
}

type WebSearchInput = Readonly<{
  domains: ReadonlyArray<string>
  limit: number
  query: string
  recencyDays?: number
}>

function executeWebSearchTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeWebSearchInput(input)
      const result = await Effect.runPromise(
        context.services.search.search({
          domains: args.domains,
          limit: args.limit,
          query: args.query,
          ...(args.recencyDays === undefined ? {} : { recencyDays: args.recencyDays }),
        }),
      )
      return renderWebSearchResult(args, result)
    } catch (error) {
      if (error instanceof KhalaToolRuntimeError) return khalaToolError(error.code, error.reason)
      return khalaToolError("web_search_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

function decodeWebSearchInput(input: Readonly<Record<string, unknown>>): WebSearchInput {
  rejectCredentialArgs(input)
  const query = typeof input.query === "string" ? input.query.trim() : ""
  if (query.length === 0) throw new Error("web_search requires query")
  if (query.length > 500) throw new Error("web_search query must be 500 characters or fewer")
  const limit = boundedInteger(input.limit, "limit", 1, 20) ?? 5
  const recencyDays = boundedInteger(input.recency_days, "recency_days", 1, 365)
  const domains = decodeDomains(input.domains)
  return {
    domains,
    limit,
    query,
    ...(recencyDays === undefined ? {} : { recencyDays }),
  }
}

function renderWebSearchResult(args: WebSearchInput, result: KhalaWebSearchResult): KhalaToolResult {
  const searchedAt = new Date(result.searchedAtMs).toISOString()
  const limited = result.results.slice(0, args.limit)
  const modelText = limited.length === 0
    ? `No web results from ${result.provider} at ${searchedAt}.`
    : [
      `Web search results from ${result.provider} at ${searchedAt}.`,
      ...limited.map((item, index) => renderSearchItem(item, index)),
    ].join("\n\n")
  return khalaToolOk({
    modelText,
    publicSafety: "private",
    publicSummary:
      `Web search returned ${limited.length} result${limited.length === 1 ? "" : "s"} from ${result.provider} at ${searchedAt}.`,
    ui: {
      domains: args.domains,
      kind: "web_search",
      limit: args.limit,
      provider: result.provider,
      query: args.query,
      recencyDays: args.recencyDays ?? null,
      redaction: {
        classification: "private_search_query_and_snippets",
        publicSafe: false,
      },
      resultCount: limited.length,
      results: limited,
      searchedAt,
    },
  })
}

function renderSearchItem(item: KhalaWebSearchItem, index: number): string {
  const published = item.publishedAt === undefined ? "" : `\nPublished: ${item.publishedAt}`
  return `${index + 1}. ${item.title}\nURL: ${item.url}${published}\nSnippet: ${item.snippet}`
}

function decodeDomains(value: unknown): ReadonlyArray<string> {
  if (value === undefined) return []
  if (!Array.isArray(value)) throw new Error("web_search domains must be an array")
  if (value.length > 10) throw new Error("web_search accepts at most 10 domains")
  return value.map((entry, index) => {
    const domain = typeof entry === "string" ? entry.trim().toLowerCase() : ""
    if (domain.length === 0) throw new Error(`web_search domain ${index + 1} is required`)
    if (!/^[a-z0-9.-]+$/u.test(domain) || domain.includes("..") || domain.startsWith(".") || domain.endsWith(".")) {
      throw new Error(`web_search domain ${index + 1} must be a domain name, not a URL`)
    }
    return domain
  })
}

function boundedInteger(
  value: unknown,
  field: string,
  min: number,
  max: number,
): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || Number(value) < min || Number(value) > max) {
    throw new Error(`web_search ${field} must be an integer from ${min} through ${max}`)
  }
  return Number(value)
}

function rejectCredentialArgs(input: Readonly<Record<string, unknown>>): void {
  const rejected = Object.keys(input).find(key => FORBIDDEN_WEB_SEARCH_ARG_KEYS.has(normalizeToken(key)))
  if (rejected !== undefined) {
    throw new Error(`web_search does not accept ${rejected}; provider credentials stay in local host storage`)
  }
}

function normalizeToken(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]/gu, "")
}

const FORBIDDEN_WEB_SEARCH_ARG_KEYS = new Set([
  "apikey",
  "authorization",
  "bearer",
  "cookie",
  "credential",
  "headers",
  "hostedpublic",
  "network",
  "networkpermission",
  "ownerfullaccess",
  "providercredential",
  "providerkey",
  "token",
])
