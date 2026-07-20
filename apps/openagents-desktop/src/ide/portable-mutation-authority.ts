import type { DesktopWorkspaceAdmission } from "../desktop-coding-catalog.ts"
import type { IdePortableClientSnapshot } from "./portable-client-contract.ts"

export type IdePortableMutationPermit = Readonly<{
  _tag: "LocalOnly" | "Portable"
  key: string
  grantRef: string
  sessionRef: string
  workContextRef: string
  attachmentRef: string | null
  generation: number | null
  targetRef: string | null
}>

export type IdePortableMutationRefusalReason =
  | "admission_unavailable"
  | "sync_unavailable"
  | "projection_issue"
  | "session_ambiguous"
  | "work_context_mismatch"
  | "attachment_ambiguous"
  | "target_ambiguous"
  | "target_incoherent"
  | "target_not_local"

export type IdePortableMutationAuthorization =
  | Readonly<{ _tag: "Permitted"; permit: IdePortableMutationPermit }>
  | Readonly<{ _tag: "Refused"; reason: IdePortableMutationRefusalReason }>

export type IdePortableMutationAuthority = Readonly<{
  authorize: (grantRef: string) => IdePortableMutationAuthorization
  reauthorize: (permit: IdePortableMutationPermit) => boolean
}>

export type IdePortableMutationAuthoritySource = Readonly<{
  admission: () => DesktopWorkspaceAdmission | null
  portableSnapshot: () => IdePortableClientSnapshot | null
  identityTier: () => "local_only" | "account_linked" | "unavailable"
  knownPortableSession: (sessionRef: string) => boolean
}>

const refused = (reason: IdePortableMutationRefusalReason): IdePortableMutationAuthorization => ({
  _tag: "Refused",
  reason,
})

const exactlyOne = <A>(values: ReadonlyArray<A>): A | null =>
  values.length === 1 ? values[0] ?? null : null

const localPermit = (admission: DesktopWorkspaceAdmission): IdePortableMutationPermit => ({
  _tag: "LocalOnly",
  key: `local:${admission.grantRef}:${admission.sessionRef}:${admission.workContextRef}`,
  grantRef: admission.grantRef,
  sessionRef: admission.sessionRef,
  workContextRef: admission.workContextRef,
  attachmentRef: null,
  generation: null,
  targetRef: null,
})

const authorize = (
  source: IdePortableMutationAuthoritySource,
  grantRef: string,
): IdePortableMutationAuthorization => {
  const admission = source.admission()
  if (admission === null || admission.grantRef !== grantRef) return refused("admission_unavailable")

  const tier = source.identityTier()
  const snapshot = source.portableSnapshot()
  if (snapshot === null || snapshot.status.phase !== "live") {
    return tier === "local_only" && !source.knownPortableSession(admission.sessionRef)
      ? { _tag: "Permitted", permit: localPermit(admission) }
      : refused("sync_unavailable")
  }
  if (snapshot.issues.length > 0) return refused("projection_issue")

  const sessions = snapshot.sessions.filter(value => value.sessionRef === admission.sessionRef)
  if (sessions.length === 0) {
    return source.knownPortableSession(admission.sessionRef)
      ? refused("session_ambiguous")
      : { _tag: "Permitted", permit: localPermit(admission) }
  }
  const session = exactlyOne(sessions)
  if (session === null) return refused("session_ambiguous")
  if (session.workContextRef !== admission.workContextRef) return refused("work_context_mismatch")

  const activeAttachments = snapshot.attachments.filter(value =>
    value.sessionRef === session.sessionRef && value.state === "active")
  const attachment = exactlyOne(activeAttachments)
  if (attachment === null) return refused("attachment_ambiguous")

  const directories = snapshot.targetDirectories.filter(value => value.sessionRef === session.sessionRef)
  const directory = exactlyOne(directories)
  if (directory === null) return refused("target_ambiguous")
  const target = exactlyOne(directory.targets.filter(value => value.targetRef === attachment.targetRef))
  if (target === null) return refused("target_ambiguous")
  if (target.ownerRef !== session.ownerRef || target.health !== "ready") {
    return refused("target_incoherent")
  }
  if (target.targetClass !== "owner_local" || target.dataPosture !== "owner_device_only") {
    return refused("target_not_local")
  }

  const permit: IdePortableMutationPermit = {
    _tag: "Portable",
    key: [
      "portable",
      admission.grantRef,
      session.sessionRef,
      session.workContextRef,
      attachment.attachmentRef,
      attachment.generation,
      attachment.targetRef,
    ].join(":"),
    grantRef: admission.grantRef,
    sessionRef: session.sessionRef,
    workContextRef: session.workContextRef,
    attachmentRef: attachment.attachmentRef,
    generation: attachment.generation,
    targetRef: attachment.targetRef,
  }
  return { _tag: "Permitted", permit }
}

/**
 * Creates the process-local mutation gate from confirmed portable authority.
 * The source owns persistence. This gate does not infer adoption from paths.
 */
export const makeIdePortableMutationAuthority = (
  source: IdePortableMutationAuthoritySource,
): IdePortableMutationAuthority => ({
  authorize: grantRef => authorize(source, grantRef),
  reauthorize: permit => {
    const current = authorize(source, permit.grantRef)
    return current._tag === "Permitted" && current.permit.key === permit.key
  },
})
