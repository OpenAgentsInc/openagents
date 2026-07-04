import {
  decodeAgentDefinition,
  type AgentDefinition,
} from '@openagentsinc/agent-runtime-schema'
import { Schema as S } from 'effect'

import type { AgentDefinitionRunRequest } from './agent-definition-run-routes'

export const AUTOPILOT_LEAD_GEN_PROMISE_ID = 'autopilot.lead_gen.v1' as const
export const LEAD_GEN_AGENT_DEFINITION_ID =
  'agent_definition.autopilot.lead_gen.v1' as const
export const LEAD_GEN_AGENT_OWNER_REF =
  'agent:openagents_lead_gen_dogfood' as const
export const LEAD_GEN_AGENT_CREATED_AT = '2026-07-04T16:00:00.000Z' as const

export const LEAD_GEN_CUSTOMER_CONFIG_SCHEMA =
  'openagents.autopilot_lead_gen.customer_config.v0.1' as const
export const LEAD_GEN_RUN_PAYLOAD_SCHEMA =
  'openagents.autopilot_lead_gen.run_payload.v0.1' as const
export const LEAD_GEN_DOGFOOD_RUN_RECEIPT_SCHEMA =
  'openagents.autopilot_lead_gen.dogfood_run_receipt.v0.1' as const

const PublicSafeRef = S.String.check(
  S.isNonEmpty(),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/#=-]*$/),
)
const BusinessSourceRef = S.String.check(
  S.isNonEmpty(),
  S.isMaxLength(80),
  S.isPattern(
    /^(direct|unknown|ai_search|own_your_ai|apollo_model_custody|apollo_agent_readiness_[a-z0-9][a-z0-9_-]{0,63}|affiliate_[a-z0-9][a-z0-9_-]{0,63}|partner_[a-z0-9][a-z0-9_-]{0,63}|content_[a-z0-9][a-z0-9_-]{0,63}|vertical_[a-z0-9][a-z0-9_-]{0,63})$/,
  ),
)
const PositiveInt = S.Int.check(S.isGreaterThan(0))
const NonNegativeInt = S.Int.check(S.isGreaterThanOrEqualTo(0))

export const LeadGenCustomerCaps = S.Struct({
  maxContactsPerRun: PositiveInt,
  maxCreditsPerDay: NonNegativeInt,
  maxDomainsPerRun: PositiveInt,
  maxDraftedReportsPerRun: PositiveInt,
  maxDraftedSequenceEntriesPerRun: PositiveInt,
  maxRunSeconds: PositiveInt,
  maxRunsPerDay: PositiveInt,
})
export type LeadGenCustomerCaps = typeof LeadGenCustomerCaps.Type

export const LeadGenCustomerConfig = S.Struct({
  schema: S.Literal(LEAD_GEN_CUSTOMER_CONFIG_SCHEMA),
  analyzerConfigRef: PublicSafeRef,
  approvalGateRef: PublicSafeRef,
  caps: LeadGenCustomerCaps,
  configRef: PublicSafeRef,
  customerRef: PublicSafeRef,
  displayName: S.NonEmptyString,
  icpSpecRef: PublicSafeRef,
  operatorInboxRef: PublicSafeRef,
  sourceRef: BusinessSourceRef,
  sourceRefs: S.Array(S.String),
  targetDiscoveryConfigRef: PublicSafeRef,
  templateFamilyRef: PublicSafeRef,
})
export type LeadGenCustomerConfig = typeof LeadGenCustomerConfig.Type

export const LeadGenRunPayload = S.Struct({
  schema: S.Literal(LEAD_GEN_RUN_PAYLOAD_SCHEMA),
  approvalGateRef: PublicSafeRef,
  customerConfig: LeadGenCustomerConfig,
  pipeline: S.NonEmptyArray(PublicSafeRef),
  sendAuthority: S.Struct({
    allowed: S.Literal(false),
    deniedToolRefs: S.Array(S.String),
    reasonRef: PublicSafeRef,
  }),
})
export type LeadGenRunPayload = typeof LeadGenRunPayload.Type

export const LeadGenDogfoodRunReceipt = S.Struct({
  schema: S.Literal(LEAD_GEN_DOGFOOD_RUN_RECEIPT_SCHEMA),
  analyzerBatchRef: PublicSafeRef,
  approvalGateRef: PublicSafeRef,
  blockerRefs: S.Array(S.String),
  budgetCaps: LeadGenCustomerCaps,
  customerConfigRef: PublicSafeRef,
  customerRef: PublicSafeRef,
  definitionId: S.String,
  draftedReportRefs: S.Array(PublicSafeRef),
  draftedSequenceEntryRefs: S.Array(PublicSafeRef),
  operatorInboxRef: PublicSafeRef,
  receiptRef: PublicSafeRef,
  recordedAt: S.String,
  runHistoryRoute: S.String,
  sendAuthority: S.Struct({
    allowed: S.Literal(false),
    deniedToolRefs: S.Array(S.String),
    requiredApprovalGateRef: PublicSafeRef,
    requiredApprovalReceiptRef: PublicSafeRef,
  }),
  sourceRefs: S.Array(S.String),
  status: S.Literal('drafts_ready_for_operator_approval'),
  targetDiscoveryConfigRef: PublicSafeRef,
  triggerRef: PublicSafeRef,
})
export type LeadGenDogfoodRunReceipt = typeof LeadGenDogfoodRunReceipt.Type

export const decodeLeadGenCustomerConfig =
  S.decodeUnknownSync(LeadGenCustomerConfig)
export const decodeLeadGenRunPayload =
  S.decodeUnknownSync(LeadGenRunPayload)
export const decodeLeadGenDogfoodRunReceipt =
  S.decodeUnknownSync(LeadGenDogfoodRunReceipt)

export const LEAD_GEN_SEND_TOOL_REFS = [
  'tool.openagents.email.send',
  'tool.openagents.email_sequence.send',
  'tool.openagents.email_sequence.activate',
  'tool.openagents.apollo.sequence.send',
  'tool.openagents.apollo.emailer_campaigns_approve',
  'tool.openagents.apollo.emailer_campaigns_add_contact_ids',
  'tool.openagents.apollo.emailer_messages.*',
] as const

export const LEAD_GEN_AGENT_ALLOWED_TOOL_REFS = [
  'tool.openagents.business_lead_gen.customer_config.read',
  'tool.openagents.business_lead_gen.target_discovery.read',
  'tool.openagents.agent_readiness.batch.run',
  'tool.openagents.agent_readiness.report.draft',
  'tool.openagents.email_sequence.draft',
  'tool.openagents.business_pipeline.draft_sequence_entry',
  'tool.openagents.operator_inbox.escalate',
  'tool.openagents.receipt.write',
  'tool.openagents.forge.git.receive_pack',
] as const

export const LEAD_GEN_PIPELINE_REFS = [
  'step.autopilot_lead_gen.target_discovery_config',
  'step.autopilot_lead_gen.agent_readiness_batch',
  'step.autopilot_lead_gen.drafted_reports',
  'step.autopilot_lead_gen.drafted_sequence_entries',
  'step.autopilot_lead_gen.operator_inbox_approval',
] as const

export const OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG =
  decodeLeadGenCustomerConfig({
    schema: LEAD_GEN_CUSTOMER_CONFIG_SCHEMA,
    analyzerConfigRef: 'analyzer.agent_readiness.ora_style_default.v1',
    approvalGateRef: 'approval_gate.lead_gen.openagents.lg4.sequence_send.v1',
    caps: {
      maxContactsPerRun: 20,
      maxCreditsPerDay: 0,
      maxDomainsPerRun: 40,
      maxDraftedReportsPerRun: 10,
      maxDraftedSequenceEntriesPerRun: 20,
      maxRunSeconds: 1800,
      maxRunsPerDay: 1,
    },
    configRef: 'lead_gen_config.openagents.customer_001.v1',
    customerRef: 'customer.openagents.dogfood',
    displayName: 'OpenAgents dogfood ICP',
    icpSpecRef: 'icp.openagents.agent_ready_businesses.v1',
    operatorInboxRef: 'operator_inbox.autopilot.lead_gen.openagents.v1',
    sourceRef: 'apollo_agent_readiness_openagents',
    sourceRefs: [
      'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#10',
      'external.ora.agent_view.llms_txt.20260704',
      'https://github.com/OpenAgentsInc/openagents/issues/8268',
    ],
    targetDiscoveryConfigRef:
      'target_discovery.openagents.agent_ready_businesses.v1',
    templateFamilyRef: 'template_family.lead_gen.report_led_sequence.v1',
  })

export const OPENAGENTS_MODEL_CUSTODY_LEAD_GEN_CONFIG =
  decodeLeadGenCustomerConfig({
    schema: LEAD_GEN_CUSTOMER_CONFIG_SCHEMA,
    analyzerConfigRef:
      'analyzer.agent_readiness.model_custody.own_your_ai.v1',
    approvalGateRef:
      'approval_gate.lead_gen.openagents.model_custody.owner_send.v1',
    caps: {
      maxContactsPerRun: 25,
      maxCreditsPerDay: 0,
      maxDomainsPerRun: 25,
      maxDraftedReportsPerRun: 25,
      maxDraftedSequenceEntriesPerRun: 25,
      maxRunSeconds: 1800,
      maxRunsPerDay: 1,
    },
    configRef: 'lead_gen_config.openagents.model_custody.campaign_b.v1',
    customerRef: 'customer.openagents.dogfood',
    displayName: 'OpenAgents Own Your AI model-custody segment',
    icpSpecRef: 'icp.openagents.own_your_ai.data_rich_mid_market.v1',
    operatorInboxRef:
      'operator_inbox.autopilot.lead_gen.openagents.model_custody.v1',
    sourceRef: 'apollo_model_custody',
    sourceRefs: [
      'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#11-campaign-b-own-your-ai',
      'docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md',
      'packages/agent-readiness/src/index.ts',
      'https://github.com/OpenAgentsInc/openagents/issues/8281',
    ],
    targetDiscoveryConfigRef:
      'target_discovery.openagents.model_custody.hand_approved.v1',
    templateFamilyRef:
      'template_family.lead_gen.model_custody_regulated.reactor_assessment.v1',
  })

export const leadGenAgentDefinition = (
  input: Readonly<{
    createdAt?: string
    ownerRef?: string
    updatedAt?: string
  }> = {},
): AgentDefinition =>
  decodeAgentDefinition({
    schema: 'openagents.agent_definition.v1',
    id: LEAD_GEN_AGENT_DEFINITION_ID,
    ownerRef: input.ownerRef ?? LEAD_GEN_AGENT_OWNER_REF,
    name: 'Autopilot Lead Gen',
    slug: 'autopilot-lead-gen',
    goal:
      'Run approval-gated lead generation: read the customer ICP config, discover target domains, run the LG-1 agent-readiness analyzer batch, draft LG-5 reports, draft LG-4 sequence entries, and escalate the drafts to the operator inbox. Never send outreach.',
    harness: {
      kind: 'codex',
      modelHint: 'openagents/pylon-codex',
      versionPin: 'lead-gen-v0',
    },
    toolset: {
      allow: [...LEAD_GEN_AGENT_ALLOWED_TOOL_REFS],
      ask: [],
      deny: [
        ...LEAD_GEN_SEND_TOOL_REFS,
        'tool.openagents.forge.git.admin',
      ],
      networkPolicy: 'owner_scoped',
      secretPolicy: 'owner_scoped_refs_only',
    },
    triggers: [
      {
        kind: 'cron',
        triggerRef: 'trigger.autopilot.lead_gen.weekday_discovery',
        expr: '0 15 * * 1-5',
        tz: 'America/Chicago',
      },
      {
        kind: 'manual',
        triggerRef: 'trigger.autopilot.lead_gen.manual',
      },
    ],
    lane: 'own_pylon',
    budget: {
      maxRunSeconds: OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG.caps.maxRunSeconds,
      maxRunsPerDay: OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG.caps.maxRunsPerDay,
      maxCreditsPerDay:
        OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG.caps.maxCreditsPerDay,
    },
    escalation: {
      channel: 'operator',
      askPolicy: {
        policyRef: 'policy.autopilot.lead_gen.operator_required.v1',
        mode: 'operator_required',
      },
    },
    sourceRefs: [
      'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#10',
      'docs/fable/ROADMAP_BACKGROUND_AGENTS.md#ba-b5',
      'external.ora.agent_view.llms_txt.20260704',
      'https://github.com/OpenAgentsInc/openagents/issues/8261',
      'https://github.com/OpenAgentsInc/openagents/issues/8262',
      'https://github.com/OpenAgentsInc/openagents/issues/8265',
      'https://github.com/OpenAgentsInc/openagents/issues/8266',
      'https://github.com/OpenAgentsInc/openagents/issues/8268',
    ],
    createdAt: input.createdAt ?? LEAD_GEN_AGENT_CREATED_AT,
    updatedAt: input.updatedAt ?? input.createdAt ?? LEAD_GEN_AGENT_CREATED_AT,
  })

export const OPENAGENTS_LEAD_GEN_AGENT_DEFINITION = leadGenAgentDefinition()

export const leadGenRunPayload = (
  customerConfig: LeadGenCustomerConfig,
): LeadGenRunPayload =>
  decodeLeadGenRunPayload({
    schema: LEAD_GEN_RUN_PAYLOAD_SCHEMA,
    approvalGateRef: customerConfig.approvalGateRef,
    customerConfig,
    pipeline: [...LEAD_GEN_PIPELINE_REFS],
    sendAuthority: {
      allowed: false,
      deniedToolRefs: [...LEAD_GEN_SEND_TOOL_REFS],
      reasonRef: 'reason.autopilot_lead_gen.drafting_only',
    },
  })

export const leadGenRunRequest = (
  customerConfig: LeadGenCustomerConfig,
  input: Readonly<{
    triggerRef?: string
  }> = {},
): AgentDefinitionRunRequest => ({
  objectiveSummary:
    `Run Autopilot Lead Gen for ${customerConfig.configRef}: target discovery, ` +
    'agent-readiness reports, sequence drafts, and operator approval request only.',
  triggerPayload: leadGenRunPayload(customerConfig),
  triggerRef: input.triggerRef ?? 'trigger.autopilot.lead_gen.manual',
})

export const OPENAGENTS_LEAD_GEN_DOGFOOD_RUN_RECEIPT =
  decodeLeadGenDogfoodRunReceipt({
    schema: LEAD_GEN_DOGFOOD_RUN_RECEIPT_SCHEMA,
    analyzerBatchRef:
      'agent_readiness_batch.openagents.lead_gen.customer_001.20260704',
    approvalGateRef: OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG.approvalGateRef,
    blockerRefs: [
      'blocker.autopilot_lead_gen.live_customer_send_approval_missing',
      'blocker.autopilot_lead_gen.customer_result_receipt_missing',
    ],
    budgetCaps: OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG.caps,
    customerConfigRef: OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG.configRef,
    customerRef: OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG.customerRef,
    definitionId: LEAD_GEN_AGENT_DEFINITION_ID,
    draftedReportRefs: [
      'draft_report.agent_readiness.openagents.lead_gen.001',
      'draft_report.agent_readiness.openagents.lead_gen.002',
    ],
    draftedSequenceEntryRefs: [
      'draft_sequence_entry.openagents.lead_gen.001',
      'draft_sequence_entry.openagents.lead_gen.002',
    ],
    operatorInboxRef: OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG.operatorInboxRef,
    receiptRef: 'receipt.autopilot_lead_gen.openagents.dogfood.20260704',
    recordedAt: '2026-07-04T16:20:00.000Z',
    runHistoryRoute:
      '/v1/agent-definitions/agent_definition.autopilot.lead_gen.v1/runs',
    sendAuthority: {
      allowed: false,
      deniedToolRefs: [...LEAD_GEN_SEND_TOOL_REFS],
      requiredApprovalGateRef:
        OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG.approvalGateRef,
      requiredApprovalReceiptRef:
        'approval_receipt.lead_gen.openagents.sequence_send.required',
    },
    sourceRefs: [
      'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#10',
      'docs/fable/2026-07-04-autopilot-lead-gen-agent-definition-receipt.md',
      'external.ora.agent_view.llms_txt.20260704',
      'https://github.com/OpenAgentsInc/openagents/issues/8268',
    ],
    status: 'drafts_ready_for_operator_approval',
    targetDiscoveryConfigRef:
      OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG.targetDiscoveryConfigRef,
    triggerRef: 'trigger.autopilot.lead_gen.manual',
  })
