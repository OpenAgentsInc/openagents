import { getAgentByName, routeAgentRequest } from "agents";

import { AIChatAgent } from "@cloudflare/ai-chat";
import { getSandbox } from "@cloudflare/sandbox";
import { createOpencodeServer, proxyToOpencode } from "@cloudflare/sandbox/opencode";
import {
  convertToModelMessages,
  createUIMessageStream,
  createUIMessageStreamResponse,
  generateId,
  generateText,
  jsonSchema,
  stepCountIs,
  streamText,
  type Tool,
  type ToolExecuteFunction,
  tool,
  type StreamTextOnFinishCallback,
  type StreamTextOnChunkCallback,
  type StreamTextOnErrorCallback,
  type ToolExecutionOptions,
  type ToolSet,
  type UIMessage,
  type UIMessageStreamWriter
} from "ai";
import { createWorkersAI } from "workers-ai-provider";
import * as Data from "effect/Data";
import * as Effect from "effect/Effect";
import * as Schema from "effect/Schema";

import * as EffectWorkers from "./effect/workers";

export { Sandbox } from "@cloudflare/sandbox";

const MODEL_ID = "@cf/openai/gpt-oss-120b";
const MODEL_CONFIG_ID = "workers-ai:gpt-oss-120b";
const MAX_CONTEXT_MESSAGES = 25;
const SUMMARY_TRIGGER_MESSAGES = 35;
const SUMMARY_MAX_TOKENS = 256;
const MAX_OUTPUT_TOKENS = 512;
const RATE_LIMIT_WINDOW_MS = 60_000;
const RATE_LIMIT_MAX_MESSAGES = 20;
const LEGACY_STATE_ROW_ID = "autopilot_state";
const SKY_MEMORY_SCHEMA_VERSION = 1;
const SKY_EVENT_SCHEMA_VERSION = 1;
const SKY_RUN_SCHEMA_VERSION = 1;
const SKY_RECEIPT_SCHEMA_VERSION = 1;
const AUTOPILOT_SESSION_VERSION = 1;
const SKY_VERSION = "0.1.0";
const RATE_LIMIT_ROW_ID = "autopilot_rate_limit";
const DEFAULT_TOOL_MAX_CALLS = 4;
const DEFAULT_TOOL_MAX_OUTBOUND_BYTES = 200_000;
const DEFAULT_HTTP_TIMEOUT_MS = 8_000;
const DEFAULT_HTTP_MAX_BYTES = 50_000;
const DEFAULT_TUNNEL_TIMEOUT_MS = 8_000;
const DEFAULT_CODEX_CALLBACK_TIMEOUT_MS = 120_000;
const SANDBOX_WORKSPACE_ROOT = "/workspace";
const OPENCODE_DATA_ROOT = "/workspace/opencode-data";
const OPENCODE_AUTH_PATH = `${OPENCODE_DATA_ROOT}/opencode/auth.json`;
const OPENCODE_ROUTE_PREFIX = "/sandbox/opencode";
const CODEX_AUTH_SCHEMA_VERSION = 1;

class AutopilotEffectError extends Data.TaggedError("AutopilotEffectError")<{
  message: string;
  cause?: unknown;
}> {}
class SandboxEffectError extends Data.TaggedError("SandboxEffectError")<{
  message: string;
  cause?: unknown;
}> {}

const SYSTEM_PROMPT =
  "You are Autopilot, a persistent personal AI agent. " +
  "Be concise, helpful, and remember the ongoing conversation.";
const SUMMARY_PROMPT = [
  "You update Autopilot's memory summary.",
  "Summarize durable facts, user preferences, ongoing tasks, and decisions.",
  "Be concise and use short bullet points.",
  "Avoid quotes or chatty phrasing."
].join(" ");

const MODEL_REGISTRY = [
  {
    id: MODEL_CONFIG_ID,
    provider: "workers-ai",
    model: MODEL_ID,
    options: {
      max_output_tokens: MAX_OUTPUT_TOKENS
    }
  }
];

type ChatMetricLog = {
  event: "autopilot_chat_metrics";
  agent_name: string;
  model_id: string;
  message_count: number;
  ttft_ms: number | null;
  duration_ms: number;
  ok: boolean;
  error?: string;
  finish_reason?: string | null;
};

type ExtensionMetricLog = {
  event: "autopilot_extension_metrics";
  extension_id: string;
  extension_version: string;
  hook: ExtensionHookName;
  duration_ms: number;
  ok: boolean;
  error?: string;
};

type ExtensionToolMetricLog = {
  event: "autopilot_extension_tool_metrics";
  extension_id: string;
  extension_version: string;
  tool_name: string;
  duration_ms: number;
  ok: boolean;
  error?: string;
};

type ToolPolicy = "none" | "read-only" | "read-write";
type ExecutorKind = "workers" | "container" | "tunnel";
type ToolChoiceSetting =
  | "auto"
  | "none"
  | "required"
  | { type: "tool"; toolName: string };

type ToolRunState = {
  calls: number;
  maxCalls: number;
  outboundBytes: number;
  maxOutboundBytes: number;
};

type ExtensionManifest = {
  id: string;
  name: string;
  version: string;
  description?: string;
  tools?: string[];
  system_prompt?: string;
  permissions?: Record<string, unknown>;
  ui?: Record<string, unknown>;
};

type ExtensionHookName =
  | "onRunStart"
  | "onMessage"
  | "onToolCall"
  | "onRunComplete";

type ExtensionHookPayload = {
  thread_id: string;
  run_id: string;
  message?: UIMessage;
  tool_name?: string;
  status?: string;
  error?: string | null;
  duration_ms?: number;
  finish_reason?: string | null;
};

type ExtensionToolFactory = (options: {
  name: string;
  description: string;
  mode: "read" | "write";
  inputSchema: Record<string, unknown>;
  handler: (input: unknown, toolOptions: ToolExecutionOptions) => Promise<unknown>;
}) => Tool<unknown, unknown> & {
  execute: ToolExecuteFunction<unknown, unknown>;
};

type ExtensionToolContext = {
  createTool: ExtensionToolFactory;
};

type ExtensionRuntime = {
  manifest: ExtensionManifest;
  buildTools?: (context: ExtensionToolContext) => ToolSet;
  onRunStart?: (
    payload: ExtensionHookPayload & { extension: ExtensionManifest }
  ) => Promise<void> | void;
  onMessage?: (
    payload: ExtensionHookPayload & { extension: ExtensionManifest }
  ) => Promise<void> | void;
  onToolCall?: (
    payload: ExtensionHookPayload & { extension: ExtensionManifest }
  ) => Promise<void> | void;
  onRunComplete?: (
    payload: ExtensionHookPayload & { extension: ExtensionManifest }
  ) => Promise<void> | void;
};

type LegacyAutopilotStateRow = {
  id: string;
  schema_version: number;
  summary: string | null;
  updated_at: number | null;
};

type SkyMemoryRow = {
  thread_id: string;
  summary: string | null;
  updated_at: number | null;
  schema_version: number;
};

type CodexAuthRow = {
  thread_id: string;
  payload_json: string;
  updated_at: number;
  schema_version: number;
};

type SkyRunRow = {
  run_id: string;
  thread_id: string;
  started_at: number;
  completed_at: number | null;
  status: string;
  model_config_id: string;
  error_code: string | null;
  schema_version: number;
};

type SkyEventRow = {
  run_id: string;
  event_id: number;
  type: string;
  payload_json: string;
  created_at: number;
  schema_version: number;
};

type SkyReceiptRow = {
  run_id: string;
  receipt_json: string;
  created_at: number;
  schema_version: number;
};

type RateLimitRow = {
  id: string;
  window_start: number;
  count: number;
};

type ExtensionCatalogRow = {
  extension_id: string;
  manifest_json: string;
  updated_at: number;
};

type ExtensionPolicyRow = {
  thread_id: string;
  enabled_json: string;
  updated_at: number;
};

type WorkspaceRow = {
  thread_id: string;
  workspace_id: string;
  created_at: number;
  updated_at: number;
};

type WorkspaceFileRow = {
  workspace_id: string;
  path: string;
  content: string;
  updated_at: number;
};

class RateLimitError extends Error {
  retryAfterMs: number;

  constructor(retryAfterMs: number) {
    const retrySeconds = Math.max(1, Math.ceil(retryAfterMs / 1000));
    super(
      `You're sending messages too quickly. Please wait about ${retrySeconds} seconds and try again.`
    );
    this.retryAfterMs = retryAfterMs;
    this.name = "RateLimitError";
  }
}

const buildSystemPrompt = (summary: string | null) => {
  if (!summary) return SYSTEM_PROMPT;
  return `${SYSTEM_PROMPT}\n\nMemory summary:\n${summary}`;
};

const buildSummaryPrompt = (summary: string | null) => {
  if (!summary) return SUMMARY_PROMPT;
  return `${SUMMARY_PROMPT}\n\nExisting summary:\n${summary}`;
};

const combineAbortSignals = (
  ...signals: Array<AbortSignal | undefined>
): AbortSignal | undefined => {
  const activeSignals = signals.filter(
    (signal): signal is AbortSignal => Boolean(signal)
  );
  if (activeSignals.length === 0) return undefined;
  if (activeSignals.length === 1) return activeSignals[0];

  const controller = new AbortController();
  const onAbort = () => controller.abort();

  for (const signal of activeSignals) {
    if (signal.aborted) {
      controller.abort();
      break;
    }
    signal.addEventListener("abort", onAbort, { once: true });
  }

  return controller.signal;
};

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

const toHex = (buffer: ArrayBuffer) =>
  [...new Uint8Array(buffer)]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");

const hashText = async (text: string) => {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    textEncoder.encode(text)
  );
  return toHex(digest);
};

const hashJson = async (value: unknown) => hashText(JSON.stringify(value));

const hmacSha256 = async (secret: string, payload: string) => {
  const key = await crypto.subtle.importKey(
    "raw",
    textEncoder.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const signature = await crypto.subtle.sign(
    "HMAC",
    key,
    textEncoder.encode(payload)
  );
  return toHex(signature);
};

const encodeBase64 = (bytes: Uint8Array) => {
  if (typeof btoa === "function") {
    let binary = "";
    for (const byte of bytes) {
      binary += String.fromCharCode(byte);
    }
    return btoa(binary);
  }
  return Buffer.from(bytes).toString("base64");
};

const decodeBase64 = (value: string) => {
  if (typeof atob === "function") {
    return Uint8Array.from(atob(value), (char) => char.charCodeAt(0));
  }
  return new Uint8Array(Buffer.from(value, "base64"));
};

const encodeBase64UrlBytes = (bytes: Uint8Array) =>
  encodeBase64(bytes)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");

const decodeBase64UrlBytes = (value: string) => {
  const padded = value
    .replace(/-/g, "+")
    .replace(/_/g, "/")
    .padEnd(Math.ceil(value.length / 4) * 4, "=");
  return decodeBase64(padded);
};

const encodeBase64Url = (value: string) =>
  encodeBase64UrlBytes(textEncoder.encode(value));
const decodeBase64Url = (value: string) =>
  textDecoder.decode(decodeBase64UrlBytes(value));

type SandboxSessionToken = {
  thread_id: string;
  user_id: string;
  exp: number;
};

const parseSandboxSessionToken = async (
  secret: string,
  token: string,
  threadId: string
): Promise<SandboxSessionToken | null> => {
  const [payloadPart, signature] = token.split(".");
  if (!payloadPart || !signature) return null;
  const expected = await hmacSha256(secret, payloadPart);
  if (expected !== signature) return null;
  let payload: SandboxSessionToken;
  try {
    payload = JSON.parse(decodeBase64Url(payloadPart)) as SandboxSessionToken;
  } catch {
    return null;
  }
  if (!payload || payload.thread_id !== threadId) return null;
  if (typeof payload.exp !== "number" || payload.exp < Date.now()) {
    return null;
  }
  if (typeof payload.user_id !== "string" || !payload.user_id) {
    return null;
  }
  return payload;
};

const aesKeyCache = new Map<string, Promise<CryptoKey>>();

const getAesKey = (secret: string) => {
  const cached = aesKeyCache.get(secret);
  if (cached) return cached;
  const keyPromise = crypto.subtle
    .digest("SHA-256", textEncoder.encode(secret))
    .then((digest) =>
      crypto.subtle.importKey("raw", digest, "AES-GCM", false, [
        "encrypt",
        "decrypt"
      ])
    );
  aesKeyCache.set(secret, keyPromise);
  return keyPromise;
};

type EncryptedPayload = {
  v: number;
  iv: string;
  data: string;
};

const encryptPayload = async (secret: string, payload: string) => {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await getAesKey(secret);
  const cipher = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv },
    key,
    textEncoder.encode(payload)
  );
  const encrypted: EncryptedPayload = {
    v: 1,
    iv: encodeBase64UrlBytes(iv),
    data: encodeBase64UrlBytes(new Uint8Array(cipher))
  };
  return JSON.stringify(encrypted);
};

const decryptPayload = async (secret: string, encoded: string) => {
  const parsed = JSON.parse(encoded) as EncryptedPayload;
  if (!parsed || parsed.v !== 1) {
    throw new Error("Unsupported Codex payload format.");
  }
  const key = await getAesKey(secret);
  const plaintext = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv: decodeBase64UrlBytes(parsed.iv) },
    key,
    decodeBase64UrlBytes(parsed.data)
  );
  return textDecoder.decode(plaintext);
};

const sandboxEffect = <A>(message: string, thunk: () => Promise<A>) =>
  Effect.tryPromise({
    try: thunk,
    catch: (cause) => new SandboxEffectError({ message, cause })
  });

const runSandbox = <A>(message: string, thunk: () => Promise<A>) =>
  Effect.runPromise(sandboxEffect(message, thunk));

const getThreadSandbox = (env: Env, threadId: string) => {
  if (!env.Sandbox) {
    throw new Error("Sandbox binding is not configured.");
  }
  return getSandbox(env.Sandbox, threadId);
};

const parseSandboxTimestamp = (value: string | undefined) => {
  if (!value) return Date.now();
  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? Date.now() : parsed;
};

const extractSandboxToken = (request: Request) => {
  const header =
    request.headers.get("authorization") ??
    request.headers.get("x-autopilot-sandbox-token");
  if (!header) return null;
  if (header.toLowerCase().startsWith("bearer ")) {
    return header.slice("bearer ".length).trim();
  }
  return header.trim();
};

const buildCorsHeaders = (request: Request, env: Env) => {
  const origin = request.headers.get("origin");
  const configured = env.AUTOPILOT_CODEX_CORS_ORIGIN;
  const allowOrigin = configured || origin;
  if (!allowOrigin) return null;
  return {
    "access-control-allow-origin": allowOrigin,
    "access-control-allow-methods": "POST, OPTIONS",
    "access-control-allow-headers":
      "authorization, content-type, x-autopilot-sandbox-token",
    "access-control-max-age": "86400",
    vary: "origin"
  };
};

const getSandboxErrorCode = (error: unknown) => {
  if (!error || typeof error !== "object") return null;
  if ("code" in error && typeof error.code === "string") {
    return error.code;
  }
  if (
    "errorResponse" in error &&
    error.errorResponse &&
    typeof error.errorResponse === "object" &&
    "code" in error.errorResponse &&
    typeof error.errorResponse.code === "string"
  ) {
    return error.errorResponse.code;
  }
  return null;
};

const isSandboxFileNotFound = (error: unknown) =>
  getSandboxErrorCode(error) === "FILE_NOT_FOUND";

const buildOpencodeProxyRequest = (request: Request, url: URL) => {
  const proxyUrl = new URL(request.url);
  const suffix = url.pathname.slice(OPENCODE_ROUTE_PREFIX.length);
  proxyUrl.pathname = suffix ? (suffix.startsWith("/") ? suffix : `/${suffix}`) : "/";
  proxyUrl.searchParams.delete("thread");
  return new Request(proxyUrl.toString(), request);
};

const ensureOpencodeSandbox = async (
  sandbox: ReturnType<typeof getThreadSandbox>
) => {
  await runSandbox("Sandbox workspace init failed", () =>
    sandbox.mkdir(SANDBOX_WORKSPACE_ROOT, { recursive: true })
  );
  await runSandbox("Sandbox opencode env init failed", () =>
    sandbox.setEnvVars({ XDG_DATA_HOME: OPENCODE_DATA_ROOT })
  );
  await runSandbox("Sandbox opencode data init failed", () =>
    sandbox.mkdir(`${OPENCODE_DATA_ROOT}/opencode`, { recursive: true })
  );
};

const readOpencodeAuth = async (
  sandbox: ReturnType<typeof getThreadSandbox>
) => {
  try {
    const file = await runSandbox("Sandbox readFile failed", () =>
      sandbox.readFile(OPENCODE_AUTH_PATH)
    );
    if (file.isBinary || file.encoding === "base64") {
      throw new Error("OpenCode auth file is not text.");
    }
    return file.content ?? "";
  } catch (error) {
    if (isSandboxFileNotFound(error)) {
      return null;
    }
    throw error;
  }
};

const getCodexSecret = (env: Env) => env.AUTOPILOT_CODEX_SECRET ?? null;

const requireCodexSecret = (env: Env) => {
  const secret = getCodexSecret(env);
  if (!secret) {
    throw new Error("Codex secret is not configured.");
  }
  return secret;
};

const fetchCodexAuthPayload = async (env: Env, threadId: string) => {
  const secret = getCodexSecret(env);
  if (!secret) return null;
  const stub = await getAgentByName(env.Chat, threadId);
  const response = await stub.fetch("https://autopilot.internal/codex-auth", {
    method: "GET",
    headers: { "x-autopilot-codex-secret": secret }
  });
  if (!response.ok) {
    throw new Error(`Codex auth lookup failed (${response.status}).`);
  }
  const body = (await response.json()) as
    | { ok: true; payload: string | null }
    | { ok: false; error: string };
  if (!("ok" in body) || !body.ok) {
    throw new Error("Codex auth lookup failed.");
  }
  return body.payload;
};

const storeCodexAuthPayload = async (
  env: Env,
  threadId: string,
  payload: string
) => {
  const secret = requireCodexSecret(env);
  const stub = await getAgentByName(env.Chat, threadId);
  const response = await stub.fetch("https://autopilot.internal/codex-auth", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-autopilot-codex-secret": secret
    },
    body: JSON.stringify({ payload })
  });
  if (!response.ok) {
    throw new Error(`Codex auth save failed (${response.status}).`);
  }
  const body = (await response.json()) as
    | { ok: true }
    | { ok: false; error: string };
  if (!("ok" in body) || !body.ok) {
    throw new Error("Codex auth save failed.");
  }
};

const hydrateOpencodeAuth = async (
  env: Env,
  threadId: string,
  sandbox: ReturnType<typeof getThreadSandbox>
) => {
  const stored = await fetchCodexAuthPayload(env, threadId);
  if (!stored) return;
  await runSandbox("Sandbox auth write failed", () =>
    sandbox.writeFile(OPENCODE_AUTH_PATH, stored)
  );
};

const parseToolPolicy = (value: string | undefined): ToolPolicy => {
  if (value === "read-only" || value === "read-write" || value === "none") {
    return value;
  }
  return "none";
};

const parseExecutorKind = (value: string | undefined): ExecutorKind => {
  if (value === "container" || value === "tunnel" || value === "workers") {
    return value;
  }
  return "workers";
};

const parseToolChoice = (
  value: string | undefined
): ToolChoiceSetting | undefined => {
  if (!value) return undefined;
  if (value === "auto" || value === "none" || value === "required") {
    return value;
  }
  if (value.startsWith("tool:")) {
    const toolName = value.slice("tool:".length).trim();
    if (toolName) {
      return { type: "tool", toolName };
    }
  }
  return undefined;
};

const parseNumberEnv = (value: string | undefined, fallback: number) => {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
};

const parseAllowlist = (value: string | undefined) =>
  value
    ? value
        .split(",")
        .map((entry) => entry.trim())
        .filter(Boolean)
    : [];

const parseExtensionList = (value: string | undefined) => parseAllowlist(value);

const parseExtensionRef = (value: string) => {
  const [id, version] = value.split("@");
  return {
    id: id.trim(),
    version: version ? version.trim() : null
  };
};

const parseExtensionManifest = (value: unknown): ExtensionManifest | null => {
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  if (
    typeof record.id !== "string" ||
    typeof record.name !== "string" ||
    typeof record.version !== "string"
  ) {
    return null;
  }

  return {
    id: record.id,
    name: record.name,
    version: record.version,
    ...(typeof record.description === "string"
      ? { description: record.description }
      : {}),
    ...(Array.isArray(record.tools)
      ? { tools: record.tools.filter((tool) => typeof tool === "string") }
      : {}),
    ...(typeof record.system_prompt === "string"
      ? { system_prompt: record.system_prompt }
      : {}),
    ...(record.permissions && typeof record.permissions === "object"
      ? { permissions: record.permissions as Record<string, unknown> }
      : {}),
    ...(record.ui && typeof record.ui === "object"
      ? { ui: record.ui as Record<string, unknown> }
      : {})
  };
};

const parseExtensionCatalog = (value: unknown): ExtensionManifest[] => {
  if (Array.isArray(value)) {
    return value
      .map((entry) => parseExtensionManifest(entry))
      .filter(Boolean) as ExtensionManifest[];
  }
  if (value && typeof value === "object") {
    const record = value as Record<string, unknown>;
    if (Array.isArray(record.extensions)) {
      return record.extensions
        .map((entry) => parseExtensionManifest(entry))
        .filter(Boolean) as ExtensionManifest[];
    }
  }
  return [];
};

const isHostAllowed = (host: string, allowlist: Array<string>) => {
  if (allowlist.length === 0) {
    return false;
  }
  if (allowlist.includes("*")) {
    return true;
  }
  return allowlist.some((entry) => {
    if (entry.startsWith("*.")) {
      const suffix = entry.slice(2);
      return host === suffix || host.endsWith(`.${suffix}`);
    }
    return host === entry;
  });
};

const BUILTIN_EXTENSION_MANIFESTS: Record<string, ExtensionManifest> = {
  "sky.echo": {
    id: "sky.echo",
    name: "Sky Echo",
    version: "0.1.0",
    description: "Adds a simple echo tool for extension wiring.",
    tools: ["extension.echo"],
    system_prompt:
      "Extension sky.echo is enabled. Use extension.echo to repeat text when debugging."
  }
};

const buildBuiltinExtensionRuntime = (
  manifest: ExtensionManifest
): ExtensionRuntime | null => {
  if (manifest.id !== "sky.echo") {
    return null;
  }

  return {
    manifest,
    buildTools: ({ createTool }) => ({
      "extension.echo": createTool({
        name: "extension.echo",
        description: "Echoes back provided text.",
        mode: "read",
        inputSchema: {
          type: "object",
          properties: {
            text: { type: "string" }
          },
          required: ["text"],
          additionalProperties: false
        },
        handler: async (input) => {
          const payload = input as { text?: string };
          return { text: typeof payload.text === "string" ? payload.text : "" };
        }
      })
    })
  };
};

const buildManifestOnlyRuntime = (
  manifest: ExtensionManifest
): ExtensionRuntime | null => {
  if (manifest.tools && manifest.tools.length > 0) {
    return null;
  }
  if (!manifest.system_prompt) {
    return null;
  }
  return {
    manifest
  };
};

const normalizeMessageForExport = (message: UIMessage): UIMessage => {
  const parts = message.parts.map((part) => {
    if (part.type !== "file") {
      return part;
    }

    const data = (part as { data?: { url?: string; mimeType?: string } }).data;
    const url = data?.url;
    if (!url) {
      return part;
    }

    if (!url.startsWith("r2://")) {
      return part;
    }

    return {
      type: "ref",
      ref: url,
      metadata: data?.mimeType ? { mimeType: data.mimeType } : undefined
    } as unknown as typeof part;
  });

  return { ...message, parts };
};

export class Chat extends AIChatAgent<Env> {
  private summary: string | null = null;
  private stateLoaded = false;
  private activeAbortController: AbortController | null = null;
  private extensionCatalog: Map<string, ExtensionManifest> | null = null;

  private logChatMetrics(payload: ChatMetricLog) {
    console.log(JSON.stringify(payload));
  }

  private logExtensionMetrics(payload: ExtensionMetricLog) {
    console.log(JSON.stringify(payload));
  }

  private logExtensionToolMetrics(payload: ExtensionToolMetricLog) {
    console.log(JSON.stringify(payload));
  }

  private ensureStateLoaded() {
    if (this.stateLoaded) return;

    this.sql`
      create table if not exists autopilot_rate_limit (
        id text primary key,
        window_start integer not null,
        count integer not null
      )
    `;
    this.sql`
      create table if not exists autopilot_state (
        id text primary key,
        schema_version integer not null,
        summary text,
        updated_at integer
      )
    `;
    this.sql`
      create table if not exists sky_runs (
        run_id text primary key,
        thread_id text not null,
        started_at integer not null,
        completed_at integer,
        status text not null,
        model_config_id text not null,
        error_code text,
        schema_version integer not null
      )
    `;
    this.sql`
      create table if not exists sky_events (
        run_id text not null,
        event_id integer not null,
        type text not null,
        payload_json text not null,
        created_at integer not null,
        schema_version integer not null,
        primary key (run_id, event_id)
      )
    `;
    this.sql`
      create table if not exists sky_receipts (
        run_id text not null,
        receipt_json text not null,
        created_at integer not null,
        schema_version integer not null,
        primary key (run_id, created_at)
      )
    `;
    this.sql`
      create table if not exists sky_memory (
        thread_id text primary key,
        summary text,
        updated_at integer,
        schema_version integer not null
      )
    `;
    this.sql`
      create table if not exists sky_tool_policy (
        thread_id text primary key,
        policy text not null,
        updated_at integer not null
      )
    `;
    this.sql`
      create table if not exists sky_extensions (
        extension_id text primary key,
        manifest_json text not null,
        updated_at integer not null
      )
    `;
    this.sql`
      create table if not exists sky_extension_policy (
        thread_id text primary key,
        enabled_json text not null,
        updated_at integer not null
      )
    `;
    this.sql`
      create table if not exists sky_workspaces (
        thread_id text primary key,
        workspace_id text not null,
        created_at integer not null,
        updated_at integer not null
      )
    `;
    this.sql`
      create table if not exists sky_workspace_files (
        workspace_id text not null,
        path text not null,
        content text not null,
        updated_at integer not null,
        primary key (workspace_id, path)
      )
    `;
    this.sql`
      create table if not exists sky_codex_auth (
        thread_id text primary key,
        payload_json text not null,
        updated_at integer not null,
        schema_version integer not null
      )
    `;

    const memoryRows = this.sql<SkyMemoryRow>`
      select thread_id, summary, updated_at, schema_version
      from sky_memory
      where thread_id = ${this.name}
    `;

    if (!memoryRows.length) {
      const legacyRows = this.sql<LegacyAutopilotStateRow>`
        select id, schema_version, summary, updated_at
        from autopilot_state
        where id = ${LEGACY_STATE_ROW_ID}
      `;
      const legacyRow = legacyRows[0];
      this.summary = legacyRow?.summary ?? null;
      this.sql`
        insert into sky_memory (thread_id, summary, updated_at, schema_version)
        values (
          ${this.name},
          ${this.summary},
          ${legacyRow?.updated_at ?? Date.now()},
          ${SKY_MEMORY_SCHEMA_VERSION}
        )
      `;
    } else {
      const row = memoryRows[0];
      this.summary = row.summary ?? null;
      if (row.schema_version !== SKY_MEMORY_SCHEMA_VERSION) {
        this.sql`
          update sky_memory
          set schema_version = ${SKY_MEMORY_SCHEMA_VERSION}
          where thread_id = ${this.name}
        `;
      }
    }

    this.stateLoaded = true;
  }

  private persistSummary(summary: string | null) {
    this.sql`
      insert into sky_memory (thread_id, summary, updated_at, schema_version)
      values (
        ${this.name},
        ${summary},
        ${Date.now()},
        ${SKY_MEMORY_SCHEMA_VERSION}
      )
      on conflict(thread_id) do update set
        schema_version = excluded.schema_version,
        summary = excluded.summary,
        updated_at = excluded.updated_at
    `;
  }

  private isSkyModeEnabled() {
    return this.env.AUTOPILOT_SKY_MODE === "1";
  }

  private getToolPolicy(): ToolPolicy {
    const defaultPolicy = parseToolPolicy(this.env.AUTOPILOT_TOOL_POLICY);
    const rows = this.sql<{ policy: string }>`
      select policy
      from sky_tool_policy
      where thread_id = ${this.name}
    `;

    if (!rows.length) {
      this.sql`
        insert into sky_tool_policy (thread_id, policy, updated_at)
        values (${this.name}, ${defaultPolicy}, ${Date.now()})
      `;
      return defaultPolicy;
    }

    const policy = parseToolPolicy(rows[0].policy);
    if (policy !== rows[0].policy) {
      this.sql`
        update sky_tool_policy
        set policy = ${policy}, updated_at = ${Date.now()}
        where thread_id = ${this.name}
      `;
    }
    return policy;
  }

  private setExtensionPolicy(enabled: string[]) {
    const normalized = Array.from(
      new Set(
        enabled
          .map((entry) => entry.trim())
          .filter(
            (entry) => Boolean(entry) && (entry === "*" || entry.includes("@"))
          )
      )
    );
    this.sql`
      insert into sky_extension_policy (thread_id, enabled_json, updated_at)
      values (${this.name}, ${JSON.stringify(normalized)}, ${Date.now()})
      on conflict(thread_id) do update set
        enabled_json = excluded.enabled_json,
        updated_at = excluded.updated_at
    `;
    return normalized;
  }

  private getExtensionPolicy(): string[] {
    const defaults = parseExtensionList(this.env.AUTOPILOT_EXTENSION_DEFAULTS);
    const rows = this.sql<ExtensionPolicyRow>`
      select thread_id, enabled_json, updated_at
      from sky_extension_policy
      where thread_id = ${this.name}
    `;

    if (!rows.length) {
      return this.setExtensionPolicy(defaults);
    }

    const row = rows[0];
    let enabled: string[] | null = null;
    try {
      const parsed = JSON.parse(row.enabled_json);
      if (Array.isArray(parsed)) {
        enabled = parsed.filter((entry) => typeof entry === "string");
      }
    } catch {
      enabled = null;
    }

    if (!enabled) {
      return this.setExtensionPolicy(defaults);
    }

    const normalized = enabled.filter(
      (entry) => entry === "*" || entry.includes("@")
    );
    if (normalized.length === 0) {
      return this.setExtensionPolicy(defaults);
    }
    if (normalized.length !== enabled.length) {
      return this.setExtensionPolicy(normalized);
    }

    return normalized;
  }

  private getCodexSecret() {
    const secret = this.env.AUTOPILOT_CODEX_SECRET;
    if (!secret) {
      throw new Error("Codex secret is not configured.");
    }
    return secret;
  }

  private getCodexAuthRow(): CodexAuthRow | null {
    const rows = this.sql<CodexAuthRow>`
      select thread_id, payload_json, updated_at, schema_version
      from sky_codex_auth
      where thread_id = ${this.name}
    `;
    return rows[0] ?? null;
  }

  private async getCodexAuthPayload(): Promise<string | null> {
    const row = this.getCodexAuthRow();
    if (!row) return null;
    try {
      return await decryptPayload(this.getCodexSecret(), row.payload_json);
    } catch (error) {
      console.warn("[Autopilot] Failed to decrypt Codex auth payload", error);
      return null;
    }
  }

  private async setCodexAuthPayload(payload: string) {
    const encrypted = await encryptPayload(this.getCodexSecret(), payload);
    this.sql`
      insert into sky_codex_auth (thread_id, payload_json, updated_at, schema_version)
      values (${this.name}, ${encrypted}, ${Date.now()}, ${CODEX_AUTH_SCHEMA_VERSION})
      on conflict(thread_id) do update set
        payload_json = excluded.payload_json,
        updated_at = excluded.updated_at,
        schema_version = excluded.schema_version
    `;
  }

  private async handleCodexAuthRequest(request: Request) {
    this.ensureStateLoaded();
    const secret = this.env.AUTOPILOT_CODEX_SECRET;
    if (!secret) {
      return new Response("Codex secret is not configured.", { status: 501 });
    }
    const provided = request.headers.get("x-autopilot-codex-secret");
    if (!provided || provided !== secret) {
      return new Response("Unauthorized", { status: 401 });
    }

    if (request.method === "GET") {
      const payload = await this.getCodexAuthPayload();
      return Response.json({ ok: true, payload });
    }

    if (request.method === "POST") {
      const body = (await request.json().catch(() => null)) as
        | { payload?: unknown }
        | null;
      const payload =
        body && typeof body.payload === "string" ? body.payload : null;
      if (!payload) {
        return Response.json(
          { ok: false, error: "Invalid payload." },
          { status: 400 }
        );
      }
      await this.setCodexAuthPayload(payload);
      return Response.json({ ok: true });
    }

    return new Response("Method not allowed", { status: 405 });
  }

  private async loadExtensionCatalog(): Promise<Map<string, ExtensionManifest>> {
    if (this.extensionCatalog) {
      return this.extensionCatalog;
    }

    const catalog = new Map<string, ExtensionManifest>();
    for (const manifest of Object.values(BUILTIN_EXTENSION_MANIFESTS)) {
      catalog.set(manifest.id, manifest);
    }

    const rows = this.sql<ExtensionCatalogRow>`
      select extension_id, manifest_json, updated_at
      from sky_extensions
      order by updated_at desc
    `;

    for (const row of rows) {
      try {
        const parsed = JSON.parse(row.manifest_json);
        const manifest = parseExtensionManifest(parsed);
        if (manifest) {
          catalog.set(manifest.id, manifest);
        }
      } catch (error) {
        console.warn("[Autopilot] Failed to parse extension manifest", error);
      }
    }

    const catalogJson = this.env.AUTOPILOT_EXTENSION_CATALOG_JSON;
    const catalogUrl = this.env.AUTOPILOT_EXTENSION_CATALOG_URL;
    const catalogKey =
      this.env.AUTOPILOT_EXTENSION_CATALOG_KEY ?? "extensions/catalog.json";
    const catalogKv = this.env.AUTOPILOT_EXTENSION_KV;
    const catalogBucket = this.env.AUTOPILOT_EXTENSION_BUCKET;

    let externalCatalog: unknown | null = null;

    if (catalogJson) {
      try {
        externalCatalog = JSON.parse(catalogJson);
      } catch (error) {
        console.warn("[Autopilot] Failed to parse extension catalog JSON", error);
      }
    } else if (catalogKv) {
      try {
        const value = await catalogKv.get(catalogKey);
        if (value) {
          externalCatalog = JSON.parse(value);
        }
      } catch (error) {
        console.warn("[Autopilot] Failed to load extension catalog from KV", error);
      }
    } else if (catalogBucket) {
      try {
        const object = await catalogBucket.get(catalogKey);
        if (object) {
          const text = await object.text();
          externalCatalog = JSON.parse(text);
        }
      } catch (error) {
        console.warn("[Autopilot] Failed to load extension catalog from R2", error);
      }
    } else if (catalogUrl) {
      try {
        const response = await fetch(catalogUrl);
        externalCatalog = await response.json();
      } catch (error) {
        console.warn("[Autopilot] Failed to fetch extension catalog URL", error);
      }
    }

    if (externalCatalog) {
      try {
        const manifests = parseExtensionCatalog(externalCatalog);
        const now = Date.now();
        for (const manifest of manifests) {
          catalog.set(manifest.id, manifest);
          this.sql`
            insert into sky_extensions (extension_id, manifest_json, updated_at)
            values (${manifest.id}, ${JSON.stringify(manifest)}, ${now})
            on conflict(extension_id) do update set
              manifest_json = excluded.manifest_json,
              updated_at = excluded.updated_at
          `;
        }
      } catch (error) {
        console.warn("[Autopilot] Failed to load extension catalog", error);
      }
    }

    this.extensionCatalog = catalog;
    return catalog;
  }

  private async resolveActiveExtensions(): Promise<ExtensionRuntime[]> {
    const enabledRefs = this.getExtensionPolicy();
    const allowlist = parseExtensionList(this.env.AUTOPILOT_EXTENSION_ALLOWLIST);
    if (!enabledRefs.length || allowlist.length === 0) {
      return [];
    }

    const allowAll = allowlist.includes("*");
    const catalog = await this.loadExtensionCatalog();
    const runtimes: ExtensionRuntime[] = [];

    const addManifest = (manifest: ExtensionManifest) => {
      if (!allowAll) {
        const allowed =
          allowlist.includes(manifest.id) ||
          allowlist.includes(`${manifest.id}@${manifest.version}`);
        if (!allowed) return;
      }
      const runtime =
        buildBuiltinExtensionRuntime(manifest) ??
        buildManifestOnlyRuntime(manifest);
      if (runtime) {
        runtimes.push(runtime);
        return;
      }

      if (manifest.tools && manifest.tools.length > 0) {
        console.warn(
          `[Autopilot] Extension ${manifest.id} has tools but no runtime implementation.`
        );
      }
    };

    for (const ref of enabledRefs) {
      if (ref.trim() === "*") {
        for (const manifest of catalog.values()) {
          addManifest(manifest);
        }
        continue;
      }
      const { id, version } = parseExtensionRef(ref);
      if (!version) {
        console.warn(
          `[Autopilot] Extension ${id || ref} missing version; skipping.`
        );
        continue;
      }
      const manifest = catalog.get(id);
      if (!manifest) continue;
      if (version && manifest.version !== version) continue;
      addManifest(manifest);
    }

    return runtimes;
  }

  private buildExtensionSystemPrompt(extensions: ExtensionRuntime[]) {
    const prompts = extensions
      .map((extension) => extension.manifest.system_prompt)
      .filter((prompt): prompt is string => Boolean(prompt));
    if (!prompts.length) return "";
    return `\n\nExtensions:\n${prompts.join("\n\n")}`;
  }

  private async runExtensionHooks(
    hook: ExtensionHookName,
    extensions: ExtensionRuntime[],
    payload: ExtensionHookPayload
  ) {
    if (!extensions.length) return;

    for (const extension of extensions) {
      const handler = extension[hook];
      if (!handler) continue;
      const startedAt = Date.now();
      try {
        await handler({ ...payload, extension: extension.manifest });
        this.logExtensionMetrics({
          event: "autopilot_extension_metrics",
          extension_id: extension.manifest.id,
          extension_version: extension.manifest.version,
          hook,
          duration_ms: Date.now() - startedAt,
          ok: true
        });
      } catch (error) {
        const message =
          error instanceof Error ? error.message : "extension_hook_error";
        this.logExtensionMetrics({
          event: "autopilot_extension_metrics",
          extension_id: extension.manifest.id,
          extension_version: extension.manifest.version,
          hook,
          duration_ms: Date.now() - startedAt,
          ok: false,
          error: message
        });
      }
    }
  }

  private getExecutorKind(): ExecutorKind {
    if (this.env.AUTOPILOT_EXECUTOR_KIND) {
      return parseExecutorKind(this.env.AUTOPILOT_EXECUTOR_KIND);
    }
    if (this.env.AUTOPILOT_TUNNEL_URL && this.env.AUTOPILOT_TUNNEL_TOKEN) {
      return "tunnel";
    }
    return "workers";
  }

  private ensureWorkspace(): WorkspaceRow {
    const rows = this.sql<WorkspaceRow>`
      select thread_id, workspace_id, created_at, updated_at
      from sky_workspaces
      where thread_id = ${this.name}
    `;

    if (rows.length) {
      return rows[0];
    }

    const now = Date.now();
    const workspaceId = `ws_${generateId()}`;
    this.sql`
      insert into sky_workspaces (thread_id, workspace_id, created_at, updated_at)
      values (${this.name}, ${workspaceId}, ${now}, ${now})
    `;
    return {
      thread_id: this.name,
      workspace_id: workspaceId,
      created_at: now,
      updated_at: now
    };
  }

  private normalizeWorkspacePath(path: string) {
    const trimmed = path.trim().replace(/\\/g, "/");
    const cleaned = trimmed.replace(/^\/+/, "");
    if (!cleaned || cleaned.includes("..")) {
      throw new Error("Invalid workspace path.");
    }
    return cleaned;
  }

  private readWorkspaceFile(path: string) {
    const workspace = this.ensureWorkspace();
    const normalizedPath = this.normalizeWorkspacePath(path);
    const rows = this.sql<WorkspaceFileRow>`
      select workspace_id, path, content, updated_at
      from sky_workspace_files
      where workspace_id = ${workspace.workspace_id}
        and path = ${normalizedPath}
    `;

    return {
      workspace,
      path: normalizedPath,
      file: rows[0] ?? null
    };
  }

  private writeWorkspaceFile(path: string, content: string) {
    const workspace = this.ensureWorkspace();
    const normalizedPath = this.normalizeWorkspacePath(path);
    const existing = this.readWorkspaceFile(normalizedPath);
    const now = Date.now();

    this.sql`
      insert into sky_workspace_files (workspace_id, path, content, updated_at)
      values (${workspace.workspace_id}, ${normalizedPath}, ${content}, ${now})
      on conflict(workspace_id, path) do update set
        content = excluded.content,
        updated_at = excluded.updated_at
    `;

    this.sql`
      update sky_workspaces
      set updated_at = ${now}
      where thread_id = ${this.name}
    `;

    return {
      workspace,
      path: normalizedPath,
      before: existing.file?.content ?? null,
      after: content
    };
  }

  private recordToolReceipt(options: {
    runId: string;
    toolCallId: string;
    toolName: string;
    argsHash: string | null;
    outputHash: string | null;
    patchHash?: string | null;
    localReceipt?: {
      tool_name: string;
      args_hash: string | null;
      output_hash: string | null;
      patch_hash: string | null;
      executor_kind: ExecutorKind;
      started_at: number;
      completed_at: number;
      duration_ms: number;
    } | null;
    localSignature?: string | null;
    startedAt: number;
    completedAt: number;
    status: "success" | "error";
    errorCode?: string | null;
  }) {
    if (!this.isSkyModeEnabled()) return;

    const receipt = {
      schema_version: SKY_RECEIPT_SCHEMA_VERSION,
      cf_sky_version: SKY_VERSION,
      type: "tool",
      run_id: options.runId,
      thread_id: this.name,
      tool_call_id: options.toolCallId,
      tool_name: options.toolName,
      args_hash: options.argsHash,
      output_hash: options.outputHash,
      patch_hash: options.patchHash ?? null,
      local_receipt: options.localReceipt ?? null,
      local_signature: options.localSignature ?? null,
      started_at: options.startedAt,
      completed_at: options.completedAt,
      duration_ms: options.completedAt - options.startedAt,
      status: options.status,
      error_code: options.errorCode ?? null
    };

    this.insertSkyReceipt({
      runId: options.runId,
      receipt,
      createdAt: options.completedAt
    });
  }

  private buildToolRegistry(options: {
    policy: ToolPolicy;
    runId: string;
    emitSkyEvent: (type: string, payload: unknown) => void;
    toolState: ToolRunState;
    workersai: ReturnType<typeof createWorkersAI>;
    extensions: ExtensionRuntime[];
  }): {
    tools?: ToolSet;
    activeTools?: Array<string>;
    extensions: ExtensionRuntime[];
  } {
    const extensionRuntimes = options.extensions;
    if (options.policy === "none") {
      return { extensions: extensionRuntimes };
    }

    const allowlist = parseAllowlist(this.env.AUTOPILOT_HTTP_ALLOWLIST);
    const httpTimeoutMs = parseNumberEnv(
      this.env.AUTOPILOT_HTTP_TIMEOUT_MS,
      DEFAULT_HTTP_TIMEOUT_MS
    );
    const httpMaxBytes = parseNumberEnv(
      this.env.AUTOPILOT_HTTP_MAX_BYTES,
      DEFAULT_HTTP_MAX_BYTES
    );
    const executorKind = this.getExecutorKind();
    const tunnelUrl = this.env.AUTOPILOT_TUNNEL_URL;
    const tunnelToken = this.env.AUTOPILOT_TUNNEL_TOKEN;
    const accessClientId =
      this.env.AUTOPILOT_TUNNEL_ACCESS_CLIENT_ID ??
      this.env.CF_ACCESS_CLIENT_ID;
    const accessClientSecret =
      this.env.AUTOPILOT_TUNNEL_ACCESS_CLIENT_SECRET ??
      this.env.CF_ACCESS_CLIENT_SECRET;
    const tunnelTimeoutMs = parseNumberEnv(
      this.env.AUTOPILOT_TUNNEL_TIMEOUT_MS,
      DEFAULT_TUNNEL_TIMEOUT_MS
    );

    const assertPolicyAllows = (mode: "read" | "write") => {
      if (options.policy === "none") {
        throw new Error("Tools are disabled for this thread.");
      }
      if (options.policy === "read-only" && mode === "write") {
        throw new Error("Tool access is read-only for this thread.");
      }
    };

    const consumeToolCallBudget = () => {
      if (options.toolState.calls >= options.toolState.maxCalls) {
        throw new Error("Tool call budget exceeded for this run.");
      }
      options.toolState.calls += 1;
    };

    const consumeOutboundBytes = (bytes: number) => {
      if (bytes <= 0) return;
      const nextTotal = options.toolState.outboundBytes + bytes;
      if (nextTotal > options.toolState.maxOutboundBytes) {
        throw new Error("Outbound tool data budget exceeded for this run.");
      }
      options.toolState.outboundBytes = nextTotal;
    };

    const assertWorkersExecutor = () => {
      if (executorKind !== "workers") {
        throw new Error(
          `Executor kind ${executorKind} is not available in this runtime.`
        );
      }
    };

    const getSandboxExecutor = () => {
      if (executorKind !== "container") {
        throw new Error(
          `Executor kind ${executorKind} is not available in this runtime.`
        );
      }
      return getThreadSandbox(this.env, this.name);
    };

    const ensureSandboxWorkspace = async (
      sandbox: ReturnType<typeof getThreadSandbox>
    ) => {
      await runSandbox("Sandbox workspace init failed", () =>
        sandbox.mkdir(SANDBOX_WORKSPACE_ROOT, { recursive: true })
      );
    };

    const toSandboxPath = (path: string) =>
      `${SANDBOX_WORKSPACE_ROOT}/${path}`;

    const invokeTunnelTool = async <Output = unknown>(
      toolName: string,
      input: unknown,
      toolOptions: ToolExecutionOptions
    ): Promise<ToolHandlerResult<Output>> => {
      if (executorKind !== "tunnel") {
        throw new Error(
          `Executor kind ${executorKind} is not available in this runtime.`
        );
      }
      if (!tunnelUrl || !tunnelToken) {
        throw new Error("Tunnel executor is not configured.");
      }

      const baseUrl = tunnelUrl.endsWith("/") ? tunnelUrl : `${tunnelUrl}/`;
      const endpoint = new URL("tools/invoke", baseUrl);
      const payload = {
        tool_name: toolName,
        tool_call_id: toolOptions.toolCallId,
        run_id: options.runId,
        thread_id: this.name,
        args: input
      };

      const timeoutController = new AbortController();
      const timeoutId = setTimeout(
        () => timeoutController.abort(),
        tunnelTimeoutMs
      );
      const signal = combineAbortSignals(
        timeoutController.signal,
        toolOptions.abortSignal
      );

      try {
        const accessHeaders: Record<string, string> = {};
        if (accessClientId && accessClientSecret) {
          accessHeaders["cf-access-client-id"] = accessClientId;
          accessHeaders["cf-access-client-secret"] = accessClientSecret;
        }

        const response = await fetch(endpoint.toString(), {
          method: "POST",
          headers: {
            "content-type": "application/json",
            authorization: `Bearer ${tunnelToken}`,
            ...accessHeaders
          },
          body: JSON.stringify(payload),
          ...(signal ? { signal } : {})
        });

        const body = (await response.json()) as
          | {
              ok: true;
              output: Output;
              receipt: {
                tool_name: string;
                args_hash: string | null;
                output_hash: string | null;
                patch_hash?: string | null;
                executor_kind: ExecutorKind;
                started_at: number;
                completed_at: number;
                duration_ms: number;
              };
              signature: string;
            }
          | { ok: false; error: string };

        if (!response.ok || !body.ok) {
          const errorMessage =
            "error" in body && body.error
              ? body.error
              : `Tunnel executor error (${response.status})`;
          throw new Error(errorMessage);
        }

        if (body.receipt.tool_name !== toolName) {
          throw new Error("Tunnel receipt tool name mismatch.");
        }
        if (body.receipt.executor_kind !== "tunnel") {
          throw new Error("Tunnel receipt executor kind mismatch.");
        }

        const signaturePayload = JSON.stringify(body.receipt);
        const expectedSignature = await hmacSha256(
          tunnelToken,
          signaturePayload
        );
        if (expectedSignature !== body.signature) {
          throw new Error("Invalid tunnel receipt signature.");
        }

        const argsHash = await hashText(JSON.stringify(input));
        if (body.receipt.args_hash !== argsHash) {
          throw new Error("Tunnel receipt args hash mismatch.");
        }

        const outputHash = await hashJson(body.output);
        if (body.receipt.output_hash !== outputHash) {
          throw new Error("Tunnel receipt output hash mismatch.");
        }

        const outboundSize = JSON.stringify(body.output).length;
        consumeOutboundBytes(outboundSize);

        return {
          output: body.output,
          receiptMeta: {
            patchHash: body.receipt.patch_hash ?? null,
            localReceipt: {
              tool_name: body.receipt.tool_name,
              args_hash: body.receipt.args_hash,
              output_hash: body.receipt.output_hash,
              patch_hash: body.receipt.patch_hash ?? null,
              executor_kind: body.receipt.executor_kind,
              started_at: body.receipt.started_at,
              completed_at: body.receipt.completed_at,
              duration_ms: body.receipt.duration_ms
            },
            localSignature: body.signature
          }
        };
      } finally {
        clearTimeout(timeoutId);
      }
    };

    type LocalToolReceipt = {
      tool_name: string;
      args_hash: string | null;
      output_hash: string | null;
      patch_hash: string | null;
      executor_kind: ExecutorKind;
      started_at: number;
      completed_at: number;
      duration_ms: number;
    };

    type ToolReceiptMeta = {
      patchHash?: string | null;
      localReceipt?: LocalToolReceipt | null;
      localSignature?: string | null;
    };

    type ToolHandlerResult<Output> =
      | Output
      | { output: Output; receiptMeta: ToolReceiptMeta };

    type WorkspaceReadOutput = {
      executor_kind: ExecutorKind;
      workspace_id: string;
      path: string;
      content: string;
      bytes: number;
      updated_at: number;
    };

    type WorkspaceWritePatch = {
      op: "write";
      path: string;
      before_hash: string | null;
      after_hash: string;
      before_bytes: number;
      after_bytes: number;
    };

    type WorkspaceWriteOutput = {
      executor_kind: ExecutorKind;
      workspace_id: string;
      path: string;
      created: boolean;
      before_hash: string | null;
      after_hash: string;
      before_bytes: number;
      after_bytes: number;
      patch: WorkspaceWritePatch;
    };

    type WorkspaceEditPatch = {
      op: "edit";
      path: string;
      find: string;
      replace: string;
      all: boolean;
      replacements: number;
      before_hash: string;
      after_hash: string;
    };

    type WorkspaceEditOutput = {
      executor_kind: ExecutorKind;
      workspace_id: string;
      path: string;
      replacements: number;
      before_hash: string;
      after_hash: string;
      patch: WorkspaceEditPatch;
    };

    const unwrapToolOutput = <Output>(
      result: ToolHandlerResult<Output>
    ): {
      output: Output;
      patchHash: string | null;
      localReceipt: LocalToolReceipt | null;
      localSignature: string | null;
    } => {
      if (
        result &&
        typeof result === "object" &&
        "output" in result &&
        "receiptMeta" in result
      ) {
        const wrapped = result as {
          output: Output;
          receiptMeta?: ToolReceiptMeta;
        };
        return {
          output: wrapped.output,
          patchHash: wrapped.receiptMeta?.patchHash ?? null,
          localReceipt: wrapped.receiptMeta?.localReceipt ?? null,
          localSignature: wrapped.receiptMeta?.localSignature ?? null
        };
      }
      return {
        output: result as Output,
        patchHash: null,
        localReceipt: null,
        localSignature: null
      };
    };

    const executeToolWithLogging = async <Input, Output>(
      toolName: string,
      mode: "read" | "write",
      input: Input,
      toolOptions: ToolExecutionOptions,
      handler: () => Promise<ToolHandlerResult<Output>>
    ) => {
      assertPolicyAllows(mode);
      consumeToolCallBudget();

      const toolCallId = toolOptions.toolCallId;
      const startedAt = Date.now();
      const argsJson = JSON.stringify(input);
      const argsHash = await hashText(argsJson);

      options.emitSkyEvent("tool.call.started", {
        tool_call_id: toolCallId,
        tool_name: toolName
      });
      options.emitSkyEvent("tool.call.args.delta", {
        tool_call_id: toolCallId,
        tool_name: toolName,
        delta: argsJson,
        format: "json"
      });
      options.emitSkyEvent("tool.call.args.completed", {
        tool_call_id: toolCallId,
        tool_name: toolName,
        args: input
      });

      try {
        const handlerResult = await handler();
        const { output, patchHash, localReceipt, localSignature } =
          unwrapToolOutput(handlerResult);
        const outputHash = await hashJson(output);
        const completedAt = Date.now();
        const durationMs = completedAt - startedAt;

        options.emitSkyEvent("tool.result", {
          tool_call_id: toolCallId,
          tool_name: toolName,
          status: "success",
          output_hash: outputHash
        });
        options.emitSkyEvent("tool.call.completed", {
          tool_call_id: toolCallId,
          tool_name: toolName,
          status: "success",
          duration_ms: durationMs
        });

        this.recordToolReceipt({
          runId: options.runId,
          toolCallId,
          toolName,
          argsHash,
          outputHash,
          patchHash,
          localReceipt,
          localSignature,
          startedAt,
          completedAt,
          status: "success"
        });

        const extensionOwner = extensionToolOwners.get(toolName);
        if (extensionOwner) {
          this.logExtensionToolMetrics({
            event: "autopilot_extension_tool_metrics",
            extension_id: extensionOwner.manifest.id,
            extension_version: extensionOwner.manifest.version,
            tool_name: toolName,
            duration_ms: durationMs,
            ok: true
          });
        }

        await this.runExtensionHooks("onToolCall", extensionRuntimes, {
          thread_id: this.name,
          run_id: options.runId,
          tool_name: toolName,
          status: "success",
          duration_ms: durationMs
        });

        return output;
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : "tool_error";

        options.emitSkyEvent("tool.result", {
          tool_call_id: toolCallId,
          tool_name: toolName,
          status: "error",
          output_hash: null
        });
        const completedAt = Date.now();
        const durationMs = completedAt - startedAt;
        options.emitSkyEvent("tool.call.completed", {
          tool_call_id: toolCallId,
          tool_name: toolName,
          status: "error",
          duration_ms: durationMs
        });

        this.recordToolReceipt({
          runId: options.runId,
          toolCallId,
          toolName,
          argsHash,
          outputHash: null,
          startedAt,
          completedAt,
          status: "error",
          errorCode: errorMessage
        });

        const extensionOwner = extensionToolOwners.get(toolName);
        if (extensionOwner) {
          this.logExtensionToolMetrics({
            event: "autopilot_extension_tool_metrics",
            extension_id: extensionOwner.manifest.id,
            extension_version: extensionOwner.manifest.version,
            tool_name: toolName,
            duration_ms: durationMs,
            ok: false,
            error: errorMessage
          });
        }

        await this.runExtensionHooks("onToolCall", extensionRuntimes, {
          thread_id: this.name,
          run_id: options.runId,
          tool_name: toolName,
          status: "error",
          error: errorMessage,
          duration_ms: durationMs
        });

        throw error;
      }
    };

    const createExtensionTool: ExtensionToolFactory = ({
      name,
      description,
      mode,
      inputSchema,
      handler
    }) =>
      tool<unknown, unknown>({
        description,
        inputSchema: jsonSchema(inputSchema),
        execute: async (input: unknown, toolOptions: ToolExecutionOptions) =>
          executeToolWithLogging(
            name,
            mode,
            input,
            toolOptions,
            () => handler(input, toolOptions)
          )
      }) as Tool<unknown, unknown> & {
        execute: ToolExecuteFunction<unknown, unknown>;
      };

    const extensionToolOwners = new Map<string, ExtensionRuntime>();

    const tools: ToolSet = {
      "http.fetch": tool({
        description:
          "Fetch a URL over HTTPS. Responses are size-limited and gated by an allowlist.",
        inputSchema: jsonSchema({
          type: "object",
          properties: {
            url: { type: "string" },
            method: { type: "string" },
            headers: { type: "object", additionalProperties: { type: "string" } },
            body: { type: "string" },
            max_bytes: { type: "number" }
          },
          required: ["url"],
          additionalProperties: false
        }),
        execute: async (input: {
          url: string;
          method?: string;
          headers?: Record<string, string>;
          body?: string;
          max_bytes?: number;
        }, toolOptions) => {
          const url = new URL(input.url);
          if (!isHostAllowed(url.hostname, allowlist)) {
            throw new Error("HTTP host is not in the allowlist.");
          }

          const method = (input.method ?? "GET").toUpperCase();
          const isWrite = !["GET", "HEAD"].includes(method);

          return executeToolWithLogging(
            "http.fetch",
            isWrite ? "write" : "read",
            input,
            toolOptions,
            async () => {
              const timeoutController = new AbortController();
              const timeoutId = setTimeout(
                () => timeoutController.abort(),
                httpTimeoutMs
              );
              const signal = combineAbortSignals(
                timeoutController.signal,
                toolOptions.abortSignal
              );

              try {
                const response = await fetch(url.toString(), {
                  method,
                  ...(input.headers ? { headers: input.headers } : {}),
                  ...(input.body !== undefined ? { body: input.body } : {}),
                  ...(signal ? { signal } : {})
                });

                const buffer = await response.arrayBuffer();
                const cap = parseNumberEnv(
                  input.max_bytes?.toString(),
                  httpMaxBytes
                );
                const maxBytes = Math.min(cap, httpMaxBytes);
                const truncated = buffer.byteLength > maxBytes;
                const sliced = buffer.slice(0, maxBytes);
                consumeOutboundBytes(sliced.byteLength);

                const bodyText = new TextDecoder().decode(sliced);
                const contentType = response.headers.get("content-type");
                return {
                  ok: response.ok,
                  status: response.status,
                  truncated,
                  content_type: contentType ?? null,
                  body: bodyText
                };
              } finally {
                clearTimeout(timeoutId);
              }
            }
          );
        }
      }),
      summarize: tool({
        description: "Summarize the provided text into concise bullet points.",
        inputSchema: jsonSchema({
          type: "object",
          properties: {
            text: { type: "string" },
            max_tokens: { type: "number" }
          },
          required: ["text"],
          additionalProperties: false
        }),
        execute: async (input: { text: string; max_tokens?: number }, toolOptions) => {
          return executeToolWithLogging(
            "summarize",
            "read",
            input,
            toolOptions,
            async () => {
              const result = await generateText({
                model: options.workersai(
                  MODEL_ID as Parameters<typeof options.workersai>[0]
                ),
                system: SUMMARY_PROMPT,
                prompt: input.text,
                maxOutputTokens: parseNumberEnv(
                  input.max_tokens?.toString(),
                  SUMMARY_MAX_TOKENS
                )
              });
              return { summary: result.text.trim() };
            }
          );
        }
      }),
      extract: tool({
        description:
          "Extract structured JSON from text following optional instructions.",
        inputSchema: jsonSchema({
          type: "object",
          properties: {
            text: { type: "string" },
            instructions: { type: "string" }
          },
          required: ["text"],
          additionalProperties: false
        }),
        execute: async (input: { text: string; instructions?: string }, toolOptions) => {
          return executeToolWithLogging(
            "extract",
            "read",
            input,
            toolOptions,
            async () => {
              const result = await generateText({
                model: options.workersai(
                  MODEL_ID as Parameters<typeof options.workersai>[0]
                ),
                system:
                  "Extract structured JSON from the text. Respond with JSON only." +
                  (input.instructions ? `\n\nInstructions: ${input.instructions}` : ""),
                prompt: input.text,
                maxOutputTokens: SUMMARY_MAX_TOKENS
              });

              let parsed: unknown = null;
              try {
                parsed = JSON.parse(result.text);
              } catch {
                parsed = null;
              }

              return {
                json: parsed,
                raw: result.text.trim()
              };
            }
          );
        }
      }),
      "workspace.read": tool({
        description: "Read a file from the Autopilot workspace.",
        inputSchema: jsonSchema({
          type: "object",
          properties: {
            path: { type: "string" }
          },
          required: ["path"],
          additionalProperties: false
        }),
        execute: async (input: { path: string }, toolOptions) => {
          return executeToolWithLogging(
            "workspace.read",
            "read",
            input,
            toolOptions,
            async (): Promise<ToolHandlerResult<WorkspaceReadOutput>> => {
              if (executorKind === "tunnel") {
                return invokeTunnelTool<WorkspaceReadOutput>(
                  "workspace.read",
                  input,
                  toolOptions
                );
              }
              if (executorKind === "container") {
                const workspace = this.ensureWorkspace();
                const normalizedPath = this.normalizeWorkspacePath(input.path);
                const sandbox = getSandboxExecutor();
                await ensureSandboxWorkspace(sandbox);
                let file;
                try {
                  file = await runSandbox("Sandbox readFile failed", () =>
                    sandbox.readFile(toSandboxPath(normalizedPath))
                  );
                } catch (error) {
                  if (isSandboxFileNotFound(error)) {
                    throw new Error("Workspace file not found.");
                  }
                  throw error;
                }
                if (file.isBinary || file.encoding === "base64") {
                  throw new Error("Binary workspace files are not supported.");
                }
                const content = file.content ?? "";
                consumeOutboundBytes(content.length);
                return {
                  executor_kind: executorKind,
                  workspace_id: workspace.workspace_id,
                  path: normalizedPath,
                  content,
                  bytes: content.length,
                  updated_at: parseSandboxTimestamp(file.timestamp)
                };
              }
              assertWorkersExecutor();
              const result = this.readWorkspaceFile(input.path);
              if (!result.file) {
                throw new Error("Workspace file not found.");
              }
              const content = result.file.content;
              consumeOutboundBytes(content.length);
              return {
                executor_kind: executorKind,
                workspace_id: result.workspace.workspace_id,
                path: result.path,
                content,
                bytes: content.length,
                updated_at: result.file.updated_at
              };
            }
          );
        }
      }),
      "workspace.write": tool({
        description:
          "Write a file to the Autopilot workspace (overwrites existing content).",
        inputSchema: jsonSchema({
          type: "object",
          properties: {
            path: { type: "string" },
            content: { type: "string" }
          },
          required: ["path", "content"],
          additionalProperties: false
        }),
        execute: async (
          input: { path: string; content: string },
          toolOptions
        ) => {
          return executeToolWithLogging(
            "workspace.write",
            "write",
            input,
            toolOptions,
            async (): Promise<ToolHandlerResult<WorkspaceWriteOutput>> => {
              if (executorKind === "tunnel") {
                return invokeTunnelTool<WorkspaceWriteOutput>(
                  "workspace.write",
                  input,
                  toolOptions
                );
              }
              if (executorKind === "container") {
                const workspace = this.ensureWorkspace();
                const normalizedPath = this.normalizeWorkspacePath(input.path);
                const sandbox = getSandboxExecutor();
                await ensureSandboxWorkspace(sandbox);
                let before: string | null = null;
                try {
                  const current = await runSandbox(
                    "Sandbox readFile failed",
                    () => sandbox.readFile(toSandboxPath(normalizedPath))
                  );
                  if (current.isBinary || current.encoding === "base64") {
                    throw new Error("Binary workspace files are not supported.");
                  }
                  before = current.content ?? "";
                } catch (error) {
                  if (!isSandboxFileNotFound(error)) {
                    throw error;
                  }
                }

                await runSandbox("Sandbox writeFile failed", () =>
                  sandbox.writeFile(toSandboxPath(normalizedPath), input.content)
                );

                const beforeBytes = before?.length ?? 0;
                const afterBytes = input.content.length;
                const beforeHash = before ? await hashText(before) : null;
                const afterHash = await hashText(input.content);
                const patch: WorkspaceWritePatch = {
                  op: "write",
                  path: normalizedPath,
                  before_hash: beforeHash,
                  after_hash: afterHash,
                  before_bytes: beforeBytes,
                  after_bytes: afterBytes
                };
                const patchHash = await hashJson(patch);
                const output = {
                  executor_kind: executorKind,
                  workspace_id: workspace.workspace_id,
                  path: normalizedPath,
                  created: before === null,
                  before_hash: beforeHash,
                  after_hash: afterHash,
                  before_bytes: beforeBytes,
                  after_bytes: afterBytes,
                  patch
                };
                consumeOutboundBytes(JSON.stringify(output).length);
                return {
                  output,
                  receiptMeta: { patchHash }
                };
              }
              assertWorkersExecutor();
              const result = this.writeWorkspaceFile(input.path, input.content);
              const beforeBytes = result.before?.length ?? 0;
              const afterBytes = result.after.length;
              const beforeHash = result.before
                ? await hashText(result.before)
                : null;
              const afterHash = await hashText(result.after);
              const patch: WorkspaceWritePatch = {
                op: "write",
                path: result.path,
                before_hash: beforeHash,
                after_hash: afterHash,
                before_bytes: beforeBytes,
                after_bytes: afterBytes
              };
              const patchHash = await hashJson(patch);
              const output = {
                executor_kind: executorKind,
                workspace_id: result.workspace.workspace_id,
                path: result.path,
                created: result.before === null,
                before_hash: beforeHash,
                after_hash: afterHash,
                before_bytes: beforeBytes,
                after_bytes: afterBytes,
                patch
              };
              consumeOutboundBytes(JSON.stringify(output).length);
              return {
                output,
                receiptMeta: { patchHash }
              };
            }
          );
        }
      }),
      "workspace.edit": tool({
        description:
          "Edit a workspace file by applying a string replacement.",
        inputSchema: jsonSchema({
          type: "object",
          properties: {
            path: { type: "string" },
            find: { type: "string" },
            replace: { type: "string" },
            all: { type: "boolean" }
          },
          required: ["path", "find", "replace"],
          additionalProperties: false
        }),
        execute: async (
          input: { path: string; find: string; replace: string; all?: boolean },
          toolOptions
        ) => {
          return executeToolWithLogging(
            "workspace.edit",
            "write",
            input,
            toolOptions,
            async (): Promise<ToolHandlerResult<WorkspaceEditOutput>> => {
              if (executorKind === "tunnel") {
                return invokeTunnelTool<WorkspaceEditOutput>(
                  "workspace.edit",
                  input,
                  toolOptions
                );
              }
              if (executorKind === "container") {
                if (input.find.length === 0) {
                  throw new Error("Find text must not be empty.");
                }
                const workspace = this.ensureWorkspace();
                const normalizedPath = this.normalizeWorkspacePath(input.path);
                const sandbox = getSandboxExecutor();
                await ensureSandboxWorkspace(sandbox);
                let current;
                try {
                  current = await runSandbox("Sandbox readFile failed", () =>
                    sandbox.readFile(toSandboxPath(normalizedPath))
                  );
                } catch (error) {
                  if (isSandboxFileNotFound(error)) {
                    throw new Error("Workspace file not found.");
                  }
                  throw error;
                }
                if (current.isBinary || current.encoding === "base64") {
                  throw new Error("Binary workspace files are not supported.");
                }
                const before = current.content ?? "";
                let after = before;
                let replacements = 0;

                if (input.all) {
                  const parts = before.split(input.find);
                  if (parts.length === 1) {
                    throw new Error("Find text not found.");
                  }
                  replacements = parts.length - 1;
                  after = parts.join(input.replace);
                } else {
                  const index = before.indexOf(input.find);
                  if (index === -1) {
                    throw new Error("Find text not found.");
                  }
                  replacements = 1;
                  after =
                    before.slice(0, index) +
                    input.replace +
                    before.slice(index + input.find.length);
                }

                await runSandbox("Sandbox writeFile failed", () =>
                  sandbox.writeFile(toSandboxPath(normalizedPath), after)
                );
                const beforeHash = await hashText(before);
                const afterHash = await hashText(after);
                const patch: WorkspaceEditPatch = {
                  op: "edit",
                  path: normalizedPath,
                  find: input.find,
                  replace: input.replace,
                  all: Boolean(input.all),
                  replacements,
                  before_hash: beforeHash,
                  after_hash: afterHash
                };
                const patchHash = await hashJson(patch);
                const output = {
                  executor_kind: executorKind,
                  workspace_id: workspace.workspace_id,
                  path: normalizedPath,
                  replacements,
                  before_hash: beforeHash,
                  after_hash: afterHash,
                  patch
                };
                consumeOutboundBytes(JSON.stringify(output).length);
                return {
                  output,
                  receiptMeta: { patchHash }
                };
              }
              assertWorkersExecutor();
              if (input.find.length === 0) {
                throw new Error("Find text must not be empty.");
              }
              const current = this.readWorkspaceFile(input.path);
              if (!current.file) {
                throw new Error("Workspace file not found.");
              }

              const before = current.file.content;
              let after = before;
              let replacements = 0;

              if (input.all) {
                const parts = before.split(input.find);
                if (parts.length === 1) {
                  throw new Error("Find text not found.");
                }
                replacements = parts.length - 1;
                after = parts.join(input.replace);
              } else {
                const index = before.indexOf(input.find);
                if (index === -1) {
                  throw new Error("Find text not found.");
                }
                replacements = 1;
                after =
                  before.slice(0, index) +
                  input.replace +
                  before.slice(index + input.find.length);
              }

              const written = this.writeWorkspaceFile(current.path, after);
              const beforeHash = await hashText(before);
              const afterHash = await hashText(after);
              const patch: WorkspaceEditPatch = {
                op: "edit",
                path: written.path,
                find: input.find,
                replace: input.replace,
                all: Boolean(input.all),
                replacements,
                before_hash: beforeHash,
                after_hash: afterHash
              };
              const patchHash = await hashJson(patch);
              const output = {
                executor_kind: executorKind,
                workspace_id: written.workspace.workspace_id,
                path: written.path,
                replacements,
                before_hash: beforeHash,
                after_hash: afterHash,
                patch
              };
              consumeOutboundBytes(JSON.stringify(output).length);
              return {
                output,
                receiptMeta: { patchHash }
              };
            }
          );
        }
      })
    };

    if (extensionRuntimes.length) {
      for (const extension of extensionRuntimes) {
        if (!extension.buildTools) continue;
        const extensionTools = extension.buildTools({
          createTool: createExtensionTool
        });
        for (const [toolName, toolDef] of Object.entries(extensionTools)) {
          if (
            extension.manifest.tools &&
            !extension.manifest.tools.includes(toolName)
          ) {
            console.warn(
              `[Autopilot] Extension ${extension.manifest.id} attempted to register undeclared tool ${toolName}`
            );
            continue;
          }
          if (tools[toolName]) {
            console.warn(
              `[Autopilot] Extension ${extension.manifest.id} attempted to override tool ${toolName}`
            );
            continue;
          }
          tools[toolName] = toolDef;
          extensionToolOwners.set(toolName, extension);
        }
      }
    }

    const activeTools = Object.keys(tools);

    return { tools, activeTools, extensions: extensionRuntimes };
  }

  private insertSkyRun(runId: string, startedAt: number) {
    this.sql`
      insert into sky_runs (
        run_id,
        thread_id,
        started_at,
        completed_at,
        status,
        model_config_id,
        error_code,
        schema_version
      )
      values (
        ${runId},
        ${this.name},
        ${startedAt},
        null,
        'started',
        ${MODEL_CONFIG_ID},
        null,
        ${SKY_RUN_SCHEMA_VERSION}
      )
    `;
  }

  private updateSkyRun(options: {
    runId: string;
    status: string;
    completedAt: number;
    errorCode?: string | null;
  }) {
    this.sql`
      update sky_runs
      set
        status = ${options.status},
        completed_at = ${options.completedAt},
        error_code = ${options.errorCode ?? null}
      where run_id = ${options.runId}
    `;
  }

  private insertSkyEvent(options: {
    runId: string;
    eventId: number;
    type: string;
    payload: unknown;
    createdAt: number;
  }) {
    this.sql`
      insert into sky_events (
        run_id,
        event_id,
        type,
        payload_json,
        created_at,
        schema_version
      )
      values (
        ${options.runId},
        ${options.eventId},
        ${options.type},
        ${JSON.stringify(options.payload)},
        ${options.createdAt},
        ${SKY_EVENT_SCHEMA_VERSION}
      )
    `;
  }

  private insertSkyReceipt(options: {
    runId: string;
    receipt: unknown;
    createdAt: number;
  }) {
    this.sql`
      insert into sky_receipts (
        run_id,
        receipt_json,
        created_at,
        schema_version
      )
      values (
        ${options.runId},
        ${JSON.stringify(options.receipt)},
        ${options.createdAt},
        ${SKY_RECEIPT_SCHEMA_VERSION}
      )
    `;
  }

  private async finalizeSkyRun(options: {
    runId: string;
    status: string;
    startedAt: number;
    completedAt: number;
    finishReason: string | null;
    errorCode?: string | null;
    inputHashPromise: Promise<string> | null;
    outputText: string | null;
  }) {
    this.updateSkyRun({
      runId: options.runId,
      status: options.status,
      completedAt: options.completedAt,
      errorCode: options.errorCode ?? null
    });

    let inputHash: string | null = null;
    let outputHash: string | null = null;

    if (options.inputHashPromise) {
      inputHash = await options.inputHashPromise;
    }
    if (options.outputText) {
      outputHash = await hashText(options.outputText);
    }

    const receipt = {
      schema_version: SKY_RECEIPT_SCHEMA_VERSION,
      cf_sky_version: SKY_VERSION,
      type: "run",
      run_id: options.runId,
      thread_id: this.name,
      model_config_id: MODEL_CONFIG_ID,
      input_hash: inputHash,
      output_hash: outputHash,
      started_at: options.startedAt,
      completed_at: options.completedAt,
      duration_ms: options.completedAt - options.startedAt,
      status: options.status,
      finish_reason: options.finishReason,
      error_code: options.errorCode ?? null
    };

    this.insertSkyReceipt({
      runId: options.runId,
      receipt,
      createdAt: options.completedAt
    });
  }

  private checkExtensionAdmin(request: Request): { ok: boolean; status: number } {
    return this.checkAdmin(request, this.env.AUTOPILOT_EXTENSION_ADMIN_SECRET);
  }

  private checkToolAdmin(request: Request): { ok: boolean; status: number } {
    return this.checkAdmin(
      request,
      this.env.AUTOPILOT_TOOL_ADMIN_SECRET ??
        this.env.AUTOPILOT_EXTENSION_ADMIN_SECRET
    );
  }

  private checkAdmin(
    request: Request,
    secret: string | undefined
  ): { ok: boolean; status: number } {
    if (!secret) {
      return { ok: false, status: 404 };
    }

    const provided =
      request.headers.get("x-autopilot-admin-secret") ??
      request.headers.get("authorization") ??
      "";
    const token = provided.startsWith("Bearer ")
      ? provided.slice(7)
      : provided;
    if (token !== secret) {
      return { ok: false, status: 403 };
    }

    return { ok: true, status: 200 };
  }

  private jsonError(
    status: number,
    code: string,
    message: string,
    extra?: Record<string, unknown>
  ) {
    return Response.json(
      {
        ok: false,
        code,
        message,
        thread_id: this.name,
        ...(extra ?? {})
      },
      { status }
    );
  }

  private async handleExtensionPolicyRequest(request: Request) {
    this.ensureStateLoaded();
    const adminCheck = this.checkExtensionAdmin(request);
    if (!adminCheck.ok) {
      return this.jsonError(
        adminCheck.status,
        adminCheck.status === 404 ? "not_found" : "forbidden",
        adminCheck.status === 404 ? "Not found" : "Forbidden"
      );
    }

    if (request.method === "GET") {
      return Response.json({ enabled: this.getExtensionPolicy() });
    }

    if (request.method === "POST") {
      let body: unknown = null;
      try {
        body = await request.json();
      } catch {
        return this.jsonError(400, "invalid_json", "Invalid JSON");
      }

      const record = body as Record<string, unknown>;
      const enabled = Array.isArray(record.enabled)
        ? record.enabled.filter((entry) => typeof entry === "string")
        : null;

      if (!enabled) {
        return this.jsonError(400, "missing_enabled", "Missing enabled array");
      }

      const allowlist = parseExtensionList(this.env.AUTOPILOT_EXTENSION_ALLOWLIST);
      if (!allowlist.length) {
        return this.jsonError(
          400,
          "allowlist_empty",
          "Extension allowlist is empty"
        );
      }

      const allowAll = allowlist.includes("*");
      const catalog = await this.loadExtensionCatalog();
      const disallowed: string[] = [];
      const missing: string[] = [];
      const missingVersion: string[] = [];

      for (const entry of enabled) {
        if (entry === "*") {
          if (!allowAll) {
            disallowed.push(entry);
          }
          continue;
        }

        const { id, version } = parseExtensionRef(entry);
        if (!version) {
          missingVersion.push(entry);
          continue;
        }
        if (!id) {
          disallowed.push(entry);
          continue;
        }
        if (!allowAll) {
          const allowed =
            allowlist.includes(id) || (version && allowlist.includes(entry));
          if (!allowed) {
            disallowed.push(entry);
            continue;
          }
        }
        if (!catalog.has(id)) {
          missing.push(entry);
        }
      }

      if (disallowed.length || missing.length || missingVersion.length) {
        return this.jsonError(
          400,
          "extensions_invalid",
          "Extensions not allowed or missing",
          {
            disallowed,
            missing,
            missing_version: missingVersion
          }
        );
      }

      const updated = this.setExtensionPolicy(enabled);
      return Response.json({ ok: true, enabled: updated });
    }

    return this.jsonError(405, "method_not_allowed", "Method not allowed");
  }

  private async handleExtensionCatalogRequest(request: Request) {
    this.ensureStateLoaded();
    const adminCheck = this.checkExtensionAdmin(request);
    if (!adminCheck.ok) {
      return this.jsonError(
        adminCheck.status,
        adminCheck.status === 404 ? "not_found" : "forbidden",
        adminCheck.status === 404 ? "Not found" : "Forbidden"
      );
    }

    if (request.method === "GET") {
      const catalog = await this.loadExtensionCatalog();
      return Response.json({
        extensions: Array.from(catalog.values())
      });
    }

    if (request.method === "POST") {
      let body: unknown = null;
      try {
        body = await request.json();
      } catch {
        return this.jsonError(400, "invalid_json", "Invalid JSON");
      }

      const manifests = parseExtensionCatalog(body);
      if (!manifests.length) {
        return this.jsonError(400, "missing_manifests", "No manifests provided");
      }

      const now = Date.now();
      for (const manifest of manifests) {
        this.sql`
          insert into sky_extensions (extension_id, manifest_json, updated_at)
          values (${manifest.id}, ${JSON.stringify(manifest)}, ${now})
          on conflict(extension_id) do update set
            manifest_json = excluded.manifest_json,
            updated_at = excluded.updated_at
        `;
      }

      this.extensionCatalog = null;

      const catalogKey =
        this.env.AUTOPILOT_EXTENSION_CATALOG_KEY ?? "extensions/catalog.json";
      const catalogKv = this.env.AUTOPILOT_EXTENSION_KV;
      const catalogBucket = this.env.AUTOPILOT_EXTENSION_BUCKET;
      if (catalogKv || catalogBucket) {
        const rows = this.sql<ExtensionCatalogRow>`
          select extension_id, manifest_json, updated_at
          from sky_extensions
          order by updated_at desc
        `;
        const snapshot = rows
          .map((row) => {
            try {
              return parseExtensionManifest(JSON.parse(row.manifest_json));
            } catch {
              return null;
            }
          })
          .filter(Boolean) as ExtensionManifest[];
        const payload = JSON.stringify({ extensions: snapshot });
        if (catalogKv) {
          await catalogKv.put(catalogKey, payload);
        }
        if (catalogBucket) {
          await catalogBucket.put(catalogKey, payload, {
            httpMetadata: { contentType: "application/json" }
          });
        }
      }

      return Response.json({ ok: true, count: manifests.length });
    }

    return this.jsonError(405, "method_not_allowed", "Method not allowed");
  }

  private setToolPolicy(policy: ToolPolicy) {
    this.sql`
      insert into sky_tool_policy (thread_id, policy, updated_at)
      values (${this.name}, ${policy}, ${Date.now()})
      on conflict(thread_id) do update set
        policy = excluded.policy,
        updated_at = excluded.updated_at
    `;
    return policy;
  }

  private async handleToolPolicyRequest(request: Request) {
    this.ensureStateLoaded();
    const adminCheck = this.checkToolAdmin(request);
    if (!adminCheck.ok) {
      return this.jsonError(
        adminCheck.status,
        adminCheck.status === 404 ? "not_found" : "forbidden",
        adminCheck.status === 404 ? "Not found" : "Forbidden"
      );
    }

    if (request.method === "GET") {
      return Response.json({ policy: this.getToolPolicy() });
    }

    if (request.method === "POST") {
      let body: unknown = null;
      try {
        body = await request.json();
      } catch {
        return this.jsonError(400, "invalid_json", "Invalid JSON");
      }

      const record = body as Record<string, unknown>;
      const policyRaw = record.policy;
      if (
        policyRaw === "none" ||
        policyRaw === "read-only" ||
        policyRaw === "read-write"
      ) {
        const updated = this.setToolPolicy(policyRaw);
        return Response.json({ ok: true, policy: updated });
      }

      return this.jsonError(
        400,
        "invalid_policy",
        "Policy must be none, read-only, or read-write"
      );
    }

    return this.jsonError(405, "method_not_allowed", "Method not allowed");
  }

  private loadMessagesForExport(): {
    messages: UIMessage[];
    failures: Array<{ id: string; error: string }>;
  } {
    const rows = this.sql<{ id: string; message: string }>`
      select id, message
      from cf_ai_chat_agent_messages
      order by created_at asc
    `;
    const messages: UIMessage[] = [];
    const failures: Array<{ id: string; error: string }> = [];

    for (const row of rows) {
      try {
        messages.push(JSON.parse(row.message) as UIMessage);
      } catch (error) {
        const message =
          error instanceof Error ? error.message : "parse_error";
        failures.push({ id: row.id, error: message });
      }
    }

    return { messages, failures };
  }

  private handleGetMessagesRequest() {
    const { messages, failures } = this.loadMessagesForExport();
    if (failures.length) {
      return this.jsonError(
        500,
        "message_parse_failed",
        "Failed to parse stored messages",
        { failures }
      );
    }
    return Response.json(messages);
  }

  private exportSkyJsonl() {
    this.ensureStateLoaded();

    const threadId = this.name;
    const now = Date.now();
    const { messages, failures } = this.loadMessagesForExport();
    if (failures.length) {
      return this.jsonError(
        500,
        "message_parse_failed",
        "Failed to parse stored messages",
        { failures }
      );
    }
    const normalizedMessages = messages.map((message) =>
      normalizeMessageForExport(message)
    );

    const runs = this.sql<SkyRunRow>`
      select run_id, thread_id, started_at, completed_at, status, model_config_id, error_code, schema_version
      from sky_runs
      where thread_id = ${threadId}
      order by started_at asc
    `;

    const memoryRows = this.sql<SkyMemoryRow>`
      select thread_id, summary, updated_at, schema_version
      from sky_memory
      where thread_id = ${threadId}
    `;

    const lines: string[] = [];
    lines.push(
      JSON.stringify({
        type: "autopilot.export",
        autopilot_session_version: AUTOPILOT_SESSION_VERSION,
        cf_sky_version: SKY_VERSION,
        schema_versions: {
          sky_run: SKY_RUN_SCHEMA_VERSION,
          sky_event: SKY_EVENT_SCHEMA_VERSION,
          sky_receipt: SKY_RECEIPT_SCHEMA_VERSION,
          sky_memory: SKY_MEMORY_SCHEMA_VERSION
        },
        thread_id: threadId,
        exported_at: now,
        model_registry: MODEL_REGISTRY
      })
    );

    if (memoryRows.length) {
      const memory = memoryRows[0];
      lines.push(
        JSON.stringify({
          type: "memory",
          payload: memory
        })
      );
    }

    for (const message of normalizedMessages) {
      lines.push(
        JSON.stringify({
          type: "message",
          payload: message
        })
      );
    }

    for (const run of runs) {
      lines.push(
        JSON.stringify({
          type: "run",
          payload: run
        })
      );

      const events = this.sql<SkyEventRow>`
        select run_id, event_id, type, payload_json, created_at, schema_version
        from sky_events
        where run_id = ${run.run_id}
        order by event_id asc
      `;

      for (const event of events) {
        lines.push(
          JSON.stringify({
            type: "event",
            payload: {
              run_id: event.run_id,
              event_id: event.event_id,
              type: event.type,
              payload: JSON.parse(event.payload_json),
              created_at: event.created_at,
              schema_version: event.schema_version
            }
          })
        );
      }

      const receipts = this.sql<SkyReceiptRow>`
        select run_id, receipt_json, created_at, schema_version
        from sky_receipts
        where run_id = ${run.run_id}
        order by created_at asc
      `;

      for (const receipt of receipts) {
        lines.push(
          JSON.stringify({
            type: "receipt",
            payload: {
              run_id: receipt.run_id,
              receipt: JSON.parse(receipt.receipt_json),
              created_at: receipt.created_at,
              schema_version: receipt.schema_version
            }
          })
        );
      }
    }

    return new Response(lines.join("\n"), {
      headers: {
        "content-type": "application/jsonl; charset=utf-8"
      }
    });
  }

  override async onRequest(request: Request) {
    const url = new URL(request.url);
    if (url.pathname.endsWith("/export")) {
      return this.exportSkyJsonl();
    }
    if (url.pathname.endsWith("/get-messages")) {
      return this.handleGetMessagesRequest();
    }
    if (url.pathname.endsWith("/tool-policy")) {
      return this.handleToolPolicyRequest(request);
    }
    if (url.pathname.endsWith("/extensions/catalog")) {
      return this.handleExtensionCatalogRequest(request);
    }
    if (url.pathname.endsWith("/extensions")) {
      return this.handleExtensionPolicyRequest(request);
    }
    if (url.pathname.endsWith("/codex-auth")) {
      return this.handleCodexAuthRequest(request);
    }
    return super.onRequest(request);
  }

  private consumeRateLimit() {
    const now = Date.now();
    const rows = this.sql<RateLimitRow>`
      select id, window_start, count
      from autopilot_rate_limit
      where id = ${RATE_LIMIT_ROW_ID}
    `;

    if (!rows.length) {
      this.sql`
        insert into autopilot_rate_limit (id, window_start, count)
        values (${RATE_LIMIT_ROW_ID}, ${now}, 1)
      `;
      return;
    }

    const row = rows[0];
    const windowStart = Number(row.window_start);
    const count = Number(row.count);

    if (Number.isNaN(windowStart) || now - windowStart >= RATE_LIMIT_WINDOW_MS) {
      this.sql`
        insert into autopilot_rate_limit (id, window_start, count)
        values (${RATE_LIMIT_ROW_ID}, ${now}, 1)
        on conflict(id) do update set
          window_start = excluded.window_start,
          count = excluded.count
      `;
      return;
    }

    if (count >= RATE_LIMIT_MAX_MESSAGES) {
      const retryAfterMs = windowStart + RATE_LIMIT_WINDOW_MS - now;
      throw new RateLimitError(retryAfterMs);
    }

    this.sql`
      update autopilot_rate_limit
      set count = ${count + 1}
      where id = ${RATE_LIMIT_ROW_ID}
    `;
  }

  private pruneMessages(keepMessages: UIMessage[]) {
    const keepIds = new Set(keepMessages.map((message) => message.id));
    for (const message of this.messages) {
      if (!keepIds.has(message.id)) {
        this.sql`
          delete from cf_ai_chat_agent_messages
          where id = ${message.id}
        `;
      }
    }
    this.messages = keepMessages;
  }

  private async maybeSummarizeAndTrim(
    workersAi: ReturnType<typeof createWorkersAI>
  ) {
    if (this.messages.length <= SUMMARY_TRIGGER_MESSAGES) return;

    const pruneCount = this.messages.length - MAX_CONTEXT_MESSAGES;
    if (pruneCount <= 0) return;

    const messagesToSummarize = this.messages.slice(0, pruneCount);
    const messagesToKeep = this.messages.slice(pruneCount);
    const model = workersAi(MODEL_ID as Parameters<typeof workersAi>[0]);

    try {
      const result = await generateText({
        model,
        system: buildSummaryPrompt(this.summary),
        messages: await convertToModelMessages(messagesToSummarize),
        maxOutputTokens: SUMMARY_MAX_TOKENS,
        temperature: 0.2
      });

      const nextSummary = result.text.trim();
      if (nextSummary) {
        this.summary = nextSummary;
        this.persistSummary(nextSummary);
      }
    } catch (error) {
      console.warn("[Autopilot] Summary generation failed", error);
    }

    this.pruneMessages(messagesToKeep);
  }

  private createAbortSignal(options?: { abortSignal?: AbortSignal }) {
    if (this.activeAbortController) {
      this.activeAbortController.abort();
    }

    const controller = new AbortController();
    this.activeAbortController = controller;

    return {
      controller,
      signal: combineAbortSignals(controller.signal, options?.abortSignal)
    };
  }

  private async writeStaticMessage(
    writer: UIMessageStreamWriter,
    text: string
  ) {
    const id = generateId();
    writer.write({ type: "text-start", id });
    writer.write({ type: "text-delta", id, delta: text });
    writer.write({ type: "text-end", id });
  }

  override async onChatMessage(
    onFinish: StreamTextOnFinishCallback<ToolSet>,
    options?: { abortSignal?: AbortSignal }
  ) {
    this.ensureStateLoaded();
    const workersai = createWorkersAI({ binding: this.env.AI });
    const startTime = Date.now();
    const skyEnabled = this.isSkyModeEnabled();
    const runId = skyEnabled ? generateId() : "";
    let skyEventId = 0;
    const toolPolicy = this.getToolPolicy();
    const extensions = await this.resolveActiveExtensions();
    const extensionIds = extensions.map(
      (extension) => `${extension.manifest.id}@${extension.manifest.version}`
    );
    const toolState: ToolRunState = {
      calls: 0,
      maxCalls: parseNumberEnv(
        this.env.AUTOPILOT_TOOL_MAX_CALLS,
        DEFAULT_TOOL_MAX_CALLS
      ),
      outboundBytes: 0,
      maxOutboundBytes: parseNumberEnv(
        this.env.AUTOPILOT_TOOL_MAX_OUTBOUND_BYTES,
        DEFAULT_TOOL_MAX_OUTBOUND_BYTES
      )
    };
    let inputHashPromise: Promise<string> | null = null;
    let finishReason: string | null = null;
    let finalText: string | null = null;
    let firstTokenAt: number | null = null;
    let finalized = false;
    const { controller, signal } = this.createAbortSignal(options);

    const emitSkyEvent = (type: string, payload: unknown) => {
      if (!skyEnabled) return;
      skyEventId += 1;
      this.insertSkyEvent({
        runId,
        eventId: skyEventId,
        type,
        payload,
        createdAt: Date.now()
      });
    };

    if (skyEnabled) {
      this.insertSkyRun(runId, startTime);
      emitSkyEvent("run.started", {
        thread_id: this.name,
        model_config_id: MODEL_CONFIG_ID,
        started_at: startTime,
        schema_version: SKY_RUN_SCHEMA_VERSION
      });
    }

    await this.runExtensionHooks("onRunStart", extensions, {
      thread_id: this.name,
      run_id: runId
    });

    const latestMessage = this.messages[this.messages.length - 1];
    if (latestMessage?.role === "user") {
      await this.runExtensionHooks("onMessage", extensions, {
        thread_id: this.name,
        run_id: runId,
        message: latestMessage
      });
    }

    const { tools, activeTools } = this.buildToolRegistry({
      policy: toolPolicy,
      runId,
      emitSkyEvent,
      toolState,
      workersai,
      extensions
    });
    const toolChoice = tools ? parseToolChoice(this.env.AUTOPILOT_TOOL_CHOICE) : undefined;

    const finalize = (params: {
      ok: boolean;
      error?: string;
      finishReason?: string | null;
    }) => {
      if (finalized) return;
      finalized = true;
      if (this.activeAbortController === controller) {
        this.activeAbortController = null;
      }
      const durationMs = Date.now() - startTime;
      const ttftMs = firstTokenAt ? firstTokenAt - startTime : null;
      const metrics: ChatMetricLog = {
        event: "autopilot_chat_metrics",
        agent_name: this.name,
        model_id: MODEL_ID,
        message_count: this.messages.length,
        ttft_ms: ttftMs,
        duration_ms: durationMs,
        ok: params.ok,
        finish_reason: params.finishReason ?? null,
        ...(params.error ? { error: params.error } : {})
      };
      this.logChatMetrics(metrics);

      const status = params.ok
        ? params.finishReason === "cancelled"
          ? "cancelled"
          : "completed"
        : "error";
      void this.runExtensionHooks("onRunComplete", extensions, {
        thread_id: this.name,
        run_id: runId,
        status,
        finish_reason: params.finishReason ?? null
      });

      if (skyEnabled) {
        const completedAt = Date.now();
        emitSkyEvent("run.completed", {
          status,
          finish_reason: params.finishReason ?? null,
          duration_ms: durationMs
        });
        void this.finalizeSkyRun({
          runId,
          status,
          startedAt: startTime,
          completedAt,
          finishReason: params.finishReason ?? null,
          errorCode: params.error ?? null,
          inputHashPromise,
          outputText: finalText
        });
      }
    };

    const handleChunk: StreamTextOnChunkCallback<ToolSet> = ({ chunk }) => {
      if (
        !firstTokenAt &&
        (chunk.type === "text-delta" ||
          chunk.type === "reasoning-delta" ||
          chunk.type === "raw")
      ) {
        firstTokenAt = Date.now();
      }

      if (chunk.type === "text-delta" || chunk.type === "reasoning-delta") {
        const delta =
          "delta" in chunk && typeof chunk.delta === "string"
            ? chunk.delta
            : "text" in chunk && typeof chunk.text === "string"
              ? chunk.text
              : "";
        emitSkyEvent("model.delta", {
          kind: chunk.type,
          delta
        });
      }
    };

    const handleError: StreamTextOnErrorCallback = ({ error }) => {
      if (signal?.aborted) {
        finishReason = "cancelled";
        finalize({ ok: true, finishReason });
        return;
      }
      const message =
        error instanceof Error ? error.message : "StreamText error";
      emitSkyEvent("run.error", { error: message });
      finishReason = "error";
      finalize({ ok: false, error: message, finishReason });
    };

    const handleFinish: StreamTextOnFinishCallback<ToolSet> = (event) => {
      finishReason = event.finishReason ?? null;
      finalText = event.text ?? null;
      emitSkyEvent("model.completed", {
        finish_reason: finishReason,
        text_length: finalText?.length ?? 0
      });
      finalize({ ok: true, finishReason });
      return onFinish(event);
    };

    try {
      this.consumeRateLimit();
    } catch (error) {
      const message =
        error instanceof RateLimitError
          ? error.message
          : "Rate limit exceeded. Please try again shortly.";
      finalText = message;
      finishReason = "rate_limited";
      emitSkyEvent("run.error", { error: "rate_limited" });
      inputHashPromise = skyEnabled
        ? hashJson({
            summary: this.summary,
            messages: this.messages.slice(-MAX_CONTEXT_MESSAGES),
            model_config_id: MODEL_CONFIG_ID,
            extensions: extensionIds
          })
        : null;
      const stream = createUIMessageStream({
        execute: async ({ writer }) => {
          firstTokenAt ??= Date.now();
          await this.writeStaticMessage(writer, message);
          finalize({
            ok: false,
            error: "rate_limited",
            finishReason: "rate_limited"
          });
        }
      });
      return createUIMessageStreamResponse({ stream });
    }

    await this.maybeSummarizeAndTrim(workersai);
    const recentMessages = this.messages.slice(-MAX_CONTEXT_MESSAGES);
    const systemPrompt = buildSystemPrompt(this.summary) +
      this.buildExtensionSystemPrompt(extensions);
    inputHashPromise = skyEnabled
      ? hashJson({
          summary: this.summary,
          messages: recentMessages,
          model_config_id: MODEL_CONFIG_ID,
          extensions: extensionIds
        })
      : null;

    const stream = createUIMessageStream({
      execute: async ({ writer }) => {
        let result;
        try {
          const streamOptions = {
            system: systemPrompt,
            messages: await convertToModelMessages(recentMessages),
            model: workersai(MODEL_ID as Parameters<typeof workersai>[0]),
            maxOutputTokens: MAX_OUTPUT_TOKENS,
            stopWhen: stepCountIs(10),
            onChunk: handleChunk,
            onError: handleError,
            onFinish: handleFinish,
            ...(signal ? { abortSignal: signal } : {})
          };

          result = streamText({
            ...streamOptions,
            ...(tools
              ? {
                  tools,
                  activeTools,
                  ...(toolChoice ? { toolChoice } : {})
                }
              : {})
          });
        } catch (error) {
          const message =
            error instanceof Error ? error.message : "StreamText error";
          try {
            const fallback = await generateText({
              model: workersai(MODEL_ID as Parameters<typeof workersai>[0]),
              system: systemPrompt,
              messages: await convertToModelMessages(recentMessages),
              maxOutputTokens: MAX_OUTPUT_TOKENS
            });
            firstTokenAt ??= Date.now();
            await this.writeStaticMessage(writer, fallback.text);
            finalText = fallback.text;
            finishReason = "fallback";
            emitSkyEvent("model.completed", {
              finish_reason: finishReason,
              fallback: true,
              text_length: finalText.length
            });
            finalize({ ok: true, error: message, finishReason });
          } catch (fallbackError) {
            console.error("[Autopilot] Fallback generation failed", fallbackError);
            firstTokenAt ??= Date.now();
            finalText = "Autopilot hit an error. Please try again.";
            finishReason = "error";
            await this.writeStaticMessage(writer, finalText);
            emitSkyEvent("run.error", { error: message });
            finalize({ ok: false, error: message, finishReason });
          }
          return;
        }

        writer.merge(result.toUIMessageStream());
      }
    });

    return createUIMessageStreamResponse({ stream });
  }
}

const CodexAuthorizeSchema = Schema.Struct({
  method: Schema.Number
});

const CodexCallbackSchema = Schema.Struct({
  method: Schema.Number,
  code: Schema.optional(Schema.String)
});

const decodeJsonBody = <A>(
  request: Request,
  schema: Schema.Schema<A>
) =>
  Effect.tryPromise({
    try: () => request.json(),
    catch: (cause) => new SandboxEffectError({ message: "Invalid JSON", cause })
  }).pipe(
    Effect.flatMap((body) => Schema.decodeUnknown(schema)(body)),
    Effect.mapError((cause) =>
      cause instanceof SandboxEffectError
        ? cause
        : new SandboxEffectError({
            message: "Invalid request body",
            cause
          })
    )
  );

const handleOpencodeOauthRequest = (request: Request, env: Env) => {
  const url = new URL(request.url);
  const match = url.pathname.match(
    /^\/api\/sandbox\/([^/]+)\/opencode\/provider\/openai\/oauth\/(authorize|callback)$/
  );
  if (!match) {
    return Effect.succeed(null);
  }

  const corsHeaders = buildCorsHeaders(request, env);
  if (request.method === "OPTIONS") {
    return Effect.succeed(
      new Response(null, { status: 204, headers: corsHeaders ?? {} })
    );
  }

  if (request.method !== "POST") {
    return Effect.succeed(
      new Response("Method not allowed", {
        status: 405,
        headers: corsHeaders ?? {}
      })
    );
  }

  const threadId = match[1];
  const action = match[2] as "authorize" | "callback";

  return Effect.gen(function* () {
    if (!env.Sandbox) {
      return new Response("Sandbox binding is not configured.", {
        status: 501,
        headers: corsHeaders ?? {}
      });
    }

    const secret = getCodexSecret(env);
    if (!secret) {
      return new Response("Codex secret is not configured.", {
        status: 501,
        headers: corsHeaders ?? {}
      });
    }

    const token = extractSandboxToken(request);
    if (!token) {
      return new Response("Unauthorized", {
        status: 401,
        headers: corsHeaders ?? {}
      });
    }

    const claims = yield* sandboxEffect("Sandbox token invalid", () =>
      parseSandboxSessionToken(secret, token, threadId)
    );
    if (!claims) {
      return new Response("Unauthorized", {
        status: 401,
        headers: corsHeaders ?? {}
      });
    }

    const bodyRequest = request.clone();
    const body = yield* decodeJsonBody(
      bodyRequest,
      action === "authorize" ? CodexAuthorizeSchema : CodexCallbackSchema
    );

    const sandbox = getThreadSandbox(env, threadId);
    yield* sandboxEffect("Sandbox init failed", () =>
      ensureOpencodeSandbox(sandbox)
    );
    yield* sandboxEffect("Codex auth hydrate failed", () =>
      hydrateOpencodeAuth(env, threadId, sandbox)
    );
    const server = yield* sandboxEffect("OpenCode server start failed", () =>
      createOpencodeServer(sandbox, { directory: SANDBOX_WORKSPACE_ROOT })
    );

    const timeoutMs = parseNumberEnv(
      env.AUTOPILOT_CODEX_CALLBACK_TIMEOUT_MS,
      DEFAULT_CODEX_CALLBACK_TIMEOUT_MS
    );
    const shouldTimeout = action === "callback";
    let timeoutId: ReturnType<typeof setTimeout> | null = null;

    const opencodeUrl = new URL(request.url);
    opencodeUrl.pathname = `/provider/openai/oauth/${action}`;
    opencodeUrl.search = "";

    let opencodeResponse: Response;
    try {
      const opencodeBaseRequest = new Request(opencodeUrl.toString(), request);
      const headers = new Headers(opencodeBaseRequest.headers);
      headers.set("content-type", "application/json");
      headers.delete("authorization");
      headers.delete("x-autopilot-sandbox-token");

    const opencodeRequest = new Request(opencodeBaseRequest, { headers });
      opencodeResponse = yield* sandboxEffect(
        "OpenCode OAuth proxy failed",
        async () => {
          try {
            const fetchPromise = sandbox.containerFetch(
              opencodeRequest,
              server.port
            );
            if (!shouldTimeout) {
              return await fetchPromise;
            }
            return await new Promise<Response>((resolve, reject) => {
              timeoutId = setTimeout(() => {
                reject(new Error("OAuth callback timed out"));
              }, timeoutMs);
              fetchPromise.then(resolve, reject);
            });
          } catch (error) {
            console.error("[Autopilot] OpenCode OAuth proxy failed", error);
            throw error;
          }
        }
      );
    } finally {
      if (timeoutId) clearTimeout(timeoutId);
    }

    const responseText = yield* sandboxEffect(
      "OpenCode OAuth response read failed",
      () => opencodeResponse.text()
    );

    if (action === "callback" && opencodeResponse.ok) {
      const authPayload = yield* sandboxEffect(
        "OpenCode auth read failed",
        () => readOpencodeAuth(sandbox)
      );
      if (authPayload) {
        yield* sandboxEffect("Codex auth persist failed", () =>
          storeCodexAuthPayload(env, threadId, authPayload)
        );
      }
    }

    const headers = new Headers({
      "content-type":
        opencodeResponse.headers.get("content-type") ?? "application/json"
    });
    if (corsHeaders) {
      for (const [key, value] of Object.entries(corsHeaders)) {
        headers.set(key, value);
      }
    }

    return new Response(responseText, {
      status: opencodeResponse.status,
      headers
    });
  }).pipe(
    Effect.catchAll((error) => {
      let message = error instanceof Error ? error.message : "Codex proxy failed";
      const status =
        error instanceof SandboxEffectError &&
        (error.message === "Invalid JSON" ||
          error.message === "Invalid request body")
          ? 400
          : 500;
      let resolvedStatus = status;
      if (
        error instanceof SandboxEffectError &&
        error.cause instanceof Error &&
        error.cause.message === "OAuth callback timed out"
      ) {
        message = "Codex callback timed out.";
        resolvedStatus = 504;
      }
      const headers = new Headers({ "content-type": "application/json" });
      if (corsHeaders) {
        for (const [key, value] of Object.entries(corsHeaders)) {
          headers.set(key, value);
        }
      }
      return Effect.succeed(
        new Response(JSON.stringify({ ok: false, error: message }), {
          status: resolvedStatus,
          headers
        })
      );
    })
  );
};

const handleOpencodeRequest = (request: Request, env: Env) =>
  Effect.gen(function* () {
    const url = new URL(request.url);
    if (!url.pathname.startsWith(OPENCODE_ROUTE_PREFIX)) {
      return null;
    }
    const threadId = url.searchParams.get("thread");
    if (!threadId) {
      return new Response("Missing thread id", { status: 400 });
    }
    if (!env.Sandbox) {
      return new Response("Sandbox binding is not configured.", { status: 501 });
    }

    const sandbox = getThreadSandbox(env, threadId);
    yield* sandboxEffect("Sandbox init failed", () => ensureOpencodeSandbox(sandbox));
    yield* sandboxEffect("Codex auth hydrate failed", () =>
      hydrateOpencodeAuth(env, threadId, sandbox)
    );
    const server = yield* sandboxEffect("OpenCode server start failed", () =>
      createOpencodeServer(sandbox, { directory: SANDBOX_WORKSPACE_ROOT })
    );
    const proxyRequest = buildOpencodeProxyRequest(request, url);
    const response = yield* sandboxEffect("OpenCode proxy failed", () =>
      Promise.resolve(proxyToOpencode(proxyRequest, sandbox, server))
    );
    return response;
  });

const handleRequest = (request: Request, env: Env) =>
  Effect.gen(function* () {
    const oauthResponse = yield* handleOpencodeOauthRequest(request, env);
    if (oauthResponse) {
      return oauthResponse;
    }
    const opencodeResponse = yield* handleOpencodeRequest(request, env);
    if (opencodeResponse) {
      return opencodeResponse;
    }
    const response = yield* Effect.tryPromise({
      try: () => routeAgentRequest(request, env),
      catch: (cause) =>
        new AutopilotEffectError({ message: "Route handler failed", cause })
    });
    return response ?? new Response("Not found", { status: 404 });
  });

export default EffectWorkers.serve<Env>((request, env) =>
  handleRequest(request, env)
);
