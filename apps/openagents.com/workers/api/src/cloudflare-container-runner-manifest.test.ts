import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OpenAgentsCloudflareContainerRunnerImageLifecycleManifest,
  OpenAgentsCloudflareContainerRunnerManifestProjection,
  OpenAgentsCloudflareContainerRunnerManifestUnsafeMaterial,
  openAgentsCloudflareContainerRunnerArtifactManifestFromLifecycle,
  openAgentsCloudflareContainerRunnerManifestHasPrivateMaterial,
  openAgentsCloudflareContainerRunnerReadinessFromManifest,
  projectOpenAgentsCloudflareContainerRunnerManifest,
  validateOpenAgentsCloudflareContainerRunnerManifest,
} from './cloudflare-container-runner-manifest'
import {
  makeRealCloudflareContainerRunnerAdapter,
  openAgentsRealCloudflareContainerRunnerBlockedGateRefs,
} from './real-cloudflare-container-runner'

const manifest = (
  overrides: Partial<OpenAgentsCloudflareContainerRunnerImageLifecycleManifest> = {},
): OpenAgentsCloudflareContainerRunnerImageLifecycleManifest => ({
  allowedToolRefs: [
    'tool.opencode.shell_limited',
    'tool.codex.apply_patch',
  ],
  artifactRootRef: 'artifact.cloudflare_container.workspace.output_root',
  backendKind: 'cloudflare_container',
  bindingRefs: {
    classNameRef: 'cloudflare_container.class.openagents_runner',
    durableObjectBindingRef: 'cloudflare_container.do.openagents_runner',
    imageRef: 'cloudflare_container.image.openagents_runner.v1',
  },
  callbackRef: 'callback.runner.gateway.redacted_ref',
  cancelSemantics: {
    cancelReceiptRef: 'receipt.container.cancel.requested',
    gracePeriodMs: 30_000,
    signalRef: 'signal.container.sigterm',
    timeoutReceiptRef: 'receipt.container.cancel.timeout',
  },
  closeoutReceiptRefs: [
    'receipt.container.closeout.scrubbed',
    'receipt.container.closeout.artifacts_uploaded',
  ],
  commandPhases: [
    {
      allowedToolRefs: ['tool.opencode.shell_limited'],
      commandRef: 'command.container.prepare_workspace',
      inputRefs: ['input.assignment.site_builder'],
      outputRefs: ['artifact.container.workspace.prepared'],
      phaseKind: 'prepare_workspace',
      publicSummaryRef: 'summary.container.prepare_workspace',
      receiptRefs: ['receipt.container.prepare_workspace'],
      timeoutMs: 60_000,
    },
    {
      allowedToolRefs: ['tool.codex.apply_patch'],
      commandRef: 'command.container.start',
      inputRefs: ['input.assignment.site_builder'],
      outputRefs: ['artifact.container.generated_site'],
      phaseKind: 'start',
      publicSummaryRef: 'summary.container.start',
      receiptRefs: ['receipt.container.start'],
      timeoutMs: 300_000,
    },
    {
      allowedToolRefs: [],
      commandRef: 'command.container.scrub_grants',
      inputRefs: ['grant.provider_account.ref'],
      outputRefs: ['receipt.container.closeout.scrubbed'],
      phaseKind: 'scrub_grants',
      publicSummaryRef: 'summary.container.scrub_grants',
      receiptRefs: ['receipt.container.scrub_grants'],
      timeoutMs: 30_000,
    },
  ],
  costRefs: ['cost.container.metered.preview'],
  healthProbes: [
    {
      intervalMs: 10_000,
      probeRef: 'probe.container.health.ready',
      successStatusRefs: ['status.container.healthy'],
      timeoutMs: 5_000,
    },
  ],
  manifestRef: 'manifest.cloudflare_container.lifecycle.v1',
  publicArtifactRefs: ['artifact.site.preview_url'],
  publicSummaryRef: 'summary.cloudflare_container.lifecycle.ready',
  resourceProfile: {
    diskRef: 'resource.container.disk.2gb',
    instanceTypeRef: 'resource.container.instance.lite',
    maxInstancesRef: 'resource.container.max_instances.2',
    memoryRef: 'resource.container.memory.256mib',
    vcpuRef: 'resource.container.vcpu.0_0625',
  },
  runtimeRef: 'runtime.cloudflare_container.opencode_codex',
  status: 'ready',
  statusCaveatRefs: ['caveat.container.operator_approved_preview_only'],
  timeoutMs: 600_000,
  trustLevel: 'medium',
  versionRef: 'version.cloudflare_container.lifecycle.v1',
  workspaceRef: 'workspace.container.ephemeral',
  ...overrides,
})

describe('Cloudflare Container runner image lifecycle manifest', () => {
  test('decodes the manifest and derives real-adapter readiness plus artifact refs', async () => {
    const decoded = S.decodeUnknownSync(
      OpenAgentsCloudflareContainerRunnerManifestProjection,
    )(
      projectOpenAgentsCloudflareContainerRunnerManifest(
        manifest(),
        'operator',
      ),
    )
    const readiness = openAgentsCloudflareContainerRunnerReadinessFromManifest(
      manifest(),
    )
    const adapter = makeRealCloudflareContainerRunnerAdapter({ readiness })
    const artifactManifest =
      openAgentsCloudflareContainerRunnerArtifactManifestFromLifecycle(
        manifest(),
      )

    expect(decoded.status).toBe('ready')
    expect(readiness).toMatchObject({
      allowedWorkloadTrusts: ['medium'],
      configured: true,
      enabled: true,
      policyApproved: true,
      policySelected: true,
      stagingSmokePassed: true,
    })
    expect(
      openAgentsRealCloudflareContainerRunnerBlockedGateRefs({
        controlPlaneBound: false,
        readiness,
        workloadTrust: 'medium',
      }),
    ).toEqual(['gate.cloudflare_container.control_plane_bound.blocked'])
    await expect(adapter.checkHealth({
      backendKind: 'cloudflare_container',
      policyRefs: ['policy.runner.container.operator_selected'],
      probeRef: 'probe.container.health',
      requestId: 'health.1',
      runnerId: 'runner.container',
    })).resolves.toBe('blocked')
    expect(artifactManifest).toMatchObject({
      artifactRefs: [
        'artifact.cloudflare_container.workspace.output_root',
        'artifact.container.workspace.prepared',
        'artifact.container.generated_site',
        'receipt.container.closeout.scrubbed',
      ],
      digestRef: 'digest.manifest.cloudflare_container.lifecycle.v1',
      manifestRef: 'manifest.cloudflare_container.lifecycle.v1',
      publicArtifactRefs: ['artifact.site.preview_url'],
      receiptRefs: [
        'receipt.container.closeout.scrubbed',
        'receipt.container.closeout.artifacts_uploaded',
      ],
    })
  })

  test('keeps public and customer projections limited to status caveats', () => {
    const publicProjection = projectOpenAgentsCloudflareContainerRunnerManifest(
      manifest(),
      'public',
    )
    const customerProjection =
      projectOpenAgentsCloudflareContainerRunnerManifest(manifest(), 'customer')
    const publicText = JSON.stringify(publicProjection)

    expect(publicProjection).toMatchObject({
      allowedToolRefs: [],
      audience: 'public',
      backendKind: 'cloudflare_container',
      cancelRefs: [],
      closeoutReceiptRefs: [],
      commandPhaseRefs: [],
      costRefs: [],
      healthProbeRefs: [],
      publicArtifactRefs: [],
      resourceRefs: [],
      status: 'ready',
      statusCaveatRefs: ['caveat.container.operator_approved_preview_only'],
    })
    expect(customerProjection).toEqual({
      ...publicProjection,
      audience: 'customer',
    })
    expect(publicText).not.toContain('image.openagents_runner')
    expect(publicText).not.toContain('workspace.container')
    expect(publicText).not.toContain('metered')
    expect(publicText).not.toContain('callback')
  })

  test('operator projection includes only safe image, workspace, phase, health, cost, and resource refs', () => {
    const projection = projectOpenAgentsCloudflareContainerRunnerManifest(
      manifest(),
      'operator',
    )
    const text = JSON.stringify(projection)

    expect(projection).toMatchObject({
      allowedToolRefs: [
        'tool.opencode.shell_limited',
        'tool.codex.apply_patch',
      ],
      artifactRootRef: 'artifact.cloudflare_container.workspace.output_root',
      callbackRef: 'callback.runner.gateway.redacted_ref',
      costRefs: ['cost.container.metered.preview'],
      imageRef: 'cloudflare_container.image.openagents_runner.v1',
      resourceRefs: [
        'resource.container.instance.lite',
        'resource.container.vcpu.0_0625',
        'resource.container.memory.256mib',
        'resource.container.disk.2gb',
        'resource.container.max_instances.2',
      ],
      runtimeRef: 'runtime.cloudflare_container.opencode_codex',
      timeoutRef: 'timeout.600000ms',
      workspaceRef: 'workspace.container.ephemeral',
    })
    expect(projection.commandPhaseRefs).toContain(
      'phase.cloudflare_container.prepare_workspace',
    )
    expect(projection.healthProbeRefs).toContain('probe.container.health.ready')
    expect(text).not.toMatch(/raw|secret|source_archive|wallet/i)
  })

  test('rejects raw source archives, logs, private prompts, credentials, wallet material, and customer PII', () => {
    const unsafe = manifest({
      commandPhases: [
        {
          allowedToolRefs: ['tool.opencode.shell_limited'],
          commandRef: 'command.container.prepare_workspace',
          inputRefs: [
            'source_archive.raw_private_bundle',
            'raw_prompt.customer_brief',
            'ben@example.com',
          ],
          outputRefs: ['runner_log.raw_full_text', 'wallet_secret'],
          phaseKind: 'prepare_workspace',
          publicSummaryRef: 'summary.container.prepare_workspace',
          receiptRefs: ['receipt.container.prepare_workspace'],
          timeoutMs: 60_000,
        },
      ],
    })
    const result = validateOpenAgentsCloudflareContainerRunnerManifest(unsafe)

    expect(openAgentsCloudflareContainerRunnerManifestHasPrivateMaterial(unsafe))
      .toBe(true)
    expect(result).toBeInstanceOf(
      OpenAgentsCloudflareContainerRunnerManifestUnsafeMaterial,
    )
  })
})
