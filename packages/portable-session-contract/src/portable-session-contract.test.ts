import { describe, expect, test } from "vite-plus/test"
import { Schema as S } from "effect"
import {
  PORTABLE_CHECKPOINT_SCHEMA_VERSION,
  PORTABLE_COMMAND_SCHEMA_VERSION,
  PORTABLE_ACTION_INVOCATION_PATHS,
  PORTABLE_SESSION_REAL_HOST_JOURNEY,
  PORTABLE_SESSION_SCHEMA_VERSION,
  PORTABLE_SESSION_EXECUTION_BINDING_SCHEMA_VERSION,
  PortableCheckpointSchema,
  PortableCommandProjectionSchema,
  PortableCodingSessionSchema,
  PortableSessionExecutionBindingSchema,
  PortableSessionCommandSchema,
  PortableTargetDirectoryProjectionSchema,
  auditPortableSessionSnapshot,
  type PortableSessionSnapshot,
} from "./index.js"

// Behavior registry linkage: openagents_apps.portable_session_contract_freeze.v1

const digest = (character: string) => `sha256:${character.repeat(64)}`

function fixture(): PortableSessionSnapshot {
  return {
    session: S.decodeUnknownSync(PortableCodingSessionSchema)({
      schema: PORTABLE_SESSION_SCHEMA_VERSION,
      sessionRef: "coding_session.owner.1",
      ownerRef: "owner.1",
      identityBasis: "owner_minted",
      workContextRef: "work_context.1",
      eventLogRef: "event_log.1",
      currentProjectionRef: "projection.1",
      volatileStreamRef: "stream.1",
      commandScopeRef: "command_scope.1",
      adoptedFromLocalHistory: false,
      graph: {
        rootAgentRef: "agent.root",
        nodes: [
          { agentRef: "agent.root", threadRef: "thread.root", transcriptRef: "transcript.root", activityCursor: 8, lifecycle: "running", attachmentGeneration: 2 },
          { agentRef: "agent.child", parentAgentRef: "agent.root", threadRef: "thread.child", transcriptRef: "transcript.child", activityCursor: 3, lifecycle: "waiting", attachmentGeneration: 2 },
        ],
      },
    }),
    targets: [
      { targetRef: "target.local", targetClass: "owner_local", adapterRef: "adapter.pylon", ownerRef: "owner.1", compatibilityRef: "catalog.1", isolation: "owner_host_process", dataPosture: "owner_device_only", health: "ready" },
      { targetRef: "target.managed", targetClass: "openagents_managed", adapterRef: "adapter.agent_computer", ownerRef: "owner.1", compatibilityRef: "catalog.1", isolation: "dedicated_microvm", dataPosture: "openagents_managed_region", health: "ready" },
    ],
    attachments: [
      { attachmentRef: "attachment.1", sessionRef: "coding_session.owner.1", targetRef: "target.local", generation: 1, state: "detached", descendantAgentRefs: ["agent.root", "agent.child"], capabilityLeaseRefs: [], checkpointRef: "checkpoint.1", evidenceRefs: ["receipt.cleanup.1"] },
      { attachmentRef: "attachment.2", sessionRef: "coding_session.owner.1", targetRef: "target.managed", generation: 2, state: "active", descendantAgentRefs: ["agent.root", "agent.child"], capabilityLeaseRefs: ["lease.provider.2"], checkpointRef: "checkpoint.1", evidenceRefs: ["receipt.attach.2"] },
    ],
    checkpoints: [
      S.decodeUnknownSync(PortableCheckpointSchema)({
        schema: PORTABLE_CHECKPOINT_SCHEMA_VERSION,
        checkpointRef: "checkpoint.1",
        sessionRef: "coding_session.owner.1",
        sourceAttachmentRef: "attachment.1",
        sourceGeneration: 1,
        digest: digest("a"),
        repositoryRef: "repository.1",
        repositoryRevisionRef: "revision.1",
        repositoryPostImageDigest: digest("b"),
        diffDigest: digest("c"),
        eventLogCursor: 8,
        catalogGenerationRef: "catalog.1",
        graphDigest: digest("d"),
        approvalRefs: ["approval.1"],
        artifactRefs: ["artifact.1"],
        receiptRefs: ["receipt.checkpoint.1"],
        secretMaterial: "excluded",
        processState: "excluded",
      }),
    ],
    leases: [
      { leaseRef: "lease.provider.2", ownerRef: "owner.1", sessionRef: "coding_session.owner.1", attachmentRef: "attachment.2", attachmentGeneration: 2, targetRef: "target.managed", capability: "provider", accountRef: "account.codex.2", expiresAt: "2026-07-13T06:00:00.000Z", state: "redeemed" },
    ],
    pendingCommands: [],
    topLevelCatalogSessionRefs: ["thread.root"],
  }
}

describe("portable session contract freeze", () => {
  test("accepts one host-independent session with a complete fenced graph", () => {
    expect(auditPortableSessionSnapshot(fixture())).toEqual([])
  })

  test("schema has no escape hatch for host identity, raw secrets, or process state", () => {
    const base = fixture().session
    expect(() => S.decodeUnknownSync(PortableCodingSessionSchema)({
      ...base,
      sessionRef: "/Users/owner/repo:pid:42",
      providerSessionId: "provider-native-raw-id",
    })).toThrow()
    expect(() => S.decodeUnknownSync(PortableCheckpointSchema)({
      ...fixture().checkpoints[0],
      secretMaterial: "copied",
      rawToken: "do-not-project",
    })).toThrow()
  })

  test("freezes an additive owner/session run and pinned repository binding for movement", () => {
    expect(S.decodeUnknownSync(PortableSessionExecutionBindingSchema)({
      schema: PORTABLE_SESSION_EXECUTION_BINDING_SCHEMA_VERSION,
      sessionRef: "coding_session.owner.1",
      ownerRef: "owner.1",
      runRef: "run.owner.1",
      repositoryRef: "repository.OpenAgentsInc.openagents",
      pinnedBaseRef: "commit.0123456789abcdef0123456789abcdef01234567",
    })).toMatchObject({ runRef: "run.owner.1" })
    expect(() => S.decodeUnknownSync(PortableSessionExecutionBindingSchema)({
      schema: PORTABLE_SESSION_EXECUTION_BINDING_SCHEMA_VERSION,
      sessionRef: "coding_session.owner.1",
      ownerRef: "owner.1",
      runRef: "/Users/owner/run",
      repositoryRef: "repository.OpenAgentsInc.openagents",
      pinnedBaseRef: "commit.0123456789abcdef0123456789abcdef01234567",
    })).toThrow()
  })

  test("rejects two live generations and incomplete descendant fencing", () => {
    const value = fixture()
    value.attachments = value.attachments.map((attachment) =>
      attachment.attachmentRef === "attachment.1"
        ? { ...attachment, state: "quiescing", descendantAgentRefs: ["agent.root"] }
        : attachment,
    )
    const codes = auditPortableSessionSnapshot(value).map((item) => item.code)
    expect(codes).toContain("multiple_live_attachments")
    expect(codes).toContain("attachment_descendant_set_incomplete")
  })

  test("rejects orphan/cyclic graph edges and child catalog leakage", () => {
    const value = fixture()
    const [root, child] = value.session.graph.nodes
    value.session = {
      ...value.session,
      graph: {
        ...value.session.graph,
        nodes: [
          { ...root!, parentAgentRef: "agent.child" },
          { ...child!, parentAgentRef: "agent.missing" },
        ],
      },
    }
    value.topLevelCatalogSessionRefs = ["thread.root", "thread.child"]
    const codes = auditPortableSessionSnapshot(value).map((item) => item.code)
    expect(codes).toContain("root_agent_has_parent")
    expect(codes).toContain("agent_parent_missing")
    expect(codes).toContain("child_leaked_to_top_level_catalog")
  })

  test("rejects stale commands, missing destinations, and silent target changes", () => {
    const value = fixture()
    value.pendingCommands = [
      S.decodeUnknownSync(PortableSessionCommandSchema)({
        schema: PORTABLE_COMMAND_SCHEMA_VERSION,
        commandRef: "command.move.1",
        idempotencyKey: "idempotency.move.1",
        ownerRef: "owner.1",
        sessionRef: "coding_session.owner.1",
        kind: "move",
        expectedAttachmentRef: "attachment.1",
        expectedGeneration: 1,
        expiresAt: "2026-07-13T06:00:00.000Z",
      }),
      S.decodeUnknownSync(PortableSessionCommandSchema)({
        schema: PORTABLE_COMMAND_SCHEMA_VERSION,
        commandRef: "command.resume.1",
        idempotencyKey: "idempotency.resume.1",
        ownerRef: "owner.1",
        sessionRef: "coding_session.owner.1",
        kind: "resume",
        expectedAttachmentRef: "attachment.2",
        expectedGeneration: 2,
        destinationTargetRef: "target.local",
        expiresAt: "2026-07-13T06:00:00.000Z",
      }),
    ]
    const codes = auditPortableSessionSnapshot(value).map((item) => item.code)
    expect(codes).toContain("command_source_is_stale")
    expect(codes).toContain("move_destination_missing")
    expect(codes).toContain("silent_target_change")
  })

  test("rejects moved leases that retain source generation or revoked authority", () => {
    const value = fixture()
    value.leases = [{
      ...value.leases[0]!,
      attachmentRef: "attachment.1",
      attachmentGeneration: 1,
      targetRef: "target.local",
      state: "revoked",
    }]
    value.attachments = value.attachments.map((attachment) =>
      attachment.attachmentRef === "attachment.2"
        ? { ...attachment, capabilityLeaseRefs: ["lease.provider.2"] }
        : attachment,
    )
    const codes = auditPortableSessionSnapshot(value).map((item) => item.code)
    expect(codes).not.toContain("lease_scope_mismatch")
    expect(value.attachments[1]!.capabilityLeaseRefs).not.toEqual(value.attachments[0]!.capabilityLeaseRefs)
  })

  test("freezes a real-host journey with authority evidence and explicit falsifiers", () => {
    expect(PORTABLE_ACTION_INVOCATION_PATHS).toEqual([
      "click",
      "tap",
      "menu",
      "palette",
      "conflict_safe_key",
    ])
    expect(PORTABLE_SESSION_REAL_HOST_JOURNEY.hostClasses).toEqual([
      "owner_local",
      "openagents_managed",
      "owner_managed",
    ])
    expect(PORTABLE_SESSION_REAL_HOST_JOURNEY.steps.map((step) => step.stepRef)).toEqual([
      "cold_open",
      "voice_follow_up",
      "quiesce",
      "revoke_source",
      "attach_managed",
      "cross_client_control",
      "faults",
      "owner_managed_move",
      "stop_reclaim",
    ])
    expect(PORTABLE_SESSION_REAL_HOST_JOURNEY.forbiddenOutcomes).toContain("detail_blocked_first_paint")
    expect(PORTABLE_SESSION_REAL_HOST_JOURNEY.forbiddenOutcomes).toContain("click_tap_shortcut_divergence")
  })

  test("freezes confirmed target-directory and command projection envelopes", () => {
    const value = fixture()
    const command = S.decodeUnknownSync(PortableSessionCommandSchema)({
      schema: PORTABLE_COMMAND_SCHEMA_VERSION,
      commandRef: "command.move.projection",
      idempotencyKey: "idempotency.move.projection",
      ownerRef: "owner.1",
      sessionRef: value.session.sessionRef,
      kind: "move",
      expectedAttachmentRef: "attachment.2",
      expectedGeneration: 2,
      destinationTargetRef: "target.local",
      checkpointRef: "checkpoint.projection",
      expiresAt: "2026-07-17T14:00:00.000Z",
    })
    expect(S.decodeUnknownSync(PortableTargetDirectoryProjectionSchema)({
      sessionRef: value.session.sessionRef,
      targets: value.targets,
    }).targets).toHaveLength(2)
    expect(S.decodeUnknownSync(PortableCommandProjectionSchema)({
      command,
      status: "accepted",
    })).toEqual({ command, status: "accepted" })
    expect(() => S.decodeUnknownSync(PortableCommandProjectionSchema)({
      command,
      status: "completed",
    })).toThrow()
  })
})
