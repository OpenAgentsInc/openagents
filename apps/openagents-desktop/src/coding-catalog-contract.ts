import {
  CodingNavigationEntity,
  CodingNavigationFocus,
  CodingRef,
  queryCodingSessions,
  resolveCodingNavigation,
} from "@openagentsinc/khala-sync"
import { Schema } from "effect"

import type { DesktopCodingCatalogSnapshot } from "./desktop-coding-catalog.ts"

export const DesktopCodingCatalogSnapshotChannel = "openagents-desktop/coding-catalog-snapshot"
export const DesktopCodingCatalogChooseChannel = "openagents-desktop/coding-catalog-choose"
export const DesktopCodingCatalogOpenChannel = "openagents-desktop/coding-catalog-open"
export const DesktopCodingCatalogArchiveChannel = "openagents-desktop/coding-catalog-archive"
export const DesktopCodingCatalogRecoverChannel = "openagents-desktop/coding-catalog-recover"
export const DesktopCodingCatalogFocusChannel = "openagents-desktop/coding-catalog-focus"

export const DesktopCodingCatalogRecoveryReason = Schema.Literals([
  "ambiguous_alias",
  "archived",
  "catalog_invalid",
  "grant_revoked",
  "grant_unavailable",
  "missing_repository",
  "missing_session",
  "missing_worktree",
  "owner_scope_mismatch",
])

export const DesktopCodingCatalogSessionSchema = Schema.Struct({
  sessionRef: CodingRef,
  // Optional on decode so retained pre-admission local rows stay viewable.
  // Every newly projected admitted workspace supplies both fields.
  workContextRef: Schema.optionalKey(CodingRef),
  grantRef: Schema.optionalKey(Schema.NullOr(CodingRef)),
  projectRef: CodingRef,
  repositoryRef: CodingRef,
  worktreeRef: CodingRef,
  projectLabel: Schema.String.check(Schema.isMaxLength(160)),
  repositoryLabel: Schema.String.check(Schema.isMaxLength(160)),
  worktreeLabel: Schema.String.check(Schema.isMaxLength(160)),
  state: Schema.Literals(["active", "idle", "recovery_required", "archived"]),
  lastActiveAt: Schema.String,
  recoveryReason: Schema.NullOr(DesktopCodingCatalogRecoveryReason),
})
export type DesktopCodingCatalogSession = typeof DesktopCodingCatalogSessionSchema.Type

export const DesktopCodingCatalogProjectionSchema = Schema.Struct({
  authority: Schema.Literal("device_local"),
  authorityLabel: Schema.Literal("This Mac"),
  selectedSessionRef: Schema.NullOr(CodingRef),
  focus: CodingNavigationFocus,
  sessions: Schema.Array(DesktopCodingCatalogSessionSchema).check(Schema.isMaxLength(2_048)),
})
export type DesktopCodingCatalogProjection = typeof DesktopCodingCatalogProjectionSchema.Type

export type DesktopCodingCatalogQueryPlan = Readonly<{
  projectRef?: string
  repositoryRef?: string
  state?: "active" | "idle" | "recovery" | "archived"
}>

export type DesktopCodingCatalogQueryResult =
  | Readonly<{ state: "empty"; plan: DesktopCodingCatalogQueryPlan }>
  | Readonly<{ state: "valid"; plan: DesktopCodingCatalogQueryPlan }>
  | Readonly<{ state: "invalid"; reason: string }>

export const DesktopCodingSessionRequestSchema = Schema.Struct({ sessionRef: CodingRef })
export const DesktopCodingFocusRequestSchema = Schema.Struct({
  sessionRef: CodingRef,
  focus: CodingNavigationFocus,
})

// Effect Schema's decoder service type is intentionally erased at this IPC
// perimeter; each exported call pins a concrete schema immediately below.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const decode = <A>(schema: any, raw: unknown): A | null => {
  const exit = Schema.decodeUnknownExit(schema)(raw)
  return exit._tag === "Success" ? exit.value : null
}

export const decodeDesktopCodingSessionRequest = (raw: unknown) =>
  decode<typeof DesktopCodingSessionRequestSchema.Type>(DesktopCodingSessionRequestSchema, raw)
export const decodeDesktopCodingFocusRequest = (raw: unknown) =>
  decode<typeof DesktopCodingFocusRequestSchema.Type>(DesktopCodingFocusRequestSchema, raw)
export const decodeDesktopCodingCatalogProjection = (raw: unknown) =>
  decode<DesktopCodingCatalogProjection>(DesktopCodingCatalogProjectionSchema, raw)

export const emptyDesktopCodingCatalogProjection = (): DesktopCodingCatalogProjection => ({
  authority: "device_local",
  authorityLabel: "This Mac",
  selectedSessionRef: null,
  focus: { kind: "none" },
  sessions: [],
})

/**
 * Explicit bounded query grammar. Free-form keyword retrieval is intentionally
 * absent until a central semantic selector exists.
 */
export const parseDesktopCodingCatalogQuery = (
  input: string,
): DesktopCodingCatalogQueryResult => {
  const source = input.trim()
  if (source === "") return { state: "empty", plan: {} }
  const plan: { projectRef?: string; repositoryRef?: string; state?: "active" | "idle" | "recovery" | "archived" } = {}
  for (const token of source.split(/\s+/)) {
    const separator = token.indexOf(":")
    if (separator <= 0 || separator === token.length - 1) {
      return { state: "invalid", reason: "Use field:value filters." }
    }
    const field = token.slice(0, separator)
    const value = token.slice(separator + 1)
    if (field === "project" && plan.projectRef === undefined) {
      const ref = decode<string>(CodingRef, value)
      if (ref === null) return { state: "invalid", reason: "Project ref is invalid." }
      plan.projectRef = ref
    } else if (field === "repository" && plan.repositoryRef === undefined) {
      const ref = decode<string>(CodingRef, value)
      if (ref === null) return { state: "invalid", reason: "Repository ref is invalid." }
      plan.repositoryRef = ref
    } else if (field === "state" && plan.state === undefined &&
      (value === "active" || value === "idle" || value === "recovery" || value === "archived")) {
      plan.state = value
    } else {
      return { state: "invalid", reason: `Unsupported or duplicate filter: ${field}.` }
    }
  }
  return { state: "valid", plan }
}

export const filterDesktopCodingCatalog = (
  projection: DesktopCodingCatalogProjection,
  query: DesktopCodingCatalogQueryPlan,
): ReadonlyArray<DesktopCodingCatalogSession> => projection.sessions.filter(session =>
  (query.projectRef === undefined || session.projectRef === query.projectRef) &&
  (query.repositoryRef === undefined || session.repositoryRef === query.repositoryRef) &&
  (query.state === undefined || (
    query.state === "recovery"
      ? session.recoveryReason !== null || session.state === "recovery_required"
      : session.state === query.state
  )))

export const desktopWorkspaceForCodingFocus = (
  focus: typeof CodingNavigationFocus.Type,
): "chat" | "files" | "terminal" | "home" =>
  focus.kind === "conversation"
    ? "chat"
    : focus.kind === "editor"
      ? "files"
      : focus.kind === "terminal"
        ? "terminal"
        : "home"

export const projectDesktopCodingCatalog = (
  snapshot: DesktopCodingCatalogSnapshot,
): DesktopCodingCatalogProjection => {
  const navigation = snapshot.navigation
  const sessions = queryCodingSessions(snapshot.catalog, {}).map(session => {
    const project = snapshot.catalog.projects.find(value => value.projectRef === session.projectRef)
    const repository = snapshot.catalog.repositories.find(value => value.repositoryRef === session.repositoryRef)
    const worktree = snapshot.catalog.worktrees.find(value => value.worktreeRef === session.worktreeRef)
    const resolution = resolveCodingNavigation(
      snapshot.catalog,
      new CodingNavigationEntity({
        schema: "openagents.coding_catalog.v1",
        navigationRef: navigation?.navigationRef ?? "navigation.desktop.projection",
        ownerScopeRef: session.ownerScopeRef,
        selectedProjectRef: session.projectRef,
        selectedRepositoryRef: session.repositoryRef,
        selectedWorktreeRef: session.worktreeRef,
        selectedSessionRef: session.sessionRef,
        openSessionRefs: [session.sessionRef],
        focus: { kind: "none" },
        updatedAt: navigation?.updatedAt ?? session.updatedAt,
      }),
    )
    return {
      sessionRef: session.sessionRef,
      workContextRef: session.workContextRef,
      grantRef: session.grant.state === "granted" ? session.grant.grantRef : null,
      projectRef: session.projectRef,
      repositoryRef: session.repositoryRef,
      worktreeRef: session.worktreeRef,
      projectLabel: project?.displayName ?? "Project unavailable",
      repositoryLabel: repository?.displayName ?? "Repository unavailable",
      worktreeLabel: worktree?.displayName ?? "Worktree unavailable",
      state: session.state,
      lastActiveAt: session.lastActiveAt,
      recoveryReason: resolution.state === "recovery_required" ? resolution.reason : null,
    }
  })
  return Schema.decodeUnknownSync(DesktopCodingCatalogProjectionSchema)({
    authority: "device_local",
    authorityLabel: "This Mac",
    selectedSessionRef: navigation?.selectedSessionRef ?? null,
    focus: navigation?.focus ?? { kind: "none" },
    sessions,
  })
}
