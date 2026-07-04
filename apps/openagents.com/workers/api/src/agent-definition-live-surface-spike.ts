import { Schema as S } from 'effect'

export const AgentDefinitionRunLiveSurfaceTransport = S.Literals([
  'durable_streams',
  'thin_do_live_surface',
])
export type AgentDefinitionRunLiveSurfaceTransport =
  typeof AgentDefinitionRunLiveSurfaceTransport.Type

export const AgentDefinitionRunLiveDoAlarmTaskKind = S.Literals([
  'client_idle_timeout',
  'pending_outbox_flush',
  'run_status_decay',
  'terminal_gc',
])
export type AgentDefinitionRunLiveDoAlarmTaskKind =
  typeof AgentDefinitionRunLiveDoAlarmTaskKind.Type

export const AgentDefinitionRunLiveSocketAttachment = S.Struct({
  schema: S.Literals([
    'openagents.agent_definition_run_live_socket_attachment.v1',
  ]),
  ownerAgentUserId: S.String,
  definitionId: S.String,
  runId: S.String,
  clientId: S.String,
  watchIntentRef: S.String,
  lastAckedSequence: S.Number,
  connectedAtEpochMs: S.Number,
  lastSeenAtEpochMs: S.Number,
})
export type AgentDefinitionRunLiveSocketAttachment =
  typeof AgentDefinitionRunLiveSocketAttachment.Type

export const AgentDefinitionRunLiveSurfaceSpike = S.Struct({
  schema: S.Literals(['openagents.agent_definition_run_live_surface_spike.v1']),
  currentDefaultTransport: AgentDefinitionRunLiveSurfaceTransport,
  futureCandidateTransport: AgentDefinitionRunLiveSurfaceTransport,
  durableObjectClassName: S.String,
  wranglerBindingName: S.String,
  adoptedByDefault: S.Boolean,
  adoptionBlockerRefs: S.Array(S.String),
  docsRefs: S.Array(S.String),
  serviceRefs: S.Array(S.String),
  testRefs: S.Array(S.String),
})
export type AgentDefinitionRunLiveSurfaceSpike =
  typeof AgentDefinitionRunLiveSurfaceSpike.Type

export interface AgentDefinitionRunLiveSurfaceDecisionInput {
  readonly ws10StatusSpineClientLiveChannel: boolean
  readonly operatorEnabledThinDoLiveSurface: boolean
}

export interface AgentDefinitionRunLiveSurfaceDecision {
  readonly transport: AgentDefinitionRunLiveSurfaceTransport
  readonly adopted: boolean
  readonly durableStreamsRemainDefault: boolean
  readonly blockerRefs: ReadonlyArray<string>
}

export interface AgentDefinitionRunLiveDoNameInput {
  readonly ownerAgentUserId: string
  readonly runId: string
}

export type AgentDefinitionRunLiveDoNameDecision =
  | {
      readonly _tag: 'accepted'
      readonly name: string
    }
  | {
      readonly _tag: 'rejected'
      readonly reason: 'blank_owner_agent_user_id' | 'blank_run_id'
    }

export interface AgentDefinitionRunLiveDoMigration {
  readonly id: number
  readonly statements: ReadonlyArray<string>
}

export interface AgentDefinitionRunLiveDoAlarmTask {
  readonly taskKind: AgentDefinitionRunLiveDoAlarmTaskKind
  readonly taskRef: string
  readonly dueAtEpochMs: number
}

export const AGENT_DEFINITION_RUN_LIVE_DO_NAME_PREFIX =
  'agent-definition-run-live'

export const AGENT_DEFINITION_RUN_LIVE_DO_WRANGLER_BINDING =
  'AGENT_DEFINITION_RUN_LIVE'

export const AGENT_DEFINITION_RUN_LIVE_DO_CLASS_NAME =
  'AgentDefinitionRunLiveObject'

export const AGENT_DEFINITION_RUN_LIVE_DO_INJECTED_SERVICE_REFS = [
  'service.agent_definition_live.auth_verifier',
  'service.agent_definition_live.clock',
  'service.agent_definition_live.event_projector',
  'service.agent_definition_live.run_store',
  'service.agent_definition_live.status_spine_reader',
  'service.agent_definition_live.operator_audit_sink',
] as const

export const AGENT_DEFINITION_RUN_LIVE_ATTACHMENT_FORBIDDEN_KEYS = [
  'authorization',
  'body',
  'commentBody',
  'email',
  'prompt',
  'providerPayload',
  'rawBody',
  'rawPrompt',
  'secret',
  'signature',
  'token',
] as const

export const AGENT_DEFINITION_RUN_LIVE_DO_MIGRATIONS = [
  {
    id: 1,
    statements: [
      `CREATE TABLE IF NOT EXISTS _sql_schema_migrations (
  id INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
)`,
      `CREATE TABLE IF NOT EXISTS live_clients (
  client_id TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  definition_id TEXT NOT NULL,
  run_id TEXT NOT NULL,
  watch_intent_ref TEXT NOT NULL,
  last_acked_sequence INTEGER NOT NULL,
  connected_at_ms INTEGER NOT NULL,
  last_seen_at_ms INTEGER NOT NULL,
  serialized_attachment_json TEXT NOT NULL
)`,
      `CREATE TABLE IF NOT EXISTS live_events (
  sequence INTEGER PRIMARY KEY,
  status_event_ref TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL
)`,
      `CREATE TABLE IF NOT EXISTS live_alarm_tasks (
  task_ref TEXT PRIMARY KEY,
  task_kind TEXT NOT NULL,
  due_at_ms INTEGER NOT NULL,
  payload_json TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
)`,
    ],
  },
  {
    id: 2,
    statements: [
      `CREATE INDEX IF NOT EXISTS idx_live_clients_run
  ON live_clients(owner_agent_user_id, run_id)`,
      `CREATE INDEX IF NOT EXISTS idx_live_alarm_tasks_due
  ON live_alarm_tasks(due_at_ms, task_kind)`,
    ],
  },
] as const satisfies ReadonlyArray<AgentDefinitionRunLiveDoMigration>

export const AGENT_DEFINITION_RUN_LIVE_SURFACE_SPIKE = {
  schema: 'openagents.agent_definition_run_live_surface_spike.v1',
  currentDefaultTransport: 'durable_streams',
  futureCandidateTransport: 'thin_do_live_surface',
  durableObjectClassName: AGENT_DEFINITION_RUN_LIVE_DO_CLASS_NAME,
  wranglerBindingName: AGENT_DEFINITION_RUN_LIVE_DO_WRANGLER_BINDING,
  adoptedByDefault: false,
  adoptionBlockerRefs: [
    'blocker.ba_g3.ws10_client_facing_live_channel_missing',
    'blocker.ba_g3.operator_enablement_missing',
    'blocker.ba_g3.worker_binding_not_declared',
  ],
  docsRefs: [
    'docs/fable/2026-07-04-background-agent-per-run-live-surface-thin-do.md',
    'docs/fable/ROADMAP_BACKGROUND_AGENTS.md#ws-g--client-surfaces-harvest-h6--h8-definitions-audit-45',
    'docs/fable/ROADMAP.md#ws-10--one-status-spine-orca-p2',
  ],
  serviceRefs: [...AGENT_DEFINITION_RUN_LIVE_DO_INJECTED_SERVICE_REFS],
  testRefs: [
    'apps/openagents.com/workers/api/src/agent-definition-live-surface-spike.test.ts',
  ],
} as const satisfies AgentDefinitionRunLiveSurfaceSpike

export const decideAgentDefinitionRunLiveSurface = (
  input: AgentDefinitionRunLiveSurfaceDecisionInput,
): AgentDefinitionRunLiveSurfaceDecision => {
  const blockerRefs: Array<string> = []

  if (!input.ws10StatusSpineClientLiveChannel) {
    blockerRefs.push('blocker.ba_g3.ws10_client_facing_live_channel_missing')
  }

  if (!input.operatorEnabledThinDoLiveSurface) {
    blockerRefs.push('blocker.ba_g3.operator_enablement_missing')
  }

  const adopted =
    input.ws10StatusSpineClientLiveChannel &&
    input.operatorEnabledThinDoLiveSurface

  return {
    transport: adopted ? 'thin_do_live_surface' : 'durable_streams',
    adopted,
    durableStreamsRemainDefault: true,
    blockerRefs,
  }
}

export const agentDefinitionRunLiveDoName = (
  input: AgentDefinitionRunLiveDoNameInput,
): AgentDefinitionRunLiveDoNameDecision => {
  const ownerSegment = stableDoNameSegment(input.ownerAgentUserId)

  if (ownerSegment === undefined) {
    return { _tag: 'rejected', reason: 'blank_owner_agent_user_id' }
  }

  const runSegment = stableDoNameSegment(input.runId)

  if (runSegment === undefined) {
    return { _tag: 'rejected', reason: 'blank_run_id' }
  }

  return {
    _tag: 'accepted',
    name: `${AGENT_DEFINITION_RUN_LIVE_DO_NAME_PREFIX}:${ownerSegment}:${runSegment}`,
  }
}

export const allAgentDefinitionRunLiveDoMigrationStatements =
  (): ReadonlyArray<string> =>
    AGENT_DEFINITION_RUN_LIVE_DO_MIGRATIONS.flatMap(
      migration => migration.statements,
    )

export const nextAgentDefinitionRunLiveDoAlarmAt = (
  tasks: ReadonlyArray<AgentDefinitionRunLiveDoAlarmTask>,
): number | undefined =>
  tasks
    .map(task => task.dueAtEpochMs)
    .filter(dueAtEpochMs => Number.isFinite(dueAtEpochMs))
    .sort((left, right) => left - right)[0]

export const liveSocketAttachmentHasForbiddenKey = (
  attachment: AgentDefinitionRunLiveSocketAttachment,
): boolean =>
  Object.keys(attachment).some(key =>
    AGENT_DEFINITION_RUN_LIVE_ATTACHMENT_FORBIDDEN_KEYS.includes(
      key as (typeof AGENT_DEFINITION_RUN_LIVE_ATTACHMENT_FORBIDDEN_KEYS)[number],
    ),
  )

const stableDoNameSegment = (value: string): string | undefined => {
  const trimmed = value.trim()

  if (trimmed === '') {
    return undefined
  }

  return encodeURIComponent(trimmed)
}
