import { Schema as S } from 'effect'

import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

// Prefilled project workspace primitive (Epic C / C1).
//
// A reusable onboarding unit a prospect or holder is invited into instead of a
// blank chat: a named project + seeded grounded memory (public-source refs
// only) + 1-3 one-click starter accepted-outcome workflows + an intro receipt.
//
// PUBLIC-SAFE / COMPLIANCE INVARIANT: everything modeled here is seeded from
// public data only. No private account material, secrets, credentials, raw
// prompts, wallet data, or individual people's names belong in any column. The
// seeded-memory and starter-workflow projections are public-safe by
// construction; the public projection helper strips operator-only fields.

export const WorkspaceStatus = S.Literals([
  'draft',
  'invited',
  'active',
  'archived',
])
export type WorkspaceStatus = typeof WorkspaceStatus.Type

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
  createdAt: string
  updatedAt: string
}>

type WorkspaceRow = Readonly<{
  id: string
  holder_user_id: string | null
  holder_ref: string
  project_name: string
  status: WorkspaceStatus
  intro_receipt_json: string
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
  holderRef?: string | undefined
  holderUserId?: string | null | undefined
  introReceipt: IntroReceipt
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
    id: runtime.makeId('workspace'),
    holderUserId: input.holderUserId ?? null,
    holderRef,
    projectName,
    status: input.status ?? 'draft',
    seededMemory: normalizeSeededMemory(input.seededMemory ?? []),
    starterWorkflows: normalizeStarterWorkflows(input.starterWorkflows ?? []),
    introReceipt: normalizeIntroReceipt(input.introReceipt),
    createdAt: now,
    updatedAt: now,
  }
}

const parseIntroReceipt = (json: string): IntroReceipt => {
  try {
    const parsed: unknown = JSON.parse(json)
    return S.decodeUnknownSync(IntroReceipt)(parsed)
  } catch {
    return { summary: '', publicSourceRefs: [] }
  }
}

const recordFromRows = (
  workspace: WorkspaceRow,
  memoryRows: ReadonlyArray<SeededMemoryRow>,
  workflowRows: ReadonlyArray<StarterWorkflowRow>,
): PrefilledWorkspaceRecord => ({
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
  createdAt: workspace.created_at,
  updatedAt: workspace.updated_at,
})

// Public-safe projection: the holder-facing view. The whole record is
// public-data-only by invariant, so this drops only operator-internal fields
// such as the holderUserId/holderRef binding while keeping seeded-memory
// provenance refs intact.
export type PrefilledWorkspacePublicProjection = Readonly<{
  id: string
  projectName: string
  status: WorkspaceStatus
  seededMemory: ReadonlyArray<SeededMemoryEntry>
  starterWorkflows: ReadonlyArray<StarterWorkflow>
  introReceipt: IntroReceipt
}>

export const toPublicProjection = (
  record: PrefilledWorkspaceRecord,
): PrefilledWorkspacePublicProjection => ({
  id: record.id,
  projectName: record.projectName,
  status: record.status,
  seededMemory: record.seededMemory,
  starterWorkflows: record.starterWorkflows,
  introReceipt: record.introReceipt,
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
              intro_receipt_json, created_at, updated_at
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
           intro_receipt_json, created_at, updated_at, archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        record.id,
        record.holderUserId,
        record.holderRef,
        record.projectName,
        record.status,
        JSON.stringify(record.introReceipt),
        record.createdAt,
        record.updatedAt,
      )
      .run()

    await insertChildren(db, record.id, record)

    return record
  },
  readWorkspace: async workspaceId => loadWorkspace(db, 'id = ?', [workspaceId]),
  readWorkspaceForHolder: async (workspaceId, holderUserId) =>
    loadWorkspace(db, 'id = ? AND holder_user_id = ?', [
      workspaceId,
      holderUserId,
    ]),
})
