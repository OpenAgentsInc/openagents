import { Effect } from "effect"
import {
  khalaToolError,
  khalaToolOk,
  khalaToolUnavailable,
  KhalaToolRuntimeError,
  type KhalaBrowserActionInput,
  type KhalaBrowserNavigateInput,
  type KhalaBrowserPageSnapshot,
  type KhalaBrowserReadInput,
  type KhalaBrowserReadDomResult,
  type KhalaBrowserReadTextResult,
  type KhalaBrowserScreenshotInput,
  type KhalaBrowserScreenshotResult,
  type KhalaBrowserTypeInput,
  type KhalaBrowserWaitInput,
  type KhalaBrowserWaitKind,
  type KhalaBrowserWaitResult,
  type KhalaToolArtifact,
  type KhalaToolDefinition,
  type KhalaToolExecuteContext,
  type KhalaToolResult,
  type RegisteredKhalaTool,
} from "./index.js"

export const browserNavigateToolDefinition: KhalaToolDefinition = {
  authority: "browser",
  availability: ["browser", "owner_local_full"],
  description: "Navigate the host-provided browser surface to an absolute or host-relative URL.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      timeout_ms: {
        description: "Navigation timeout in milliseconds.",
        minimum: 1,
        type: "integer",
      },
      url: {
        description: "Absolute URL or host-relative URL to open in the browser surface.",
        type: "string",
      },
    },
    required: ["url"],
    type: "object",
  },
  internalId: "khala.browser.navigate",
  label: "Browser Navigate",
  name: "browser_navigate",
  outputSchema: browserSnapshotOutputSchema(),
  permissionMode: "approval_required",
  prompt: "Navigate the browser after explicit browser-surface approval.",
  promptGuidelines: [
    "Use browser tools only when the active task needs page state, not for repository or web search.",
    "Do not pass credentials, cookies, headers, or network authority flags.",
    "Treat page URLs, titles, text, DOM, and screenshots as private local artifacts.",
  ],
  renderer: { kind: "browser", rendererRef: "khala.renderer.browser.v1" },
}

export const browserClickToolDefinition: KhalaToolDefinition = {
  authority: "browser",
  availability: ["browser", "owner_local_full"],
  description: "Click an element in the host-provided browser surface by selector.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      label: {
        description: "Optional human-readable element label for UI display.",
        type: "string",
      },
      selector: {
        description: "Role, test id, text locator, or CSS selector understood by the host browser service.",
        type: "string",
      },
      timeout_ms: {
        description: "Click timeout in milliseconds.",
        minimum: 1,
        type: "integer",
      },
    },
    required: ["selector"],
    type: "object",
  },
  internalId: "khala.browser.click",
  label: "Browser Click",
  name: "browser_click",
  outputSchema: browserSnapshotOutputSchema(),
  permissionMode: "approval_required",
  prompt: "Click a browser element after explicit browser-surface approval.",
  promptGuidelines: [
    "Prefer stable accessibility or data-test selectors when the host supports them.",
    "Do not use click to grant permissions or bypass host approval boundaries.",
    "Keep page details private; public summaries must not expose selectors or page text.",
  ],
  renderer: { kind: "browser", rendererRef: "khala.renderer.browser.v1" },
}

export const browserTypeToolDefinition: KhalaToolDefinition = {
  authority: "browser",
  availability: ["browser", "owner_local_full"],
  description: "Type text into an element in the host-provided browser surface.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      label: {
        description: "Optional human-readable element label for UI display.",
        type: "string",
      },
      selector: {
        description: "Role, test id, text locator, or CSS selector understood by the host browser service.",
        type: "string",
      },
      text: {
        description: "Text to type. This stays in the private tool lane.",
        type: "string",
      },
      timeout_ms: {
        description: "Type timeout in milliseconds.",
        minimum: 1,
        type: "integer",
      },
    },
    required: ["selector", "text"],
    type: "object",
  },
  internalId: "khala.browser.type",
  label: "Browser Type",
  name: "browser_type",
  outputSchema: browserSnapshotOutputSchema(),
  permissionMode: "approval_required",
  prompt: "Type into the browser after explicit browser-surface approval.",
  promptGuidelines: [
    "Do not echo typed text in public summaries.",
    "Do not pass credentials, cookies, headers, or authority flags.",
    "Use ask_user for missing secrets or credentials instead of inventing them.",
  ],
  renderer: { kind: "browser", rendererRef: "khala.renderer.browser.v1" },
}

export const browserReadTextToolDefinition: KhalaToolDefinition = {
  authority: "browser",
  availability: ["browser", "owner_local_full"],
  description: "Read bounded visible text from the host-provided browser surface.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      max_output_tokens: {
        description: "Approximate token budget for model-visible text.",
        minimum: 1,
        type: "integer",
      },
      selector: {
        description: "Optional selector to scope the visible text capture.",
        type: "string",
      },
    },
    required: [],
    type: "object",
  },
  internalId: "khala.browser.read_text",
  label: "Browser Read Text",
  name: "browser_read_text",
  outputSchema: browserTextOutputSchema("textPreview"),
  permissionMode: "approval_required",
  prompt: "Read browser visible text after explicit browser-surface approval.",
  promptGuidelines: [
    "Use a selector when only a small region is relevant.",
    "Keep visible page text private unless the page is already known public-safe.",
    "Use max_output_tokens to keep model context bounded.",
  ],
  renderer: { kind: "browser_text", rendererRef: "khala.renderer.browser.v1" },
}

export const browserReadDomToolDefinition: KhalaToolDefinition = {
  authority: "browser",
  availability: ["browser", "owner_local_full"],
  description: "Read bounded DOM HTML from the host-provided browser surface and store the raw DOM privately.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      max_output_tokens: {
        description: "Approximate token budget for model-visible DOM preview.",
        minimum: 1,
        type: "integer",
      },
      selector: {
        description: "Optional selector to scope the DOM capture.",
        type: "string",
      },
    },
    required: [],
    type: "object",
  },
  internalId: "khala.browser.read_dom",
  label: "Browser Read DOM",
  name: "browser_read_dom",
  outputSchema: browserTextOutputSchema("htmlPreview"),
  permissionMode: "approval_required",
  prompt: "Read browser DOM after explicit browser-surface approval.",
  promptGuidelines: [
    "Use a selector when only a small DOM region is relevant.",
    "Raw DOM is always stored as a private artifact; cite only bounded previews in model reasoning.",
    "Do not use DOM output as a public receipt.",
  ],
  renderer: { kind: "browser_dom", rendererRef: "khala.renderer.browser.v1" },
}

export const browserWaitForToolDefinition: KhalaToolDefinition = {
  authority: "browser",
  availability: ["browser", "owner_local_full"],
  description: "Wait for a bounded browser condition without sleeping blindly.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      kind: {
        enum: ["selector-visible", "text-visible", "url-includes"],
        type: "string",
      },
      selector: {
        description: "Selector required for selector-visible waits.",
        type: "string",
      },
      timeout_ms: {
        description: "Wait timeout in milliseconds.",
        minimum: 1,
        type: "integer",
      },
      value: {
        description: "Text or URL substring required for text-visible and url-includes waits.",
        type: "string",
      },
    },
    required: ["kind"],
    type: "object",
  },
  internalId: "khala.browser.wait_for",
  label: "Browser Wait",
  name: "browser_wait_for",
  outputSchema: {
    additionalProperties: false,
    properties: {
      action: { type: "string" },
      kind: { type: "string" },
      met: { type: "boolean" },
      timestamp: { type: "string" },
      url: { type: "string" },
    },
    required: ["action", "kind", "met", "timestamp", "url"],
    type: "object",
  },
  permissionMode: "approval_required",
  prompt: "Wait for browser page state after explicit browser-surface approval.",
  promptGuidelines: [
    "Use semantic waits such as selector-visible, text-visible, or url-includes.",
    "Do not use this as an arbitrary sleep; every wait must name a condition.",
    "Keep waited-for values private in public summaries.",
  ],
  renderer: { kind: "browser_wait", rendererRef: "khala.renderer.browser.v1" },
}

export const browserScreenshotToolDefinition: KhalaToolDefinition = {
  authority: "browser",
  availability: ["browser", "owner_local_full"],
  description: "Capture the current browser page as a private image artifact.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      label: {
        description: "Optional screenshot label for private UI display.",
        type: "string",
      },
    },
    required: [],
    type: "object",
  },
  internalId: "khala.browser.screenshot",
  label: "Browser Screenshot",
  name: "browser_screenshot",
  outputSchema: {
    additionalProperties: false,
    properties: {
      action: { type: "string" },
      artifactRef: { type: "string" },
      capturedAt: { type: "string" },
      height: { type: "integer" },
      mediaType: { type: "string" },
      url: { type: "string" },
      width: { type: "integer" },
    },
    required: ["action", "artifactRef", "capturedAt", "mediaType", "url"],
    type: "object",
  },
  permissionMode: "approval_required",
  prompt: "Capture a private browser screenshot after explicit browser-surface approval.",
  promptGuidelines: [
    "Screenshots stay private local artifacts.",
    "Do not describe screenshot contents in public summaries.",
    "Use read_text or read_dom when exact text is needed.",
  ],
  renderer: { kind: "browser_screenshot", rendererRef: "khala.renderer.browser.v1" },
}

export const browserToolDefinitions = [
  browserNavigateToolDefinition,
  browserClickToolDefinition,
  browserTypeToolDefinition,
  browserReadTextToolDefinition,
  browserReadDomToolDefinition,
  browserWaitForToolDefinition,
  browserScreenshotToolDefinition,
] as const

export function createBrowserNavigateTool(): RegisteredKhalaTool {
  return {
    definition: browserNavigateToolDefinition,
    execute: (input, context) => executeBrowserNavigate(input, context),
  }
}

export function createBrowserClickTool(): RegisteredKhalaTool {
  return {
    definition: browserClickToolDefinition,
    execute: (input, context) => executeBrowserClick(input, context),
  }
}

export function createBrowserTypeTool(): RegisteredKhalaTool {
  return {
    definition: browserTypeToolDefinition,
    execute: (input, context) => executeBrowserType(input, context),
  }
}

export function createBrowserReadTextTool(): RegisteredKhalaTool {
  return {
    definition: browserReadTextToolDefinition,
    execute: (input, context) => executeBrowserReadText(input, context),
  }
}

export function createBrowserReadDomTool(): RegisteredKhalaTool {
  return {
    definition: browserReadDomToolDefinition,
    execute: (input, context) => executeBrowserReadDom(input, context),
  }
}

export function createBrowserWaitForTool(): RegisteredKhalaTool {
  return {
    definition: browserWaitForToolDefinition,
    execute: (input, context) => executeBrowserWaitFor(input, context),
  }
}

export function createBrowserScreenshotTool(): RegisteredKhalaTool {
  return {
    definition: browserScreenshotToolDefinition,
    execute: (input, context) => executeBrowserScreenshot(input, context),
  }
}

export function createBrowserTools(): ReadonlyArray<RegisteredKhalaTool> {
  return [
    createBrowserNavigateTool(),
    createBrowserClickTool(),
    createBrowserTypeTool(),
    createBrowserReadTextTool(),
    createBrowserReadDomTool(),
    createBrowserWaitForTool(),
    createBrowserScreenshotTool(),
  ]
}

type BrowserTextArgs = KhalaBrowserReadInput & Readonly<{
  maxOutputTokens: number
}>

function executeBrowserNavigate(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeNavigateInput(input)
      const result = await Effect.runPromise(context.services.browser.navigate(args))
      return browserSnapshotResult("navigate", result, `Navigated browser to ${result.url}.`)
    } catch (error) {
      return browserFailure(error)
    }
  })
}

function executeBrowserClick(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeActionInput(input, "browser_click")
      const result = await Effect.runPromise(context.services.browser.click(args))
      return browserSnapshotResult("click", result, `Clicked browser element ${args.label ?? args.selector}.`)
    } catch (error) {
      return browserFailure(error)
    }
  })
}

function executeBrowserType(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeTypeInput(input)
      const result = await Effect.runPromise(context.services.browser.typeText(args))
      return browserSnapshotResult(
        "type",
        result,
        `Typed ${args.text.length} character${args.text.length === 1 ? "" : "s"} into browser element ${args.label ?? args.selector}.`,
      )
    } catch (error) {
      return browserFailure(error)
    }
  })
}

function executeBrowserReadText(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeReadInput(input, "browser_read_text")
      const result = await Effect.runPromise(context.services.browser.readText(readServiceInput(args)))
      return await browserReadTextResult(args, result, context)
    } catch (error) {
      return browserFailure(error)
    }
  })
}

function executeBrowserReadDom(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeReadInput(input, "browser_read_dom")
      const result = await Effect.runPromise(context.services.browser.readDom(readServiceInput(args)))
      return await browserReadDomResult(args, result, context)
    } catch (error) {
      return browserFailure(error)
    }
  })
}

function executeBrowserWaitFor(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeWaitInput(input)
      const result = await Effect.runPromise(context.services.browser.waitFor(args))
      return browserWaitResult(args, result)
    } catch (error) {
      return browserFailure(error)
    }
  })
}

function executeBrowserScreenshot(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeScreenshotInput(input)
      const result = await Effect.runPromise(context.services.browser.screenshot(args))
      return await browserScreenshotResult(args, result, context)
    } catch (error) {
      return browserFailure(error)
    }
  })
}

function decodeNavigateInput(input: Readonly<Record<string, unknown>>): KhalaBrowserNavigateInput {
  rejectBrowserCredentialArgs(input, "browser_navigate")
  const url = optionalString(input.url)?.trim() ?? ""
  if (url.length === 0) throw new Error("browser_navigate requires url")
  if (/^[a-z][a-z0-9+.-]*:/iu.test(url)) {
    const parsed = new URL(url)
    if (parsed.username.length > 0 || parsed.password.length > 0) {
      throw new Error("browser_navigate URL credentials are not allowed")
    }
  }
  return {
    timeoutMs: boundedInteger(input.timeout_ms, "browser_navigate", "timeout_ms", 1, 120_000) ?? 30_000,
    url,
  }
}

function decodeActionInput(
  input: Readonly<Record<string, unknown>>,
  toolName: "browser_click" | "browser_type",
): KhalaBrowserActionInput {
  rejectBrowserCredentialArgs(input, toolName)
  const selector = requiredString(input.selector, toolName, "selector")
  const label = optionalString(input.label)
  return {
    ...(label === undefined ? {} : { label }),
    selector,
    timeoutMs: boundedInteger(input.timeout_ms, toolName, "timeout_ms", 1, 120_000) ?? 10_000,
  }
}

function decodeTypeInput(input: Readonly<Record<string, unknown>>): KhalaBrowserTypeInput {
  const action = decodeActionInput(input, "browser_type")
  const text = requiredString(input.text, "browser_type", "text")
  return {
    ...action,
    text,
  }
}

function decodeReadInput(
  input: Readonly<Record<string, unknown>>,
  toolName: "browser_read_dom" | "browser_read_text",
): BrowserTextArgs {
  rejectBrowserCredentialArgs(input, toolName)
  const selector = optionalString(input.selector)
  return {
    maxOutputTokens: boundedInteger(input.max_output_tokens, toolName, "max_output_tokens", 1, 60_000) ?? 4_000,
    ...(selector === undefined ? {} : { selector }),
  }
}

function decodeWaitInput(input: Readonly<Record<string, unknown>>): KhalaBrowserWaitInput {
  rejectBrowserCredentialArgs(input, "browser_wait_for")
  const kind = requiredString(input.kind, "browser_wait_for", "kind")
  if (!isWaitKind(kind)) throw new Error("browser_wait_for kind must be selector-visible, text-visible, or url-includes")
  const timeoutMs = boundedInteger(input.timeout_ms, "browser_wait_for", "timeout_ms", 1, 120_000) ?? 10_000
  if (kind === "selector-visible") {
    return {
      kind,
      selector: requiredString(input.selector, "browser_wait_for", "selector"),
      timeoutMs,
    }
  }
  return {
    kind,
    timeoutMs,
    value: requiredString(input.value, "browser_wait_for", "value"),
  }
}

function decodeScreenshotInput(input: Readonly<Record<string, unknown>>): KhalaBrowserScreenshotInput {
  rejectBrowserCredentialArgs(input, "browser_screenshot")
  const label = optionalString(input.label)
  return label === undefined ? {} : { label }
}

function browserSnapshotResult(action: string, snapshot: KhalaBrowserPageSnapshot, modelText: string): KhalaToolResult {
  const timestamp = new Date(snapshot.timestampMs).toISOString()
  return khalaToolOk({
    modelText: `${modelText}\nURL: ${snapshot.url}${snapshot.title === undefined ? "" : `\nTitle: ${snapshot.title}`}\nTimestamp: ${timestamp}`,
    publicSafety: "private",
    publicSummary: `Browser ${action} completed at ${timestamp}.`,
    ui: {
      action,
      kind: "browser",
      redaction: browserPrivateRedaction(),
      timestamp,
      ...(snapshot.title === undefined ? {} : { title: snapshot.title }),
      url: snapshot.url,
    },
  })
}

async function browserReadTextResult(
  args: BrowserTextArgs,
  result: KhalaBrowserReadTextResult,
  context: KhalaToolExecuteContext,
): Promise<KhalaToolResult> {
  const capturedAt = new Date(result.timestampMs).toISOString()
  const preview = boundedPreview(result.text, args.maxOutputTokens)
  const artifacts: KhalaToolArtifact[] = []
  if (preview.truncated) {
    artifacts.push(await Effect.runPromise(
      context.services.outputStore.writeArtifact({
        bytes: new TextEncoder().encode(result.text),
        mediaType: "text/plain; charset=utf-8",
        summary: `browser visible text for ${result.url}`,
      }),
    ))
  }
  const artifactLine = artifacts[0] === undefined ? "" : `\n[browser text truncated; see private artifact ${artifacts[0].artifactRef}]`
  return khalaToolOk({
    artifacts,
    modelText:
      `Browser visible text captured at ${capturedAt} from ${result.url}.` +
      `${result.title === undefined ? "" : `\nTitle: ${result.title}`}\n\n${preview.text}${artifactLine}`,
    privateDataRefs: artifacts.map(artifact => artifact.artifactRef),
    publicSafety: "private",
    publicSummary: `Browser visible text captured at ${capturedAt}; ${preview.truncated ? "private artifact written" : "bounded preview returned"}.`,
    ui: {
      action: "read_text",
      artifactRef: artifacts[0]?.artifactRef ?? null,
      capturedAt,
      kind: "browser_text",
      redaction: browserPrivateRedaction(),
      selector: args.selector ?? null,
      textPreview: preview.text,
      truncated: preview.truncated,
      ...(result.title === undefined ? {} : { title: result.title }),
      url: result.url,
    },
  })
}

async function browserReadDomResult(
  args: BrowserTextArgs,
  result: KhalaBrowserReadDomResult,
  context: KhalaToolExecuteContext,
): Promise<KhalaToolResult> {
  const capturedAt = new Date(result.timestampMs).toISOString()
  const preview = boundedPreview(result.html, args.maxOutputTokens)
  const artifact = await Effect.runPromise(
    context.services.outputStore.writeArtifact({
      bytes: new TextEncoder().encode(result.html),
      mediaType: "text/html; charset=utf-8",
      summary: `browser DOM snapshot for ${result.url}`,
    }),
  )
  return khalaToolOk({
    artifacts: [artifact],
    modelText:
      `Browser DOM captured at ${capturedAt} from ${result.url}; raw DOM stored as private artifact ${artifact.artifactRef}.` +
      `${result.title === undefined ? "" : `\nTitle: ${result.title}`}\n\n${preview.text}` +
      `${preview.truncated ? `\n[browser DOM truncated; see private artifact ${artifact.artifactRef}]` : ""}`,
    privateDataRefs: [artifact.artifactRef],
    publicSafety: "private",
    publicSummary: `Browser DOM captured at ${capturedAt}; raw DOM stored as private artifact.`,
    ui: {
      action: "read_dom",
      artifactRef: artifact.artifactRef,
      capturedAt,
      htmlPreview: preview.text,
      kind: "browser_dom",
      redaction: browserPrivateRedaction(),
      selector: args.selector ?? null,
      truncated: preview.truncated,
      ...(result.title === undefined ? {} : { title: result.title }),
      url: result.url,
    },
  })
}

function browserWaitResult(args: KhalaBrowserWaitInput, result: KhalaBrowserWaitResult): KhalaToolResult {
  const timestamp = new Date(result.timestampMs).toISOString()
  return khalaToolOk({
    modelText:
      `Browser wait ${result.met ? "matched" : "did not match"} ${args.kind} at ${timestamp}.` +
      `\nURL: ${result.url}${result.title === undefined ? "" : `\nTitle: ${result.title}`}`,
    publicSafety: "private",
    publicSummary: `Browser wait ${result.met ? "matched" : "did not match"} at ${timestamp}.`,
    ui: {
      action: "wait_for",
      kind: "browser_wait",
      redaction: browserPrivateRedaction(),
      selector: args.selector ?? null,
      timestamp,
      url: result.url,
      value: args.value ?? null,
      waitKind: args.kind,
      ...(result.title === undefined ? {} : { title: result.title }),
      met: result.met,
    },
  })
}

async function browserScreenshotResult(
  args: KhalaBrowserScreenshotInput,
  result: KhalaBrowserScreenshotResult,
  context: KhalaToolExecuteContext,
): Promise<KhalaToolResult> {
  const capturedAt = new Date(result.timestampMs).toISOString()
  const artifact = await Effect.runPromise(
    context.services.outputStore.writeArtifact({
      bytes: result.bytes,
      mediaType: result.mediaType,
      summary: `browser screenshot${args.label === undefined ? "" : ` ${args.label}`} for ${result.url}`,
    }),
  )
  return khalaToolOk({
    artifacts: [artifact],
    modelText:
      `Browser screenshot captured at ${capturedAt} as private artifact ${artifact.artifactRef}.` +
      `\nURL: ${result.url}${result.title === undefined ? "" : `\nTitle: ${result.title}`}`,
    privateDataRefs: [artifact.artifactRef],
    publicSafety: "private",
    publicSummary: `Browser screenshot captured at ${capturedAt}; private artifact written.`,
    ui: {
      action: "screenshot",
      artifactRef: artifact.artifactRef,
      capturedAt,
      height: result.height ?? null,
      kind: "browser_screenshot",
      label: args.label ?? null,
      mediaType: result.mediaType,
      redaction: browserPrivateRedaction(),
      ...(result.title === undefined ? {} : { title: result.title }),
      url: result.url,
      width: result.width ?? null,
    },
  })
}

function browserFailure(error: unknown): KhalaToolResult {
  if (error instanceof KhalaToolRuntimeError && error.code === "browser_unavailable") {
    return khalaToolUnavailable({
      modelText: `${error.code}: ${error.reason}`,
      publicSummary: `${error.code}: ${error.reason}`,
      ui: { code: error.code, kind: "browser_unavailable" },
    })
  }
  if (error instanceof KhalaToolRuntimeError) {
    return khalaToolError(error.code, error.reason)
  }
  return khalaToolError("browser_failed", error instanceof Error ? error.message : String(error))
}

function readServiceInput(args: BrowserTextArgs): KhalaBrowserReadInput {
  return args.selector === undefined ? {} : { selector: args.selector }
}

function requiredString(value: unknown, toolName: string, field: string): string {
  const text = optionalString(value)?.trim() ?? ""
  if (text.length === 0) throw new Error(`${toolName} requires ${field}`)
  if (text.length > 2_000) throw new Error(`${toolName} ${field} must be 2000 characters or fewer`)
  return text
}

function optionalString(value: unknown): string | undefined {
  if (value === undefined) return undefined
  if (typeof value !== "string") throw new Error("expected string value")
  return value
}

function boundedInteger(
  value: unknown,
  toolName: string,
  field: string,
  min: number,
  max: number,
): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || Number(value) < min || Number(value) > max) {
    throw new Error(`${toolName} ${field} must be an integer from ${min} through ${max}`)
  }
  return Number(value)
}

function boundedPreview(text: string, maxOutputTokens: number): Readonly<{ text: string; truncated: boolean }> {
  const maxChars = Math.max(256, maxOutputTokens * 4)
  if (text.length <= maxChars) return { text, truncated: false }
  return {
    text: text.slice(0, maxChars),
    truncated: true,
  }
}

function rejectBrowserCredentialArgs(input: Readonly<Record<string, unknown>>, toolName: string): void {
  const rejected = Object.keys(input).find(key => FORBIDDEN_BROWSER_ARG_KEYS.has(normalizeToken(key)))
  if (rejected !== undefined) {
    throw new Error(`${toolName} does not accept ${rejected}; browser authority must come from permission policy`)
  }
}

function normalizeToken(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]/gu, "")
}

function isWaitKind(value: string): value is KhalaBrowserWaitKind {
  return value === "selector-visible" || value === "text-visible" || value === "url-includes"
}

function browserPrivateRedaction(): Readonly<{ classification: string; publicSafe: false }> {
  return {
    classification: "private_browser_surface",
    publicSafe: false,
  }
}

function browserSnapshotOutputSchema(): Record<string, unknown> {
  return {
    additionalProperties: false,
    properties: {
      action: { type: "string" },
      timestamp: { type: "string" },
      title: { type: "string" },
      url: { type: "string" },
    },
    required: ["action", "timestamp", "url"],
    type: "object",
  }
}

function browserTextOutputSchema(previewField: string): Record<string, unknown> {
  return {
    additionalProperties: false,
    properties: {
      action: { type: "string" },
      artifactRef: { type: ["string", "null"] },
      capturedAt: { type: "string" },
      [previewField]: { type: "string" },
      truncated: { type: "boolean" },
      url: { type: "string" },
    },
    required: ["action", "capturedAt", previewField, "truncated", "url"],
    type: "object",
  }
}

const FORBIDDEN_BROWSER_ARG_KEYS = new Set([
  "apikey",
  "authorization",
  "bearer",
  "cookie",
  "credential",
  "headers",
  "network",
  "networkpermission",
  "ownerfullaccess",
  "providercredential",
  "providerkey",
  "shell",
  "shellpermission",
  "token",
])
