import { Schema as S } from 'effect'

import {
  OpenAgentsRunnerProjectionAudience,
  OpenAgentsRunnerWorkloadTrust,
} from './runner-backends'
import {
  type OpenAgentsRunnerGatewayArtifactManifest,
  openAgentsRunnerGatewayPayloadHasPrivateMaterial,
} from './runner-gateway'
import {
  type OpenAgentsRealCloudflareContainerRunnerReadiness,
} from './real-cloudflare-container-runner'

export const OpenAgentsCloudflareContainerRunnerManifestStatus = S.Literals([
  'blocked',
  'draft',
  'ready',
])
export type OpenAgentsCloudflareContainerRunnerManifestStatus =
  typeof OpenAgentsCloudflareContainerRunnerManifestStatus.Type

export const OpenAgentsCloudflareContainerRunnerCommandPhaseKind = S.Literals([
  'cancel',
  'closeout',
  'collect_artifacts',
  'health_probe',
  'prepare_workspace',
  'progress',
  'resolve_grants',
  'scrub_grants',
  'start',
])
export type OpenAgentsCloudflareContainerRunnerCommandPhaseKind =
  typeof OpenAgentsCloudflareContainerRunnerCommandPhaseKind.Type

export const OpenAgentsCloudflareContainerRunnerCommandPhase = S.Struct({
  allowedToolRefs: S.Array(S.String),
  commandRef: S.String,
  inputRefs: S.Array(S.String),
  outputRefs: S.Array(S.String),
  phaseKind: OpenAgentsCloudflareContainerRunnerCommandPhaseKind,
  publicSummaryRef: S.String,
  receiptRefs: S.Array(S.String),
  timeoutMs: S.Int,
})
export type OpenAgentsCloudflareContainerRunnerCommandPhase =
  typeof OpenAgentsCloudflareContainerRunnerCommandPhase.Type

export const OpenAgentsCloudflareContainerRunnerHealthProbe = S.Struct({
  intervalMs: S.Int,
  probeRef: S.String,
  successStatusRefs: S.Array(S.String),
  timeoutMs: S.Int,
})
export type OpenAgentsCloudflareContainerRunnerHealthProbe =
  typeof OpenAgentsCloudflareContainerRunnerHealthProbe.Type

export const OpenAgentsCloudflareContainerRunnerCancelSemantics = S.Struct({
  cancelReceiptRef: S.String,
  gracePeriodMs: S.Int,
  signalRef: S.String,
  timeoutReceiptRef: S.String,
})
export type OpenAgentsCloudflareContainerRunnerCancelSemantics =
  typeof OpenAgentsCloudflareContainerRunnerCancelSemantics.Type

export const OpenAgentsCloudflareContainerRunnerResourceProfile = S.Struct({
  diskRef: S.optionalKey(S.String),
  instanceTypeRef: S.String,
  maxInstancesRef: S.optionalKey(S.String),
  memoryRef: S.optionalKey(S.String),
  vcpuRef: S.optionalKey(S.String),
})
export type OpenAgentsCloudflareContainerRunnerResourceProfile =
  typeof OpenAgentsCloudflareContainerRunnerResourceProfile.Type

export const OpenAgentsCloudflareContainerRunnerImageLifecycleManifest = S.Struct(
  {
    allowedToolRefs: S.Array(S.String),
    artifactRootRef: S.String,
    backendKind: S.Literal('cloudflare_container'),
    bindingRefs: S.Struct({
      classNameRef: S.String,
      durableObjectBindingRef: S.String,
      imageRef: S.String,
    }),
    callbackRef: S.String,
    cancelSemantics: OpenAgentsCloudflareContainerRunnerCancelSemantics,
    closeoutReceiptRefs: S.Array(S.String),
    commandPhases: S.Array(OpenAgentsCloudflareContainerRunnerCommandPhase),
    costRefs: S.Array(S.String),
    healthProbes: S.Array(OpenAgentsCloudflareContainerRunnerHealthProbe),
    manifestRef: S.String,
    publicArtifactRefs: S.Array(S.String),
    publicSummaryRef: S.String,
    resourceProfile: OpenAgentsCloudflareContainerRunnerResourceProfile,
    runtimeRef: S.String,
    status: OpenAgentsCloudflareContainerRunnerManifestStatus,
    statusCaveatRefs: S.Array(S.String),
    timeoutMs: S.Int,
    trustLevel: OpenAgentsRunnerWorkloadTrust,
    versionRef: S.String,
    workspaceRef: S.String,
  },
)
export type OpenAgentsCloudflareContainerRunnerImageLifecycleManifest =
  typeof OpenAgentsCloudflareContainerRunnerImageLifecycleManifest.Type

export const OpenAgentsCloudflareContainerRunnerManifestProjection = S.Struct({
  allowedToolRefs: S.Array(S.String),
  artifactRootRef: S.optionalKey(S.String),
  audience: OpenAgentsRunnerProjectionAudience,
  backendKind: S.Literal('cloudflare_container'),
  callbackRef: S.optionalKey(S.String),
  cancelRefs: S.Array(S.String),
  closeoutReceiptRefs: S.Array(S.String),
  commandPhaseRefs: S.Array(S.String),
  costRefs: S.Array(S.String),
  healthProbeRefs: S.Array(S.String),
  imageRef: S.optionalKey(S.String),
  manifestRef: S.String,
  publicArtifactRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  resourceRefs: S.Array(S.String),
  runtimeRef: S.optionalKey(S.String),
  status: OpenAgentsCloudflareContainerRunnerManifestStatus,
  statusCaveatRefs: S.Array(S.String),
  timeoutRef: S.optionalKey(S.String),
  trustLevel: OpenAgentsRunnerWorkloadTrust,
  versionRef: S.String,
  workspaceRef: S.optionalKey(S.String),
})
export type OpenAgentsCloudflareContainerRunnerManifestProjection =
  typeof OpenAgentsCloudflareContainerRunnerManifestProjection.Type

const unsafeManifestMaterialPattern =
  /(auth[_-]?content|bearer\s+|callback[_-]?token(?![_-]?ref)|cookie|customer[_-]?(email|name)|email[_-]?body|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|github[_-]?token(?![_-]?ref)|mnemonic|oauth|password|preimage|private[_-]?(key|prompt)|prompt[_-]?private|raw[_-]?(email|prompt|runner|run[_-]?log)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|wallet[_-]?(material|secret)|\S+@\S+)/i

const hasUnsafeManifestMaterial = (value: unknown): boolean =>
  unsafeManifestMaterialPattern.test(JSON.stringify(value)) ||
  openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)

const safeRef = (ref: string): boolean =>
  ref.trim() !== '' && !hasUnsafeManifestMaterial(ref)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(safeRef)

const safeRefOrFallback = (ref: string, fallback: string): string =>
  safeRefs([ref])[0] ?? fallback

export class OpenAgentsCloudflareContainerRunnerManifestUnsafeMaterial extends S.TaggedErrorClass<OpenAgentsCloudflareContainerRunnerManifestUnsafeMaterial>()(
  'OpenAgentsCloudflareContainerRunnerManifestUnsafeMaterial',
  {
    message: S.String,
    manifestRef: S.String,
  },
) {}

export const openAgentsCloudflareContainerRunnerManifestHasPrivateMaterial =
  (manifest: unknown): boolean => hasUnsafeManifestMaterial(manifest)

export const validateOpenAgentsCloudflareContainerRunnerManifest = (
  manifest: OpenAgentsCloudflareContainerRunnerImageLifecycleManifest,
):
  | OpenAgentsCloudflareContainerRunnerImageLifecycleManifest
  | OpenAgentsCloudflareContainerRunnerManifestUnsafeMaterial =>
  hasUnsafeManifestMaterial(manifest)
    ? new OpenAgentsCloudflareContainerRunnerManifestUnsafeMaterial({
        manifestRef:
          typeof manifest === 'object' &&
          manifest !== null &&
          'manifestRef' in manifest &&
          typeof manifest.manifestRef === 'string'
            ? manifest.manifestRef
            : 'manifest.cloudflare_container.unknown',
        message:
          'Cloudflare Container runner image/workspace manifests must carry refs and caveats, not raw source archives, prompts, logs, credentials, wallet material, callback secrets, or customer private data.',
      })
    : manifest

const phaseSummaryRefs = (
  phases: ReadonlyArray<OpenAgentsCloudflareContainerRunnerCommandPhase>,
): ReadonlyArray<string> =>
  safeRefs(
    phases.flatMap(phase => [
      `phase.cloudflare_container.${phase.phaseKind}`,
      phase.publicSummaryRef,
      ...phase.receiptRefs,
    ]),
  )

const healthProbeRefs = (
  probes: ReadonlyArray<OpenAgentsCloudflareContainerRunnerHealthProbe>,
): ReadonlyArray<string> =>
  safeRefs(
    probes.flatMap(probe => [
      probe.probeRef,
      ...probe.successStatusRefs,
    ]),
  )

const cancelRefs = (
  cancel: OpenAgentsCloudflareContainerRunnerCancelSemantics,
): ReadonlyArray<string> =>
  safeRefs([
    cancel.cancelReceiptRef,
    cancel.signalRef,
    cancel.timeoutReceiptRef,
  ])

const resourceRefs = (
  resource: OpenAgentsCloudflareContainerRunnerResourceProfile,
): ReadonlyArray<string> =>
  safeRefs([
    resource.instanceTypeRef,
    resource.vcpuRef ?? '',
    resource.memoryRef ?? '',
    resource.diskRef ?? '',
    resource.maxInstancesRef ?? '',
  ])

export const projectOpenAgentsCloudflareContainerRunnerManifest = (
  manifest: OpenAgentsCloudflareContainerRunnerImageLifecycleManifest,
  audience: typeof OpenAgentsRunnerProjectionAudience.Type,
): OpenAgentsCloudflareContainerRunnerManifestProjection => {
  const operator = audience === 'operator'
  const statusCaveatRefs = safeRefs(manifest.statusCaveatRefs)

  return {
    allowedToolRefs: operator ? safeRefs(manifest.allowedToolRefs) : [],
    ...(operator ? { artifactRootRef: manifest.artifactRootRef } : {}),
    audience,
    backendKind: 'cloudflare_container',
    ...(operator ? { callbackRef: manifest.callbackRef } : {}),
    cancelRefs: operator ? cancelRefs(manifest.cancelSemantics) : [],
    closeoutReceiptRefs: operator
      ? safeRefs(manifest.closeoutReceiptRefs)
      : [],
    commandPhaseRefs: operator
      ? phaseSummaryRefs(manifest.commandPhases)
      : [],
    costRefs: operator ? safeRefs(manifest.costRefs) : [],
    healthProbeRefs: operator ? healthProbeRefs(manifest.healthProbes) : [],
    ...(operator ? { imageRef: manifest.bindingRefs.imageRef } : {}),
    manifestRef: safeRefOrFallback(
      manifest.manifestRef,
      'manifest.cloudflare_container.redacted',
    ),
    publicArtifactRefs: operator
      ? safeRefs(manifest.publicArtifactRefs)
      : [],
    publicSummaryRef: safeRefOrFallback(
      manifest.publicSummaryRef,
      'summary.cloudflare_container.manifest.redacted',
    ),
    resourceRefs: operator ? resourceRefs(manifest.resourceProfile) : [],
    ...(operator ? { runtimeRef: manifest.runtimeRef } : {}),
    status: manifest.status,
    statusCaveatRefs,
    ...(operator ? { timeoutRef: `timeout.${manifest.timeoutMs}ms` } : {}),
    trustLevel: manifest.trustLevel,
    versionRef: safeRefOrFallback(
      manifest.versionRef,
      'version.cloudflare_container.manifest.redacted',
    ),
    ...(operator ? { workspaceRef: manifest.workspaceRef } : {}),
  }
}

export const openAgentsCloudflareContainerRunnerReadinessFromManifest = (
  manifest: OpenAgentsCloudflareContainerRunnerImageLifecycleManifest,
  overrides: Partial<OpenAgentsRealCloudflareContainerRunnerReadiness> = {},
): OpenAgentsRealCloudflareContainerRunnerReadiness => ({
  allowedWorkloadTrusts: [manifest.trustLevel],
  backendKind: 'cloudflare_container',
  bindingRefs: manifest.bindingRefs,
  capacityRef: manifest.resourceProfile.maxInstancesRef ??
    manifest.resourceProfile.instanceTypeRef,
  configured: manifest.status === 'ready',
  enabled: manifest.status === 'ready',
  healthStatus: manifest.status === 'ready' ? 'healthy' : 'blocked',
  policyApproved: manifest.status === 'ready',
  policySelected: manifest.status === 'ready',
  publicSummaryRef: manifest.publicSummaryRef,
  runnerId: manifest.runtimeRef,
  stagingSmokePassed: manifest.status === 'ready',
  ...overrides,
})

export const openAgentsCloudflareContainerRunnerArtifactManifestFromLifecycle =
  (
    manifest: OpenAgentsCloudflareContainerRunnerImageLifecycleManifest,
  ): OpenAgentsRunnerGatewayArtifactManifest => ({
    artifactRefs: safeRefs([
      manifest.artifactRootRef,
      ...manifest.commandPhases.flatMap(phase => phase.outputRefs),
    ]),
    digestRef: `digest.${safeRefOrFallback(
      manifest.manifestRef,
      'manifest.cloudflare_container.redacted',
    )}`,
    manifestRef: manifest.manifestRef,
    publicArtifactRefs: safeRefs(manifest.publicArtifactRefs),
    receiptRefs: safeRefs(manifest.closeoutReceiptRefs),
  })
