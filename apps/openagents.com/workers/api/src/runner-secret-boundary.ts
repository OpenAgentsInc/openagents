import { Schema as S } from 'effect'

import {
  OpenAgentsRunnerBackendKind,
  type OpenAgentsRunnerBackendKind as OpenAgentsRunnerBackendKindValue,
} from './runner-backends'
import {
  OpenAgentsRunnerGatewayOperation,
  openAgentsRunnerGatewayPayloadHasPrivateMaterial,
} from './runner-gateway'

export const OpenAgentsRunnerGrantKind = S.Literals([
  'callback',
  'github_write',
  'provider_account',
])
export type OpenAgentsRunnerGrantKind =
  typeof OpenAgentsRunnerGrantKind.Type

export const OpenAgentsRunnerGrantDenialReason = S.Literals([
  'callback_token_material',
  'github_token_material',
  'missing_required_grant_ref',
  'oauth_material',
  'provider_token_material',
  'raw_credential_material',
  'wallet_or_payment_material',
])
export type OpenAgentsRunnerGrantDenialReason =
  typeof OpenAgentsRunnerGrantDenialReason.Type

export const OpenAgentsRunnerGrantRef = S.Struct({
  grantKind: OpenAgentsRunnerGrantKind,
  grantRef: S.String,
  providerAccountRef: S.optionalKey(S.String),
  required: S.Boolean,
  runnerSessionRef: S.String,
})
export type OpenAgentsRunnerGrantRef = typeof OpenAgentsRunnerGrantRef.Type

export const OpenAgentsRunnerGrantResolutionReceipt = S.Struct({
  grantKind: OpenAgentsRunnerGrantKind,
  grantRef: S.String,
  materializationRef: S.String,
  receiptRef: S.String,
  resolvedAtRef: S.String,
  runnerSessionRef: S.String,
  scrubRequired: S.Boolean,
  status: S.Literals(['denied', 'resolved']),
})
export type OpenAgentsRunnerGrantResolutionReceipt =
  typeof OpenAgentsRunnerGrantResolutionReceipt.Type

export const OpenAgentsRunnerGrantScrubReceipt = S.Struct({
  grantKind: OpenAgentsRunnerGrantKind,
  grantRef: S.String,
  receiptRef: S.String,
  runnerSessionRef: S.String,
  scrubbedAtRef: S.String,
  status: S.Literals(['not_materialized', 'scrubbed']),
})
export type OpenAgentsRunnerGrantScrubReceipt =
  typeof OpenAgentsRunnerGrantScrubReceipt.Type

export const OpenAgentsRunnerDispatchSecretBoundary = S.Struct({
  backendKind: OpenAgentsRunnerBackendKind,
  denialReasons: S.Array(OpenAgentsRunnerGrantDenialReason),
  dispatchRef: S.String,
  grantRefs: S.Array(OpenAgentsRunnerGrantRef),
  publicSummaryRef: S.String,
  resolutionReceipts: S.Array(OpenAgentsRunnerGrantResolutionReceipt),
  runnerSessionRef: S.String,
  scrubReceipts: S.Array(OpenAgentsRunnerGrantScrubReceipt),
  status: S.Literals(['denied', 'ready']),
})
export type OpenAgentsRunnerDispatchSecretBoundary =
  typeof OpenAgentsRunnerDispatchSecretBoundary.Type

export const OpenAgentsRunnerDispatchSecretBoundaryPublicProjection = S.Struct({
  backendKind: OpenAgentsRunnerBackendKind,
  denialReasons: S.Array(OpenAgentsRunnerGrantDenialReason),
  dispatchRef: S.String,
  grantCount: S.Number,
  hasRequiredGrants: S.Boolean,
  publicSummaryRef: S.String,
  resolutionReceiptRefs: S.Array(S.String),
  runnerSessionRef: S.String,
  scrubReceiptRefs: S.Array(S.String),
  status: S.Literals(['denied', 'ready']),
})
export type OpenAgentsRunnerDispatchSecretBoundaryPublicProjection =
  typeof OpenAgentsRunnerDispatchSecretBoundaryPublicProjection.Type

export class OpenAgentsRunnerDispatchSecretBoundaryDenied extends S.TaggedErrorClass<OpenAgentsRunnerDispatchSecretBoundaryDenied>()(
  'OpenAgentsRunnerDispatchSecretBoundaryDenied',
  {
    denialReason: OpenAgentsRunnerGrantDenialReason,
    message: S.String,
    operation: OpenAgentsRunnerGatewayOperation,
  },
) {}

const refPart = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9_.-]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 96) || 'ref'

const maybeGrantRef = (
  input: Readonly<{
    grantKind: OpenAgentsRunnerGrantKind
    grantRef?: string | undefined
    providerAccountRef?: string | undefined
    required: boolean
    runnerSessionRef: string
  }>,
): OpenAgentsRunnerGrantRef | undefined =>
  input.grantRef === undefined
    ? undefined
    : {
        grantKind: input.grantKind,
        grantRef: input.grantRef,
        ...(input.providerAccountRef === undefined
          ? {}
          : { providerAccountRef: input.providerAccountRef }),
        required: input.required,
        runnerSessionRef: input.runnerSessionRef,
      }

export const openAgentsRunnerGrantRefsForDispatch = (
  input: Readonly<{
    authGrantRef?: string | undefined
    callbackRef?: string | undefined
    githubWriteGrantRef?: string | undefined
    providerAccountRef?: string | undefined
    runnerSessionRef: string
  }>,
): ReadonlyArray<OpenAgentsRunnerGrantRef> =>
  [
    maybeGrantRef({
      grantKind: 'provider_account',
      grantRef: input.authGrantRef,
      providerAccountRef: input.providerAccountRef,
      required: true,
      runnerSessionRef: input.runnerSessionRef,
    }),
    maybeGrantRef({
      grantKind: 'github_write',
      grantRef: input.githubWriteGrantRef,
      required: false,
      runnerSessionRef: input.runnerSessionRef,
    }),
    maybeGrantRef({
      grantKind: 'callback',
      grantRef: input.callbackRef,
      required: true,
      runnerSessionRef: input.runnerSessionRef,
    }),
  ].filter((grant): grant is OpenAgentsRunnerGrantRef => grant !== undefined)

export const openAgentsRunnerGrantResolutionReceipts = (
  grants: ReadonlyArray<OpenAgentsRunnerGrantRef>,
): ReadonlyArray<OpenAgentsRunnerGrantResolutionReceipt> =>
  grants.map(grant => ({
    grantKind: grant.grantKind,
    grantRef: grant.grantRef,
    materializationRef: `materialization.${grant.grantKind}.${refPart(grant.grantRef)}.runner_boundary`,
    receiptRef: `receipt.${grant.grantKind}.${refPart(grant.grantRef)}.resolved`,
    resolvedAtRef: 'time.runner_boundary.resolved',
    runnerSessionRef: grant.runnerSessionRef,
    scrubRequired: true,
    status: 'resolved',
  }))

export const openAgentsRunnerGrantScrubReceipts = (
  grants: ReadonlyArray<OpenAgentsRunnerGrantRef>,
): ReadonlyArray<OpenAgentsRunnerGrantScrubReceipt> =>
  grants.map(grant => ({
    grantKind: grant.grantKind,
    grantRef: grant.grantRef,
    receiptRef: `receipt.${grant.grantKind}.${refPart(grant.grantRef)}.scrubbed`,
    runnerSessionRef: grant.runnerSessionRef,
    scrubbedAtRef: 'time.runner_boundary.scrubbed',
    status: 'scrubbed',
  }))

const missingRequiredGrantReasons = (
  grants: ReadonlyArray<OpenAgentsRunnerGrantRef>,
): ReadonlyArray<OpenAgentsRunnerGrantDenialReason> => {
  const hasProviderGrant = grants.some(
    grant => grant.grantKind === 'provider_account',
  )
  const hasCallbackGrant = grants.some(grant => grant.grantKind === 'callback')

  return hasProviderGrant && hasCallbackGrant
    ? []
    : ['missing_required_grant_ref']
}

export const buildOpenAgentsRunnerDispatchSecretBoundary = (
  input: Readonly<{
    authGrantRef?: string | undefined
    backendKind: OpenAgentsRunnerBackendKindValue
    callbackRef?: string | undefined
    dispatchPayload: unknown
    dispatchRef: string
    githubWriteGrantRef?: string | undefined
    providerAccountRef?: string | undefined
    runnerSessionRef: string
  }>,
):
  | OpenAgentsRunnerDispatchSecretBoundary
  | OpenAgentsRunnerDispatchSecretBoundaryDenied => {
  if (openAgentsRunnerGatewayPayloadHasPrivateMaterial(input.dispatchPayload)) {
    return new OpenAgentsRunnerDispatchSecretBoundaryDenied({
      denialReason: 'raw_credential_material',
      message:
        'Runner dispatch payloads must contain grant refs only; credential material resolves inside the runner/service boundary.',
      operation: 'dispatch',
    })
  }

  const grantRefs = openAgentsRunnerGrantRefsForDispatch({
    authGrantRef: input.authGrantRef,
    callbackRef: input.callbackRef,
    githubWriteGrantRef: input.githubWriteGrantRef,
    providerAccountRef: input.providerAccountRef,
    runnerSessionRef: input.runnerSessionRef,
  })
  const denialReasons = missingRequiredGrantReasons(grantRefs)

  return {
    backendKind: input.backendKind,
    denialReasons,
    dispatchRef: input.dispatchRef,
    grantRefs,
    publicSummaryRef: `summary.runner_secret_boundary.${refPart(input.dispatchRef)}`,
    resolutionReceipts:
      denialReasons.length === 0
        ? openAgentsRunnerGrantResolutionReceipts(grantRefs)
        : [],
    runnerSessionRef: input.runnerSessionRef,
    scrubReceipts:
      denialReasons.length === 0
        ? openAgentsRunnerGrantScrubReceipts(grantRefs)
        : [],
    status: denialReasons.length === 0 ? 'ready' : 'denied',
  }
}

export const projectOpenAgentsRunnerDispatchSecretBoundaryPublic = (
  boundary: OpenAgentsRunnerDispatchSecretBoundary,
): OpenAgentsRunnerDispatchSecretBoundaryPublicProjection => ({
  backendKind: boundary.backendKind,
  denialReasons: boundary.denialReasons,
  dispatchRef: boundary.dispatchRef,
  grantCount: boundary.grantRefs.length,
  hasRequiredGrants: boundary.status === 'ready',
  publicSummaryRef: boundary.publicSummaryRef,
  resolutionReceiptRefs: boundary.resolutionReceipts.map(
    (_receipt, index) =>
      `receipt.runner_secret_boundary.resolution.${index + 1}`,
  ),
  runnerSessionRef: boundary.runnerSessionRef,
  scrubReceiptRefs: boundary.scrubReceipts.map(
    (_receipt, index) => `receipt.runner_secret_boundary.scrub.${index + 1}`,
  ),
  status: boundary.status,
})
