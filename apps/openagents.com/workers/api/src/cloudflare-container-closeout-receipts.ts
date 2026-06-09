import { Schema as S } from 'effect'

import {
  type OpenAgentsRunnerGatewayArtifactManifest,
  type OpenAgentsRunnerGatewayLifecycleCallback,
  openAgentsRunnerGatewayPayloadHasPrivateMaterial,
} from './runner-gateway'
import {
  openAgentsCloudflareContainerRunnerManifestHasPrivateMaterial,
} from './cloudflare-container-runner-manifest'

export const OpenAgentsCloudflareContainerCloseoutAudience = S.Literals([
  'customer',
  'operator',
  'public',
  'team',
])
export type OpenAgentsCloudflareContainerCloseoutAudience =
  typeof OpenAgentsCloudflareContainerCloseoutAudience.Type

export const OpenAgentsCloudflareContainerLifecyclePhase = S.Literals([
  'accepted',
  'artifact',
  'cancelled',
  'completed',
  'failed',
  'progress',
  'started',
  'timed_out',
])
export type OpenAgentsCloudflareContainerLifecyclePhase =
  typeof OpenAgentsCloudflareContainerLifecyclePhase.Type

export const OpenAgentsCloudflareContainerArtifactCloseout = S.Struct({
  buildLogRefs: S.Array(S.String),
  diffRefs: S.Array(S.String),
  generatedFileRefs: S.Array(S.String),
  publicArtifactRefs: S.Array(S.String),
  redactionReportRefs: S.Array(S.String),
  screenshotRefs: S.Array(S.String),
  validationResultRefs: S.Array(S.String),
})
export type OpenAgentsCloudflareContainerArtifactCloseout =
  typeof OpenAgentsCloudflareContainerArtifactCloseout.Type

export const OpenAgentsCloudflareContainerCloseoutReceipt = S.Struct({
  artifactCloseout: OpenAgentsCloudflareContainerArtifactCloseout,
  artifactManifestRef: S.optionalKey(S.String),
  backendKind: S.Literal('cloudflare_container'),
  callbackRef: S.String,
  closeoutReceiptRefs: S.Array(S.String),
  credentialScrubReceiptRefs: S.Array(S.String),
  eventRefs: S.Array(S.String),
  externalRunRef: S.optionalKey(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  phase: OpenAgentsCloudflareContainerLifecyclePhase,
  providerAccountScrubReceiptRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  runRef: S.String,
  runnerId: S.String,
  statusCaveatRefs: S.Array(S.String),
})
export type OpenAgentsCloudflareContainerCloseoutReceipt =
  typeof OpenAgentsCloudflareContainerCloseoutReceipt.Type

export const OpenAgentsCloudflareContainerCloseoutProjection = S.Struct({
  artifactManifestRef: S.optionalKey(S.String),
  audience: OpenAgentsCloudflareContainerCloseoutAudience,
  backendKind: S.Literal('cloudflare_container'),
  buildLogRefs: S.Array(S.String),
  callbackRef: S.optionalKey(S.String),
  closeoutReceiptRefs: S.Array(S.String),
  diffRefs: S.Array(S.String),
  eventRefs: S.Array(S.String),
  externalRunRef: S.optionalKey(S.String),
  generatedFileRefs: S.Array(S.String),
  operatorDiagnosticRefs: S.Array(S.String),
  phase: OpenAgentsCloudflareContainerLifecyclePhase,
  publicArtifactRefs: S.Array(S.String),
  publicSummaryRef: S.String,
  redactionReportRefs: S.Array(S.String),
  runRef: S.String,
  screenshotRefs: S.Array(S.String),
  scrubReceiptRefs: S.Array(S.String),
  statusCaveatRefs: S.Array(S.String),
  validationResultRefs: S.Array(S.String),
})
export type OpenAgentsCloudflareContainerCloseoutProjection =
  typeof OpenAgentsCloudflareContainerCloseoutProjection.Type

export class OpenAgentsCloudflareContainerCloseoutUnsafeMaterial extends S.TaggedErrorClass<OpenAgentsCloudflareContainerCloseoutUnsafeMaterial>()(
  'OpenAgentsCloudflareContainerCloseoutUnsafeMaterial',
  {
    message: S.String,
    runRef: S.String,
  },
) {}

export class OpenAgentsCloudflareContainerCloseoutMissingScrubReceipt extends S.TaggedErrorClass<OpenAgentsCloudflareContainerCloseoutMissingScrubReceipt>()(
  'OpenAgentsCloudflareContainerCloseoutMissingScrubReceipt',
  {
    message: S.String,
    phase: OpenAgentsCloudflareContainerLifecyclePhase,
    runRef: S.String,
  },
) {}

const terminalPhases = new Set<OpenAgentsCloudflareContainerLifecyclePhase>([
  'cancelled',
  'completed',
  'failed',
  'timed_out',
])

const phaseToGatewayStatus: Record<
  OpenAgentsCloudflareContainerLifecyclePhase,
  OpenAgentsRunnerGatewayLifecycleCallback['dispatchStatus']
> = {
  accepted: 'queued',
  artifact: 'artifact_ready',
  cancelled: 'cancelled',
  completed: 'completed',
  failed: 'failed',
  progress: 'running',
  started: 'running',
  timed_out: 'failed',
}

const safeRef = (ref: string): boolean =>
  ref.trim() !== '' &&
  !openAgentsRunnerGatewayPayloadHasPrivateMaterial(ref) &&
  !openAgentsCloudflareContainerRunnerManifestHasPrivateMaterial(ref)

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].filter(safeRef)

const hasUnsafeCloseoutMaterial = (
  receipt: OpenAgentsCloudflareContainerCloseoutReceipt,
): boolean =>
  openAgentsRunnerGatewayPayloadHasPrivateMaterial(receipt) ||
  openAgentsCloudflareContainerRunnerManifestHasPrivateMaterial(receipt)

const terminalCloseoutHasScrubEvidence = (
  receipt: OpenAgentsCloudflareContainerCloseoutReceipt,
): boolean =>
  !terminalPhases.has(receipt.phase) ||
  (receipt.credentialScrubReceiptRefs.length > 0 &&
    receipt.providerAccountScrubReceiptRefs.length > 0)

export const validateOpenAgentsCloudflareContainerCloseoutReceipt = (
  receipt: OpenAgentsCloudflareContainerCloseoutReceipt,
):
  | OpenAgentsCloudflareContainerCloseoutReceipt
  | OpenAgentsCloudflareContainerCloseoutMissingScrubReceipt
  | OpenAgentsCloudflareContainerCloseoutUnsafeMaterial => {
  if (hasUnsafeCloseoutMaterial(receipt)) {
    return new OpenAgentsCloudflareContainerCloseoutUnsafeMaterial({
      message:
        'Cloudflare Container closeout receipts must carry refs only, not raw logs, source archives, auth material, callback-token values, wallet/payment secrets, or customer private data.',
      runRef: receipt.runRef,
    })
  }

  if (!terminalCloseoutHasScrubEvidence(receipt)) {
    return new OpenAgentsCloudflareContainerCloseoutMissingScrubReceipt({
      message:
        'Terminal Container closeout requires credential and provider-account scrub receipt refs before it can be accepted.',
      phase: receipt.phase,
      runRef: receipt.runRef,
    })
  }

  return receipt
}

const artifactRefsForAudience = (
  receipt: OpenAgentsCloudflareContainerCloseoutReceipt,
  audience: OpenAgentsCloudflareContainerCloseoutAudience,
): OpenAgentsCloudflareContainerArtifactCloseout => {
  const artifact = receipt.artifactCloseout
  const customerOrMore = audience !== 'public'
  const teamOrOperator = audience === 'team' || audience === 'operator'

  return {
    buildLogRefs: teamOrOperator ? safeRefs(artifact.buildLogRefs) : [],
    diffRefs: customerOrMore ? safeRefs(artifact.diffRefs) : [],
    generatedFileRefs: customerOrMore
      ? safeRefs(artifact.generatedFileRefs)
      : [],
    publicArtifactRefs: safeRefs(artifact.publicArtifactRefs),
    redactionReportRefs: customerOrMore
      ? safeRefs(artifact.redactionReportRefs)
      : [],
    screenshotRefs: customerOrMore ? safeRefs(artifact.screenshotRefs) : [],
    validationResultRefs: customerOrMore
      ? safeRefs(artifact.validationResultRefs)
      : [],
  }
}

export const projectOpenAgentsCloudflareContainerCloseoutReceipt = (
  receipt: OpenAgentsCloudflareContainerCloseoutReceipt,
  audience: OpenAgentsCloudflareContainerCloseoutAudience,
): OpenAgentsCloudflareContainerCloseoutProjection => {
  const artifact = artifactRefsForAudience(receipt, audience)
  const teamOrOperator = audience === 'team' || audience === 'operator'
  const operator = audience === 'operator'

  return {
    ...(teamOrOperator && receipt.artifactManifestRef !== undefined
      ? { artifactManifestRef: receipt.artifactManifestRef }
      : {}),
    audience,
    backendKind: 'cloudflare_container',
    buildLogRefs: artifact.buildLogRefs,
    ...(operator ? { callbackRef: receipt.callbackRef } : {}),
    closeoutReceiptRefs: safeRefs(receipt.closeoutReceiptRefs),
    diffRefs: artifact.diffRefs,
    eventRefs: teamOrOperator ? safeRefs(receipt.eventRefs) : [],
    ...(operator && receipt.externalRunRef !== undefined
      ? { externalRunRef: receipt.externalRunRef }
      : {}),
    generatedFileRefs: artifact.generatedFileRefs,
    operatorDiagnosticRefs: operator
      ? safeRefs(receipt.operatorDiagnosticRefs)
      : [],
    phase: receipt.phase,
    publicArtifactRefs: artifact.publicArtifactRefs,
    publicSummaryRef:
      safeRefs([receipt.publicSummaryRef])[0] ??
      'summary.cloudflare_container.closeout.redacted',
    redactionReportRefs: artifact.redactionReportRefs,
    runRef:
      safeRefs([receipt.runRef])[0] ??
      'run.cloudflare_container.closeout.redacted',
    screenshotRefs: artifact.screenshotRefs,
    scrubReceiptRefs: teamOrOperator
      ? safeRefs([
          ...receipt.credentialScrubReceiptRefs,
          ...receipt.providerAccountScrubReceiptRefs,
        ])
      : [],
    statusCaveatRefs: safeRefs(receipt.statusCaveatRefs),
    validationResultRefs: artifact.validationResultRefs,
  }
}

export const openAgentsCloudflareContainerArtifactManifestFromCloseout = (
  receipt: OpenAgentsCloudflareContainerCloseoutReceipt,
): OpenAgentsRunnerGatewayArtifactManifest => ({
  artifactRefs: safeRefs([
    ...receipt.artifactCloseout.generatedFileRefs,
    ...receipt.artifactCloseout.diffRefs,
    ...receipt.artifactCloseout.screenshotRefs,
    ...receipt.artifactCloseout.buildLogRefs,
    ...receipt.artifactCloseout.validationResultRefs,
    ...receipt.artifactCloseout.redactionReportRefs,
  ]),
  digestRef: `digest.${
    safeRefs([receipt.artifactManifestRef ?? receipt.runRef])[0] ??
    'cloudflare_container.closeout.redacted'
  }`,
  manifestRef:
    safeRefs([receipt.artifactManifestRef ?? receipt.runRef])[0] ??
    'manifest.cloudflare_container.closeout.redacted',
  publicArtifactRefs: safeRefs(receipt.artifactCloseout.publicArtifactRefs),
  receiptRefs: safeRefs(receipt.closeoutReceiptRefs),
})

export const openAgentsCloudflareContainerGatewayCallbackFromCloseout = (
  receipt: OpenAgentsCloudflareContainerCloseoutReceipt,
): OpenAgentsRunnerGatewayLifecycleCallback => ({
  ...(receipt.artifactManifestRef === undefined
    ? {}
    : { artifactManifestRef: receipt.artifactManifestRef }),
  backendKind: 'cloudflare_container',
  callbackRef: receipt.callbackRef,
  dispatchStatus: phaseToGatewayStatus[receipt.phase],
  eventRefs: safeRefs(receipt.eventRefs),
  ...(receipt.externalRunRef === undefined
    ? {}
    : { externalRunRef: receipt.externalRunRef }),
  receiptRefs: safeRefs(receipt.closeoutReceiptRefs),
  runRef: receipt.runRef,
  runnerId: receipt.runnerId,
})
