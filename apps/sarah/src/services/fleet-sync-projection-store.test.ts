import { describe, expect, test } from "bun:test"
import {
  FLEET_ACCOUNT_ENTITY_TYPE,
  FLEET_APPROVAL_ENTITY_TYPE,
  FLEET_ASSIGNMENT_ENTITY_TYPE,
  FLEET_COMMAND_OUTCOME_ENTITY_TYPE,
  FLEET_INBOX_FLAG_ENTITY_TYPE,
  FLEET_RUN_ENTITY_TYPE,
  FLEET_STEER_ENTITY_TYPE,
  FLEET_WORKER_ENTITY_TYPE,
  KHALA_SYNC_PROTOCOL_VERSION,
  canonicalJson,
  decodeBootstrapResponse,
  decodeFleetAccountEntity,
  decodeFleetApprovalEntity,
  decodeFleetAssignmentEntity,
  decodeFleetCommandOutcomeEntity,
  decodeFleetInboxFlagEntity,
  decodeFleetRunEntity,
  decodeFleetSteerEntity,
  decodeFleetWorkerEntity,
  decodeLogPage,
  encodeFleetAccountEntity,
  encodeFleetApprovalEntity,
  encodeFleetAssignmentEntity,
  encodeFleetCommandOutcomeEntity,
  encodeFleetInboxFlagEntity,
  encodeFleetRunEntity,
  encodeFleetSteerEntity,
  encodeFleetWorkerEntity,
  type BootstrapResponse,
  type LogPage,
} from "@openagentsinc/khala-sync"

import {
  SARAH_FLEET_CURSOR_STATE_SCHEMA,
  type SarahFleetSyncClient,
  type SarahFleetSyncCursorState,
} from "./fleet-sync-client.ts"
import {
  SARAH_FLEET_PROJECTION_STATE_SCHEMA,
  MAX_SARAH_FLEET_PROJECTION_ENTITIES,
  MAX_SARAH_FLEET_PROJECTION_STATE_BYTES,
  SarahFleetProjectionReducerError,
  decodeSarahFleetProjectionState,
  makeSarahFleetProjectionStore,
  projectSarahFleetProjectionState,
  reduceSarahFleetBootstrapPages,
  reduceSarahFleetLogPages,
  type SarahFleetProjectionState,
} from "./fleet-sync-projection-store.ts"

const NOW = Date.parse("2026-07-09T20:00:00.000Z")
const scope = "scope.fleet_run.fleet.run.fc3.reducer"
const runRef = "fleet.run.fc3.reducer"
const foreignScope = "scope.fleet_run.fleet.run.other-owner"

const run = decodeFleetRunEntity({
  runId: runRef,
  status: "running",
  desiredSlots: 3,
  workerKind: "auto",
  startedAt: "2026-07-09T19:50:00.000Z",
  counters: {
    workUnitsTotal: 1,
    activeAssignments: 1,
    completedAssignments: 0,
    failedAssignments: 0,
    blockedAssignments: 1,
  },
  updatedAt: "2026-07-09T19:59:58.000Z",
})

const worker = decodeFleetWorkerEntity({
  workerId: "worker.fc3.codex",
  phase: "blocked",
  harnessKind: "codex",
  assignmentRef: "assignment.fc3.codex",
  accountRefHash: "account.pylon.codex.11111111",
  lastProgressAt: "2026-07-09T19:59:45.000Z",
  updatedAt: "2026-07-09T19:59:45.000Z",
})

const assignment = decodeFleetAssignmentEntity({
  assignmentRef: "assignment.fc3.codex",
  issueRef: "#8639",
  status: "running",
  updatedAt: "2026-07-09T19:59:45.000Z",
})

const approval = decodeFleetApprovalEntity({
  approvalRef: "approval.fc3.codex",
  status: "pending",
  workerId: worker.workerId,
  toolClass: "write_file",
  openedAt: "2026-07-09T19:59:45.000Z",
  updatedAt: "2026-07-09T19:59:45.000Z",
})

const inboxFlag = decodeFleetInboxFlagEntity({
  flagRef: "flag.fc3.approval",
  kind: "approval_required",
  status: "open",
  openedAt: "2026-07-09T19:59:45.000Z",
  updatedAt: "2026-07-09T19:59:45.000Z",
})

const account = decodeFleetAccountEntity({
  accountRefHash: "account.pylon.codex.11111111",
  readiness: "ready",
  provider: "codex",
  capacityAvailable: 1,
  capacityBusy: 0,
  capacityQueued: 0,
  updatedAt: "2026-07-09T19:59:45.000Z",
})

const steer = decodeFleetSteerEntity({
  steerRef: "steer.fc3.codex",
  targetRef: worker.workerId,
  bodyCarrier: "ref",
  createdAt: "2026-07-09T19:59:50.000Z",
  updatedAt: "2026-07-09T19:59:50.000Z",
})

const commandOutcome = decodeFleetCommandOutcomeEntity({
  intentId: "intent.fc3.pause",
  seq: 41,
  kind: "fleet_run_control",
  targetRef: runRef,
  deliveryOutcome: "applied",
  effectiveOutcome: "paused",
  completionRef: "outcome.pylon.fleet_steering.d93f26d5c3e00b404336608a",
  completedAt: "2026-07-09T19:59:55.000Z",
  outcomeRef: "outcome.pylon.fleet_steering.d93f26d5c3e00b404336608a",
  observedAt: "2026-07-09T19:59:54.000Z",
  recordedAt: "2026-07-09T19:59:55.000Z",
  updatedAt: "2026-07-09T19:59:55.000Z",
})

type BootstrapEntityInput = Readonly<{
  entityType: string
  entityId: string
  postImageJson: string
}>

const entity = (
  entityType: string,
  entityId: string,
  encoded: unknown,
): BootstrapEntityInput => ({
  entityType,
  entityId,
  postImageJson: canonicalJson(encoded),
})

const baseEntities: ReadonlyArray<BootstrapEntityInput> = [
  entity(FLEET_RUN_ENTITY_TYPE, run.runId, encodeFleetRunEntity(run)),
  entity(
    FLEET_WORKER_ENTITY_TYPE,
    worker.workerId,
    encodeFleetWorkerEntity(worker),
  ),
  entity(
    FLEET_ASSIGNMENT_ENTITY_TYPE,
    assignment.assignmentRef,
    encodeFleetAssignmentEntity(assignment),
  ),
  entity(
    FLEET_APPROVAL_ENTITY_TYPE,
    approval.approvalRef,
    encodeFleetApprovalEntity(approval),
  ),
  entity(
    FLEET_INBOX_FLAG_ENTITY_TYPE,
    inboxFlag.flagRef,
    encodeFleetInboxFlagEntity(inboxFlag),
  ),
  entity(
    FLEET_ACCOUNT_ENTITY_TYPE,
    account.accountRefHash,
    encodeFleetAccountEntity(account),
  ),
  entity(FLEET_STEER_ENTITY_TYPE, steer.steerRef, encodeFleetSteerEntity(steer)),
  entity(
    FLEET_COMMAND_OUTCOME_ENTITY_TYPE,
    commandOutcome.intentId,
    encodeFleetCommandOutcomeEntity(commandOutcome),
  ),
]

const bootstrapPage = (
  entities: ReadonlyArray<BootstrapEntityInput>,
  tail: Readonly<{ cursor: number } | { nextPageToken: string }>,
  pageScope = scope,
): BootstrapResponse =>
  decodeBootstrapResponse({
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    scope: pageScope,
    entities,
    ...tail,
  })

const bootstrapPages = (): ReadonlyArray<BootstrapResponse> => [
  bootstrapPage(baseEntities.slice(0, 3), { nextPageToken: "page-two" }),
  bootstrapPage(baseEntities.slice(3), { cursor: 10 }),
]

type LogEntryInput = Readonly<{
  scope?: string
  version: number
  entityType: string
  entityId: string
  op: "upsert" | "delete"
  postImageJson?: string
  mutationRef?: string
  committedAt: string
}>

const logPage = (
  entries: ReadonlyArray<LogEntryInput>,
  nextCursor: number,
  upToDate = true,
  pageScope = scope,
): LogPage =>
  decodeLogPage({
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    scope: pageScope,
    entries: entries.map((entry) => ({ scope: pageScope, ...entry })),
    nextCursor,
    upToDate,
  })

const changedWorker = decodeFleetWorkerEntity({
  ...worker,
  phase: "completed",
  updatedAt: "2026-07-09T20:00:01.000Z",
})

const deleteAndCompletePage = (): LogPage =>
  logPage(
    [
      {
        version: 11,
        entityType: FLEET_ASSIGNMENT_ENTITY_TYPE,
        entityId: assignment.assignmentRef,
        op: "delete",
        committedAt: "2026-07-09T20:00:01.000Z",
      },
      {
        version: 11,
        entityType: FLEET_WORKER_ENTITY_TYPE,
        entityId: worker.workerId,
        op: "upsert",
        postImageJson: canonicalJson(encodeFleetWorkerEntity(changedWorker)),
        committedAt: "2026-07-09T20:00:01.000Z",
      },
    ],
    11,
  )

const cursorState = (cursor: number): SarahFleetSyncCursorState => ({
  schema: SARAH_FLEET_CURSOR_STATE_SCHEMA,
  scope: scope as SarahFleetSyncCursorState["scope"],
  cursor: cursor as SarahFleetSyncCursorState["cursor"],
})

const expectReason = (
  thunk: () => unknown,
  reason: SarahFleetProjectionReducerError["reason"],
): void => {
  let caught: unknown
  try {
    thunk()
  } catch (error) {
    caught = error
  }
  expect(caught).toBeInstanceOf(SarahFleetProjectionReducerError)
  expect(caught).toMatchObject({ reason })
}

describe("Sarah FC-3 fleet entity reducer", () => {
  test("reduces all bounded bootstrap pages and projects only decoded owner-safe entities", () => {
    const state = reduceSarahFleetBootstrapPages(bootstrapPages())

    expect(state).toMatchObject({
      schema: SARAH_FLEET_PROJECTION_STATE_SCHEMA,
      scope,
      cursor: 10,
    })
    expect(state.entities).toHaveLength(8)
    expect(state.entities.every((row) => row.version === 10)).toBe(true)
    expect(
      state.entities.map((row) => `${row.entityType}/${row.entityId}`),
    ).toEqual([...state.entities]
      .sort((left, right) =>
        left.entityType.localeCompare(right.entityType) ||
        left.entityId.localeCompare(right.entityId),
      )
      .map((row) => `${row.entityType}/${row.entityId}`))

    const projection = projectSarahFleetProjectionState(state, NOW)
    expect(projection.run.runRef).toBe(runRef)
    expect(projection.workUnits[0]?.assignmentRef).toBe(assignment.assignmentRef)
    expect(projection.workers[0]?.workerRef).toBe(worker.workerId)
    expect(projection.approvals[0]?.approvalRef).toBe(approval.approvalRef)
    expect(projection.commandOutcomes).toEqual([commandOutcome])
    expect(JSON.stringify(projection)).not.toMatch(
      /bodyCarrier|capacityAvailable|postImageJson/,
    )
  })

  test("honors deletes and replays the same version and images idempotently", () => {
    const bootstrapped = reduceSarahFleetBootstrapPages(bootstrapPages())
    const page = deleteAndCompletePage()
    const reduced = reduceSarahFleetLogPages(bootstrapped, [page])
    const replayed = reduceSarahFleetLogPages(reduced, [page])

    expect(replayed).toEqual(reduced)
    expect(Number(replayed.cursor)).toBe(11)
    expect(
      replayed.entities.find(
        (row) => row.entityId === assignment.assignmentRef,
      ),
    ).toMatchObject({ version: 11, postImageJson: null })
    expect(projectSarahFleetProjectionState(replayed, NOW).workUnits).toEqual([])
    expect(
      projectSarahFleetProjectionState(replayed, NOW).workers[0]?.phase,
    ).toBe("completed")
  })

  test("rejects conflicting duplicates and version regressions", () => {
    const bootstrapped = reduceSarahFleetBootstrapPages(bootstrapPages())
    const reduced = reduceSarahFleetLogPages(bootstrapped, [
      deleteAndCompletePage(),
    ])
    const conflictingWorker = decodeFleetWorkerEntity({
      ...changedWorker,
      phase: "failed",
    })

    expectReason(
      () =>
        reduceSarahFleetLogPages(reduced, [
          logPage(
            [
              {
                version: 11,
                entityType: FLEET_WORKER_ENTITY_TYPE,
                entityId: worker.workerId,
                op: "upsert",
                postImageJson: canonicalJson(
                  encodeFleetWorkerEntity(conflictingWorker),
                ),
                committedAt: "2026-07-09T20:00:01.000Z",
              },
            ],
            11,
          ),
        ]),
      "duplicate_conflict",
    )

    expectReason(
      () =>
        reduceSarahFleetLogPages(reduced, [
          logPage(
            [
              {
                version: 10,
                entityType: FLEET_WORKER_ENTITY_TYPE,
                entityId: worker.workerId,
                op: "upsert",
                postImageJson: canonicalJson(encodeFleetWorkerEntity(worker)),
                committedAt: "2026-07-09T19:59:45.000Z",
              },
            ],
            10,
          ),
        ]),
      "version_regression",
    )

    const olderWorkerState = decodeSarahFleetProjectionState({
      ...bootstrapped,
      entities: bootstrapped.entities.map((row) =>
        row.entityId === worker.workerId ? { ...row, version: 5 } : row,
      ),
    })
    expectReason(
      () =>
        reduceSarahFleetLogPages(olderWorkerState, [
          logPage(
            [
              {
                version: 8,
                entityType: FLEET_WORKER_ENTITY_TYPE,
                entityId: worker.workerId,
                op: "upsert",
                postImageJson: canonicalJson(
                  encodeFleetWorkerEntity(changedWorker),
                ),
                committedAt: "2026-07-09T20:00:00.000Z",
              },
              {
                version: 10,
                entityType: FLEET_ACCOUNT_ENTITY_TYPE,
                entityId: account.accountRefHash,
                op: "upsert",
                postImageJson: canonicalJson(encodeFleetAccountEntity(account)),
                committedAt: "2026-07-09T20:00:00.000Z",
              },
            ],
            10,
          ),
        ]),
      "version_regression",
    )

    expectReason(
      () =>
        reduceSarahFleetLogPages(bootstrapped, [
          logPage([], 9),
        ]),
      "version_regression",
    )

    expectReason(
      () =>
        reduceSarahFleetLogPages(bootstrapped, [
          logPage(
            [
              {
                version: 12,
                entityType: FLEET_ACCOUNT_ENTITY_TYPE,
                entityId: account.accountRefHash,
                op: "upsert",
                postImageJson: canonicalJson(encodeFleetAccountEntity(account)),
                committedAt: "2026-07-09T20:00:02.000Z",
              },
              {
                version: 11,
                entityType: FLEET_WORKER_ENTITY_TYPE,
                entityId: changedWorker.workerId,
                op: "upsert",
                postImageJson: canonicalJson(
                  encodeFleetWorkerEntity(changedWorker),
                ),
                committedAt: "2026-07-09T20:00:01.000Z",
              },
            ],
            11,
          ),
        ]),
      "version_regression",
    )

    const conflictingRun = decodeFleetRunEntity({ ...run, desiredSlots: 2 })
    expectReason(
      () =>
        reduceSarahFleetBootstrapPages([
          bootstrapPage(
            [
              ...baseEntities,
              entity(
                FLEET_RUN_ENTITY_TYPE,
                run.runId,
                encodeFleetRunEntity(conflictingRun),
              ),
            ],
            { cursor: 10 },
          ),
        ]),
      "duplicate_conflict",
    )
  })

  test("rejects cross-scope pages, unknown types, mismatched ids, and missing or multiple runs", () => {
    const state = reduceSarahFleetBootstrapPages(bootstrapPages())
    expectReason(
      () =>
        reduceSarahFleetLogPages(state, [
          logPage([], 10, true, foreignScope),
        ]),
      "foreign_scope",
    )
    expectReason(
      () =>
        reduceSarahFleetBootstrapPages([
          bootstrapPage(
            [
              ...baseEntities,
              entity("fleet_private", "private.fc3", { value: "hidden" }),
            ],
            { cursor: 10 },
          ),
        ]),
      "unknown_entity_type",
    )
    expectReason(
      () =>
        reduceSarahFleetBootstrapPages([
          bootstrapPage(
            [
              ...baseEntities.filter(
                (row) => row.entityType !== FLEET_WORKER_ENTITY_TYPE,
              ),
              entity(
                FLEET_WORKER_ENTITY_TYPE,
                "worker.fc3.wrong-key",
                encodeFleetWorkerEntity(worker),
              ),
            ],
            { cursor: 10 },
          ),
        ]),
      "entity_key_mismatch",
    )
    expectReason(
      () =>
        reduceSarahFleetBootstrapPages([
          bootstrapPage(
            baseEntities.filter(
              (row) => row.entityType !== FLEET_RUN_ENTITY_TYPE,
            ),
            { cursor: 10 },
          ),
        ]),
      "missing_run",
    )

    const otherRun = decodeFleetRunEntity({ ...run, runId: "fleet.run.extra" })
    expectReason(
      () =>
        reduceSarahFleetBootstrapPages([
          bootstrapPage(
            [
              ...baseEntities,
              entity(
                FLEET_RUN_ENTITY_TYPE,
                otherRun.runId,
                encodeFleetRunEntity(otherRun),
              ),
            ],
            { cursor: 10 },
          ),
        ]),
      "multiple_runs",
    )
  })

  test("rejects private or excess fields instead of relying on schema stripping", () => {
    const unsafeRun = {
      ...encodeFleetRunEntity(run),
      rawPrompt: "PRIVATE PROMPT SENTINEL",
    }
    let caught: unknown
    try {
      reduceSarahFleetBootstrapPages([
        bootstrapPage(
          [
            ...baseEntities.filter(
              (row) => row.entityType !== FLEET_RUN_ENTITY_TYPE,
            ),
            entity(FLEET_RUN_ENTITY_TYPE, run.runId, unsafeRun),
          ],
          { cursor: 10 },
        ),
      ])
    } catch (error) {
      caught = error
    }
    expect(caught).toMatchObject({ reason: "invalid_post_image" })
    expect(JSON.stringify(caught)).not.toContain("PRIVATE PROMPT SENTINEL")

    const unsafeCounters = {
      ...encodeFleetRunEntity(run),
      counters: {
        ...encodeFleetRunEntity(run).counters,
        privateOutput: "PRIVATE OUTPUT SENTINEL",
      },
    }
    expectReason(
      () =>
        reduceSarahFleetBootstrapPages([
          bootstrapPage(
            [
              ...baseEntities.filter(
                (row) => row.entityType !== FLEET_RUN_ENTITY_TYPE,
              ),
              entity(FLEET_RUN_ENTITY_TYPE, run.runId, unsafeCounters),
            ],
            { cursor: 10 },
          ),
        ]),
      "invalid_post_image",
    )
  })

  test("round-trips one strict serializable state and rejects excess persisted fields", () => {
    const state = reduceSarahFleetBootstrapPages(bootstrapPages())
    const reopened = decodeSarahFleetProjectionState(
      JSON.parse(JSON.stringify(state)) as unknown,
    )
    expect(reopened).toEqual(state)

    expectReason(
      () => decodeSarahFleetProjectionState({ ...state, privateState: true }),
      "invalid_state",
    )
    expectReason(
      () =>
        decodeSarahFleetProjectionState({
          ...state,
          entities: Array.from(
            { length: MAX_SARAH_FLEET_PROJECTION_ENTITIES + 1 },
            () => state.entities[0],
          ),
        }),
      "invalid_state",
    )
    expectReason(
      () =>
        decodeSarahFleetProjectionState({
          ...state,
          oversized: "x".repeat(MAX_SARAH_FLEET_PROJECTION_STATE_BYTES),
        }),
      "invalid_state",
    )
    expectReason(
      () =>
        decodeSarahFleetProjectionState({
          ...state,
          entities: [
            { ...state.entities[0], privateRow: true },
            ...state.entities.slice(1),
          ],
        }),
      "invalid_state",
    )
  })
})

describe("Sarah FC-3 persisted reconnect boundary", () => {
  test("serializes, reopens, resumes the exact cursor, and replays without latest", async () => {
    let persistedJson: string | null = null
    let bootstrapCalls = 0
    const resumeInputs: SarahFleetSyncCursorState[] = []
    let resumeCall = 0
    const client: Pick<SarahFleetSyncClient, "bootstrap" | "resume"> = {
      bootstrap: async (requestedScope) => {
        bootstrapCalls += 1
        expect(requestedScope).toBe(scope)
        return { pages: bootstrapPages(), state: cursorState(10) }
      },
      resume: async (state) => {
        resumeInputs.push(state)
        resumeCall += 1
        return resumeCall === 1
          ? { pages: [deleteAndCompletePage()], state: cursorState(11) }
          : { pages: [logPage([], 11)], state: cursorState(11) }
      },
    }
    const persistence = {
      load: async () =>
        persistedJson === null
          ? null
          : (JSON.parse(persistedJson) as unknown),
      save: async (state: SarahFleetProjectionState) => {
        persistedJson = JSON.stringify(state)
      },
    }
    const store = makeSarahFleetProjectionStore({
      client,
      persistence,
      now: () => NOW,
    })

    const first = await store.open(scope)
    const second = await store.open(scope)
    const third = await store.open(scope)

    expect(first.source).toBe("bootstrap")
    expect(second.source).toBe("resume")
    expect(third.source).toBe("resume")
    expect(bootstrapCalls).toBe(1)
    expect(resumeInputs).toEqual([cursorState(10), cursorState(11)])
    expect(Number(second.state.cursor)).toBe(11)
    expect(third.state).toEqual(second.state)
    expect(third.projection.run.runRef).toBe(runRef)
    expect(
      JSON.stringify({ resumeInputs, persistedJson }),
    ).not.toContain("latest")
  })

  test("refuses a persisted state from another owner scope before any client call", async () => {
    const persisted = reduceSarahFleetBootstrapPages(bootstrapPages())
    let called = false
    const store = makeSarahFleetProjectionStore({
      client: {
        bootstrap: async () => {
          called = true
          throw new Error("must not bootstrap")
        },
        resume: async () => {
          called = true
          throw new Error("must not resume")
        },
      },
      persistence: {
        load: async () => JSON.parse(JSON.stringify(persisted)) as unknown,
        save: async () => {},
      },
      now: () => NOW,
    })

    await expect(store.open(foreignScope)).rejects.toMatchObject({
      reason: "foreign_scope",
    })
    expect(called).toBe(false)
  })

  test("maps a throwing injected clock to a fixed public-safe failure", async () => {
    const store = makeSarahFleetProjectionStore({
      client: {
        bootstrap: async () => ({
          pages: bootstrapPages(),
          state: cursorState(10),
        }),
        resume: async () => {
          throw new Error("resume must not run")
        },
      },
      persistence: {
        load: async () => null,
        save: async () => {
          throw new Error("save must not run")
        },
      },
      now: () => {
        throw new Error("PRIVATE CLOCK FAILURE")
      },
    })

    const failure = await store.open(scope).catch((error) => error)
    expect(failure).toMatchObject({ reason: "projection_failed" })
    expect(JSON.stringify(failure)).not.toContain("PRIVATE CLOCK FAILURE")
  })
})
