import { Schema as S } from "effect"

/**
 * Provider-neutral project/repository/coding-session catalog (CUT-13).
 *
 * These are durable product identities, not placement identities. No ref may
 * contain a local path, hostname, process id, provider session id, credential,
 * or transport handle. A local checkout may resolve an opaque alias ref to a
 * worktree, but the alias never becomes worktree or session authority.
 */

export const CODING_PROJECT_ENTITY_TYPE = "coding_project"
export const CODING_REPOSITORY_ENTITY_TYPE = "coding_repository"
export const CODING_WORKTREE_ENTITY_TYPE = "coding_worktree"
export const CODING_SESSION_ENTITY_TYPE = "coding_session"
export const CODING_NAVIGATION_ENTITY_TYPE = "coding_navigation"

export const CodingCatalogSchema = S.Literal("openagents.coding_catalog.v1")
export type CodingCatalogSchema = typeof CodingCatalogSchema.Type

export const CodingRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
export type CodingRef = typeof CodingRef.Type

export const CodingOwnerScopeRef = S.String.check(
  S.isMaxLength(320),
  S.isPattern(/^scope\.(user|team)\.[A-Za-z0-9._:-]+$/),
)
export type CodingOwnerScopeRef = typeof CodingOwnerScopeRef.Type

export const CodingIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)
export type CodingIsoTimestamp = typeof CodingIsoTimestamp.Type

export const CodingDisplayName = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(160),
)
export type CodingDisplayName = typeof CodingDisplayName.Type

const boundedAliases = S.Array(CodingRef).check(S.isMaxLength(32))
const boundedSessionRefs = S.Array(CodingRef).check(S.isMaxLength(64))
const boundedActivityCursors = S.Array(S.Struct({
  threadRef: CodingRef,
  cursor: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
})).check(S.isMaxLength(2_000))

export const CodingGrantState = S.Union([
  S.Struct({ state: S.Literal("granted"), grantRef: CodingRef }),
  S.Struct({
    state: S.Literal("revoked"),
    grantRef: CodingRef,
    revokedAt: CodingIsoTimestamp,
  }),
  S.Struct({
    state: S.Literal("unavailable"),
    reason: S.Literals(["not_required", "not_projected", "unsupported"]),
  }),
])
export type CodingGrantState = typeof CodingGrantState.Type

export const CodingAvailability = S.Union([
  S.Struct({ state: S.Literal("available") }),
  S.Struct({
    state: S.Literal("missing"),
    reason: S.Literals(["not_found", "deleted", "host_unavailable"]),
  }),
])
export type CodingAvailability = typeof CodingAvailability.Type

export class CodingProjectEntity extends S.Class<CodingProjectEntity>(
  "CodingProjectEntity",
)({
  schema: CodingCatalogSchema,
  projectRef: CodingRef,
  ownerScopeRef: CodingOwnerScopeRef,
  displayName: CodingDisplayName,
  aliasRefs: boundedAliases,
  state: S.Literals(["active", "archived"]),
  createdAt: CodingIsoTimestamp,
  updatedAt: CodingIsoTimestamp,
  archivedAt: S.NullOr(CodingIsoTimestamp),
}) {}

export class CodingRepositoryEntity extends S.Class<CodingRepositoryEntity>(
  "CodingRepositoryEntity",
)({
  schema: CodingCatalogSchema,
  repositoryRef: CodingRef,
  projectRef: CodingRef,
  ownerScopeRef: CodingOwnerScopeRef,
  displayName: CodingDisplayName,
  /** Former names and source aliases resolve to this stable ref. Never paths. */
  aliasRefs: boundedAliases,
  pinnedBaseRef: CodingRef,
  availability: CodingAvailability,
  grant: CodingGrantState,
  createdAt: CodingIsoTimestamp,
  updatedAt: CodingIsoTimestamp,
}) {}

export class CodingWorktreeEntity extends S.Class<CodingWorktreeEntity>(
  "CodingWorktreeEntity",
)({
  schema: CodingCatalogSchema,
  worktreeRef: CodingRef,
  repositoryRef: CodingRef,
  projectRef: CodingRef,
  ownerScopeRef: CodingOwnerScopeRef,
  displayName: CodingDisplayName,
  /** Opaque device/host alias refs; raw filesystem paths are structurally absent. */
  aliasRefs: boundedAliases,
  baseRef: CodingRef,
  availability: CodingAvailability,
  grant: CodingGrantState,
  createdAt: CodingIsoTimestamp,
  updatedAt: CodingIsoTimestamp,
}) {}

export const CodingProviderFact = S.Union([
  S.Struct({ state: S.Literal("known"), providerRef: CodingRef }),
  S.Struct({
    state: S.Literal("unavailable"),
    reason: S.Literals(["not_selected", "not_projected", "unsupported"]),
  }),
])
export type CodingProviderFact = typeof CodingProviderFact.Type

export const CodingRuntimeFact = S.Union([
  S.Struct({ state: S.Literal("known"), runtimeRef: CodingRef }),
  S.Struct({
    state: S.Literal("unavailable"),
    reason: S.Literals(["not_attached", "not_projected", "unsupported"]),
  }),
])
export type CodingRuntimeFact = typeof CodingRuntimeFact.Type

export class CodingSessionEntity extends S.Class<CodingSessionEntity>(
  "CodingSessionEntity",
)({
  schema: CodingCatalogSchema,
  sessionRef: CodingRef,
  ownerScopeRef: CodingOwnerScopeRef,
  projectRef: CodingRef,
  repositoryRef: CodingRef,
  worktreeRef: CodingRef,
  workContextRef: CodingRef,
  threadRef: CodingRef,
  conversationRef: CodingRef,
  runRef: S.NullOr(CodingRef),
  fleetRef: S.NullOr(CodingRef),
  currentAttachmentRef: S.NullOr(CodingRef),
  currentCheckpointRef: S.NullOr(CodingRef),
  agentTopologyRef: S.NullOr(CodingRef),
  canonicalEventCursor: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  activityCursors: boundedActivityCursors,
  provider: CodingProviderFact,
  runtime: CodingRuntimeFact,
  grant: CodingGrantState,
  state: S.Literals(["active", "idle", "recovery_required", "archived"]),
  createdAt: CodingIsoTimestamp,
  updatedAt: CodingIsoTimestamp,
  lastActiveAt: CodingIsoTimestamp,
  archivedAt: S.NullOr(CodingIsoTimestamp),
}) {}

export const CodingNavigationFocus = S.Union([
  S.Struct({ kind: S.Literal("none") }),
  S.Struct({ kind: S.Literal("conversation"), conversationRef: CodingRef }),
  S.Struct({ kind: S.Literal("editor"), artifactRef: CodingRef }),
  S.Struct({ kind: S.Literal("terminal"), terminalRef: CodingRef }),
  S.Struct({ kind: S.Literal("agent"), agentRef: CodingRef }),
])
export type CodingNavigationFocus = typeof CodingNavigationFocus.Type

export class CodingNavigationEntity extends S.Class<CodingNavigationEntity>(
  "CodingNavigationEntity",
)({
  schema: CodingCatalogSchema,
  navigationRef: CodingRef,
  ownerScopeRef: CodingOwnerScopeRef,
  selectedProjectRef: S.NullOr(CodingRef),
  selectedRepositoryRef: S.NullOr(CodingRef),
  selectedWorktreeRef: S.NullOr(CodingRef),
  selectedSessionRef: S.NullOr(CodingRef),
  openSessionRefs: boundedSessionRefs,
  focus: CodingNavigationFocus,
  updatedAt: CodingIsoTimestamp,
}) {}

export const decodeCodingProjectEntity = S.decodeUnknownSync(CodingProjectEntity)
export const decodeCodingRepositoryEntity = S.decodeUnknownSync(CodingRepositoryEntity)
export const decodeCodingWorktreeEntity = S.decodeUnknownSync(CodingWorktreeEntity)
export const decodeCodingSessionEntity = S.decodeUnknownSync(CodingSessionEntity)
export const decodeCodingNavigationEntity = S.decodeUnknownSync(CodingNavigationEntity)

export const encodeCodingProjectEntity = S.encodeSync(CodingProjectEntity)
export const encodeCodingRepositoryEntity = S.encodeSync(CodingRepositoryEntity)
export const encodeCodingWorktreeEntity = S.encodeSync(CodingWorktreeEntity)
export const encodeCodingSessionEntity = S.encodeSync(CodingSessionEntity)
export const encodeCodingNavigationEntity = S.encodeSync(CodingNavigationEntity)

export type CodingSessionCatalog = Readonly<{
  projects: ReadonlyArray<CodingProjectEntity>
  repositories: ReadonlyArray<CodingRepositoryEntity>
  worktrees: ReadonlyArray<CodingWorktreeEntity>
  sessions: ReadonlyArray<CodingSessionEntity>
}>

export type CodingCatalogIssue = Readonly<{
  code:
    | "duplicate_ref"
    | "ambiguous_alias"
    | "owner_scope_mismatch"
    | "missing_project"
    | "missing_repository"
    | "missing_worktree"
    | "repository_project_mismatch"
    | "worktree_project_mismatch"
    | "worktree_repository_mismatch"
  affectedRef: string
}>

type RefEntity = Readonly<{ ref: string; aliases: ReadonlyArray<string> }>

const duplicateIssues = (
  kind: string,
  entities: ReadonlyArray<RefEntity>,
): CodingCatalogIssue[] => {
  const issues: CodingCatalogIssue[] = []
  const refs = new Set<string>()
  const refCounts = new Map<string, number>()
  const aliases = new Map<string, string>()
  for (const entity of entities) {
    refs.add(entity.ref)
    refCounts.set(entity.ref, (refCounts.get(entity.ref) ?? 0) + 1)
  }
  for (const [ref, count] of refCounts) {
    if (count > 1) issues.push({ code: "duplicate_ref", affectedRef: ref })
  }
  for (const entity of entities) {
    for (const alias of entity.aliases) {
      const prior = aliases.get(alias)
      if ((refs.has(alias) && alias !== entity.ref) || (prior !== undefined && prior !== entity.ref)) {
        issues.push({ code: "ambiguous_alias", affectedRef: `${kind}:${alias}` })
      } else {
        aliases.set(alias, entity.ref)
      }
    }
  }
  return issues
}

export const validateCodingSessionCatalog = (
  catalog: CodingSessionCatalog,
): ReadonlyArray<CodingCatalogIssue> => {
  const issues = [
    ...duplicateIssues("project", catalog.projects.map(value => ({ ref: value.projectRef, aliases: value.aliasRefs }))),
    ...duplicateIssues("repository", catalog.repositories.map(value => ({ ref: value.repositoryRef, aliases: value.aliasRefs }))),
    ...duplicateIssues("worktree", catalog.worktrees.map(value => ({ ref: value.worktreeRef, aliases: value.aliasRefs }))),
    ...duplicateIssues("session", catalog.sessions.map(value => ({ ref: value.sessionRef, aliases: [] }))),
  ]
  const projects = new Map(catalog.projects.map(value => [value.projectRef, value]))
  const repositories = new Map(catalog.repositories.map(value => [value.repositoryRef, value]))
  const worktrees = new Map(catalog.worktrees.map(value => [value.worktreeRef, value]))

  for (const repository of catalog.repositories) {
    const project = projects.get(repository.projectRef)
    if (project === undefined) issues.push({ code: "missing_project", affectedRef: repository.repositoryRef })
    else if (project.ownerScopeRef !== repository.ownerScopeRef) {
      issues.push({ code: "owner_scope_mismatch", affectedRef: repository.repositoryRef })
    }
  }
  for (const worktree of catalog.worktrees) {
    const project = projects.get(worktree.projectRef)
    const repository = repositories.get(worktree.repositoryRef)
    if (project === undefined) issues.push({ code: "missing_project", affectedRef: worktree.worktreeRef })
    if (repository === undefined) issues.push({ code: "missing_repository", affectedRef: worktree.worktreeRef })
    if (project !== undefined && project.ownerScopeRef !== worktree.ownerScopeRef) {
      issues.push({ code: "owner_scope_mismatch", affectedRef: worktree.worktreeRef })
    }
    if (repository !== undefined && repository.projectRef !== worktree.projectRef) {
      issues.push({ code: "worktree_project_mismatch", affectedRef: worktree.worktreeRef })
    }
  }
  for (const session of catalog.sessions) {
    const project = projects.get(session.projectRef)
    const repository = repositories.get(session.repositoryRef)
    const worktree = worktrees.get(session.worktreeRef)
    if (project === undefined) issues.push({ code: "missing_project", affectedRef: session.sessionRef })
    if (repository === undefined) issues.push({ code: "missing_repository", affectedRef: session.sessionRef })
    if (worktree === undefined) issues.push({ code: "missing_worktree", affectedRef: session.sessionRef })
    if (project !== undefined && project.ownerScopeRef !== session.ownerScopeRef) {
      issues.push({ code: "owner_scope_mismatch", affectedRef: session.sessionRef })
    }
    if (repository !== undefined && repository.projectRef !== session.projectRef) {
      issues.push({ code: "repository_project_mismatch", affectedRef: session.sessionRef })
    }
    if (worktree !== undefined && worktree.projectRef !== session.projectRef) {
      issues.push({ code: "worktree_project_mismatch", affectedRef: session.sessionRef })
    }
    if (worktree !== undefined && worktree.repositoryRef !== session.repositoryRef) {
      issues.push({ code: "worktree_repository_mismatch", affectedRef: session.sessionRef })
    }
  }
  return issues
}

const resolveAlias = <T>(
  requested: string,
  entities: ReadonlyArray<T>,
  ref: (value: T) => string,
  aliases: (value: T) => ReadonlyArray<string>,
): { state: "found"; value: T } | { state: "missing" | "ambiguous" } => {
  const matches = entities.filter(value => ref(value) === requested || aliases(value).includes(requested))
  return matches.length === 1
    ? { state: "found", value: matches[0]! }
    : { state: matches.length === 0 ? "missing" : "ambiguous" }
}

export type CodingNavigationRecoveryReason =
  | "ambiguous_alias"
  | "archived"
  | "catalog_invalid"
  | "grant_revoked"
  | "grant_unavailable"
  | "missing_repository"
  | "missing_session"
  | "missing_worktree"
  | "owner_scope_mismatch"

export type CodingNavigationResolution =
  | Readonly<{ state: "empty"; navigation: CodingNavigationEntity }>
  | Readonly<{
      state: "ready"
      navigation: CodingNavigationEntity
      project: CodingProjectEntity
      repository: CodingRepositoryEntity
      worktree: CodingWorktreeEntity
      session: CodingSessionEntity
    }>
  | Readonly<{
      state: "recovery_required"
      reason: CodingNavigationRecoveryReason
      affectedRef: string
      navigation: CodingNavigationEntity
    }>

const grantRevoked = (value: CodingGrantState): boolean => value.state === "revoked"
const grantUnavailable = (value: CodingGrantState): boolean =>
  value.state === "unavailable" && value.reason !== "not_required"

/**
 * Restores one navigation snapshot by stable identity. Alias resolution is
 * bounded and fail-closed; duplicate tabs collapse to one canonical session.
 */
export const resolveCodingNavigation = (
  catalog: CodingSessionCatalog,
  navigation: CodingNavigationEntity,
): CodingNavigationResolution => {
  const issues = validateCodingSessionCatalog(catalog)
  if (issues.length > 0) {
    return {
      state: "recovery_required",
      reason: issues.some(issue => issue.code === "ambiguous_alias")
        ? "ambiguous_alias"
        : "catalog_invalid",
      affectedRef: issues[0]!.affectedRef,
      navigation,
    }
  }
  const canonicalOpenRefs: string[] = []
  for (const requested of navigation.openSessionRefs) {
    const resolved = resolveAlias(requested, catalog.sessions, value => value.sessionRef, () => [])
    if (resolved.state === "found" && !canonicalOpenRefs.includes(resolved.value.sessionRef)) {
      canonicalOpenRefs.push(resolved.value.sessionRef)
    }
  }
  const normalized = new CodingNavigationEntity({
    ...navigation,
    openSessionRefs: canonicalOpenRefs,
  })
  if (navigation.selectedSessionRef === null) return { state: "empty", navigation: normalized }

  const selected = resolveAlias(
    navigation.selectedSessionRef,
    catalog.sessions,
    value => value.sessionRef,
    () => [],
  )
  if (selected.state !== "found") {
    return {
      state: "recovery_required",
      reason: selected.state === "ambiguous" ? "ambiguous_alias" : "missing_session",
      affectedRef: navigation.selectedSessionRef,
      navigation: normalized,
    }
  }
  const session = selected.value
  if (session.ownerScopeRef !== navigation.ownerScopeRef) {
    return {
      state: "recovery_required",
      reason: "owner_scope_mismatch",
      affectedRef: session.sessionRef,
      navigation: normalized,
    }
  }
  const project = resolveAlias(session.projectRef, catalog.projects, value => value.projectRef, value => value.aliasRefs)
  const repository = resolveAlias(session.repositoryRef, catalog.repositories, value => value.repositoryRef, value => value.aliasRefs)
  const worktree = resolveAlias(session.worktreeRef, catalog.worktrees, value => value.worktreeRef, value => value.aliasRefs)
  if (project.state !== "found" || repository.state !== "found" || worktree.state !== "found") {
    const reason = repository.state !== "found" ? "missing_repository" : "missing_worktree"
    return { state: "recovery_required", reason, affectedRef: session.sessionRef, navigation: normalized }
  }
  if (session.state === "archived" || project.value.state === "archived") {
    return { state: "recovery_required", reason: "archived", affectedRef: session.sessionRef, navigation: normalized }
  }
  if (
    grantRevoked(session.grant) ||
    grantRevoked(repository.value.grant) ||
    grantRevoked(worktree.value.grant)
  ) {
    return { state: "recovery_required", reason: "grant_revoked", affectedRef: session.sessionRef, navigation: normalized }
  }
  if (
    grantUnavailable(session.grant) ||
    grantUnavailable(repository.value.grant) ||
    grantUnavailable(worktree.value.grant)
  ) {
    return { state: "recovery_required", reason: "grant_unavailable", affectedRef: session.sessionRef, navigation: normalized }
  }
  if (repository.value.availability.state === "missing") {
    return { state: "recovery_required", reason: "missing_repository", affectedRef: repository.value.repositoryRef, navigation: normalized }
  }
  if (worktree.value.availability.state === "missing") {
    return { state: "recovery_required", reason: "missing_worktree", affectedRef: worktree.value.worktreeRef, navigation: normalized }
  }
  return {
    state: "ready",
    navigation: new CodingNavigationEntity({
      ...normalized,
      selectedProjectRef: project.value.projectRef,
      selectedRepositoryRef: repository.value.repositoryRef,
      selectedWorktreeRef: worktree.value.worktreeRef,
      selectedSessionRef: session.sessionRef,
      openSessionRefs: canonicalOpenRefs.includes(session.sessionRef)
        ? canonicalOpenRefs
        : [session.sessionRef, ...canonicalOpenRefs],
    }),
    project: project.value,
    repository: repository.value,
    worktree: worktree.value,
    session,
  }
}

export type CodingSessionCatalogQuery = Readonly<{
  projectRef?: string
  repositoryRef?: string
  states?: ReadonlyArray<CodingSessionEntity["state"]>
  updatedAtOrAfter?: string
}>

/** Structured catalog filtering only. Semantic text search belongs upstream. */
export const queryCodingSessions = (
  catalog: CodingSessionCatalog,
  query: CodingSessionCatalogQuery,
): ReadonlyArray<CodingSessionEntity> => catalog.sessions
  .filter(session => query.projectRef === undefined || session.projectRef === query.projectRef)
  .filter(session => query.repositoryRef === undefined || session.repositoryRef === query.repositoryRef)
  .filter(session => query.states === undefined || query.states.includes(session.state))
  .filter(session => query.updatedAtOrAfter === undefined || session.updatedAt >= query.updatedAtOrAfter)
  .sort((left, right) =>
    Number(left.state === "archived") - Number(right.state === "archived") ||
    right.lastActiveAt.localeCompare(left.lastActiveAt) ||
    left.sessionRef.localeCompare(right.sessionRef))
