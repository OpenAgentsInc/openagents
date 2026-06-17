import type {
  AutopilotWorkExternalWorkIntake,
  AutopilotWorkExternalWorkIntakeChannel,
  AutopilotWorkExternalWorkIntakeEntry,
  AutopilotWorkExternalWorkIntakeFreshness,
  AutopilotWorkExternalWorkIntakeKind,
  AutopilotWorkExternalWorkIntakeStatus,
  AutopilotWorkProjection,
} from '../model'

export type ForgeExternalWorkIntakeStatus =
  | 'admitted'
  | 'blocked'
  | 'delivered'
  | 'empty'
  | 'expired'
  | 'pending'
  | 'rejected'
  | 'routed'
  | 'stale'
  | 'unknown'

export type ForgeExternalWorkIntakeAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  adapterSelectionAuthority: false
  admissionAuthority: false
  budgetReserveAuthority: false
  deploymentAuthority: false
  enqueueWorkAuthority: false
  paymentAuthority: false
  publicClaimAuthority: false
  rejectionAuthority: false
  settlementAuthority: false
  startExecutionAuthority: false
  workOrderCreateAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeExternalWorkIntakeItem = Readonly<{
  acceptancePolicyRefs: ReadonlyArray<string>
  accountRefs: ReadonlyArray<string>
  adapterPreferenceRefs: ReadonlyArray<string>
  admissionReceiptRefs: ReadonlyArray<string>
  apiParityRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  budgetRefs: ReadonlyArray<string>
  budgetRequired: boolean
  capabilityRefs: ReadonlyArray<string>
  channel: AutopilotWorkExternalWorkIntakeChannel
  dataClassificationRefs: ReadonlyArray<string>
  deliveryReceiptRefs: ReadonlyArray<string>
  expirationRefs: ReadonlyArray<string>
  freshness: AutopilotWorkExternalWorkIntakeFreshness
  idempotencyRefs: ReadonlyArray<string>
  intakeRef: string
  paymentRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  rejectionReceiptRefs: ReadonlyArray<string>
  requestRefs: ReadonlyArray<string>
  requesterRefs: ReadonlyArray<string>
  reviewPolicyRefs: ReadonlyArray<string>
  routingReceiptRefs: ReadonlyArray<string>
  scopeRefs: ReadonlyArray<string>
  status: AutopilotWorkExternalWorkIntakeStatus
  statusReceiptRefs: ReadonlyArray<string>
  verificationRefs: ReadonlyArray<string>
  workKind: AutopilotWorkExternalWorkIntakeKind
  workOrderRefs: ReadonlyArray<string>
}>

export type ForgeExternalWorkIntakeInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkExternalWorkIntakeEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeExternalWorkIntakeCounts = Readonly<{
  admitted: number
  delivered: number
  pending: number
  rejected: number
  routed: number
  total: number
}>

export type ForgeExternalWorkIntakeView = Readonly<{
  authority: ForgeExternalWorkIntakeAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeExternalWorkIntakeCounts
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeExternalWorkIntakeStatus
  versionRef: string | null
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

type OptionalRefBundle = Readonly<{
  omittedUnsafeRefCount: number
  ref: string | null
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_INTAKE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:artifact|body|command|content|customer|file|intake|issue|log|objective|payload|prompt|provider|repo|request|shell|source|workspace)/i,
  /private[-_ ](?:artifact|content|customer|file|intake|objective|payload|prompt|repo|request|source|workspace)/i,
  /customer[-_ ]data/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeExternalWorkIntakeAuthority = {
  acceptedOutcomeAuthority: false,
  adapterSelectionAuthority: false,
  admissionAuthority: false,
  budgetReserveAuthority: false,
  deploymentAuthority: false,
  enqueueWorkAuthority: false,
  paymentAuthority: false,
  publicClaimAuthority: false,
  rejectionAuthority: false,
  settlementAuthority: false,
  startExecutionAuthority: false,
  workOrderCreateAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_INTAKE_MARKERS.some(marker => marker.test(trimmed))
    ? trimmed
    : null
}

const safeRefs = (
  ...groups: ReadonlyArray<ReadonlyArray<string> | undefined>
): RefBundle => {
  const refs = groups.flatMap(group => group ?? [])
  const sanitized = refs.reduce<Readonly<{ omitted: number; refs: string[] }>>(
    (state, ref) => {
      const safe = safeRef(ref)

      return safe === null
        ? { omitted: state.omitted + 1, refs: state.refs }
        : { omitted: state.omitted, refs: [...state.refs, safe] }
    },
    { omitted: 0, refs: [] },
  )

  return {
    omittedUnsafeRefCount: sanitized.omitted,
    refs: Array.from(new Set(sanitized.refs)),
  }
}

const safeOptionalRef = (
  value: string | null | undefined,
): OptionalRefBundle => {
  if (value === null || value === undefined) {
    return { omittedUnsafeRefCount: 0, ref: null }
  }

  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-external-work-intake-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkExternalWorkIntakeEntry,
): Readonly<{
  entry: ForgeExternalWorkIntakeItem | null
  omittedUnsafeRefCount: number
}> => {
  const acceptancePolicyRefs = safeRefs(entry.acceptancePolicyRefs)
  const accountRefs = safeRefs(entry.accountRefs)
  const adapterPreferenceRefs = safeRefs(entry.adapterPreferenceRefs)
  const admissionReceiptRefs = safeRefs(entry.admissionReceiptRefs)
  const apiParityRefs = safeRefs(entry.apiParityRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const budgetRefs = safeRefs(entry.budgetRefs)
  const capabilityRefs = safeRefs(entry.capabilityRefs)
  const dataClassificationRefs = safeRefs(entry.dataClassificationRefs)
  const deliveryReceiptRefs = safeRefs(entry.deliveryReceiptRefs)
  const expirationRefs = safeRefs(entry.expirationRefs)
  const idempotencyRefs = safeRefs(entry.idempotencyRefs)
  const intakeRef = safeOptionalRef(entry.intakeRef)
  const paymentRefs = safeRefs(entry.paymentRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const rejectionReceiptRefs = safeRefs(entry.rejectionReceiptRefs)
  const requestRefs = safeRefs(entry.requestRefs)
  const requesterRefs = safeRefs(entry.requesterRefs)
  const reviewPolicyRefs = safeRefs(entry.reviewPolicyRefs)
  const routingReceiptRefs = safeRefs(entry.routingReceiptRefs)
  const scopeRefs = safeRefs(entry.scopeRefs)
  const statusReceiptRefs = safeRefs(entry.statusReceiptRefs)
  const verificationRefs = safeRefs(entry.verificationRefs)
  const workOrderRefs = safeRefs(entry.workOrderRefs)
  const omittedUnsafeRefCount =
    acceptancePolicyRefs.omittedUnsafeRefCount +
    accountRefs.omittedUnsafeRefCount +
    adapterPreferenceRefs.omittedUnsafeRefCount +
    admissionReceiptRefs.omittedUnsafeRefCount +
    apiParityRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    budgetRefs.omittedUnsafeRefCount +
    capabilityRefs.omittedUnsafeRefCount +
    dataClassificationRefs.omittedUnsafeRefCount +
    deliveryReceiptRefs.omittedUnsafeRefCount +
    expirationRefs.omittedUnsafeRefCount +
    idempotencyRefs.omittedUnsafeRefCount +
    intakeRef.omittedUnsafeRefCount +
    paymentRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    rejectionReceiptRefs.omittedUnsafeRefCount +
    requestRefs.omittedUnsafeRefCount +
    requesterRefs.omittedUnsafeRefCount +
    reviewPolicyRefs.omittedUnsafeRefCount +
    routingReceiptRefs.omittedUnsafeRefCount +
    scopeRefs.omittedUnsafeRefCount +
    statusReceiptRefs.omittedUnsafeRefCount +
    verificationRefs.omittedUnsafeRefCount +
    workOrderRefs.omittedUnsafeRefCount

  return intakeRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          acceptancePolicyRefs: acceptancePolicyRefs.refs,
          accountRefs: accountRefs.refs,
          adapterPreferenceRefs: adapterPreferenceRefs.refs,
          admissionReceiptRefs: admissionReceiptRefs.refs,
          apiParityRefs: apiParityRefs.refs,
          blockerRefs: blockerRefs.refs,
          budgetRefs: budgetRefs.refs,
          budgetRequired: entry.budgetRequired ?? false,
          capabilityRefs: capabilityRefs.refs,
          channel: entry.channel,
          dataClassificationRefs: dataClassificationRefs.refs,
          deliveryReceiptRefs: deliveryReceiptRefs.refs,
          expirationRefs: expirationRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          idempotencyRefs: idempotencyRefs.refs,
          intakeRef: intakeRef.ref,
          paymentRefs: paymentRefs.refs,
          policyRefs: policyRefs.refs,
          rejectionReceiptRefs: rejectionReceiptRefs.refs,
          requestRefs: requestRefs.refs,
          requesterRefs: requesterRefs.refs,
          reviewPolicyRefs: reviewPolicyRefs.refs,
          routingReceiptRefs: routingReceiptRefs.refs,
          scopeRefs: scopeRefs.refs,
          status: entry.status,
          statusReceiptRefs: statusReceiptRefs.refs,
          verificationRefs: verificationRefs.refs,
          workKind: entry.workKind,
          workOrderRefs: workOrderRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
): ForgeExternalWorkIntakeCounts => ({
  admitted: entries.filter(entry => entry.status === 'admitted').length,
  delivered: entries.filter(entry => entry.status === 'delivered').length,
  pending: entries.filter(entry => entry.status === 'pending').length,
  rejected: entries.filter(entry => entry.status === 'rejected').length,
  routed: entries.filter(entry => entry.status === 'routed').length,
  total: entries.length,
})

const hasPrivateScope = (entry: ForgeExternalWorkIntakeItem): boolean =>
  [...entry.scopeRefs, ...entry.dataClassificationRefs].some(ref =>
    /(?:^|[.:/_-])(?:private|customer_private|restricted)(?:$|[.:/_-])/i.test(ref),
  )

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-intake-evidence:${entry.intakeRef}`))

const identityBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.requesterRefs.length === 0 || entry.accountRefs.length === 0) &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `intake-identity-missing:${entry.intakeRef}`))

const budgetBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.budgetRequired &&
        entry.budgetRefs.length === 0 &&
        entry.paymentRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `budget-or-payment-ref-missing:${entry.intakeRef}`))

const capabilityBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.status === 'admitted' ||
          entry.status === 'routed' ||
          entry.status === 'delivered') &&
        entry.capabilityRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `intake-capability-ref-missing:${entry.intakeRef}`))

const adapterRoutingBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.adapterPreferenceRefs.length > 0 &&
        (entry.routingReceiptRefs.length === 0 || entry.policyRefs.length === 0) &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `adapter-routing-policy-missing:${entry.intakeRef}`))

const paymentAdmissionBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.paymentRefs.length > 0 &&
        (entry.admissionReceiptRefs.length === 0 ||
          entry.routingReceiptRefs.length === 0) &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `payment-without-admission-routing:${entry.intakeRef}`))

const idempotencyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.status === 'pending' ||
          entry.status === 'admitted' ||
          entry.status === 'routed') &&
        entry.idempotencyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `idempotency-ref-missing:${entry.intakeRef}`))

const apiParityBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.channel === 'ui' &&
        entry.apiParityRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `browser-action-api-parity-missing:${entry.intakeRef}`))

const privateScopeBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        hasPrivateScope(entry) &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `private-intake-scope-policy-missing:${entry.intakeRef}`))

const statusForView = (
  entries: ReadonlyArray<ForgeExternalWorkIntakeItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeExternalWorkIntakeStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.status === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.status === 'rejected')) {
    return 'rejected'
  }

  if (entries.some(entry => entry.status === 'expired')) {
    return 'expired'
  }

  if (entries.some(entry => entry.status === 'pending')) {
    return 'pending'
  }

  if (entries.some(entry => entry.status === 'routed')) {
    return 'routed'
  }

  if (entries.some(entry => entry.status === 'admitted')) {
    return 'admitted'
  }

  return entries.every(entry => entry.status === 'delivered') ? 'delivered' : 'unknown'
}

export const projectForgeExternalWorkIntake = (
  input: ForgeExternalWorkIntakeInput,
): ForgeExternalWorkIntakeView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.status.localeCompare(right.status) ||
        left.channel.localeCompare(right.channel) ||
        left.intakeRef.localeCompare(right.intakeRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...staleBlockers(input.workOrderRef, entries),
      ...identityBlockers(input.workOrderRef, entries),
      ...budgetBlockers(input.workOrderRef, entries),
      ...capabilityBlockers(input.workOrderRef, entries),
      ...adapterRoutingBlockers(input.workOrderRef, entries),
      ...paymentAdmissionBlockers(input.workOrderRef, entries),
      ...idempotencyBlockers(input.workOrderRef, entries),
      ...apiParityBlockers(input.workOrderRef, entries),
      ...privateScopeBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-external-work-intake-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-external-work-intake-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(entries, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeExternalWorkIntakeInput = (
  work: AutopilotWorkProjection,
): ForgeExternalWorkIntakeInput => {
  const source: AutopilotWorkExternalWorkIntake | undefined =
    work.externalWorkIntake

  if (source === undefined) {
    return {
      generatedAt: work.generatedAt,
      workOrderRef: work.workOrderRef,
    }
  }

  return {
    generatedAt: source.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source.entries === undefined ? {} : { entries: source.entries }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
