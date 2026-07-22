import { describe, expect, test } from "vite-plus/test"

import type { DesktopWorkspaceAdmission } from "../desktop-coding-catalog.ts"
import type { IdePortableClientSnapshot } from "./portable-client-contract.ts"
import { makeIdePortableMutationAuthority } from "./portable-mutation-authority.ts"

const admission: DesktopWorkspaceAdmission = {
  grantRef: "workspace.grant.1",
  projectRef: "project.1",
  repositoryRef: "repository.1",
  worktreeRef: "worktree.1",
  workContextRef: "work-context.1",
  sessionRef: "session.portable.1",
}

const session: IdePortableClientSnapshot["sessions"][number] = {
  schema: "openagents.portable_session.v1",
  sessionRef: admission.sessionRef,
  ownerRef: "owner.1",
  identityBasis: "owner_minted",
  workContextRef: admission.workContextRef,
  eventLogRef: "event-log.1",
  currentProjectionRef: "projection.1",
  commandScopeRef: "command-scope.1",
  graph: {
    rootAgentRef: "agent.root",
    nodes: [{
      agentRef: "agent.root",
      threadRef: "thread.root",
      transcriptRef: "transcript.root",
      activityCursor: 1,
      lifecycle: "running",
      attachmentGeneration: 3,
    }],
  },
  adoptedFromLocalHistory: true,
  adoptionReceiptRef: "receipt.adoption.1",
}

const snapshot = (): IdePortableClientSnapshot => ({
  status: { phase: "live", cursor: 10, pendingCommandCount: 0 },
  sessions: [session],
  targetDirectories: [{
    sessionRef: admission.sessionRef,
    targets: [{
      targetRef: "target.local.1",
      targetClass: "owner_local",
      adapterRef: "adapter.local.1",
      ownerRef: session.ownerRef,
      compatibilityRef: "compatibility.1",
      isolation: "owner_host_process",
      dataPosture: "owner_device_only",
      health: "ready",
    }],
  }],
  attachments: [{
    attachmentRef: "attachment.3",
    sessionRef: admission.sessionRef,
    targetRef: "target.local.1",
    generation: 3,
    state: "active",
    descendantAgentRefs: ["agent.root"],
    capabilityLeaseRefs: [],
    evidenceRefs: [],
  }],
  commands: [],
  issues: [],
})

describe("Desktop portable mutation authority", () => {
  test("permits one coherent confirmed owner-local attachment", () => {
    const authority = makeIdePortableMutationAuthority({
      admission: () => admission,
      portableSnapshot: snapshot,
      identityTier: () => "account_linked",
      knownPortableSession: () => false,
    })
    const result = authority.authorize(admission.grantRef)
    expect(result).toMatchObject({
      _tag: "Permitted",
      permit: {
        _tag: "Portable",
        sessionRef: admission.sessionRef,
        workContextRef: admission.workContextRef,
        attachmentRef: "attachment.3",
        generation: 3,
        targetRef: "target.local.1",
      },
    })
    if (result._tag !== "Permitted") throw new Error("expected permit")
    expect(authority.reauthorize(result.permit)).toBe(true)
  })

  test("keeps an unadopted local-only workspace writable without Sync", () => {
    const authority = makeIdePortableMutationAuthority({
      admission: () => admission,
      portableSnapshot: () => null,
      identityTier: () => "local_only",
      knownPortableSession: () => false,
    })
    expect(authority.authorize(admission.grantRef)).toMatchObject({
      _tag: "Permitted",
      permit: { _tag: "LocalOnly", attachmentRef: null, generation: null },
    })
  })

  test("fails closed when a known portable workspace becomes local-only and offline", () => {
    const authority = makeIdePortableMutationAuthority({
      admission: () => admission,
      portableSnapshot: () => null,
      identityTier: () => "local_only",
      knownPortableSession: sessionRef => sessionRef === admission.sessionRef,
    })
    expect(authority.authorize(admission.grantRef)).toEqual({
      _tag: "Refused",
      reason: "sync_unavailable",
    })
  })

  test("permits a live account-linked workspace with no adopted portable session", () => {
    const authority = makeIdePortableMutationAuthority({
      admission: () => admission,
      portableSnapshot: () => ({ ...snapshot(), sessions: [], targetDirectories: [], attachments: [] }),
      identityTier: () => "account_linked",
      knownPortableSession: () => false,
    })
    expect(authority.authorize(admission.grantRef)).toMatchObject({
      _tag: "Permitted",
      permit: { _tag: "LocalOnly" },
    })
  })

  test("refuses a missing confirmed session or active attachment after adoption", () => {
    let current: IdePortableClientSnapshot = {
      ...snapshot(),
      sessions: [],
      targetDirectories: [],
      attachments: [],
    }
    const authority = makeIdePortableMutationAuthority({
      admission: () => admission,
      portableSnapshot: () => current,
      identityTier: () => "account_linked",
      knownPortableSession: () => true,
    })
    expect(authority.authorize(admission.grantRef)).toEqual({
      _tag: "Refused",
      reason: "session_ambiguous",
    })
    current = { ...snapshot(), attachments: [] }
    expect(authority.authorize(admission.grantRef)).toEqual({
      _tag: "Refused",
      reason: "attachment_ambiguous",
    })
  })

  test("fails closed for linked offline state and projection issues", () => {
    let current: IdePortableClientSnapshot | null = null
    const authority = makeIdePortableMutationAuthority({
      admission: () => admission,
      portableSnapshot: () => current,
      identityTier: () => "account_linked",
      knownPortableSession: () => true,
    })
    expect(authority.authorize(admission.grantRef)).toEqual({ _tag: "Refused", reason: "sync_unavailable" })
    current = { ...snapshot(), issues: [{ code: "orphaned", affectedRef: "attachment.orphaned" }] }
    expect(authority.authorize(admission.grantRef)).toEqual({ _tag: "Refused", reason: "projection_issue" })
  })

  test("refuses ambiguous, remote, and incoherent placement", () => {
    let current = snapshot()
    const authority = makeIdePortableMutationAuthority({
      admission: () => admission,
      portableSnapshot: () => current,
      identityTier: () => "account_linked",
      knownPortableSession: () => true,
    })
    current = { ...snapshot(), attachments: [...snapshot().attachments, { ...snapshot().attachments[0]!, attachmentRef: "attachment.4" }] }
    expect(authority.authorize(admission.grantRef)).toEqual({ _tag: "Refused", reason: "attachment_ambiguous" })

    const remote = snapshot()
    current = {
      ...remote,
      targetDirectories: [{
        ...remote.targetDirectories[0]!,
        targets: [{
          ...remote.targetDirectories[0]!.targets[0]!,
          targetClass: "openagents_managed",
          dataPosture: "openagents_managed_region",
        }],
      }],
    }
    expect(authority.authorize(admission.grantRef)).toEqual({ _tag: "Refused", reason: "target_not_local" })

    const foreign = snapshot()
    current = {
      ...foreign,
      targetDirectories: [{
        ...foreign.targetDirectories[0]!,
        targets: [{ ...foreign.targetDirectories[0]!.targets[0]!, ownerRef: "owner.foreign" }],
      }],
    }
    expect(authority.authorize(admission.grantRef)).toEqual({ _tag: "Refused", reason: "target_incoherent" })
  })

  test("invalidates a captured permit after an attachment generation change", () => {
    let current = snapshot()
    const authority = makeIdePortableMutationAuthority({
      admission: () => admission,
      portableSnapshot: () => current,
      identityTier: () => "account_linked",
      knownPortableSession: () => true,
    })
    const authorized = authority.authorize(admission.grantRef)
    if (authorized._tag !== "Permitted") throw new Error("expected permit")
    current = {
      ...current,
      attachments: [{
        ...current.attachments[0]!,
        attachmentRef: "attachment.4",
        generation: 4,
      }],
    }
    expect(authority.reauthorize(authorized.permit)).toBe(false)
  })

  test("refuses a mismatched admission grant or work context", () => {
    let currentAdmission = admission
    const authority = makeIdePortableMutationAuthority({
      admission: () => currentAdmission,
      portableSnapshot: snapshot,
      identityTier: () => "account_linked",
      knownPortableSession: () => true,
    })
    expect(authority.authorize("workspace.grant.foreign")).toEqual({
      _tag: "Refused",
      reason: "admission_unavailable",
    })
    currentAdmission = { ...admission, workContextRef: "work-context.other" }
    expect(authority.authorize(admission.grantRef)).toEqual({
      _tag: "Refused",
      reason: "work_context_mismatch",
    })
  })
})
