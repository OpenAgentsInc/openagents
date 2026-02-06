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
  BootstrapRitualTemplate,
  CURRENT_RITUAL_VERSION,
  DEFAULT_RITUAL_BODY,
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
  renderBootstrapRitual
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
    execute: async ({ text }: { text: string }) => ({ text })
  })
};

const SYSTEM_PROMPT_BASE =
  "You are Autopilot, a persistent personal AI agent.\n" +
  "\n" +
  "Voice:\n" +
  "- Calm, direct, terminal-like.\n" +
  "- No cheerleading, no filler, no exclamation points.\n" +
  "- Prefer short sentences.\n" +
  "\n" +
  "Important:\n" +
  "- Do not claim you can browse the web. You cannot.\n" +
  "- Only mention tools when the Tools section is present.\n";

const TOOL_PROMPT =
  "Tools available:\n" +
  "- get_time({ timeZone? }) -> current time\n" +
  "- echo({ text }) -> echoes input\n" +
  "- identity_update({ name?, creature?, vibe?, emoji?, avatar? }) -> update your Identity doc\n" +
  "- user_update({ name?, addressAs?, notes?, context? }) -> update the User doc\n" +
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
  "- Never claim you have tools you do not have.\n" +
  "- If the user asks you to search/browse the web, be explicit that you currently cannot.\n";

function shouldForceToolChoice(recentMessages: ReadonlyArray<unknown>) {
  // Heuristic: if the user explicitly asks to use tools, require at least one tool call.
  // This avoids "reasoning-only" replies that never actually invoke a tool.
  const lastUser = [...recentMessages].reverse().find((m: any) => m?.role === "user");
  const parts: ReadonlyArray<any> = Array.isArray((lastUser as any)?.parts)
    ? (lastUser as any).parts
    : [];
  const text = parts
    .filter((p) => p && typeof p === "object" && p.type === "text" && typeof p.text === "string")
    .map((p) => p.text)
    .join("")
    .trim();

  if (!text) return false;

  return (
    /\b(use|try|call|run)\b[\s\S]{0,40}\btools?\b/i.test(text)
  );
}

function getLastUserText(recentMessages: ReadonlyArray<unknown>) {
  const lastUser = [...recentMessages].reverse().find((m: any) => m?.role === "user");
  const parts: ReadonlyArray<any> = Array.isArray((lastUser as any)?.parts)
    ? (lastUser as any).parts
    : [];
  return parts
    .filter((p) => p && typeof p === "object" && p.type === "text" && typeof p.text === "string")
    .map((p) => p.text)
    .join("")
    .trim();
}

function shouldEnableTools(
  recentMessages: ReadonlyArray<unknown>,
  bootstrapStatus: "pending" | "in_progress" | "complete"
) {
  if (bootstrapStatus !== "complete") return true;

  const text = getLastUserText(recentMessages);
  if (!text) return false;

  // Preserve streaming + reasoning for normal chat by default. Tools are opt-in.
  return (
    shouldForceToolChoice(recentMessages) ||
    /\b(get_time|echo|blueprint|identity_update|user_update|soul_update)\b/i.test(
      text
    ) ||
    /\bwhat(?:'s| is)\s+the\s+time\b/i.test(text) ||
    /\bcurrent\s+time\b/i.test(text)
  );
}

function buildSystemPrompt(options: {
  toolsEnabled: boolean;
  blueprintContext: string;
  bootstrapRitual: string | null;
}) {
  let system = SYSTEM_PROMPT_BASE;
  system += "\n\n# Blueprint\n" + options.blueprintContext.trim() + "\n";

  if (options.bootstrapRitual) {
    system += "\n\n# Bootstrap\n" + options.bootstrapRitual.trim() + "\n";
  }

  if (options.toolsEnabled) {
    system += "\n\n" + TOOL_PROMPT;
  }

  return system;
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
      return Schema.decodeUnknownSync(AutopilotBlueprintStateV1)(parsed);
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
    const ritualVersion = Number(state.bootstrapState.ritualVersion);
    if (ritualVersion >= CURRENT_RITUAL_VERSION) return state;

    const nextRitual = BootstrapRitualTemplate.make({
      ...state.docs.ritual,
      version: DocVersion.make(Number(state.docs.ritual.version) + 1),
      body: DEFAULT_RITUAL_BODY
    });
    const nextDocs = BlueprintDocs.make({
      ...state.docs,
      ritual: nextRitual
    });
    const nextBootstrapState = AutopilotBootstrapState.make({
      ...state.bootstrapState,
      ritualVersion: CURRENT_RITUAL_VERSION
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
          decoded = Schema.decodeUnknownSync(AutopilotBlueprintV1)(body);
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
        const toolsEnabled = shouldEnableTools(
          recentMessages,
          blueprint.bootstrapState.status
        );
        const forceTool = toolsEnabled && shouldForceToolChoice(recentMessages);

        const blueprintContext = renderBlueprintContext(blueprint, {
          includeToolsDoc: toolsEnabled
        });
        const ritual = renderBootstrapRitual(blueprint);

        const system = buildSystemPrompt({
          toolsEnabled,
          blueprintContext,
          bootstrapRitual: ritual
        });

        const tools: ToolSet = toolsEnabled
          ? {
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
                    name: { type: "string" },
                    addressAs: { type: "string" },
                    pronouns: { type: "string" },
                    timeZone: { type: "string" },
                    notes: { type: "string" },
                    context: { type: "string" }
                  },
                  additionalProperties: false
                }),
                execute: async (input: {
                  name?: string;
                  addressAs?: string;
                  pronouns?: string;
                  timeZone?: string;
                  notes?: string;
                  context?: string;
                }) => {
                  const updated = this.updateBlueprintState((state) => {
                    const now = new Date();
                    const user = state.docs.user;
                    const nextUser = UserDoc.make({
                      ...user,
                      version: DocVersion.make(Number(user.version) + 1),
                      name: input.name ?? user.name,
                      addressAs: input.addressAs ?? user.addressAs,
                      pronouns: input.pronouns ?? user.pronouns,
                      timeZone: input.timeZone ?? user.timeZone,
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
                execute: async () => ({
                  ok: true,
                  url: `/agents/chat/${this.name}/blueprint`,
                  format: BLUEPRINT_FORMAT,
                  formatVersion: BLUEPRINT_FORMAT_VERSION
                })
              })
            }
          : {};

        const result = streamText({
          system,
          messages: await convertToModelMessages(recentMessages),
          model: workersai(MODEL_ID as Parameters<typeof workersai>[0]),
          maxOutputTokens: MAX_OUTPUT_TOKENS,
          stopWhen: stepCountIs(10),
          ...(toolsEnabled
            ? {
                tools,
                // When tools are enabled, keep default 'auto' unless we need to force a tool call.
                ...(forceTool ? { toolChoice: "required" as const } : {})
              }
            : {}),
          // Base class uses this callback to persist messages + stream metadata.
          onFinish: onFinish as unknown as StreamTextOnFinishCallback<ToolSet>,
          ...(options?.abortSignal ? { abortSignal: options.abortSignal } : {})
        });

        writer.merge(result.toUIMessageStream());
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
