import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENT_DEFINITION_RUN_LIVE_ATTACHMENT_FORBIDDEN_KEYS,
  AGENT_DEFINITION_RUN_LIVE_DO_INJECTED_SERVICE_REFS,
  AGENT_DEFINITION_RUN_LIVE_DO_MIGRATIONS,
  AGENT_DEFINITION_RUN_LIVE_SURFACE_SPIKE,
  AgentDefinitionRunLiveSocketAttachment,
  AgentDefinitionRunLiveSurfaceSpike,
  agentDefinitionRunLiveDoName,
  allAgentDefinitionRunLiveDoMigrationStatements,
  decideAgentDefinitionRunLiveSurface,
  liveSocketAttachmentHasForbiddenKey,
  nextAgentDefinitionRunLiveDoAlarmAt,
} from './agent-definition-live-surface-spike'

describe('agent definition per-run live surface spike', () => {
  test('keeps Durable Streams as the default until WS-10 has a client-facing live channel', () => {
    expect(S.decodeUnknownSync(AgentDefinitionRunLiveSurfaceSpike)(
      AGENT_DEFINITION_RUN_LIVE_SURFACE_SPIKE,
    )).toEqual(AGENT_DEFINITION_RUN_LIVE_SURFACE_SPIKE)
    expect(AGENT_DEFINITION_RUN_LIVE_SURFACE_SPIKE).toMatchObject({
      adoptedByDefault: false,
      currentDefaultTransport: 'durable_streams',
      futureCandidateTransport: 'thin_do_live_surface',
    })

    expect(decideAgentDefinitionRunLiveSurface({
      operatorEnabledThinDoLiveSurface: false,
      ws10StatusSpineClientLiveChannel: false,
    })).toMatchObject({
      adopted: false,
      durableStreamsRemainDefault: true,
      transport: 'durable_streams',
    })

    expect(decideAgentDefinitionRunLiveSurface({
      operatorEnabledThinDoLiveSurface: true,
      ws10StatusSpineClientLiveChannel: false,
    })).toMatchObject({
      adopted: false,
      durableStreamsRemainDefault: true,
      transport: 'durable_streams',
    })

    expect(decideAgentDefinitionRunLiveSurface({
      operatorEnabledThinDoLiveSurface: true,
      ws10StatusSpineClientLiveChannel: true,
    })).toMatchObject({
      adopted: true,
      durableStreamsRemainDefault: true,
      transport: 'thin_do_live_surface',
    })
  })

  test('names the future Durable Object deterministically per owner run', () => {
    const first = agentDefinitionRunLiveDoName({
      ownerAgentUserId: 'agent-user_123',
      runId: 'agent_definition_run.abc',
    })
    const again = agentDefinitionRunLiveDoName({
      ownerAgentUserId: 'agent-user_123',
      runId: 'agent_definition_run.abc',
    })
    const otherRun = agentDefinitionRunLiveDoName({
      ownerAgentUserId: 'agent-user_123',
      runId: 'agent_definition_run.def',
    })
    const otherOwner = agentDefinitionRunLiveDoName({
      ownerAgentUserId: 'agent-user_456',
      runId: 'agent_definition_run.abc',
    })

    expect(first).toEqual(again)
    expect(first).toMatchObject({
      _tag: 'accepted',
      name: 'agent-definition-run-live:agent-user_123:agent_definition_run.abc',
    })
    expect(otherRun).not.toEqual(first)
    expect(otherOwner).not.toEqual(first)
    expect(agentDefinitionRunLiveDoName({
      ownerAgentUserId: '',
      runId: 'agent_definition_run.abc',
    })).toEqual({
      _tag: 'rejected',
      reason: 'blank_owner_agent_user_id',
    })
    expect(agentDefinitionRunLiveDoName({
      ownerAgentUserId: 'agent-user_123',
      runId: ' ',
    })).toEqual({ _tag: 'rejected', reason: 'blank_run_id' })
  })

  test('pins numbered in-DO SQLite migrations without PRAGMA user_version', () => {
    expect(AGENT_DEFINITION_RUN_LIVE_DO_MIGRATIONS.map(migration => migration.id))
      .toEqual([1, 2])

    const statements = allAgentDefinitionRunLiveDoMigrationStatements()
    const joinedStatements = statements.join('\n')

    expect(joinedStatements).toContain('_sql_schema_migrations')
    expect(joinedStatements).toContain('CREATE TABLE IF NOT EXISTS live_clients')
    expect(joinedStatements).toContain('serialized_attachment_json TEXT NOT NULL')
    expect(joinedStatements).toContain('CREATE TABLE IF NOT EXISTS live_alarm_tasks')
    expect(joinedStatements).not.toMatch(/PRAGMA\s+user_version/i)
  })

  test('models hibernation-safe socket attachments without model-visible private payloads', () => {
    const attachment = S.decodeUnknownSync(AgentDefinitionRunLiveSocketAttachment)({
      schema: 'openagents.agent_definition_run_live_socket_attachment.v1',
      ownerAgentUserId: 'agent-user_123',
      definitionId: 'agent_definition.alpha',
      runId: 'agent_definition_run.abc',
      clientId: 'client.khala-code-desktop.1',
      watchIntentRef: 'watch.agent_definition_run.live.operator_panel',
      lastAckedSequence: 41,
      connectedAtEpochMs: 1_788_567_000_000,
      lastSeenAtEpochMs: 1_788_567_100_000,
    })

    expect(liveSocketAttachmentHasForbiddenKey(attachment)).toBe(false)

    for (const forbiddenKey of AGENT_DEFINITION_RUN_LIVE_ATTACHMENT_FORBIDDEN_KEYS) {
      expect(Object.keys(attachment)).not.toContain(forbiddenKey)
    }

    expect(JSON.stringify(attachment)).not.toMatch(
      /authorization|commentBody|providerPayload|rawBody|rawPrompt|secret|signature|token/i,
    )
  })

  test('documents injected service seams instead of binding runtime logic to the DO shell', () => {
    expect(AGENT_DEFINITION_RUN_LIVE_DO_INJECTED_SERVICE_REFS).toEqual([
      'service.agent_definition_live.auth_verifier',
      'service.agent_definition_live.clock',
      'service.agent_definition_live.event_projector',
      'service.agent_definition_live.run_store',
      'service.agent_definition_live.status_spine_reader',
      'service.agent_definition_live.operator_audit_sink',
    ])
    expect(AGENT_DEFINITION_RUN_LIVE_DO_INJECTED_SERVICE_REFS.join('\n'))
      .not.toMatch(/env\.|OPENAGENTS_DB|INFERENCE_DURABLE_STREAM/)
  })

  test('uses one multiplexed alarm by selecting the earliest persisted task', () => {
    expect(nextAgentDefinitionRunLiveDoAlarmAt([
      {
        taskKind: 'terminal_gc',
        taskRef: 'task.terminal_gc',
        dueAtEpochMs: 3000,
      },
      {
        taskKind: 'client_idle_timeout',
        taskRef: 'task.client_idle_timeout',
        dueAtEpochMs: 1000,
      },
      {
        taskKind: 'run_status_decay',
        taskRef: 'task.run_status_decay',
        dueAtEpochMs: 2000,
      },
      {
        taskKind: 'pending_outbox_flush',
        taskRef: 'task.pending_outbox_flush',
        dueAtEpochMs: 1500,
      },
    ])).toBe(1000)
    expect(nextAgentDefinitionRunLiveDoAlarmAt([])).toBeUndefined()
  })
})
