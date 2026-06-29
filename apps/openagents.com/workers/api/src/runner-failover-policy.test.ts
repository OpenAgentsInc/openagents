import { describe, expect, test } from 'vitest'

import type { RunnerBackendConfig } from './config'
import {
  decideOpenAgentsRunnerFailover,
  openAgentsRunnerFailoverDecisionReceiptHasPrivateMaterial,
  type OpenAgentsRunnerFailoverDecisionInput,
} from './runner-failover-policy'

const baseConfig: RunnerBackendConfig = {
  automaticFailoverEnabled: false,
  cloudflareContainer: {
    allowedWorkloadTrusts: ['low', 'medium'],
    binding: {},
    configured: false,
    enabled: false,
    policyApproved: false,
    stagingSmokePassed: false,
  },
  gcloud: {
    referenceEnabled: false,
    sensitiveApproved: false,
  },
  policy: 'shc_primary_only',
}

const configuredContainer = (): RunnerBackendConfig['cloudflareContainer'] => ({
  allowedWorkloadTrusts: ['low', 'medium'],
  binding: {
    className: 'OpenAgentsSiteRunnerContainer',
    durableObjectBinding: 'SITE_RUNNER_CONTAINER',
    imageRef: './containers/site-runner/Dockerfile',
    instanceType: 'lite',
    maxInstances: 2,
  },
  configured: true,
  enabled: true,
  policyApproved: true,
  stagingSmokePassed: true,
})

const input = (
  overrides: Partial<OpenAgentsRunnerFailoverDecisionInput> = {},
): OpenAgentsRunnerFailoverDecisionInput => ({
  capacityGateOk: true,
  config: {
    ...baseConfig,
    cloudflareContainer: configuredContainer(),
    policy: 'shc_primary_cloudflare_container_backup_gcloud_reference',
  },
  costGateApproved: true,
  gcloudReady: false,
  liveAutomaticFailoverApproved: false,
  operatorSelectedContainer: false,
  previousBackendKind: 'shc_vm',
  previousBackendRef: 'runner_backend.shc.primary',
  shcReady: true,
  trigger: 'operator_selected',
  workloadTrust: 'medium',
  ...overrides,
})

describe('runner failover staging rollout policy', () => {
  test('keeps SHC primary until an operator explicitly selects Container', () => {
    const decision = decideOpenAgentsRunnerFailover(input())

    expect(decision).toMatchObject({
      automaticFailoverEffective: false,
      automaticFailoverRequested: false,
      blockedGateRefs: [
        'gate.runner_failover.operator_selected_container.blocked',
      ],
      decisionStatus: 'primary',
      previousBackendKind: 'shc_vm',
      selectedBackendKind: 'shc_vm',
      trustLevel: 'medium',
    })
    expect(decision.reasonRefs).toEqual([
      'reason.runner_failover.shc_primary_until_container_gates_pass',
    ])
    expect(openAgentsRunnerFailoverDecisionReceiptHasPrivateMaterial(decision))
      .toBe(false)
  })

  test('selects Container only for operator-selected low or medium trust after all gates pass', () => {
    const decision = decideOpenAgentsRunnerFailover(
      input({ operatorSelectedContainer: true, workloadTrust: 'low' }),
    )

    expect(decision).toMatchObject({
      automaticFailoverEffective: false,
      blockedGateRefs: [],
      decisionStatus: 'selected',
      selectedBackendKind: 'cloudflare_container',
      selectedBackendRef: 'runner_backend.cloudflare_container',
      trigger: 'operator_selected',
      trustLevel: 'low',
    })
    expect(decision.reasonRefs).toEqual([
      'reason.runner_failover.operator_selected_container',
      'reason.runner_failover.low_medium_trust_allowed',
    ])
  })

  test('blocks Container preference when binding, smoke, cost, capacity, or approval gates fail', () => {
    const decision = decideOpenAgentsRunnerFailover(
      input({
        capacityGateOk: false,
        config: {
          ...baseConfig,
          automaticFailoverEnabled: true,
          cloudflareContainer: {
            ...configuredContainer(),
            binding: {},
            policyApproved: false,
            stagingSmokePassed: false,
          },
          policy: 'shc_primary_cloudflare_container_backup_gcloud_reference',
        },
        costGateApproved: false,
        operatorSelectedContainer: true,
      }),
    )

    expect(decision).toMatchObject({
      automaticFailoverEffective: false,
      automaticFailoverRequested: true,
      decisionStatus: 'primary',
      selectedBackendKind: 'shc_vm',
    })
    expect(decision.blockedGateRefs).toEqual([
      'gate.runner_failover.cloudflare_container_binding_configured.blocked',
      'gate.runner_failover.cloudflare_container_staging_smoke.blocked',
      'gate.runner_failover.runner_policy_approval.blocked',
      'gate.runner_failover.capacity.blocked',
      'gate.runner_failover.cost.blocked',
      'gate.runner_failover.live_automatic_approval.blocked',
    ])
  })

  test('never routes sensitive work to Container and uses explicit reference lane when ready', () => {
    const gcloudDecision = decideOpenAgentsRunnerFailover(
      input({
        gcloudReady: true,
        operatorSelectedContainer: true,
        shcReady: false,
        trigger: 'shc_unavailable',
        workloadTrust: 'sensitive',
      }),
    )
    const blockedDecision = decideOpenAgentsRunnerFailover(
      input({
        gcloudReady: false,
        operatorSelectedContainer: true,
        shcReady: false,
        trigger: 'shc_unavailable',
        workloadTrust: 'sensitive',
      }),
    )

    expect(gcloudDecision).toMatchObject({
      blockedGateRefs: [],
      decisionStatus: 'selected',
      selectedBackendKind: 'gcloud_vm',
      trustLevel: 'sensitive',
    })
    expect(gcloudDecision.reasonRefs).toContain(
      'reason.runner_failover.cloudflare_container_sensitive_denied',
    )
    expect(blockedDecision).toMatchObject({
      blockedGateRefs: [
        'gate.runner_failover.sensitive_workload_denied.blocked',
        'gate.runner_failover.shc_or_gcloud_ready.blocked',
      ],
      decisionStatus: 'blocked',
      selectedBackendKind: 'shc_vm',
    })
  })

  test('strips unsafe previous backend refs from failover receipts', () => {
    const decision = decideOpenAgentsRunnerFailover(
      input({
        operatorSelectedContainer: true,
        previousBackendRef: 'runner_backend.shc.primary bearer raw-token',
      }),
    )

    expect(decision.previousBackendRef).toBe('runner_backend.shc_vm')
    expect(openAgentsRunnerFailoverDecisionReceiptHasPrivateMaterial(decision))
      .toBe(false)
  })
})
