import { Schema as S } from 'effect'

import {
  OpenAgentsRunnerBackendKind,
  OpenAgentsRunnerDispatchStatus,
  OpenAgentsRunnerWorkloadTrust,
} from './runner-backends'

export const OpenAgentsRunnerGatewayOperation = S.Literals([
  'artifact_manifest',
  'cancel',
  'dispatch',
  'health_check',
  'lifecycle_callback',
])
export type OpenAgentsRunnerGatewayOperation =
  typeof OpenAgentsRunnerGatewayOperation.Type

export const OpenAgentsRunnerGatewayHealthStatus = S.Literals([
  'blocked',
  'degraded',
  'healthy',
  'unknown',
])
export type OpenAgentsRunnerGatewayHealthStatus =
  typeof OpenAgentsRunnerGatewayHealthStatus.Type

export const OpenAgentsRunnerGatewayArtifactManifest = S.Struct({
  artifactRefs: S.Array(S.String),
  digestRef: S.String,
  manifestRef: S.String,
  publicArtifactRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
})
export type OpenAgentsRunnerGatewayArtifactManifest =
  typeof OpenAgentsRunnerGatewayArtifactManifest.Type

export const OpenAgentsRunnerGatewayDispatchRequest = S.Struct({
  artifactManifest: OpenAgentsRunnerGatewayArtifactManifest,
  assignmentRef: S.String,
  authGrantRef: S.optionalKey(S.String),
  backendKind: OpenAgentsRunnerBackendKind,
  callbackRef: S.String,
  githubWriteGrantRef: S.optionalKey(S.String),
  goalRef: S.String,
  policyRefs: S.Array(S.String),
  providerAccountRef: S.optionalKey(S.String),
  repositoryRef: S.String,
  requestId: S.String,
  runnerId: S.String,
  runtimeRef: S.String,
  timeoutMs: S.Int,
  trustLevel: OpenAgentsRunnerWorkloadTrust,
})
export type OpenAgentsRunnerGatewayDispatchRequest =
  typeof OpenAgentsRunnerGatewayDispatchRequest.Type

export const OpenAgentsRunnerGatewayCancelRequest = S.Struct({
  actorRef: S.String,
  backendKind: OpenAgentsRunnerBackendKind,
  externalRunRef: S.optionalKey(S.String),
  policyRefs: S.Array(S.String),
  reasonRef: S.String,
  requestId: S.String,
  runRef: S.String,
  runnerId: S.String,
})
export type OpenAgentsRunnerGatewayCancelRequest =
  typeof OpenAgentsRunnerGatewayCancelRequest.Type

export const OpenAgentsRunnerGatewayHealthCheckRequest = S.Struct({
  backendKind: OpenAgentsRunnerBackendKind,
  policyRefs: S.Array(S.String),
  probeRef: S.String,
  requestId: S.String,
  runnerId: S.String,
})
export type OpenAgentsRunnerGatewayHealthCheckRequest =
  typeof OpenAgentsRunnerGatewayHealthCheckRequest.Type

export const OpenAgentsRunnerGatewayLifecycleCallback = S.Struct({
  artifactManifestRef: S.optionalKey(S.String),
  backendKind: OpenAgentsRunnerBackendKind,
  callbackRef: S.String,
  dispatchStatus: OpenAgentsRunnerDispatchStatus,
  eventRefs: S.Array(S.String),
  externalRunRef: S.optionalKey(S.String),
  receiptRefs: S.Array(S.String),
  runRef: S.String,
  runnerId: S.String,
})
export type OpenAgentsRunnerGatewayLifecycleCallback =
  typeof OpenAgentsRunnerGatewayLifecycleCallback.Type

export const OpenAgentsRunnerGatewayAdapterState = S.Struct({
  backendKind: OpenAgentsRunnerBackendKind,
  capacityRef: S.String,
  configured: S.Boolean,
  enabled: S.Boolean,
  healthStatus: OpenAgentsRunnerGatewayHealthStatus,
  policySelected: S.Boolean,
  reasonRefs: S.Array(S.String),
})
export type OpenAgentsRunnerGatewayAdapterState =
  typeof OpenAgentsRunnerGatewayAdapterState.Type

export const OpenAgentsRunnerGatewayAdapterSelection = S.Struct({
  backendKind: OpenAgentsRunnerBackendKind,
  capacityRef: S.String,
  healthStatus: OpenAgentsRunnerGatewayHealthStatus,
  reasonRefs: S.Array(S.String),
})
export type OpenAgentsRunnerGatewayAdapterSelection =
  typeof OpenAgentsRunnerGatewayAdapterSelection.Type

export const OpenAgentsRunnerGatewayDispatchReceipt = S.Struct({
  backendKind: OpenAgentsRunnerBackendKind,
  externalRunRef: S.String,
  receiptRefs: S.Array(S.String),
  status: OpenAgentsRunnerDispatchStatus,
})
export type OpenAgentsRunnerGatewayDispatchReceipt =
  typeof OpenAgentsRunnerGatewayDispatchReceipt.Type

export type OpenAgentsRunnerGatewayAdapter = Readonly<{
  backendKind: OpenAgentsRunnerBackendKind
  cancel: (
    request: OpenAgentsRunnerGatewayCancelRequest,
  ) => Promise<OpenAgentsRunnerGatewayDispatchReceipt>
  checkHealth: (
    request: OpenAgentsRunnerGatewayHealthCheckRequest,
  ) => Promise<OpenAgentsRunnerGatewayHealthStatus>
  dispatch: (
    request: OpenAgentsRunnerGatewayDispatchRequest,
  ) => Promise<OpenAgentsRunnerGatewayDispatchReceipt>
  ingestLifecycleCallback: (
    callback: OpenAgentsRunnerGatewayLifecycleCallback,
  ) => Promise<OpenAgentsRunnerGatewayDispatchReceipt>
}>

export class OpenAgentsRunnerGatewayBackendNotSelected extends S.TaggedErrorClass<OpenAgentsRunnerGatewayBackendNotSelected>()(
  'OpenAgentsRunnerGatewayBackendNotSelected',
  {
    backendKind: OpenAgentsRunnerBackendKind,
    message: S.String,
    operation: OpenAgentsRunnerGatewayOperation,
  },
) {}

export class OpenAgentsRunnerGatewayDisabledBackend extends S.TaggedErrorClass<OpenAgentsRunnerGatewayDisabledBackend>()(
  'OpenAgentsRunnerGatewayDisabledBackend',
  {
    backendKind: OpenAgentsRunnerBackendKind,
    message: S.String,
    operation: OpenAgentsRunnerGatewayOperation,
  },
) {}

export class OpenAgentsRunnerGatewayUnsafeCredentialMaterial extends S.TaggedErrorClass<OpenAgentsRunnerGatewayUnsafeCredentialMaterial>()(
  'OpenAgentsRunnerGatewayUnsafeCredentialMaterial',
  {
    message: S.String,
    operation: OpenAgentsRunnerGatewayOperation,
  },
) {}

export class OpenAgentsRunnerGatewayUnsupportedBackend extends S.TaggedErrorClass<OpenAgentsRunnerGatewayUnsupportedBackend>()(
  'OpenAgentsRunnerGatewayUnsupportedBackend',
  {
    backendKind: OpenAgentsRunnerBackendKind,
    message: S.String,
    operation: OpenAgentsRunnerGatewayOperation,
  },
) {}

export class OpenAgentsRunnerGatewayMalformedRequest extends S.TaggedErrorClass<OpenAgentsRunnerGatewayMalformedRequest>()(
  'OpenAgentsRunnerGatewayMalformedRequest',
  {
    message: S.String,
    operation: OpenAgentsRunnerGatewayOperation,
  },
) {}

export const OpenAgentsRunnerGatewayError = S.Union([
  OpenAgentsRunnerGatewayBackendNotSelected,
  OpenAgentsRunnerGatewayDisabledBackend,
  OpenAgentsRunnerGatewayMalformedRequest,
  OpenAgentsRunnerGatewayUnsafeCredentialMaterial,
  OpenAgentsRunnerGatewayUnsupportedBackend,
])
export type OpenAgentsRunnerGatewayError =
  typeof OpenAgentsRunnerGatewayError.Type

export type OpenAgentsRunnerGatewaySelectionResult =
  | Readonly<{
      _tag: 'OpenAgentsRunnerGatewayDenied'
      error: OpenAgentsRunnerGatewayError
    }>
  | Readonly<{
      _tag: 'OpenAgentsRunnerGatewaySelected'
      selection: OpenAgentsRunnerGatewayAdapterSelection
    }>

const runnerGatewayErrorTags = new Set([
  'OpenAgentsRunnerGatewayBackendNotSelected',
  'OpenAgentsRunnerGatewayDisabledBackend',
  'OpenAgentsRunnerGatewayMalformedRequest',
  'OpenAgentsRunnerGatewayUnsafeCredentialMaterial',
  'OpenAgentsRunnerGatewayUnsupportedBackend',
])

const unsafeGatewayMaterialPattern =
  /(api[_-]?key(?![_-]?ref)|auth[_-]?content[_-]?json|bearer\s+|callback[_-]?token[_-]?(raw|secret|value)|cookie|customer[_-]?email|customer[_-]?name|email[_-]?body|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|github[_-]?token(?![_-]?ref)|mnemonic|oauth[_-]?(access|refresh)?[_-]?token|openagents_admin|password|preimage|private[_-]?key|provider[_-]?payload|provider[_-]?token|raw[_-]?email|raw[_-]?runner|raw[_-]?run[_-]?log|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|wallet[_-]?secret|\S+@\S+)/i

const unknownMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

export const isOpenAgentsRunnerGatewayError = (
  error: unknown,
): error is OpenAgentsRunnerGatewayError =>
  typeof error === 'object' &&
  error !== null &&
  '_tag' in error &&
  typeof error._tag === 'string' &&
  runnerGatewayErrorTags.has(error._tag)

export const openAgentsRunnerGatewayErrorFromUnknown = (
  operation: OpenAgentsRunnerGatewayOperation,
  error: unknown,
): OpenAgentsRunnerGatewayError =>
  isOpenAgentsRunnerGatewayError(error)
    ? error
    : new OpenAgentsRunnerGatewayMalformedRequest({
        message: unknownMessage(error),
        operation,
      })

const gatewayErrorStatuses: Record<
  OpenAgentsRunnerGatewayError['_tag'],
  number
> = {
  OpenAgentsRunnerGatewayBackendNotSelected: 409,
  OpenAgentsRunnerGatewayDisabledBackend: 409,
  OpenAgentsRunnerGatewayMalformedRequest: 400,
  OpenAgentsRunnerGatewayUnsafeCredentialMaterial: 400,
  OpenAgentsRunnerGatewayUnsupportedBackend: 422,
}

export const openAgentsRunnerGatewayErrorStatus = (
  error: OpenAgentsRunnerGatewayError,
): number => gatewayErrorStatuses[error._tag]

export const openAgentsRunnerGatewayPayloadHasPrivateMaterial = (
  payload: unknown,
): boolean => unsafeGatewayMaterialPattern.test(JSON.stringify(payload))

export const validateOpenAgentsRunnerGatewayPayload = <Payload>(
  operation: OpenAgentsRunnerGatewayOperation,
  payload: Payload,
): Payload | OpenAgentsRunnerGatewayUnsafeCredentialMaterial =>
  openAgentsRunnerGatewayPayloadHasPrivateMaterial(payload)
    ? new OpenAgentsRunnerGatewayUnsafeCredentialMaterial({
        message:
          'Runner gateway payloads must carry refs and grants, not raw credentials, logs, source archives, wallet material, or customer private data.',
        operation,
      })
    : payload

export const selectOpenAgentsRunnerGatewayAdapter = (
  input: Readonly<{
    backendKind: OpenAgentsRunnerBackendKind
    operation: OpenAgentsRunnerGatewayOperation
    states: ReadonlyArray<OpenAgentsRunnerGatewayAdapterState>
  }>,
): OpenAgentsRunnerGatewaySelectionResult => {
  const state = input.states.find(
    adapterState => adapterState.backendKind === input.backendKind,
  )

  if (state === undefined) {
    return {
      _tag: 'OpenAgentsRunnerGatewayDenied',
      error: new OpenAgentsRunnerGatewayUnsupportedBackend({
        backendKind: input.backendKind,
        message: 'No runner gateway adapter is registered for this backend.',
        operation: input.operation,
      }),
    }
  }

  if (!state.policySelected) {
    return {
      _tag: 'OpenAgentsRunnerGatewayDenied',
      error: new OpenAgentsRunnerGatewayBackendNotSelected({
        backendKind: state.backendKind,
        message:
          'Runner backend execution requires explicit policy selection.',
        operation: input.operation,
      }),
    }
  }

  if (!state.enabled || !state.configured) {
    return {
      _tag: 'OpenAgentsRunnerGatewayDenied',
      error: new OpenAgentsRunnerGatewayDisabledBackend({
        backendKind: state.backendKind,
        message:
          'Runner backend is not enabled and configured for execution.',
        operation: input.operation,
      }),
    }
  }

  return {
    _tag: 'OpenAgentsRunnerGatewaySelected',
    selection: {
      backendKind: state.backendKind,
      capacityRef: state.capacityRef,
      healthStatus: state.healthStatus,
      reasonRefs: state.reasonRefs,
    },
  }
}
