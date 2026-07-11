import {
  CODING_NAVIGATION_ENTITY_TYPE,
  CODING_PROJECT_ENTITY_TYPE,
  CODING_REPOSITORY_ENTITY_TYPE,
  CODING_SESSION_ENTITY_TYPE,
  CODING_WORKTREE_ENTITY_TYPE,
  CodingOwnerScopeRef,
  EntityId,
  EntityType,
  SyncScope,
  canonicalJson,
  decodeCodingNavigationEntity,
  decodeCodingProjectEntity,
  decodeCodingRepositoryEntity,
  decodeCodingSessionEntity,
  decodeCodingWorktreeEntity,
  validateCodingSessionCatalog,
  type ChangelogEntry,
  type CodingNavigationEntity,
  type CodingProjectEntity,
  type CodingRepositoryEntity,
  type CodingSessionEntity,
  type CodingWorktreeEntity,
} from "@openagentsinc/khala-sync"

import { withSyncTransaction, type SyncTransactionWriter } from "./outbox-writer.js"
import type { SyncSql } from "./sql.js"

export const CODING_SESSION_PROJECTION_SYSTEM_REF =
  "system:coding_session_projection.catalog.v1"

const forbiddenPrivateMaterial =
  /"(?:token|apiKey|authorization|sessionToken|refreshToken|mnemonic|secret|localPath|hostname|processId|providerSessionId|transportHandle)"\s*:|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|[A-Za-z]:\\Users\\)|[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/i

export class CodingCatalogProjectionError extends Error {
  readonly _tag = "CodingCatalogProjectionError"
  override readonly name = "CodingCatalogProjectionError"
}

export type CodingCatalogChangeSet = Readonly<{
  ownerScopeRef: string
  projects: ReadonlyArray<CodingProjectEntity>
  repositories: ReadonlyArray<CodingRepositoryEntity>
  worktrees: ReadonlyArray<CodingWorktreeEntity>
  sessions: ReadonlyArray<CodingSessionEntity>
  navigation: CodingNavigationEntity | null
}>

const boundedArray = <A>(
  raw: unknown,
  max: number,
  decode: (value: unknown) => A,
  label: string,
): ReadonlyArray<A> => {
  if (!Array.isArray(raw) || raw.length > max) {
    throw new CodingCatalogProjectionError(`${label} must be a bounded array`)
  }
  return raw.map(decode)
}

export const decodeCodingCatalogChangeSet = (raw: unknown): CodingCatalogChangeSet => {
  if (forbiddenPrivateMaterial.test(canonicalJson(raw))) {
    throw new CodingCatalogProjectionError("coding catalog contains forbidden private material")
  }
  if (raw === null || typeof raw !== "object") {
    throw new CodingCatalogProjectionError("coding catalog change set must be an object")
  }
  const value = raw as Record<string, unknown>
  const ownerScopeRef = String(CodingOwnerScopeRef.make(String(value.ownerScopeRef)))
  const projects = boundedArray(value.projects, 128, decodeCodingProjectEntity, "projects")
  const repositories = boundedArray(value.repositories, 512, decodeCodingRepositoryEntity, "repositories")
  const worktrees = boundedArray(value.worktrees, 1_024, decodeCodingWorktreeEntity, "worktrees")
  const sessions = boundedArray(value.sessions, 2_048, decodeCodingSessionEntity, "sessions")
  const navigation = value.navigation === null || value.navigation === undefined
    ? null
    : decodeCodingNavigationEntity(value.navigation)
  if (projects.length + repositories.length + worktrees.length + sessions.length +
    Number(navigation !== null) === 0) {
    throw new CodingCatalogProjectionError("coding catalog change set is empty")
  }
  for (const entity of [...projects, ...repositories, ...worktrees, ...sessions]) {
    if (entity.ownerScopeRef !== ownerScopeRef) {
      throw new CodingCatalogProjectionError("coding catalog owner scope mismatch")
    }
  }
  if (navigation !== null && navigation.ownerScopeRef !== ownerScopeRef) {
    throw new CodingCatalogProjectionError("coding navigation owner scope mismatch")
  }
  const issues = validateCodingSessionCatalog({ projects, repositories, worktrees, sessions })
  if (issues.length > 0) {
    throw new CodingCatalogProjectionError(`coding catalog relation invalid: ${issues[0]!.code}`)
  }
  return { ownerScopeRef, projects, repositories, worktrees, sessions, navigation }
}

const append = (
  writer: SyncTransactionWriter,
  scope: SyncScope,
  entityType: string,
  entityId: string,
  postImage: unknown,
  mutationRef: string,
): Promise<ChangelogEntry> => writer.appendChange({
  scope,
  entityType: EntityType.make(entityType),
  entityId: EntityId.make(entityId),
  op: "upsert",
  postImage,
  mutationRef,
})

type CatalogChange = Readonly<{
  entityType: string
  entityId: string
  postImage: unknown
}>

/** Append a whole validated catalog change set at one dense owner-scope version. */
export const appendCodingCatalogChangeSet = async (
  writer: SyncTransactionWriter,
  raw: unknown,
  mutationRef: string,
): Promise<ReadonlyArray<ChangelogEntry>> => {
  const value = decodeCodingCatalogChangeSet(raw)
  const scope = SyncScope.make(value.ownerScopeRef)
  const changes: CatalogChange[] = [
    ...value.projects.map(entity => ({ entityType: CODING_PROJECT_ENTITY_TYPE, entityId: entity.projectRef, postImage: entity })),
    ...value.repositories.map(entity => ({ entityType: CODING_REPOSITORY_ENTITY_TYPE, entityId: entity.repositoryRef, postImage: entity })),
    ...value.worktrees.map(entity => ({ entityType: CODING_WORKTREE_ENTITY_TYPE, entityId: entity.worktreeRef, postImage: entity })),
    ...value.sessions.map(entity => ({ entityType: CODING_SESSION_ENTITY_TYPE, entityId: entity.sessionRef, postImage: entity })),
    ...(value.navigation === null ? [] : [{
      entityType: CODING_NAVIGATION_ENTITY_TYPE,
      entityId: value.navigation.navigationRef,
      postImage: value.navigation,
    }]),
  ]
  const entries: ChangelogEntry[] = []
  // The transaction writer's first append allocates the dense scope version.
  // Keep calls sequential so concurrent first-use cannot race that allocation.
  for (const change of changes) {
    entries.push(await append(
      writer,
      scope,
      change.entityType,
      change.entityId,
      change.postImage,
      mutationRef,
    ))
  }
  return entries
}

export type CodingCatalogProjectionOutcome =
  | Readonly<{ ok: true; entries: ReadonlyArray<ChangelogEntry> }>
  | Readonly<{ ok: false; reason: "invalid" | "storage_failed"; messageSafe: string }>

export const projectCodingCatalogBestEffort = async (
  sql: SyncSql,
  raw: unknown,
): Promise<CodingCatalogProjectionOutcome> => {
  let decoded: CodingCatalogChangeSet
  try {
    decoded = decodeCodingCatalogChangeSet(raw)
  } catch (error) {
    return {
      ok: false,
      reason: "invalid",
      messageSafe: error instanceof CodingCatalogProjectionError
        ? error.message
        : "coding catalog validation failed",
    }
  }
  try {
    const entries = await withSyncTransaction(sql, writer => appendCodingCatalogChangeSet(
      writer,
      decoded,
      CODING_SESSION_PROJECTION_SYSTEM_REF,
    ))
    return { ok: true, entries }
  } catch {
    return { ok: false, reason: "storage_failed", messageSafe: "coding catalog projection failed" }
  }
}
