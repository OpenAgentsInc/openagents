import { Agent, routeAgentRequest, type AgentContext, type Connection, type WSMessage } from "agents";
import * as AiLanguageModel from "@effect/ai/LanguageModel";
import * as AiPrompt from "@effect/ai/Prompt";
import * as AiResponse from "@effect/ai/Response";
import * as AiTokenizer from "@effect/ai/Tokenizer";
import * as AiToolkit from "@effect/ai/Toolkit";
import { Effect, Layer, Schema, Stream } from "effect";
import { BlobStore, CompiledArtifact, Lm, Predict } from "@openagentsinc/dse";
import {
  AutopilotBootstrapState,
  AutopilotBlueprintStateV1,
  AutopilotBlueprintV1,
  BLUEPRINT_FORMAT,
  BLUEPRINT_FORMAT_VERSION,
  BlueprintDocs,
  BootstrapTemplate,
  CURRENT_BOOTSTRAP_TEMPLATE_VERSION,
  DEFAULT_BOOTSTRAP_TEMPLATE_BODY,
  DocVersion,
  HeartbeatDoc,
  CharacterDoc,
  IdentityDoc,
  MemoryEntry,
  MemoryEntryId,
  ToolsDoc,
  ThreadId,
  UserDoc,
  UserId,
  makeDefaultBlueprintState,
  renderBlueprintContext,
  renderBootstrapInstructions
} from "./blueprint";
import {
  BASE_TOOL_NAMES,
  renderToolPrompt,
  toolContracts,
  toolContractsExport
} from "./tools";
import {
  signatures,
  moduleContractsExport,
  signatureContractsExport
} from "./dseCatalog";
import {
  initDseTables,
  ensureDefaultArtifacts,
  layerDseFromSql,
  listReceipts,
  rollbackActiveArtifact,
  type SqlTag
} from "./dseServices";

import { MessageType, type ChatMessage, type ChatPart, type ChatToolPart } from "./chatProtocol";
import { makeWorkersAiLanguageModel } from "./effect/ai/languageModel";
import { initAiReceiptTables, listAiModelReceipts, recordAiModelReceipt, type AiModelReceiptV1, type UsageEncoded } from "./effect/ai/receipts";
import { encodeWirePart } from "./effect/ai/streaming";
import { aiToolFromContract } from "./effect/ai/toolkit";
import { sha256IdFromString } from "./hash";

const MODEL_ID = "@cf/openai/gpt-oss-120b";
const MAX_CONTEXT_MESSAGES = 25;
const MAX_PROMPT_TOKENS = 8_000;
const MAX_SYSTEM_PROMPT_CHARS = 20_000;
const MAX_OUTPUT_TOKENS = 512;

const FIRST_OPEN_WELCOME_MESSAGE =
  "Autopilot online.\n\n" +
  "Greetings, user. What shall I call you?";

const AUTOPILOT_TOOLKIT = AiToolkit.make(
  aiToolFromContract(toolContracts.get_time),
  aiToolFromContract(toolContracts.echo),
  aiToolFromContract(toolContracts.bootstrap_set_user_handle),
  aiToolFromContract(toolContracts.bootstrap_set_agent_name),
  aiToolFromContract(toolContracts.bootstrap_set_agent_vibe),
  aiToolFromContract(toolContracts.identity_update),
  aiToolFromContract(toolContracts.user_update),
  aiToolFromContract(toolContracts.character_update),
  aiToolFromContract(toolContracts.tools_update_notes),
  aiToolFromContract(toolContracts.heartbeat_set_checklist),
  aiToolFromContract(toolContracts.memory_append),
  aiToolFromContract(toolContracts.bootstrap_complete),
  aiToolFromContract(toolContracts.blueprint_export),
);

const encodeStreamPart = Schema.encodeSync(AiResponse.StreamPart(AUTOPILOT_TOOLKIT));

const TOKENIZER_LAYER = Layer.succeed(
  AiTokenizer.Tokenizer,
  AiTokenizer.make({
    tokenize: (prompt) =>
      Effect.sync(() => {
        const textForPart = (part: any): string => {
          if (!part || typeof part !== "object") return "";
          switch (part.type) {
            case "text":
            case "reasoning":
              return typeof part.text === "string" ? part.text : "";
            case "tool-call":
              try {
                return JSON.stringify(part.params ?? {});
              } catch {
                return "";
              }
            case "tool-result":
              try {
                return JSON.stringify(part.result ?? part.encodedResult ?? null);
              } catch {
                return "";
              }
            case "file":
              return typeof part.fileName === "string" ? part.fileName : "";
            default:
              return "";
          }
        };

        const messageText = (msg: any): string => {
          if (!msg || typeof msg !== "object") return "";
          if (typeof msg.content === "string") return msg.content;
          if (Array.isArray(msg.content)) return msg.content.map(textForPart).join("\n");
          return "";
        };

        const all = prompt.content.map(messageText).join("\n\n");
        // This is an intentionally naive token estimate to enforce a hard cap.
        // v2 can swap in a model-accurate tokenizer without changing call sites.
        const estimate = Math.ceil(all.length / 4);
        const n = Math.max(0, Math.min(estimate, 200_000));
        return Array.from({ length: n }, (_, i) => i);
      }),
  }),
);

const capText = (text: string, maxChars: number): string => {
  const limit = Math.max(0, Math.floor(maxChars));
  if (text.length <= limit) return text;
  return text.slice(0, limit) + "\n\n[truncated for prompt budget]"
};

const budgetChatPrompt = (raw: AiPrompt.RawInput) =>
  Effect.gen(function* () {
    const tokenizer = yield* AiTokenizer.Tokenizer;
    const prompt = AiPrompt.make(raw);
    const messages = prompt.content;
    const first = messages[0];

    if (!first || first.role !== "system") {
      return yield* tokenizer.truncate(prompt, MAX_PROMPT_TOKENS);
    }

    const systemOnly = AiPrompt.fromMessages([first]);
    const systemTokens = (yield* tokenizer.tokenize(systemOnly)).length;
    const remaining = Math.max(0, MAX_PROMPT_TOKENS - systemTokens);
    const rest = messages.length > 1 ? AiPrompt.fromMessages(messages.slice(1)) : AiPrompt.empty;
    const truncatedRest = yield* tokenizer.truncate(rest, remaining);
    return AiPrompt.fromMessages([first, ...truncatedRest.content]);
  }).pipe(Effect.provide(TOKENIZER_LAYER));

const tokenEstimate = (prompt: AiPrompt.Prompt) =>
  Effect.gen(function* () {
    const tokenizer = yield* AiTokenizer.Tokenizer;
    return (yield* tokenizer.tokenize(prompt)).length;
  }).pipe(Effect.provide(TOKENIZER_LAYER));

const SYSTEM_PROMPT_BASE =
  "You are Autopilot.\n" +
  "\n" +
  "Voice:\n" +
  "- Follow the Blueprint vibe(s) as written. Treat them as the source of truth for tone and style.\n" +
  "- Avoid cheerleading/filler. Prefer short sentences.\n" +
  "- Do not describe your tone. Just use it.\n" +
  "- Do not reveal internal reasoning or planning.\n" +
  "- Do not add marketing language or capability fluff.\n" +
  "\n" +
  "Important:\n" +
  "- Do not claim you can browse the web. You cannot.\n" +
  "- Only mention tools when the Tools section is present.\n" +
  "- Bootstrap is a state machine. Follow bootstrapState.stage; do not repeat earlier bootstrap questions.\n" +
  "- Do not ask the user to confirm their answers during bootstrap. Apply the answer and proceed.\n" +
  "- Never ask for personal info (physical address, email, phone, legal name, etc.).\n" +
  "  Ask only for a preferred handle (what to call the user).\n" +
  "- Avoid the word \"address\" in user-facing messages. Say \"call you\" or \"handle\".\n";

function buildSystemPrompt(options: {
  blueprintContext: string;
  bootstrapInstructions: string | null;
  identityVibe: string;
  characterVibe: string;
  bootstrapState: {
    status: string;
    stage: string | undefined;
    startedAt: Date | undefined;
    completedAt: Date | undefined;
  };
  toolPrompt: string | null;
}) {
  let system = SYSTEM_PROMPT_BASE;
  system +=
    "\n\n# Voice Vibe (verbatim)\n" +
    `IDENTITY.vibe: ${options.identityVibe}\n` +
    `CHARACTER.vibe: ${options.characterVibe}\n`;
  system +=
    "\n\n# Bootstrap State\n" +
    `status: ${options.bootstrapState.status}\n` +
    (options.bootstrapState.stage ? `stage: ${options.bootstrapState.stage}\n` : "") +
    (options.bootstrapState.startedAt
      ? `startedAt: ${options.bootstrapState.startedAt.toISOString()}\n`
      : "") +
    (options.bootstrapState.completedAt
      ? `completedAt: ${options.bootstrapState.completedAt.toISOString()}\n`
      : "");
  system += "\n\n# Blueprint\n" + options.blueprintContext.trim() + "\n";

  if (options.bootstrapInstructions) {
    system += "\n\n# Bootstrap\n" + options.bootstrapInstructions.trim() + "\n";
  }

  if (options.toolPrompt) {
    system += "\n\n" + options.toolPrompt.trim() + "\n";
  }

  return system;
}

function normalizeLegacyBlueprintKeys(input: unknown): unknown {
  if (!input || typeof input !== "object") return input;

  const root = input as Record<string, unknown>;

  // legacy bootstrapState key -> bootstrapState.templateVersion
  if (root.bootstrapState && typeof root.bootstrapState === "object") {
    const bootstrapState = root.bootstrapState as Record<string, unknown>;
    if (
      "ritualVersion" in bootstrapState &&
      !("templateVersion" in bootstrapState)
    ) {
      bootstrapState.templateVersion = bootstrapState.ritualVersion;
      delete bootstrapState.ritualVersion;
    }
  }

  // docs.<legacy> -> docs.bootstrap
  if (root.docs && typeof root.docs === "object") {
    const docs = root.docs as Record<string, unknown>;
    if ("ritual" in docs && !("bootstrap" in docs)) {
      docs.bootstrap = docs.ritual;
      delete docs.ritual;
    }

    // docs.soul -> docs.character
    if ("soul" in docs && !("character" in docs)) {
      docs.character = docs.soul;
      delete docs.soul;
    }
  }

  return root;
}

function safeJsonParse(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function messageText(message: ChatMessage): string {
  const parts: ReadonlyArray<ChatPart> = Array.isArray(message.parts)
    ? (message.parts as ReadonlyArray<ChatPart>)
    : [];
  return parts
    .filter((p) => p && typeof p === "object" && (p as any).type === "text")
    .map((p) => String((p as any).text ?? ""))
    .join("");
}

/**
 * Minimal persistent chat agent:
 * - Durable Object-backed transcript via DO SQLite
 * - Workers AI model (no keys) via `@effect/ai` (`LanguageModel` + Response parts)
 * - No tools beyond the explicit Tool contracts in `./tools`
 */
export class Chat extends Agent<Env> {
  private static readonly BLUEPRINT_ROW_ID = "autopilot_blueprint_state_v1";
  private dseDefaultsInit: Promise<void> | null = null;
  messages: Array<ChatMessage>;
  private chatAbortControllers = new Map<string, AbortController>();

  constructor(ctx: AgentContext, env: Env) {
    super(ctx, env);
    this.sql`create table if not exists cf_ai_chat_agent_messages (
      id text primary key,
      message text not null,
      created_at datetime default current_timestamp
    )`;
    this.sql`create table if not exists autopilot_blueprint_state (
      id text primary key,
      json text not null,
      updated_at integer not null
    )`;
    initDseTables(this.sql.bind(this) as unknown as SqlTag);
    initAiReceiptTables(this.sql.bind(this) as unknown as SqlTag);
    this.messages = this.loadMessagesFromDb();
  }

  private ensureDefaultDsePolicies(): Promise<void> {
    if (this.dseDefaultsInit) return this.dseDefaultsInit;

    this.dseDefaultsInit = (async () => {
      const result = await ensureDefaultArtifacts(
        this.sql.bind(this) as unknown as SqlTag,
        [signatures.blueprint_select_tool]
      );
      if (result.errors.length > 0) {
        console.warn("[dse] default artifact install failed", result.errors);
      }
    })();

    return this.dseDefaultsInit;
  }

  private loadMessagesFromDb(): Array<ChatMessage> {
    const rows =
      this.sql<{ message: string }>`select message from cf_ai_chat_agent_messages order by created_at` ||
      [];
    return rows
      .map((row) => {
        try {
          return JSON.parse(row.message) as ChatMessage;
        } catch (error) {
          console.error("[chat] Failed to parse stored message", error);
          return null;
        }
      })
      .filter((msg): msg is ChatMessage => msg !== null);
  }

  private persistMessages(messages: ReadonlyArray<ChatMessage>) {
    for (const message of messages) {
      const json = JSON.stringify(message);
      this.sql`
        insert into cf_ai_chat_agent_messages (id, message)
        values (${message.id}, ${json})
        on conflict(id) do update set message = excluded.message
      `;
    }
    this.messages = this.loadMessagesFromDb();
  }

  private clearMessages() {
    this.sql`delete from cf_ai_chat_agent_messages`;
    this.messages = [];
  }

  private loadBlueprintState(): AutopilotBlueprintStateV1 | null {
    const rows =
      this.sql<{ json: string }>`select json from autopilot_blueprint_state where id = ${Chat.BLUEPRINT_ROW_ID}` ||
      [];
    const row = rows[0];
    if (!row) return null;

    try {
      const parsed: unknown = JSON.parse(row.json);
      const normalized = normalizeLegacyBlueprintKeys(parsed);
      return Schema.decodeUnknownSync(AutopilotBlueprintStateV1)(normalized);
    } catch (error) {
      console.error("[blueprint] Failed to decode state; resetting.", error);
      return null;
    }
  }

  private saveBlueprintState(state: AutopilotBlueprintStateV1) {
    const encoded = Schema.encodeSync(AutopilotBlueprintStateV1)(state);
    const json = JSON.stringify(encoded);
    this.sql`
      insert into autopilot_blueprint_state (id, json, updated_at)
      values (${Chat.BLUEPRINT_ROW_ID}, ${json}, ${Date.now()})
      on conflict(id) do update set json = excluded.json, updated_at = excluded.updated_at
    `;
  }

  private maybeMigrateBlueprintState(
    state: AutopilotBlueprintStateV1
  ): AutopilotBlueprintStateV1 {
    const templateVersion = Number(state.bootstrapState.templateVersion);
    if (templateVersion >= CURRENT_BOOTSTRAP_TEMPLATE_VERSION) return state;

    const nextBootstrap = BootstrapTemplate.make({
      ...state.docs.bootstrap,
      version: DocVersion.make(Number(state.docs.bootstrap.version) + 1),
      body: DEFAULT_BOOTSTRAP_TEMPLATE_BODY
    });
    const nextDocs = BlueprintDocs.make({
      ...state.docs,
      bootstrap: nextBootstrap
    });
    const nextBootstrapState = AutopilotBootstrapState.make({
      ...state.bootstrapState,
      templateVersion: CURRENT_BOOTSTRAP_TEMPLATE_VERSION
    });

    return AutopilotBlueprintStateV1.make({
      ...state,
      bootstrapState: nextBootstrapState,
      docs: nextDocs
    });
  }

  private ensureBootstrapStage(
    state: AutopilotBlueprintStateV1
  ): AutopilotBlueprintStateV1 {
    // If bootstrap is complete, stage is irrelevant. Normalize to unset.
    if (state.bootstrapState.status === "complete") {
      if (state.bootstrapState.stage == null) return state;
      return AutopilotBlueprintStateV1.make({
        ...state,
        bootstrapState: AutopilotBootstrapState.make({
          ...state.bootstrapState,
          stage: undefined
        })
      });
    }

    if (state.bootstrapState.stage != null) return state;

    // Best-effort inference for older states without a stage field.
    // This keeps the flow deterministic without requiring filesystem/stateful prompts.
    const userKnown = state.docs.user.addressAs !== "Unknown";
    const agentNamed = state.docs.identity.name !== "Autopilot";
    const vibeChanged = state.docs.identity.vibe !== "calm, direct, pragmatic";

    const stage = !userKnown
      ? "ask_user_handle"
      : !agentNamed
        ? "ask_agent_name"
        : !vibeChanged
          ? "ask_vibe"
          : "ask_boundaries";

    return AutopilotBlueprintStateV1.make({
      ...state,
      bootstrapState: AutopilotBootstrapState.make({
        ...state.bootstrapState,
        stage
      })
    });
  }

  private ensureBlueprintState(): AutopilotBlueprintStateV1 {
    const existing = this.loadBlueprintState();
    if (existing) {
      const migrated = this.ensureBootstrapStage(
        this.maybeMigrateBlueprintState(existing)
      );
      if (migrated !== existing) {
        this.saveBlueprintState(migrated);
      }
      return migrated;
    }
    const created = makeDefaultBlueprintState(this.name);
    this.saveBlueprintState(created);
    return created;
  }

  private ensureBlueprintStateForChat(): AutopilotBlueprintStateV1 {
    const current = this.ensureBlueprintState();
    if (current.bootstrapState.status !== "pending") return current;

    return this.updateBlueprintState((state) => {
      const now = new Date();
      const nextBootstrapState = AutopilotBootstrapState.make({
        ...state.bootstrapState,
        status: "in_progress",
        stage: state.bootstrapState.stage ?? "ask_user_handle",
        startedAt: state.bootstrapState.startedAt ?? now
      });
      return AutopilotBlueprintStateV1.make({
        ...state,
        bootstrapState: nextBootstrapState
      });
    });
  }

  private ensureWelcomeMessage(blueprint: AutopilotBlueprintStateV1) {
    if (blueprint.bootstrapState.status === "complete") return;
    if (this.messages.length > 0) return;

    this.persistMessages([
      {
        id: crypto.randomUUID(),
        role: "assistant",
        parts: [{ type: "text", text: FIRST_OPEN_WELCOME_MESSAGE }]
      }
    ]);
  }

  private updateBlueprintState(
    updater: (state: AutopilotBlueprintStateV1) => AutopilotBlueprintStateV1
  ): AutopilotBlueprintStateV1 {
    const current = this.ensureBlueprintState();
    const next = updater(current);
    this.saveBlueprintState(next);
    return next;
  }

  override async onRequest(request: Request): Promise<Response> {
    const url = new URL(request.url);

    await this.ensureDefaultDsePolicies();

    if (url.pathname.endsWith("/get-messages")) {
      const blueprint = this.ensureBlueprintStateForChat();
      this.ensureWelcomeMessage(blueprint);
      return Response.json(this.loadMessagesFromDb());
    }

    if (url.pathname.endsWith("/reset-agent")) {
      if (request.method !== "POST") {
        return new Response("Method not allowed", { status: 405 });
      }

      // Reset persistent Blueprint state.
      const reset = makeDefaultBlueprintState(this.name);
      this.saveBlueprintState(reset);

      // Reset chat transcript + streaming state (server-side).
      //
      // NOTE: We intentionally do this here (instead of relying only on the WS
      // CF_AGENT_CHAT_CLEAR message) so the UI can reset via a single POST,
      // and immediately see the post-reset welcome message without a refresh.
      for (const controller of this.chatAbortControllers.values()) {
        try {
          controller.abort();
        } catch {
          // ignore
        }
      }
      this.chatAbortControllers.clear();
      this.clearMessages();

      // Ensure the first-open UX restarts immediately for any connected clients.
      // Keep bootstrapState at defaults ("pending") on reset; it will transition
      // to "in_progress" when the chat is actually opened (get-messages).
      this.ensureWelcomeMessage(reset);

      return Response.json({ ok: true });
    }

    if (url.pathname.endsWith("/dse/artifacts")) {
      if (request.method === "GET") {
        const signatureId = url.searchParams.get("signatureId");
        const compiledId = url.searchParams.get("compiled_id");
        if (!signatureId || !compiledId) {
          return Response.json(
            {
              code: "invalid_request",
              message: "Expected query params: signatureId, compiled_id"
            },
            { status: 400 }
          );
        }

        const rows =
          this.sql<{ json: string }>`
            select json from dse_artifacts
            where signature_id = ${signatureId} and compiled_id = ${compiledId}
          ` || [];
        const row = rows[0];
        if (!row) {
          return Response.json(
            { code: "not_found", message: "Artifact not found." },
            { status: 404 }
          );
        }
        try {
          return Response.json(JSON.parse(row.json));
        } catch {
          return Response.json(
            { code: "corrupt_artifact", message: "Stored artifact JSON is invalid." },
            { status: 500 }
          );
        }
      }

      if (request.method === "POST") {
        let body: unknown = null;
        try {
          body = await request.json();
        } catch {
          return Response.json(
            { code: "invalid_json", message: "Expected JSON body." },
            { status: 400 }
          );
        }

        let decoded: CompiledArtifact.DseCompiledArtifactV1;
        try {
          decoded = Schema.decodeUnknownSync(CompiledArtifact.DseCompiledArtifactV1Schema)(
            body
          );
        } catch (error) {
          console.error("[dse] invalid artifact", error);
          return Response.json(
            { code: "invalid_artifact", message: "Artifact failed validation." },
            { status: 400 }
          );
        }

        const encoded = Schema.encodeSync(CompiledArtifact.DseCompiledArtifactV1Schema)(
          decoded
        );
        const json = JSON.stringify(encoded);
        const createdAtMs = Date.parse(decoded.createdAt);
        const createdAt = Number.isFinite(createdAtMs) ? createdAtMs : Date.now();

        this.sql`
          insert into dse_artifacts (signature_id, compiled_id, json, created_at)
          values (${decoded.signatureId}, ${decoded.compiled_id}, ${json}, ${createdAt})
          on conflict(signature_id, compiled_id) do nothing
        `;

        return Response.json({
          ok: true,
          signatureId: decoded.signatureId,
          compiled_id: decoded.compiled_id
        });
      }

      return new Response("Method not allowed", { status: 405 });
    }

    if (url.pathname.endsWith("/dse/active")) {
      if (request.method === "GET") {
        const signatureId = url.searchParams.get("signatureId");
        if (!signatureId) {
          return Response.json(
            { code: "invalid_request", message: "Expected query param: signatureId" },
            { status: 400 }
          );
        }

        const rows =
          this.sql<{ compiled_id: string }>`
            select compiled_id from dse_active_artifacts where signature_id = ${signatureId}
          ` || [];

        return Response.json({
          signatureId,
          compiled_id: rows[0]?.compiled_id ?? null
        });
      }

      if (request.method === "POST") {
        let body: unknown = null;
        try {
          body = await request.json();
        } catch {
          return Response.json(
            { code: "invalid_json", message: "Expected JSON body." },
            { status: 400 }
          );
        }

        const signatureId =
          body && typeof body === "object" ? (body as any).signatureId : null;
        const compiledId =
          body && typeof body === "object" ? (body as any).compiled_id : null;

        if (typeof signatureId !== "string" || typeof compiledId !== "string") {
          return Response.json(
            {
              code: "invalid_request",
              message: "Expected body: { signatureId: string, compiled_id: string }"
            },
            { status: 400 }
          );
        }

        const exists =
          (this.sql<{ ok: number }>`
            select 1 as ok from dse_artifacts
            where signature_id = ${signatureId} and compiled_id = ${compiledId}
            limit 1
          ` || [])[0]?.ok === 1;

        if (!exists) {
          return Response.json(
            { code: "not_found", message: "Artifact not found; cannot set active." },
            { status: 404 }
          );
        }

        const ts = Date.now();
        this.sql`
          insert into dse_active_artifact_history (signature_id, compiled_id, updated_at)
          values (${signatureId}, ${compiledId}, ${ts})
        `;
        this.sql`
          insert into dse_active_artifacts (signature_id, compiled_id, updated_at)
          values (${signatureId}, ${compiledId}, ${ts})
          on conflict(signature_id) do update set compiled_id = excluded.compiled_id, updated_at = excluded.updated_at
        `;

        return Response.json({ ok: true, signatureId, compiled_id: compiledId });
      }

      if (request.method === "DELETE") {
        const signatureId = url.searchParams.get("signatureId");
        if (!signatureId) {
          return Response.json(
            { code: "invalid_request", message: "Expected query param: signatureId" },
            { status: 400 }
          );
        }
        this.sql`delete from dse_active_artifacts where signature_id = ${signatureId}`;
        return Response.json({ ok: true, signatureId });
      }

      return new Response("Method not allowed", { status: 405 });
    }

    if (url.pathname.endsWith("/dse/rollback")) {
      if (request.method !== "POST") {
        return new Response("Method not allowed", { status: 405 });
      }

      let body: unknown = null;
      try {
        body = await request.json();
      } catch {
        return Response.json(
          { code: "invalid_json", message: "Expected JSON body." },
          { status: 400 }
        );
      }

      const signatureId =
        body && typeof body === "object" ? (body as any).signatureId : null;
      if (typeof signatureId !== "string") {
        return Response.json(
          { code: "invalid_request", message: "Expected body: { signatureId: string }" },
          { status: 400 }
        );
      }

      const result = rollbackActiveArtifact(
        this.sql.bind(this) as unknown as SqlTag,
        signatureId
      );
      if (!result.ok) {
        return Response.json(
          { code: "rollback_failed", message: result.message ?? "Rollback failed." },
          { status: 400 }
        );
      }
      return Response.json(result);
    }

    if (url.pathname.endsWith("/dse/receipts")) {
      if (request.method !== "GET") {
        return new Response("Method not allowed", { status: 405 });
      }

      const signatureId = url.searchParams.get("signatureId") ?? undefined;
      const limitRaw = url.searchParams.get("limit");
      const limitParsed = limitRaw ? Number(limitRaw) : Number.NaN;
      const limit = Number.isFinite(limitParsed) ? limitParsed : undefined;

      const receipts = listReceipts(this.sql.bind(this) as unknown as SqlTag, {
        ...(signatureId ? { signatureId } : {}),
        ...(limit !== undefined ? { limit } : {})
      });

      return Response.json(receipts);
    }

    if (url.pathname.endsWith("/ai/receipts")) {
      if (request.method !== "GET") {
        return new Response("Method not allowed", { status: 405 });
      }

      const limitRaw = url.searchParams.get("limit");
      const limitParsed = limitRaw ? Number(limitRaw) : Number.NaN;
      const limit = Number.isFinite(limitParsed) ? limitParsed : undefined;

      const receipts = listAiModelReceipts(this.sql.bind(this) as unknown as SqlTag, {
        ...(limit !== undefined ? { limit } : {})
      });

      return Response.json(receipts);
    }

    if (url.pathname.endsWith("/tool-contracts")) {
      if (request.method !== "GET") {
        return new Response("Method not allowed", { status: 405 });
      }
      return Response.json(toolContractsExport());
    }

    if (url.pathname.endsWith("/signature-contracts")) {
      if (request.method !== "GET") {
        return new Response("Method not allowed", { status: 405 });
      }
      return Response.json(signatureContractsExport());
    }

    if (url.pathname.endsWith("/module-contracts")) {
      if (request.method !== "GET") {
        return new Response("Method not allowed", { status: 405 });
      }
      return Response.json(moduleContractsExport());
    }

    if (url.pathname.endsWith("/blueprint")) {
      if (request.method === "GET") {
        const blueprint = this.ensureBlueprintState();
        const exportObj = AutopilotBlueprintV1.make({
          format: BLUEPRINT_FORMAT,
          formatVersion: BLUEPRINT_FORMAT_VERSION,
          exportedAt: new Date(),
          app: { name: "autopilot" },
          bootstrapState: blueprint.bootstrapState,
          docs: blueprint.docs,
          memory: blueprint.memory,
          ...(blueprint.audit ? { audit: blueprint.audit } : {})
        });
        const encoded = Schema.encodeSync(AutopilotBlueprintV1)(exportObj);
        return Response.json(encoded);
      }

      if (request.method === "POST") {
        let body: unknown = null;
        try {
          body = await request.json();
        } catch {
          return Response.json(
            { code: "invalid_json", message: "Expected JSON body." },
            { status: 400 }
          );
        }

        let decoded: AutopilotBlueprintV1;
        try {
          const normalized = normalizeLegacyBlueprintKeys(body);
          decoded = Schema.decodeUnknownSync(AutopilotBlueprintV1)(normalized);
        } catch (error) {
          console.error("[blueprint] Import decode failed.", error);
          return Response.json(
            { code: "invalid_blueprint", message: "Blueprint failed validation." },
            { status: 400 }
          );
        }

        const threadId = ThreadId.make(this.name);
        const userId = UserId.make(this.name);

        const next = AutopilotBlueprintStateV1.make({
          bootstrapState: AutopilotBootstrapState.make({
            ...decoded.bootstrapState,
            userId,
            threadId
          }),
          docs: decoded.docs,
          memory: decoded.memory,
          ...(decoded.audit ? { audit: decoded.audit } : {})
        });

        this.saveBlueprintState(next);

        return Response.json({ ok: true });
      }

      return new Response("Method not allowed", { status: 405 });
    }

    return super.onRequest(request);
  }

  override async onMessage(connection: Connection, message: WSMessage) {
    if (typeof message !== "string") return;

    const parsed = safeJsonParse(message) as any;
    if (!parsed || typeof parsed !== "object") return;

    switch (parsed.type) {
      case MessageType.CF_AGENT_CHAT_CLEAR: {
        for (const controller of this.chatAbortControllers.values()) {
          try {
            controller.abort();
          } catch {
            // ignore
          }
        }
        this.chatAbortControllers.clear();
        this.clearMessages();
        this.broadcast(JSON.stringify({ type: MessageType.CF_AGENT_CHAT_CLEAR }), [
          connection.id
        ]);
        return;
      }

      case MessageType.CF_AGENT_CHAT_MESSAGES: {
        const messages = Array.isArray(parsed.messages)
          ? (parsed.messages as ReadonlyArray<ChatMessage>)
          : [];
        this.clearMessages();
        this.persistMessages(messages);
        this.broadcast(
          JSON.stringify({
            type: MessageType.CF_AGENT_CHAT_MESSAGES,
            messages
          }),
          [connection.id]
        );
        return;
      }

      case MessageType.CF_AGENT_CHAT_REQUEST_CANCEL: {
        const id = typeof parsed.id === "string" ? parsed.id : "";
        const controller = id ? this.chatAbortControllers.get(id) : null;
        if (controller) {
          try {
            controller.abort();
          } catch {
            // ignore
          }
          this.chatAbortControllers.delete(id);
        }
        return;
      }

      case MessageType.CF_AGENT_USE_CHAT_REQUEST: {
        const requestId = typeof parsed.id === "string" ? parsed.id : "";
        if (!requestId) return;
        const init = parsed.init;
        if (!init || init.method !== "POST") return;

        const bodyText = typeof init.body === "string" ? init.body : "";
        const body = bodyText ? safeJsonParse(bodyText) : null;
        const messages = Array.isArray((body as any)?.messages)
          ? ((body as any).messages as ReadonlyArray<ChatMessage>)
          : [];

        this.persistMessages(messages);
        this.broadcast(
          JSON.stringify({
            type: MessageType.CF_AGENT_CHAT_MESSAGES,
            messages
          }),
          [connection.id]
        );

        const controller = new AbortController();
        this.chatAbortControllers.set(requestId, controller);

        try {
          await this.streamChatResponse(connection, requestId, controller.signal);
        } finally {
          this.chatAbortControllers.delete(requestId);
        }

        return;
      }

      default:
        return;
    }
  }

  private toolHandlers() {
    return AUTOPILOT_TOOLKIT.of({
      get_time: ({ timeZone }: any) =>
        Effect.try({
          try: () => {
            const now = new Date();
            const iso = now.toISOString();
            const epochMs = now.getTime();
            const epochSec = Math.floor(epochMs / 1000);

            let formatted: string | null = null;
            let resolvedTimeZone: string | null = null;

            if (timeZone) {
              // Intl is available in Workers. Keep output stable and small.
              const dtf = new Intl.DateTimeFormat("en-US", {
                timeZone,
                dateStyle: "medium",
                timeStyle: "medium"
              });
              formatted = dtf.format(now);
              resolvedTimeZone = timeZone;
            }

            return {
              iso,
              epochMs,
              epochSec,
              ...(formatted ? { formatted } : {}),
              ...(resolvedTimeZone ? { timeZone: resolvedTimeZone } : {})
            };
          },
          catch: (cause) => cause
        }),

      echo: ({ text }: any) => Effect.succeed({ text: String(text ?? "") }),

      bootstrap_set_user_handle: ({ handle }: any) =>
        Effect.try({
          try: () => {
            const updated = this.updateBlueprintState((state) => {
              const now = new Date();
              const user = state.docs.user;
              const nextHandle = String(handle ?? "").trim();

              const nextUser = UserDoc.make({
                ...user,
                version: DocVersion.make(Number(user.version) + 1),
                name: nextHandle,
                addressAs: nextHandle,
                updatedAt: now,
                updatedBy: "agent"
              });
              const nextDocs = BlueprintDocs.make({
                ...state.docs,
                user: nextUser
              });
              const nextBootstrapState =
                state.bootstrapState.status === "complete"
                  ? state.bootstrapState
                  : AutopilotBootstrapState.make({
                      ...state.bootstrapState,
                      status:
                        state.bootstrapState.status === "pending"
                          ? "in_progress"
                          : state.bootstrapState.status,
                      stage: "ask_agent_name",
                      startedAt: state.bootstrapState.startedAt ?? now
                    });
              return AutopilotBlueprintStateV1.make({
                ...state,
                bootstrapState: nextBootstrapState,
                docs: nextDocs
              });
            });

            return {
              ok: true,
              handle: updated.docs.user.addressAs,
              stage: updated.bootstrapState.stage ?? null
            };
          },
          catch: (cause) => cause
        }),

      bootstrap_set_agent_name: ({ name }: any) =>
        Effect.try({
          try: () => {
            const updated = this.updateBlueprintState((state) => {
              const now = new Date();
              const identity = state.docs.identity;
              const nextName = String(name ?? "").trim();

              const nextIdentity = IdentityDoc.make({
                ...identity,
                version: DocVersion.make(Number(identity.version) + 1),
                name: nextName,
                updatedAt: now,
                updatedBy: "agent"
              });
              const nextDocs = BlueprintDocs.make({
                ...state.docs,
                identity: nextIdentity
              });
              const nextBootstrapState =
                state.bootstrapState.status === "complete"
                  ? state.bootstrapState
                  : AutopilotBootstrapState.make({
                      ...state.bootstrapState,
                      status:
                        state.bootstrapState.status === "pending"
                          ? "in_progress"
                          : state.bootstrapState.status,
                      stage: "ask_vibe",
                      startedAt: state.bootstrapState.startedAt ?? now
                    });

              return AutopilotBlueprintStateV1.make({
                ...state,
                bootstrapState: nextBootstrapState,
                docs: nextDocs
              });
            });

            return {
              ok: true,
              name: updated.docs.identity.name,
              stage: updated.bootstrapState.stage ?? null
            };
          },
          catch: (cause) => cause
        }),

      bootstrap_set_agent_vibe: ({ vibe }: any) =>
        Effect.try({
          try: () => {
            const updated = this.updateBlueprintState((state) => {
              const now = new Date();
              const identity = state.docs.identity;
              const nextVibe = String(vibe ?? "").trim();

              const nextIdentity = IdentityDoc.make({
                ...identity,
                version: DocVersion.make(Number(identity.version) + 1),
                vibe: nextVibe,
                updatedAt: now,
                updatedBy: "agent"
              });
              const nextDocs = BlueprintDocs.make({
                ...state.docs,
                identity: nextIdentity
              });
              const nextBootstrapState =
                state.bootstrapState.status === "complete"
                  ? state.bootstrapState
                  : AutopilotBootstrapState.make({
                      ...state.bootstrapState,
                      status:
                        state.bootstrapState.status === "pending"
                          ? "in_progress"
                          : state.bootstrapState.status,
                      stage: "ask_boundaries",
                      startedAt: state.bootstrapState.startedAt ?? now
                    });

              return AutopilotBlueprintStateV1.make({
                ...state,
                bootstrapState: nextBootstrapState,
                docs: nextDocs
              });
            });

            return {
              ok: true,
              vibe: updated.docs.identity.vibe,
              stage: updated.bootstrapState.stage ?? null
            };
          },
          catch: (cause) => cause
        }),

      identity_update: (input: any) =>
        Effect.try({
          try: () => {
            const updated = this.updateBlueprintState((state) => {
              const now = new Date();
              const identity = state.docs.identity;
              const name = typeof input?.name === "string" ? input.name.trim() : undefined;
              const creature =
                typeof input?.creature === "string" ? input.creature.trim() : undefined;
              const vibe = typeof input?.vibe === "string" ? input.vibe.trim() : undefined;
              const emoji = typeof input?.emoji === "string" ? input.emoji.trim() : undefined;
              const avatar =
                typeof input?.avatar === "string" ? input.avatar.trim() : undefined;

              const nextIdentity = IdentityDoc.make({
                ...identity,
                version: DocVersion.make(Number(identity.version) + 1),
                name: name && name.length > 0 ? name : identity.name,
                creature:
                  creature && creature.length > 0 ? creature : identity.creature,
                vibe: vibe && vibe.length > 0 ? vibe : identity.vibe,
                emoji: emoji && emoji.length > 0 ? emoji : identity.emoji,
                avatar: avatar && avatar.length > 0 ? avatar : identity.avatar,
                updatedAt: now,
                updatedBy: "agent"
              });
              const nextDocs = BlueprintDocs.make({
                ...state.docs,
                identity: nextIdentity
              });

              // Bootstrap progression: agent naming and vibe are both Identity updates.
              let nextBootstrapState = state.bootstrapState;
              if (state.bootstrapState.status !== "complete") {
                const stage = state.bootstrapState.stage;
                if (stage === "ask_agent_name" && name && name.length > 0) {
                  nextBootstrapState = AutopilotBootstrapState.make({
                    ...state.bootstrapState,
                    stage: "ask_vibe"
                  });
                } else if (stage === "ask_vibe" && vibe && vibe.length > 0) {
                  nextBootstrapState = AutopilotBootstrapState.make({
                    ...state.bootstrapState,
                    stage: "ask_boundaries"
                  });
                }
              }

              return AutopilotBlueprintStateV1.make({
                ...state,
                bootstrapState: nextBootstrapState,
                docs: nextDocs
              });
            });
            return {
              ok: true,
              version: Number(updated.docs.identity.version)
            };
          },
          catch: (cause) => cause
        }),

      user_update: (input: any) =>
        Effect.try({
          try: () => {
            const updated = this.updateBlueprintState((state) => {
              const now = new Date();
              const user = state.docs.user;
              const handle =
                typeof input?.handle === "string" ? input.handle.trim() : undefined;
              const hasHandle = Boolean(handle) && handle!.length > 0;
              const nextName = hasHandle ? handle! : user.name;
              const nextAddressAs = hasHandle ? handle! : user.addressAs;

              const nextUser = UserDoc.make({
                ...user,
                version: DocVersion.make(Number(user.version) + 1),
                name: nextName,
                addressAs: nextAddressAs,
                pronouns: user.pronouns,
                timeZone: user.timeZone,
                notes: typeof input?.notes === "string" ? input.notes : user.notes,
                context:
                  typeof input?.context === "string" ? input.context : user.context,
                updatedAt: now,
                updatedBy: "agent"
              });
              const nextDocs = BlueprintDocs.make({
                ...state.docs,
                user: nextUser
              });

              // Bootstrap progression: once we have a handle, move to naming the agent.
              let nextBootstrapState = state.bootstrapState;
              if (
                state.bootstrapState.status !== "complete" &&
                state.bootstrapState.stage === "ask_user_handle" &&
                hasHandle
              ) {
                nextBootstrapState = AutopilotBootstrapState.make({
                  ...state.bootstrapState,
                  stage: "ask_agent_name"
                });
              }

              return AutopilotBlueprintStateV1.make({
                ...state,
                bootstrapState: nextBootstrapState,
                docs: nextDocs
              });
            });
            return {
              ok: true,
              version: Number(updated.docs.user.version)
            };
          },
          catch: (cause) => cause
        }),

      character_update: (input: any) =>
        Effect.try({
          try: () => {
            const updated = this.updateBlueprintState((state) => {
              const now = new Date();
              const character = state.docs.character;

              const nextCoreTruths = Array.isArray(input?.coreTruths)
                ? [...character.coreTruths, ...input.coreTruths]
                    .map((s) => s.trim())
                    .filter(Boolean)
                    .filter((v, idx, arr) => arr.indexOf(v) === idx)
                : character.coreTruths;

              const nextBoundaries = Array.isArray(input?.boundaries)
                ? [...character.boundaries, ...input.boundaries]
                    .map((s) => s.trim())
                    .filter(Boolean)
                    .filter((v, idx, arr) => arr.indexOf(v) === idx)
                : character.boundaries;

              const nextCharacter = CharacterDoc.make({
                ...character,
                version: DocVersion.make(Number(character.version) + 1),
                coreTruths: nextCoreTruths,
                boundaries: nextBoundaries,
                vibe: typeof input?.vibe === "string" ? input.vibe : character.vibe,
                continuity:
                  typeof input?.continuity === "string"
                    ? input.continuity
                    : character.continuity,
                updatedAt: now,
                updatedBy: "agent"
              });
              const nextDocs = BlueprintDocs.make({
                ...state.docs,
                character: nextCharacter
              });

              // Bootstrap completion: the final prompt is "boundaries/preferences".
              // If the agent calls character_update at that stage, we treat bootstrap as complete.
              let nextBootstrapState = state.bootstrapState;
              if (
                state.bootstrapState.status !== "complete" &&
                state.bootstrapState.stage === "ask_boundaries"
              ) {
                nextBootstrapState = AutopilotBootstrapState.make({
                  ...state.bootstrapState,
                  status: "complete",
                  stage: undefined,
                  startedAt: state.bootstrapState.startedAt ?? now,
                  completedAt: now
                });
              }

              return AutopilotBlueprintStateV1.make({
                ...state,
                bootstrapState: nextBootstrapState,
                docs: nextDocs
              });
            });
            return {
              ok: true,
              version: Number(updated.docs.character.version)
            };
          },
          catch: (cause) => cause
        }),

      tools_update_notes: ({ notes }: any) =>
        Effect.try({
          try: () => {
            const updated = this.updateBlueprintState((state) => {
              const now = new Date();
              const toolsDoc = state.docs.tools;
              const nextTools = ToolsDoc.make({
                ...toolsDoc,
                version: DocVersion.make(Number(toolsDoc.version) + 1),
                notes: String(notes ?? ""),
                updatedAt: now,
                updatedBy: "agent"
              });
              const nextDocs = BlueprintDocs.make({
                ...state.docs,
                tools: nextTools
              });
              return AutopilotBlueprintStateV1.make({
                ...state,
                docs: nextDocs
              });
            });
            return {
              ok: true,
              version: Number(updated.docs.tools.version)
            };
          },
          catch: (cause) => cause
        }),

      heartbeat_set_checklist: ({ checklist }: any) =>
        Effect.try({
          try: () => {
            const updated = this.updateBlueprintState((state) => {
              const now = new Date();
              const heartbeat = state.docs.heartbeat;
              const nextHeartbeat = HeartbeatDoc.make({
                ...heartbeat,
                version: DocVersion.make(Number(heartbeat.version) + 1),
                checklist: Array.isArray(checklist)
                  ? (checklist as ReadonlyArray<unknown>).map((s) => String(s)).filter(Boolean)
                  : heartbeat.checklist,
                updatedAt: now,
                updatedBy: "agent"
              });
              const nextDocs = BlueprintDocs.make({
                ...state.docs,
                heartbeat: nextHeartbeat
              });
              return AutopilotBlueprintStateV1.make({
                ...state,
                docs: nextDocs
              });
            });
            return {
              ok: true,
              version: Number(updated.docs.heartbeat.version)
            };
          },
          catch: (cause) => cause
        }),

      memory_append: (input: any) =>
        Effect.try({
          try: () => {
            const updated = this.updateBlueprintState((state) => {
              const entry = MemoryEntry.make({
                id: MemoryEntryId.make(crypto.randomUUID()),
                createdAt: new Date(),
                kind: input?.kind,
                title: String(input?.title ?? ""),
                body: String(input?.body ?? ""),
                visibility: input?.visibility
              });
              return AutopilotBlueprintStateV1.make({
                ...state,
                memory: [...state.memory, entry]
              });
            });
            return { ok: true, count: updated.memory.length };
          },
          catch: (cause) => cause
        }),

      bootstrap_complete: () =>
        Effect.try({
          try: () => {
            const updated = this.updateBlueprintState((state) => {
              const now = new Date();
              const nextBootstrapState = AutopilotBootstrapState.make({
                ...state.bootstrapState,
                status: "complete",
                stage: undefined,
                startedAt: state.bootstrapState.startedAt ?? now,
                completedAt: now
              });
              return AutopilotBlueprintStateV1.make({
                ...state,
                bootstrapState: nextBootstrapState
              });
            });
            return { ok: true, status: updated.bootstrapState.status };
          },
          catch: (cause) => cause
        }),

      blueprint_export: () =>
        Effect.succeed({
          ok: true,
          url: `/agents/chat/${this.name}/blueprint`,
          format: BLUEPRINT_FORMAT,
          formatVersion: BLUEPRINT_FORMAT_VERSION
        })
    });
  }

  private applyWirePart(activeParts: Array<ChatPart>, chunkData: AiResponse.StreamPartEncoded): void {
    switch (chunkData.type) {
      case "text-start": {
        activeParts.push({ type: "text", text: "", state: "streaming" });
        return;
      }
      case "text-delta": {
        const lastTextPart = [...activeParts].reverse().find((p) => (p as any)?.type === "text") as any;
        if (lastTextPart && lastTextPart.type === "text") {
          lastTextPart.text += String((chunkData as any).delta ?? "");
        } else {
          activeParts.push({ type: "text", text: String((chunkData as any).delta ?? "") });
        }
        return;
      }
      case "text-end": {
        const lastTextPart = [...activeParts].reverse().find((p) => (p as any)?.type === "text") as any;
        if (lastTextPart && "state" in lastTextPart) lastTextPart.state = "done";
        return;
      }
      case "tool-call": {
        activeParts.push({
          type: `tool-${String((chunkData as any).name ?? "tool")}`,
          toolName: (chunkData as any).name,
          toolCallId: String((chunkData as any).id ?? ""),
          state: "input-available",
          input: (chunkData as any).params
        } satisfies ChatToolPart);
        return;
      }
      case "tool-result": {
        const toolCallId = String((chunkData as any).id ?? "");
        const isFailure = Boolean((chunkData as any).isFailure);
        const toolName = String((chunkData as any).name ?? "tool");
        const result = (chunkData as any).result;

        const stable = (value: unknown) => {
          if (value == null) return String(value);
          if (typeof value === "string") return value;
          try {
            return JSON.stringify(value);
          } catch {
            return String(value);
          }
        };

        let didUpdate = false;
        for (let i = 0; i < activeParts.length; i++) {
          const p = activeParts[i] as any;
          if (!p || typeof p !== "object") continue;
          if (p.toolCallId !== toolCallId) continue;
          p.state = isFailure ? "output-error" : "output-available";
          p.output = result;
          if (isFailure) p.errorText = stable(result);
          didUpdate = true;
          break;
        }

        if (!didUpdate) {
          activeParts.push({
            type: `tool-${toolName}`,
            toolName,
            toolCallId,
            state: isFailure ? "output-error" : "output-available",
            output: result,
            ...(isFailure ? { errorText: stable(result) } : {})
          } satisfies ChatToolPart);
        }

        return;
      }
      default:
        return;
    }
  }

  private sendChatChunk(connection: Connection, requestId: string, body: string, options: { done: boolean; error?: boolean }) {
    connection.send(
      JSON.stringify({
        type: MessageType.CF_AGENT_USE_CHAT_RESPONSE,
        id: requestId,
        body,
        done: options.done,
        ...(options.error ? { error: true } : {})
      })
    );
  }

  private async recordModelReceipt(input: {
    readonly step: number
    readonly requestId: string
    readonly params: unknown
    readonly prompt: AiPrompt.Prompt
    readonly promptTokenEstimate: number
    readonly outputText: string
    readonly toolCalls: ReadonlyArray<AiResponse.ToolCallPartEncoded>
    readonly finish: { readonly reason: string; readonly usage: UsageEncoded } | null
    readonly startedAtMs: number
    readonly endedAtMs: number
    readonly error: unknown | null
  }): Promise<void> {
    try {
      const paramsHash = await sha256IdFromString(JSON.stringify(input.params))

      const encodedPrompt = Schema.encodeSync(AiPrompt.Prompt)(input.prompt)
      const promptJson = JSON.stringify(encodedPrompt)

      const outputJson = JSON.stringify({
        text: input.outputText,
        toolCalls: input.toolCalls.map((tc) => ({
          id: tc.id,
          name: tc.name,
          params: tc.params,
          providerExecuted: tc.providerExecuted,
        })),
      })

      const { promptBlob, outputBlob } = await Effect.runPromise(
        Effect.gen(function* () {
          const blobs = yield* BlobStore.BlobStoreService
          const promptBlob = yield* blobs.putText({
            text: promptJson,
            mime: "application/json",
          })
          const outputBlob = yield* blobs.putText({
            text: outputJson,
            mime: "application/json",
          })
          return { promptBlob, outputBlob }
        }).pipe(
          Effect.provide(layerDseFromSql(this.sql.bind(this) as unknown as SqlTag)),
        ),
      )

      const finish = input.finish
        ? { reason: input.finish.reason, usage: input.finish.usage }
        : undefined

      const receiptKey = {
        provider: "cloudflare-workers-ai",
        modelId: MODEL_ID,
        paramsHash,
        promptBlobId: promptBlob.id,
        outputBlobId: outputBlob.id,
        finish,
        step: input.step,
      }
      const receiptId = await sha256IdFromString(JSON.stringify(receiptKey))

      const receipt: AiModelReceiptV1 = {
        format: "openagents.ai.model_receipt",
        formatVersion: 1,
        receiptId,
        createdAt: new Date().toISOString(),
        provider: "cloudflare-workers-ai",
        modelId: MODEL_ID,
        paramsHash,
        promptBlobs: [promptBlob],
        outputBlobs: [outputBlob],
        ...(finish ? { finish } : {}),
        ...(input.toolCalls.length > 0
          ? { toolCallIds: input.toolCalls.map((tc) => tc.id) }
          : {}),
        promptTokenEstimate: input.promptTokenEstimate,
        maxPromptTokens: MAX_PROMPT_TOKENS,
        timing: {
          startedAtMs: input.startedAtMs,
          endedAtMs: input.endedAtMs,
          durationMs: Math.max(0, input.endedAtMs - input.startedAtMs),
        },
        correlation: {
          agentName: this.name,
          requestId: input.requestId,
          step: input.step,
        },
        result: input.error
          ? {
              _tag: "Error",
              errorName:
                input.error && typeof input.error === "object" && "name" in input.error
                  ? String((input.error as any).name)
                  : "Error",
              message:
                input.error && typeof input.error === "object" && "message" in input.error
                  ? String((input.error as any).message)
                  : String(input.error),
            }
          : { _tag: "Ok" },
      }

      recordAiModelReceipt(this.sql.bind(this) as unknown as SqlTag, receipt)
    } catch (error) {
      console.warn("[ai] failed to record model receipt", error)
    }
  }

  private async streamChatResponse(connection: Connection, requestId: string, abortSignal: AbortSignal) {
    await this.ensureDefaultDsePolicies();

    const buildSystem = (
      state: AutopilotBlueprintStateV1,
      toolNames?: ReadonlyArray<string>
    ) => {
      const blueprintContext = renderBlueprintContext(state, {
        // Only include the Tools doc when tools are active for this step.
        includeToolsDoc: Boolean(toolNames && toolNames.length > 0)
      });
      const bootstrapInstructions = renderBootstrapInstructions(state);
      const toolPrompt =
        toolNames && toolNames.length > 0
          ? renderToolPrompt({ toolNames })
          : null;
      const system = buildSystemPrompt({
        identityVibe: state.docs.identity.vibe,
        characterVibe: state.docs.character.vibe,
        bootstrapState: {
          status: state.bootstrapState.status,
          stage: state.bootstrapState.stage,
          startedAt: state.bootstrapState.startedAt,
          completedAt: state.bootstrapState.completedAt
        },
        blueprintContext,
        bootstrapInstructions,
        toolPrompt
      });
      return capText(system, MAX_SYSTEM_PROMPT_CHARS);
    };

    const blueprint = this.ensureBlueprintStateForChat();
    this.ensureWelcomeMessage(blueprint);

    const recentMessages = this.messages.slice(-MAX_CONTEXT_MESSAGES);
    const lastUserMessageText = (() => {
      for (let i = recentMessages.length - 1; i >= 0; i--) {
        const m = recentMessages[i];
        if (!m || m.role !== "user") continue;
        const text = messageText(m).trim();
        if (text) return text;
      }
      return "";
    })();

    const dseLmClient: Lm.LmClient = {
      complete: (req) =>
        Effect.tryPromise({
          try: async () => {
            const messages = req.messages.map((m) => ({
              role: m.role,
              content: m.content
            }));

            const output = (await this.env.AI.run(
              MODEL_ID as any,
              {
                model: MODEL_ID,
                max_tokens: req.maxTokens ?? 256,
                messages: messages as any,
                ...(typeof req.temperature === "number"
                  ? { temperature: req.temperature }
                  : {}),
                ...(typeof req.topP === "number" ? { top_p: req.topP } : {})
              } as any,
              {}
            )) as any;

            const text =
              output?.choices?.[0]?.message?.content ??
              (typeof output?.response === "string"
                ? output.response
                : JSON.stringify(output?.response ?? ""));

            return { text: typeof text === "string" ? text : String(text) };
          },
          catch: (cause) =>
            Lm.LmClientError.make({
              message: "DSE LM client failed",
              cause
            })
        })
    };

    const selectBlueprintTool = Predict.make(signatures.blueprint_select_tool);

    const prepareStep0 = async () => {
      const fresh = this.ensureBlueprintState();

      // Bootstrap: force the appropriate update tool based on the current stage.
      if (fresh.bootstrapState.status !== "complete") {
        const stage = fresh.bootstrapState.stage ?? "ask_user_handle";

        if (stage === "ask_user_handle") {
          const toolNames = ["bootstrap_set_user_handle"];
          return {
            system: buildSystem(fresh, toolNames),
            toolChoice: { tool: "bootstrap_set_user_handle" as const },
            toolNames
          };
        }

        if (stage === "ask_agent_name") {
          const toolNames = ["bootstrap_set_agent_name"];
          return {
            system: buildSystem(fresh, toolNames),
            toolChoice: { tool: "bootstrap_set_agent_name" as const },
            toolNames
          };
        }

        if (stage === "ask_vibe") {
          const toolNames = ["bootstrap_set_agent_vibe"];
          return {
            system: buildSystem(fresh, toolNames),
            toolChoice: { tool: "bootstrap_set_agent_vibe" as const },
            toolNames
          };
        }

        if (stage === "ask_boundaries") {
          const toolNames = ["character_update", "bootstrap_complete"] as const;
          return {
            system: buildSystem(fresh, toolNames),
            toolChoice: { mode: "auto" as const, oneOf: toolNames },
            toolNames: [...toolNames]
          };
        }

        const toolNames = [...BASE_TOOL_NAMES];
        return {
          system: buildSystem(fresh, toolNames),
          toolChoice: { mode: "auto" as const, oneOf: toolNames },
          toolNames
        };
      }

      // Post-bootstrap: keep the default tool surface minimal (base tools only).
      // If the message looks like a Blueprint update request, route to exactly one
      // Blueprint tool via a DSE signature, and force that tool call.
      if (lastUserMessageText) {
        try {
          const selection = await Effect.runPromise(
            selectBlueprintTool({
              message: lastUserMessageText,
              blueprintHint: {
                userHandle: fresh.docs.user.addressAs,
                agentName: fresh.docs.identity.name
              }
            }).pipe(
              Effect.provideService(Lm.LmClientService, dseLmClient),
              Effect.provide(
                layerDseFromSql(this.sql.bind(this) as unknown as SqlTag)
              )
            )
          );

          if (selection.action === "tool") {
            const toolName = selection.toolName as string;
            const toolNames = [toolName];
            return {
              system: buildSystem(fresh, toolNames),
              toolChoice: { tool: toolName },
              toolNames
            };
          }
        } catch (error) {
          console.warn("[dse] blueprint tool routing failed; falling back", error);
        }
      }

      const toolNames = [...BASE_TOOL_NAMES];
      return {
        system: buildSystem(fresh, toolNames),
        toolChoice: { mode: "auto" as const, oneOf: toolNames },
        toolNames
      };
    };

    const { system, toolChoice } = await prepareStep0();
    const promptBase = [
      { role: "system" as const, content: system },
      ...recentMessages.flatMap((m) => {
        const text = messageText(m).trim();
        if (!text) return [];
        return [
          {
            role: m.role,
            content: [{ type: "text" as const, text }]
          }
        ];
      })
    ];

    const prompt0 = await Effect.runPromise(budgetChatPrompt(promptBase));
    const prompt0TokenEstimate = await Effect.runPromise(tokenEstimate(prompt0));
    const prompt0Encoded = Schema.encodeSync(AiPrompt.Prompt)(prompt0);

    const toolCalls: Array<AiResponse.ToolCallPartEncoded> = [];
    const toolResults: Array<AiResponse.ToolResultPartEncoded> = [];
    const activeParts: Array<ChatPart> = [];

    const handlers = this.toolHandlers();
    const toolLayer = AUTOPILOT_TOOLKIT.toLayer(handlers);
    const modelLayer = Layer.effect(
      AiLanguageModel.LanguageModel,
      makeWorkersAiLanguageModel({
        binding: this.env.AI,
        model: MODEL_ID,
        maxOutputTokens: MAX_OUTPUT_TOKENS
      })
    );
    const live = Layer.mergeAll(toolLayer, modelLayer);

    const step0Params = {
      provider: "cloudflare-workers-ai",
      modelId: MODEL_ID,
      maxOutputTokens: MAX_OUTPUT_TOKENS,
      toolChoice,
      concurrency: 1,
      step: 0,
    };

    const step0StartedAtMs = Date.now();
    let step0OutputText = "";
    let step0Finish: { readonly reason: string; readonly usage: UsageEncoded } | null = null;
    let step0Error: unknown | null = null;

    const runStep0 = () =>
      AiLanguageModel.streamText({
        prompt: prompt0,
        toolkit: AUTOPILOT_TOOLKIT,
        toolChoice: toolChoice as any,
        concurrency: 1,
        disableToolCallResolution: true
      }).pipe(
        Stream.runForEach((part) =>
          Effect.sync(() => {
            if (abortSignal.aborted) return;
            const encoded = encodeStreamPart(part as any);
            // Autopilot must not reveal internal reasoning on the UI wire.
            if (
              encoded.type === "reasoning-start" ||
              encoded.type === "reasoning-delta" ||
              encoded.type === "reasoning-end"
            ) {
              return;
            }
            if (encoded.type === "text-delta") {
              step0OutputText += String((encoded as any).delta ?? "");
            }
            if (encoded.type === "tool-call") {
              toolCalls.push(encoded as any);
            }
            if (encoded.type === "finish") {
              step0Finish = {
                reason: String((encoded as any).reason ?? "unknown"),
                usage: (encoded as any).usage as UsageEncoded,
              };
            }
            this.sendChatChunk(
              connection,
              requestId,
              encodeWirePart(encoded),
              { done: encoded.type === "finish" && toolCalls.length === 0 }
            );
            this.applyWirePart(activeParts, encoded);
          })
        ),
        Effect.provide(live)
      );

    try {
      await Effect.runPromise(runStep0() as Effect.Effect<void, any, never>);
    } catch (error) {
      step0Error = error;
      console.error("[chat] step0 stream failed", error);
      this.sendChatChunk(connection, requestId, "Internal error.", {
        done: true,
        error: true
      });
    } finally {
      const endedAtMs = Date.now();
      await this.recordModelReceipt({
        step: 0,
        requestId,
        params: step0Params,
        prompt: prompt0,
        promptTokenEstimate: prompt0TokenEstimate,
        outputText: step0OutputText,
        toolCalls,
        finish: step0Finish,
        startedAtMs: step0StartedAtMs,
        endedAtMs,
        error: step0Error,
      });
    }

    if (step0Error) return;

    if (abortSignal.aborted) return;

    if (toolCalls.length > 0) {
      const toolkit = await Effect.runPromise(
        AUTOPILOT_TOOLKIT.pipe(Effect.provide(toolLayer))
      );

      for (const toolCall of toolCalls) {
        if (abortSignal.aborted) return;
        const handlerResult = (await Effect.runPromise(
          toolkit.handle(toolCall.name as any, toolCall.params as any) as any
        )) as { readonly encodedResult: unknown; readonly isFailure: boolean };
        const resultPart: AiResponse.ToolResultPartEncoded = {
          type: "tool-result",
          id: toolCall.id,
          name: toolCall.name,
          result: handlerResult.encodedResult,
          isFailure: handlerResult.isFailure,
          providerExecuted: false
        };
        toolResults.push(resultPart);
        this.sendChatChunk(connection, requestId, encodeWirePart(resultPart), {
          done: false
        });
        this.applyWirePart(activeParts, resultPart);
      }

      const fresh = this.ensureBlueprintState();
      const system2 = buildSystem(fresh);

      const toolCallMessage = {
        role: "assistant" as const,
        content: toolCalls.map((tc) => ({
          type: "tool-call" as const,
          id: tc.id,
          name: tc.name,
          params: tc.params,
          providerExecuted: false
        }))
      };
      const toolResultMessage = {
        role: "tool" as const,
        content: toolResults.map((tr) => ({
          type: "tool-result" as const,
          id: tr.id,
          name: tr.name,
          isFailure: tr.isFailure,
          result: tr.result,
          providerExecuted: false
        }))
      };

      const prompt2 = [
        { role: "system" as const, content: system2 },
        ...(prompt0Encoded.content as any).slice(1), // reuse truncated user/assistant history (no system)
        toolCallMessage,
        toolResultMessage
      ];

      const prompt1 = await Effect.runPromise(budgetChatPrompt(prompt2));
      const prompt1TokenEstimate = await Effect.runPromise(tokenEstimate(prompt1));

      const step1Params = {
        provider: "cloudflare-workers-ai",
        modelId: MODEL_ID,
        maxOutputTokens: MAX_OUTPUT_TOKENS,
        toolChoice: "none",
        step: 1,
      };

      const step1StartedAtMs = Date.now();
      let step1OutputText = "";
      let step1Finish: { readonly reason: string; readonly usage: UsageEncoded } | null = null;
      let step1Error: unknown | null = null;

      const runStep1 = () =>
        AiLanguageModel.streamText({
          prompt: prompt1,
          toolChoice: "none",
          disableToolCallResolution: true
        }).pipe(
          Stream.runForEach((part) =>
            Effect.sync(() => {
              if (abortSignal.aborted) return;
              const encoded = encodeStreamPart(part as any);
              // Autopilot must not reveal internal reasoning on the UI wire.
              if (
                encoded.type === "reasoning-start" ||
                encoded.type === "reasoning-delta" ||
                encoded.type === "reasoning-end"
              ) {
                return;
              }
              if (encoded.type === "text-delta") {
                step1OutputText += String((encoded as any).delta ?? "");
              }
              if (encoded.type === "finish") {
                step1Finish = {
                  reason: String((encoded as any).reason ?? "unknown"),
                  usage: (encoded as any).usage as UsageEncoded,
                };
              }
              this.sendChatChunk(connection, requestId, encodeWirePart(encoded), {
                done: encoded.type === "finish"
              });
              this.applyWirePart(activeParts, encoded);
            })
          ),
          Effect.provide(modelLayer)
        );

      try {
        await Effect.runPromise(runStep1());
      } catch (error) {
        step1Error = error;
        console.error("[chat] step1 stream failed", error);
        this.sendChatChunk(connection, requestId, "Internal error.", {
          done: true,
          error: true
        });
      } finally {
        const endedAtMs = Date.now();
        await this.recordModelReceipt({
          step: 1,
          requestId,
          params: step1Params,
          prompt: prompt1,
          promptTokenEstimate: prompt1TokenEstimate,
          outputText: step1OutputText,
          toolCalls: [],
          finish: step1Finish,
          startedAtMs: step1StartedAtMs,
          endedAtMs,
          error: step1Error,
        });
      }

      if (step1Error) return;

      if (abortSignal.aborted) return;
    }

    // Persist a single assistant message (tool parts + final text).
    const assistantMessage: ChatMessage = {
      id: `assistant_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`,
      role: "assistant",
      parts: activeParts
    };
    this.persistMessages([assistantMessage]);
    this.broadcast(
      JSON.stringify({
        type: MessageType.CF_AGENT_MESSAGE_UPDATED,
        message: assistantMessage
      })
    );
  }
}

export default {
  async fetch(request: Request, env: Env) {
    const response = await routeAgentRequest(request, env);
    return response ?? new Response("Not found", { status: 404 });
  }
} satisfies ExportedHandler<Env>;
