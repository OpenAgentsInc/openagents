import { Schema } from "effect";

export const BLUEPRINT_FORMAT = "openagents.autopilot.blueprint" as const;
export const BLUEPRINT_FORMAT_VERSION = 1 as const;

export const UserId = Schema.String.pipe(Schema.brand("UserId"));
export type UserId = typeof UserId.Type;

export const ThreadId = Schema.String.pipe(Schema.brand("ThreadId"));
export type ThreadId = typeof ThreadId.Type;

export const DocVersion = Schema.Int.pipe(
  Schema.positive(),
  Schema.brand("DocVersion")
);
export type DocVersion = typeof DocVersion.Type;

export const BootstrapStatus = Schema.Literal(
  "pending",
  "in_progress",
  "complete"
);
export type BootstrapStatus = typeof BootstrapStatus.Type;

export const UpdatedBy = Schema.Literal("user", "agent");
export type UpdatedBy = typeof UpdatedBy.Type;

export class AgentRulesDoc extends Schema.Class<AgentRulesDoc>("AgentRulesDoc")({
  version: DocVersion,
  body: Schema.String
}) {}

export class BootstrapRitualTemplate extends Schema.Class<BootstrapRitualTemplate>(
  "BootstrapRitualTemplate"
)({
  version: DocVersion,
  body: Schema.String
}) {}

export class IdentityDoc extends Schema.Class<IdentityDoc>("IdentityDoc")({
  version: DocVersion,
  name: Schema.String,
  creature: Schema.String,
  vibe: Schema.String,
  emoji: Schema.String,
  avatar: Schema.optional(Schema.String),
  updatedAt: Schema.DateFromString,
  updatedBy: UpdatedBy
}) {}

export class UserDoc extends Schema.Class<UserDoc>("UserDoc")({
  version: DocVersion,
  name: Schema.String,
  addressAs: Schema.String,
  pronouns: Schema.optional(Schema.String),
  timeZone: Schema.optional(Schema.String),
  notes: Schema.optional(Schema.String),
  context: Schema.optional(Schema.String),
  updatedAt: Schema.DateFromString,
  updatedBy: UpdatedBy
}) {}

export class SoulDoc extends Schema.Class<SoulDoc>("SoulDoc")({
  version: DocVersion,
  coreTruths: Schema.Array(Schema.String),
  boundaries: Schema.Array(Schema.String),
  vibe: Schema.String,
  continuity: Schema.String,
  updatedAt: Schema.DateFromString,
  updatedBy: UpdatedBy
}) {}

export class ToolsDoc extends Schema.Class<ToolsDoc>("ToolsDoc")({
  version: DocVersion,
  notes: Schema.String,
  updatedAt: Schema.DateFromString,
  updatedBy: UpdatedBy
}) {}

export class HeartbeatDoc extends Schema.Class<HeartbeatDoc>("HeartbeatDoc")({
  version: DocVersion,
  checklist: Schema.Array(Schema.String),
  updatedAt: Schema.DateFromString,
  updatedBy: UpdatedBy
}) {}

export const MemoryKind = Schema.Literal("daily", "long_term");
export type MemoryKind = typeof MemoryKind.Type;

export const MemoryVisibility = Schema.Literal("main_only", "all");
export type MemoryVisibility = typeof MemoryVisibility.Type;

export const MemoryEntryId = Schema.String.pipe(Schema.brand("MemoryEntryId"));
export type MemoryEntryId = typeof MemoryEntryId.Type;

export class MemoryEntry extends Schema.Class<MemoryEntry>("MemoryEntry")({
  id: MemoryEntryId,
  createdAt: Schema.DateFromString,
  kind: MemoryKind,
  title: Schema.String,
  body: Schema.String,
  visibility: MemoryVisibility
}) {}

export class AutopilotBootstrapState extends Schema.Class<AutopilotBootstrapState>(
  "AutopilotBootstrapState"
)({
  userId: UserId,
  threadId: ThreadId,
  status: BootstrapStatus,
  startedAt: Schema.optional(Schema.DateFromString),
  completedAt: Schema.optional(Schema.DateFromString),
  ritualVersion: Schema.Int
}) {}

export class BlueprintDocs extends Schema.Class<BlueprintDocs>("BlueprintDocs")({
  rules: AgentRulesDoc,
  ritual: BootstrapRitualTemplate,
  identity: IdentityDoc,
  user: UserDoc,
  soul: SoulDoc,
  tools: ToolsDoc,
  heartbeat: HeartbeatDoc
}) {}

export class AutopilotBlueprintStateV1 extends Schema.Class<AutopilotBlueprintStateV1>(
  "AutopilotBlueprintStateV1"
)({
  bootstrapState: AutopilotBootstrapState,
  docs: BlueprintDocs,
  memory: Schema.Array(MemoryEntry),
  audit: Schema.optional(Schema.Array(Schema.Unknown))
}) {}

export class AutopilotBlueprintV1 extends Schema.Class<AutopilotBlueprintV1>(
  "AutopilotBlueprintV1"
)({
  format: Schema.Literal(BLUEPRINT_FORMAT),
  formatVersion: Schema.Literal(BLUEPRINT_FORMAT_VERSION),
  exportedAt: Schema.DateFromString,
  app: Schema.optional(
    Schema.Struct({
      name: Schema.Literal("autopilot"),
      version: Schema.optional(Schema.String)
    })
  ),
  bootstrapState: AutopilotBootstrapState,
  docs: BlueprintDocs,
  memory: Schema.Array(MemoryEntry),
  audit: Schema.optional(Schema.Array(Schema.Unknown))
}) {}

export function makeDefaultBlueprintState(
  threadIdRaw: string
): AutopilotBlueprintStateV1 {
  const now = new Date();
  const v1 = DocVersion.make(1);
  const threadId = ThreadId.make(threadIdRaw);
  const userId = UserId.make(threadIdRaw);

  return AutopilotBlueprintStateV1.make({
    bootstrapState: AutopilotBootstrapState.make({
      userId,
      threadId,
      status: "pending",
      ritualVersion: 1
    }),
    docs: BlueprintDocs.make({
      rules: AgentRulesDoc.make({
        version: v1,
        body:
          "You are Autopilot, a persistent personal AI agent.\n" +
          "Follow the Blueprint and keep it up to date using the Blueprint tools when available.\n"
      }),
      ritual: BootstrapRitualTemplate.make({
        version: v1,
        body:
          "Birth Ritual (Blueprint):\n" +
          "1) Greet the user and explain you will set up their Autopilot Blueprint.\n" +
          "2) Ask for: their name, how to address them, their time zone.\n" +
          "3) Offer to customize your own persona (name/creature/vibe/emoji) and boundaries.\n" +
          "4) As answers arrive, call the Blueprint update tools to save them.\n" +
          "5) When the user is satisfied, call bootstrap.complete().\n" +
          "Keep the ritual short and conversational.\n"
      }),
      identity: IdentityDoc.make({
        version: v1,
        name: "Autopilot",
        creature: "assistant",
        vibe: "calm, direct, pragmatic",
        emoji: ":lobster:",
        updatedAt: now,
        updatedBy: "agent"
      }),
      user: UserDoc.make({
        version: v1,
        name: "Unknown",
        addressAs: "Unknown",
        updatedAt: now,
        updatedBy: "agent"
      }),
      soul: SoulDoc.make({
        version: v1,
        coreTruths: [
          "I am your Autopilot: persistent, careful, and action-oriented.",
          "I prefer verification and small, safe steps over guesses."
        ],
        boundaries: [
          "I do not pretend to have capabilities I do not have.",
          "I will ask before taking irreversible actions."
        ],
        vibe: "helpful, concise, engineering-minded",
        continuity:
          "I keep a durable Blueprint (Identity/User/Soul/Memory) and update it when allowed.",
        updatedAt: now,
        updatedBy: "agent"
      }),
      tools: ToolsDoc.make({
        version: v1,
        notes:
          "Built-in tools only. When available, use Blueprint tools to persist identity/user/soul/memory updates.",
        updatedAt: now,
        updatedBy: "agent"
      }),
      heartbeat: HeartbeatDoc.make({
        version: v1,
        checklist: [],
        updatedAt: now,
        updatedBy: "agent"
      })
    }),
    memory: []
  });
}

const TRUNCATION_MARKER = "\n...(truncated)...\n";

export function truncateHeadTail(text: string, maxChars: number): string {
  if (maxChars <= 0) return "";
  if (text.length <= maxChars) return text;
  const keep = Math.max(0, maxChars - TRUNCATION_MARKER.length);
  const head = Math.ceil(keep / 2);
  const tail = Math.floor(keep / 2);
  return text.slice(0, head) + TRUNCATION_MARKER + text.slice(text.length - tail);
}

export function renderBlueprintContext(
  blueprint: AutopilotBlueprintStateV1,
  options: { maxCharsPerSection?: number; includeToolsDoc?: boolean } = {}
): string {
  const maxChars = options.maxCharsPerSection ?? 4000;
  const includeToolsDoc = options.includeToolsDoc ?? false;

  const sections: Array<{ title: string; body: string }> = [
    { title: "RULES", body: blueprint.docs.rules.body },
    {
      title: "IDENTITY",
      body:
        `name: ${blueprint.docs.identity.name}\n` +
        `creature: ${blueprint.docs.identity.creature}\n` +
        `vibe: ${blueprint.docs.identity.vibe}\n` +
        `emoji: ${blueprint.docs.identity.emoji}\n` +
        (blueprint.docs.identity.avatar
          ? `avatar: ${blueprint.docs.identity.avatar}\n`
          : "")
    },
    {
      title: "USER",
      body:
        `name: ${blueprint.docs.user.name}\n` +
        `addressAs: ${blueprint.docs.user.addressAs}\n` +
        (blueprint.docs.user.pronouns
          ? `pronouns: ${blueprint.docs.user.pronouns}\n`
          : "") +
        (blueprint.docs.user.timeZone
          ? `timeZone: ${blueprint.docs.user.timeZone}\n`
          : "") +
        (blueprint.docs.user.notes ? `notes: ${blueprint.docs.user.notes}\n` : "") +
        (blueprint.docs.user.context
          ? `context: ${blueprint.docs.user.context}\n`
          : "")
    },
    {
      title: "SOUL",
      body:
        `coreTruths:\n- ${blueprint.docs.soul.coreTruths.join("\n- ")}\n\n` +
        `boundaries:\n- ${blueprint.docs.soul.boundaries.join("\n- ")}\n\n` +
        `vibe: ${blueprint.docs.soul.vibe}\n` +
        `continuity: ${blueprint.docs.soul.continuity}\n`
    },
    {
      title: "HEARTBEAT",
      body:
        blueprint.docs.heartbeat.checklist.length === 0
          ? "(empty)"
          : `checklist:\n- ${blueprint.docs.heartbeat.checklist.join("\n- ")}\n`
    }
  ];

  if (includeToolsDoc) {
    sections.push({ title: "TOOLS", body: blueprint.docs.tools.notes });
  }

  if (blueprint.memory.length > 0) {
    const memoryBody = blueprint.memory
      .slice(-20)
      .map((m) => {
        const when = m.createdAt.toISOString();
        return (
          `- [${when}] (${m.kind}, ${m.visibility}) ${m.title}\n` +
          truncateHeadTail(m.body.trim(), 500)
        );
      })
      .join("\n");
    sections.push({ title: "MEMORY", body: memoryBody });
  }

  return sections
    .map(({ title, body }) => {
      const rendered = truncateHeadTail(body.trim(), maxChars);
      return `### ${title}\n${rendered}\n`;
    })
    .join("\n");
}

export function renderBootstrapRitual(
  blueprint: AutopilotBlueprintStateV1
): string | null {
  if (blueprint.bootstrapState.status === "complete") return null;
  return blueprint.docs.ritual.body.trim();
}
