import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

import type { OpenAgentsAutopilotRepositoryVisibility } from './autopilot-work-request'
import type { CodingAutopilotRepoTrustTier } from './coding-autopilot-repo-placement'
import { isoTimestampAfterIso } from './runtime-primitives'

export const PACK_C_REPO_WORKTREE_IDENTITY_VERSION =
  'pack-c-repo-worktree-identity:v1' as const

const PACK_C_REPO_WORKTREE_IDENTITY_COLLECTION =
  'pack_c_repo_worktree_identity_public'

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const SAFE_REPOSITORY_PART_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,100}$/
const SAFE_HOST_PATTERN = /^[A-Za-z0-9][A-Za-z0-9.-]{0,120}$/
const SAFE_BRANCH_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_./-]{0,180}$/
const SHA_REF_PATTERN =
  /^(?:[A-Fa-f0-9]{7,64}|[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260})$/
const PACK_C_PRIVATE_MARKERS: ReadonlyArray<RegExp> = [
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /github\.com[:/][^\s]+\/(?:private|secret|customer|internal)/i,
  /\bprivate[-_]repo\b/i,
  /\bprivate[-_]content\b/i,
  /raw[-_ ]prompt/i,
  /raw[-_ ]shell/i,
  /shell[-_ ]fragment/i,
  /(?:^|\s)\/Users\//,
  /\.ssh\//i,
  /\.git\/config/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
]

class PackCRepoWorktreeIdentityError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'PackCRepoWorktreeIdentityError'
  }
}

export type PackCRepositoryIdentityInput = Readonly<{
  caveatRefs?: ReadonlyArray<string> | undefined
  dataScopeRefs?: ReadonlyArray<string> | undefined
  defaultBranch: string | null
  host: string
  name: string
  owner: string
  pinnedCommitRef: string | null
  remoteDigestRef: string | null
  repositoryRef: string
  trustTier: CodingAutopilotRepoTrustTier
  visibility: OpenAgentsAutopilotRepositoryVisibility
}>

export type PackCWorktreeIdentityInput = Readonly<{
  baseCommitRef: string | null
  branchRef: string
  cleanliness: 'clean' | 'dirty' | 'unknown'
  headCommitRef: string | null
  retentionPolicyRef: string | null
  sandboxProfileRef: string | null
  worktreeRef: string
  workspaceRef: string
}>

export type PackCRepoWorktreeIdentityInput = Readonly<{
  generatedAt: string
  observedAt: string
  repository: PackCRepositoryIdentityInput
  staleAfterMs: number
  worktree: PackCWorktreeIdentityInput
}>

export type PackCRepositoryIdentityProjection = Readonly<{
  caveatRefs: ReadonlyArray<string>
  dataScopeRefs: ReadonlyArray<string>
  defaultBranch: string | null
  host: string
  name: string
  owner: string
  pinnedCommitRef: string | null
  remoteDigestRef: string | null
  repositoryRef: string
  trustTier: CodingAutopilotRepoTrustTier
  visibility: OpenAgentsAutopilotRepositoryVisibility
}>

export type PackCWorktreeIdentityProjection = Readonly<{
  baseCommitRef: string | null
  branchRef: string
  cleanliness: 'clean' | 'dirty' | 'unknown'
  headCommitRef: string | null
  retentionPolicyRef: string | null
  sandboxProfileRef: string | null
  worktreeRef: string
  workspaceRef: string
}>

export type PackCRepoWorktreeIdentityProjection = Readonly<{
  ageMs: number
  blockerRefs: ReadonlyArray<string>
  freshness: 'fresh' | 'stale'
  generatedAt: string
  identityVersion: typeof PACK_C_REPO_WORKTREE_IDENTITY_VERSION
  observedAt: string
  repository: PackCRepositoryIdentityProjection
  staleAt: string
  status: 'ready' | 'blocked' | 'stale'
  worktree: PackCWorktreeIdentityProjection
}>

const assertNoPrivateMaterial = (value: unknown, context: string): void => {
  assertNoProviderSecretMaterial(value, context)

  const text = typeof value === 'string' ? value : JSON.stringify(value)

  if (PACK_C_PRIVATE_MARKERS.some(marker => marker.test(text))) {
    throw new PackCRepoWorktreeIdentityError(
      `${context} contains private repo, local path, or shell material.`,
    )
  }
}

const safeRef = (field: string, value: string): string => {
  const trimmed = value.trim()
  assertNoPrivateMaterial(trimmed, field)

  if (!SAFE_REF_PATTERN.test(trimmed)) {
    throw new PackCRepoWorktreeIdentityError(
      `${field} must be a stable Pack C ref.`,
    )
  }

  return trimmed
}

const safeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => (values ?? []).map(value => safeRef(field, value))

const safeRepositoryPart = (field: string, value: string): string => {
  const trimmed = value.trim()
  assertNoPrivateMaterial(trimmed, field)

  if (!SAFE_REPOSITORY_PART_PATTERN.test(trimmed)) {
    throw new PackCRepoWorktreeIdentityError(
      `${field} must be a safe repository identifier part.`,
    )
  }

  return trimmed
}

const safeHost = (value: string): string => {
  const trimmed = value.trim().toLowerCase()
  assertNoPrivateMaterial(trimmed, 'pack-c-repo-identity.host')

  if (!SAFE_HOST_PATTERN.test(trimmed) || trimmed.includes('..')) {
    throw new PackCRepoWorktreeIdentityError(
      'Repository host must be a safe host identifier.',
    )
  }

  return trimmed
}

export const normalizePackCBranchRef = (branchRef: string): string => {
  const trimmed = branchRef.trim()
  assertNoPrivateMaterial(trimmed, 'pack-c-repo-identity.branchRef')

  if (
    !SAFE_BRANCH_PATTERN.test(trimmed) ||
    trimmed.includes('..') ||
    trimmed.includes('//') ||
    trimmed.includes('@{') ||
    trimmed.startsWith('-') ||
    trimmed.endsWith('.') ||
    trimmed.endsWith('/') ||
    trimmed.endsWith('.lock')
  ) {
    throw new PackCRepoWorktreeIdentityError(
      'Branch refs must be parseable safe Git refs.',
    )
  }

  return trimmed
}

const safeShaRef = (field: string, value: string | null): string | null => {
  if (value === null) {
    return null
  }

  const trimmed = safeRef(field, value)

  if (!SHA_REF_PATTERN.test(trimmed)) {
    throw new PackCRepoWorktreeIdentityError(
      `${field} must be a safe commit or digest ref.`,
    )
  }

  return trimmed
}

const blockerRefs = (
  input: PackCRepoWorktreeIdentityInput,
  repository: PackCRepositoryIdentityProjection,
  worktree: PackCWorktreeIdentityProjection,
): ReadonlyArray<string> => [
  ...(repository.defaultBranch === null
    ? [
        `pack-c-identity-blocker:${repository.repositoryRef}:missing-default-branch`,
      ]
    : []),
  ...(repository.pinnedCommitRef === null
    ? [
        `pack-c-identity-blocker:${repository.repositoryRef}:missing-pinned-commit`,
      ]
    : []),
  ...(repository.remoteDigestRef === null
    ? [
        `pack-c-identity-blocker:${repository.repositoryRef}:missing-remote-digest`,
      ]
    : []),
  ...(repository.dataScopeRefs.length === 0
    ? [`pack-c-identity-blocker:${repository.repositoryRef}:missing-data-scope`]
    : []),
  ...(worktree.baseCommitRef === null
    ? [`pack-c-identity-blocker:${worktree.worktreeRef}:missing-base-commit`]
    : []),
  ...(worktree.headCommitRef === null
    ? [`pack-c-identity-blocker:${worktree.worktreeRef}:missing-head-commit`]
    : []),
  ...(worktree.sandboxProfileRef === null
    ? [
        `pack-c-identity-blocker:${worktree.worktreeRef}:missing-sandbox-profile`,
      ]
    : []),
  ...(worktree.retentionPolicyRef === null
    ? [
        `pack-c-identity-blocker:${worktree.worktreeRef}:missing-retention-policy`,
      ]
    : []),
  ...(worktree.cleanliness === 'unknown'
    ? [`pack-c-identity-blocker:${worktree.worktreeRef}:unknown-cleanliness`]
    : []),
]

const ageMs = (generatedAt: string, observedAt: string): number =>
  Math.max(0, Date.parse(generatedAt) - Date.parse(observedAt))

const staleAt = (observedAt: string, staleAfterMs: number): string =>
  isoTimestampAfterIso(observedAt, staleAfterMs)

export const projectPackCRepoWorktreeIdentity = (
  input: PackCRepoWorktreeIdentityInput,
): PackCRepoWorktreeIdentityProjection => {
  const repository: PackCRepositoryIdentityProjection = {
    caveatRefs: safeRefs(
      'pack-c-repo-identity.caveatRefs',
      input.repository.caveatRefs,
    ),
    dataScopeRefs: safeRefs(
      'pack-c-repo-identity.dataScopeRefs',
      input.repository.dataScopeRefs,
    ),
    defaultBranch:
      input.repository.defaultBranch === null
        ? null
        : normalizePackCBranchRef(input.repository.defaultBranch),
    host: safeHost(input.repository.host),
    name: safeRepositoryPart(
      'pack-c-repo-identity.name',
      input.repository.name,
    ),
    owner: safeRepositoryPart(
      'pack-c-repo-identity.owner',
      input.repository.owner,
    ),
    pinnedCommitRef: safeShaRef(
      'pack-c-repo-identity.pinnedCommitRef',
      input.repository.pinnedCommitRef,
    ),
    remoteDigestRef: safeShaRef(
      'pack-c-repo-identity.remoteDigestRef',
      input.repository.remoteDigestRef,
    ),
    repositoryRef: safeRef(
      'pack-c-repo-identity.repositoryRef',
      input.repository.repositoryRef,
    ),
    trustTier: input.repository.trustTier,
    visibility: input.repository.visibility,
  }
  const worktree: PackCWorktreeIdentityProjection = {
    baseCommitRef: safeShaRef(
      'pack-c-worktree-identity.baseCommitRef',
      input.worktree.baseCommitRef,
    ),
    branchRef: normalizePackCBranchRef(input.worktree.branchRef),
    cleanliness: input.worktree.cleanliness,
    headCommitRef: safeShaRef(
      'pack-c-worktree-identity.headCommitRef',
      input.worktree.headCommitRef,
    ),
    retentionPolicyRef:
      input.worktree.retentionPolicyRef === null
        ? null
        : safeRef(
            'pack-c-worktree-identity.retentionPolicyRef',
            input.worktree.retentionPolicyRef,
          ),
    sandboxProfileRef:
      input.worktree.sandboxProfileRef === null
        ? null
        : safeRef(
            'pack-c-worktree-identity.sandboxProfileRef',
            input.worktree.sandboxProfileRef,
          ),
    worktreeRef: safeRef(
      'pack-c-worktree-identity.worktreeRef',
      input.worktree.worktreeRef,
    ),
    workspaceRef: safeRef(
      'pack-c-worktree-identity.workspaceRef',
      input.worktree.workspaceRef,
    ),
  }
  const observedAgeMs = ageMs(input.generatedAt, input.observedAt)
  const freshness =
    Number.isFinite(observedAgeMs) && observedAgeMs <= input.staleAfterMs
      ? 'fresh'
      : 'stale'
  const blockers = blockerRefs(input, repository, worktree)
  const projection: PackCRepoWorktreeIdentityProjection = {
    ageMs: observedAgeMs,
    blockerRefs: blockers,
    freshness,
    generatedAt: input.generatedAt,
    identityVersion: PACK_C_REPO_WORKTREE_IDENTITY_VERSION,
    observedAt: input.observedAt,
    repository,
    staleAt: staleAt(input.observedAt, input.staleAfterMs),
    status:
      blockers.length > 0
        ? 'blocked'
        : freshness === 'stale'
          ? 'stale'
          : 'ready',
    worktree,
  }

  assertNoPrivateMaterial(projection, PACK_C_REPO_WORKTREE_IDENTITY_COLLECTION)

  return projection
}
