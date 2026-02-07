import { routeAgentRequest, type AgentContext } from "agents";
import { AIChatAgent } from "@cloudflare/ai-chat";
import {
  convertToModelMessages,
  createUIMessageStream,
  createUIMessageStreamResponse,
  generateText,
  InvalidToolInputError,
  jsonSchema,
  NoSuchToolError,
  stepCountIs,
  streamText,
  type StreamTextOnFinishCallback,
  tool,
  type ToolSet
} from "ai";
import { createWorkersAI } from "workers-ai-provider";
import { Effect, Schema } from "effect";
import {
  CompiledArtifact,
  Lm,
  Predict,
  Tool,
  type DseToolContract
} from "@openagentsinc/dse";
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

const MODEL_ID = "@cf/openai/gpt-oss-120b";
const MAX_CONTEXT_MESSAGES = 25;
const MAX_OUTPUT_TOKENS = 512;

const FIRST_OPEN_WELCOME_MESSAGE =
  "Autopilot online.\n\n" +
  "Greetings, user. What shall I call you?";

function aiToolFromContract<I, O>(
  contract: DseToolContract<I, O>,
  execute: (input: I) => Promise<O>
) {
  const inputSchema = jsonSchema(Tool.inputJsonSchema(contract));
  return tool({
    description: contract.description,
    inputSchema,
    strict: true,
    ...(contract.inputExamples ? { inputExamples: contract.inputExamples as any } : {}),
    execute: execute as any
  });
}

const BASE_TOOLS: ToolSet = {
  get_time: aiToolFromContract(
    toolContracts.get_time,
    async ({ timeZone }) => {
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
    }
  ),
  echo: aiToolFromContract(
    toolContracts.echo,
    async ({ text }) => ({ text })
  )
};

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

function stripToolExecution<T extends ToolSet>(tools: T): T {
  // Tool-call repair should never have side effects. We strip executors so
  // we can ask the model to "re-emit" a tool call without running it.
  return Object.fromEntries(
    Object.entries(tools).map(([name, toolDef]) => [
      name,
      {
        ...(toolDef as any),
        execute: undefined,
        onInputStart: undefined,
        onInputDelta: undefined,
        onInputAvailable: undefined
      }
    ])
  ) as T;
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

/**
 * Minimal persistent chat agent:
 * - Durable Object-backed transcript via AIChatAgent
 * - Workers AI model (no keys)
 * - No tools, no sandbox, no containers
 */
export class Chat extends AIChatAgent<Env> {
  private static readonly BLUEPRINT_ROW_ID = "autopilot_blueprint_state_v1";
  private dseDefaultsInit: Promise<void> | null = null;

  constructor(ctx: AgentContext, env: Env) {
    super(ctx, env);
    this.sql`create table if not exists autopilot_blueprint_state (
      id text primary key,
      json text not null,
      updated_at integer not null
    )`;
    initDseTables(this.sql.bind(this) as unknown as SqlTag);
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

  private async ensureWelcomeMessage(blueprint: AutopilotBlueprintStateV1) {
    if (blueprint.bootstrapState.status === "complete") return;
    if (this.messages.length > 0) return;

    await this.persistMessages([
      {
        id: crypto.randomUUID(),
        role: "assistant",
        parts: [{ type: "text", text: FIRST_OPEN_WELCOME_MESSAGE }]
      } as any
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
      await this.ensureWelcomeMessage(blueprint);
      return super.onRequest(request);
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
      // NOTE: We intentionally do this here (instead of relying on the WS-only
      // CF_AGENT_CHAT_CLEAR message) so the UI can reset via a single POST,
      // and immediately see the post-reset welcome message without a refresh.
      this.sql`delete from cf_ai_chat_agent_messages`;
      this.sql`delete from cf_ai_chat_stream_chunks`;
      this.sql`delete from cf_ai_chat_stream_metadata`;
      this.messages = [];
      this._activeStreamId = null;
      this._activeRequestId = null;

      // Ensure the first-open UX restarts immediately for any connected clients.
      // Keep bootstrapState at defaults ("pending") on reset; it will transition
      // to "in_progress" when the chat is actually opened (get-messages).
      await this.ensureWelcomeMessage(reset);

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

  override async onChatMessage(
    onFinish: StreamTextOnFinishCallback<ToolSet>,
    options?: { abortSignal?: AbortSignal }
  ) {
    const workersai = createWorkersAI({ binding: this.env.AI });
    const blueprint = this.ensureBlueprintStateForChat();
    const recentMessages = this.messages.slice(-MAX_CONTEXT_MESSAGES);

    const stream = createUIMessageStream({
      execute: async ({ writer }) => {
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
          return buildSystemPrompt({
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
        };

        const tools: ToolSet = {
          ...BASE_TOOLS,
          bootstrap_set_user_handle: aiToolFromContract(
            toolContracts.bootstrap_set_user_handle,
            async ({ handle }) => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const user = state.docs.user;
                const nextHandle = handle.trim();

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
            }
          ),
          bootstrap_set_agent_name: aiToolFromContract(
            toolContracts.bootstrap_set_agent_name,
            async ({ name }) => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const identity = state.docs.identity;
                const nextName = name.trim();

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
            }
          ),
          bootstrap_set_agent_vibe: aiToolFromContract(
            toolContracts.bootstrap_set_agent_vibe,
            async ({ vibe }) => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const identity = state.docs.identity;
                const nextVibe = vibe.trim();

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
            }
          ),
          identity_update: aiToolFromContract(
            toolContracts.identity_update,
            async (input) => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const identity = state.docs.identity;
                const name = input.name?.trim();
                const creature = input.creature?.trim();
                const vibe = input.vibe?.trim();
                const emoji = input.emoji?.trim();
                const avatar = input.avatar?.trim();

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
            }
          ),
          user_update: aiToolFromContract(
            toolContracts.user_update,
            async (input) => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const user = state.docs.user;
                const handle = input.handle?.trim();
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
                  notes: input.notes ?? user.notes,
                  context: input.context ?? user.context,
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
            }
          ),
          character_update: aiToolFromContract(
            toolContracts.character_update,
            async (input) => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const character = state.docs.character;

                const nextCoreTruths = input.coreTruths
                  ? [...character.coreTruths, ...input.coreTruths]
                      .map((s) => s.trim())
                      .filter(Boolean)
                      .filter((v, idx, arr) => arr.indexOf(v) === idx)
                  : character.coreTruths;

                const nextBoundaries = input.boundaries
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
                  vibe: input.vibe ?? character.vibe,
                  continuity: input.continuity ?? character.continuity,
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
            }
          ),
          tools_update_notes: aiToolFromContract(
            toolContracts.tools_update_notes,
            async ({ notes }) => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const toolsDoc = state.docs.tools;
                const nextTools = ToolsDoc.make({
                  ...toolsDoc,
                  version: DocVersion.make(Number(toolsDoc.version) + 1),
                  notes,
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
            }
          ),
          heartbeat_set_checklist: aiToolFromContract(
            toolContracts.heartbeat_set_checklist,
            async ({ checklist }) => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const heartbeat = state.docs.heartbeat;
                const nextHeartbeat = HeartbeatDoc.make({
                  ...heartbeat,
                  version: DocVersion.make(Number(heartbeat.version) + 1),
                  checklist,
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
            }
          ),
          memory_append: aiToolFromContract(
            toolContracts.memory_append,
            async (input) => {
              const updated = this.updateBlueprintState((state) => {
                const entry = MemoryEntry.make({
                  id: MemoryEntryId.make(crypto.randomUUID()),
                  createdAt: new Date(),
                  kind: input.kind,
                  title: input.title,
                  body: input.body,
                  visibility: input.visibility
                });
                return AutopilotBlueprintStateV1.make({
                  ...state,
                  memory: [...state.memory, entry]
                });
              });
              return { ok: true, count: updated.memory.length };
            }
          ),
          bootstrap_complete: aiToolFromContract(
            toolContracts.bootstrap_complete,
            async () => {
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
            }
          ),
          blueprint_export: aiToolFromContract(
            toolContracts.blueprint_export,
            async () => ({
              ok: true,
              url: `/agents/chat/${this.name}/blueprint`,
              format: BLUEPRINT_FORMAT,
              formatVersion: BLUEPRINT_FORMAT_VERSION
            })
          )
        };

        const lastUserMessageText = (() => {
          for (let i = recentMessages.length - 1; i >= 0; i--) {
            const m: any = recentMessages[i];
            if (!m || m.role !== "user") continue;
            const parts: any[] = Array.isArray(m.parts) ? m.parts : [];
            const text = parts
              .filter((p) => p && typeof p === "object" && p.type === "text")
              .map((p) => String((p as any).text ?? ""))
              .join("");
            if (text.trim()) return text.trim();
          }
          return "";
        })();

        const dseLmClient: Lm.LmClient = {
          complete: (req) =>
            Effect.tryPromise({
              try: async () => {
                const systemText = req.messages.find((m) => m.role === "system")
                  ?.content;
                const messages = req.messages
                  .filter((m) => m.role !== "system")
                  .map((m) => ({ role: m.role, content: m.content }));

                const res = await generateText({
                  model: workersai(MODEL_ID as Parameters<typeof workersai>[0]),
                  ...(systemText ? { system: systemText } : {}),
                  messages: messages as any,
                  maxOutputTokens: req.maxTokens ?? 256,
                  ...(typeof req.temperature === "number"
                    ? { temperature: req.temperature }
                    : {}),
                  ...(typeof req.topP === "number" ? { topP: req.topP } : {})
                });

                return { text: res.text };
              },
              catch: (cause) =>
                Lm.LmClientError.make({
                  message: "DSE LM client failed",
                  cause
                })
            })
        };

        const selectBlueprintTool = Predict.make(signatures.blueprint_select_tool);

        const result = streamText({
          system: buildSystem(blueprint, BASE_TOOL_NAMES),
          prepareStep: async ({ stepNumber }) => {
            const fresh = this.ensureBlueprintState();

            // After the first step (tool call), produce text only. This keeps the loop
            // deterministic and prevents cascading tool calls / tool list exposure.
            if (stepNumber > 0) {
              return {
                system: buildSystem(fresh),
                toolChoice: "none" as const,
                activeTools: []
              };
            }

            // Bootstrap: force the appropriate update tool based on the current stage.
            // This prevents the model from "confirming" in text without persisting state.
            if (fresh.bootstrapState.status !== "complete") {
              const stage = fresh.bootstrapState.stage ?? "ask_user_handle";

              if (stage === "ask_user_handle") {
                const activeTools = ["bootstrap_set_user_handle"];
                return {
                  system: buildSystem(fresh, activeTools),
                  toolChoice: {
                    type: "tool",
                    toolName: "bootstrap_set_user_handle"
                  } as const,
                  activeTools
                };
              }

              if (stage === "ask_agent_name") {
                const activeTools = ["bootstrap_set_agent_name"];
                return {
                  system: buildSystem(fresh, activeTools),
                  toolChoice: {
                    type: "tool",
                    toolName: "bootstrap_set_agent_name"
                  } as const,
                  activeTools
                };
              }

              if (stage === "ask_vibe") {
                const activeTools = ["bootstrap_set_agent_vibe"];
                return {
                  system: buildSystem(fresh, activeTools),
                  toolChoice: {
                    type: "tool",
                    toolName: "bootstrap_set_agent_vibe"
                  } as const,
                  activeTools
                };
              }

              if (stage === "ask_boundaries") {
                const activeTools = [
                  "character_update",
                  "bootstrap_complete"
                ];
                return {
                  system: buildSystem(fresh, activeTools),
                  // Only call tools when the user actually provides boundaries or says "none".
                  toolChoice: "auto" as const,
                  activeTools
                };
              }

              const activeTools = [...BASE_TOOL_NAMES];
              return {
                system: buildSystem(fresh, activeTools),
                toolChoice: "auto" as const,
                activeTools
              };
            }

            // Post-bootstrap: keep the default tool surface minimal (base tools only).
            // If the message looks like a Blueprint update request, route to exactly one
            // Blueprint tool via a DSE signature, and force that tool call.
            if (!lastUserMessageText) {
              const activeTools = [...BASE_TOOL_NAMES];
              return {
                system: buildSystem(fresh, activeTools),
                toolChoice: "auto" as const,
                activeTools
              };
            }

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
                const activeTools = [toolName];
                return {
                  system: buildSystem(fresh, activeTools),
                  toolChoice: { type: "tool", toolName } as const,
                  activeTools
                };
              }
            } catch (error) {
              console.warn("[dse] blueprint tool routing failed; falling back", error);
            }

            const activeTools = [...BASE_TOOL_NAMES];
            return {
              system: buildSystem(fresh, activeTools),
              toolChoice: "auto" as const,
              activeTools
            };
          },
	          messages: await convertToModelMessages(recentMessages),
	          model: workersai(MODEL_ID as Parameters<typeof workersai>[0]),
	          maxOutputTokens: MAX_OUTPUT_TOKENS,
	          stopWhen: stepCountIs(10),
	          tools,
	          experimental_repairToolCall: async ({
	            toolCall,
	            tools: availableTools,
	            error,
	            messages,
	            system
	          }) => {
	            // AI SDK: parseToolCall() only calls this for NoSuchToolError / InvalidToolInputError.
	            // We do a single "re-ask" to produce a valid tool call, without executing tools.
	            if (!availableTools) return null;

	            const model = workersai(MODEL_ID as Parameters<typeof workersai>[0]);

	            // NOTE: Keep repair prompts short. The goal is a syntactically-valid tool call,
	            // not a full response. The main stream will execute the tool and then respond.
	            const repairPrompt =
	              "Repair the following tool call.\n" +
	              "- Output MUST be a tool call.\n" +
	              "- Do not output any normal assistant text.\n" +
	              `- Error: ${error instanceof Error ? error.message : String(error)}\n` +
	              `- Original toolName: ${toolCall.toolName}\n` +
	              `- Original input: ${toolCall.input}\n`;

	            try {
	              // Invalid inputs: force the same tool name and regenerate inputs.
		              if (InvalidToolInputError.isInstance(error)) {
		                const toolName = toolCall.toolName as keyof typeof availableTools & string;
		                const repairTools = stripToolExecution(availableTools);

		                const repaired = await generateText({
		                  model,
		                  ...(system != null ? { system } : {}),
		                  messages: [
		                    ...messages.slice(-6),
		                    { role: "user", content: repairPrompt }
		                  ],
		                  tools: repairTools,
	                  toolChoice: { type: "tool", toolName } as const,
	                  activeTools: [toolName] as const,
	                  maxOutputTokens: 256
	                });

	                const next = repaired.toolCalls.find((tc) => tc.toolName === toolName);
	                if (!next) return null;

	                console.warn("[chat] repaired tool inputs", {
	                  toolName,
	                  toolCallId: toolCall.toolCallId
	                });

	                return {
	                  ...toolCall,
	                  toolName,
	                  input: JSON.stringify(next.input ?? {})
	                };
	              }

	              // Unknown tool name: restrict to BASE_TOOLS (safe) and require *some* tool call.
	              // This avoids silent stalls when the model invents tool names.
	              if (NoSuchToolError.isInstance(error)) {
	                const repairTools = stripToolExecution(BASE_TOOLS);
	                const repaired = await generateText({
	                  model,
	                  system:
	                    (typeof system === "string" ? system : "") +
	                    "\n\nTool call repair: only use the provided tools.",
	                  messages: [
	                    ...messages.slice(-6),
	                    { role: "user", content: repairPrompt }
	                  ],
	                  tools: repairTools,
	                  toolChoice: "required",
	                  maxOutputTokens: 256
	                });

	                const next = repaired.toolCalls[0];
	                if (!next) return null;

	                console.warn("[chat] repaired tool name", {
	                  from: toolCall.toolName,
	                  to: next.toolName,
	                  toolCallId: toolCall.toolCallId
	                });

	                return {
	                  ...toolCall,
	                  toolName: next.toolName,
	                  input: JSON.stringify(next.input ?? {})
	                };
	              }
	            } catch (repairError) {
	              console.error("[chat] tool call repair failed", repairError);
	            }

	            return null;
	          },
	          onStepFinish: ({ toolCalls, toolResults, text, finishReason, usage }) => {
	            // Observability: when tool calling misbehaves, it's usually because the
	            // model emitted an invalid tool name/input. Log enough to debug quickly.
	            const hasToolActivity =
	              (toolCalls?.length ?? 0) > 0 || (toolResults?.length ?? 0) > 0;
	            if (!hasToolActivity) return;

	            const summarizedToolCalls = (toolCalls ?? []).map((tc) => ({
	              toolCallId: (tc as any).toolCallId,
	              toolName: (tc as any).toolName,
	              invalid: Boolean((tc as any).invalid)
	            }));
	            const summarizedToolResults = (toolResults ?? []).map((tr) => ({
	              toolCallId: (tr as any).toolCallId,
	              toolName: (tr as any).toolName,
	              type: (tr as any).type
	            }));

	            console.log("[chat] step.finish", {
	              finishReason,
	              usage,
	              hasText: Boolean(text && text.trim().length > 0),
	              toolCalls: summarizedToolCalls,
	              toolOutputs: summarizedToolResults
	            });
	          },
	          // Base class uses this callback to persist messages + stream metadata.
	          onFinish: onFinish as unknown as StreamTextOnFinishCallback<ToolSet>,
	          ...(options?.abortSignal ? { abortSignal: options.abortSignal } : {})
	        });

        writer.merge(
          result.toUIMessageStream({
            sendReasoning: false,
            onError: (error) => {
              console.error("[chat] stream error", error);
              return "Internal error.";
            }
          })
        );
      }
    });

    return createUIMessageStreamResponse({ stream });
  }
}

export default {
  async fetch(request: Request, env: Env) {
    const response = await routeAgentRequest(request, env);
    return response ?? new Response("Not found", { status: 404 });
  }
} satisfies ExportedHandler<Env>;
