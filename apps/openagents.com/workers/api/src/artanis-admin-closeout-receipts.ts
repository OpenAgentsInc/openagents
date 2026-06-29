import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { parseJsonStringArray } from './json-boundary'
import {
  assertNexusPylonPublicSafe,
  type NexusPylonPublicReceiptDetail,
} from './nexus-pylon-visibility'
import {
  publicScannerSafeRef,
  publicScannerSafeRefs,
} from './public-ref-scanner-safety'

type NullableString = string | null

type ArtanisAdminCloseoutReceiptRow = Readonly<{
  accepted_work_refs_json: string
  artifact_refs_json: string
  assignment_created_at: string
  assignment_ref: string
  assignment_state: string
  assignment_updated_at: string
  closeout_refs_json: string
  decision_created_at: NullableString
  decision_id: NullableString
  decision_state: NullableString
  job_kind: string
  proof_refs_json: string
  pylon_ref: string
  verdict_accept_state: NullableString
  verdict_created_at: NullableString
  verdict_outcome: NullableString
  verdict_trace_digest_prefix: NullableString
}>

export type ArtanisAdminCloseoutReceiptRecord = Readonly<{
  acceptedWorkRefs: ReadonlyArray<string>
  artifactRefs: ReadonlyArray<string>
  assignmentCreatedAt: string
  assignmentRef: string
  assignmentState: string
  assignmentUpdatedAt: string
  claimedTraceDigest: string | null
  claimedTraceDigestPrefix: string | null
  closeoutRefs: ReadonlyArray<string>
  decisionCreatedAt: string | null
  decisionId: string | null
  decisionState: string | null
  jobKind: string
  proofRefs: ReadonlyArray<string>
  pylonRef: string
  verdictAcceptState: string | null
  verdictCreatedAt: string | null
  verdictOutcome: string | null
  verdictRef: string | null
}>

export type ArtanisAdminCloseoutReceiptStore = Readonly<{
  readCloseoutReceiptByRef: (
    receiptRef: string,
  ) => Promise<ArtanisAdminCloseoutReceiptRecord | undefined>
}>

const assignmentRefPattern = /^assignment\.artanis_admin\.[A-Za-z0-9_.-]+$/
const closeoutReceiptPrefix = 'receipt.nexus_pylon.artanis_admin_closeout.'
const digestPattern = /([a-f0-9]{64})/

const normalizeLookupRef = (receiptRef: string): string =>
  decodeURIComponent(receiptRef).trim()

export const artanisAdminCloseoutReceiptRef = (
  assignmentRef: string,
): string => `${closeoutReceiptPrefix}${assignmentRef}`

export const artanisAdminAssignmentRefFromReceiptLookup = (
  receiptRef: string,
): string | null => {
  const normalized = normalizeLookupRef(receiptRef)
  const assignmentRef = normalized.startsWith(closeoutReceiptPrefix)
    ? normalized.slice(closeoutReceiptPrefix.length)
    : normalized

  return assignmentRefPattern.test(assignmentRef) ? assignmentRef : null
}

const traceDigestFromRefs = (refs: ReadonlyArray<string>): string | null => {
  for (const ref of refs) {
    const match = digestPattern.exec(ref)

    if (match !== null) {
      return match[1]!
    }
  }

  return null
}

const traceDigestPrefixFromRefs = (
  refs: ReadonlyArray<string>,
): string | null => {
  const digest = traceDigestFromRefs(refs)

  if (digest !== null) {
    return digest.slice(0, 16)
  }

  for (const ref of refs) {
    const match = /trace_digest\.([a-f0-9]{16})/.exec(ref)

    if (match !== null) {
      return match[1]!
    }
  }

  return null
}

const rowString = (value: unknown): string =>
  typeof value === 'string' ? value : ''

const rowNullableString = (value: unknown): string | null =>
  typeof value === 'string' && value.trim() !== '' ? value : null

const publicRefs = (
  scope: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => publicScannerSafeRefs(scope, refs)

export const artanisAdminCloseoutReceiptRecordFromRow = (
  row: Record<string, unknown>,
): ArtanisAdminCloseoutReceiptRecord => {
  const typed = row as Partial<ArtanisAdminCloseoutReceiptRow>
  const artifactRefs = parseJsonStringArray(typed.artifact_refs_json)
  const proofRefs = parseJsonStringArray(typed.proof_refs_json)
  const closeoutRefs = parseJsonStringArray(typed.closeout_refs_json)
  const digest =
    traceDigestFromRefs(artifactRefs) ??
    traceDigestFromRefs(proofRefs) ??
    traceDigestFromRefs(closeoutRefs)
  const digestPrefix =
    rowNullableString(typed.verdict_trace_digest_prefix) ??
    (digest === null
      ? traceDigestPrefixFromRefs([...artifactRefs, ...proofRefs, ...closeoutRefs])
      : digest.slice(0, 16))
  const verdictOutcome = rowNullableString(typed.verdict_outcome)

  return {
    acceptedWorkRefs: parseJsonStringArray(typed.accepted_work_refs_json),
    artifactRefs,
    assignmentCreatedAt: rowString(typed.assignment_created_at),
    assignmentRef: rowString(typed.assignment_ref),
    assignmentState: rowString(typed.assignment_state),
    assignmentUpdatedAt: rowString(typed.assignment_updated_at),
    claimedTraceDigest: digest,
    claimedTraceDigestPrefix: digestPrefix,
    closeoutRefs,
    decisionCreatedAt: rowNullableString(typed.decision_created_at),
    decisionId: rowNullableString(typed.decision_id),
    decisionState: rowNullableString(typed.decision_state),
    jobKind: rowString(typed.job_kind),
    proofRefs,
    pylonRef: rowString(typed.pylon_ref),
    verdictAcceptState: rowNullableString(typed.verdict_accept_state),
    verdictCreatedAt: rowNullableString(typed.verdict_created_at),
    verdictOutcome,
    verdictRef:
      verdictOutcome === null ? null : `verdict.artanis_closeout.${verdictOutcome}`,
  }
}

const closeoutStatus = (
  record: ArtanisAdminCloseoutReceiptRecord,
): string => {
  if (
    record.assignmentState === 'accepted_work' &&
    record.verdictOutcome === 'verified' &&
    record.verdictAcceptState === 'accepted'
  ) {
    return 'accepted_work_verified'
  }

  if (record.verdictOutcome === 'verified') {
    return 'closeout_verified'
  }

  if (record.verdictOutcome === 'rejected') {
    return 'closeout_rejected'
  }

  if (record.verdictOutcome === 'unreadable') {
    return 'closeout_unreadable'
  }

  return record.assignmentState === '' ? 'unknown' : record.assignmentState
}

const closeoutStateLabel = (
  record: ArtanisAdminCloseoutReceiptRecord,
): string => {
  const status = closeoutStatus(record)

  return status === 'accepted_work_verified'
    ? 'Accepted work verified'
    : status === 'closeout_verified'
      ? 'Closeout verified'
      : status === 'closeout_rejected'
        ? 'Closeout rejected'
        : status === 'closeout_unreadable'
          ? 'Closeout unreadable'
          : `Assignment ${status.replaceAll('_', ' ')}`
}

const displayTime = (
  value: string | null,
  nowIso: string,
): string | null =>
  value === null || value.trim() === ''
    ? null
    : friendlyBlueprintMissionBriefingTime(value, nowIso)

const assignmentReceiptRoute = (assignmentRef: string): string =>
  `route:/api/public/nexus-pylon/receipts/${assignmentRef}`

const expectationRef = (
  record: ArtanisAdminCloseoutReceiptRecord,
): string | null =>
  record.claimedTraceDigestPrefix === null
    ? null
    : `expectation.tassadar_poc.trace_digest.${record.claimedTraceDigestPrefix}`

export const artanisAdminCloseoutReceiptDetail = (
  input: Readonly<{
    appUrl: string
    nowIso: string
    record: ArtanisAdminCloseoutReceiptRecord
  }>,
): NexusPylonPublicReceiptDetail => {
  const receiptRef = artanisAdminCloseoutReceiptRef(input.record.assignmentRef)
  const expectation = expectationRef(input.record)
  const status = closeoutStatus(input.record)
  const detail: NexusPylonPublicReceiptDetail = {
    schemaVersion: 'openagents.nexus_pylon.public_receipt.v1',
    apiUrl: `${input.appUrl}/api/public/nexus-pylon/receipts/${encodeURIComponent(
      receiptRef,
    )}`,
    assignmentRef: input.record.assignmentRef,
    audience: 'public',
    caveatRefs: [
      'caveat.public.artanis_admin.closeout_receipt_not_payout_settlement',
      'caveat.public.artanis_admin.closeout_receipt_omits_private_runner_logs',
      'caveat.public.no_private_payment_material',
    ],
    movementMode: 'simulation',
    payoutAttemptRef: null,
    payoutIntentRef: null,
    publicProjection: {
      projectionKind: 'artanis_admin_assignment_closeout',
      acceptedWorkObserved: input.record.assignmentState === 'accepted_work',
      acceptedWorkRefs: publicRefs(
        'accepted_work.public.artanis_admin.closeout',
        input.record.acceptedWorkRefs,
      ),
      artifactRefs: publicRefs(
        'artifact.public.artanis_admin.closeout',
        input.record.artifactRefs,
      ),
      assignmentCreatedAtDisplay: displayTime(
        input.record.assignmentCreatedAt,
        input.nowIso,
      ),
      assignmentRef: input.record.assignmentRef,
      assignmentState: input.record.assignmentState,
      assignmentUpdatedAtDisplay: displayTime(
        input.record.assignmentUpdatedAt,
        input.nowIso,
      ),
      claimedTraceDigest: input.record.claimedTraceDigest,
      claimedTraceDigestPrefix: input.record.claimedTraceDigestPrefix,
      closeoutRefs: publicRefs(
        'closeout.public.artanis_admin.closeout',
        input.record.closeoutRefs,
      ),
      closeoutSubmittedObserved: input.record.closeoutRefs.length > 0,
      decisionCreatedAtDisplay: displayTime(
        input.record.decisionCreatedAt,
        input.nowIso,
      ),
      decisionRef:
        input.record.decisionId === null
          ? null
          : publicScannerSafeRef(
              'decision.public.artanis_admin.tick',
              `artanis_admin_tick_decision.${input.record.decisionId}`,
            ),
      decisionState: input.record.decisionState,
      evidenceRefs: publicRefs(
        'evidence.public.artanis_admin.closeout',
        [
          input.record.assignmentRef,
          assignmentReceiptRoute(input.record.assignmentRef),
          ...(input.record.verdictRef === null ? [] : [input.record.verdictRef]),
          ...(expectation === null ? [] : [expectation]),
        ],
      ),
      expectationRef: expectation,
      jobKind: input.record.jobKind,
      proofRefs: publicRefs(
        'proof.public.artanis_admin.closeout',
        input.record.proofRefs,
      ),
      pylonRef: publicScannerSafeRef(
        'pylon.public.artanis_admin.closeout',
        input.record.pylonRef,
      ),
      verdictAcceptState: input.record.verdictAcceptState,
      verdictCreatedAtDisplay: displayTime(
        input.record.verdictCreatedAt,
        input.nowIso,
      ),
      verdictOutcome: input.record.verdictOutcome,
      verdictRef: input.record.verdictRef,
    },
    realBitcoinMoved: false,
    receiptKind: 'artanis_admin_assignment_closeout',
    receiptPageUrl: `${input.appUrl}/nexus-pylon/receipts/${encodeURIComponent(
      receiptRef,
    )}`,
    receiptRef,
    payoutMovement: {
      dispatchAccepted: input.record.decisionState === 'dispatched',
      terminalResultObserved: input.record.verdictOutcome !== null,
      terminalSettlementClaimAllowed: false,
    },
    settlement: {
      buyerPaymentEvidencePresent: false,
      liveWalletSpendAllowed: false,
      providerRef: publicScannerSafeRef(
        'pylon.public.artanis_admin.closeout',
        input.record.pylonRef,
      ),
      settlementMutationAllowed: false,
      settlementRefs: publicRefs(
        'settlement.public.artanis_admin.closeout',
        [
          input.record.assignmentRef,
          ...(input.record.verdictRef === null ? [] : [input.record.verdictRef]),
          ...(expectation === null ? [] : [expectation]),
        ],
      ),
      state: status,
      stateLabel: closeoutStateLabel(input.record),
      updatedAtDisplay:
        displayTime(input.record.verdictCreatedAt, input.nowIso) ??
        displayTime(input.record.assignmentUpdatedAt, input.nowIso) ??
        'not recorded',
      walletReadinessStateLabel: 'Not a payout receipt',
    },
    status,
  }

  assertNexusPylonPublicSafe('Artanis admin closeout public receipt', detail)

  return detail
}

export const makeD1ArtanisAdminCloseoutReceiptStore = (
  db: D1Database,
): ArtanisAdminCloseoutReceiptStore => ({
  readCloseoutReceiptByRef: async receiptRef => {
    const assignmentRef = artanisAdminAssignmentRefFromReceiptLookup(receiptRef)

    if (assignmentRef === null) {
      return undefined
    }

    const row = await db
      .prepare(
        `SELECT a.assignment_ref,
                a.pylon_ref,
                a.job_kind,
                a.state AS assignment_state,
                a.accepted_work_refs_json,
                a.artifact_refs_json,
                a.proof_refs_json,
                a.closeout_refs_json,
                a.created_at AS assignment_created_at,
                a.updated_at AS assignment_updated_at,
                d.id AS decision_id,
                d.state AS decision_state,
                d.created_at AS decision_created_at,
                v.outcome AS verdict_outcome,
                v.claimed_trace_digest_prefix AS verdict_trace_digest_prefix,
                v.accept_state AS verdict_accept_state,
                v.created_at AS verdict_created_at
           FROM pylon_api_assignments a
      LEFT JOIN artanis_admin_tick_decisions d
             ON d.assignment_ref = a.assignment_ref
      LEFT JOIN artanis_closeout_verdicts v
             ON v.assignment_ref = a.assignment_ref
          WHERE a.archived_at IS NULL
            AND a.assignment_ref = ?
            AND a.assignment_ref LIKE 'assignment.artanis_admin.%'
       ORDER BY d.created_at DESC
          LIMIT 1`,
      )
      .bind(assignmentRef)
      .first<ArtanisAdminCloseoutReceiptRow>()

    return row === null
      ? undefined
      : artanisAdminCloseoutReceiptRecordFromRow(row)
  },
})
