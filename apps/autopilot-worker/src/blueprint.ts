import { Schema } from "effect";

export const BLUEPRINT_FORMAT = "openagents.autopilot.blueprint" as const;
export const BLUEPRINT_FORMAT_VERSION = 1 as const;

export const CURRENT_BOOTSTRAP_TEMPLATE_VERSION = 5 as const;

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

export const BootstrapStage = Schema.Literal(
  "ask_user_handle",
  "ask_agent_name",
  "ask_vibe",
  "ask_boundaries"
);
export type BootstrapStage = typeof BootstrapStage.Type;

export const UpdatedBy = Schema.Literal("user", "agent");
export type UpdatedBy = typeof UpdatedBy.Type;

export class AgentRulesDoc extends Schema.Class<AgentRulesDoc>("AgentRulesDoc")({
  version: DocVersion,
  body: Schema.String
}) {}

export class BootstrapTemplate extends Schema.Class<BootstrapTemplate>(
  "BootstrapTemplate"
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

export class CharacterDoc extends Schema.Class<CharacterDoc>("CharacterDoc")({
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
  stage: Schema.optional(BootstrapStage),
  startedAt: Schema.optional(Schema.DateFromString),
  completedAt: Schema.optional(Schema.DateFromString),
  templateVersion: Schema.Int
}) {}

export class BlueprintDocs extends Schema.Class<BlueprintDocs>("BlueprintDocs")({
  rules: AgentRulesDoc,
  bootstrap: BootstrapTemplate,
  identity: IdentityDoc,
  user: UserDoc,
  character: CharacterDoc,
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

export const DEFAULT_BOOTSTRAP_TEMPLATE_BODY =
  "Bootstrap (Blueprint):\n" +
  "\n" +
  "Keep it short. Terminal tone. No cheerleading.\n" +
  "Ask one question at a time.\n" +
  "Do not ask the user to confirm their answers. Apply the answer and move on.\n" +
  "\n" +
  "Never ask for personal info (physical address, email, phone, legal name, etc.).\n" +
  "Do not use the word \"address\". Ask \"What should I call you?\".\n" +
  "\n" +
  "Avoid filler like: \"Great\", \"Awesome\", \"No problem\", exclamation points.\n" +
  "Use acknowledgements like: \"Noted.\", \"Confirmed.\", \"Acknowledged.\".\n" +
  "\n" +
  "Goal: establish operator identity + agent identity.\n" +
  "\n" +
  "Order:\n" +
  "1) Ask what to call the user.\n" +
  "   - Call user_update({ handle: <handle> }) to persist it.\n" +
  "2) What should the user call you? (identity name; default Autopilot)\n" +
  "   - Call identity_update({ name: <name> }) to persist it.\n" +
  "3) Pick your operating vibe (one short phrase)\n" +
  "   - Call identity_update({ vibe: <vibe> }) to persist it.\n" +
  "4) Optional: any boundaries/preferences (update Character)\n" +
  "   - If they provide boundaries, call character_update({ boundaries: [ ... ] }).\n" +
  "   - If they say \"none\", call bootstrap_complete({}).\n" +
  "\n" +
  "Do not ask for time zone. Do not ask for pronouns.\n" +
  "\n" +
  "As answers arrive, call the Blueprint update tools to save them.\n" +
  "When the setup is stable, call bootstrap_complete().\n" +
  "\n" +
  "Example follow-up after the user gives a name:\n" +
  "\"Confirmed. I'll call you <handle>.\n" +
  "What should you call me?\"";

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
      stage: "ask_user_handle",
      templateVersion: CURRENT_BOOTSTRAP_TEMPLATE_VERSION
    }),
    docs: BlueprintDocs.make({
      rules: AgentRulesDoc.make({
        version: v1,
        body:
          "You are Autopilot, a persistent personal AI agent.\n" +
          "Follow the Blueprint and keep it up to date using the Blueprint tools when available.\n"
      }),
      bootstrap: BootstrapTemplate.make({
        version: v1,
        body: DEFAULT_BOOTSTRAP_TEMPLATE_BODY
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
      character: CharacterDoc.make({
        version: v1,
        coreTruths: [
          "I am your Autopilot: persistent, careful, and action-oriented.",
          "I prefer verification and small, safe steps over guesses."
        ],
        boundaries: [
          "I do not pretend to have capabilities I do not have.",
          "I will ask before taking irreversible actions.",
          "I do not ask for personal info (physical address, email, phone, legal name, etc.)."
        ],
        vibe: "helpful, concise, engineering-minded",
        continuity:
          "I keep a durable Blueprint (Identity/User/Character/Memory) and update it when allowed.",
        updatedAt: now,
        updatedBy: "agent"
      }),
      tools: ToolsDoc.make({
        version: v1,
        notes:
          "Built-in tools only. When available, use Blueprint tools to persist identity/user/character/memory updates.",
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
        `handle: ${blueprint.docs.user.addressAs}\n` +
        (blueprint.docs.user.notes ? `notes: ${blueprint.docs.user.notes}\n` : "") +
        (blueprint.docs.user.context
          ? `context: ${blueprint.docs.user.context}\n`
          : "")
    },
    {
      title: "CHARACTER",
      body:
        `coreTruths:\n- ${blueprint.docs.character.coreTruths.join("\n- ")}\n\n` +
        `boundaries:\n- ${blueprint.docs.character.boundaries.join("\n- ")}\n\n` +
        `vibe: ${blueprint.docs.character.vibe}\n` +
        `continuity: ${blueprint.docs.character.continuity}\n`
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

export function renderBootstrapInstructions(
  blueprint: AutopilotBlueprintStateV1
): string | null {
  if (blueprint.bootstrapState.status === "complete") return null;
  return blueprint.docs.bootstrap.body.trim();
}
