import { Effect } from "effect"
import {
  khalaToolError,
  khalaToolOk,
  type KhalaNetworkFetchResult,
  type KhalaToolArtifact,
  type KhalaToolDefinition,
  type KhalaToolExecuteContext,
  type KhalaToolResult,
  type RegisteredKhalaTool,
} from "./index.js"

export interface KhalaWebFetchToolOptions {
  readonly maxBytes?: number
}

export const webFetchToolDefinition: KhalaToolDefinition = {
  authority: "network",
  availability: ["network", "owner_local_full"],
  description: "Fetch one HTTP(S) URL with network approval, bounded redirects, timeouts, and byte limits.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      max_bytes: {
        description: "Maximum response bytes to read before truncating.",
        minimum: 1,
        type: "integer",
      },
      max_output_tokens: {
        description: "Approximate token budget for model-visible text.",
        minimum: 1,
        type: "integer",
      },
      max_redirects: {
        description: "Maximum redirects to follow.",
        minimum: 0,
        type: "integer",
      },
      timeout_ms: {
        description: "Fetch timeout in milliseconds.",
        minimum: 1,
        type: "integer",
      },
      url: {
        description: "HTTP or HTTPS URL to fetch.",
        type: "string",
      },
    },
    required: ["url"],
    type: "object",
  },
  internalId: "khala.network.web_fetch",
  label: "Web Fetch",
  name: "web_fetch",
  outputSchema: {
    additionalProperties: false,
    properties: {
      contentType: { type: "string" },
      fetchedAt: { type: "string" },
      finalUrl: { type: "string" },
      status: { type: "integer" },
      truncated: { type: "boolean" },
      url: { type: "string" },
    },
    required: ["url", "finalUrl", "status", "contentType", "fetchedAt", "truncated"],
    type: "object",
  },
  permissionMode: "approval_required",
  prompt: "Fetch one web URL after explicit network approval.",
  promptGuidelines: [
    "Use repository-local read, glob, and grep for code navigation; do not use web_fetch as local search.",
    "Do not pass headers, cookies, credentials, tokens, or authority flags.",
    "Preserve fetched URL and fetched-at metadata when citing fetched material.",
  ],
  renderer: { kind: "web_fetch", rendererRef: "khala.renderer.web_fetch.v1" },
}

export function createWebFetchTool(options: KhalaWebFetchToolOptions = {}): RegisteredKhalaTool {
  return {
    definition: webFetchToolDefinition,
    execute: (input, context) => executeWebFetchTool(input, context, options),
  }
}

type WebFetchInput = Readonly<{
  maxBytes: number
  maxOutputTokens: number
  maxRedirects: number
  timeoutMs: number
  url: string
}>

function executeWebFetchTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaWebFetchToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeWebFetchInput(input, options)
      const result = await Effect.runPromise(
        context.services.network.fetchUrl({
          maxBytes: args.maxBytes,
          maxRedirects: args.maxRedirects,
          timeoutMs: args.timeoutMs,
          url: args.url,
        }),
      )
      return await renderWebFetchResult(args, result, context)
    } catch (error) {
      return khalaToolError("web_fetch_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

function decodeWebFetchInput(
  input: Readonly<Record<string, unknown>>,
  options: KhalaWebFetchToolOptions,
): WebFetchInput {
  rejectAuthoritySmuggling(input)
  const rawUrl = typeof input.url === "string" ? input.url.trim() : ""
  if (rawUrl.length === 0) throw new Error("web_fetch requires url")
  const url = new URL(rawUrl)
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error("web_fetch only supports http and https URLs")
  }
  if (url.username.length > 0 || url.password.length > 0) {
    throw new Error("web_fetch URL credentials are not allowed")
  }
  return {
    maxBytes: boundedInteger(input.max_bytes, "max_bytes", 1, 1024 * 1024) ?? options.maxBytes ?? 256 * 1024,
    maxOutputTokens: boundedInteger(input.max_output_tokens, "max_output_tokens", 1, 60_000) ?? 6_000,
    maxRedirects: boundedInteger(input.max_redirects, "max_redirects", 0, 10) ?? 3,
    timeoutMs: boundedInteger(input.timeout_ms, "timeout_ms", 1, 30_000) ?? 10_000,
    url: url.toString(),
  }
}

async function renderWebFetchResult(
  args: WebFetchInput,
  result: KhalaNetworkFetchResult,
  context: KhalaToolExecuteContext,
): Promise<KhalaToolResult> {
  const contentType = result.contentType.split(";")[0]?.trim().toLowerCase() ?? "application/octet-stream"
  const textLike = isTextLikeContentType(contentType)
  const bodyText = textLike ? new TextDecoder("utf-8", { fatal: false }).decode(result.body) : ""
  const preview = textLike ? boundedPreview(bodyText, args.maxOutputTokens) : { text: "", truncated: false }
  const shouldSpill = result.bodyTruncated || !textLike || preview.truncated
  const artifacts: KhalaToolArtifact[] = []
  if (shouldSpill) {
    const artifact = await Effect.runPromise(
      context.services.outputStore.writeArtifact({
        bytes: result.body,
        mediaType: result.contentType,
        summary: `web_fetch response body for ${result.finalUrl}`,
      }),
    )
    artifacts.push(artifact)
  }
  const fetchedAt = new Date(result.fetchedAtMs).toISOString()
  const header = `Fetched ${result.finalUrl} (${result.status}${result.statusText.length > 0 ? ` ${result.statusText}` : ""}, ${result.contentType}) at ${fetchedAt}.`
  const redirectText = result.redirectChain.length === 0
    ? ""
    : `\nRedirects:\n${result.redirectChain.map(redirect => `${redirect.status}: ${redirect.from} -> ${redirect.to}`).join("\n")}`
  const bodyLine = textLike
    ? `\n\n${preview.text}${preview.truncated || result.bodyTruncated ? "\n[web_fetch body truncated; see private artifact]" : ""}`
    : `\n\nBinary response body stored as private artifact${artifacts[0] === undefined ? "" : ` ${artifacts[0].artifactRef}`}.`
  const ok = khalaToolOk({
    artifacts,
    modelText: `${header}${redirectText}${bodyLine}`,
    privateDataRefs: artifacts.map(artifact => artifact.artifactRef),
    publicSafety: "private",
    publicSummary:
      `Fetched ${result.finalUrl} with status ${result.status} at ${fetchedAt}; ` +
      `${textLike ? "text" : "binary"} body ${result.body.byteLength} bytes${result.bodyTruncated ? " truncated" : ""}.`,
    ui: {
      artifactRef: artifacts[0]?.artifactRef ?? null,
      binary: !textLike,
      bodyBytes: result.body.byteLength,
      bodyTruncated: result.bodyTruncated || preview.truncated,
      contentType: result.contentType,
      fetchedAt,
      finalUrl: result.finalUrl,
      kind: "web_fetch",
      maxOutputTokens: args.maxOutputTokens,
      redaction: {
        classification: "private_network_body",
        publicSafe: false,
      },
      redirectChain: result.redirectChain,
      status: result.status,
      statusText: result.statusText,
      textPreview: textLike ? preview.text : null,
      url: result.url,
    },
  })
  return result.status < 200 || result.status >= 300 ? { ...ok, status: "failed" } : ok
}

function isTextLikeContentType(contentType: string): boolean {
  return contentType.startsWith("text/") ||
    contentType === "application/json" ||
    contentType === "application/javascript" ||
    contentType === "application/xml" ||
    contentType === "application/xhtml+xml" ||
    contentType.endsWith("+json") ||
    contentType.endsWith("+xml")
}

function boundedPreview(text: string, maxOutputTokens: number): Readonly<{ text: string; truncated: boolean }> {
  const maxChars = Math.max(256, maxOutputTokens * 4)
  if (text.length <= maxChars) return { text, truncated: false }
  return {
    text: text.slice(0, maxChars),
    truncated: true,
  }
}

function boundedInteger(
  value: unknown,
  field: string,
  min: number,
  max: number,
): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || Number(value) < min || Number(value) > max) {
    throw new Error(`web_fetch ${field} must be an integer from ${min} through ${max}`)
  }
  return Number(value)
}

function rejectAuthoritySmuggling(input: Readonly<Record<string, unknown>>): void {
  const rejected = Object.keys(input).find(key => FORBIDDEN_WEB_FETCH_ARG_KEYS.has(normalizeToken(key)))
  if (rejected !== undefined) {
    throw new Error(`web_fetch does not accept ${rejected}; network authority must come from permission policy`)
  }
}

function normalizeToken(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]/gu, "")
}

const FORBIDDEN_WEB_FETCH_ARG_KEYS = new Set([
  "allowheader",
  "allownetwork",
  "authorization",
  "bearer",
  "cookie",
  "headers",
  "hostedpublic",
  "network",
  "networkpermission",
  "openrouterapikey",
  "ownerfullaccess",
  "token",
])
