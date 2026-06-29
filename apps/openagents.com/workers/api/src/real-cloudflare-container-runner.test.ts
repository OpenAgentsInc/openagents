import { Schema as S } from 'effect'
import { describe, expect, test, vi } from 'vitest'

import {
  type OpenAgentsRunnerGatewayCancelRequest,
  type OpenAgentsRunnerGatewayDispatchRequest,
  OpenAgentsRunnerGatewayDispatchRequest as OpenAgentsRunnerGatewayDispatchRequestSchema,
  type OpenAgentsRunnerGatewayHealthCheckRequest,
  type OpenAgentsRunnerGatewayLifecycleCallback,
  OpenAgentsRunnerGatewayUnsafeCredentialMaterial,
  openAgentsRunnerGatewayPayloadHasPrivateMaterial,
} from './runner-gateway'
import {
  type OpenAgentsRealCloudflareContainerRunnerControlReceipt,
  type OpenAgentsRealCloudflareContainerRunnerControlPlane,
  type OpenAgentsRealCloudflareContainerRunnerReadiness,
  makeRealCloudflareContainerRunnerAdapter,
  openAgentsRealCloudflareContainerRunnerBlockedGateRefs,
} from './real-cloudflare-container-runner'

const readiness = (
  overrides: Partial<OpenAgentsRealCloudflareContainerRunnerReadiness> = {},
): OpenAgentsRealCloudflareContainerRunnerReadiness => ({
  allowedWorkloadTrusts: ['low', 'medium'],
  backendKind: 'cloudflare_container',
  bindingRefs: {
    classNameRef: 'cloudflare_container.class.openagents_runner',
    durableObjectBindingRef: 'cloudflare_container.do.openagents_runner',
    imageRef: 'cloudflare_container.image.openagents_runner',
  },
  capacityRef: 'capacity.cloudflare_container.low_medium.preview',
  configured: true,
  enabled: true,
  healthStatus: 'healthy',
  policyApproved: true,
  policySelected: true,
  publicSummaryRef: 'summary.cloudflare_container.ready_for_staging',
  runnerId: 'runner.cloudflare_container.preview',
  stagingSmokePassed: true,
  ...overrides,
})

const dispatchRequest = (
  overrides: Partial<OpenAgentsRunnerGatewayDispatchRequest> = {},
): OpenAgentsRunnerGatewayDispatchRequest =>
  S.decodeUnknownSync(OpenAgentsRunnerGatewayDispatchRequestSchema)({
    artifactManifest: {
      artifactRefs: ['artifact.site.bundle'],
      digestRef: 'digest.site.bundle.sha256',
      manifestRef: 'manifest.site.bundle',
      publicArtifactRefs: ['artifact.site.preview_url'],
      receiptRefs: ['receipt.runner.requested'],
    },
    assignmentRef: 'assignment.site_builder.1',
    authGrantRef: 'grant.provider_account.codex.1',
    backendKind: 'cloudflare_container',
    callbackRef: 'callback.runner.gateway.redacted_ref',
    githubWriteGrantRef: 'grant.github_write.public_fork_pr',
    goalRef: 'goal.site_builder.1',
    policyRefs: ['policy.runner.container.operator_selected'],
    providerAccountRef: 'provider_account.codex.1',
    repositoryRef: 'github.openagents.autopilot_omega.main',
    requestId: 'runner.dispatch.1',
    runnerId: 'runner.cloudflare_container.preview',
    runtimeRef: 'runtime.codex.default',
    timeoutMs: 300_000,
    trustLevel: 'medium',
    ...overrides,
  })

const cancelRequest = (): OpenAgentsRunnerGatewayCancelRequest => ({
  actorRef: 'operator.chris',
  backendKind: 'cloudflare_container',
  externalRunRef: 'cloudflare_container.run.1',
  policyRefs: ['policy.runner.container.operator_selected'],
  reasonRef: 'reason.operator_cancelled',
  requestId: 'runner.cancel.1',
  runRef: 'run.site_builder.1',
  runnerId: 'runner.cloudflare_container.preview',
})

const healthRequest = (): OpenAgentsRunnerGatewayHealthCheckRequest => ({
  backendKind: 'cloudflare_container',
  policyRefs: ['policy.runner.container.operator_selected'],
  probeRef: 'probe.container.health',
  requestId: 'runner.health.1',
  runnerId: 'runner.cloudflare_container.preview',
})

const lifecycleCallback = (): OpenAgentsRunnerGatewayLifecycleCallback => ({
  artifactManifestRef: 'manifest.site.bundle',
  backendKind: 'cloudflare_container',
  callbackRef: 'callback.runner.gateway.redacted_ref',
  dispatchStatus: 'completed',
  eventRefs: ['event.container.completed'],
  externalRunRef: 'cloudflare_container.run.1',
  receiptRefs: ['receipt.container.completed'],
  runRef: 'run.site_builder.1',
  runnerId: 'runner.cloudflare_container.preview',
})

const receipt = (
  overrides: Partial<OpenAgentsRealCloudflareContainerRunnerControlReceipt> = {},
): OpenAgentsRealCloudflareContainerRunnerControlReceipt => ({
  backendKind: 'cloudflare_container',
  externalRunRef: 'cloudflare_container.run.default',
  operatorDiagnosticRefs: ['diagnostic.container.default'],
  publicSummaryRef: 'summary.container.default',
  receiptRefs: ['receipt.container.default'],
  status: 'starting',
  ...overrides,
})

const controlPlane = (
  overrides: Partial<OpenAgentsRealCloudflareContainerRunnerControlPlane> = {},
): OpenAgentsRealCloudflareContainerRunnerControlPlane => ({
  cancel: vi.fn(async () =>
    receipt({
      externalRunRef: 'cloudflare_container.run.cancelled',
      operatorDiagnosticRefs: ['diagnostic.container.cancelled'],
      publicSummaryRef: 'summary.container.cancelled',
      receiptRefs: ['receipt.container.cancelled'],
      status: 'cancelled',
    })
  ),
  checkHealth: vi.fn(async () => 'healthy' as const),
  dispatch: vi.fn(async () =>
    receipt({
      externalRunRef: 'cloudflare_container.run.started',
      operatorDiagnosticRefs: ['diagnostic.container.started'],
      publicSummaryRef: 'summary.container.started',
      receiptRefs: ['receipt.container.started'],
      status: 'starting',
    })
  ),
  ingestLifecycleCallback: vi.fn(async () =>
    receipt({
      externalRunRef: 'cloudflare_container.run.completed',
      operatorDiagnosticRefs: ['diagnostic.container.completed'],
      publicSummaryRef: 'summary.container.completed',
      receiptRefs: ['receipt.container.completed'],
      status: 'completed',
    })
  ),
  ...overrides,
})

describe('real Cloudflare Container runner adapter contract', () => {
  test('returns blocked receipts when the real Container control plane is not bound', async () => {
    const adapter = makeRealCloudflareContainerRunnerAdapter({
      readiness: readiness(),
    })

    const receipt = await adapter.dispatch(dispatchRequest())
    const health = await adapter.checkHealth(healthRequest())

    expect(receipt).toMatchObject({
      backendKind: 'cloudflare_container',
      externalRunRef: 'cloudflare_container.blocked.runner.dispatch.1',
      receiptRefs: [
        'receipt.cloudflare_container.dispatch.runner.dispatch.1.blocked',
        'gate.cloudflare_container.control_plane_bound.blocked',
      ],
      status: 'blocked',
    })
    expect(health).toBe('blocked')
    expect(openAgentsRunnerGatewayPayloadHasPrivateMaterial(receipt)).toBe(
      false,
    )
  })

  test('reports all blocked gates before dispatch can call a control plane', () => {
    expect(
      openAgentsRealCloudflareContainerRunnerBlockedGateRefs({
        controlPlaneBound: false,
        readiness: readiness({
          bindingRefs: {},
          configured: false,
          enabled: false,
          policyApproved: false,
          policySelected: false,
          stagingSmokePassed: false,
        }),
        workloadTrust: 'sensitive',
      }),
    ).toEqual([
      'gate.cloudflare_container.policy_selected.blocked',
      'gate.cloudflare_container.enabled.blocked',
      'gate.cloudflare_container.binding_configured.blocked',
      'gate.cloudflare_container.staging_smoke_passed.blocked',
      'gate.cloudflare_container.policy_approved.blocked',
      'gate.cloudflare_container.control_plane_bound.blocked',
      'gate.cloudflare_container.workload_trust_allowed.blocked',
    ])
  })

  test('calls the injected control plane only after readiness gates pass', async () => {
    const plane = controlPlane()
    const adapter = makeRealCloudflareContainerRunnerAdapter({
      controlPlane: plane,
      readiness: readiness(),
    })

    await expect(adapter.dispatch(dispatchRequest())).resolves.toMatchObject({
      backendKind: 'cloudflare_container',
      externalRunRef: 'cloudflare_container.run.started',
      receiptRefs: ['receipt.container.started'],
      status: 'starting',
    })
    await expect(adapter.cancel(cancelRequest())).resolves.toMatchObject({
      status: 'cancelled',
    })
    await expect(adapter.checkHealth(healthRequest())).resolves.toBe('healthy')
    await expect(adapter.ingestLifecycleCallback(lifecycleCallback())).resolves
      .toMatchObject({ status: 'completed' })

    expect(plane.dispatch).toHaveBeenCalledTimes(1)
    expect(plane.cancel).toHaveBeenCalledTimes(1)
    expect(plane.checkHealth).toHaveBeenCalledTimes(1)
    expect(plane.ingestLifecycleCallback).toHaveBeenCalledTimes(1)
  })

  test('rejects raw credential or callback-token material before control-plane calls', async () => {
    const plane = controlPlane()
    const adapter = makeRealCloudflareContainerRunnerAdapter({
      controlPlane: plane,
      readiness: readiness(),
    })

    await expect(
      adapter.dispatch(dispatchRequest({ callbackRef: 'Bearer raw-token' })),
    ).rejects.toBeInstanceOf(OpenAgentsRunnerGatewayUnsafeCredentialMaterial)
    expect(plane.dispatch).not.toHaveBeenCalled()
  })

  test('rejects unsafe control-plane receipts before returning gateway receipts', async () => {
    const adapter = makeRealCloudflareContainerRunnerAdapter({
      controlPlane: controlPlane({
        dispatch: vi.fn(async () => receipt({
          externalRunRef: 'cloudflare_container.run.started',
          operatorDiagnosticRefs: ['raw_runner_log.full_text'],
          publicSummaryRef: 'summary.container.started',
          receiptRefs: ['receipt.container.started'],
          status: 'starting',
        })),
      }),
      readiness: readiness(),
    })

    await expect(adapter.dispatch(dispatchRequest())).rejects.toBeInstanceOf(
      OpenAgentsRunnerGatewayUnsafeCredentialMaterial,
    )
  })
})
