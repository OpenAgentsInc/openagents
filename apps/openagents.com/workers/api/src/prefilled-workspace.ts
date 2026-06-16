import { Schema as S } from 'effect'

import { parseJsonWithSchema } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

// Prefilled project workspace primitive (Epic C / C1).
//
// A reusable onboarding unit a prospect, holder, or private team member is
// invited into instead of a blank chat: a named project + seeded grounded
// memory + 1-3 one-click starter accepted-outcome workflows + an intro receipt.
//
// ACCESS INVARIANT: public_safe rows keep the original public-source-only
// onboarding contract. private_team rows may carry private project material,
// but must be denied by default and projected only after active team membership
// gating. No secrets, credentials, raw prompts, wallet data, raw invite tokens,
// or individual people's names belong in public-safe rows or public artifacts.

export const WorkspaceStatus = S.Literals([
  'draft',
  'invited',
  'active',
  'archived',
])
export type WorkspaceStatus = typeof WorkspaceStatus.Type

export const WorkspaceAccessMode = S.Literals(['public_safe', 'private_team'])
export type WorkspaceAccessMode = typeof WorkspaceAccessMode.Type

export const StarterWorkflowStatus = S.Literals([
  'queued',
  'ready',
  'completed',
  'dismissed',
])
export type StarterWorkflowStatus = typeof StarterWorkflowStatus.Type

// A single seeded grounded-memory fact, always carrying its public provenance.
export const SeededMemoryEntry = S.Struct({
  label: S.String,
  value: S.String,
  // A public source ref (URL, public document ref, or "conversation" marker)
  // backing this fact. Provenance-first: nothing private until the holder
  // connects their own accounts.
  publicSourceRef: S.String,
})
export type SeededMemoryEntry = typeof SeededMemoryEntry.Type

// A queued / one-click-runnable starter accepted-outcome workflow scoped to the
// holder's stated need.
export const StarterWorkflow = S.Struct({
  title: S.String,
  description: S.String,
  // The accepted-outcome kind this workflow would produce (generic, e.g.
  // 'draft', 'campaign', 'landing_page', 'review_checklist').
  outcomeKind: S.String,
  status: StarterWorkflowStatus,
})
export type StarterWorkflow = typeof StarterWorkflow.Type

// The intro receipt: what we set up, and the (public) sources used.
export const IntroReceipt = S.Struct({
  summary: S.String,
  publicSourceRefs: S.Array(S.String),
})
export type IntroReceipt = typeof IntroReceipt.Type

export const PrefilledWorkspaceEngagement = S.Struct({
  invitedAt: S.NullOr(S.String),
  firstViewedAt: S.NullOr(S.String),
  firstClaimedAt: S.NullOr(S.String),
  firstRunAt: S.NullOr(S.String),
  lastViewedAt: S.NullOr(S.String),
  revisitCount: S.Number,
})
export type PrefilledWorkspaceEngagement =
  typeof PrefilledWorkspaceEngagement.Type

export type PrefilledWorkspaceRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export const systemPrefilledWorkspaceRuntime: PrefilledWorkspaceRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

// The full typed workspace record (operator view).
export type PrefilledWorkspaceRecord = Readonly<{
  accessMode: WorkspaceAccessMode
  id: string
  // The holder once they sign in (FK to users). Null until claimed.
  holderUserId: string | null
  // An opaque, generic prospect reference used to seed the workspace before the
  // holder signs in (no person/client names).
  holderRef: string
  projectName: string
  status: WorkspaceStatus
  seededMemory: ReadonlyArray<SeededMemoryEntry>
  starterWorkflows: ReadonlyArray<StarterWorkflow>
  introReceipt: IntroReceipt
  privateProjectId: string | null
  privateTeamId: string | null
  engagement: PrefilledWorkspaceEngagement
  createdAt: string
  updatedAt: string
}>

type WorkspaceRow = Readonly<{
  access_mode: WorkspaceAccessMode
  id: string
  holder_user_id: string | null
  holder_ref: string
  project_name: string
  status: WorkspaceStatus
  private_project_id: string | null
  private_team_id: string | null
  intro_receipt_json: string
  invited_at: string | null
  first_viewed_at: string | null
  first_claimed_at: string | null
  first_run_at: string | null
  last_viewed_at: string | null
  revisit_count: number
  created_at: string
  updated_at: string
}>

type SeededMemoryRow = Readonly<{
  workspace_id: string
  label: string
  value: string
  public_source_ref: string
}>

type StarterWorkflowRow = Readonly<{
  workspace_id: string
  title: string
  description: string
  outcome_kind: string
  status: StarterWorkflowStatus
}>

const clampText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const slugRef = (value: string): string =>
  clampText(value, 120)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 120)

const STARTER_WORKFLOW_LIMIT = 3
const SEEDED_MEMORY_LIMIT = 50

const normalizeSeededMemory = (
  entries: ReadonlyArray<SeededMemoryEntry>,
): ReadonlyArray<SeededMemoryEntry> =>
  entries.slice(0, SEEDED_MEMORY_LIMIT).map(entry => ({
    label: clampText(entry.label, 160),
    value: clampText(entry.value, 1000),
    publicSourceRef: clampText(entry.publicSourceRef, 500),
  }))

const normalizeStarterWorkflows = (
  workflows: ReadonlyArray<StarterWorkflow>,
): ReadonlyArray<StarterWorkflow> =>
  workflows.slice(0, STARTER_WORKFLOW_LIMIT).map(workflow => ({
    title: clampText(workflow.title, 200),
    description: clampText(workflow.description, 1000),
    outcomeKind: clampText(workflow.outcomeKind, 80),
    status: workflow.status,
  }))

const normalizeIntroReceipt = (receipt: IntroReceipt): IntroReceipt => ({
  summary: clampText(receipt.summary, 2000),
  publicSourceRefs: receipt.publicSourceRefs
    .slice(0, SEEDED_MEMORY_LIMIT)
    .map(ref => clampText(ref, 500)),
})

export type CreatePrefilledWorkspaceInput = Readonly<{
  accessMode?: WorkspaceAccessMode | undefined
  holderRef?: string | undefined
  holderUserId?: string | null | undefined
  introReceipt: IntroReceipt
  privateProjectId?: string | null | undefined
  privateTeamId?: string | null | undefined
  projectName: string
  seededMemory?: ReadonlyArray<SeededMemoryEntry> | undefined
  starterWorkflows?: ReadonlyArray<StarterWorkflow> | undefined
  status?: WorkspaceStatus | undefined
}>

export const makePrefilledWorkspaceRecord = (
  input: CreatePrefilledWorkspaceInput,
  runtime: PrefilledWorkspaceRuntime = systemPrefilledWorkspaceRuntime,
): PrefilledWorkspaceRecord => {
  const now = runtime.nowIso()
  const projectName = clampText(input.projectName, 200)
  const holderRef =
    input.holderRef === undefined || input.holderRef.trim() === ''
      ? `workspace-${slugRef(projectName)}-${runtime.makeId('h')}`
      : clampText(input.holderRef, 200)

  return {
    accessMode: input.accessMode ?? 'public_safe',
    id: runtime.makeId('workspace'),
    holderUserId: input.holderUserId ?? null,
    holderRef,
    projectName,
    status: input.status ?? 'draft',
    seededMemory: normalizeSeededMemory(input.seededMemory ?? []),
    starterWorkflows: normalizeStarterWorkflows(input.starterWorkflows ?? []),
    introReceipt: normalizeIntroReceipt(input.introReceipt),
    privateProjectId: input.privateProjectId ?? null,
    privateTeamId: input.privateTeamId ?? null,
    engagement: {
      invitedAt: (input.status ?? 'draft') === 'invited' ? now : null,
      firstViewedAt: null,
      firstClaimedAt: null,
      firstRunAt: null,
      lastViewedAt: null,
      revisitCount: 0,
    },
    createdAt: now,
    updatedAt: now,
  }
}

const parseIntroReceipt = (json: string): IntroReceipt => {
  try {
    return parseJsonWithSchema(IntroReceipt, json)
  } catch {
    return { summary: '', publicSourceRefs: [] }
  }
}

const recordFromRows = (
  workspace: WorkspaceRow,
  memoryRows: ReadonlyArray<SeededMemoryRow>,
  workflowRows: ReadonlyArray<StarterWorkflowRow>,
): PrefilledWorkspaceRecord => ({
  accessMode: workspace.access_mode,
  id: workspace.id,
  holderUserId: workspace.holder_user_id,
  holderRef: workspace.holder_ref,
  projectName: workspace.project_name,
  status: workspace.status,
  seededMemory: memoryRows
    .filter(row => row.workspace_id === workspace.id)
    .map(row => ({
      label: row.label,
      value: row.value,
      publicSourceRef: row.public_source_ref,
    })),
  starterWorkflows: workflowRows
    .filter(row => row.workspace_id === workspace.id)
    .map(row => ({
      title: row.title,
      description: row.description,
      outcomeKind: row.outcome_kind,
      status: row.status,
    })),
  introReceipt: parseIntroReceipt(workspace.intro_receipt_json),
  privateProjectId: workspace.private_project_id,
  privateTeamId: workspace.private_team_id,
  engagement: {
    invitedAt: workspace.invited_at,
    firstViewedAt: workspace.first_viewed_at,
    firstClaimedAt: workspace.first_claimed_at,
    firstRunAt: workspace.first_run_at,
    lastViewedAt: workspace.last_viewed_at,
    revisitCount: workspace.revisit_count,
  },
  createdAt: workspace.created_at,
  updatedAt: workspace.updated_at,
})

// Holder-facing projection. For public_safe rows, the data remains
// public-source-only. For private_team rows, callers must enforce active team
// membership before returning this projection. It always drops operator-only
// bindings and private team/project refs while keeping seeded-memory provenance
// refs intact.
export type PrefilledWorkspacePublicProjection = Readonly<{
  accessMode: WorkspaceAccessMode
  id: string
  projectName: string
  status: WorkspaceStatus
  seededMemory: ReadonlyArray<SeededMemoryEntry>
  starterWorkflows: ReadonlyArray<StarterWorkflow>
  introReceipt: IntroReceipt
  engagement: PrefilledWorkspaceEngagement
}>

export const toPublicProjection = (
  record: PrefilledWorkspaceRecord,
): PrefilledWorkspacePublicProjection => ({
  accessMode: record.accessMode,
  id: record.id,
  projectName: record.projectName,
  status: record.status,
  seededMemory: record.seededMemory,
  starterWorkflows: record.starterWorkflows,
  introReceipt: record.introReceipt,
  engagement: record.engagement,
})

export type PrefilledWorkspaceServiceShape = Readonly<{
  createWorkspace: (
    input: CreatePrefilledWorkspaceInput,
  ) => Promise<PrefilledWorkspaceRecord>
  readWorkspace: (
    workspaceId: string,
  ) => Promise<PrefilledWorkspaceRecord | undefined>
  // Holder-scoped read: returns the workspace only if it is bound to the given
  // holderUserId. Used by the signed-in holder path.
  readWorkspaceForHolder: (
    workspaceId: string,
    holderUserId: string,
  ) => Promise<PrefilledWorkspaceRecord | undefined>
  readOrClaimWorkspaceForHolder: (
    workspaceId: string,
    holderUserId: string,
  ) => Promise<PrefilledWorkspaceRecord | undefined>
  readPrivateWorkspaceForTeamMember: (
    workspaceId: string,
    userId: string,
  ) => Promise<PrefilledWorkspaceRecord | undefined>
  readPrivateWorkspaceByTarget: (
    privateTeamId: string,
    privateProjectId: string | null,
  ) => Promise<PrefilledWorkspaceRecord | undefined>
  recordFirstRunForHolder: (
    workspaceId: string,
    holderUserId: string,
  ) => Promise<PrefilledWorkspaceRecord | undefined>
  recordFirstRunForOperator: (
    workspaceId: string,
  ) => Promise<PrefilledWorkspaceRecord | undefined>
  recordFirstRunForPrivateTeamMember: (
    workspaceId: string,
    userId: string,
  ) => Promise<PrefilledWorkspaceRecord | undefined>
}>

const insertChildren = async (
  db: D1Database,
  workspaceId: string,
  record: PrefilledWorkspaceRecord,
): Promise<void> => {
  const statements: Array<D1PreparedStatement> = []

  record.seededMemory.forEach((entry, index) => {
    statements.push(
      db
        .prepare(
          `INSERT INTO prefilled_workspace_seeded_memory
            (workspace_id, position, label, value, public_source_ref)
           VALUES (?, ?, ?, ?, ?)`,
        )
        .bind(
          workspaceId,
          index,
          entry.label,
          entry.value,
          entry.publicSourceRef,
        ),
    )
  })

  record.starterWorkflows.forEach((workflow, index) => {
    statements.push(
      db
        .prepare(
          `INSERT INTO prefilled_workspace_starter_workflows
            (workspace_id, position, title, description, outcome_kind, status)
           VALUES (?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          workspaceId,
          index,
          workflow.title,
          workflow.description,
          workflow.outcomeKind,
          workflow.status,
        ),
    )
  })

  if (statements.length > 0) {
    await db.batch(statements)
  }
}

const loadWorkspace = async (
  db: D1Database,
  whereClause: string,
  binds: ReadonlyArray<unknown>,
): Promise<PrefilledWorkspaceRecord | undefined> => {
  const workspace = await db
    .prepare(
      `SELECT id, holder_user_id, holder_ref, project_name, status,
              access_mode, private_team_id, private_project_id,
              intro_receipt_json, invited_at, first_viewed_at,
              first_claimed_at, first_run_at, last_viewed_at,
              revisit_count, created_at, updated_at
         FROM prefilled_workspaces
        WHERE ${whereClause}
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(...binds)
    .first<WorkspaceRow>()

  if (workspace === null) {
    return undefined
  }

  const [memory, workflows] = await Promise.all([
    db
      .prepare(
        `SELECT workspace_id, label, value, public_source_ref
           FROM prefilled_workspace_seeded_memory
          WHERE workspace_id = ?
          ORDER BY position ASC`,
      )
      .bind(workspace.id)
      .all<SeededMemoryRow>(),
    db
      .prepare(
        `SELECT workspace_id, title, description, outcome_kind, status
           FROM prefilled_workspace_starter_workflows
          WHERE workspace_id = ?
          ORDER BY position ASC`,
      )
      .bind(workspace.id)
      .all<StarterWorkflowRow>(),
  ])

  return recordFromRows(workspace, memory.results, workflows.results)
}

const trackHolderWorkspaceView = async (
  db: D1Database,
  workspaceId: string,
  holderUserId: string,
  nowIso: string,
): Promise<void> => {
  await db
    .prepare(
      `UPDATE prefilled_workspaces
          SET holder_user_id = COALESCE(holder_user_id, ?),
              status = CASE
                WHEN holder_user_id IS NULL AND status = 'invited' THEN 'active'
                ELSE status
              END,
              first_viewed_at = COALESCE(first_viewed_at, ?),
              first_claimed_at = CASE
                WHEN holder_user_id IS NULL AND status = 'invited'
                THEN COALESCE(first_claimed_at, ?)
                ELSE first_claimed_at
              END,
              last_viewed_at = ?,
              revisit_count = CASE
                WHEN first_viewed_at IS NULL THEN revisit_count
                ELSE revisit_count + 1
              END,
              updated_at = ?
        WHERE id = ?
          AND archived_at IS NULL
          AND (
            holder_user_id = ?
            OR (holder_user_id IS NULL AND status = 'invited')
          )`,
    )
    .bind(
      holderUserId,
      nowIso,
      nowIso,
      nowIso,
      nowIso,
      workspaceId,
      holderUserId,
    )
    .run()
}

const trackWorkspaceView = async (
  db: D1Database,
  workspaceId: string,
  nowIso: string,
): Promise<void> => {
  await db
    .prepare(
      `UPDATE prefilled_workspaces
          SET first_viewed_at = COALESCE(first_viewed_at, ?),
              last_viewed_at = ?,
              revisit_count = CASE
                WHEN first_viewed_at IS NULL THEN revisit_count
                ELSE revisit_count + 1
              END,
              updated_at = ?
        WHERE id = ?
          AND archived_at IS NULL`,
    )
    .bind(nowIso, nowIso, nowIso, workspaceId)
    .run()
}

const privateTeamMemberWhereClause = `
  id = ?
  AND access_mode = 'private_team'
  AND private_team_id IS NOT NULL
  AND EXISTS (
    SELECT 1
      FROM team_memberships
      INNER JOIN teams ON teams.id = team_memberships.team_id
     WHERE team_memberships.team_id = prefilled_workspaces.private_team_id
       AND team_memberships.user_id = ?
       AND team_memberships.status = 'active'
       AND teams.status = 'active'
       AND teams.archived_at IS NULL
  )
  AND (
    private_project_id IS NULL
    OR EXISTS (
      SELECT 1
        FROM team_projects
       WHERE team_projects.id = prefilled_workspaces.private_project_id
         AND team_projects.team_id = prefilled_workspaces.private_team_id
         AND team_projects.status = 'active'
         AND team_projects.archived_at IS NULL
    )
  )`

const recordFirstRun = async (
  db: D1Database,
  workspaceId: string,
  nowIso: string,
  holderUserId?: string,
): Promise<void> => {
  const holderClause =
    holderUserId === undefined ? '' : 'AND holder_user_id = ?'
  const statement = db.prepare(
    `UPDATE prefilled_workspaces
        SET first_run_at = COALESCE(first_run_at, ?),
            updated_at = ?
      WHERE id = ?
        AND archived_at IS NULL
        ${holderClause}`,
  )

  await (holderUserId === undefined
    ? statement.bind(nowIso, nowIso, workspaceId)
    : statement.bind(nowIso, nowIso, workspaceId, holderUserId)
  ).run()
}

export const makePrefilledWorkspaceService = (
  db: D1Database,
  runtime: PrefilledWorkspaceRuntime = systemPrefilledWorkspaceRuntime,
): PrefilledWorkspaceServiceShape => ({
  createWorkspace: async input => {
    const record = makePrefilledWorkspaceRecord(input, runtime)

    await db
      .prepare(
        `INSERT INTO prefilled_workspaces
          (id, holder_user_id, holder_ref, project_name, status,
           access_mode, private_team_id, private_project_id, intro_receipt_json,
           invited_at, first_viewed_at, first_claimed_at, first_run_at,
           last_viewed_at, revisit_count, created_at, updated_at, archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        record.id,
        record.holderUserId,
        record.holderRef,
        record.projectName,
        record.status,
        record.accessMode,
        record.privateTeamId,
        record.privateProjectId,
        JSON.stringify(record.introReceipt),
        record.engagement.invitedAt,
        record.engagement.firstViewedAt,
        record.engagement.firstClaimedAt,
        record.engagement.firstRunAt,
        record.engagement.lastViewedAt,
        record.engagement.revisitCount,
        record.createdAt,
        record.updatedAt,
      )
      .run()

    await insertChildren(db, record.id, record)

    return record
  },
  readWorkspace: async workspaceId =>
    loadWorkspace(db, 'id = ?', [workspaceId]),
  readWorkspaceForHolder: async (workspaceId, holderUserId) =>
    loadWorkspace(db, 'id = ? AND holder_user_id = ?', [
      workspaceId,
      holderUserId,
    ]),
  readOrClaimWorkspaceForHolder: async (workspaceId, holderUserId) => {
    const nowIso = runtime.nowIso()
    await trackHolderWorkspaceView(db, workspaceId, holderUserId, nowIso)

    return loadWorkspace(db, 'id = ? AND holder_user_id = ?', [
      workspaceId,
      holderUserId,
    ])
  },
  readPrivateWorkspaceForTeamMember: async (workspaceId, userId) => {
    const nowIso = runtime.nowIso()
    const record = await loadWorkspace(db, privateTeamMemberWhereClause, [
      workspaceId,
      userId,
    ])

    if (record === undefined) {
      return undefined
    }

    await trackWorkspaceView(db, workspaceId, nowIso)

    return loadWorkspace(db, privateTeamMemberWhereClause, [
      workspaceId,
      userId,
    ])
  },
  readPrivateWorkspaceByTarget: async (privateTeamId, privateProjectId) =>
    loadWorkspace(
      db,
      `access_mode = 'private_team'
       AND private_team_id = ?
       AND COALESCE(private_project_id, '') = COALESCE(?, '')`,
      [privateTeamId, privateProjectId],
    ),
  recordFirstRunForHolder: async (workspaceId, holderUserId) => {
    const nowIso = runtime.nowIso()
    await recordFirstRun(db, workspaceId, nowIso, holderUserId)

    return loadWorkspace(db, 'id = ? AND holder_user_id = ?', [
      workspaceId,
      holderUserId,
    ])
  },
  recordFirstRunForOperator: async workspaceId => {
    const nowIso = runtime.nowIso()
    await recordFirstRun(db, workspaceId, nowIso)

    return loadWorkspace(db, 'id = ?', [workspaceId])
  },
  recordFirstRunForPrivateTeamMember: async (workspaceId, userId) => {
    const nowIso = runtime.nowIso()
    const record = await loadWorkspace(db, privateTeamMemberWhereClause, [
      workspaceId,
      userId,
    ])

    if (record === undefined) {
      return undefined
    }

    await recordFirstRun(db, workspaceId, nowIso)

    return loadWorkspace(db, privateTeamMemberWhereClause, [
      workspaceId,
      userId,
    ])
  },
})
