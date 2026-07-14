import { chmodSync, existsSync, mkdirSync, readFileSync, realpathSync, renameSync, statSync, writeFileSync } from "node:fs"
import path from "node:path"

import {
  CODING_NAVIGATION_ENTITY_TYPE,
  CODING_PROJECT_ENTITY_TYPE,
  CODING_REPOSITORY_ENTITY_TYPE,
  CODING_SESSION_ENTITY_TYPE,
  CODING_WORKTREE_ENTITY_TYPE,
  CodingNavigationEntity,
  LocalRevision,
  deviceLocalScope,
  personalScope,
  decodeCodingNavigationEntity,
  decodeCodingProjectEntity,
  decodeCodingRepositoryEntity,
  decodeCodingSessionEntity,
  decodeCodingWorktreeEntity,
  encodeCodingNavigationEntity,
  encodeCodingProjectEntity,
  encodeCodingRepositoryEntity,
  encodeCodingSessionEntity,
  encodeCodingWorktreeEntity,
  queryCodingSessions,
  resolveCodingNavigation,
  validateCodingSessionCatalog,
  type CodingNavigationFocus,
  type CodingNavigationResolution,
  type CodingProjectEntity,
  type CodingRepositoryEntity,
  type CodingSessionCatalog,
  type CodingSessionCatalogQuery,
  type CodingSessionEntity,
  type CodingWorktreeEntity,
  type LocalIdentityRef,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import type { DesktopSyncStore } from "./desktop-sync-store.ts"

const schema = "openagents.coding_catalog.v1" as const
const navigationRef = "navigation.desktop.primary"

type Binding = Readonly<{
  worktreeRef: string
  root: string
}>

type BindingDocument = Readonly<{
  version: 1
  bindings: ReadonlyArray<Binding>
}>

export type DesktopCodingCatalogSnapshot = Readonly<{
  authority: "device_local"
  catalog: CodingSessionCatalog
  navigation: CodingNavigationEntity | null
  resolution: CodingNavigationResolution | null
  issues: ReturnType<typeof validateCodingSessionCatalog>
}>

/**
 * One durable identity admission for a user-selected repository checkout.
 *
 * The raw root is deliberately absent. It remains only in the private binding
 * file, while every product/runtime surface shares these opaque refs.
 */
export type DesktopWorkspaceAdmission = Readonly<{
  grantRef: string
  projectRef: string
  repositoryRef: string
  worktreeRef: string
  workContextRef: string
  sessionRef: string
}>

export type DesktopWorkspaceAdmissionResult = Readonly<{
  snapshot: DesktopCodingCatalogSnapshot
  admission: DesktopWorkspaceAdmission
}>

export type DesktopCodingCatalog = Readonly<{
  snapshot: () => DesktopCodingCatalogSnapshot
  selectWorkspace: (root: string) => DesktopCodingCatalogSnapshot
  admitWorkspace: (root: string) => DesktopWorkspaceAdmissionResult
  openSession: (sessionRef: string) => DesktopCodingCatalogSnapshot
  archiveSession: (sessionRef: string) => DesktopCodingCatalogSnapshot
  deleteSession: (sessionRef: string) => DesktopCodingCatalogSnapshot
  recoverSession: (sessionRef: string, root: string) => DesktopCodingCatalogSnapshot
  saveFocus: (sessionRef: string, focus: CodingNavigationFocus) => DesktopCodingCatalogSnapshot
  query: (query: CodingSessionCatalogQuery) => ReadonlyArray<CodingSessionEntity>
  selectedRoot: () => string | null
  ownerScopedChangeSet: (ownerUserId: string) => Readonly<{
    ownerScopeRef: string
    projects: ReadonlyArray<CodingProjectEntity>
    repositories: ReadonlyArray<CodingRepositoryEntity>
    worktrees: ReadonlyArray<CodingWorktreeEntity>
    sessions: ReadonlyArray<CodingSessionEntity>
    navigation: CodingNavigationEntity | null
  }>
}>

export const workspaceAdmissionFromSnapshot = (
  snapshot: DesktopCodingCatalogSnapshot,
): DesktopWorkspaceAdmission | null => {
  const resolved = snapshot.resolution
  if (resolved?.state !== "ready") return null
  const grants = [resolved.repository.grant, resolved.worktree.grant, resolved.session.grant]
  if (grants.some(grant => grant.state !== "granted")) return null
  const grantRefs = grants.map(grant => grant.state === "granted" ? grant.grantRef : null)
  const grantRef = grantRefs[0] ?? null
  if (grantRef === null || grantRefs.some(value => value !== grantRef)) return null
  return {
    grantRef,
    projectRef: resolved.project.projectRef,
    repositoryRef: resolved.repository.repositoryRef,
    worktreeRef: resolved.worktree.worktreeRef,
    workContextRef: resolved.session.workContextRef,
    sessionRef: resolved.session.sessionRef,
  }
}

const safeRefPart = (value: string): string => {
  const cleaned = value.replace(/[^A-Za-z0-9._:-]/g, "")
  return cleaned.length === 0 ? "generated" : cleaned.slice(0, 120)
}

const canonicalRoot = (root: string): string => {
  const resolved = realpathSync(root)
  if (!statSync(resolved).isDirectory()) throw new Error("coding workspace must be a directory")
  return resolved
}

const emptyCatalog = (): CodingSessionCatalog => ({ projects: [], repositories: [], worktrees: [], sessions: [] })

const readBindings = (file: string): Binding[] => {
  try {
    const raw = JSON.parse(readFileSync(file, "utf8")) as Partial<BindingDocument>
    if (raw.version !== 1 || !Array.isArray(raw.bindings)) return []
    return raw.bindings.filter((value): value is Binding =>
      typeof value === "object" && value !== null &&
      typeof (value as { worktreeRef?: unknown }).worktreeRef === "string" &&
      typeof (value as { root?: unknown }).root === "string").slice(0, 1_024)
  } catch {
    return []
  }
}

const writeBindings = (file: string, bindings: ReadonlyArray<Binding>): void => {
  mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(path.dirname(file), 0o700)
  const temporary = `${file}.tmp`
  writeFileSync(temporary, JSON.stringify({ version: 1, bindings }), { encoding: "utf8", mode: 0o600 })
  if (process.platform !== "win32") chmodSync(temporary, 0o600)
  renameSync(temporary, file)
}

export const openDesktopCodingCatalog = (input: Readonly<{
  store: DesktopSyncStore
  identityRef: LocalIdentityRef
  bindingFile: string
  randomId: () => string
  now?: () => string
}>): DesktopCodingCatalog => {
  const scope = deviceLocalScope(input.identityRef)
  const authorityRef = String(scope)
  const now = input.now ?? (() => new Date().toISOString())

  const rows = () => Effect.runSync(input.store.readLocalEntities(scope))
  const read = (): DesktopCodingCatalogSnapshot => {
    const projects: CodingProjectEntity[] = []
    const repositories: CodingRepositoryEntity[] = []
    const worktrees: CodingWorktreeEntity[] = []
    const sessions: CodingSessionEntity[] = []
    const navigations: Array<{ value: CodingNavigationEntity; revision: number }> = []
    for (const row of rows()) {
      try {
        const raw = JSON.parse(row.postImageJson)
        if (row.entityType === CODING_PROJECT_ENTITY_TYPE) {
          const value = decodeCodingProjectEntity(raw)
          if (value.projectRef === row.entityId && value.ownerScopeRef === authorityRef) projects.push(value)
        } else if (row.entityType === CODING_REPOSITORY_ENTITY_TYPE) {
          const value = decodeCodingRepositoryEntity(raw)
          if (value.repositoryRef === row.entityId && value.ownerScopeRef === authorityRef) repositories.push(value)
        } else if (row.entityType === CODING_WORKTREE_ENTITY_TYPE) {
          const value = decodeCodingWorktreeEntity(raw)
          if (value.worktreeRef === row.entityId && value.ownerScopeRef === authorityRef) worktrees.push(value)
        } else if (row.entityType === CODING_SESSION_ENTITY_TYPE) {
          const value = decodeCodingSessionEntity(raw)
          if (value.sessionRef === row.entityId && value.ownerScopeRef === authorityRef) sessions.push(value)
        } else if (row.entityType === CODING_NAVIGATION_ENTITY_TYPE) {
          const value = decodeCodingNavigationEntity(raw)
          if (value.navigationRef === row.entityId && value.ownerScopeRef === authorityRef) {
            navigations.push({ value, revision: Number(row.revision) })
          }
        }
      } catch {
        // A later local revision replaces malformed/pre-contract rows.
      }
    }
    const catalog = { projects, repositories, worktrees, sessions }
    const navigation = navigations.sort((left, right) => right.revision - left.revision)[0]?.value ?? null
    return {
      authority: "device_local",
      catalog,
      navigation,
      resolution: navigation === null ? null : resolveCodingNavigation(catalog, navigation),
      issues: validateCodingSessionCatalog(catalog),
    }
  }

  const persist = (inputRows: ReadonlyArray<Readonly<{
    entityType: string
    entityId: string
    postImageJson: string
  }>>): void => {
    const revision = LocalRevision.make(
      rows().reduce((max, row) => Math.max(max, Number(row.revision)), 0) + 1,
    )
    Effect.runSync(input.store.writeLocalEntities(scope, inputRows.map(row => ({ ...row, revision }))))
  }

  const persistNavigation = (
    catalog: CodingSessionCatalog,
    selectedSessionRef: string | null,
    focus: CodingNavigationFocus,
    openSessionRefs: ReadonlyArray<string>,
  ): void => {
    const selected = selectedSessionRef === null
      ? null
      : catalog.sessions.find(session => session.sessionRef === selectedSessionRef) ?? null
    const value = new CodingNavigationEntity({
      schema,
      navigationRef,
      ownerScopeRef: authorityRef,
      selectedProjectRef: selected?.projectRef ?? null,
      selectedRepositoryRef: selected?.repositoryRef ?? null,
      selectedWorktreeRef: selected?.worktreeRef ?? null,
      selectedSessionRef: selected?.sessionRef ?? null,
      openSessionRefs: [...new Set(openSessionRefs)].slice(0, 64),
      focus,
      updatedAt: now(),
    })
    persist([{
      entityType: CODING_NAVIGATION_ENTITY_TYPE,
      entityId: value.navigationRef,
      postImageJson: JSON.stringify(encodeCodingNavigationEntity(value)),
    }])
  }

  const refreshAvailability = (): void => {
    const current = read()
    const bindings = readBindings(input.bindingFile)
    const changed: Array<{ entityType: string; entityId: string; postImageJson: string }> = []
    for (const worktree of current.catalog.worktrees) {
      const binding = bindings.find(value => value.worktreeRef === worktree.worktreeRef)
      const isAvailable = binding !== undefined && existsSync(binding.root)
      const desired = isAvailable
        ? { state: "available" as const }
        : { state: "missing" as const, reason: "not_found" as const }
      if (JSON.stringify(worktree.availability) === JSON.stringify(desired)) continue
      const value = decodeCodingWorktreeEntity({ ...worktree, availability: desired, updatedAt: now() })
      changed.push({
        entityType: CODING_WORKTREE_ENTITY_TYPE,
        entityId: value.worktreeRef,
        postImageJson: JSON.stringify(encodeCodingWorktreeEntity(value)),
      })
    }
    if (changed.length > 0) persist(changed)
  }

  const snapshot = (): DesktopCodingCatalogSnapshot => {
    refreshAvailability()
    return read()
  }

  const selectWorkspaceSnapshot = (requestedRoot: string): DesktopCodingCatalogSnapshot => {
    const root = canonicalRoot(requestedRoot)
    const bindings = readBindings(input.bindingFile)
    const existingBinding = bindings.find(binding => binding.root === root)
    const current = read()
    if (existingBinding !== undefined) {
      const session = queryCodingSessions(current.catalog, { states: ["active", "idle"] })
        .find(value => value.worktreeRef === existingBinding.worktreeRef)
      if (session !== undefined) {
        // Upgrade the retained pre-admission catalog shape exactly once. Only
        // the former local `not_required` state is eligible: revoked,
        // unavailable, or conflicting grants remain fail-closed.
        const repository = current.catalog.repositories.find(value =>
          value.repositoryRef === session.repositoryRef)
        const worktree = current.catalog.worktrees.find(value =>
          value.worktreeRef === session.worktreeRef)
        if (
          repository?.grant.state === "unavailable" && repository.grant.reason === "not_required" &&
          worktree?.grant.state === "unavailable" && worktree.grant.reason === "not_required" &&
          session.grant.state === "unavailable" && session.grant.reason === "not_required"
        ) {
          const grant = {
            state: "granted" as const,
            grantRef: `workspace.grant.desktop.${safeRefPart(session.sessionRef)}`,
          }
          const updatedAt = now()
          const admittedRepository = decodeCodingRepositoryEntity({ ...repository, grant, updatedAt })
          const admittedWorktree = decodeCodingWorktreeEntity({ ...worktree, grant, updatedAt })
          const admittedSession = decodeCodingSessionEntity({ ...session, grant, updatedAt })
          persist([
            { entityType: CODING_REPOSITORY_ENTITY_TYPE, entityId: admittedRepository.repositoryRef, postImageJson: JSON.stringify(encodeCodingRepositoryEntity(admittedRepository)) },
            { entityType: CODING_WORKTREE_ENTITY_TYPE, entityId: admittedWorktree.worktreeRef, postImageJson: JSON.stringify(encodeCodingWorktreeEntity(admittedWorktree)) },
            { entityType: CODING_SESSION_ENTITY_TYPE, entityId: admittedSession.sessionRef, postImageJson: JSON.stringify(encodeCodingSessionEntity(admittedSession)) },
          ])
        }
        const admitted = read()
        const admittedSession = admitted.catalog.sessions.find(value =>
          value.sessionRef === session.sessionRef) ?? session
        persistNavigation(
          admitted.catalog,
          admittedSession.sessionRef,
          { kind: "conversation", conversationRef: admittedSession.conversationRef },
          [admittedSession.sessionRef, ...(admitted.navigation?.openSessionRefs ?? [])],
        )
        return snapshot()
      }
    }

    const id = safeRefPart(input.randomId())
    const createdAt = now()
    const displayName = path.basename(root) || "Workspace"
    const projectRef = `project.desktop.${id}`
    const repositoryRef = `repository.desktop.${id}`
    const worktreeRef = `worktree.desktop.${id}`
    const sessionRef = `session.desktop.${id}`
    const workContextRef = `work-context.desktop.${id}`
    const grantRef = `workspace.grant.desktop.${id}`
    const grant = { state: "granted" as const, grantRef }
    const project = decodeCodingProjectEntity({ schema, projectRef, ownerScopeRef: authorityRef, displayName, aliasRefs: [], state: "active", createdAt, updatedAt: createdAt, archivedAt: null })
    const repository = decodeCodingRepositoryEntity({ schema, repositoryRef, projectRef, ownerScopeRef: authorityRef, displayName, aliasRefs: [], pinnedBaseRef: `base.desktop.${id}`, availability: { state: "available" }, grant, createdAt, updatedAt: createdAt })
    const worktree = decodeCodingWorktreeEntity({ schema, worktreeRef, repositoryRef, projectRef, ownerScopeRef: authorityRef, displayName, aliasRefs: [], baseRef: repository.pinnedBaseRef, availability: { state: "available" }, grant, createdAt, updatedAt: createdAt })
    const session = decodeCodingSessionEntity({ schema, sessionRef, ownerScopeRef: authorityRef, projectRef, repositoryRef, worktreeRef, workContextRef, threadRef: `thread.desktop.${id}`, conversationRef: `conversation.desktop.${id}`, runRef: null, fleetRef: null, currentAttachmentRef: null, currentCheckpointRef: null, agentTopologyRef: null, canonicalEventCursor: 0, activityCursors: [], provider: { state: "unavailable", reason: "not_selected" }, runtime: { state: "unavailable", reason: "not_attached" }, grant, state: "active", createdAt, updatedAt: createdAt, lastActiveAt: createdAt, archivedAt: null })
    persist([
      { entityType: CODING_PROJECT_ENTITY_TYPE, entityId: projectRef, postImageJson: JSON.stringify(encodeCodingProjectEntity(project)) },
      { entityType: CODING_REPOSITORY_ENTITY_TYPE, entityId: repositoryRef, postImageJson: JSON.stringify(encodeCodingRepositoryEntity(repository)) },
      { entityType: CODING_WORKTREE_ENTITY_TYPE, entityId: worktreeRef, postImageJson: JSON.stringify(encodeCodingWorktreeEntity(worktree)) },
      { entityType: CODING_SESSION_ENTITY_TYPE, entityId: sessionRef, postImageJson: JSON.stringify(encodeCodingSessionEntity(session)) },
    ])
    writeBindings(input.bindingFile, [
      ...bindings.filter(binding => binding.worktreeRef !== worktreeRef && binding.root !== root),
      { worktreeRef, root },
    ].slice(-1_024))
    const next = read()
    persistNavigation(
      next.catalog,
      sessionRef,
      { kind: "conversation", conversationRef: session.conversationRef },
      [sessionRef, ...(current.navigation?.openSessionRefs ?? [])],
    )
    return snapshot()
  }

  const admitWorkspace = (root: string): DesktopWorkspaceAdmissionResult => {
    const admitted = selectWorkspaceSnapshot(root)
    const admission = workspaceAdmissionFromSnapshot(admitted)
    if (admission === null) throw new Error("selected workspace identity admission is inconsistent")
    return { snapshot: admitted, admission }
  }

  const selectWorkspace = (root: string): DesktopCodingCatalogSnapshot =>
    admitWorkspace(root).snapshot

  const openSession = (sessionRef: string): DesktopCodingCatalogSnapshot => {
    const current = snapshot()
    const selected = current.catalog.sessions.find(value => value.sessionRef === sessionRef)
    if (selected === undefined) return current
    persistNavigation(
      current.catalog,
      selected.sessionRef,
      current.navigation?.selectedSessionRef === selected.sessionRef
        ? current.navigation.focus
        : { kind: "conversation", conversationRef: selected.conversationRef },
      [selected.sessionRef, ...(current.navigation?.openSessionRefs ?? [])],
    )
    return snapshot()
  }

  const archiveSession = (sessionRef: string): DesktopCodingCatalogSnapshot => {
    const current = snapshot()
    const selected = current.catalog.sessions.find(value => value.sessionRef === sessionRef)
    if (selected === undefined || selected.state === "archived") return current
    const archived = decodeCodingSessionEntity({
      ...selected,
      state: "archived",
      archivedAt: now(),
      updatedAt: now(),
    })
    persist([{ entityType: CODING_SESSION_ENTITY_TYPE, entityId: sessionRef, postImageJson: JSON.stringify(encodeCodingSessionEntity(archived)) }])
    const next = read()
    const recent = queryCodingSessions(next.catalog, { states: ["active", "idle"] })[0] ?? null
    persistNavigation(
      next.catalog,
      recent?.sessionRef ?? null,
      recent === null ? { kind: "none" } : { kind: "conversation", conversationRef: recent.conversationRef },
      (current.navigation?.openSessionRefs ?? []).filter(value => value !== sessionRef),
    )
    return snapshot()
  }

  const deleteSession = (sessionRef: string): DesktopCodingCatalogSnapshot => {
    const current = snapshot()
    const selected = current.catalog.sessions.find(value => value.sessionRef === sessionRef)
    // Permanent deletion is deliberately a second, explicit lifecycle step.
    // Active/recovery sessions must first be archived so one accidental
    // action cannot destroy the durable catalog identity.
    if (selected === undefined || selected.state !== "archived") return current

    const remainingSessions = current.catalog.sessions.filter(value => value.sessionRef !== sessionRef)
    const deleteWorktree = !remainingSessions.some(value => value.worktreeRef === selected.worktreeRef)
    const remainingWorktrees = deleteWorktree
      ? current.catalog.worktrees.filter(value => value.worktreeRef !== selected.worktreeRef)
      : current.catalog.worktrees
    const deleteRepository = !remainingSessions.some(value => value.repositoryRef === selected.repositoryRef) &&
      !remainingWorktrees.some(value => value.repositoryRef === selected.repositoryRef)
    const remainingRepositories = deleteRepository
      ? current.catalog.repositories.filter(value => value.repositoryRef !== selected.repositoryRef)
      : current.catalog.repositories
    const deleteProject = !remainingSessions.some(value => value.projectRef === selected.projectRef) &&
      !remainingWorktrees.some(value => value.projectRef === selected.projectRef) &&
      !remainingRepositories.some(value => value.projectRef === selected.projectRef)
    const remainingCatalog: CodingSessionCatalog = {
      sessions: remainingSessions,
      worktrees: remainingWorktrees,
      repositories: remainingRepositories,
      projects: deleteProject
        ? current.catalog.projects.filter(value => value.projectRef !== selected.projectRef)
        : current.catalog.projects,
    }
    const recent = queryCodingSessions(remainingCatalog, { states: ["active", "idle"] })[0] ??
      queryCodingSessions(remainingCatalog, { states: ["archived"] })[0] ?? null

    // Move navigation before removing the entity. If the process stops
    // between the two durable operations, the archived row remains safely
    // retryable rather than leaving navigation pointed at a missing session.
    persistNavigation(
      remainingCatalog,
      recent?.sessionRef ?? null,
      recent === null ? { kind: "none" } : { kind: "conversation", conversationRef: recent.conversationRef },
      (current.navigation?.openSessionRefs ?? []).filter(value => value !== sessionRef),
    )
    Effect.runSync(input.store.deleteLocalEntities(scope, [
      { entityType: CODING_SESSION_ENTITY_TYPE, entityId: sessionRef },
      ...(deleteWorktree ? [{ entityType: CODING_WORKTREE_ENTITY_TYPE, entityId: selected.worktreeRef }] : []),
      ...(deleteRepository ? [{ entityType: CODING_REPOSITORY_ENTITY_TYPE, entityId: selected.repositoryRef }] : []),
      ...(deleteProject ? [{ entityType: CODING_PROJECT_ENTITY_TYPE, entityId: selected.projectRef }] : []),
    ]))
    if (deleteWorktree) {
      writeBindings(input.bindingFile, readBindings(input.bindingFile).filter(binding =>
        binding.worktreeRef !== selected.worktreeRef))
    }
    return snapshot()
  }

  const recoverSession = (sessionRef: string, requestedRoot: string): DesktopCodingCatalogSnapshot => {
    const root = canonicalRoot(requestedRoot)
    const current = read()
    const selected = current.catalog.sessions.find(value => value.sessionRef === sessionRef)
    const worktree = selected === undefined
      ? undefined
      : current.catalog.worktrees.find(value => value.worktreeRef === selected.worktreeRef)
    if (selected === undefined || worktree === undefined) return snapshot()
    const updatedAt = now()
    const recoveredWorktree = decodeCodingWorktreeEntity({
      ...worktree,
      displayName: path.basename(root) || worktree.displayName,
      availability: { state: "available" },
      updatedAt,
    })
    const recoveredSession = decodeCodingSessionEntity({
      ...selected,
      state: "active",
      archivedAt: null,
      updatedAt,
      lastActiveAt: updatedAt,
    })
    persist([
      { entityType: CODING_WORKTREE_ENTITY_TYPE, entityId: worktree.worktreeRef, postImageJson: JSON.stringify(encodeCodingWorktreeEntity(recoveredWorktree)) },
      { entityType: CODING_SESSION_ENTITY_TYPE, entityId: sessionRef, postImageJson: JSON.stringify(encodeCodingSessionEntity(recoveredSession)) },
    ])
    const bindings = readBindings(input.bindingFile)
    writeBindings(input.bindingFile, [
      ...bindings.filter(binding => binding.worktreeRef !== worktree.worktreeRef && binding.root !== root),
      { worktreeRef: worktree.worktreeRef, root },
    ])
    const next = read()
    persistNavigation(next.catalog, sessionRef, { kind: "conversation", conversationRef: selected.conversationRef }, [sessionRef, ...(current.navigation?.openSessionRefs ?? [])])
    return snapshot()
  }

  const saveFocus = (sessionRef: string, focus: CodingNavigationFocus): DesktopCodingCatalogSnapshot => {
    const current = snapshot()
    const selected = current.catalog.sessions.find(value => value.sessionRef === sessionRef)
    if (selected === undefined) return current
    if (focus.kind === "conversation" &&
      (selected.threadRef !== focus.conversationRef || selected.conversationRef !== focus.conversationRef)) {
      const updatedAt = now()
      const rebound = decodeCodingSessionEntity({
        ...selected,
        threadRef: focus.conversationRef,
        conversationRef: focus.conversationRef,
        updatedAt,
        lastActiveAt: updatedAt,
      })
      persist([{
        entityType: CODING_SESSION_ENTITY_TYPE,
        entityId: sessionRef,
        postImageJson: JSON.stringify(encodeCodingSessionEntity(rebound)),
      }])
    }
    const next = snapshot()
    persistNavigation(next.catalog, sessionRef, focus, [sessionRef, ...(current.navigation?.openSessionRefs ?? [])])
    return snapshot()
  }

  const ownerScopedChangeSet = (ownerUserId: string) => {
    const current = snapshot()
    const ownerScopeRef = String(personalScope(ownerUserId))
    return {
      ownerScopeRef,
      projects: current.catalog.projects.map(value =>
        decodeCodingProjectEntity({ ...value, ownerScopeRef })),
      repositories: current.catalog.repositories.map(value =>
        decodeCodingRepositoryEntity({ ...value, ownerScopeRef })),
      worktrees: current.catalog.worktrees.map(value =>
        decodeCodingWorktreeEntity({ ...value, ownerScopeRef })),
      sessions: current.catalog.sessions.map(value =>
        decodeCodingSessionEntity({ ...value, ownerScopeRef })),
      navigation: current.navigation === null
        ? null
        : decodeCodingNavigationEntity({ ...current.navigation, ownerScopeRef }),
    }
  }

  return {
    snapshot,
    selectWorkspace,
    admitWorkspace,
    openSession,
    archiveSession,
    deleteSession,
    recoverSession,
    saveFocus,
    query: query => queryCodingSessions(snapshot().catalog, query),
    ownerScopedChangeSet,
    selectedRoot: () => {
      const current = read()
      const selected = current.navigation?.selectedSessionRef === null || current.navigation === null
        ? null
        : current.catalog.sessions.find(value => value.sessionRef === current.navigation!.selectedSessionRef) ?? null
      if (selected === null) return null
      const binding = readBindings(input.bindingFile).find(value => value.worktreeRef === selected.worktreeRef)
      return binding !== undefined && existsSync(binding.root) ? binding.root : null
    },
  }
}
