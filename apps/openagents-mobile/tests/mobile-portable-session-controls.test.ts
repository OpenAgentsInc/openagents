import { describe, expect, test } from "vite-plus/test";
import type { ConfirmedPortableSessionSnapshot } from "@openagentsinc/khala-sync-client";

import {
  buildMobilePortableSessionCommand,
  projectMobilePortableSessionControl,
} from "../src/coding/mobile-portable-session-controls";

const sessionRef = "session.portable.mobile";
const base = (): ConfirmedPortableSessionSnapshot => ({
  status: { phase: "live", cursor: 12, pendingCommandCount: 0 },
  sessions: [
    {
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
        nodes: [
          {
            agentRef: "agent.root",
            threadRef: "thread.root",
            transcriptRef: "transcript.root",
            activityCursor: 8,
            lifecycle: "running",
            attachmentGeneration: 2,
          },
        ],
      },
      adoptedFromLocalHistory: false,
    },
  ],
  targetDirectories: [
    {
      sessionRef,
      targets: [
        {
          targetRef: "target.local",
          targetClass: "owner_local",
          adapterRef: "adapter.pylon",
          ownerRef: "owner.mobile",
          compatibilityRef: "catalog.1",
          isolation: "owner_host_process",
          dataPosture: "owner_device_only",
          health: "ready",
        },
        {
          targetRef: "target.managed",
          targetClass: "openagents_managed",
          adapterRef: "adapter.agent-computer",
          ownerRef: "owner.mobile",
          compatibilityRef: "catalog.1",
          isolation: "dedicated_microvm",
          dataPosture: "openagents_managed_region",
          health: "ready",
        },
        {
          targetRef: "target.offline",
          targetClass: "owner_managed",
          adapterRef: "adapter.oa-node",
          ownerRef: "owner.mobile",
          compatibilityRef: "catalog.1",
          isolation: "owner_host_container",
          dataPosture: "owner_managed_region",
          health: "offline",
        },
      ],
    },
  ],
  attachments: [
    {
      attachmentRef: "attachment.mobile.2",
      sessionRef,
      targetRef: "target.local",
      generation: 2,
      state: "active",
      descendantAgentRefs: ["agent.root"],
      capabilityLeaseRefs: ["lease.provider.2"],
      evidenceRefs: ["receipt.attach.2"],
    },
  ],
  commands: [],
  issues: [],
});

describe("contract openagents_mobile.portable_session_controls.v1", () => {
  test("projects explicit source generation and eligible movement targets", () => {
    const control = projectMobilePortableSessionControl(base(), sessionRef);
    expect(control).toMatchObject({
      state: "ready",
      sourceAttachment: { attachmentRef: "attachment.mobile.2", generation: 2 },
      sourceTarget: { targetRef: "target.local" },
      actions: {
        stop: { available: true },
        checkpoint: { available: true },
        move: { available: true },
        resume: { available: false },
        failback: { available: false },
      },
    });
    if (control.state !== "ready") throw new Error("expected ready control");
    expect(control.actions.move.destinations.map((value) => value.targetRef)).toEqual([
      "target.managed",
    ]);
    expect(control.actions.failback.reason).toBe("failback_target_missing");
  });

  test("builds byte-identical fenced move commands for one invocation", () => {
    const control = projectMobilePortableSessionControl(base(), sessionRef);
    const input = {
      control,
      action: "move" as const,
      invocationRef: "tap.0001",
      issuedAt: "2026-07-17T12:00:00.000Z",
      destinationTargetRef: "target.managed",
    };
    const first = buildMobilePortableSessionCommand(input);
    const replay = buildMobilePortableSessionCommand(input);
    expect(first).toEqual(replay);
    expect(JSON.stringify(first)).toBe(JSON.stringify(replay));
    expect(first).toMatchObject({
      state: "admitted",
      command: {
        commandRef: "command.mobile.move.tap.0001",
        idempotencyKey: "idempotency.mobile.move.tap.0001",
        expectedAttachmentRef: "attachment.mobile.2",
        expectedGeneration: 2,
        destinationTargetRef: "target.managed",
        checkpointRef: "checkpoint.mobile.move.tap.0001",
        expiresAt: "2026-07-17T12:01:00.000Z",
      },
    });
  });

  test("rejects missing, same, offline, and implicit destinations", () => {
    const control = projectMobilePortableSessionControl(base(), sessionRef);
    const build = (destinationTargetRef?: string) =>
      buildMobilePortableSessionCommand({
        control,
        action: "move",
        invocationRef: "tap.0002",
        issuedAt: "2026-07-17T12:00:00.000Z",
        ...(destinationTargetRef === undefined ? {} : { destinationTargetRef }),
      });
    expect(build()).toEqual({ state: "rejected", reason: "destination_required" });
    expect(build("target.local")).toEqual({ state: "rejected", reason: "destination_is_source" });
    expect(build("target.offline")).toEqual({ state: "rejected", reason: "destination_not_ready" });
  });

  test("blocks all controls while a local or confirmed command is unresolved", () => {
    const initial = base();
    const pendingLocal = { ...initial, status: { ...initial.status, pendingCommandCount: 1 } };
    const localControl = projectMobilePortableSessionControl(pendingLocal, sessionRef);
    expect(localControl.state === "ready" && localControl.actions.stop.reason).toBe(
      "command_in_flight",
    );
    expect(
      buildMobilePortableSessionCommand({
        control: localControl,
        action: "stop",
        invocationRef: "tap.pending.1",
        issuedAt: "2026-07-17T12:00:00.000Z",
      }),
    ).toEqual({ state: "rejected", reason: "command_in_flight" });

    const accepted = {
      ...base(),
      commands: [
        {
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
        },
      ],
    };
    const confirmedControl = projectMobilePortableSessionControl(accepted, sessionRef);
    expect(confirmedControl.state === "ready" && confirmedControl.actions.move.available).toBe(
      false,
    );

    const acceptedAttach = {
      ...base(),
      commands: [
        {
          command: {
            schema: "openagents.portable_session_command.v1" as const,
            commandRef: "command.attach.pending",
            idempotencyKey: "idempotency.attach.pending",
            ownerRef: "owner.mobile",
            sessionRef,
            kind: "attach" as const,
            expectedAttachmentRef: "attachment.mobile.2",
            expectedGeneration: 2,
            destinationTargetRef: "target.managed",
            expiresAt: "2026-07-17T12:01:00.000Z",
          },
          status: "accepted" as const,
        },
      ],
    };
    const attachControl = projectMobilePortableSessionControl(acceptedAttach, sessionRef);
    expect(attachControl.state === "ready" && attachControl.actions.stop.reason).toBe(
      "command_in_flight",
    );
  });

  test("admits resume only from one suspended latest generation", () => {
    const initial = base();
    const value = {
      ...initial,
      attachments: [{ ...initial.attachments[0]!, state: "quiesced" as const }],
    };
    const control = projectMobilePortableSessionControl(value, sessionRef);
    expect(control.state === "ready" && control.actions.resume.available).toBe(true);
    expect(
      buildMobilePortableSessionCommand({
        control,
        action: "resume",
        invocationRef: "tap.resume.1",
        issuedAt: "2026-07-17T12:00:00.000Z",
      }),
    ).toMatchObject({
      state: "admitted",
      command: {
        kind: "resume",
        expectedGeneration: 2,
      },
    });
  });

  test("withholds malformed, stale, and ambiguous authority without leaking private material", () => {
    const stale = {
      ...base(),
      status: { phase: "must_refetch" as const, cursor: 12, pendingCommandCount: 0 },
    };
    expect(projectMobilePortableSessionControl(stale, sessionRef)).toEqual({
      state: "unavailable",
      sessionRef,
      reason: "authority_unavailable",
    });
    const initial = base();
    const ambiguous = {
      ...initial,
      attachments: [
        initial.attachments[0]!,
        { ...initial.attachments[0]!, attachmentRef: "attachment.mobile.other", generation: 3 },
      ],
    };
    expect(projectMobilePortableSessionControl(ambiguous, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "attachment_authority_ambiguous",
    });
    const serialized = JSON.stringify(projectMobilePortableSessionControl(base(), sessionRef));
    expect(serialized).not.toContain("/Users/");
    expect(serialized).not.toContain("token");
    expect(serialized).not.toContain("hostname");
  });

  test("rejects duplicate session, work-context, directory, and owner joins", () => {
    const initial = base();
    const duplicateSession = {
      ...initial,
      sessions: [...initial.sessions, { ...initial.sessions[0]! }],
    };
    expect(projectMobilePortableSessionControl(duplicateSession, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "projection_invalid",
    });

    const sharedWorkContext = {
      ...initial,
      sessions: [
        ...initial.sessions,
        {
          ...initial.sessions[0]!,
          sessionRef: "session.portable.other",
        },
      ],
    };
    expect(projectMobilePortableSessionControl(sharedWorkContext, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "projection_invalid",
    });

    const duplicateDirectory = {
      ...initial,
      targetDirectories: [...initial.targetDirectories, { ...initial.targetDirectories[0]! }],
    };
    expect(projectMobilePortableSessionControl(duplicateDirectory, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "projection_invalid",
    });

    const mixedOwner = {
      ...initial,
      targetDirectories: [
        {
          ...initial.targetDirectories[0]!,
          targets: [
            initial.targetDirectories[0]!.targets[0]!,
            { ...initial.targetDirectories[0]!.targets[1]!, ownerRef: "owner.other" },
            initial.targetDirectories[0]!.targets[2]!,
          ],
        },
      ],
    };
    expect(projectMobilePortableSessionControl(mixedOwner, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "projection_invalid",
    });

    const duplicateTarget = {
      ...initial,
      targetDirectories: [
        {
          ...initial.targetDirectories[0]!,
          targets: [
            ...initial.targetDirectories[0]!.targets,
            { ...initial.targetDirectories[0]!.targets[1]! },
          ],
        },
      ],
    };
    expect(projectMobilePortableSessionControl(duplicateTarget, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "projection_invalid",
    });
  });

  test("rejects stale, duplicate, and transitional attachment authority", () => {
    const initial = base();
    const staleActive = {
      ...initial,
      attachments: [
        initial.attachments[0]!,
        {
          ...initial.attachments[0]!,
          attachmentRef: "attachment.mobile.3",
          generation: 3,
          state: "detached" as const,
        },
      ],
    };
    expect(projectMobilePortableSessionControl(staleActive, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "attachment_authority_ambiguous",
    });

    const duplicateAttachmentRef = {
      ...initial,
      attachments: [
        {
          ...initial.attachments[0]!,
          generation: 1,
          state: "detached" as const,
        },
        initial.attachments[0]!,
      ],
    };
    expect(projectMobilePortableSessionControl(duplicateAttachmentRef, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "attachment_authority_ambiguous",
    });

    const transitional = {
      ...initial,
      attachments: [
        {
          ...initial.attachments[0]!,
          attachmentRef: "attachment.mobile.1",
          generation: 1,
          state: "quiescing" as const,
        },
        initial.attachments[0]!,
      ],
    };
    expect(projectMobilePortableSessionControl(transitional, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "attachment_authority_ambiguous",
    });
  });

  test("rejects stale graph generations, incomplete descendants, and invalid parents", () => {
    const initial = base();
    const staleGraph = {
      ...initial,
      sessions: [
        {
          ...initial.sessions[0]!,
          graph: {
            ...initial.sessions[0]!.graph,
            nodes: [
              {
                ...initial.sessions[0]!.graph.nodes[0]!,
                attachmentGeneration: 1,
              },
            ],
          },
        },
      ],
    };
    expect(projectMobilePortableSessionControl(staleGraph, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "attachment_authority_ambiguous",
    });

    const incompleteDescendants = {
      ...initial,
      attachments: [{ ...initial.attachments[0]!, descendantAgentRefs: [] }],
    };
    expect(projectMobilePortableSessionControl(incompleteDescendants, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "attachment_authority_ambiguous",
    });

    const rootWithParent = {
      ...initial,
      sessions: [
        {
          ...initial.sessions[0]!,
          graph: {
            ...initial.sessions[0]!.graph,
            nodes: [
              {
                ...initial.sessions[0]!.graph.nodes[0]!,
                parentAgentRef: "agent.root",
              },
            ],
          },
        },
      ],
    };
    expect(projectMobilePortableSessionControl(rootWithParent, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "projection_invalid",
    });
  });

  test("rejects stale accepted command fences and incoherent outcomes", () => {
    const initial = base();

    const staleCommand = {
      ...initial,
      commands: [
        {
          command: {
            schema: "openagents.portable_session_command.v1" as const,
            commandRef: "command.checkpoint.stale",
            idempotencyKey: "idempotency.checkpoint.stale",
            ownerRef: "owner.mobile",
            sessionRef,
            kind: "checkpoint" as const,
            expectedAttachmentRef: "attachment.mobile.1",
            expectedGeneration: 1,
            checkpointRef: "checkpoint.stale",
            expiresAt: "2026-07-17T12:01:00.000Z",
          },
          status: "accepted" as const,
        },
      ],
    };
    expect(projectMobilePortableSessionControl(staleCommand, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "projection_invalid",
    });

    const completed = {
      ...initial,
      commands: [
        {
          command: {
            schema: "openagents.portable_session_command.v1" as const,
            commandRef: "command.checkpoint.completed",
            idempotencyKey: "idempotency.checkpoint.completed",
            ownerRef: "owner.mobile",
            sessionRef,
            kind: "checkpoint" as const,
            expectedAttachmentRef: "attachment.mobile.2",
            expectedGeneration: 2,
            checkpointRef: "checkpoint.completed",
            expiresAt: "2026-07-17T12:01:00.000Z",
          },
          outcome: {
            commandRef: "command.checkpoint.completed",
            sessionRef,
            status: "completed" as const,
            sourceAttachmentRef: "attachment.mobile.1",
            sourceGeneration: 1,
            checkpointRef: "checkpoint.completed",
            evidenceRefs: ["receipt.command.completed"],
          },
        },
      ],
    };
    expect(projectMobilePortableSessionControl(completed, sessionRef)).toMatchObject({
      state: "unavailable",
      reason: "projection_invalid",
    });
  });

  test("revalidates pending state and canonical invocation bytes at command build", () => {
    const ready = projectMobilePortableSessionControl(base(), sessionRef);
    if (ready.state !== "ready") throw new Error("expected ready control");
    const forgedPendingControl = {
      ...ready,
      pendingLocalCommandCount: 1,
    };
    expect(
      buildMobilePortableSessionCommand({
        control: forgedPendingControl,
        action: "stop",
        invocationRef: "tap.pending.forged",
        issuedAt: "2026-07-17T12:00:00.000Z",
      }),
    ).toEqual({ state: "rejected", reason: "invalid_invocation" });

    for (const invocation of [
      { invocationRef: " tap.noncanonical", issuedAt: "2026-07-17T12:00:00.000Z" },
      { invocationRef: "tap.noncanonical", issuedAt: "2026-07-17T12:00:00Z" },
    ]) {
      expect(
        buildMobilePortableSessionCommand({
          control: ready,
          action: "stop",
          ...invocation,
        }),
      ).toEqual({ state: "rejected", reason: "invalid_invocation" });
    }
  });

  test("rejects path, credential, and native-handle shaped facts without reflecting them", () => {
    const initial = base();
    const unsafeSnapshots: ReadonlyArray<ConfirmedPortableSessionSnapshot> = [
      { ...initial, sessions: [{ ...initial.sessions[0]!, ownerRef: "/Users/private-owner" }] },
      {
        ...initial,
        targetDirectories: [
          {
            ...initial.targetDirectories[0]!,
            targets: [
              {
                ...initial.targetDirectories[0]!.targets[0]!,
                adapterRef: "github_pat_private-value",
              },
            ],
          },
        ],
      },
      {
        ...initial,
        attachments: [
          {
            ...initial.attachments[0]!,
            evidenceRefs: ["pid:12345"],
          },
        ],
      },
    ];
    for (const unsafe of unsafeSnapshots) {
      const serialized = JSON.stringify(projectMobilePortableSessionControl(unsafe, sessionRef));
      expect(serialized).toContain('"reason":"projection_invalid"');
      expect(serialized).not.toContain("/Users/");
      expect(serialized).not.toContain("github_pat_");
      expect(serialized).not.toContain("pid:12345");
    }
  });
});
