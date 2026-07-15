import type { RunnerWorkloadTrust } from './config'

export type RunnerBackendCheckStatus = 'blocked' | 'ok' | 'unknown' | 'warning'

export type RunnerBackendReadinessCheck = Readonly<{
  details: Record<string, unknown>
  message: string
  name: 'runner_backends'
  status: RunnerBackendCheckStatus
}>

type RunnerBackendReadinessInput = Readonly<{
  callbackStatus: RunnerBackendCheckStatus
  gcloudControlStatus: RunnerBackendCheckStatus
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
  const gcloudReady =
    input.gcloudControlStatus === 'ok' && input.callbackStatus === 'ok'
  const status: RunnerBackendCheckStatus = gcloudReady ? 'ok' : 'blocked'
  const message = gcloudReady
    ? 'Google Cloud runner and callback paths are ready.'
    : 'Google Cloud runner control and callback paths must both be ready.'

  return {
    details: {
      lanes: {
        google_cloud: {
          callbackStatus: input.callbackStatus,
          controlStatus: input.gcloudControlStatus,
          ready: gcloudReady,
          role: 'sole_runtime',
        },
      },
      workloadTrust: input.workloadTrust,
    },
    message,
    name: 'runner_backends',
    status,
  }
}
