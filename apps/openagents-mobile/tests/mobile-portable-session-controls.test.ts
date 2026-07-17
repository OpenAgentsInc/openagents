import { describe, expect, test } from "vite-plus/test"
import type { ConfirmedPortableSessionSnapshot } from "@openagentsinc/khala-sync-client"

import {
  buildMobilePortableSessionCommand,
  projectMobilePortableSessionControl,
} from "../src/coding/mobile-portable-session-controls"

const sessionRef = "session.portable.mobile"
const base = (): ConfirmedPortableSessionSnapshot => ({
  status: { phase: "live", cursor: 12, pendingCommandCount: 0 },
  sessions: [{
    schema: "openagents.portable_session.v1",
    sessionRef,
    ownerRef: "owner.mobile",
    identityBasis: "owner_minted",
    workContextRef: "work-context.mobile",
    eventLogRef: "event-log.mobile",
    currentProjectionRef: "projection.mobile",
    commandScopeRef: "command-scope.mobile",
    graph: {
      rootAgentRef: "agent.root",
      nodes: [{ agentRef: "agent.root", threadRef: "thread.root", transcriptRef: "transcript.root", activityCursor: 8, lifecycle: "running", attachmentGeneration: 2 }],
    },
    adoptedFromLocalHistory: false,
  }],
  targetDirectories: [{
    sessionRef,
    targets: [
      { targetRef: "target.local", targetClass: "owner_local", adapterRef: "adapter.pylon", ownerRef: "owner.mobile", compatibilityRef: "catalog.1", isolation: "owner_host_process", dataPosture: "owner_device_only", health: "ready" },
      { targetRef: "target.managed", targetClass: "openagents_managed", adapterRef: "adapter.agent-computer", ownerRef: "owner.mobile", compatibilityRef: "catalog.1", isolation: "dedicated_microvm", dataPosture: "openagents_managed_region", health: "ready" },
      { targetRef: "target.offline", targetClass: "owner_managed", adapterRef: "adapter.oa-node", ownerRef: "owner.mobile", compatibilityRef: "catalog.1", isolation: "owner_host_container", dataPosture: "owner_managed_region", health: "offline" },
    ],
  }],
  attachments: [{
    attachmentRef: "attachment.mobile.2",
    sessionRef,
    targetRef: "target.local",
    generation: 2,
    state: "active",
    descendantAgentRefs: ["agent.root"],
    capabilityLeaseRefs: ["lease.provider.2"],
    evidenceRefs: ["receipt.attach.2"],
  }],
  commands: [],
  issues: [],
})

describe("contract openagents_mobile.portable_session_controls.v1", () => {
  test("projects explicit source generation and eligible movement targets", () => {
    const control = projectMobilePortableSessionControl(base(), sessionRef)
    expect(control).toMatchObject({
      state: "ready",
      sourceAttachment: { attachmentRef: "attachment.mobile.2", generation: 2 },
      sourceTarget: { targetRef: "target.local" },
      actions: {
        stop: { available: true }, checkpoint: { available: true },
        move: { available: true }, resume: { available: false }, failback: { available: false },
      },
    })
    if (control.state !== "ready") throw new Error("expected ready control")
    expect(control.actions.move.destinations.map(value => value.targetRef)).toEqual(["target.managed"])
    expect(control.actions.failback.reason).toBe("failback_target_missing")
  })

  test("builds byte-identical fenced move commands for one invocation", () => {
    const control = projectMobilePortableSessionControl(base(), sessionRef)
    const input = {
      control,
      action: "move" as const,
      invocationRef: "tap.0001",
      issuedAt: "2026-07-17T12:00:00.000Z",
      destinationTargetRef: "target.managed",
    }
    const first = buildMobilePortableSessionCommand(input)
    const replay = buildMobilePortableSessionCommand(input)
    expect(first).toEqual(replay)
    expect(first).toMatchObject({ state: "admitted", command: {
      commandRef: "command.mobile.move.tap.0001",
      idempotencyKey: "idempotency.mobile.move.tap.0001",
      expectedAttachmentRef: "attachment.mobile.2",
      expectedGeneration: 2,
      destinationTargetRef: "target.managed",
      checkpointRef: "checkpoint.mobile.move.tap.0001",
      expiresAt: "2026-07-17T12:01:00.000Z",
    } })
  })

  test("rejects missing, same, offline, and implicit destinations", () => {
    const control = projectMobilePortableSessionControl(base(), sessionRef)
    const build = (destinationTargetRef?: string) => buildMobilePortableSessionCommand({
      control, action: "move", invocationRef: "tap.0002",
      issuedAt: "2026-07-17T12:00:00.000Z",
      ...(destinationTargetRef === undefined ? {} : { destinationTargetRef }),
    })
    expect(build()).toEqual({ state: "rejected", reason: "destination_required" })
    expect(build("target.local")).toEqual({ state: "rejected", reason: "destination_is_source" })
    expect(build("target.offline")).toEqual({ state: "rejected", reason: "destination_not_ready" })
  })

  test("blocks all controls while a local or confirmed command is unresolved", () => {
    const initial = base()
    const pendingLocal = { ...initial, status: { ...initial.status, pendingCommandCount: 1 } }
    const localControl = projectMobilePortableSessionControl(pendingLocal, sessionRef)
    expect(localControl.state === "ready" && localControl.actions.stop.reason).toBe("command_in_flight")

    const accepted = { ...base(), commands: [{
      command: {
        schema: "openagents.portable_session_command.v1" as const,
        commandRef: "command.checkpoint.pending",
        idempotencyKey: "idempotency.checkpoint.pending",
        ownerRef: "owner.mobile",
        sessionRef,
        kind: "checkpoint" as const,
        expectedAttachmentRef: "attachment.mobile.2",
        expectedGeneration: 2,
        checkpointRef: "checkpoint.pending",
        expiresAt: "2026-07-17T12:01:00.000Z",
      },
      status: "accepted" as const,
    }] }
    const confirmedControl = projectMobilePortableSessionControl(accepted, sessionRef)
    expect(confirmedControl.state === "ready" && confirmedControl.actions.move.available).toBe(false)
  })

  test("admits resume only from one suspended latest generation", () => {
    const initial = base()
    const value = { ...initial, attachments: [{ ...initial.attachments[0]!, state: "quiesced" as const }] }
    const control = projectMobilePortableSessionControl(value, sessionRef)
    expect(control.state === "ready" && control.actions.resume.available).toBe(true)
    expect(buildMobilePortableSessionCommand({
      control, action: "resume", invocationRef: "tap.resume.1",
      issuedAt: "2026-07-17T12:00:00.000Z",
    })).toMatchObject({ state: "admitted", command: {
      kind: "resume", expectedGeneration: 2,
    } })
  })

  test("withholds malformed, stale, and ambiguous authority without leaking private material", () => {
    const stale = { ...base(), status: { phase: "must_refetch" as const, cursor: 12, pendingCommandCount: 0 } }
    expect(projectMobilePortableSessionControl(stale, sessionRef)).toEqual({
      state: "unavailable", sessionRef, reason: "authority_unavailable",
    })
    const initial = base()
    const ambiguous = { ...initial, attachments: [
      initial.attachments[0]!,
      { ...initial.attachments[0]!, attachmentRef: "attachment.mobile.other", generation: 3 },
    ] }
    expect(projectMobilePortableSessionControl(ambiguous, sessionRef)).toMatchObject({
      state: "unavailable", reason: "attachment_authority_ambiguous",
    })
    const serialized = JSON.stringify(projectMobilePortableSessionControl(base(), sessionRef))
    expect(serialized).not.toContain("/Users/")
    expect(serialized).not.toContain("token")
    expect(serialized).not.toContain("hostname")
  })
})
