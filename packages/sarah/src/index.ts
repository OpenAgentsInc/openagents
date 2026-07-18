import { Schema as S } from "effect";

import type { AuthorityRuntimeProfile } from "@openagentsinc/authority";

export const SARAH_PRINCIPAL_SCHEMA = "openagents.sarah.principal.v1" as const;
export const SARAH_CONTEXT_SCHEMA = "openagents.sarah.business_context.v1" as const;
export const SARAH_AUTHORITY_PROFILE_REF = "openagents.sarah-owner-orchestrator" as const;
export const SARAH_AUTHORITY_REVISION = 2 as const;
export const ROOT_AUTHORITY_PROFILE_REF = "openagents.owner-delegated-autonomy" as const;
export const ROOT_AUTHORITY_REVISION = 4 as const;

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Summary = S.String.check(S.isMaxLength(4_000));

export const SarahCapabilitySchema = S.Struct({
  capabilityRef: Ref,
  label: S.String.check(S.isMinLength(1), S.isMaxLength(80)),
  mode: S.Literals(["live", "brokered", "reserved"]),
  access: S.Literals(["read", "propose", "act", "none"]),
});
export interface SarahCapability extends S.Schema.Type<typeof SarahCapabilitySchema> {}

export const SARAH_CAPABILITIES: ReadonlyArray<SarahCapability> = [
  {
    capabilityRef: "capability.sarah.owner_conversation",
    label: "Owner conversation",
    mode: "live",
    access: "act",
  },
  {
    capabilityRef: "capability.sarah.persistent_memory",
    label: "Persistent cited memory",
    mode: "live",
    access: "act",
  },
  {
    capabilityRef: "capability.sarah.release_status",
    label: "Releases and open issues",
    mode: "live",
    access: "read",
  },
  {
    capabilityRef: "capability.sarah.forum_context",
    label: "Forum activity",
    mode: "live",
    access: "read",
  },
  {
    capabilityRef: "capability.sarah.full_auto_status",
    label: "Full Auto and fleet status",
    mode: "live",
    access: "read",
  },
  {
    capabilityRef: "capability.sarah.full_auto_control",
    label: "Full Auto run control",
    mode: "brokered",
    access: "act",
  },
  {
    capabilityRef: "capability.sarah.cloud_health",
    label: "OpenAgents Cloud health",
    mode: "live",
    access: "read",
  },
  {
    capabilityRef: "capability.sarah.repository_delivery",
    label: "Repository delivery",
    mode: "brokered",
    access: "act",
  },
  {
    capabilityRef: "capability.sarah.codex_worker_dispatch",
    label: "Owner-capacity Codex workers",
    mode: "brokered",
    access: "act",
  },
  {
    capabilityRef: "capability.sarah.rc_release",
    label: "RC release operations",
    mode: "brokered",
    access: "act",
  },
  {
    capabilityRef: "capability.sarah.company_communications",
    label: "GitHub and Forum updates",
    mode: "brokered",
    access: "act",
  },
  {
    capabilityRef: "capability.sarah.financial_custody",
    label: "Financial custody",
    mode: "reserved",
    access: "none",
  },
  {
    capabilityRef: "capability.sarah.legal_people",
    label: "Legal and employment commitments",
    mode: "reserved",
    access: "none",
  },
];

export const SarahPrincipalProjectionSchema = S.Struct({
  schema: S.Literal(SARAH_PRINCIPAL_SCHEMA),
  principalRef: S.Literal("principal.sarah"),
  displayName: S.Literal("Sarah"),
  role: S.Literal("Owner orchestrator"),
  threadRef: Ref,
  authorityProfileRef: S.Literal(SARAH_AUTHORITY_PROFILE_REF),
  authorityRevision: S.Literal(SARAH_AUTHORITY_REVISION),
  rootAuthorityProfileRef: S.Literal(ROOT_AUTHORITY_PROFILE_REF),
  rootAuthorityRevision: S.Literal(ROOT_AUTHORITY_REVISION),
  memory: S.Literals(["durable_cited", "unavailable"]),
  capabilities: S.Array(SarahCapabilitySchema),
});
export interface SarahPrincipalProjection extends S.Schema.Type<
  typeof SarahPrincipalProjectionSchema
> {}

export const SarahPrincipalApiResponseSchema = S.Struct({
  ok: S.Literal(true),
  routeRef: S.Literal("route.mobile.sarah.principal.v1"),
  principal: SarahPrincipalProjectionSchema,
});
export interface SarahPrincipalApiResponse extends S.Schema.Type<
  typeof SarahPrincipalApiResponseSchema
> {}

export const SarahContextSourceSchema = S.Struct({
  sourceRef: Ref,
  kind: S.Literals([
    "memory",
    "conversation",
    "github_release",
    "github_issue",
    "forum",
    "full_auto",
    "fleet",
    "cloud_health",
    "product_contract",
  ]),
  observedAt: S.DateTimeUtcFromString,
  freshness: S.Literals(["live", "recent", "historical", "unavailable"]),
  sensitivity: S.Literals(["public", "owner_private"]),
  summary: Summary,
});
export interface SarahContextSource extends S.Schema.Type<typeof SarahContextSourceSchema> {}

export const SarahBusinessContextSchema = S.Struct({
  schema: S.Literal(SARAH_CONTEXT_SCHEMA),
  threadRef: Ref,
  generatedAt: S.DateTimeUtcFromString,
  sources: S.Array(SarahContextSourceSchema),
});
export interface SarahBusinessContext extends S.Schema.Type<typeof SarahBusinessContextSchema> {}

export interface SarahRuntimeIdentity {
  readonly laneRef: string;
  readonly modelRef: string;
  readonly providerLabel: string;
  readonly runtimeLabel: string;
}

/** Raw provenance refs belong to the private evidence layer, not the owner's
 * conversational transcript. This is a final presentation fence in addition
 * to the model instruction, so a provider cannot leak bracketed internal refs
 * into ordinary Sarah replies. */
export const sanitizeSarahConversationResponse = (value: string): string =>
  value
    .replace(/\s*\[source\.[^\]\n]{1,512}\]/gi, "")
    .replace(/[ \t]+([.,;:!?])/g, "$1")
    .replace(/[ \t]{2,}/g, " ")
    .replace(/\n[ \t]+/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();

export const SARAH_RUNTIME_AUTHORITY_PROFILE: AuthorityRuntimeProfile = {
  profileRef: SARAH_AUTHORITY_PROFILE_REF,
  revision: SARAH_AUTHORITY_REVISION,
  lifecycle: "admitted",
  authorityMayAmplify: false,
  explicitDenyWins: true,
  grants: [
    {
      grantRef: "grant.sarah.owner_contact",
      roles: ["sarah_orchestrator"],
      actions: ["maintain_owner_contact", "read_business_context", "propose_company_decision"],
      resources: [
        "owner_private_conversation",
        "owner_business_context",
        "company_priority_ledger",
      ],
      programs: ["program.sarah_company_operations"],
      conditionRefs: ["condition.owner_scope", "condition.redaction", "condition.citations"],
    },
    {
      grantRef: "grant.sarah.delegated_operations",
      roles: ["sarah_orchestrator"],
      actions: [
        "delegate_repository_work",
        "inspect_owner_coding_capacity",
        "inspect_existing_full_auto_run",
        "dispatch_owner_capacity_coding_workers",
        "control_existing_full_auto_run",
        "operate_google_cloud",
        "publish_release_candidate",
        "communicate_release_status",
      ],
      resources: [
        "OpenAgentsInc/openagents",
        "owner_linked_pylon_coding_capacity",
        "owner_full_auto_runs",
        "google_cloud_project_openagentsgemini",
        "openagents_rc_release_channel",
        "openagents_github_and_forum",
      ],
      programs: ["program.sarah_company_operations"],
      conditionRefs: [
        "condition.owner_scope",
        "condition.redaction",
        "condition.existing_runtime_gate",
        "condition.rollback",
      ],
    },
  ],
  reservedActions: [
    "increase_own_authority",
    "export_secret",
    "move_financial_value",
    "execute_legal_commitment",
    "make_employment_decision",
    "destroy_customer_data",
    "publish_stable_release_without_direction",
    "weaken_invariant",
    "make_unsupported_public_claim",
  ],
};

export const buildSarahSystemPrompt = (
  context: SarahBusinessContext,
  runtimeIdentity: SarahRuntimeIdentity,
): string => {
  const evidence = context.sources
    .map(
      (source, index) =>
        `- Context ${index + 1} (${source.kind}, ${source.freshness}): ${source.summary}`,
    )
    .join("\n");
  return [
    "You are Sarah, OpenAgents' owner orchestrator and the owner's single point of contact.",
    "Be warm, direct, concise, and conversational. Answer the owner's actual message instead of volunteering an operations briefing.",
    "For a greeting or brief conversational message, reply naturally in one or two sentences. Do not introduce yourself, summarize the company, list active work, or recommend next actions unless asked.",
    "Default to under 120 words. Give a longer status report, audit, or action list only when the owner explicitly asks for that detail.",
    "Use only the supplied, owner-scoped business context for current-state claims.",
    `Authoritative runtime identity for this exact reply: model ${runtimeIdentity.modelRef}; provider ${runtimeIdentity.providerLabel}; runtime ${runtimeIdentity.runtimeLabel}; lane ${runtimeIdentity.laneRef}.`,
    "If the owner asks what powers this response, repeat that runtime identity exactly and briefly. Never infer the current model, provider, runtime, or backend from business context, fleet status, prior messages, or your own generated prose, and never claim a different one.",
    "Provenance is retained in the private context layer. Never print raw source refs, internal IDs, UUIDs, contract refs, fleet-run refs, or bracketed citations in conversational prose. If the owner asks for evidence, use readable issue numbers, titles, and normal links available in context.",
    "Never invent a source or imply an action ran when it did not.",
    "You may recommend and prioritize broadly. Mutations still travel through typed capability brokers and the admitted authority profile.",
    "You have real tools for reading owner-linked coding capacity, dispatching bounded Codex workers against an exact public OpenAgents commit, reading their status, reading the current Full Auto projection, and dispatching pause/resume/stop intents for an existing Full Auto run. Use those tools when the owner asks you to act or when current state is required; never claim dispatch, application, or completion until the corresponding tool result says it happened.",
    "A pending Full Auto control intent is queued for Desktop application, not completed. Starting a new Full Auto run, editing its harness modules, reading the private experience bank, adapting during a run, and promoting a harness candidate are not available tools in this revision.",
    "Never request, reveal, or reproduce raw credentials, secrets, mnemonics, private paths, or customer-private payloads.",
    "Financial custody, legal/employment commitments, destructive customer-data actions, invariant weakening, self-amplification, unsupported public claims, and stable releases without current direction remain reserved.",
    "The public /sarah web surface and avatar remain retired. You live inside authenticated OpenAgents surfaces.",
    "When evidence is absent or stale, say exactly that and propose the narrowest next action.",
    "\nPrivate reference context. Use only what is relevant to the owner's request; do not summarize this block by default:\n" +
      evidence,
  ].join("\n");
};

export const decodeSarahPrincipalApiResponse = (value: unknown) =>
  S.decodeUnknownSync(SarahPrincipalApiResponseSchema)(value, {
    onExcessProperty: "error",
  });
