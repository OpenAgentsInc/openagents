import { Schema as S } from "effect";

import type { AuthorityRuntimeProfile } from "@openagentsinc/authority";

export const SARAH_PRINCIPAL_SCHEMA = "openagents.sarah.principal.v1" as const;
export const SARAH_CONTEXT_SCHEMA = "openagents.sarah.business_context.v1" as const;
export const SARAH_AUTHORITY_PROFILE_REF = "openagents.sarah-owner-orchestrator" as const;
export const SARAH_AUTHORITY_REVISION = 1 as const;
export const ROOT_AUTHORITY_PROFILE_REF = "openagents.owner-delegated-autonomy" as const;
export const ROOT_AUTHORITY_REVISION = 3 as const;

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
        "operate_google_cloud",
        "publish_release_candidate",
        "communicate_release_status",
      ],
      resources: [
        "OpenAgentsInc/openagents",
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

export const buildSarahSystemPrompt = (context: SarahBusinessContext): string => {
  const evidence = context.sources
    .map((source) => `- [${source.sourceRef}] ${source.summary}`)
    .join("\n");
  return [
    "You are Sarah, OpenAgents' owner orchestrator and the owner's single point of contact.",
    "You are an AI. Be direct, decisive, compact, and honest about what is observed versus unavailable.",
    "Use only the supplied, owner-scoped business context for current-state claims.",
    "Cite current-state claims inline with their exact [source.ref]. Never invent a source or imply an action ran when it did not.",
    "You may recommend and prioritize broadly. Mutations still travel through typed capability brokers and the admitted authority profile.",
    "Never request, reveal, or reproduce raw credentials, secrets, mnemonics, private paths, or customer-private payloads.",
    "Financial custody, legal/employment commitments, destructive customer-data actions, invariant weakening, self-amplification, unsupported public claims, and stable releases without current direction remain reserved.",
    "The public /sarah web surface and avatar remain retired. You live inside authenticated OpenAgents surfaces.",
    "When evidence is absent or stale, say exactly that and propose the narrowest next action.",
    "\nCurrent cited context:\n" + evidence,
  ].join("\n");
};

export const decodeSarahPrincipalApiResponse = (value: unknown) =>
  S.decodeUnknownSync(SarahPrincipalApiResponseSchema)(value, {
    onExcessProperty: "error",
  });
