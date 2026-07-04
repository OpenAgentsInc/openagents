import {
  compileAgentDefinitionToolRuntimePolicy,
  decideAgentDefinitionToolAuthority,
} from '@openagentsinc/agent-runtime-schema'
import { describe, expect, test } from 'vitest'

import {
  LEAD_GEN_AGENT_ALLOWED_TOOL_REFS,
  LEAD_GEN_AGENT_DEFINITION_ID,
  LEAD_GEN_PIPELINE_REFS,
  LEAD_GEN_SEND_TOOL_REFS,
  OPENAGENTS_LEAD_GEN_AGENT_DEFINITION,
  OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG,
  OPENAGENTS_LEAD_GEN_DOGFOOD_RUN_RECEIPT,
  OPENAGENTS_MODEL_CUSTODY_LEAD_GEN_CONFIG,
  decodeLeadGenCustomerConfig,
  decodeLeadGenRunPayload,
  leadGenRunRequest,
} from './autopilot-lead-gen-agent-definition'
import { compileAgentDefinitionForgeGitAccessScopes } from './forge-tenant-git-auth-store'

// Behavior contract oracle: lead_gen_agent.drafting_only_toolset.v1
// Behavior contract oracle: lead_gen_agent.no_send_without_approval_receipt.v1

describe('Autopilot Lead Gen agent definition', () => {
  test('decodes a standing approval-gated definition with cron, manual run, and BA-B4 budget caps', () => {
    const definition = OPENAGENTS_LEAD_GEN_AGENT_DEFINITION

    expect(definition).toMatchObject({
      schema: 'openagents.agent_definition.v1',
      id: LEAD_GEN_AGENT_DEFINITION_ID,
      name: 'Autopilot Lead Gen',
      lane: 'own_pylon',
      budget: {
        maxCreditsPerDay: 0,
        maxRunSeconds: 1800,
        maxRunsPerDay: 1,
      },
      escalation: {
        channel: 'operator',
        askPolicy: { mode: 'operator_required' },
      },
    })
    expect(definition.triggers.map(trigger => trigger.kind)).toEqual([
      'cron',
      'manual',
    ])
    expect(definition.toolset.allow).toEqual([
      ...LEAD_GEN_AGENT_ALLOWED_TOOL_REFS,
    ])
    expect(definition.toolset.deny).toEqual([
      ...LEAD_GEN_SEND_TOOL_REFS,
      'tool.openagents.forge.git.admin',
    ])
    expect(definition.sourceRefs).toEqual(
      expect.arrayContaining([
        'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#10',
        'external.ora.agent_view.llms_txt.20260704',
        'https://github.com/OpenAgentsInc/openagents/issues/8268',
      ]),
    )
  })

  test('keeps the toolset drafting-only and still dispatchable through the existing own-Pylon route', () => {
    const definition = OPENAGENTS_LEAD_GEN_AGENT_DEFINITION
    const policy = compileAgentDefinitionToolRuntimePolicy(definition)
    const allowedOrEscalatedToolRefs = [
      ...definition.toolset.allow,
      ...definition.toolset.ask,
    ]

    for (const sendToolRef of LEAD_GEN_SEND_TOOL_REFS) {
      expect(allowedOrEscalatedToolRefs).not.toContain(sendToolRef)
      expect(
        decideAgentDefinitionToolAuthority({
          definition,
          toolRef: sendToolRef,
        }),
      ).toMatchObject({
        allowed: false,
        status: 'denied',
        reasonRef: 'reason.agent_definition.tool_denied',
      })
    }
    expect(
      decideAgentDefinitionToolAuthority({
        definition,
        toolRef: 'tool.openagents.apollo.emailer_messages.send',
      }),
    ).toMatchObject({
      allowed: false,
      matchedPolicyRef: 'tool.openagents.apollo.emailer_messages.*',
      status: 'denied',
    })
    expect(
      decideAgentDefinitionToolAuthority({
        definition,
        toolRef: 'tool.openagents.email_sequence.draft',
      }),
    ).toMatchObject({
      allowed: true,
      status: 'allowed',
    })

    expect(
      compileAgentDefinitionForgeGitAccessScopes({
        policy,
        requestedScopes: ['git:receive-pack'],
      }),
    ).toMatchObject({
      scopes: ['git:receive-pack'],
      status: 'allowed',
    })
  })

  test('keeps per-customer ICP and template config in the run payload instead of forking the definition', () => {
    const secondTenantConfig = decodeLeadGenCustomerConfig({
      ...OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG,
      analyzerConfigRef: 'analyzer.agent_readiness.partner_agency.v1',
      approvalGateRef: 'approval_gate.lead_gen.partner_agency.lg4.v1',
      configRef: 'lead_gen_config.partner_agency.customer_002.v1',
      customerRef: 'customer.partner_agency.002',
      displayName: 'Partner agency ICP',
      icpSpecRef: 'icp.partner_agency.agent_ready_local_services.v1',
      operatorInboxRef: 'operator_inbox.autopilot.lead_gen.partner_agency.v1',
      sourceRef: 'partner_agency',
      targetDiscoveryConfigRef:
        'target_discovery.partner_agency.local_services.v1',
      templateFamilyRef: 'template_family.lead_gen.agency_white_label.v1',
    })
    const dogfoodPayload = decodeLeadGenRunPayload(
      leadGenRunRequest(OPENAGENTS_LEAD_GEN_DOGFOOD_CONFIG).triggerPayload,
    )
    const secondPayload = decodeLeadGenRunPayload(
      leadGenRunRequest(secondTenantConfig).triggerPayload,
    )

    expect(OPENAGENTS_LEAD_GEN_AGENT_DEFINITION.id).toBe(
      LEAD_GEN_AGENT_DEFINITION_ID,
    )
    expect(dogfoodPayload.customerConfig.configRef).toBe(
      'lead_gen_config.openagents.customer_001.v1',
    )
    expect(secondPayload.customerConfig.configRef).toBe(
      'lead_gen_config.partner_agency.customer_002.v1',
    )
    expect(dogfoodPayload.pipeline).toEqual([...LEAD_GEN_PIPELINE_REFS])
    expect(secondPayload.pipeline).toEqual(dogfoodPayload.pipeline)
    expect(secondPayload.sendAuthority).toMatchObject({
      allowed: false,
      deniedToolRefs: [...LEAD_GEN_SEND_TOOL_REFS],
    })
  })

  test('registers the RX-8 Own Your AI model-custody segment as another LG-7 customer config', () => {
    const payload = decodeLeadGenRunPayload(
      leadGenRunRequest(OPENAGENTS_MODEL_CUSTODY_LEAD_GEN_CONFIG)
        .triggerPayload,
    )

    expect(payload.customerConfig).toMatchObject({
      analyzerConfigRef:
        'analyzer.agent_readiness.model_custody.own_your_ai.v1',
      configRef: 'lead_gen_config.openagents.model_custody.campaign_b.v1',
      sourceRef: 'apollo_model_custody',
      targetDiscoveryConfigRef:
        'target_discovery.openagents.model_custody.hand_approved.v1',
      templateFamilyRef:
        'template_family.lead_gen.model_custody_regulated.reactor_assessment.v1',
    })
    expect(payload.customerConfig.caps).toMatchObject({
      maxContactsPerRun: 25,
      maxCreditsPerDay: 0,
      maxDomainsPerRun: 25,
    })
    expect(payload.pipeline).toEqual([...LEAD_GEN_PIPELINE_REFS])
    expect(payload.sendAuthority).toMatchObject({
      allowed: false,
      deniedToolRefs: [...LEAD_GEN_SEND_TOOL_REFS],
    })
    expect(payload.customerConfig.sourceRefs).toEqual(
      expect.arrayContaining([
        'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#11-campaign-b-own-your-ai',
        'https://github.com/OpenAgentsInc/openagents/issues/8281',
      ]),
    )
  })

  test('records the OpenAgents dogfood run receipt as drafts awaiting operator approval', () => {
    const receipt = OPENAGENTS_LEAD_GEN_DOGFOOD_RUN_RECEIPT

    expect(receipt).toMatchObject({
      definitionId: LEAD_GEN_AGENT_DEFINITION_ID,
      status: 'drafts_ready_for_operator_approval',
      customerConfigRef: 'lead_gen_config.openagents.customer_001.v1',
      customerRef: 'customer.openagents.dogfood',
      sendAuthority: {
        allowed: false,
        deniedToolRefs: [...LEAD_GEN_SEND_TOOL_REFS],
      },
    })
    expect(receipt.draftedReportRefs.length).toBeGreaterThan(0)
    expect(receipt.draftedSequenceEntryRefs.length).toBeGreaterThan(0)
    expect(receipt.operatorInboxRef).toBe(
      'operator_inbox.autopilot.lead_gen.openagents.v1',
    )
    expect(receipt.runHistoryRoute).toBe(
      '/v1/agent-definitions/agent_definition.autopilot.lead_gen.v1/runs',
    )
    expect(receipt.sourceRefs).toEqual(
      expect.arrayContaining([
        'docs/fable/2026-07-04-autopilot-lead-gen-agent-definition-receipt.md',
        'external.ora.agent_view.llms_txt.20260704',
        'https://github.com/OpenAgentsInc/openagents/issues/8268',
      ]),
    )
  })
})
