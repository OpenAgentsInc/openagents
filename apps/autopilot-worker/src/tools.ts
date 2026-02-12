import { Schema } from "effect";

import { Tool, type DseToolContract } from "@openagentsinc/dse";

import { BLUEPRINT_FORMAT, BLUEPRINT_FORMAT_VERSION } from "./blueprint";

const Handle = Schema.NonEmptyString.annotations({
  description: "What to call the user (handle/nickname). Not a physical address."
});

const AgentName = Schema.NonEmptyString.annotations({
  description: "The name the user should call the agent."
});

const Vibe = Schema.NonEmptyString.annotations({
  description: "One short phrase describing the operating vibe."
});

const HttpMethod = Schema.Literal("GET", "POST", "PUT", "PATCH", "DELETE");

const L402CacheStatus = Schema.Literal("miss", "hit", "stale", "invalid");
const L402PaymentBackend = Schema.Literal("spark", "lnd_deterministic");
const LightningTaskStatus = Schema.Literal(
  "queued",
  "approved",
  "running",
  "paid",
  "cached",
  "blocked",
  "failed",
  "completed",
);

const L402FetchInput = Schema.Struct({
  url: Schema.NonEmptyString.annotations({
    description: "L402-protected endpoint URL."
  }),
  method: Schema.optional(HttpMethod),
  headers: Schema.optional(Schema.Record({ key: Schema.String, value: Schema.String })),
  body: Schema.optional(Schema.String),
  maxSpendMsats: Schema.Number.pipe(Schema.nonNegative()).annotations({
    description: "Maximum spend cap for this fetch in millisatoshis."
  }),
  challengeHeader: Schema.optional(Schema.String),
  forceRefresh: Schema.optional(Schema.Boolean),
  scope: Schema.optional(Schema.String),
  cacheTtlMs: Schema.optional(Schema.Number.pipe(Schema.nonNegative())),
  requireApproval: Schema.optional(Schema.Boolean).annotations({
    description: "If true (default), the task is queued and requires explicit approval before any payment executes."
  }),
});

const L402FetchOutput = Schema.Struct({
  taskId: Schema.NullOr(Schema.String),
  status: Schema.Literal("queued", "completed", "cached", "blocked", "failed"),
  proofReference: Schema.NullOr(Schema.String),
  denyReason: Schema.NullOr(Schema.String),
  denyReasonCode: Schema.NullOr(Schema.String),
  host: Schema.NullOr(Schema.String),
  maxSpendMsats: Schema.NullOr(Schema.Number),
  quotedAmountMsats: Schema.NullOr(Schema.Number),
  paymentId: Schema.NullOr(Schema.String),
  amountMsats: Schema.NullOr(Schema.Number),
  responseStatusCode: Schema.NullOr(Schema.Number),
  responseContentType: Schema.NullOr(Schema.String),
  responseBytes: Schema.NullOr(Schema.Number),
  responseBodyTextPreview: Schema.NullOr(Schema.String),
  responseBodySha256: Schema.NullOr(Schema.String),
  cacheHit: Schema.Boolean,
  paid: Schema.Boolean,
  cacheStatus: Schema.NullOr(L402CacheStatus),
  paymentBackend: Schema.NullOr(L402PaymentBackend),
  approvalRequired: Schema.Boolean,
});

const L402ApproveInput = Schema.Struct({
  taskId: Schema.NonEmptyString.annotations({
    description: "Task id to approve for execution."
  })
});

const L402ApproveOutput = Schema.Struct({
  taskId: Schema.NullOr(Schema.String),
  ok: Schema.Boolean,
  changed: Schema.Boolean,
  taskStatus: Schema.NullOr(LightningTaskStatus),
  denyReason: Schema.NullOr(Schema.String),
});

const PaywallStatus = Schema.Literal("active", "paused", "archived");

const PaywallPolicyInput = Schema.Struct({
  pricingMode: Schema.Literal("fixed"),
  fixedAmountMsats: Schema.Number.pipe(Schema.nonNegative()),
  maxPerRequestMsats: Schema.optional(Schema.Number.pipe(Schema.nonNegative())),
  allowedHosts: Schema.optional(Schema.Array(Schema.String)),
  blockedHosts: Schema.optional(Schema.Array(Schema.String)),
  quotaPerMinute: Schema.optional(Schema.Number.pipe(Schema.nonNegative())),
  quotaPerDay: Schema.optional(Schema.Number.pipe(Schema.nonNegative())),
  killSwitch: Schema.optional(Schema.Boolean),
});

const PaywallRouteInput = Schema.Struct({
  hostPattern: Schema.String,
  pathPattern: Schema.String,
  upstreamUrl: Schema.String,
  protocol: Schema.optional(Schema.Literal("http", "https")),
  timeoutMs: Schema.optional(Schema.Number.pipe(Schema.nonNegative())),
  priority: Schema.optional(Schema.Number),
});

const PaywallSummary = Schema.Struct({
  paywallId: Schema.String,
  ownerId: Schema.NullOr(Schema.String),
  name: Schema.NullOr(Schema.String),
  status: Schema.NullOr(PaywallStatus),
  fixedAmountMsats: Schema.NullOr(Schema.Number),
  routeCount: Schema.Number.pipe(Schema.nonNegative()),
});

const SettlementSummary = Schema.Struct({
  settlementId: Schema.String,
  paywallId: Schema.NullOr(Schema.String),
  amountMsats: Schema.NullOr(Schema.Number),
  paymentProofRef: Schema.NullOr(Schema.String),
  createdAtMs: Schema.NullOr(Schema.Number),
});

const PaywallToolDenyCode = Schema.Literal(
  "not_configured",
  "not_authorized",
  "forbidden",
  "not_found",
  "invalid_input",
  "invalid_route",
  "paused",
  "over_cap",
  "policy_violation",
  "rate_limited",
  "unknown_denial",
);

const PaywallToolSideEffect = Schema.Struct({
  kind: Schema.Literal("http_request"),
  target: Schema.String,
  method: Schema.String,
  status_code: Schema.NullOr(Schema.Number),
  changed: Schema.NullOr(Schema.Boolean),
  detail: Schema.NullOr(Schema.String),
});

const PaywallToolReceipt = Schema.Struct({
  params_hash: Schema.String,
  output_hash: Schema.String,
  latency_ms: Schema.Number.pipe(Schema.nonNegative()),
  side_effects: Schema.Array(PaywallToolSideEffect),
});

const PaywallToolStatus = Schema.Literal("ok", "denied", "error");

const PaywallToolOutput = Schema.Struct({
  status: PaywallToolStatus,
  denyCode: Schema.NullOr(PaywallToolDenyCode),
  denyReason: Schema.NullOr(Schema.String),
  errorCode: Schema.NullOr(Schema.String),
  errorMessage: Schema.NullOr(Schema.String),
  httpStatus: Schema.NullOr(Schema.Number),
  requestId: Schema.NullOr(Schema.String),
  paywall: Schema.NullOr(PaywallSummary),
  paywalls: Schema.Array(PaywallSummary),
  settlements: Schema.Array(SettlementSummary),
  nextCursor: Schema.NullOr(Schema.Number),
  receipt: PaywallToolReceipt,
});

const PaywallCreateInput = Schema.Struct({
  name: Schema.NonEmptyString,
  description: Schema.optional(Schema.String),
  status: Schema.optional(Schema.Literal("active", "paused")),
  policy: PaywallPolicyInput,
  routes: Schema.Array(PaywallRouteInput),
  metadata: Schema.optional(Schema.Unknown),
});

const PaywallUpdateInput = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  name: Schema.optional(Schema.NonEmptyString),
  description: Schema.optional(Schema.String),
  policy: Schema.optional(PaywallPolicyInput),
  routes: Schema.optional(Schema.Array(PaywallRouteInput)),
  metadata: Schema.optional(Schema.Unknown),
});

const PaywallLifecycleInput = Schema.Struct({
  paywallId: Schema.NonEmptyString,
  reason: Schema.optional(Schema.String),
});

const PaywallGetInput = Schema.Struct({
  paywallId: Schema.NonEmptyString,
});

const PaywallListInput = Schema.Struct({
  status: Schema.optional(PaywallStatus),
  limit: Schema.optional(Schema.Number.pipe(Schema.nonNegative())),
});

const PaywallSettlementListInput = Schema.Struct({
  paywallId: Schema.optional(Schema.NonEmptyString),
  limit: Schema.optional(Schema.Number.pipe(Schema.nonNegative())),
  beforeCreatedAtMs: Schema.optional(Schema.Number),
});

export const toolContracts = {
  get_time: Tool.make({
    name: "get_time",
    description:
      "Return the current time. Use this tool whenever the user asks you to use a tool but doesn't specify which one.",
    usage: "get_time({ timeZone? }) -> current time",
    input: Schema.Struct({
      timeZone: Schema.optional(
        Schema.String.annotations({
          description: "Optional IANA time zone name (e.g. 'UTC', 'America/Chicago')."
        })
      )
    }),
    output: Schema.Struct({
      iso: Schema.String,
      epochMs: Schema.Number,
      epochSec: Schema.Number,
      formatted: Schema.optional(Schema.String),
      timeZone: Schema.optional(Schema.String)
    })
  }),

  echo: Tool.make({
    name: "echo",
    description: "Echo back the provided text. Useful for testing tool calling.",
    usage: "echo({ text }) -> echoes input",
    input: Schema.Struct({
      text: Schema.String.annotations({ description: "Text to echo back verbatim." })
    }),
    output: Schema.Struct({ text: Schema.String })
  }),

  lightning_l402_fetch: Tool.make({
    name: "lightning_l402_fetch",
    description:
      "Queue an L402 paid fetch via the Lightning control-plane and wait for a terminal task status. This tool never executes wallet payment in the web worker directly.",
    usage:
      "lightning_l402_fetch({ url, method?, headers?, body?, maxSpendMsats, challengeHeader?, forceRefresh?, scope?, cacheTtlMs?, requireApproval? }) -> task status + proof/deny fields (or queued + approvalRequired)",
    input: L402FetchInput,
    output: L402FetchOutput,
  }),

  lightning_l402_approve: Tool.make({
    name: "lightning_l402_approve",
    description:
      "Approve a queued L402 fetch task for execution by the desktop executor. This tool never executes wallet payment in the web worker directly.",
    usage: "lightning_l402_approve({ taskId }) -> { ok, changed, taskStatus }",
    input: L402ApproveInput,
    output: L402ApproveOutput,
  }),

  lightning_paywall_create: Tool.make({
    name: "lightning_paywall_create",
    description:
      "Create a hosted L402 paywall through the Lightning control-plane.",
    usage:
      "lightning_paywall_create({ name, description?, status?, policy, routes, metadata? }) -> paywall lifecycle status + deterministic receipt fields",
    input: PaywallCreateInput,
    output: PaywallToolOutput,
  }),

  lightning_paywall_update: Tool.make({
    name: "lightning_paywall_update",
    description:
      "Update an existing hosted L402 paywall through the Lightning control-plane.",
    usage:
      "lightning_paywall_update({ paywallId, name?, description?, policy?, routes?, metadata? }) -> paywall lifecycle status + deterministic receipt fields",
    input: PaywallUpdateInput,
    output: PaywallToolOutput,
  }),

  lightning_paywall_pause: Tool.make({
    name: "lightning_paywall_pause",
    description:
      "Pause an existing hosted L402 paywall.",
    usage:
      "lightning_paywall_pause({ paywallId, reason? }) -> paywall lifecycle status + deterministic receipt fields",
    input: PaywallLifecycleInput,
    output: PaywallToolOutput,
  }),

  lightning_paywall_resume: Tool.make({
    name: "lightning_paywall_resume",
    description:
      "Resume a paused hosted L402 paywall.",
    usage:
      "lightning_paywall_resume({ paywallId, reason? }) -> paywall lifecycle status + deterministic receipt fields",
    input: PaywallLifecycleInput,
    output: PaywallToolOutput,
  }),

  lightning_paywall_get: Tool.make({
    name: "lightning_paywall_get",
    description:
      "Get a hosted L402 paywall by id.",
    usage:
      "lightning_paywall_get({ paywallId }) -> paywall lifecycle status + deterministic receipt fields",
    input: PaywallGetInput,
    output: PaywallToolOutput,
  }),

  lightning_paywall_list: Tool.make({
    name: "lightning_paywall_list",
    description:
      "List hosted L402 paywalls for the current owner scope.",
    usage:
      "lightning_paywall_list({ status?, limit? }) -> paywall list status + deterministic receipt fields",
    input: PaywallListInput,
    output: PaywallToolOutput,
  }),

  lightning_paywall_settlement_list: Tool.make({
    name: "lightning_paywall_settlement_list",
    description:
      "List owner settlements or settlements for a specific hosted paywall.",
    usage:
      "lightning_paywall_settlement_list({ paywallId?, limit?, beforeCreatedAtMs? }) -> settlement list status + deterministic receipt fields",
    input: PaywallSettlementListInput,
    output: PaywallToolOutput,
  }),

  bootstrap_set_user_handle: Tool.make({
    name: "bootstrap_set_user_handle",
    description:
      "Bootstrap step: set what to call the user (a handle/nickname), persist it, and advance bootstrap stage.",
    usage:
      "bootstrap_set_user_handle({ handle }) -> (bootstrap) set what to call the user",
    input: Schema.Struct({ handle: Handle }),
    output: Schema.Struct({
      ok: Schema.Boolean,
      handle: Schema.String,
      stage: Schema.NullOr(Schema.String)
    })
  }),

  bootstrap_set_agent_name: Tool.make({
    name: "bootstrap_set_agent_name",
    description:
      "Bootstrap step: set what to call the agent (identity name), persist it, and advance bootstrap stage.",
    usage:
      "bootstrap_set_agent_name({ name }) -> (bootstrap) set what to call the agent",
    input: Schema.Struct({ name: AgentName }),
    output: Schema.Struct({
      ok: Schema.Boolean,
      name: Schema.String,
      stage: Schema.NullOr(Schema.String)
    })
  }),

  bootstrap_set_agent_vibe: Tool.make({
    name: "bootstrap_set_agent_vibe",
    description:
      "Bootstrap step: set the agent operating vibe, persist it, and advance bootstrap stage.",
    usage:
      "bootstrap_set_agent_vibe({ vibe }) -> (bootstrap) set the agent's operating vibe",
    input: Schema.Struct({ vibe: Vibe }),
    output: Schema.Struct({
      ok: Schema.Boolean,
      vibe: Schema.String,
      stage: Schema.NullOr(Schema.String)
    })
  }),

  identity_update: Tool.make({
    name: "identity_update",
    description: "Update Identity fields in the Blueprint.",
    usage:
      "identity_update({ name?, creature?, vibe?, emoji?, avatar? }) -> update your Identity doc",
    input: Schema.Struct({
      name: Schema.optional(Schema.String),
      creature: Schema.optional(Schema.String),
      vibe: Schema.optional(Schema.String),
      emoji: Schema.optional(Schema.String),
      avatar: Schema.optional(Schema.String)
    }),
    inputExamples: [
      { input: { name: "Autopilot" } },
      { input: { vibe: "calm, direct, terminal-like" } }
    ],
    output: Schema.Struct({ ok: Schema.Boolean, version: Schema.Number })
  }),

  user_update: Tool.make({
    name: "user_update",
    description: "Update User fields in the Blueprint.",
    usage:
      "user_update({ handle?, notes?, context? }) -> update the User doc (handle = what to call the user; not a postal address)",
    input: Schema.Struct({
      handle: Schema.optional(Handle),
      notes: Schema.optional(Schema.String),
      context: Schema.optional(Schema.String)
    }),
    inputExamples: [{ input: { handle: "TimeLord" } }, { input: { handle: "Jimbo" } }],
    output: Schema.Struct({ ok: Schema.Boolean, version: Schema.Number })
  }),

  character_update: Tool.make({
    name: "character_update",
    description: "Update Character fields in the Blueprint.",
    usage:
      "character_update({ coreTruths?, boundaries?, vibe?, continuity? }) -> update the Character doc",
    input: Schema.Struct({
      coreTruths: Schema.optional(Schema.Array(Schema.String)),
      boundaries: Schema.optional(Schema.Array(Schema.String)),
      vibe: Schema.optional(Schema.String),
      continuity: Schema.optional(Schema.String)
    }),
    inputExamples: [
      {
        input: {
          boundaries: ["No web browsing.", "Ask before destructive actions."]
        }
      }
    ],
    output: Schema.Struct({ ok: Schema.Boolean, version: Schema.Number })
  }),

  tools_update_notes: Tool.make({
    name: "tools_update_notes",
    description: "Update the Tools doc notes in the Blueprint.",
    usage: "tools_update_notes({ notes }) -> update the Tools doc",
    input: Schema.Struct({
      notes: Schema.NonEmptyString.annotations({
        description: "A short note about the available tools or tool limitations."
      })
    }),
    inputExamples: [{ input: { notes: "Tools are DO-backed. No web browsing." } }],
    output: Schema.Struct({ ok: Schema.Boolean, version: Schema.Number })
  }),

  heartbeat_set_checklist: Tool.make({
    name: "heartbeat_set_checklist",
    description: "Replace the Heartbeat checklist in the Blueprint.",
    usage:
      "heartbeat_set_checklist({ checklist }) -> update the Heartbeat doc",
    input: Schema.Struct({
      checklist: Schema.Array(Schema.String)
    }),
    inputExamples: [{ input: { checklist: ["Ask before deleting data."] } }],
    output: Schema.Struct({ ok: Schema.Boolean, version: Schema.Number })
  }),

  memory_append: Tool.make({
    name: "memory_append",
    description: "Append a new Memory entry to the Blueprint.",
    usage:
      "memory_append({ kind, title, body, visibility }) -> append a Memory entry",
    input: Schema.Struct({
      kind: Schema.Literal("daily", "long_term"),
      title: Schema.NonEmptyString,
      body: Schema.NonEmptyString,
      visibility: Schema.Literal("main_only", "all")
    }),
    output: Schema.Struct({ ok: Schema.Boolean, count: Schema.Number })
  }),

  bootstrap_complete: Tool.make({
    name: "bootstrap_complete",
    description: "Mark the Blueprint bootstrap sequence as complete.",
    usage: "bootstrap_complete({}) -> mark bootstrap complete",
    input: Schema.Struct({}),
    output: Schema.Struct({ ok: Schema.Boolean, status: Schema.String })
  }),

  blueprint_export: Tool.make({
    name: "blueprint_export",
    description: "Return the export URL for this thread's Blueprint JSON.",
    usage: "blueprint_export({}) -> get a URL to export your Blueprint",
    input: Schema.Struct({}),
    output: Schema.Struct({
      ok: Schema.Boolean,
      url: Schema.String,
      format: Schema.Literal(BLUEPRINT_FORMAT),
      formatVersion: Schema.Literal(BLUEPRINT_FORMAT_VERSION)
    })
  })
} satisfies Record<string, DseToolContract<any, any>>;

export type AutopilotToolName = keyof typeof toolContracts;

export const PAYWALL_TOOL_NAMES = [
  "lightning_paywall_create",
  "lightning_paywall_update",
  "lightning_paywall_pause",
  "lightning_paywall_resume",
  "lightning_paywall_get",
  "lightning_paywall_list",
  "lightning_paywall_settlement_list",
] as const satisfies ReadonlyArray<AutopilotToolName>;

export const BASE_TOOL_NAMES = [
  "get_time",
  "echo",
  "lightning_l402_fetch",
  "lightning_l402_approve",
  ...PAYWALL_TOOL_NAMES,
] as const satisfies ReadonlyArray<AutopilotToolName>;

export const BLUEPRINT_TOOL_NAMES = [
  "identity_update",
  "user_update",
  "character_update",
  "tools_update_notes",
  "heartbeat_set_checklist",
  "memory_append",
  "blueprint_export"
] as const satisfies ReadonlyArray<AutopilotToolName>;

// Stable ordering for prompt rendering (and future UI usage).
export const TOOL_ORDER = [
  "get_time",
  "echo",
  "lightning_l402_fetch",
  "lightning_l402_approve",
  "lightning_paywall_create",
  "lightning_paywall_update",
  "lightning_paywall_pause",
  "lightning_paywall_resume",
  "lightning_paywall_get",
  "lightning_paywall_list",
  "lightning_paywall_settlement_list",
  "bootstrap_set_user_handle",
  "bootstrap_set_agent_name",
  "bootstrap_set_agent_vibe",
  "identity_update",
  "user_update",
  "character_update",
  "tools_update_notes",
  "heartbeat_set_checklist",
  "memory_append",
  "bootstrap_complete",
  "blueprint_export"
] as const satisfies ReadonlyArray<AutopilotToolName>;

export const TOOL_RULES_PROMPT =
  "Tool use rules:\n" +
  "- If the user asks you to use a tool, you MUST call an appropriate tool.\n" +
  "- During bootstrap, prefer the bootstrap_* tools.\n" +
  "- After using tools, ALWAYS send a normal assistant reply with user-visible text.\n" +
  "- Always include a user-visible text reply. Never output reasoning-only.\n" +
  "- Never claim you have tools you do not have.\n" +
  "- If the user asks you to search/browse the web, be explicit that you currently cannot.\n";

export function renderToolPrompt(options?: {
  readonly toolNames?: ReadonlyArray<string>;
}): string {
  const toolNames =
    options?.toolNames && options.toolNames.length > 0
      ? options.toolNames
      : TOOL_ORDER;

  const renderedLines: Array<string> = [];
  for (const name of toolNames) {
    const contract = (toolContracts as any)[name] as DseToolContract<any, any> | undefined;
    if (!contract) {
      renderedLines.push(`- ${name}`);
      continue;
    }
    const line = contract.usage ? `- ${contract.usage}` : `- ${contract.name} -> ${contract.description}`;
    renderedLines.push(line);
  }

  return "Tools available:\n" + renderedLines.join("\n") + "\n\n" + TOOL_RULES_PROMPT;
}

export function toolContractsExport(): ReadonlyArray<{
  readonly name: string;
  readonly description: string;
  readonly usage?: string;
  readonly inputSchemaJson: unknown;
  readonly outputSchemaJson: unknown | null;
}> {
  return TOOL_ORDER.map((name) => {
    const c = toolContracts[name] as DseToolContract<any, any>;
    return {
      name: c.name,
      description: c.description,
      ...(c.usage ? { usage: c.usage } : {}),
      inputSchemaJson: Tool.inputJsonSchema(c),
      outputSchemaJson: Tool.outputJsonSchema(c)
    };
  });
}
