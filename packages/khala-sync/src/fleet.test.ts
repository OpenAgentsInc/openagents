import { describe, expect, test } from "bun:test"
import {
  canonicalJson,
  decodeFleetAccountEntity,
  decodeFleetApprovalEntity,
  decodeFleetAssignmentEntity,
  decodeFleetAttemptEntity,
  decodeFleetCommandOutcomeEntity,
  decodeFleetInboxFlagEntity,
  decodeFleetRunEntity,
  decodeFleetWorkUnitEntity,
  decodeFleetWorkerEntity,
  encodeFleetCommandOutcomeEntity,
  encodeFleetInboxFlagEntity,
  encodeFleetRunEntity,
  FLEET_ENTITY_TYPES,
} from "./index.js"

/**
 * Fleet entity contracts (KS-6.1). The load-bearing property here is
 * SPEC §7 invariant 9: these shapes must structurally REFUSE raw private
 * material — emails, filesystem paths, bearer strings — so a redaction bug
 * upstream fails to decode instead of replicating.
 */

const FORBIDDEN = /token|apiKey|authorization|\/Users\//i

const validRun = {
  counters: {
    activeAssignments: 2,
    blockedAssignments: 0,
    completedAssignments: 3,
    failedAssignments: 1,
    workUnitsTotal: 10,
  },
  desiredSlots: 4,
  runId: "fleet-run.pylon.supervisor.abc123",
  startedAt: "2026-07-04T15:00:00.000Z",
  status: "running",
  updatedAt: "2026-07-04T15:20:11.412Z",
  workerKind: "codex",
}

describe("fleet entity contracts", () => {
  test("entity type names are the closed public set", () => {
    expect([...FLEET_ENTITY_TYPES]).toEqual([
      "fleet_run",
      "fleet_worker",
      "fleet_assignment",
      "fleet_work_unit",
      "fleet_attempt",
      "fleet_account",
      "fleet_inbox_flag",
      "fleet_approval",
      "fleet_steer",
      "fleet_command_outcome",
    ])
  })

  test("fleet work units keep stable plan identity across attempt retries", () => {
    expect(
      decodeFleetWorkUnitEntity({
        workUnitRef: "unit-a",
        issueRef: "#8639",
        dependsOnRefs: ["unit-bootstrap"],
        state: "succeeded",
        latestAttemptRef: "work_claim.unit-a.attempt-2",
        acceptedAttemptRef: "work_claim.unit-a.attempt-2",
        updatedAt: "2026-07-09T23:00:04.000Z",
      }),
    ).toMatchObject({
      workUnitRef: "unit-a",
      acceptedAttemptRef: "work_claim.unit-a.attempt-2",
    })
    expect(() =>
      decodeFleetWorkUnitEntity({
        workUnitRef: "unit-a",
        issueRef: null,
        dependsOnRefs: [],
        state: "planned",
        latestAttemptRef: "work_claim.invented",
        acceptedAttemptRef: null,
        updatedAt: "2026-07-09T23:00:04.000Z",
      }),
    ).toThrow()
  })

  test("fleet attempts require exact evidence before success", () => {
    const succeeded = {
      attemptRef: "work_claim.unit-a.attempt-2",
      workUnitRef: "unit-a",
      intakeClaimRef: `claim.sarah_fleet_run.${"a".repeat(24)}`,
      pylonRef: "pylon-owner-1",
      workerKind: "codex",
      state: "succeeded",
      progressClass: "terminal",
      assignmentRef: null,
      accountRefHash: `account.pylon.codex.${"b".repeat(24)}`,
      capacityClass: "owner_local",
      marginalCostClass: "subscription",
      verification: {
        truth: "passed",
        verifierRef: "verifier.bun-test.1",
        evidenceRefs: ["test.run.unit-a.1"],
      },
      artifactRefs: ["artifact.patch.unit-a.1"],
      proofRefs: ["proof.unit-a.1"],
      authorityReceiptRefs: ["receipt.authority.unit-a.1"],
      closeoutRef: "closeout.unit-a.1",
      usageEvidence: {
        schema: "openagents.pylon.fleet_run_usage_evidence.v1",
        truth: "exact",
        harnessKind: "codex",
        evidenceRef: "evidence.public.pylon.fleet_run.exact.1",
        assignmentRef: "assignment.unit-a.edge-1",
        pylonRef: "pylon-owner-1",
        provider: "pylon-codex-own-capacity",
        model: "openagents/pylon-codex",
        demandKind: "own_capacity",
        demandSource: "khala_coding_delegation",
        inputTokens: 8,
        outputTokens: 5,
        reasoningTokens: 2,
        cacheReadTokens: 3,
        totalTokens: 13,
        tokenRows: 1,
        tokenUsageRefs: ["usage_row.unit-a.1"],
        proofRefs: ["proof.usage.unit-a.1"],
        closeoutChecklistRefs: ["check.closeout.unit-a.1"],
        proofChecklistRefs: ["check.proof.unit-a.1"],
      },
      blockerRefs: [],
      lastEventRef: `event.pylon.fleet_run.${"c".repeat(24)}`,
      startedAt: "2026-07-09T23:00:01.000Z",
      lastObservedAt: "2026-07-09T23:00:03.000Z",
      remoteObservedAt: "2026-07-09T23:00:02.000Z",
      terminalAt: "2026-07-09T23:00:04.000Z",
      updatedAt: "2026-07-09T23:00:04.000Z",
    } as const
    expect(decodeFleetAttemptEntity(succeeded).attemptRef).toBe(
      succeeded.attemptRef,
    )
    expect(() =>
      decodeFleetAttemptEntity({ ...succeeded, proofRefs: [] }),
    ).toThrow()
    expect(() =>
      decodeFleetAttemptEntity({
        ...succeeded,
        usageEvidence: {
          ...succeeded.usageEvidence,
          reasoningTokens: succeeded.usageEvidence.outputTokens + 1,
        },
      }),
    ).toThrow()
    expect(() =>
      decodeFleetAttemptEntity({
        ...succeeded,
        usageEvidence: {
          ...succeeded.usageEvidence,
          totalTokens: 1,
        },
      }),
    ).toThrow()
    expect(() =>
      decodeFleetAttemptEntity({
        ...succeeded,
        workerKind: "claude",
      }),
    ).toThrow()
    expect(() =>
      decodeFleetAttemptEntity({
        ...succeeded,
        assignmentRef: "/Users/operator/worktree",
      }),
    ).toThrow()
    expect(() =>
      decodeFleetAttemptEntity({ ...succeeded, rawPrompt: "private" }),
    ).toThrow()
  })

  test("fleet approvals preserve legacy decoding and require complete exact bindings", () => {
    const legacy = decodeFleetApprovalEntity({
      approvalRef: "approval.fc3.legacy",
      status: "pending",
      workerId: "worker.fc3.legacy",
      toolClass: "bash",
      openedAt: "2026-07-09T23:00:00.000Z",
      updatedAt: "2026-07-09T23:00:00.000Z",
    })
    expect(legacy).not.toHaveProperty("runRef")

    const bound = {
      ...legacy,
      approvalRef: "approval.fc3.bound",
      runRef: "fleet_run.sarah.0123456789abcdef0123",
      workUnitRef: "unit-a",
      attemptRef: "work_claim.unit-a.attempt-1",
      assignmentRef: null,
      accountRefHash: null,
      requestEventRef: `event.pylon.fleet_run.${"a".repeat(24)}`,
    }
    expect(decodeFleetApprovalEntity(bound)).toMatchObject({
      attemptRef: bound.attemptRef,
      assignmentRef: null,
      accountRefHash: null,
    })
    expect(() =>
      decodeFleetApprovalEntity({ ...bound, requestEventRef: undefined }),
    ).toThrow()
    expect(() =>
      decodeFleetApprovalEntity({
        ...bound,
        rawPrompt: "PRIVATE-SENTINEL-MUST-NOT-DECODE",
      }),
    ).toThrow()
    expect(() =>
      decodeFleetApprovalEntity({
        ...bound,
        workerId: "/Users/operator/private-worker",
      }),
    ).toThrow()
  })

  test("fleet_command_outcome separates delivery from effective state", () => {
    const entity = decodeFleetCommandOutcomeEntity({
      intentId: "intent.sarah.pause.1",
      seq: 41,
      kind: "fleet_run_control",
      targetRef: "fleet_run.sarah.0123456789abcdef0123",
      deliveryOutcome: "applied",
      completionOutcome: "applied",
      effectiveOutcome: "paused",
      completionRef: "outcome.pylon.fleet_steering.d93f26d5c3e00b404336608a",
      completedAt: "2026-07-09T23:00:02.000Z",
      outcomeRef: "outcome.pylon.fleet_steering.d93f26d5c3e00b404336608a",
      observedAt: "2026-07-09T23:00:01.000Z",
      recordedAt: "2026-07-09T23:00:02.000Z",
      updatedAt: "2026-07-09T23:00:02.000Z",
    })
    expect(entity.effectiveOutcome).toBe("paused")
    expect(canonicalJson(encodeFleetCommandOutcomeEntity(entity))).not.toMatch(
      FORBIDDEN,
    )
  })

  test("fleet_command_outcome refuses private targets and invented effective states", () => {
    const valid = {
      intentId: "intent.sarah.steer.1",
      seq: 42,
      kind: "steer_message",
      targetRef: null,
      deliveryOutcome: "queued_follow_up",
      completionOutcome: null,
      effectiveOutcome: null,
      completionRef: null,
      completedAt: null,
      outcomeRef: "outcome.pylon.fleet_steering.0123456789abcdef01234567",
      observedAt: "2026-07-09T23:00:01.000Z",
      recordedAt: "2026-07-09T23:00:02.000Z",
      updatedAt: "2026-07-09T23:00:02.000Z",
    }
    expect(decodeFleetCommandOutcomeEntity(valid).effectiveOutcome).toBeNull()
    expect(
      decodeFleetCommandOutcomeEntity({
        ...valid,
        completionOutcome: "applied",
        effectiveOutcome: "steer_delivered",
        completionRef:
          "completion.pylon.fleet_steering.abcdef0123456789abcdef01",
        completedAt: "2026-07-09T23:00:03.000Z",
      }),
    ).toMatchObject({
      deliveryOutcome: "queued_follow_up",
      outcomeRef: valid.outcomeRef,
      effectiveOutcome: "steer_delivered",
      completionOutcome: "applied",
      completionRef:
        "completion.pylon.fleet_steering.abcdef0123456789abcdef01",
    })
    expect(() =>
      decodeFleetCommandOutcomeEntity({
        ...valid,
        targetRef: "/Users/alice/private-turn",
      }),
    ).toThrow()
    expect(() =>
      decodeFleetCommandOutcomeEntity({
        ...valid,
        effectiveOutcome: "probably_applied",
      }),
    ).toThrow()
    expect(() =>
      decodeFleetCommandOutcomeEntity({
        ...valid,
        completionOutcome: "applied",
        effectiveOutcome: "steer_delivered",
        completionRef: "worker.steering.not-a-completion-receipt",
        completedAt: "2026-07-09T23:00:03.000Z",
      }),
    ).toThrow()
    expect(() =>
      decodeFleetCommandOutcomeEntity({
        ...valid,
        deliveryOutcome: "rejected",
        completionOutcome: "applied",
        effectiveOutcome: "steer_delivered",
        completionRef:
          "completion.pylon.fleet_steering.abcdef0123456789abcdef01",
        completedAt: "2026-07-09T23:00:03.000Z",
      }),
    ).toThrow()
    expect(() =>
      decodeFleetCommandOutcomeEntity({
        ...valid,
        completionOutcome: "failed",
        completionRef: null,
        completedAt: "2026-07-09T23:00:03.000Z",
      }),
    ).toThrow()
  })

  test("fleet_run decodes and re-encodes canonically", () => {
    const entity = decodeFleetRunEntity(validRun)
    expect(entity.status).toBe("running")
    expect(canonicalJson(encodeFleetRunEntity(entity))).not.toMatch(FORBIDDEN)
  })

  test("fleet_run rejects out-of-range desiredSlots and unknown status", () => {
    expect(() =>
      decodeFleetRunEntity({ ...validRun, desiredSlots: -1 }),
    ).toThrow()
    expect(() =>
      decodeFleetRunEntity({ ...validRun, desiredSlots: 5000 }),
    ).toThrow()
    expect(() =>
      decodeFleetRunEntity({ ...validRun, status: "exploded" }),
    ).toThrow()
  })

  test("refs structurally refuse emails, paths, and whitespace", () => {
    // runId with a filesystem path
    expect(() =>
      decodeFleetRunEntity({ ...validRun, runId: "/Users/alice/run" }),
    ).toThrow()
    // workerId with an email
    expect(() =>
      decodeFleetWorkerEntity({
        phase: "idle",
        updatedAt: "2026-07-04T15:20:11.412Z",
        workerId: "alice@example.com",
      }),
    ).toThrow()
    // assignmentRef with whitespace
    expect(() =>
      decodeFleetAssignmentEntity({
        assignmentRef: "assignment with spaces",
        status: "offered",
        updatedAt: "2026-07-04T15:20:11.412Z",
      }),
    ).toThrow()
  })

  test("accountRefHash accepts ONLY the public hash-ref shape", () => {
    const base = {
      readiness: "ready",
      updatedAt: "2026-07-04T15:20:11.412Z",
    }
    expect(
      decodeFleetAccountEntity({
        ...base,
        accountRefHash: "account.pylon.codex.4e5f6a7b8c9d0e1f2a3b4c5d",
      }).readiness,
    ).toBe("ready")
    // raw email — refused
    expect(() =>
      decodeFleetAccountEntity({
        ...base,
        accountRefHash: "alice@example.com",
      }),
    ).toThrow()
    // non-hex tail (a raw handle, not a digest) — refused
    expect(() =>
      decodeFleetAccountEntity({
        ...base,
        accountRefHash: "account.pylon.codex.alice_handle",
      }),
    ).toThrow()
    // home path — refused
    expect(() =>
      decodeFleetAccountEntity({
        ...base,
        accountRefHash: "/Users/alice/.codex/auth.json",
      }),
    ).toThrow()
  })

  test("fleet_assignment issueRef accepts #N and owner/repo#N only", () => {
    const base = {
      assignmentRef: "assignment.public.issue8302.1",
      status: "offered",
      updatedAt: "2026-07-04T15:20:11.412Z",
    }
    expect(
      decodeFleetAssignmentEntity({ ...base, issueRef: "#8302" }).issueRef,
    ).toBe("#8302")
    expect(
      decodeFleetAssignmentEntity({
        ...base,
        issueRef: "OpenAgentsInc/openagents#8302",
      }).issueRef,
    ).toBe("OpenAgentsInc/openagents#8302")
    expect(() =>
      decodeFleetAssignmentEntity({
        ...base,
        issueRef: "https://github.com/OpenAgentsInc/openagents/issues/8302",
      }),
    ).toThrow()
  })

  test("fleet_worker accepts the operator-desired paused phase", () => {
    expect(
      decodeFleetWorkerEntity({
        phase: "paused",
        updatedAt: "2026-07-04T15:20:11.412Z",
        workerId: "dispatch-context.pylon.supervisor.9ab31c44",
      }).phase,
    ).toBe("paused")
  })

  test("fleet_inbox_flag decodes, and refs/kinds refuse private material", () => {
    const validFlag = {
      acknowledgedAt: "2026-07-04T15:21:00.000Z",
      flagRef: "inbox-flag.run_blocked.4f2a9c1d",
      kind: "run_blocked",
      openedAt: "2026-07-04T15:18:02.000Z",
      status: "acknowledged",
      updatedAt: "2026-07-04T15:21:00.000Z",
    }
    const entity = decodeFleetInboxFlagEntity(validFlag)
    expect(entity.status).toBe("acknowledged")
    expect(canonicalJson(encodeFleetInboxFlagEntity(entity))).not.toMatch(
      FORBIDDEN,
    )
    // flagRef with a filesystem path — refused
    expect(() =>
      decodeFleetInboxFlagEntity({
        ...validFlag,
        flagRef: "/Users/alice/inbox",
      }),
    ).toThrow()
    // flagRef with an email — refused
    expect(() =>
      decodeFleetInboxFlagEntity({
        ...validFlag,
        flagRef: "alice@example.com",
      }),
    ).toThrow()
    // kind must be a bounded lower_snake_case token
    expect(() =>
      decodeFleetInboxFlagEntity({ ...validFlag, kind: "Run Blocked!" }),
    ).toThrow()
    // status is a closed set
    expect(() =>
      decodeFleetInboxFlagEntity({ ...validFlag, status: "dismissed" }),
    ).toThrow()
  })

  test("timestamps must be ISO-8601 UTC", () => {
    expect(() =>
      decodeFleetRunEntity({ ...validRun, updatedAt: "yesterday" }),
    ).toThrow()
    expect(() =>
      decodeFleetRunEntity({
        ...validRun,
        updatedAt: "2026-07-04T15:20:11+02:00",
      }),
    ).toThrow()
  })
})
