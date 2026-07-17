import type { ScopeSyncState } from "@openagentsinc/khala-sync-client"

import type { MobileCodingDirectory } from "./mobile-coding-navigation"

export type MobileControllerTargetReadiness =
  | "ready"
  | "recovery_required"
  | "provider_unavailable"
  | "runtime_unavailable"

export type MobileControllerSession = Readonly<{
  sessionRef: string
  projectRef: string
  repositoryRef: string
  repositoryName: string
  worktreeRef: string
  threadRef: string
  runRef: string | null
  fleetRef: string | null
  currentCheckpointRef: string | null
  agentTopologyRef: string | null
  canonicalEventCursor: number
  provider: MobileCodingDirectory["sessions"][number]["provider"]
  runtime: MobileCodingDirectory["sessions"][number]["runtime"]
  state: MobileCodingDirectory["sessions"][number]["state"]
  targetReadiness: MobileControllerTargetReadiness
  attention: "none" | "needs_recovery"
  lastActiveAt: string
}>

export type MobileControllerRepository = Readonly<{
  repositoryRef: string
  projectRef: string
  displayName: string
  sessions: ReadonlyArray<MobileControllerSession>
}>

export type MobileControllerDirectory = Readonly<{
  authority: MobileCodingDirectory["authority"]
  phase: ScopeSyncState["phase"] | "signed_out"
  cacheState: MobileCodingDirectory["cacheState"]
  offlineCache: MobileCodingDirectory["offlineCache"]
  summary: Readonly<{
    repositoryCount: number
    sessionCount: number
    attentionCount: number
  }>
  recent: ReadonlyArray<MobileControllerSession>
  repositories: ReadonlyArray<MobileControllerRepository>
  attention: ReadonlyArray<MobileControllerSession>
}>

const readinessOf = (
  session: MobileCodingDirectory["sessions"][number],
): MobileControllerTargetReadiness => {
  if (session.state === "recovery_required") return "recovery_required"
  if (session.provider.state === "unavailable") return "provider_unavailable"
  if (session.runtime.state === "unavailable") return "runtime_unavailable"
  return "ready"
}

/**
 * Metadata-only controller projection over the already authorized mobile
 * coding directory. It never rehydrates withheld cache rows and it cannot
 * invent host identity: provider/runtime facts remain the exact known or
 * unavailable union supplied by the confirmed catalog.
 */
export const projectMobileControllerDirectory = (
  directory: MobileCodingDirectory,
): MobileControllerDirectory => {
  if (directory.authority !== "confirmed") {
    return {
      authority: directory.authority,
      phase: directory.phase,
      cacheState: directory.cacheState,
      offlineCache: directory.offlineCache,
      summary: { repositoryCount: 0, sessionCount: 0, attentionCount: 0 },
      recent: [],
      repositories: [],
      attention: [],
    }
  }

  const repositoriesByRef = new Map(
    directory.repositories.map(repository => [repository.repositoryRef, repository] as const),
  )
  const sessions = directory.sessions
    .flatMap<MobileControllerSession>(session => {
      const repository = repositoriesByRef.get(session.repositoryRef)
      if (repository === undefined) return []
      return [{
        ...session,
        repositoryName: repository.displayName,
        targetReadiness: readinessOf(session),
        attention: session.state === "recovery_required" ? "needs_recovery" : "none",
      }]
    })
    .sort((left, right) =>
      right.lastActiveAt.localeCompare(left.lastActiveAt) ||
      left.sessionRef.localeCompare(right.sessionRef),
    )
  const repositories = directory.repositories
    .map<MobileControllerRepository>(repository => ({
      repositoryRef: repository.repositoryRef,
      projectRef: repository.projectRef,
      displayName: repository.displayName,
      sessions: sessions.filter(session => session.repositoryRef === repository.repositoryRef),
    }))
    .filter(repository => repository.sessions.length > 0)
    .sort((left, right) =>
      left.displayName.localeCompare(right.displayName) ||
      left.repositoryRef.localeCompare(right.repositoryRef),
    )
  const attention = sessions.filter(session => session.attention === "needs_recovery")

  return {
    authority: "confirmed",
    phase: directory.phase,
    cacheState: directory.cacheState,
    offlineCache: directory.offlineCache,
    summary: {
      repositoryCount: repositories.length,
      sessionCount: sessions.length,
      attentionCount: attention.length,
    },
    recent: sessions,
    repositories,
    attention,
  }
}
