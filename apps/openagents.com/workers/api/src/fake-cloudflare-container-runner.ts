import { Schema as S } from 'effect'

import {
  type OpenAgentsRunnerGatewayAdapter,
  type OpenAgentsRunnerGatewayArtifactManifest,
  type OpenAgentsRunnerGatewayCancelRequest,
  type OpenAgentsRunnerGatewayDispatchReceipt,
  type OpenAgentsRunnerGatewayDispatchRequest,
  type OpenAgentsRunnerGatewayHealthCheckRequest,
  type OpenAgentsRunnerGatewayLifecycleCallback,
  OpenAgentsRunnerGatewayUnsafeCredentialMaterial,
  isOpenAgentsRunnerGatewayError,
  openAgentsRunnerGatewayPayloadHasPrivateMaterial,
  validateOpenAgentsRunnerGatewayPayload,
} from './runner-gateway'

export const OpenAgentsFakeCloudflareContainerRunnerEventKind = S.Literals([
  'artifact',
  'cancelled',
  'completed',
  'failed',
  'queued',
  'started',
])
export type OpenAgentsFakeCloudflareContainerRunnerEventKind =
  typeof OpenAgentsFakeCloudflareContainerRunnerEventKind.Type

export const OpenAgentsFakeCloudflareContainerRunnerEvent = S.Struct({
  artifactRefs: S.Array(S.String),
  eventKind: OpenAgentsFakeCloudflareContainerRunnerEventKind,
  eventRef: S.String,
  operatorDebugRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  status: S.String,
  summaryRef: S.String,
})
export type OpenAgentsFakeCloudflareContainerRunnerEvent =
  typeof OpenAgentsFakeCloudflareContainerRunnerEvent.Type

export const OpenAgentsFakeCloudflareContainerRun = S.Struct({
  artifactManifest: S.Array(S.String),
  artifactManifestRef: S.String,
  backendKind: S.Literal('cloudflare_container'),
  events: S.Array(OpenAgentsFakeCloudflareContainerRunnerEvent),
  externalRunRef: S.String,
  operatorDebugRefs: S.Array(S.String),
  publicArtifactRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  status: S.Literals(['cancelled', 'completed', 'failed']),
})
export type OpenAgentsFakeCloudflareContainerRun =
  typeof OpenAgentsFakeCloudflareContainerRun.Type

const safeRefPart = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9_.-]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 96) || 'request'

const event = (
  requestId: string,
  eventKind: OpenAgentsFakeCloudflareContainerRunnerEventKind,
  status: string,
  artifactRefs: ReadonlyArray<string> = [],
): OpenAgentsFakeCloudflareContainerRunnerEvent => {
  const baseRef = `fake_container.${safeRefPart(requestId)}.${eventKind}`

  return {
    artifactRefs,
    eventKind,
    eventRef: `event.${baseRef}`,
    operatorDebugRefs: [`debug.${baseRef}.no_customer_code_executed`],
    receiptRefs: [`receipt.${baseRef}`],
    status,
    summaryRef: `summary.${baseRef}`,
  }
}

const ensureSafePayload = <Payload>(
  operation: Parameters<typeof validateOpenAgentsRunnerGatewayPayload>[0],
  payload: Payload,
): Payload => {
  const result = validateOpenAgentsRunnerGatewayPayload(operation, payload)

  if (isOpenAgentsRunnerGatewayError(result)) {
    throw result
  }

  return result
}

const manifestRefs = (
  manifest: OpenAgentsRunnerGatewayArtifactManifest,
): ReadonlyArray<string> => [
  manifest.manifestRef,
  manifest.digestRef,
  ...manifest.artifactRefs,
  ...manifest.publicArtifactRefs,
  ...manifest.receiptRefs,
]

const assertFakeRunPublicSafe = (
  run: OpenAgentsFakeCloudflareContainerRun,
): OpenAgentsFakeCloudflareContainerRun => {
  if (openAgentsRunnerGatewayPayloadHasPrivateMaterial(run)) {
    throw new OpenAgentsRunnerGatewayUnsafeCredentialMaterial({
      message:
        'Fake Container runner output contained private material after redaction.',
      operation: 'dispatch',
    })
  }

  return run
}

export const fakeCloudflareContainerCompletedRun = (
  request: OpenAgentsRunnerGatewayDispatchRequest,
): OpenAgentsFakeCloudflareContainerRun => {
  const safeRequest = ensureSafePayload('dispatch', request)
  const artifactRefs = manifestRefs(safeRequest.artifactManifest)
  const events = [
    event(safeRequest.requestId, 'queued', 'queued'),
    event(safeRequest.requestId, 'started', 'running'),
    event(safeRequest.requestId, 'artifact', 'artifact_ready', artifactRefs),
    event(safeRequest.requestId, 'completed', 'completed', artifactRefs),
  ]
  const receiptRefs = events.flatMap(next => next.receiptRefs)

  return assertFakeRunPublicSafe({
    artifactManifest: artifactRefs,
    artifactManifestRef: safeRequest.artifactManifest.manifestRef,
    backendKind: 'cloudflare_container',
    events,
    externalRunRef: `fake_container.run.${safeRefPart(safeRequest.requestId)}`,
    operatorDebugRefs: [
      `debug.fake_container.${safeRefPart(safeRequest.requestId)}.staging_adapter`,
      `debug.fake_container.${safeRefPart(safeRequest.requestId)}.no_customer_code_executed`,
    ],
    publicArtifactRefs: safeRequest.artifactManifest.publicArtifactRefs,
    receiptRefs,
    status: 'completed',
  })
}

export const fakeCloudflareContainerFailedRun = (
  request: OpenAgentsRunnerGatewayDispatchRequest,
  reasonRef: string,
): OpenAgentsFakeCloudflareContainerRun => {
  const safeRequest = ensureSafePayload('dispatch', request)
  const artifactRefs = manifestRefs(safeRequest.artifactManifest)
  const safeReasonRef = ensureSafePayload('lifecycle_callback', { reasonRef })
  const events = [
    event(safeRequest.requestId, 'queued', 'queued'),
    event(safeRequest.requestId, 'started', 'running'),
    event(safeRequest.requestId, 'failed', 'failed', [
      ...artifactRefs,
      safeReasonRef.reasonRef,
    ]),
  ]
  const receiptRefs = events.flatMap(next => next.receiptRefs)

  return assertFakeRunPublicSafe({
    artifactManifest: artifactRefs,
    artifactManifestRef: safeRequest.artifactManifest.manifestRef,
    backendKind: 'cloudflare_container',
    events,
    externalRunRef: `fake_container.run.${safeRefPart(safeRequest.requestId)}`,
    operatorDebugRefs: [
      `debug.fake_container.${safeRefPart(safeRequest.requestId)}.failed_without_execution`,
    ],
    publicArtifactRefs: safeRequest.artifactManifest.publicArtifactRefs,
    receiptRefs,
    status: 'failed',
  })
}

export const fakeCloudflareContainerCancelledRun = (
  request: OpenAgentsRunnerGatewayCancelRequest,
): OpenAgentsFakeCloudflareContainerRun => {
  const safeRequest = ensureSafePayload('cancel', request)
  const events = [event(safeRequest.requestId, 'cancelled', 'cancelled')]
  const receiptRefs = events.flatMap(next => next.receiptRefs)

  return assertFakeRunPublicSafe({
    artifactManifest: [],
    artifactManifestRef: `manifest.fake_container.${safeRefPart(safeRequest.requestId)}.cancelled`,
    backendKind: 'cloudflare_container',
    events,
    externalRunRef:
      safeRequest.externalRunRef ??
      `fake_container.run.${safeRefPart(safeRequest.runRef)}`,
    operatorDebugRefs: [
      `debug.fake_container.${safeRefPart(safeRequest.requestId)}.cancelled_without_execution`,
    ],
    publicArtifactRefs: [],
    receiptRefs,
    status: 'cancelled',
  })
}

const dispatchReceipt = (
  run: OpenAgentsFakeCloudflareContainerRun,
): OpenAgentsRunnerGatewayDispatchReceipt => ({
  backendKind: 'cloudflare_container',
  externalRunRef: run.externalRunRef,
  receiptRefs: run.receiptRefs,
  status: run.status,
})

export const makeFakeCloudflareContainerRunnerAdapter =
  (): OpenAgentsRunnerGatewayAdapter => ({
    backendKind: 'cloudflare_container',
    cancel: async request =>
      dispatchReceipt(fakeCloudflareContainerCancelledRun(request)),
    checkHealth: async (request: OpenAgentsRunnerGatewayHealthCheckRequest) => {
      ensureSafePayload('health_check', request)

      return 'healthy'
    },
    dispatch: async request =>
      dispatchReceipt(fakeCloudflareContainerCompletedRun(request)),
    ingestLifecycleCallback: async (
      callback: OpenAgentsRunnerGatewayLifecycleCallback,
    ) => {
      const safeCallback = ensureSafePayload('lifecycle_callback', callback)

      return {
        backendKind: 'cloudflare_container',
        externalRunRef:
          safeCallback.externalRunRef ??
          `fake_container.run.${safeRefPart(safeCallback.runRef)}`,
        receiptRefs: safeCallback.receiptRefs,
        status: safeCallback.dispatchStatus,
      }
    },
  })
