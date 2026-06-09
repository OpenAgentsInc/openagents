import { Schema as S } from 'effect'

import {
  OpenAgentsRunnerDispatchStatus,
  OpenAgentsRunnerWorkloadTrust,
} from './runner-backends'
import {
  type OpenAgentsRunnerGatewayAdapter,
  type OpenAgentsRunnerGatewayCancelRequest,
  type OpenAgentsRunnerGatewayDispatchReceipt,
  type OpenAgentsRunnerGatewayDispatchRequest,
  type OpenAgentsRunnerGatewayHealthCheckRequest,
  type OpenAgentsRunnerGatewayHealthStatus,
  type OpenAgentsRunnerGatewayLifecycleCallback,
  type OpenAgentsRunnerGatewayOperation,
  OpenAgentsRunnerGatewayMalformedRequest,
  OpenAgentsRunnerGatewayUnsafeCredentialMaterial,
  isOpenAgentsRunnerGatewayError,
  openAgentsRunnerGatewayPayloadHasPrivateMaterial,
  validateOpenAgentsRunnerGatewayPayload,
} from './runner-gateway'

export const OpenAgentsRealCloudflareContainerRunnerGate = S.Literals([
  'binding_configured',
  'control_plane_bound',
  'enabled',
  'policy_approved',
  'policy_selected',
  'staging_smoke_passed',
  'workload_trust_allowed',
])
export type OpenAgentsRealCloudflareContainerRunnerGate =
  typeof OpenAgentsRealCloudflareContainerRunnerGate.Type

export const OpenAgentsRealCloudflareContainerRunnerBindingRefs = S.Struct({
  classNameRef: S.optionalKey(S.String),
  durableObjectBindingRef: S.optionalKey(S.String),
  imageRef: S.optionalKey(S.String),
})
export type OpenAgentsRealCloudflareContainerRunnerBindingRefs =
  typeof OpenAgentsRealCloudflareContainerRunnerBindingRefs.Type

export const OpenAgentsRealCloudflareContainerRunnerReadiness = S.Struct({
  allowedWorkloadTrusts: S.Array(OpenAgentsRunnerWorkloadTrust),
  backendKind: S.Literal('cloudflare_container'),
  bindingRefs: OpenAgentsRealCloudflareContainerRunnerBindingRefs,
  capacityRef: S.String,
  configured: S.Boolean,
  enabled: S.Boolean,
  healthStatus: S.Literals(['blocked', 'degraded', 'healthy', 'unknown']),
  policyApproved: S.Boolean,
  policySelected: S.Boolean,
  publicSummaryRef: S.String,
  runnerId: S.String,
  stagingSmokePassed: S.Boolean,
})
export type OpenAgentsRealCloudflareContainerRunnerReadiness =
  typeof OpenAgentsRealCloudflareContainerRunnerReadiness.Type

export const OpenAgentsRealCloudflareContainerRunnerControlReceipt = S.Struct({
  backendKind: S.Literal('cloudflare_container'),
  externalRunRef: S.String,
  operatorDiagnosticRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  receiptRefs: S.Array(S.String),
  status: OpenAgentsRunnerDispatchStatus,
})
export type OpenAgentsRealCloudflareContainerRunnerControlReceipt =
  typeof OpenAgentsRealCloudflareContainerRunnerControlReceipt.Type

export type OpenAgentsRealCloudflareContainerRunnerControlPlane = Readonly<{
  cancel: (
    request: OpenAgentsRunnerGatewayCancelRequest,
  ) => Promise<OpenAgentsRealCloudflareContainerRunnerControlReceipt>
  checkHealth: (
    request: OpenAgentsRunnerGatewayHealthCheckRequest,
  ) => Promise<OpenAgentsRunnerGatewayHealthStatus>
  dispatch: (
    request: OpenAgentsRunnerGatewayDispatchRequest,
  ) => Promise<OpenAgentsRealCloudflareContainerRunnerControlReceipt>
  ingestLifecycleCallback: (
    callback: OpenAgentsRunnerGatewayLifecycleCallback,
  ) => Promise<OpenAgentsRealCloudflareContainerRunnerControlReceipt>
}>

const safeRefPart = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9_.-]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 96) || 'request'

const bindingConfigured = (
  readiness: OpenAgentsRealCloudflareContainerRunnerReadiness,
): boolean =>
  readiness.configured &&
  readiness.bindingRefs.classNameRef !== undefined &&
  readiness.bindingRefs.durableObjectBindingRef !== undefined &&
  readiness.bindingRefs.imageRef !== undefined

const gateRef = (
  gate: OpenAgentsRealCloudflareContainerRunnerGate,
): string => `gate.cloudflare_container.${gate}.blocked`

export const openAgentsRealCloudflareContainerRunnerBlockedGateRefs = (
  input: Readonly<{
    controlPlaneBound: boolean
    readiness: OpenAgentsRealCloudflareContainerRunnerReadiness
    workloadTrust?: typeof OpenAgentsRunnerWorkloadTrust.Type | undefined
  }>,
): ReadonlyArray<string> => {
  const refs: string[] = []

  if (!input.readiness.policySelected) {
    refs.push(gateRef('policy_selected'))
  }
  if (!input.readiness.enabled) {
    refs.push(gateRef('enabled'))
  }
  if (!bindingConfigured(input.readiness)) {
    refs.push(gateRef('binding_configured'))
  }
  if (!input.readiness.stagingSmokePassed) {
    refs.push(gateRef('staging_smoke_passed'))
  }
  if (!input.readiness.policyApproved) {
    refs.push(gateRef('policy_approved'))
  }
  if (!input.controlPlaneBound) {
    refs.push(gateRef('control_plane_bound'))
  }
  if (
    input.workloadTrust !== undefined &&
    !input.readiness.allowedWorkloadTrusts.includes(input.workloadTrust)
  ) {
    refs.push(gateRef('workload_trust_allowed'))
  }

  return [...new Set(refs)]
}

const ensureSafePayload = <Payload>(
  operation: OpenAgentsRunnerGatewayOperation,
  payload: Payload,
): Payload => {
  const result = validateOpenAgentsRunnerGatewayPayload(operation, payload)

  if (isOpenAgentsRunnerGatewayError(result)) {
    throw result
  }

  return result
}

const blockedControlReceipt = (
  input: Readonly<{
    gateRefs: ReadonlyArray<string>
    operation: OpenAgentsRunnerGatewayOperation
    publicSummaryRef: string
    requestId: string
  }>,
): OpenAgentsRealCloudflareContainerRunnerControlReceipt => {
  const requestRef = safeRefPart(input.requestId)

  return {
    backendKind: 'cloudflare_container',
    externalRunRef: `cloudflare_container.blocked.${requestRef}`,
    operatorDiagnosticRefs: input.gateRefs.map(
      ref => `diagnostic.${ref}`,
    ),
    publicSummaryRef: input.publicSummaryRef,
    receiptRefs: [
      `receipt.cloudflare_container.${input.operation}.${requestRef}.blocked`,
      ...input.gateRefs,
    ],
    status: 'blocked',
  }
}

const dispatchReceipt = (
  receipt: OpenAgentsRealCloudflareContainerRunnerControlReceipt,
): OpenAgentsRunnerGatewayDispatchReceipt => ({
  backendKind: 'cloudflare_container',
  externalRunRef: receipt.externalRunRef,
  receiptRefs: receipt.receiptRefs,
  status: receipt.status,
})

const ensureSafeControlReceipt = (
  operation: OpenAgentsRunnerGatewayOperation,
  receipt: unknown,
): OpenAgentsRealCloudflareContainerRunnerControlReceipt => {
  let decoded: OpenAgentsRealCloudflareContainerRunnerControlReceipt

  try {
    decoded = S.decodeUnknownSync(
      OpenAgentsRealCloudflareContainerRunnerControlReceipt,
    )(receipt)
  } catch (error) {
    throw new OpenAgentsRunnerGatewayMalformedRequest({
      message:
        error instanceof Error
          ? error.message
          : 'Container runner control receipt was malformed.',
      operation,
    })
  }

  if (openAgentsRunnerGatewayPayloadHasPrivateMaterial(decoded)) {
    throw new OpenAgentsRunnerGatewayUnsafeCredentialMaterial({
      message:
        'Container runner control receipts must carry refs, not raw logs, credentials, source archives, wallet material, callback secrets, or customer private data.',
      operation,
    })
  }

  return decoded
}

const requestIdForOperation = (
  _operation: OpenAgentsRunnerGatewayOperation,
  payload:
    | OpenAgentsRunnerGatewayCancelRequest
    | OpenAgentsRunnerGatewayDispatchRequest
    | OpenAgentsRunnerGatewayHealthCheckRequest
    | OpenAgentsRunnerGatewayLifecycleCallback,
): string =>
  'requestId' in payload ? payload.requestId : payload.runRef

const blockedOrControlReceipt = async <
  Payload extends
    | OpenAgentsRunnerGatewayCancelRequest
    | OpenAgentsRunnerGatewayDispatchRequest
    | OpenAgentsRunnerGatewayHealthCheckRequest
    | OpenAgentsRunnerGatewayLifecycleCallback,
>(
  input: Readonly<{
    controlPlane: OpenAgentsRealCloudflareContainerRunnerControlPlane | undefined
    operation: OpenAgentsRunnerGatewayOperation
    payload: Payload
    readiness: OpenAgentsRealCloudflareContainerRunnerReadiness
    run: (
      controlPlane: OpenAgentsRealCloudflareContainerRunnerControlPlane,
      payload: Payload,
    ) => Promise<OpenAgentsRealCloudflareContainerRunnerControlReceipt>
    workloadTrust?: typeof OpenAgentsRunnerWorkloadTrust.Type | undefined
  }>,
): Promise<OpenAgentsRealCloudflareContainerRunnerControlReceipt> => {
  const safePayload = ensureSafePayload(input.operation, input.payload)
  const gateRefs = openAgentsRealCloudflareContainerRunnerBlockedGateRefs({
    controlPlaneBound: input.controlPlane !== undefined,
    readiness: input.readiness,
    workloadTrust: input.workloadTrust,
  })

  if (gateRefs.length > 0) {
    return blockedControlReceipt({
      gateRefs,
      operation: input.operation,
      publicSummaryRef: input.readiness.publicSummaryRef,
      requestId: requestIdForOperation(input.operation, safePayload),
    })
  }

  if (input.controlPlane === undefined) {
    return blockedControlReceipt({
      gateRefs: [gateRef('control_plane_bound')],
      operation: input.operation,
      publicSummaryRef: input.readiness.publicSummaryRef,
      requestId: requestIdForOperation(input.operation, safePayload),
    })
  }

  return ensureSafeControlReceipt(
    input.operation,
    await input.run(input.controlPlane, safePayload),
  )
}

export const makeRealCloudflareContainerRunnerAdapter = (
  input: Readonly<{
    controlPlane?: OpenAgentsRealCloudflareContainerRunnerControlPlane
    readiness: OpenAgentsRealCloudflareContainerRunnerReadiness
  }>,
): OpenAgentsRunnerGatewayAdapter => ({
  backendKind: 'cloudflare_container',
  cancel: async request =>
    dispatchReceipt(
      await blockedOrControlReceipt({
        controlPlane: input.controlPlane,
        operation: 'cancel',
        payload: request,
        readiness: input.readiness,
        run: (controlPlane, safeRequest) => controlPlane.cancel(safeRequest),
      }),
    ),
  checkHealth: async request => {
    const safeRequest = ensureSafePayload('health_check', request)
    const gateRefs = openAgentsRealCloudflareContainerRunnerBlockedGateRefs({
      controlPlaneBound: input.controlPlane !== undefined,
      readiness: input.readiness,
    })

    if (gateRefs.length > 0) {
      return 'blocked'
    }

    return input.controlPlane?.checkHealth(safeRequest) ?? 'blocked'
  },
  dispatch: async request =>
    dispatchReceipt(
      await blockedOrControlReceipt({
        controlPlane: input.controlPlane,
        operation: 'dispatch',
        payload: request,
        readiness: input.readiness,
        run: (controlPlane, safeRequest) => controlPlane.dispatch(safeRequest),
        workloadTrust: request.trustLevel,
      }),
    ),
  ingestLifecycleCallback: async callback =>
    dispatchReceipt(
      await blockedOrControlReceipt({
        controlPlane: input.controlPlane,
        operation: 'lifecycle_callback',
        payload: callback,
        readiness: input.readiness,
        run: (controlPlane, safeCallback) =>
          controlPlane.ingestLifecycleCallback(safeCallback),
      }),
    ),
})
