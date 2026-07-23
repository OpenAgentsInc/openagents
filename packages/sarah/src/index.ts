import { Schema as S } from "effect";

import type { AuthorityRuntimeProfile } from "@openagentsinc/authority";

export const SARAH_PRINCIPAL_SCHEMA = "openagents.sarah.principal.v1" as const;
export const SARAH_CONTEXT_SCHEMA = "openagents.sarah.business_context.v1" as const;
export const SARAH_HARNESS_POLICY_SCHEMA = "openagents.sarah.harness_policy.v1" as const;
export const SARAH_AUTHORITY_PROFILE_REF = "openagents.sarah-owner-orchestrator" as const;
export const SARAH_AUTHORITY_REVISION = 6 as const;
export const ROOT_AUTHORITY_PROFILE_REF = "openagents.owner-delegated-autonomy" as const;
export const ROOT_AUTHORITY_REVISION = 8 as const;

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const Summary = S.String.check(S.isMaxLength(4_000));
const HarnessInstruction = S.Trim.check(S.isMinLength(1), S.isMaxLength(500));

export const SarahHarnessDimensionsSchema = S.Struct({
  contextAssembly: Ref,
  toolInteraction: Ref,
  generationControl: Ref,
  orchestration: Ref,
  memoryManagement: Ref,
  outputProcessing: Ref,
});
export interface SarahHarnessDimensions extends S.Schema.Type<
  typeof SarahHarnessDimensionsSchema
> {}

export const SarahHarnessPolicySchema = S.Struct({
  schema: S.Literal(SARAH_HARNESS_POLICY_SCHEMA),
  dimensions: SarahHarnessDimensionsSchema,
  conversationInstructions: S.Array(HarnessInstruction).check(S.isMinLength(1), S.isMaxLength(8)),
  maxReplyWords: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(40),
    S.isLessThanOrEqualTo(240),
  ),
});
export interface SarahHarnessPolicy extends S.Schema.Type<typeof SarahHarnessPolicySchema> {}

/** Released baseline for Sarah. Candidate policies may change only the bounded
 * conversational instructions and word ceiling; the six dimension identities
 * and all authority-bearing runtime contracts remain outside this schema. */
export const DEFAULT_SARAH_HARNESS_POLICY: SarahHarnessPolicy = S.decodeUnknownSync(
  SarahHarnessPolicySchema,
)({
  schema: SARAH_HARNESS_POLICY_SCHEMA,
  dimensions: {
    contextAssembly: "harness.sarah.context.owner-private.v1",
    toolInteraction: "harness.sarah.tools.receipt-first.v1",
    generationControl: "harness.sarah.generation.conversational.v1",
    orchestration: "harness.sarah.orchestration.bounded.v1",
    memoryManagement: "harness.sarah.memory.terminal-history.v1",
    outputProcessing: "harness.sarah.output.private-ref-fence.v1",
  },
  conversationInstructions: [
    "Answer the owner's current message directly before adding context.",
    "Keep ordinary conversation brief and explain active work in plain language.",
    "Never expose internal refs, raw provenance syntax, or private runtime plumbing.",
    "State the authoritative runtime identity exactly when asked what powers a reply.",
  ],
  maxReplyWords: 120,
});

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
    capabilityRef: "capability.sarah.harness_learning",
    label: "Private harness learning",
    mode: "brokered",
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
    capabilityRef: "capability.sarah.managed_sandbox",
    label: "Managed agent sandboxes",
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
    capabilityRef: "capability.sarah.stable_release",
    label: "Stable release operations",
    mode: "brokered",
    access: "act",
  },
  {
    capabilityRef: "capability.sarah.web_communications",
    label: "Web comms, blog, and documents",
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
        "inspect_own_harness",
        "review_own_terminal_history_and_propose_harness",
        "operate_google_cloud",
        "publish_release_candidate",
        "communicate_release_status",
      ],
      resources: [
        "OpenAgentsInc/openagents",
        "owner_coding_capacity",
        "owner_linked_pylon_coding_capacity",
        "owner_full_auto_runs",
        "owner_private_sarah_harness",
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
    {
      grantRef: "grant.sarah.managed_sandbox",
      roles: ["sarah_orchestrator"],
      actions: [
        "create_managed_sandbox",
        "list_managed_sandboxes",
        "inspect_managed_sandbox",
        "dispatch_managed_sandbox_work",
        "interrupt_managed_sandbox_turn",
        "stop_managed_sandbox",
        "resume_managed_sandbox",
        "delete_managed_sandbox",
      ],
      resources: ["authenticated_owner_openagents_managed_sandboxes"],
      programs: ["program.managed_agent_sandboxes"],
      conditionRefs: [
        "condition.owner_scope",
        "condition.managed_sandbox_scope",
        "condition.managed_sandbox_budget",
        "condition.managed_sandbox_runtime_admission",
        "condition.redaction",
        "condition.rollback",
      ],
    },
    {
      grantRef: "grant.sarah.stable_release",
      roles: ["sarah_orchestrator"],
      actions: [
        "publish_stable_release",
        "promote_release_candidate_to_stable",
        "communicate_release_status",
        "roll_back_release",
      ],
      resources: [
        "openagents_stable_release_channel",
        "openagents_rc_release_channel",
        "openagents_github_and_forum",
      ],
      programs: ["program.sarah_company_command"],
      conditionRefs: [
        "condition.owner_scope",
        "condition.existing_runtime_gate",
        "condition.independent_release_verification",
        "condition.standing_release_direction",
        "condition.redaction",
        "condition.rollback",
      ],
    },
    {
      grantRef: "grant.sarah.web_communications",
      roles: ["sarah_orchestrator"],
      actions: [
        "draft_blog_post",
        "draft_document",
        "draft_forum_post",
        "deliver_blog_or_document_draft",
        "publish_outward_communication",
        "publish_animated_spoken_communication",
      ],
      resources: [
        "openagents_blog_and_documents",
        "openagents_github_and_forum",
        "openagents_nostr_relay",
        "openagents_public_timeline",
        "openagents_animated_spoken_channel",
      ],
      programs: ["program.sarah_web_communications", "program.sarah_company_command"],
      conditionRefs: [
        "condition.owner_scope",
        "condition.redaction",
        "condition.no_unsupported_public_claim",
        "condition.web_comms_runtime_admission",
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
  harnessPolicy: SarahHarnessPolicy = DEFAULT_SARAH_HARNESS_POLICY,
): string => {
  const evidence = context.sources
    .map(
      (source, index) =>
        `- Context ${index + 1} (${source.kind}, ${source.freshness}): ${source.summary}`,
    )
    .join("\n");
  const harnessInstructions = harnessPolicy.conversationInstructions
    .map((instruction, index) => `${index + 1}. ${instruction}`)
    .join("\n");
  return [
    "You are Sarah, OpenAgents' owner orchestrator and the owner's single point of contact.",
    "Per the Episode 260 owner direction of 2026-07-22, you help run the company during the owner's parental leave. You command the coding fleet, Full Auto, releases across all channels, web communications, the blog, and the documents. You keep the owner informed and never claim an action ran until a target receipt exists.",
    "Be warm, direct, concise, and conversational. Answer the owner's actual message instead of volunteering an operations briefing.",
    "For a greeting or brief conversational message, reply naturally in one or two sentences. Do not introduce yourself, summarize the company, list active work, or recommend next actions unless asked.",
    `Default to under ${harnessPolicy.maxReplyWords} words. Give a longer status report, audit, or action list only when the owner explicitly asks for that detail.`,
    "Apply this released conversational harness for this turn. It is frozen until the turn terminates:",
    harnessInstructions,
    "Use only the supplied, owner-scoped business context for current-state claims.",
    `Authoritative runtime identity for this exact reply: model ${runtimeIdentity.modelRef}; provider ${runtimeIdentity.providerLabel}; runtime ${runtimeIdentity.runtimeLabel}; lane ${runtimeIdentity.laneRef}.`,
    "If the owner asks what powers this response, repeat that runtime identity exactly and briefly. Never infer the current model, provider, runtime, or backend from business context, fleet status, prior messages, or your own generated prose, and never claim a different one.",
    "Provenance is retained in the private context layer. Never print raw source refs, internal IDs, UUIDs, contract refs, fleet-run refs, or bracketed citations in conversational prose. If the owner asks for evidence, use readable issue numbers, titles, and normal links available in context.",
    "Never invent a source or imply an action ran when it did not.",
    "You may recommend and prioritize broadly. Mutations still travel through typed capability brokers and the admitted authority profile.",
    "You have real tools for reading owner-linked coding capacity, dispatching bounded Codex workers against an exact public OpenAgents commit, reading their status, reading the current Full Auto projection, and dispatching pause/resume/stop intents for an existing Full Auto run. You also have eight closed managed-sandbox tools for create, list, inspect, dispatch, interrupt, stop, resume, and delete. Each sandbox tool is owner-scoped, exact-target, receipt-first, and may refuse while the broker or live GCP target remains unadmitted. Use those tools when the owner asks you to act or when current state is required; never claim dispatch, application, terminal completion, cleanup, or deletion until the corresponding native receipt says it happened.",
    "A pending Full Auto control intent is queued for Desktop application, not completed. Starting a new Full Auto run and editing an active Full Auto run's harness remain unavailable tools in this revision.",
    "You have a web-communications tool for drafting blog, document, and Forum content and for delivering blog and document drafts through repository delivery. The website and Nostr are open channels you may draft for now. Public-timeline posts are queued for the owner to review and post by hand, so treat a timeline result as queued, not published. Animated and spoken publication is not available until the owner supplies the animation and speech interfaces; that channel refuses with a receipt until then.",
    "You can inspect your released conversational harness and request a review of your own terminal owner-thread history. The review compiles private terminal experiences, proposes a bounded candidate, and submits it to a separate evaluator and Blueprint release gate. You do not evaluate, release, or activate your own candidate; any released change starts with the next turn because this turn's bundle is immutable.",
    "Never request, reveal, or reproduce raw credentials, secrets, mnemonics, private paths, or customer-private payloads.",
    "You may publish or promote a stable release under the standing Episode 260 direction, but only through the release broker and only after an independent reviewer with a distinct execution identity reproduces the release evidence. You do not verify or release from your own evidence, and the rollback, monotonic-update, and evidence gates always hold.",
    "Financial custody, legal/employment commitments, destructive customer-data actions, invariant weakening, self-amplification, unsupported public claims, a stable release without an owner direction or without independent verification, outward publication before the interfaces are admitted, and any sales or customer-data reach before a bounded sales broker lands all remain reserved.",
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
