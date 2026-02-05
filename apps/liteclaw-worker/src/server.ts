import { routeAgentRequest } from "agents";

import { AIChatAgent } from "@cloudflare/ai-chat";
import {
  convertToModelMessages,
  createUIMessageStream,
  createUIMessageStreamResponse,
  generateId,
  generateText,
  jsonSchema,
  stepCountIs,
  streamText,
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

const MODEL_ID = "@cf/meta/llama-3.1-8b-instruct";
const MODEL_CONFIG_ID = "workers-ai:llama-3.1-8b-instruct";
const MAX_CONTEXT_MESSAGES = 25;
const SUMMARY_TRIGGER_MESSAGES = 35;
const SUMMARY_MAX_TOKENS = 256;
const MAX_OUTPUT_TOKENS = 512;
const RATE_LIMIT_WINDOW_MS = 60_000;
const RATE_LIMIT_MAX_MESSAGES = 20;
const LEGACY_STATE_ROW_ID = "liteclaw_state";
const SKY_MEMORY_SCHEMA_VERSION = 1;
const SKY_EVENT_SCHEMA_VERSION = 1;
const SKY_RUN_SCHEMA_VERSION = 1;
const SKY_RECEIPT_SCHEMA_VERSION = 1;
const LITECLAW_SESSION_VERSION = 1;
const SKY_VERSION = "0.1.0";
const RATE_LIMIT_ROW_ID = "liteclaw_rate_limit";
const DEFAULT_TOOL_POLICY: ToolPolicy = "none";
const DEFAULT_TOOL_MAX_CALLS = 4;
const DEFAULT_TOOL_MAX_OUTBOUND_BYTES = 200_000;
const DEFAULT_HTTP_TIMEOUT_MS = 8_000;
const DEFAULT_HTTP_MAX_BYTES = 50_000;
const DEFAULT_TUNNEL_TIMEOUT_MS = 8_000;

const SYSTEM_PROMPT =
  "You are LiteClaw, a persistent personal AI agent. " +
  "Be concise, helpful, and remember the ongoing conversation.";
const SUMMARY_PROMPT = [
  "You update LiteClaw's memory summary.",
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
  event: "liteclaw_chat_metrics";
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
  event: "liteclaw_extension_metrics";
  extension_id: string;
  extension_version: string;
  hook: ExtensionHookName;
  duration_ms: number;
  ok: boolean;
  error?: string;
};

type ExtensionToolMetricLog = {
  event: "liteclaw_extension_tool_metrics";
  extension_id: string;
  extension_version: string;
  tool_name: string;
  duration_ms: number;
  ok: boolean;
  error?: string;
};

type ToolPolicy = "none" | "read-only" | "read-write";
type ExecutorKind = "workers" | "container" | "tunnel";

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
}) => ReturnType<typeof tool>;

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

type LegacyLiteClawStateRow = {
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
    description:
      typeof record.description === "string" ? record.description : undefined,
    tools: Array.isArray(record.tools)
      ? record.tools.filter((tool) => typeof tool === "string")
      : undefined,
    system_prompt:
      typeof record.system_prompt === "string"
        ? record.system_prompt
        : undefined,
    permissions:
      record.permissions && typeof record.permissions === "object"
        ? (record.permissions as Record<string, unknown>)
        : undefined,
    ui:
      record.ui && typeof record.ui === "object"
        ? (record.ui as Record<string, unknown>)
        : undefined
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
      create table if not exists liteclaw_rate_limit (
        id text primary key,
        window_start integer not null,
        count integer not null
      )
    `;
    this.sql`
      create table if not exists liteclaw_state (
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

    const memoryRows = this.sql<SkyMemoryRow>`
      select thread_id, summary, updated_at, schema_version
      from sky_memory
      where thread_id = ${this.name}
    `;

    if (!memoryRows.length) {
      const legacyRows = this.sql<LegacyLiteClawStateRow>`
        select id, schema_version, summary, updated_at
        from liteclaw_state
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
    return this.env.LITECLAW_SKY_MODE === "1";
  }

  private getToolPolicy(): ToolPolicy {
    const defaultPolicy = parseToolPolicy(this.env.LITECLAW_TOOL_POLICY);
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
      new Set(enabled.map((entry) => entry.trim()).filter(Boolean))
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
    const defaults = parseExtensionList(this.env.LITECLAW_EXTENSION_DEFAULTS);
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

    return enabled;
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
        console.warn("[LiteClaw] Failed to parse extension manifest", error);
      }
    }

    const catalogJson = this.env.LITECLAW_EXTENSION_CATALOG_JSON;
    const catalogUrl = this.env.LITECLAW_EXTENSION_CATALOG_URL;
    const catalogKey =
      this.env.LITECLAW_EXTENSION_CATALOG_KEY ?? "extensions/catalog.json";
    const catalogKv = this.env.LITECLAW_EXTENSION_KV;
    const catalogBucket = this.env.LITECLAW_EXTENSION_BUCKET;

    let externalCatalog: unknown | null = null;

    if (catalogJson) {
      try {
        externalCatalog = JSON.parse(catalogJson);
      } catch (error) {
        console.warn("[LiteClaw] Failed to parse extension catalog JSON", error);
      }
    } else if (catalogKv) {
      try {
        const value = await catalogKv.get(catalogKey);
        if (value) {
          externalCatalog = JSON.parse(value);
        }
      } catch (error) {
        console.warn("[LiteClaw] Failed to load extension catalog from KV", error);
      }
    } else if (catalogBucket) {
      try {
        const object = await catalogBucket.get(catalogKey);
        if (object) {
          const text = await object.text();
          externalCatalog = JSON.parse(text);
        }
      } catch (error) {
        console.warn("[LiteClaw] Failed to load extension catalog from R2", error);
      }
    } else if (catalogUrl) {
      try {
        const response = await fetch(catalogUrl);
        externalCatalog = await response.json();
      } catch (error) {
        console.warn("[LiteClaw] Failed to fetch extension catalog URL", error);
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
        console.warn("[LiteClaw] Failed to load extension catalog", error);
      }
    }

    this.extensionCatalog = catalog;
    return catalog;
  }

  private async resolveActiveExtensions(): Promise<ExtensionRuntime[]> {
    const enabledRefs = this.getExtensionPolicy();
    const allowlist = parseExtensionList(this.env.LITECLAW_EXTENSION_ALLOWLIST);
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
      const runtime = buildBuiltinExtensionRuntime(manifest);
      if (runtime) {
        runtimes.push(runtime);
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
          event: "liteclaw_extension_metrics",
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
          event: "liteclaw_extension_metrics",
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
    return parseExecutorKind(this.env.LITECLAW_EXECUTOR_KIND);
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

    const allowlist = parseAllowlist(this.env.LITECLAW_HTTP_ALLOWLIST);
    const httpTimeoutMs = parseNumberEnv(
      this.env.LITECLAW_HTTP_TIMEOUT_MS,
      DEFAULT_HTTP_TIMEOUT_MS
    );
    const httpMaxBytes = parseNumberEnv(
      this.env.LITECLAW_HTTP_MAX_BYTES,
      DEFAULT_HTTP_MAX_BYTES
    );
    const executorKind = this.getExecutorKind();
    const tunnelUrl = this.env.LITECLAW_TUNNEL_URL;
    const tunnelToken = this.env.LITECLAW_TUNNEL_TOKEN;
    const tunnelTimeoutMs = parseNumberEnv(
      this.env.LITECLAW_TUNNEL_TIMEOUT_MS,
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

    const invokeTunnelTool = async <Output>(
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
        const response = await fetch(endpoint.toString(), {
          method: "POST",
          headers: {
            "content-type": "application/json",
            authorization: `Bearer ${tunnelToken}`
          },
          body: JSON.stringify(payload),
          signal
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
            event: "liteclaw_extension_tool_metrics",
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
            event: "liteclaw_extension_tool_metrics",
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
      tool({
        description,
        inputSchema: jsonSchema(inputSchema),
        execute: async (input, toolOptions) =>
          executeToolWithLogging(
            name,
            mode,
            input,
            toolOptions,
            () => handler(input, toolOptions)
          )
      });

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
                  headers: input.headers,
                  body: input.body,
                  signal
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
                model: options.workersai(MODEL_ID),
                system: SUMMARY_PROMPT,
                prompt: input.text,
                maxTokens: parseNumberEnv(
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
                model: options.workersai(MODEL_ID),
                system:
                  "Extract structured JSON from the text. Respond with JSON only." +
                  (input.instructions ? `\n\nInstructions: ${input.instructions}` : ""),
                prompt: input.text,
                maxTokens: SUMMARY_MAX_TOKENS
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
        description: "Read a file from the LiteClaw workspace.",
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
            async () => {
              if (executorKind === "tunnel") {
                return invokeTunnelTool("workspace.read", input, toolOptions);
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
          "Write a file to the LiteClaw workspace (overwrites existing content).",
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
            async () => {
              if (executorKind === "tunnel") {
                return invokeTunnelTool("workspace.write", input, toolOptions);
              }
              assertWorkersExecutor();
              const result = this.writeWorkspaceFile(input.path, input.content);
              const beforeBytes = result.before?.length ?? 0;
              const afterBytes = result.after.length;
              const beforeHash = result.before
                ? await hashText(result.before)
                : null;
              const afterHash = await hashText(result.after);
              const patch = {
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
            async () => {
              if (executorKind === "tunnel") {
                return invokeTunnelTool("workspace.edit", input, toolOptions);
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
              const patch = {
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
              `[LiteClaw] Extension ${extension.manifest.id} attempted to register undeclared tool ${toolName}`
            );
            continue;
          }
          if (tools[toolName]) {
            console.warn(
              `[LiteClaw] Extension ${extension.manifest.id} attempted to override tool ${toolName}`
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
    const secret = this.env.LITECLAW_EXTENSION_ADMIN_SECRET;
    if (!secret) {
      return { ok: false, status: 404 };
    }

    const provided =
      request.headers.get("x-liteclaw-admin-secret") ??
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

  private async handleExtensionPolicyRequest(request: Request) {
    this.ensureStateLoaded();
    const adminCheck = this.checkExtensionAdmin(request);
    if (!adminCheck.ok) {
      return new Response(
        adminCheck.status === 404 ? "Not found" : "Forbidden",
        { status: adminCheck.status }
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
        return new Response("Invalid JSON", { status: 400 });
      }

      const record = body as Record<string, unknown>;
      const enabled = Array.isArray(record.enabled)
        ? record.enabled.filter((entry) => typeof entry === "string")
        : null;

      if (!enabled) {
        return new Response("Missing enabled array", { status: 400 });
      }

      const allowlist = parseExtensionList(this.env.LITECLAW_EXTENSION_ALLOWLIST);
      if (!allowlist.length) {
        return new Response("Extension allowlist is empty", { status: 400 });
      }

      const allowAll = allowlist.includes("*");
      const catalog = await this.loadExtensionCatalog();
      const disallowed: string[] = [];
      const missing: string[] = [];

      for (const entry of enabled) {
        if (entry === "*") {
          if (!allowAll) {
            disallowed.push(entry);
          }
          continue;
        }

        const { id, version } = parseExtensionRef(entry);
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

      if (disallowed.length || missing.length) {
        return Response.json(
          {
            ok: false,
            error: "Extensions not allowed or missing",
            disallowed,
            missing
          },
          { status: 400 }
        );
      }

      const updated = this.setExtensionPolicy(enabled);
      return Response.json({ ok: true, enabled: updated });
    }

    return new Response("Method not allowed", { status: 405 });
  }

  private async handleExtensionCatalogRequest(request: Request) {
    this.ensureStateLoaded();
    const adminCheck = this.checkExtensionAdmin(request);
    if (!adminCheck.ok) {
      return new Response(
        adminCheck.status === 404 ? "Not found" : "Forbidden",
        { status: adminCheck.status }
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
        return new Response("Invalid JSON", { status: 400 });
      }

      const manifests = parseExtensionCatalog(body);
      if (!manifests.length) {
        return new Response("No manifests provided", { status: 400 });
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

      return Response.json({ ok: true, count: manifests.length });
    }

    return new Response("Method not allowed", { status: 405 });
  }

  private exportSkyJsonl() {
    this.ensureStateLoaded();

    const threadId = this.name;
    const now = Date.now();
    const messageRows = this.sql<{ message: string }>`
      select message
      from cf_ai_chat_agent_messages
      order by created_at asc
    `;
    const messages = messageRows
      .map((row) => JSON.parse(row.message) as UIMessage)
      .map((message) => normalizeMessageForExport(message));

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
        type: "liteclaw.export",
        liteclaw_session_version: LITECLAW_SESSION_VERSION,
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

    for (const message of messages) {
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

  async onRequest(request: Request) {
    const url = new URL(request.url);
    if (url.pathname.endsWith("/export")) {
      return this.exportSkyJsonl();
    }
    if (url.pathname.endsWith("/extensions/catalog")) {
      return this.handleExtensionCatalogRequest(request);
    }
    if (url.pathname.endsWith("/extensions")) {
      return this.handleExtensionPolicyRequest(request);
    }
    return super.onRequest(request);
  }

  private consumeRateLimit() {
    const now = Date.now();
    const rows = this.sql<RateLimitRow>`
      select id, window_start, count
      from liteclaw_rate_limit
      where id = ${RATE_LIMIT_ROW_ID}
    `;

    if (!rows.length) {
      this.sql`
        insert into liteclaw_rate_limit (id, window_start, count)
        values (${RATE_LIMIT_ROW_ID}, ${now}, 1)
      `;
      return;
    }

    const row = rows[0];
    const windowStart = Number(row.window_start);
    const count = Number(row.count);

    if (Number.isNaN(windowStart) || now - windowStart >= RATE_LIMIT_WINDOW_MS) {
      this.sql`
        insert into liteclaw_rate_limit (id, window_start, count)
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
      update liteclaw_rate_limit
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
    const model = workersAi(MODEL_ID);

    try {
      const result = await generateText({
        model,
        system: buildSummaryPrompt(this.summary),
        messages: await convertToModelMessages(messagesToSummarize),
        maxTokens: SUMMARY_MAX_TOKENS,
        temperature: 0.2
      });

      const nextSummary = result.text.trim();
      if (nextSummary) {
        this.summary = nextSummary;
        this.persistSummary(nextSummary);
      }
    } catch (error) {
      console.warn("[LiteClaw] Summary generation failed", error);
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

  async onChatMessage(
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
        this.env.LITECLAW_TOOL_MAX_CALLS,
        DEFAULT_TOOL_MAX_CALLS
      ),
      outboundBytes: 0,
      maxOutboundBytes: parseNumberEnv(
        this.env.LITECLAW_TOOL_MAX_OUTBOUND_BYTES,
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
      this.logChatMetrics({
        event: "liteclaw_chat_metrics",
        agent_name: this.name,
        model_id: MODEL_ID,
        message_count: this.messages.length,
        ttft_ms: ttftMs,
        duration_ms: durationMs,
        ok: params.ok,
        error: params.error,
        finish_reason: params.finishReason ?? null
      });

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
        emitSkyEvent("model.delta", {
          kind: chunk.type,
          delta: chunk.delta
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
            model: workersai(MODEL_ID),
            maxTokens: MAX_OUTPUT_TOKENS,
            stopWhen: stepCountIs(10),
            onChunk: handleChunk,
            onError: handleError,
            onFinish: handleFinish,
            abortSignal: signal
          };

          result = streamText({
            ...streamOptions,
            ...(tools ? { tools, activeTools } : {})
          });
        } catch (error) {
          const message =
            error instanceof Error ? error.message : "StreamText error";
          try {
            const fallback = await generateText({
              model: workersai(MODEL_ID),
              system: systemPrompt,
              messages: await convertToModelMessages(recentMessages),
              maxTokens: MAX_OUTPUT_TOKENS
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
            console.error("[LiteClaw] Fallback generation failed", fallbackError);
            firstTokenAt ??= Date.now();
            finalText = "LiteClaw hit an error. Please try again.";
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

export default {
  async fetch(request: Request, env: Env) {
    return (
      (await routeAgentRequest(request, env)) ||
      new Response("Not found", { status: 404 })
    );
  }
} satisfies ExportedHandler<Env>;
