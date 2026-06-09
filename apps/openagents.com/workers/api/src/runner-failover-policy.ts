import { Schema as S } from 'effect'

import type { RunnerBackendConfig, RunnerWorkloadTrust } from './config'
import {
  OpenAgentsRunnerBackendKind,
  OpenAgentsRunnerWorkloadTrust,
} from './runner-backends'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsRunnerFailoverTrigger = S.Literals([
  'cost_review',
  'launch_timeout',
  'manual_probe',
  'operator_selected',
  'runner_failure',
  'shc_unavailable',
])
export type OpenAgentsRunnerFailoverTrigger =
  typeof OpenAgentsRunnerFailoverTrigger.Type

export const OpenAgentsRunnerFailoverDecisionStatus = S.Literals([
  'blocked',
  'primary',
  'selected',
])
export type OpenAgentsRunnerFailoverDecisionStatus =
  typeof OpenAgentsRunnerFailoverDecisionStatus.Type

export const OpenAgentsRunnerFailoverDecisionReceipt = S.Struct({
  automaticFailoverEffective: S.Boolean,
  automaticFailoverRequested: S.Boolean,
  blockedGateRefs: S.Array(S.String),
  customerSafeStatusRef: S.String,
  decisionStatus: OpenAgentsRunnerFailoverDecisionStatus,
  previousBackendKind: OpenAgentsRunnerBackendKind,
  previousBackendRef: S.String,
  publicSummaryRef: S.String,
  reasonRefs: S.Array(S.String),
  receiptRef: S.String,
  selectedBackendKind: OpenAgentsRunnerBackendKind,
  selectedBackendRef: S.String,
  trigger: OpenAgentsRunnerFailoverTrigger,
  trustLevel: OpenAgentsRunnerWorkloadTrust,
})
export type OpenAgentsRunnerFailoverDecisionReceipt =
  typeof OpenAgentsRunnerFailoverDecisionReceipt.Type

export type OpenAgentsRunnerFailoverDecisionInput = Readonly<{
  capacityGateOk: boolean
  config: RunnerBackendConfig
  costGateApproved: boolean
  gcloudReady: boolean
  liveAutomaticFailoverApproved: boolean
  operatorSelectedContainer: boolean
  previousBackendKind: typeof OpenAgentsRunnerBackendKind.Type
  previousBackendRef: string
  shcReady: boolean
  trigger: OpenAgentsRunnerFailoverTrigger
  workloadTrust: RunnerWorkloadTrust
}>

const safeRefPart = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9_.-]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 96) || 'ref'

const unsafeFailoverRefPattern =
  /(bearer\s+|callback[_-]?token|cookie|customer[_-]?(email|name)|email[_-]?body|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|oauth|openagents_admin|password|preimage|private[_-]?key|provider[_-]?payload|provider[_-]?token|raw[_-]?(email|runner|run[_-]?log)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|wallet[_-]?secret|\S+@\S+)/i

const safeRef = (ref: string): boolean =>
  ref.trim() !== '' &&
  !unsafeFailoverRefPattern.test(ref) &&
  !openAgentsRunnerGatewayPayloadHasPrivateMaterial(ref)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(safeRef)

const containerBindingConfigured = (config: RunnerBackendConfig): boolean =>
  config.cloudflareContainer.configured &&
  config.cloudflareContainer.binding.className !== undefined &&
  config.cloudflareContainer.binding.durableObjectBinding !== undefined &&
  config.cloudflareContainer.binding.imageRef !== undefined

const containerPolicySelected = (config: RunnerBackendConfig): boolean =>
  config.policy === 'shc_primary_cloudflare_container_backup_gcloud_reference'

const containerTrustAllowed = (
  config: RunnerBackendConfig,
  workloadTrust: RunnerWorkloadTrust,
): boolean => config.cloudflareContainer.allowedWorkloadTrusts.includes(
  workloadTrust,
)

const gateRef = (gate: string): string =>
  `gate.runner_failover.${gate}.blocked`

const containerBlockedGateRefs = (
  input: OpenAgentsRunnerFailoverDecisionInput,
): ReadonlyArray<string> => {
  const refs: string[] = []

  if (!input.operatorSelectedContainer) {
    refs.push(gateRef('operator_selected_container'))
  }
  if (!containerPolicySelected(input.config)) {
    refs.push(gateRef('policy_selected'))
  }
  if (!input.config.cloudflareContainer.enabled) {
    refs.push(gateRef('cloudflare_container_enabled'))
  }
  if (!containerBindingConfigured(input.config)) {
    refs.push(gateRef('cloudflare_container_binding_configured'))
  }
  if (!input.config.cloudflareContainer.stagingSmokePassed) {
    refs.push(gateRef('cloudflare_container_staging_smoke'))
  }
  if (!input.config.cloudflareContainer.policyApproved) {
    refs.push(gateRef('runner_policy_approval'))
  }
  if (!input.capacityGateOk) {
    refs.push(gateRef('capacity'))
  }
  if (!input.costGateApproved) {
    refs.push(gateRef('cost'))
  }
  if (!containerTrustAllowed(input.config, input.workloadTrust)) {
    refs.push(gateRef('workload_trust'))
  }
  if (input.workloadTrust === 'sensitive') {
    refs.push(gateRef('sensitive_workload_denied'))
  }
  if (
    input.config.automaticFailoverEnabled &&
    !input.liveAutomaticFailoverApproved
  ) {
    refs.push(gateRef('live_automatic_approval'))
  }

  return safeRefs(refs)
}

const backendRef = (
  backendKind: typeof OpenAgentsRunnerBackendKind.Type,
): string => `runner_backend.${backendKind}`

const baseReceipt = (
  input: OpenAgentsRunnerFailoverDecisionInput,
  selectedBackendKind: typeof OpenAgentsRunnerBackendKind.Type,
  decisionStatus: OpenAgentsRunnerFailoverDecisionStatus,
  reasonRefs: ReadonlyArray<string>,
  blockedGateRefs: ReadonlyArray<string>,
): OpenAgentsRunnerFailoverDecisionReceipt => {
  const triggerRef = safeRefPart(input.trigger)
  const trustRef = safeRefPart(input.workloadTrust)
  const previousBackendRef = safeRefs([input.previousBackendRef])[0] ??
    backendRef(input.previousBackendKind)
  const selectedBackendRef = backendRef(selectedBackendKind)

  return {
    automaticFailoverEffective:
      input.config.automaticFailoverEnabled &&
      input.liveAutomaticFailoverApproved &&
      blockedGateRefs.length === 0 &&
      decisionStatus === 'selected',
    automaticFailoverRequested: input.config.automaticFailoverEnabled,
    blockedGateRefs: safeRefs(blockedGateRefs),
    customerSafeStatusRef: decisionStatus === 'blocked'
      ? 'status.runner_failover.blocked_operator_review_required'
      : decisionStatus === 'primary'
        ? 'status.runner_failover.shc_primary'
        : `status.runner_failover.selected.${selectedBackendKind}`,
    decisionStatus,
    previousBackendKind: input.previousBackendKind,
    previousBackendRef,
    publicSummaryRef: `summary.runner_failover.${decisionStatus}.${triggerRef}.${trustRef}`,
    reasonRefs: safeRefs(reasonRefs),
    receiptRef: `receipt.runner_failover.${decisionStatus}.${triggerRef}.${trustRef}`,
    selectedBackendKind,
    selectedBackendRef,
    trigger: input.trigger,
    trustLevel: input.workloadTrust,
  }
}

export const decideOpenAgentsRunnerFailover = (
  input: OpenAgentsRunnerFailoverDecisionInput,
): OpenAgentsRunnerFailoverDecisionReceipt => {
  if (input.workloadTrust === 'sensitive') {
    if (input.gcloudReady) {
      return baseReceipt(
        input,
        'gcloud_vm',
        'selected',
        [
          'reason.runner_failover.sensitive_workload_uses_gcloud_reference',
          'reason.runner_failover.cloudflare_container_sensitive_denied',
        ],
        [],
      )
    }

    if (input.shcReady) {
      return baseReceipt(
        input,
        'shc_vm',
        'primary',
        [
          'reason.runner_failover.shc_primary_for_sensitive_workload',
          'reason.runner_failover.cloudflare_container_sensitive_denied',
        ],
        [gateRef('sensitive_workload_denied')],
      )
    }

    return baseReceipt(
      input,
      'shc_vm',
      'blocked',
      [
        'reason.runner_failover.sensitive_workload_requires_shc_or_gcloud',
        'reason.runner_failover.cloudflare_container_sensitive_denied',
      ],
      [gateRef('sensitive_workload_denied'), gateRef('shc_or_gcloud_ready')],
    )
  }

  const blockedGateRefs = containerBlockedGateRefs(input)

  if (blockedGateRefs.length === 0) {
    return baseReceipt(
      input,
      'cloudflare_container',
      'selected',
      [
        'reason.runner_failover.operator_selected_container',
        'reason.runner_failover.low_medium_trust_allowed',
      ],
      [],
    )
  }

  if (input.shcReady) {
    return baseReceipt(
      input,
      'shc_vm',
      'primary',
      [
        'reason.runner_failover.shc_primary_until_container_gates_pass',
      ],
      blockedGateRefs,
    )
  }

  return baseReceipt(
    input,
    'shc_vm',
    'blocked',
    ['reason.runner_failover.no_eligible_backend'],
    [...blockedGateRefs, gateRef('shc_ready')],
  )
}

export const openAgentsRunnerFailoverDecisionReceiptHasPrivateMaterial = (
  receipt: OpenAgentsRunnerFailoverDecisionReceipt,
): boolean =>
  unsafeFailoverRefPattern.test(JSON.stringify(receipt)) ||
  openAgentsRunnerGatewayPayloadHasPrivateMaterial(receipt)
