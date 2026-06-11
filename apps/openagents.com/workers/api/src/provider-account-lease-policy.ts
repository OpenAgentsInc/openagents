import type {
  ProviderAccountHealth,
  ProviderAccountProvider,
  ProviderAccountStatus,
} from './provider-account-domain'

export const PROVIDER_ACCOUNT_LEASE_POLICY_VERSION =
  'provider-account-lease-policy:v2' as const

export type ProviderAccountLeaseCandidate = Readonly<{
  providerAccountRef: string
  provider: ProviderAccountProvider
  status: ProviderAccountStatus
  health: ProviderAccountHealth
  hasSecretRef: boolean
  activeLeaseCount: number
  leaseLimit: number
  operatorPriority: number
  connectedAt: string | null
  createdAt: string
  lastSelectedAt: string | null
  lastSanityCheckAt: string | null
  lastSanityCheckResult: string | null
  lastParallelProbeAt: string | null
  recentFailureClass: string | null
  cooldownUntil: string | null
  lowCredit: boolean
}>

export type ProviderAccountLeaseSelectionOptions = Readonly<{
  requiredProvider?: ProviderAccountProvider | undefined
}>

export type ProviderAccountLeaseSelection =
  | Readonly<{
      status: 'selected'
      candidate: ProviderAccountLeaseCandidate
      reason: string
      policyVersion: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION
    }>
  | Readonly<{
      status: 'none'
      reason: string
      policyVersion: typeof PROVIDER_ACCOUNT_LEASE_POLICY_VERSION
    }>

const isAfterNow = (value: string | null, now: string): boolean =>
  value !== null && Date.parse(value) > Date.parse(now)

const usable = (
  candidate: ProviderAccountLeaseCandidate,
  now: string,
  requiredProvider: ProviderAccountProvider | undefined,
): boolean =>
  (requiredProvider === undefined ||
    candidate.provider === requiredProvider) &&
  candidate.status === 'connected' &&
  candidate.health === 'healthy' &&
  candidate.hasSecretRef &&
  candidate.activeLeaseCount < candidate.leaseLimit &&
  !candidate.lowCredit &&
  !isAfterNow(candidate.cooldownUntil, now)

const oldestUseTimestamp = (candidate: ProviderAccountLeaseCandidate): string =>
  candidate.lastSelectedAt ??
  candidate.lastParallelProbeAt ??
  candidate.lastSanityCheckAt ??
  candidate.connectedAt ??
  candidate.createdAt

const noEligibleCandidateReason = (
  requiredProvider: ProviderAccountProvider | undefined,
): string =>
  requiredProvider === undefined
    ? 'No connected healthy provider account is currently eligible for lease.'
    : `No connected healthy ${requiredProvider} account is currently eligible for lease.`

export const selectProviderAccountLeaseCandidate = (
  candidates: ReadonlyArray<ProviderAccountLeaseCandidate>,
  now: string,
  options: ProviderAccountLeaseSelectionOptions = {},
): ProviderAccountLeaseSelection => {
  const requiredProvider = options.requiredProvider
  const eligible = candidates.filter(candidate =>
    usable(candidate, now, requiredProvider),
  )

  if (eligible.length === 0) {
    return {
      status: 'none',
      reason: noEligibleCandidateReason(requiredProvider),
      policyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
    }
  }

  const selected = [...eligible].sort((left, right) => {
    const activeLeaseDelta = left.activeLeaseCount - right.activeLeaseCount

    if (activeLeaseDelta !== 0) {
      return activeLeaseDelta
    }

    const priorityDelta = left.operatorPriority - right.operatorPriority

    if (priorityDelta !== 0) {
      return priorityDelta
    }

    const recencyDelta =
      Date.parse(oldestUseTimestamp(left)) -
      Date.parse(oldestUseTimestamp(right))

    if (recencyDelta !== 0) {
      return recencyDelta
    }

    return left.providerAccountRef.localeCompare(right.providerAccountRef)
  })[0]

  if (selected === undefined) {
    return {
      status: 'none',
      reason: noEligibleCandidateReason(requiredProvider),
      policyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
    }
  }

  return {
    status: 'selected',
    candidate: selected,
    reason: `Selected connected healthy ${selected.provider} account with ${selected.activeLeaseCount} active lease(s), priority ${selected.operatorPriority}, and oldest successful use timestamp ${oldestUseTimestamp(selected)}.`,
    policyVersion: PROVIDER_ACCOUNT_LEASE_POLICY_VERSION,
  }
}
