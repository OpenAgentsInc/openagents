import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsFakeCloudflareContainerRun,
  fakeCloudflareContainerCancelledRun,
  fakeCloudflareContainerCompletedRun,
  fakeCloudflareContainerFailedRun,
  makeFakeCloudflareContainerRunnerAdapter,
} from './fake-cloudflare-container-runner'
import {
  type OpenAgentsRunnerGatewayCancelRequest,
  type OpenAgentsRunnerGatewayDispatchRequest,
  OpenAgentsRunnerGatewayUnsafeCredentialMaterial,
  openAgentsRunnerGatewayPayloadHasPrivateMaterial,
} from './runner-gateway'

const dispatchRequest = (
  overrides: Partial<OpenAgentsRunnerGatewayDispatchRequest> = {},
): OpenAgentsRunnerGatewayDispatchRequest => ({
  artifactManifest: {
    artifactRefs: ['artifact.fake_container.generated_bundle'],
    digestRef: 'digest.fake_container.generated_bundle.sha256',
    manifestRef: 'manifest.fake_container.generated_bundle',
    publicArtifactRefs: ['artifact.fake_container.preview_url'],
    receiptRefs: ['receipt.fake_container.manifest_recorded'],
  },
  assignmentRef: 'assignment.site_builder.fake_container',
  authGrantRef: 'grant.provider_account.codex.fake',
  backendKind: 'cloudflare_container',
  callbackRef: 'callback.fake_container.redacted',
  githubWriteGrantRef: 'grant.github_write.fake',
  goalRef: 'goal.site_builder.fake_container',
  policyRefs: ['policy.runner.container.fake_staging'],
  providerAccountRef: 'provider_account.codex.fake',
  repositoryRef: 'github.openagents.autopilot_omega.main',
  requestId: 'runner.fake-container.1',
  runnerId: 'runner.fake_container.staging',
  runtimeRef: 'runtime.codex.default',
  timeoutMs: 300_000,
  trustLevel: 'medium',
  ...overrides,
})

const cancelRequest = (
  overrides: Partial<OpenAgentsRunnerGatewayCancelRequest> = {},
): OpenAgentsRunnerGatewayCancelRequest => ({
  actorRef: 'operator.openagents',
  backendKind: 'cloudflare_container',
  externalRunRef: 'fake_container.run.runner.fake-container.1',
  policyRefs: ['policy.runner.container.fake_staging'],
  reasonRef: 'reason.customer_requested_cancel',
  requestId: 'runner.fake-container.cancel.1',
  runRef: 'run.site_builder.fake_container',
  runnerId: 'runner.fake_container.staging',
  ...overrides,
})

describe('fake Cloudflare Container runner', () => {
  test('emits deterministic queued, started, artifact, and completed events', () => {
    const run = fakeCloudflareContainerCompletedRun(dispatchRequest())

    expect(S.decodeUnknownSync(OpenAgentsFakeCloudflareContainerRun)(run))
      .toMatchObject({
        backendKind: 'cloudflare_container',
        status: 'completed',
      })
    expect(run.events.map(next => next.eventKind)).toEqual([
      'queued',
      'started',
      'artifact',
      'completed',
    ])
    expect(run.publicArtifactRefs).toEqual([
      'artifact.fake_container.preview_url',
    ])
    expect(run.artifactManifest).toContain(
      'manifest.fake_container.generated_bundle',
    )
    expect(openAgentsRunnerGatewayPayloadHasPrivateMaterial(run)).toBe(false)
  })

  test('produces failed lifecycle events without raw logs', () => {
    const run = fakeCloudflareContainerFailedRun(
      dispatchRequest(),
      'reason.fake_container.build_failed_redacted',
    )

    expect(run.status).toBe('failed')
    expect(run.events.map(next => next.eventKind)).toEqual([
      'queued',
      'started',
      'failed',
    ])
    expect(run.events.at(-1)?.artifactRefs).toContain(
      'reason.fake_container.build_failed_redacted',
    )
    expect(openAgentsRunnerGatewayPayloadHasPrivateMaterial(run)).toBe(false)
  })

  test('produces cancellation receipts without executing customer code', () => {
    const run = fakeCloudflareContainerCancelledRun(cancelRequest())

    expect(run.status).toBe('cancelled')
    expect(run.events.map(next => next.eventKind)).toEqual(['cancelled'])
    expect(run.operatorDebugRefs).toContain(
      'debug.fake_container.runner.fake-container.cancel.1.cancelled_without_execution',
    )
    expect(openAgentsRunnerGatewayPayloadHasPrivateMaterial(run)).toBe(false)
  })

  test('implements the gateway adapter contract for fake dispatch, cancel, health, and callbacks', async () => {
    const adapter = makeFakeCloudflareContainerRunnerAdapter()
    const dispatch = await adapter.dispatch(dispatchRequest())
    const cancel = await adapter.cancel(cancelRequest())
    const health = await adapter.checkHealth({
      backendKind: 'cloudflare_container',
      policyRefs: ['policy.runner.container.fake_staging'],
      probeRef: 'probe.fake_container.health',
      requestId: 'runner.fake-container.health.1',
      runnerId: 'runner.fake_container.staging',
    })
    const callback = await adapter.ingestLifecycleCallback({
      artifactManifestRef: 'manifest.fake_container.generated_bundle',
      backendKind: 'cloudflare_container',
      callbackRef: 'callback.fake_container.redacted',
      dispatchStatus: 'completed',
      eventRefs: ['event.fake_container.completed'],
      externalRunRef: 'fake_container.run.runner.fake-container.1',
      receiptRefs: ['receipt.fake_container.callback.completed'],
      runRef: 'run.site_builder.fake_container',
      runnerId: 'runner.fake_container.staging',
    })

    expect(adapter.backendKind).toBe('cloudflare_container')
    expect(dispatch.status).toBe('completed')
    expect(cancel.status).toBe('cancelled')
    expect(health).toBe('healthy')
    expect(callback).toMatchObject({
      backendKind: 'cloudflare_container',
      status: 'completed',
    })
  })

  test('rejects raw credentials, private source archives, wallet material, and raw logs', async () => {
    const adapter = makeFakeCloudflareContainerRunnerAdapter()

    await expect(
      adapter.dispatch(
        dispatchRequest({
          callbackRef: 'Bearer raw-callback-token',
        }),
      ),
    ).rejects.toBeInstanceOf(OpenAgentsRunnerGatewayUnsafeCredentialMaterial)
    await expect(
      adapter.dispatch(
        dispatchRequest({
          artifactManifest: {
            ...dispatchRequest().artifactManifest,
            artifactRefs: ['source_archive.private_bundle'],
          },
        }),
      ),
    ).rejects.toBeInstanceOf(OpenAgentsRunnerGatewayUnsafeCredentialMaterial)
    await expect(
      adapter.dispatch(
        dispatchRequest({
          providerAccountRef: 'wallet_secret.raw_material',
        }),
      ),
    ).rejects.toBeInstanceOf(OpenAgentsRunnerGatewayUnsafeCredentialMaterial)
    await expect(
      adapter.dispatch(
        dispatchRequest({
          goalRef: 'raw_runner_log.full_text',
        }),
      ),
    ).rejects.toBeInstanceOf(OpenAgentsRunnerGatewayUnsafeCredentialMaterial)
  })
})
