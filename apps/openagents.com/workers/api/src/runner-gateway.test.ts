import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OpenAgentsRunnerGatewayAdapterState,
  type OpenAgentsRunnerGatewayDispatchRequest,
  OpenAgentsRunnerGatewayArtifactManifest,
  OpenAgentsRunnerGatewayBackendNotSelected,
  OpenAgentsRunnerGatewayCancelRequest,
  OpenAgentsRunnerGatewayDispatchRequest as OpenAgentsRunnerGatewayDispatchRequestSchema,
  OpenAgentsRunnerGatewayDisabledBackend,
  OpenAgentsRunnerGatewayError,
  OpenAgentsRunnerGatewayHealthCheckRequest,
  OpenAgentsRunnerGatewayLifecycleCallback,
  OpenAgentsRunnerGatewayMalformedRequest,
  OpenAgentsRunnerGatewayUnsupportedBackend,
  openAgentsRunnerGatewayErrorFromUnknown,
  openAgentsRunnerGatewayErrorStatus,
  openAgentsRunnerGatewayPayloadHasPrivateMaterial,
  selectOpenAgentsRunnerGatewayAdapter,
  validateOpenAgentsRunnerGatewayPayload,
} from './runner-gateway'

const adapter = (
  overrides: Partial<OpenAgentsRunnerGatewayAdapterState> = {},
): OpenAgentsRunnerGatewayAdapterState => ({
  backendKind: 'cloudflare_container',
  capacityRef: 'capacity.container.low_medium_available',
  configured: true,
  enabled: true,
  healthStatus: 'healthy',
  policySelected: true,
  reasonRefs: ['policy.runner.container.operator_selected'],
  ...overrides,
})

const dispatchRequest = (
  overrides: Partial<OpenAgentsRunnerGatewayDispatchRequest> = {},
): OpenAgentsRunnerGatewayDispatchRequest =>
  S.decodeUnknownSync(OpenAgentsRunnerGatewayDispatchRequestSchema)({
    artifactManifest: {
      artifactRefs: ['artifact.site.preview_bundle'],
      digestRef: 'digest.site.preview_bundle.sha256',
      manifestRef: 'manifest.site.preview_bundle',
      publicArtifactRefs: ['artifact.site.preview_url'],
      receiptRefs: ['receipt.runner.dispatch.requested'],
    },
    assignmentRef: 'assignment.site_builder.revision_4',
    authGrantRef: 'grant.provider_account.codex.account_3',
    backendKind: 'cloudflare_container',
    callbackRef: 'callback.runner.gateway.redacted_ref',
    githubWriteGrantRef: 'grant.github_write.repo_branch',
    goalRef: 'goal.site_builder.revision_4',
    policyRefs: ['policy.runner.container.operator_selected'],
    providerAccountRef: 'provider_account.codex.account_3',
    repositoryRef: 'github.openagents.autopilot_omega.main',
    requestId: 'runner_gateway.dispatch.1',
    runnerId: 'runner.container.preview',
    runtimeRef: 'runtime.codex.default',
    timeoutMs: 300_000,
    trustLevel: 'medium',
    ...overrides,
  })

describe('OpenAgents runner gateway contract', () => {
  test('models dispatch, cancel, health, lifecycle, and artifact envelopes', () => {
    expect(
      S.decodeUnknownSync(OpenAgentsRunnerGatewayArtifactManifest)(
        dispatchRequest().artifactManifest,
      ).manifestRef,
    ).toBe('manifest.site.preview_bundle')
    expect(
      S.decodeUnknownSync(OpenAgentsRunnerGatewayCancelRequest)({
        actorRef: 'operator.chris',
        backendKind: 'shc_vm',
        externalRunRef: 'shc.run.123',
        policyRefs: ['policy.runner.shc_primary'],
        reasonRef: 'reason.customer_requested_cancel',
        requestId: 'runner_gateway.cancel.1',
        runRef: 'run.agent.123',
        runnerId: 'runner.shc.primary',
      }).backendKind,
    ).toBe('shc_vm')
    expect(
      S.decodeUnknownSync(OpenAgentsRunnerGatewayHealthCheckRequest)({
        backendKind: 'gcloud_vm',
        policyRefs: ['policy.runner.gcloud.reference'],
        probeRef: 'probe.runner.gcloud.health',
        requestId: 'runner_gateway.health.1',
        runnerId: 'runner.gcloud.reference',
      }).backendKind,
    ).toBe('gcloud_vm')
    expect(
      S.decodeUnknownSync(OpenAgentsRunnerGatewayLifecycleCallback)({
        artifactManifestRef: 'manifest.site.preview_bundle',
        backendKind: 'cloudflare_container',
        callbackRef: 'callback.runner.gateway.redacted_ref',
        dispatchStatus: 'completed',
        eventRefs: ['event.runner.completed'],
        externalRunRef: 'container.run.123',
        receiptRefs: ['receipt.runner.completed'],
        runRef: 'run.site_builder.123',
        runnerId: 'runner.container.preview',
      }).dispatchStatus,
    ).toBe('completed')
  })

  test('selects only an enabled, configured, policy-selected adapter', () => {
    const result = selectOpenAgentsRunnerGatewayAdapter({
      backendKind: 'cloudflare_container',
      operation: 'dispatch',
      states: [
        adapter({ backendKind: 'shc_vm', policySelected: false }),
        adapter(),
      ],
    })

    expect(result).toEqual({
      _tag: 'OpenAgentsRunnerGatewaySelected',
      selection: {
        backendKind: 'cloudflare_container',
        capacityRef: 'capacity.container.low_medium_available',
        healthStatus: 'healthy',
        reasonRefs: ['policy.runner.container.operator_selected'],
      },
    })
  })

  test('denies backends that are not selected by policy', () => {
    const result = selectOpenAgentsRunnerGatewayAdapter({
      backendKind: 'cloudflare_container',
      operation: 'dispatch',
      states: [adapter({ policySelected: false })],
    })

    expect(result._tag).toBe('OpenAgentsRunnerGatewayDenied')
    if (result._tag === 'OpenAgentsRunnerGatewayDenied') {
      expect(result.error).toBeInstanceOf(
        OpenAgentsRunnerGatewayBackendNotSelected,
      )
      expect(openAgentsRunnerGatewayErrorStatus(result.error)).toBe(409)
    }
  })

  test('denies disabled or unconfigured backends without dispatch side effects', () => {
    const disabled = selectOpenAgentsRunnerGatewayAdapter({
      backendKind: 'cloudflare_container',
      operation: 'dispatch',
      states: [adapter({ configured: false, enabled: true })],
    })

    expect(disabled._tag).toBe('OpenAgentsRunnerGatewayDenied')
    if (disabled._tag === 'OpenAgentsRunnerGatewayDenied') {
      expect(disabled.error).toBeInstanceOf(
        OpenAgentsRunnerGatewayDisabledBackend,
      )
      expect(openAgentsRunnerGatewayErrorStatus(disabled.error)).toBe(409)
    }
  })

  test('maps unknown and unsupported backend failures to typed errors', () => {
    const unsupported = selectOpenAgentsRunnerGatewayAdapter({
      backendKind: 'gcloud_vm',
      operation: 'health_check',
      states: [adapter()],
    })
    const malformed = openAgentsRunnerGatewayErrorFromUnknown(
      'dispatch',
      new Error('bad assignment'),
    )

    expect(unsupported._tag).toBe('OpenAgentsRunnerGatewayDenied')
    if (unsupported._tag === 'OpenAgentsRunnerGatewayDenied') {
      expect(unsupported.error).toBeInstanceOf(
        OpenAgentsRunnerGatewayUnsupportedBackend,
      )
      expect(openAgentsRunnerGatewayErrorStatus(unsupported.error)).toBe(422)
    }
    expect(malformed).toBeInstanceOf(OpenAgentsRunnerGatewayMalformedRequest)
    expect(
      S.decodeUnknownSync(OpenAgentsRunnerGatewayError)(malformed),
    ).toMatchObject({
      _tag: 'OpenAgentsRunnerGatewayMalformedRequest',
      message: 'bad assignment',
      operation: 'dispatch',
    })
    expect(openAgentsRunnerGatewayErrorStatus(malformed)).toBe(400)
  })

  test('accepts refs and grants but rejects raw credential-shaped material', () => {
    const safeRequest = dispatchRequest()
    const unsafeRequest = dispatchRequest({
      callbackRef: 'Bearer raw-callback-token',
    })

    expect(openAgentsRunnerGatewayPayloadHasPrivateMaterial(safeRequest)).toBe(
      false,
    )
    expect(validateOpenAgentsRunnerGatewayPayload('dispatch', safeRequest))
      .toEqual(safeRequest)
    expect(openAgentsRunnerGatewayPayloadHasPrivateMaterial(unsafeRequest))
      .toBe(true)
    expect(
      validateOpenAgentsRunnerGatewayPayload('dispatch', unsafeRequest),
    ).toMatchObject({
      _tag: 'OpenAgentsRunnerGatewayUnsafeCredentialMaterial',
      operation: 'dispatch',
    })
  })
})
