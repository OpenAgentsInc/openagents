import type {
  AutopilotWorkProjection,
  AutopilotWorkSchedulingCron,
  AutopilotWorkSchedulingCronFreshness,
  AutopilotWorkSchedulingCronSchedule,
  AutopilotWorkSchedulingCronStatus,
  AutopilotWorkSchedulingCronTriggerKind,
} from '../model'

export type ForgeSchedulingCronViewStatus =
  | 'blocked'
  | 'empty'
  | 'paused'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeSchedulingCronAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  budgetMutationAuthority: false
  continuationApprovalAuthority: false
  credentialMutationAuthority: false
  deploymentAuthority: false
  notificationSendAuthority: false
  providerMutationAuthority: false
  publicClaimAuthority: false
  scheduleCreateAuthority: false
  scheduleDeleteAuthority: false
  schedulePauseAuthority: false
  scheduleResumeAuthority: false
  scheduleUpdateAuthority: false
  schedulerEnqueueAuthority: false
  schedulerFireAuthority: false
  settlementAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeSchedulingCronItem = Readonly<{
  adapterPreferenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  budgetPolicyRefs: ReadonlyArray<string>
  cancelReceiptRefs: ReadonlyArray<string>
  continuationPolicyRefs: ReadonlyArray<string>
  failureReceiptRefs: ReadonlyArray<string>
  fireReceiptRefs: ReadonlyArray<string>
  freshness: AutopilotWorkSchedulingCronFreshness
  lastRunRefs: ReadonlyArray<string>
  nextRunRefs: ReadonlyArray<string>
  noDoubleFireReceiptRefs: ReadonlyArray<string>
  notificationPolicyRefs: ReadonlyArray<string>
  ownerRefs: ReadonlyArray<string>
  permissionPolicyRefs: ReadonlyArray<string>
  providerPreferenceRefs: ReadonlyArray<string>
  repoRefs: ReadonlyArray<string>
  retentionPolicyRefs: ReadonlyArray<string>
  runReceiptRefs: ReadonlyArray<string>
  scheduleRef: string
  skipReceiptRefs: ReadonlyArray<string>
  status: AutopilotWorkSchedulingCronStatus
  teamRefs: ReadonlyArray<string>
  timezoneRefs: ReadonlyArray<string>
  triggerKind: AutopilotWorkSchedulingCronTriggerKind
  workOrderTemplateRefs: ReadonlyArray<string>
  workspaceRefs: ReadonlyArray<string>
}>

export type ForgeSchedulingCronInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  generatedAt: string
  schedules?: ReadonlyArray<AutopilotWorkSchedulingCronSchedule>
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeSchedulingCronCounts = Readonly<{
  active: number
  failed: number
  fired: number
  paused: number
  schedules: number
  skipped: number
  stale: number
}>

export type ForgeSchedulingCronView = Readonly<{
  authority: ForgeSchedulingCronAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeSchedulingCronCounts
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  schedules: ReadonlyArray<ForgeSchedulingCronItem>
  snapshotRef: string | null
  status: ForgeSchedulingCronViewStatus
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
const PRIVATE_SCHEDULING_MARKERS: ReadonlyArray<RegExp> = [
  /raw[-_ ](?:body|command|content|cron|file|log|payload|prompt|provider|schedule|shell|transcript|wallet)/i,
  /private[-_ ](?:content|cron|file|log|payload|prompt|repo|schedule|source|transcript|workspace)/i,
  /cron[-_ ](?:body|expression|payload|raw)/i,
  /schedule[-_ ](?:body|content|payload|raw)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /wallet[-_ ](?:material|mnemonic|private)/i,
  /customer[-_ ](?:data|private|payload)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeSchedulingCronAuthority = {
  acceptedOutcomeAuthority: false,
  budgetMutationAuthority: false,
  continuationApprovalAuthority: false,
  credentialMutationAuthority: false,
  deploymentAuthority: false,
  notificationSendAuthority: false,
  providerMutationAuthority: false,
  publicClaimAuthority: false,
  scheduleCreateAuthority: false,
  scheduleDeleteAuthority: false,
  schedulePauseAuthority: false,
  scheduleResumeAuthority: false,
  scheduleUpdateAuthority: false,
  schedulerEnqueueAuthority: false,
  schedulerFireAuthority: false,
  settlementAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_SCHEDULING_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-scheduling-cron-blocker:${workOrderRef}:${suffix}`

const normalizeSchedule = (
  schedule: AutopilotWorkSchedulingCronSchedule,
): Readonly<{
  omittedUnsafeRefCount: number
  schedule: ForgeSchedulingCronItem | null
}> => {
  const adapterPreferenceRefs = safeRefs(schedule.adapterPreferenceRefs)
  const blockerRefs = safeRefs(schedule.blockerRefs)
  const budgetPolicyRefs = safeRefs(schedule.budgetPolicyRefs)
  const cancelReceiptRefs = safeRefs(schedule.cancelReceiptRefs)
  const continuationPolicyRefs = safeRefs(schedule.continuationPolicyRefs)
  const failureReceiptRefs = safeRefs(schedule.failureReceiptRefs)
  const fireReceiptRefs = safeRefs(schedule.fireReceiptRefs)
  const lastRunRefs = safeRefs(schedule.lastRunRefs)
  const nextRunRefs = safeRefs(schedule.nextRunRefs)
  const noDoubleFireReceiptRefs = safeRefs(schedule.noDoubleFireReceiptRefs)
  const notificationPolicyRefs = safeRefs(schedule.notificationPolicyRefs)
  const ownerRefs = safeRefs(schedule.ownerRefs)
  const permissionPolicyRefs = safeRefs(schedule.permissionPolicyRefs)
  const providerPreferenceRefs = safeRefs(schedule.providerPreferenceRefs)
  const repoRefs = safeRefs(schedule.repoRefs)
  const retentionPolicyRefs = safeRefs(schedule.retentionPolicyRefs)
  const runReceiptRefs = safeRefs(schedule.runReceiptRefs)
  const scheduleRef = safeOptionalRef(schedule.scheduleRef)
  const skipReceiptRefs = safeRefs(schedule.skipReceiptRefs)
  const teamRefs = safeRefs(schedule.teamRefs)
  const timezoneRefs = safeRefs(schedule.timezoneRefs)
  const workOrderTemplateRefs = safeRefs(schedule.workOrderTemplateRefs)
  const workspaceRefs = safeRefs(schedule.workspaceRefs)
  const omittedUnsafeRefCount =
    adapterPreferenceRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    budgetPolicyRefs.omittedUnsafeRefCount +
    cancelReceiptRefs.omittedUnsafeRefCount +
    continuationPolicyRefs.omittedUnsafeRefCount +
    failureReceiptRefs.omittedUnsafeRefCount +
    fireReceiptRefs.omittedUnsafeRefCount +
    lastRunRefs.omittedUnsafeRefCount +
    nextRunRefs.omittedUnsafeRefCount +
    noDoubleFireReceiptRefs.omittedUnsafeRefCount +
    notificationPolicyRefs.omittedUnsafeRefCount +
    ownerRefs.omittedUnsafeRefCount +
    permissionPolicyRefs.omittedUnsafeRefCount +
    providerPreferenceRefs.omittedUnsafeRefCount +
    repoRefs.omittedUnsafeRefCount +
    retentionPolicyRefs.omittedUnsafeRefCount +
    runReceiptRefs.omittedUnsafeRefCount +
    scheduleRef.omittedUnsafeRefCount +
    skipReceiptRefs.omittedUnsafeRefCount +
    teamRefs.omittedUnsafeRefCount +
    timezoneRefs.omittedUnsafeRefCount +
    workOrderTemplateRefs.omittedUnsafeRefCount +
    workspaceRefs.omittedUnsafeRefCount

  return scheduleRef.ref === null
    ? { omittedUnsafeRefCount, schedule: null }
    : {
        omittedUnsafeRefCount,
        schedule: {
          adapterPreferenceRefs: adapterPreferenceRefs.refs,
          blockerRefs: blockerRefs.refs,
          budgetPolicyRefs: budgetPolicyRefs.refs,
          cancelReceiptRefs: cancelReceiptRefs.refs,
          continuationPolicyRefs: continuationPolicyRefs.refs,
          failureReceiptRefs: failureReceiptRefs.refs,
          fireReceiptRefs: fireReceiptRefs.refs,
          freshness: schedule.freshness ?? 'unknown',
          lastRunRefs: lastRunRefs.refs,
          nextRunRefs: nextRunRefs.refs,
          noDoubleFireReceiptRefs: noDoubleFireReceiptRefs.refs,
          notificationPolicyRefs: notificationPolicyRefs.refs,
          ownerRefs: ownerRefs.refs,
          permissionPolicyRefs: permissionPolicyRefs.refs,
          providerPreferenceRefs: providerPreferenceRefs.refs,
          repoRefs: repoRefs.refs,
          retentionPolicyRefs: retentionPolicyRefs.refs,
          runReceiptRefs: runReceiptRefs.refs,
          scheduleRef: scheduleRef.ref,
          skipReceiptRefs: skipReceiptRefs.refs,
          status: schedule.status,
          teamRefs: teamRefs.refs,
          timezoneRefs: timezoneRefs.refs,
          triggerKind: schedule.triggerKind,
          workOrderTemplateRefs: workOrderTemplateRefs.refs,
          workspaceRefs: workspaceRefs.refs,
        },
      }
}

const counts = (
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
): ForgeSchedulingCronCounts => ({
  active: schedules.filter(schedule => schedule.status === 'active').length,
  failed: schedules.filter(schedule => schedule.status === 'failed').length,
  fired: schedules.filter(schedule => schedule.status === 'fired').length,
  paused: schedules.filter(schedule => schedule.status === 'paused').length,
  schedules: schedules.length,
  skipped: schedules.filter(schedule => schedule.status === 'skipped').length,
  stale: schedules.filter(schedule => schedule.freshness === 'stale').length,
})

const isContinuationTrigger = (schedule: ForgeSchedulingCronItem): boolean =>
  schedule.triggerKind === 'continuation' ||
  schedule.triggerKind === 'retry_window'

const staleBlockers = (
  workOrderRef: string,
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
): ReadonlyArray<string> =>
  schedules
    .filter(
      schedule =>
        schedule.freshness === 'stale' && schedule.blockerRefs.length === 0,
    )
    .map(schedule =>
      blockerRef(workOrderRef, `stale-schedule-evidence:${schedule.scheduleRef}`),
    )

const activePolicyBlockers = (
  workOrderRef: string,
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
): ReadonlyArray<string> =>
  schedules
    .filter(
      schedule =>
        schedule.status === 'active' &&
        (schedule.budgetPolicyRefs.length === 0 ||
          schedule.permissionPolicyRefs.length === 0 ||
          schedule.workspaceRefs.length + schedule.repoRefs.length === 0) &&
        schedule.blockerRefs.length === 0,
    )
    .map(schedule =>
      blockerRef(
        workOrderRef,
        `active-schedule-policy-missing:${schedule.scheduleRef}`,
      ),
    )

const activeNextRunBlockers = (
  workOrderRef: string,
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
): ReadonlyArray<string> =>
  schedules
    .filter(
      schedule =>
        schedule.status === 'active' &&
        schedule.nextRunRefs.length === 0 &&
        schedule.blockerRefs.length === 0,
    )
    .map(schedule =>
      blockerRef(
        workOrderRef,
        `active-schedule-next-run-missing:${schedule.scheduleRef}`,
      ),
    )

const continuationPolicyBlockers = (
  workOrderRef: string,
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
): ReadonlyArray<string> =>
  schedules
    .filter(
      schedule =>
        isContinuationTrigger(schedule) &&
        schedule.continuationPolicyRefs.length === 0 &&
        schedule.blockerRefs.length === 0,
    )
    .map(schedule =>
      blockerRef(
        workOrderRef,
        `continuation-policy-missing:${schedule.scheduleRef}`,
      ),
    )

const firedReceiptBlockers = (
  workOrderRef: string,
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
): ReadonlyArray<string> =>
  schedules
    .filter(
      schedule =>
        schedule.status === 'fired' &&
        (schedule.fireReceiptRefs.length === 0 ||
          schedule.runReceiptRefs.length === 0) &&
        schedule.blockerRefs.length === 0,
    )
    .map(schedule =>
      blockerRef(
        workOrderRef,
        `fired-schedule-run-receipt-missing:${schedule.scheduleRef}`,
      ),
    )

const skippedReceiptBlockers = (
  workOrderRef: string,
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
): ReadonlyArray<string> =>
  schedules
    .filter(
      schedule =>
        schedule.status === 'skipped' &&
        schedule.skipReceiptRefs.length === 0 &&
        schedule.blockerRefs.length === 0,
    )
    .map(schedule =>
      blockerRef(
        workOrderRef,
        `skipped-schedule-receipt-missing:${schedule.scheduleRef}`,
      ),
    )

const terminalReceiptBlockers = (
  workOrderRef: string,
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
): ReadonlyArray<string> => [
  ...schedules
    .filter(
      schedule =>
        schedule.status === 'failed' &&
        schedule.failureReceiptRefs.length === 0 &&
        schedule.blockerRefs.length === 0,
    )
    .map(schedule =>
      blockerRef(
        workOrderRef,
        `failed-schedule-receipt-missing:${schedule.scheduleRef}`,
      ),
    ),
  ...schedules
    .filter(
      schedule =>
        schedule.status === 'cancelled' &&
        schedule.cancelReceiptRefs.length === 0 &&
        schedule.blockerRefs.length === 0,
    )
    .map(schedule =>
      blockerRef(
        workOrderRef,
        `cancelled-schedule-receipt-missing:${schedule.scheduleRef}`,
      ),
    ),
]

const recurringSafetyBlockers = (
  workOrderRef: string,
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
): ReadonlyArray<string> =>
  schedules
    .filter(
      schedule =>
        schedule.triggerKind === 'recurring' &&
        schedule.noDoubleFireReceiptRefs.length === 0 &&
        schedule.blockerRefs.length === 0,
    )
    .map(schedule =>
      blockerRef(
        workOrderRef,
        `recurring-no-double-fire-evidence-missing:${schedule.scheduleRef}`,
      ),
    )

const blockedStateBlockers = (
  workOrderRef: string,
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
): ReadonlyArray<string> =>
  schedules
    .filter(
      schedule =>
        schedule.status === 'blocked' && schedule.blockerRefs.length === 0,
    )
    .map(schedule =>
      blockerRef(
        workOrderRef,
        `blocked-schedule-without-blocker:${schedule.scheduleRef}`,
      ),
    )

const statusForView = (
  schedules: ReadonlyArray<ForgeSchedulingCronItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeSchedulingCronViewStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (schedules.length === 0) {
    return 'empty'
  }

  if (schedules.some(schedule => schedule.freshness === 'stale')) {
    return 'stale'
  }

  if (schedules.every(schedule => schedule.status === 'paused')) {
    return 'paused'
  }

  return schedules.every(schedule => schedule.freshness === 'unknown')
    ? 'unknown'
    : 'ready'
}

export const projectForgeSchedulingCron = (
  input: ForgeSchedulingCronInput,
): ForgeSchedulingCronView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedSchedules = (input.schedules ?? []).map(normalizeSchedule)
  const schedules = normalizedSchedules
    .flatMap(result => (result.schedule === null ? [] : [result.schedule]))
    .sort(
      (left, right) =>
        left.status.localeCompare(right.status) ||
        left.triggerKind.localeCompare(right.triggerKind) ||
        left.scheduleRef.localeCompare(right.scheduleRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedSchedules.reduce(
      (sum, result) => sum + result.omittedUnsafeRefCount,
      0,
    )
  const hasEntries = (input.schedules ?? []).length > 0
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...schedules.flatMap(schedule => schedule.blockerRefs),
      ...staleBlockers(input.workOrderRef, schedules),
      ...activePolicyBlockers(input.workOrderRef, schedules),
      ...activeNextRunBlockers(input.workOrderRef, schedules),
      ...continuationPolicyBlockers(input.workOrderRef, schedules),
      ...firedReceiptBlockers(input.workOrderRef, schedules),
      ...skippedReceiptBlockers(input.workOrderRef, schedules),
      ...terminalReceiptBlockers(input.workOrderRef, schedules),
      ...recurringSafetyBlockers(input.workOrderRef, schedules),
      ...blockedStateBlockers(input.workOrderRef, schedules),
      ...(hasEntries && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-scheduling-cron-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-scheduling-cron-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(schedules),
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    schedules,
    snapshotRef: snapshotRef.ref,
    status: statusForView(schedules, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeSchedulingCronInput = (
  work: AutopilotWorkProjection,
): ForgeSchedulingCronInput => {
  const source: AutopilotWorkSchedulingCron | undefined = work.schedulingCron

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
    ...(source.schedules === undefined ? {} : { schedules: source.schedules }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
