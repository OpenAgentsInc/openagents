import type { RunnerBackendConfig, RunnerWorkloadTrust } from './config'

export type RunnerBackendCheckStatus = 'blocked' | 'ok' | 'unknown' | 'warning'

export type RunnerBackendReadinessCheck = Readonly<{
  details: Record<string, unknown>
  message: string
  name: 'runner_backends'
  status: RunnerBackendCheckStatus
}>

type RunnerBackendReadinessInput = Readonly<{
  callbackStatus: RunnerBackendCheckStatus
  config: RunnerBackendConfig
  shcControlStatus: RunnerBackendCheckStatus
  workloadTrust: RunnerWorkloadTrust
}>

const knownTrust = (
  value: string | undefined,
): RunnerWorkloadTrust | undefined =>
  value === 'low' || value === 'medium' || value === 'sensitive'
    ? value
    : undefined

export const runnerWorkloadTrustFromSelector = (
  selector: Record<string, unknown>,
): RunnerWorkloadTrust => {
  const value =
    typeof selector.runnerWorkloadTrust === 'string'
      ? selector.runnerWorkloadTrust
      : typeof selector.workloadTrust === 'string'
        ? selector.workloadTrust
        : typeof selector.siteTrustLevel === 'string'
          ? selector.siteTrustLevel
          : undefined

  return knownTrust(value) ?? 'low'
}

export const runnerBackendReadinessCheck = (
  input: RunnerBackendReadinessInput,
): RunnerBackendReadinessCheck => {
  const shcReady =
    input.shcControlStatus === 'ok' && input.callbackStatus === 'ok'
  const containerBindingConfigured =
    input.config.cloudflareContainer.configured &&
    input.config.cloudflareContainer.binding.className !== undefined &&
    input.config.cloudflareContainer.binding.durableObjectBinding !== undefined &&
    input.config.cloudflareContainer.binding.imageRef !== undefined
  const containerWorkloadEligible =
    input.config.cloudflareContainer.allowedWorkloadTrusts.includes(
      input.workloadTrust,
    )
  const containerReady =
    input.config.policy ===
      'shc_primary_cloudflare_container_backup_gcloud_reference' &&
    input.config.cloudflareContainer.enabled &&
    containerBindingConfigured &&
    input.config.cloudflareContainer.stagingSmokePassed &&
    input.config.cloudflareContainer.policyApproved &&
    containerWorkloadEligible
  const gcloudReady =
    input.config.policy ===
      'shc_primary_cloudflare_container_backup_gcloud_reference' &&
    input.config.gcloud.referenceEnabled &&
    (input.workloadTrust !== 'sensitive' ||
      input.config.gcloud.sensitiveApproved)
  const automaticFailoverPrerequisitesMet =
    input.config.cloudflareContainer.enabled &&
    containerBindingConfigured &&
    input.config.cloudflareContainer.stagingSmokePassed &&
    input.config.cloudflareContainer.policyApproved
  const automaticFailoverBlocked =
    input.config.automaticFailoverEnabled && !automaticFailoverPrerequisitesMet
  const status: RunnerBackendCheckStatus = !shcReady
    ? 'blocked'
    : automaticFailoverBlocked
      ? 'blocked'
      : input.config.policy === 'shc_primary_only'
        ? 'ok'
        : containerReady || gcloudReady
          ? 'ok'
          : 'warning'
  const message = !shcReady
    ? 'SHC primary runner is not ready and automatic failover is disabled.'
    : automaticFailoverBlocked
      ? 'Automatic runner failover requires Container enablement, binding configuration, staging smoke, and policy approval.'
      : input.config.policy === 'shc_primary_only'
        ? 'SHC primary runner is ready; backup lanes are not enabled.'
        : containerReady && gcloudReady
          ? 'SHC primary, Container backup, and reference lanes are ready for this workload.'
          : containerReady
            ? 'SHC primary and Container backup lanes are ready for this low-to-medium trust workload.'
            : gcloudReady
              ? 'SHC primary and reference lanes are ready; Container backup is not eligible for this workload.'
              : 'SHC primary is ready; backup and reference lanes are not fully ready.'

  return {
    details: {
      automaticFailover: {
        effective:
          input.config.automaticFailoverEnabled &&
          automaticFailoverPrerequisitesMet,
        requested: input.config.automaticFailoverEnabled,
        requiredBeforeEnable: [
          'cloudflare_container_enabled',
          'cloudflare_container_configured',
          'cloudflare_container_staging_smoke',
          'runner_policy_approval',
        ],
      },
      lanes: {
        cloudflare_container_backup: {
          allowedWorkloadTrusts:
            input.config.cloudflareContainer.allowedWorkloadTrusts,
          binding: {
            classNameConfigured:
              input.config.cloudflareContainer.binding.className !== undefined,
            durableObjectBindingConfigured:
              input.config.cloudflareContainer.binding.durableObjectBinding !==
              undefined,
            imageRefConfigured:
              input.config.cloudflareContainer.binding.imageRef !== undefined,
            instanceTypeConfigured:
              input.config.cloudflareContainer.binding.instanceType !== undefined,
            maxInstancesConfigured:
              input.config.cloudflareContainer.binding.maxInstances !== undefined,
          },
          configured: containerBindingConfigured,
          enabled: input.config.cloudflareContainer.enabled,
          eligibleForWorkload: containerWorkloadEligible,
          policyApproved: input.config.cloudflareContainer.policyApproved,
          ready: containerReady,
          role: 'backup_burst_low_medium_trust',
          stagingSmokePassed:
            input.config.cloudflareContainer.stagingSmokePassed,
        },
        gcloud_reference: {
          ready: gcloudReady,
          referenceEnabled: input.config.gcloud.referenceEnabled,
          role: 'reference_sensitive_canonical',
          sensitiveApproved: input.config.gcloud.sensitiveApproved,
        },
        shc_primary: {
          callbackStatus: input.callbackStatus,
          controlStatus: input.shcControlStatus,
          ready: shcReady,
          role: 'primary',
        },
      },
      policy: input.config.policy,
      workloadTrust: input.workloadTrust,
    },
    message,
    name: 'runner_backends',
    status,
  }
}
