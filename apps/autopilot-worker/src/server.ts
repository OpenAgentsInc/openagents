import { routeAgentRequest, type AgentContext } from "agents";
import { AIChatAgent } from "@cloudflare/ai-chat";
import {
  convertToModelMessages,
  createUIMessageStream,
  createUIMessageStreamResponse,
  jsonSchema,
  stepCountIs,
  streamText,
  type StreamTextOnFinishCallback,
  tool,
  type ToolSet
} from "ai";
import { createWorkersAI } from "workers-ai-provider";
import { Schema } from "effect";
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
  IdentityDoc,
  MemoryEntry,
  MemoryEntryId,
  SoulDoc,
  ToolsDoc,
  ThreadId,
  UserDoc,
  UserId,
  makeDefaultBlueprintState,
  renderBlueprintContext,
  renderBootstrapInstructions
} from "./blueprint";

const MODEL_ID = "@cf/openai/gpt-oss-120b";
const MAX_CONTEXT_MESSAGES = 25;
const MAX_OUTPUT_TOKENS = 512;

const FIRST_OPEN_WELCOME_MESSAGE =
  "Autopilot online.\n\n" +
  "Greetings, user. What shall I call you?";

const BASE_TOOLS: ToolSet = {
  get_time: tool({
    description:
      "Return the current time. Use this tool whenever the user asks you to use a tool but doesn't specify which one.",
    inputSchema: jsonSchema({
      type: "object",
      properties: {
        timeZone: {
          type: "string",
          description:
            "Optional IANA time zone name (e.g. 'UTC', 'America/Chicago')."
        }
      },
      additionalProperties: false
    }),
    strict: true,
    execute: async ({ timeZone }: { timeZone?: string }) => {
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
  }),
  echo: tool({
    description: "Echo back the provided text. Useful for testing tool calling.",
    inputSchema: jsonSchema({
      type: "object",
      properties: {
        text: { type: "string" }
      },
      required: ["text"],
      additionalProperties: false
    }),
    strict: true,
    execute: async ({ text }: { text: string }) => ({ text })
  })
};

const SYSTEM_PROMPT_BASE =
  "You are Autopilot, a persistent personal AI agent.\n" +
  "\n" +
  "Voice:\n" +
  "- Follow the Blueprint vibe(s) as written. Do not override them with a generic assistant tone.\n" +
  "- Avoid cheerleading/filler. Prefer short sentences.\n" +
  "- Do not reveal internal reasoning or planning.\n" +
  "\n" +
  "Important:\n" +
  "- Do not claim you can browse the web. You cannot.\n" +
  "- Only mention tools when the Tools section is present.\n" +
  "- Never ask for personal info (physical address, email, phone, legal name, etc.).\n" +
  "  Ask only for a preferred handle (what to call the user).\n" +
  "- Avoid the word \"address\" in user-facing messages. Say \"call you\" or \"handle\".\n";

const TOOL_PROMPT =
  "Tools available:\n" +
  "- get_time({ timeZone? }) -> current time\n" +
  "- echo({ text }) -> echoes input\n" +
  "- identity_update({ name?, creature?, vibe?, emoji?, avatar? }) -> update your Identity doc\n" +
  "- user_update({ handle?, notes?, context? }) -> update the User doc (handle = what to call the user; not a postal address)\n" +
  "- soul_update({ coreTruths?, boundaries?, vibe?, continuity? }) -> update the Soul doc\n" +
  "- tools_update_notes({ notes }) -> update the Tools doc\n" +
  "- heartbeat_set_checklist({ checklist }) -> update the Heartbeat doc\n" +
  "- memory_append({ kind, title, body, visibility }) -> append a Memory entry\n" +
  "- bootstrap_complete({}) -> mark bootstrap complete\n" +
  "- blueprint_export({}) -> get a URL to export your Blueprint\n" +
  "\n" +
  "Tool use rules:\n" +
  "- If the user asks you to use a tool, you MUST call an appropriate tool.\n" +
  "- After using tools, ALWAYS send a normal assistant reply with user-visible text.\n" +
  "- Always include a user-visible text reply. Never output reasoning-only.\n" +
  "- Never claim you have tools you do not have.\n" +
  "- If the user asks you to search/browse the web, be explicit that you currently cannot.\n";

function buildSystemPrompt(options: {
  blueprintContext: string;
  bootstrapInstructions: string | null;
  identityVibe: string;
  soulVibe: string;
}) {
  let system = SYSTEM_PROMPT_BASE;
  system +=
    "\n\n# Voice Vibe (verbatim)\n" +
    `IDENTITY.vibe: ${options.identityVibe}\n` +
    `SOUL.vibe: ${options.soulVibe}\n`;
  system += "\n\n# Blueprint\n" + options.blueprintContext.trim() + "\n";

  if (options.bootstrapInstructions) {
    system += "\n\n# Bootstrap\n" + options.bootstrapInstructions.trim() + "\n";
  }

  system += "\n\n" + TOOL_PROMPT;

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

  constructor(ctx: AgentContext, env: Env) {
    super(ctx, env);
    this.sql`create table if not exists autopilot_blueprint_state (
      id text primary key,
      json text not null,
      updated_at integer not null
    )`;
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

  private ensureBlueprintState(): AutopilotBlueprintStateV1 {
    const existing = this.loadBlueprintState();
    if (existing) {
      const migrated = this.maybeMigrateBlueprintState(existing);
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
        const buildSystem = (state: AutopilotBlueprintStateV1) => {
          const blueprintContext = renderBlueprintContext(state, {
            includeToolsDoc: true
          });
          const bootstrapInstructions = renderBootstrapInstructions(state);
          return buildSystemPrompt({
            identityVibe: state.docs.identity.vibe,
            soulVibe: state.docs.soul.vibe,
            blueprintContext,
            bootstrapInstructions
          });
        };

        const tools: ToolSet = {
          ...BASE_TOOLS,
          identity_update: tool({
            description: "Update Identity fields in the Blueprint.",
            inputSchema: jsonSchema({
              type: "object",
              properties: {
                name: { type: "string" },
                creature: { type: "string" },
                vibe: { type: "string" },
                emoji: { type: "string" },
                avatar: { type: "string" }
              },
              additionalProperties: false
            }),
            strict: true,
            inputExamples: [
              { input: { name: "Autopilot" } },
              { input: { vibe: "calm, direct, terminal-like" } }
            ],
            execute: async (input: {
              name?: string;
              creature?: string;
              vibe?: string;
              emoji?: string;
              avatar?: string;
            }) => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const identity = state.docs.identity;
                const nextIdentity = IdentityDoc.make({
                  ...identity,
                  version: DocVersion.make(Number(identity.version) + 1),
                  name: input.name ?? identity.name,
                  creature: input.creature ?? identity.creature,
                  vibe: input.vibe ?? identity.vibe,
                  emoji: input.emoji ?? identity.emoji,
                  avatar: input.avatar ?? identity.avatar,
                  updatedAt: now,
                  updatedBy: "agent"
                });
                const nextDocs = BlueprintDocs.make({
                  ...state.docs,
                  identity: nextIdentity
                });
                return AutopilotBlueprintStateV1.make({
                  ...state,
                  docs: nextDocs
                });
              });
              return {
                ok: true,
                version: Number(updated.docs.identity.version)
              };
            }
          }),
          user_update: tool({
            description: "Update User fields in the Blueprint.",
            inputSchema: jsonSchema({
              type: "object",
              properties: {
                handle: {
                  type: "string",
                  description:
                    "What to call the user (nickname/handle). Not a physical address."
                },
                notes: { type: "string" },
                context: { type: "string" }
              },
              additionalProperties: false
            }),
            strict: true,
            inputExamples: [
              { input: { handle: "TimeLord" } },
              { input: { handle: "Jimbo" } }
            ],
            execute: async (input: {
              handle?: string;
              notes?: string;
              context?: string;
            }) => {
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
                return AutopilotBlueprintStateV1.make({
                  ...state,
                  docs: nextDocs
                });
              });
              return {
                ok: true,
                version: Number(updated.docs.user.version)
              };
            }
          }),
          soul_update: tool({
            description: "Update Soul fields in the Blueprint.",
            inputSchema: jsonSchema({
              type: "object",
              properties: {
                coreTruths: { type: "array", items: { type: "string" } },
                boundaries: { type: "array", items: { type: "string" } },
                vibe: { type: "string" },
                continuity: { type: "string" }
              },
              additionalProperties: false
            }),
            strict: true,
            inputExamples: [
              {
                input: {
                  boundaries: ["No web browsing.", "Ask before destructive actions."]
                }
              }
            ],
            execute: async (input: {
              coreTruths?: string[];
              boundaries?: string[];
              vibe?: string;
              continuity?: string;
            }) => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const soul = state.docs.soul;
                const nextSoul = SoulDoc.make({
                  ...soul,
                  version: DocVersion.make(Number(soul.version) + 1),
                  coreTruths: input.coreTruths ?? soul.coreTruths,
                  boundaries: input.boundaries ?? soul.boundaries,
                  vibe: input.vibe ?? soul.vibe,
                  continuity: input.continuity ?? soul.continuity,
                  updatedAt: now,
                  updatedBy: "agent"
                });
                const nextDocs = BlueprintDocs.make({
                  ...state.docs,
                  soul: nextSoul
                });
                return AutopilotBlueprintStateV1.make({
                  ...state,
                  docs: nextDocs
                });
              });
              return {
                ok: true,
                version: Number(updated.docs.soul.version)
              };
            }
          }),
          tools_update_notes: tool({
            description: "Update the Tools doc notes in the Blueprint.",
            inputSchema: jsonSchema({
              type: "object",
              properties: {
                notes: { type: "string" }
              },
              required: ["notes"],
              additionalProperties: false
            }),
            strict: true,
            inputExamples: [{ input: { notes: "Tools are DO-backed. No web browsing." } }],
            execute: async ({ notes }: { notes: string }) => {
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
          }),
          heartbeat_set_checklist: tool({
            description: "Replace the Heartbeat checklist in the Blueprint.",
            inputSchema: jsonSchema({
              type: "object",
              properties: {
                checklist: { type: "array", items: { type: "string" } }
              },
              required: ["checklist"],
              additionalProperties: false
            }),
            strict: true,
            inputExamples: [{ input: { checklist: ["Ask before deleting data."] } }],
            execute: async ({ checklist }: { checklist: string[] }) => {
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
          }),
          memory_append: tool({
            description: "Append a new Memory entry to the Blueprint.",
            inputSchema: jsonSchema({
              type: "object",
              properties: {
                kind: { type: "string", enum: ["daily", "long_term"] },
                title: { type: "string" },
                body: { type: "string" },
                visibility: { type: "string", enum: ["main_only", "all"] }
              },
              required: ["kind", "title", "body", "visibility"],
              additionalProperties: false
            }),
            strict: true,
            execute: async (input: {
              kind: "daily" | "long_term";
              title: string;
              body: string;
              visibility: "main_only" | "all";
            }) => {
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
          }),
          bootstrap_complete: tool({
            description: "Mark the Blueprint bootstrap sequence as complete.",
            inputSchema: jsonSchema({
              type: "object",
              properties: {},
              additionalProperties: false
            }),
            strict: true,
            execute: async () => {
              const updated = this.updateBlueprintState((state) => {
                const now = new Date();
                const nextBootstrapState = AutopilotBootstrapState.make({
                  ...state.bootstrapState,
                  status: "complete",
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
          }),
          blueprint_export: tool({
            description: "Return the export URL for this thread's Blueprint JSON.",
            inputSchema: jsonSchema({
              type: "object",
              properties: {},
              additionalProperties: false
            }),
            strict: true,
            execute: async () => ({
              ok: true,
              url: `/agents/chat/${this.name}/blueprint`,
              format: BLUEPRINT_FORMAT,
              formatVersion: BLUEPRINT_FORMAT_VERSION
            })
          })
        };

        const result = streamText({
          system: buildSystem(blueprint),
          prepareStep: async ({ stepNumber }) => {
            const fresh = this.ensureBlueprintState();
            const system = buildSystem(fresh);

            // During bootstrap, force at least one tool call on the first step so
            // we persist user/identity updates instead of "thinking" in text.
            if (fresh.bootstrapState.status !== "complete" && stepNumber === 0) {
              return {
                system,
                toolChoice: "required" as const,
                activeTools: [
                  "identity_update",
                  "user_update",
                  "soul_update",
                  "tools_update_notes",
                  "heartbeat_set_checklist",
                  "memory_append",
                  "bootstrap_complete"
                ] as const
              };
            }

            return { system };
          },
          messages: await convertToModelMessages(recentMessages),
          model: workersai(MODEL_ID as Parameters<typeof workersai>[0]),
          maxOutputTokens: MAX_OUTPUT_TOKENS,
          stopWhen: stepCountIs(10),
          tools,
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
